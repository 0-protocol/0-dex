//! P2P Gossip Network for distributing 0-lang graphs.
//!
//! Supports topic sharding:
//!   - `0-dex-intents`   — agents publish raw signed intents
//!   - `0-dex-solutions` — solvers publish verified MatchProof bundles
//!   - `0-dex-mempool`   — legacy single-topic mode (subscribed by all for backward compat)

use libp2p::{
    futures::StreamExt,
    gossipsub, mdns, noise, swarm::NetworkBehaviour, swarm::SwarmEvent, tcp, yamux, Swarm,
    identity, PeerId, Multiaddr,
};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{info, warn};

use crate::node_mode::NodeMode;

pub const TOPIC_INTENTS: &str = "0-dex-intents";
pub const TOPIC_SOLUTIONS: &str = "0-dex-solutions";
pub const TOPIC_MEMPOOL: &str = "0-dex-mempool";

#[derive(NetworkBehaviour)]
pub struct DexBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
}

pub struct GossipNode {
    swarm: Swarm<DexBehaviour>,
    topics: HashMap<String, gossipsub::IdentTopic>,
    /// Default publish topic based on node mode
    publish_topic_name: String,
    outbound_rx: mpsc::Receiver<Vec<u8>>,
    inbound_tx: mpsc::Sender<Vec<u8>>,
}

impl GossipNode {
    pub fn new(mode: NodeMode) -> Result<(Self, mpsc::Sender<Vec<u8>>, mpsc::Receiver<Vec<u8>>), Box<dyn std::error::Error>> {
        let id_keys = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(id_keys.public());
        info!("0-dex local peer id: {peer_id}");

        let (outbound_tx, outbound_rx) = mpsc::channel::<Vec<u8>>(100);
        let (inbound_tx, inbound_rx) = mpsc::channel::<Vec<u8>>(100);

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(id_keys)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|key| {
                let message_id_fn = |message: &gossipsub::Message| {
                    let mut s = DefaultHasher::new();
                    message.data.hash(&mut s);
                    gossipsub::MessageId::from(s.finish().to_string())
                };

                let gossipsub_config = gossipsub::ConfigBuilder::default()
                    .heartbeat_interval(Duration::from_secs(10))
                    .validation_mode(gossipsub::ValidationMode::Strict)
                    .message_id_fn(message_id_fn)
                    .build()
                    .expect("Valid config");

                let gossipsub = gossipsub::Behaviour::new(
                    gossipsub::MessageAuthenticity::Signed(key.clone()),
                    gossipsub_config,
                ).expect("Valid config");

                let mdns = mdns::tokio::Behaviour::new(
                    mdns::Config::default(),
                    key.public().to_peer_id(),
                ).expect("Valid config");

                DexBehaviour { gossipsub, mdns }
            })?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        let mut topics = HashMap::new();
        for name in [TOPIC_INTENTS, TOPIC_SOLUTIONS, TOPIC_MEMPOOL] {
            let topic = gossipsub::IdentTopic::new(name);
            topics.insert(name.to_string(), topic);
        }

        // Subscribe based on node mode
        let publish_topic_name = match mode {
            NodeMode::Agent => {
                // Agent listens on solutions + legacy mempool, publishes to intents
                swarm.behaviour_mut().gossipsub.subscribe(&topics[TOPIC_SOLUTIONS])?;
                swarm.behaviour_mut().gossipsub.subscribe(&topics[TOPIC_MEMPOOL])?;
                swarm.behaviour_mut().gossipsub.subscribe(&topics[TOPIC_INTENTS])?;
                TOPIC_INTENTS.to_string()
            }
            NodeMode::Solver => {
                // Solver listens on intents + legacy mempool, publishes to solutions
                swarm.behaviour_mut().gossipsub.subscribe(&topics[TOPIC_INTENTS])?;
                swarm.behaviour_mut().gossipsub.subscribe(&topics[TOPIC_MEMPOOL])?;
                swarm.behaviour_mut().gossipsub.subscribe(&topics[TOPIC_SOLUTIONS])?;
                TOPIC_SOLUTIONS.to_string()
            }
        };

        info!("Subscribed to gossip topics (mode={mode}), default publish topic: {publish_topic_name}");

        Ok((
            Self { swarm, topics, publish_topic_name, outbound_rx, inbound_tx },
            outbound_tx,
            inbound_rx,
        ))
    }

    pub fn listen_on(&mut self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let multiaddr: Multiaddr = addr.parse()?;
        self.swarm.listen_on(multiaddr)?;
        Ok(())
    }

    pub async fn run(mut self) {
        loop {
            tokio::select! {
                event = self.swarm.select_next_some() => {
                    match event {
                        SwarmEvent::Behaviour(DexBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                            for (peer_id, multiaddr) in list {
                                info!("mDNS discovered peer: {peer_id} @ {multiaddr}");
                                self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                            }
                        }
                        SwarmEvent::Behaviour(DexBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                            for (peer_id, _) in list {
                                self.swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                            }
                        }
                        SwarmEvent::Behaviour(DexBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                            propagation_source: peer_id,
                            message_id: id,
                            message,
                        })) => {
                            let topic_str = message.topic.as_str();
                            info!("Got message id={id} from peer={peer_id} topic={topic_str}");
                            crate::metrics::INTENTS_RECEIVED.inc();
                            let _ = self.inbound_tx.send(message.data).await;
                        }
                        SwarmEvent::NewListenAddr { address, .. } => {
                            info!("0-dex listening on {address}");
                        }
                        _ => {}
                    }
                }
                Some(payload) = self.outbound_rx.recv() => {
                    if let Some(topic) = self.topics.get(&self.publish_topic_name) {
                        if let Err(e) = {
                            crate::metrics::INTENTS_PUBLISHED.inc();
                            self.swarm.behaviour_mut().gossipsub.publish(topic.clone(), payload)
                        } {
                            warn!("Failed to publish to {}: {:?}", self.publish_topic_name, e);
                        }
                    }
                }
            }
        }
    }
}
