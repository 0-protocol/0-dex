//! P2P Gossip Network for distributing 0-lang graphs.
//!
//! Supports optional token-pair topic sharding: intents are published to both the
//! global `0-dex-mempool` topic and a pair-specific topic like `0-dex/0xAAA-0xBBB`.

use futures::StreamExt;
use libp2p::{
    gossipsub, identity, mdns, noise, swarm::NetworkBehaviour, swarm::SwarmEvent, tcp, yamux,
    Multiaddr, PeerId, Swarm,
};
use sha3::{Digest, Keccak256};
use std::collections::HashSet;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

#[derive(NetworkBehaviour)]
pub struct DexBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
}

pub struct GossipNode {
    swarm: Swarm<DexBehaviour>,
    global_topic: gossipsub::IdentTopic,
    pair_topics: HashSet<String>,
    receiver: mpsc::Receiver<Vec<u8>>,
    inbound_tx: mpsc::Sender<Vec<u8>>,
}

impl GossipNode {
    pub fn new(
    ) -> Result<(Self, mpsc::Sender<Vec<u8>>, mpsc::Receiver<Vec<u8>>), Box<dyn std::error::Error>>
    {
        let id_keys = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(id_keys.public());
        info!("0-dex local peer id: {peer_id}");

        let (outbound_tx, outbound_rx) = mpsc::channel(100);
        let (inbound_tx, inbound_rx) = mpsc::channel(256);

        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(id_keys)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|key| {
                let message_id_fn = |message: &gossipsub::Message| {
                    let mut hasher = Keccak256::new();
                    hasher.update(&message.data);
                    let digest = hasher.finalize();
                    gossipsub::MessageId::from(hex::encode(digest))
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
                )
                .expect("Valid config");

                let mdns =
                    mdns::tokio::Behaviour::new(mdns::Config::default(), key.public().to_peer_id())
                        .expect("Valid config");

                DexBehaviour { gossipsub, mdns }
            })?
            .with_swarm_config(|c| c.with_idle_connection_timeout(Duration::from_secs(60)))
            .build();

        let global_topic = gossipsub::IdentTopic::new("0-dex-mempool");
        swarm.behaviour_mut().gossipsub.subscribe(&global_topic)?;

        Ok((
            Self {
                swarm,
                global_topic,
                pair_topics: HashSet::new(),
                receiver: outbound_rx,
                inbound_tx: inbound_tx.clone(),
            },
            outbound_tx,
            inbound_rx,
        ))
    }

    pub fn subscribe_pair(&mut self, pair_id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let topic_name = format!("0-dex/{pair_id}");
        if self.pair_topics.contains(&topic_name) {
            return Ok(());
        }
        let topic = gossipsub::IdentTopic::new(&topic_name);
        self.swarm.behaviour_mut().gossipsub.subscribe(&topic)?;
        self.pair_topics.insert(topic_name.clone());
        info!("Subscribed to pair topic: {topic_name}");
        Ok(())
    }

    pub fn listen_on(&mut self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let multiaddr: Multiaddr = addr.parse()?;
        self.swarm.listen_on(multiaddr)?;
        Ok(())
    }

    pub fn broadcast_graph(
        &mut self,
        graph_payload: Vec<u8>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if let Err(e) = self
            .swarm
            .behaviour_mut()
            .gossipsub
            .publish(self.global_topic.clone(), graph_payload.clone())
        {
            error!("Error publishing to global topic: {:?}", e);
            return Err(Box::new(e));
        }

        if let Ok(intent) = serde_json::from_slice::<serde_json::Value>(&graph_payload) {
            if let Some(pair_topic) = derive_pair_topic(&intent) {
                if self.pair_topics.contains(&pair_topic) {
                    let topic = gossipsub::IdentTopic::new(&pair_topic);
                    if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(topic, graph_payload) {
                        warn!("Failed to publish to pair topic {pair_topic}: {e:?}");
                    }
                }
            }
        }

        Ok(())
    }

    pub async fn run(mut self) {
        loop {
            tokio::select! {
                Some(payload) = self.receiver.recv() => {
                    if let Err(e) = self.broadcast_graph(payload) {
                        error!("Failed to publish outbound gossip payload: {e}");
                    }
                }
                event = self.swarm.select_next_some() => {
                    match event {
                        SwarmEvent::Behaviour(DexBehaviourEvent::Mdns(mdns::Event::Discovered(list))) => {
                            for (peer_id, multiaddr) in list {
                                info!("mDNS discovered a new peer: {peer_id} @ {multiaddr}");
                                self.swarm.behaviour_mut().gossipsub.add_explicit_peer(&peer_id);
                            }
                        }
                        SwarmEvent::Behaviour(DexBehaviourEvent::Mdns(mdns::Event::Expired(list))) => {
                            for (peer_id, _multiaddr) in list {
                                info!("mDNS discover peer has expired: {peer_id}");
                                self.swarm.behaviour_mut().gossipsub.remove_explicit_peer(&peer_id);
                            }
                        }
                        SwarmEvent::Behaviour(DexBehaviourEvent::Gossipsub(gossipsub::Event::Message {
                            propagation_source: peer_id,
                            message_id: id,
                            message,
                        })) => {
                            info!("Got intent graph with id: {id} from peer: {peer_id}");
                            let _ = self.inbound_tx.send(message.data).await;
                        }
                        SwarmEvent::NewListenAddr { address, .. } => {
                            info!("0-dex listening on {address}");
                        }
                        _ => {}
                    }
                }
            }
        }
    }
}

fn derive_pair_topic(value: &serde_json::Value) -> Option<String> {
    let base = value.get("base_token")?.as_str()?.to_ascii_lowercase();
    let quote = value.get("quote_token")?.as_str()?.to_ascii_lowercase();
    let (a, b) = if base < quote {
        (base, quote)
    } else {
        (quote, base)
    };
    Some(format!("0-dex/{a}-{b}"))
}
