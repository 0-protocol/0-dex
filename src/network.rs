//! P2P Gossip Network for distributing 0-lang graphs
//! 
//! Agents use this to broadcast their intents without a centralized orderbook.

use libp2p::{
    gossipsub, mdns, noise, swarm::NetworkBehaviour, swarm::SwarmEvent, tcp, yamux, Swarm,
    identity, PeerId, Multiaddr
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{info, warn, error};

// Define the custom behaviour combining Gossipsub (for messaging) and mDNS (for local peer discovery)
#[derive(NetworkBehaviour)]
pub struct DexBehaviour {
    pub gossipsub: gossipsub::Behaviour,
    pub mdns: mdns::tokio::Behaviour,
}

pub struct GossipNode {
    swarm: Swarm<DexBehaviour>,
    topic: gossipsub::IdentTopic,
    receiver: mpsc::Receiver<Vec<u8>>,
}

impl GossipNode {
    pub fn new() -> Result<(Self, mpsc::Sender<Vec<u8>>, mpsc::Receiver<Vec<u8>>), Box<dyn std::error::Error>> {
        // Create a random PeerId
        let id_keys = identity::Keypair::generate_ed25519();
        let peer_id = PeerId::from(id_keys.public());
        info!("0-dex local peer id: {peer_id}");

        // Create a channel for receiving gossiped graphs
        let (tx, rx) = mpsc::channel(100);

        // Setup the libp2p Swarm
        let mut swarm = libp2p::SwarmBuilder::with_existing_identity(id_keys)
            .with_tokio()
            .with_tcp(
                tcp::Config::default(),
                noise::Config::new,
                yamux::Config::default,
            )?
            .with_behaviour(|key| {
                // To content-address message, we can take the hash of message and use it as an ID.
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

        // Subscribe to the global 0-dex mempool topic
        let topic = gossipsub::IdentTopic::new("0-dex-mempool");
        swarm.behaviour_mut().gossipsub.subscribe(&topic)?;

        Ok((Self { swarm, topic, receiver: mpsc::channel(1).1 }, tx, rx))
    }

    pub fn listen_on(&mut self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let multiaddr: Multiaddr = addr.parse()?;
        self.swarm.listen_on(multiaddr)?;
        Ok(())
    }

    pub fn broadcast_graph(&mut self, graph_payload: Vec<u8>) -> Result<(), Box<dyn std::error::Error>> {
        if let Err(e) = self.swarm.behaviour_mut().gossipsub.publish(self.topic.clone(), graph_payload) {
            error!("Error publishing graph: {:?}", e);
            return Err(Box::new(e));
        }
        Ok(())
    }

    /// Run the network loop. This must be spawned in a Tokio task.
    pub async fn run(mut self, tx: mpsc::Sender<Vec<u8>>) {
        loop {
            match self.swarm.select_next_some().await {
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
                    let _ = tx.send(message.data).await;
                }
                SwarmEvent::NewListenAddr { address, .. } => {
                    info!("0-dex listening on {address}");
                }
                _ => {}
            }
        }
    }
}
