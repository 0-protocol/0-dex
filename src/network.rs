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
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

const DEFAULT_MAX_GOSSIP_BYTES: usize = 48 * 1024;

pub struct GossipConfig {
    pub identity_key_file: Option<PathBuf>,
    pub bootstrap_peers: Vec<Multiaddr>,
    pub max_gossip_bytes: usize,
}

impl Default for GossipConfig {
    fn default() -> Self {
        Self {
            identity_key_file: None,
            bootstrap_peers: Vec::new(),
            max_gossip_bytes: DEFAULT_MAX_GOSSIP_BYTES,
        }
    }
}

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
    max_gossip_bytes: usize,
}

type GossipChannels = (GossipNode, mpsc::Sender<Vec<u8>>, mpsc::Receiver<Vec<u8>>);

impl GossipNode {
    pub fn new(config: GossipConfig) -> Result<GossipChannels, Box<dyn std::error::Error>> {
        let id_keys = load_or_generate_identity(config.identity_key_file.clone())?;
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
        for bootstrap in &config.bootstrap_peers {
            if let Err(e) = swarm.dial(bootstrap.clone()) {
                warn!("Failed to dial bootstrap peer {bootstrap}: {e}");
            } else {
                info!("Dialing bootstrap peer: {bootstrap}");
            }
        }

        Ok((
            Self {
                swarm,
                global_topic,
                pair_topics: HashSet::new(),
                receiver: outbound_rx,
                inbound_tx: inbound_tx.clone(),
                max_gossip_bytes: config.max_gossip_bytes,
            },
            outbound_tx,
            inbound_rx,
        ))
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
                    if let Err(e) = self
                        .swarm
                        .behaviour_mut()
                        .gossipsub
                        .publish(topic, graph_payload)
                    {
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
                            if message.data.len() > self.max_gossip_bytes {
                                warn!(
                                    "Dropping oversized gossip payload: {} bytes (max {})",
                                    message.data.len(),
                                    self.max_gossip_bytes
                                );
                                continue;
                            }
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

fn load_or_generate_identity(
    key_file: Option<PathBuf>,
) -> Result<identity::Keypair, Box<dyn std::error::Error>> {
    if let Some(path) = key_file {
        if path.exists() {
            let encoded = fs::read(&path)?;
            let keypair = identity::Keypair::from_protobuf_encoding(&encoded)?;
            info!("Loaded persistent libp2p identity from {}", path.display());
            return Ok(keypair);
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let keypair = identity::Keypair::generate_ed25519();
        let encoded = keypair.to_protobuf_encoding()?;
        fs::write(&path, encoded)?;
        info!("Generated persistent libp2p identity at {}", path.display());
        return Ok(keypair);
    }
    warn!("No persistent libp2p key file configured; generating ephemeral identity");
    Ok(identity::Keypair::generate_ed25519())
}
