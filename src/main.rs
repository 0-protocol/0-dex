use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{env, path::PathBuf};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, Level};

mod abi;
mod api;
mod crypto;
mod matching;
mod network;
mod protocol;
mod settlement;
mod vm_bridge;

use matching::MatchingEngine;
use network::{GossipConfig, GossipNode};
use protocol::{MatchProof, SignedIntent};
use settlement::SettlementEngine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Initializing 0-dex agent node...");

    let (match_tx, match_rx) = mpsc::channel::<MatchProof>(100);
    let (local_intent_tx, local_intent_rx) = mpsc::channel::<SignedIntent>(256);

    let chain_id = required_u64_env("ZERO_DEX_CHAIN_ID")?;
    let rpc_url = required_env("ZERO_DEX_RPC_URL")?;
    let escrow_address = required_env("ZERO_DEX_ESCROW_ADDRESS")?;
    if escrow_address.eq_ignore_ascii_case("0x0000000000000000000000000000000000000000") {
        return Err("ZERO_DEX_ESCROW_ADDRESS cannot be zero address".into());
    }
    let http_port = env_u16_with_default("ZERO_DEX_HTTP_PORT", 8080)?;
    let listen_addr = env::var("ZERO_DEX_P2P_LISTEN_ADDR")
        .unwrap_or_else(|_| "/ip4/0.0.0.0/tcp/7000".to_string());
    let bootstrap_peers = parse_bootstrap_peers(env::var("ZERO_DEX_BOOTSTRAP_PEERS").ok())?;
    let p2p_key_file = env::var("ZERO_DEX_P2P_KEY_FILE")
        .ok()
        .map(PathBuf::from)
        .or_else(|| Some(PathBuf::from(".zero-dex/p2p_identity.key")));
    let max_gossip_bytes = env_usize_with_default("ZERO_DEX_MAX_GOSSIP_BYTES", 48 * 1024)?;
    let allow_simulation = env::var("ZERO_DEX_ALLOW_SIMULATION")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);

    info!("Starting libp2p gossip network...");
    let gossip_config = GossipConfig {
        identity_key_file: p2p_key_file,
        bootstrap_peers,
        max_gossip_bytes,
    };
    let (mut gossip_node, gossip_tx, gossip_node_rx) = GossipNode::new(gossip_config)?;
    gossip_node.listen_on(&listen_addr)?;

    tokio::spawn(async move {
        gossip_node.run().await;
    });

    info!("Starting Settlement Engine...");
    let settlement_engine = SettlementEngine::new(&rpc_url, chain_id, &escrow_address, match_rx)
        .map_err(|e| format!("Failed to initialize settlement engine: {e}"))?;
    if allow_simulation {
        info!("Settlement simulation mode enabled by ZERO_DEX_ALLOW_SIMULATION=true");
    }
    let settlement_mode = settlement_engine.mode_name().to_string();

    let settlement_handle = tokio::spawn(async move {
        settlement_engine.run().await;
    });

    info!("Starting 0-dex Matching Engine...");
    let mut matching_engine = MatchingEngine::new(match_tx);

    let api_gossip_tx = gossip_tx.clone();
    let api_chain_id = chain_id;
    let api_escrow_address = escrow_address.clone();
    let api_local_intent_tx = local_intent_tx.clone();
    let api_settlement_mode = settlement_mode.clone();
    let api_handle = tokio::spawn(async move {
        api::start_api_server(
            api_gossip_tx,
            api_local_intent_tx,
            http_port,
            api_chain_id,
            api_escrow_address,
            api_settlement_mode,
        )
        .await;
    });

    info!(
        "0-dex node is running: p2p_listen={} http_port={} settlement_mode={}",
        listen_addr, http_port, settlement_mode
    );

    let shutdown = CancellationToken::new();

    let ingest_shutdown = shutdown.clone();
    let ingest_handle = tokio::spawn(async move {
        let mut gossip_rx = gossip_node_rx;
        let mut local_rx = local_intent_rx;
        loop {
            tokio::select! {
                _ = ingest_shutdown.cancelled() => {
                    info!("Ingest task shutting down gracefully");
                    break;
                }
                local_intent = local_rx.recv() => {
                    let Some(local_intent) = local_intent else { break };
                    handle_validated_intent(local_intent, &mut matching_engine).await;
                }
                msg = gossip_rx.recv() => {
                    let Some(msg_bytes) = msg else { break };
                    if msg_bytes.len() > max_gossip_bytes {
                        tracing::warn!(
                            "Dropping oversized payload before deserialize: {} bytes (max {})",
                            msg_bytes.len(),
                            max_gossip_bytes
                        );
                        continue;
                    }
                    if let Ok(signed_intent) = serde_json::from_slice::<SignedIntent>(&msg_bytes) {
                        info!(
                            "Received signed intent from {}",
                            signed_intent.payload.owner_address
                        );
                        let now = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map(|d| d.as_secs())
                            .unwrap_or_default();
                        if let Err(e) = signed_intent.validate_basic(now, chain_id, &escrow_address) {
                            tracing::warn!("Invalid intent shape: {}", e);
                            continue;
                        }
                        match signed_intent.verify_signature() {
                            Ok(true) => {
                                handle_validated_intent(signed_intent, &mut matching_engine).await;
                            }
                            Ok(false) => tracing::warn!("Signature invalid for intent! Dropping."),
                            Err(e) => tracing::warn!("Failed to verify signature: {}", e),
                        };
                    } else {
                        tracing::warn!("Received unrecognized payload format on gossip network");
                    }
                }
            }
        }
    });

    tokio::signal::ctrl_c().await?;
    info!("Shutdown signal received, draining in-flight work...");
    shutdown.cancel();

    let drain_timeout = Duration::from_secs(5);
    let _ = tokio::time::timeout(drain_timeout, ingest_handle).await;
    let _ = tokio::time::timeout(drain_timeout, settlement_handle).await;
    api_handle.abort();
    info!("Shutdown complete.");
    Ok(())
}

async fn handle_validated_intent(intent: SignedIntent, matching_engine: &mut MatchingEngine) {
    let matched = matching_engine.process_incoming_intent(intent).await;
    if matched {
        info!("Matched incoming intent and forwarded proof to settlement");
    }
}

fn required_env(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    env::var(name).map_err(|_| format!("{name} must be set").into())
}

fn required_u64_env(name: &str) -> Result<u64, Box<dyn std::error::Error>> {
    let raw = required_env(name)?;
    raw.parse::<u64>()
        .map_err(|_| format!("{name} must be a valid u64").into())
}

fn env_u16_with_default(name: &str, default: u16) -> Result<u16, Box<dyn std::error::Error>> {
    match env::var(name) {
        Ok(raw) => raw
            .parse::<u16>()
            .map_err(|_| format!("{name} must be a valid u16").into()),
        Err(_) => Ok(default),
    }
}

fn env_usize_with_default(name: &str, default: usize) -> Result<usize, Box<dyn std::error::Error>> {
    match env::var(name) {
        Ok(raw) => raw
            .parse::<usize>()
            .map_err(|_| format!("{name} must be a valid usize").into()),
        Err(_) => Ok(default),
    }
}

fn parse_bootstrap_peers(
    value: Option<String>,
) -> Result<Vec<libp2p::Multiaddr>, Box<dyn std::error::Error>> {
    let Some(raw) = value else {
        return Ok(Vec::new());
    };
    let mut peers = Vec::new();
    for peer in raw.split(',').map(str::trim).filter(|x| !x.is_empty()) {
        peers.push(peer.parse::<libp2p::Multiaddr>()?);
    }
    Ok(peers)
}
