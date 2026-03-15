use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{info, Level};
use tracing_subscriber;

mod abi;
mod api;
mod crypto;
mod matching;
mod network;
mod protocol;
mod settlement;
mod vm_bridge;

use matching::MatchingEngine;
use network::GossipNode;
use protocol::{MatchProof, SignedIntent};
use settlement::SettlementEngine;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt().with_max_level(Level::INFO).init();

    info!("Initializing 0-dex agent node...");

    let (match_tx, match_rx) = mpsc::channel::<MatchProof>(100);

    info!("Starting libp2p gossip network...");
    let (mut gossip_node, gossip_tx, gossip_node_rx) = GossipNode::new()?;
    gossip_node.listen_on("/ip4/0.0.0.0/tcp/0")?;

    tokio::spawn(async move {
        gossip_node.run().await;
    });

    info!("Starting Settlement Engine...");
    let chain_id = std::env::var("ZERO_DEX_CHAIN_ID")
        .ok()
        .and_then(|x| x.parse::<u64>().ok())
        .unwrap_or(1);
    let rpc_url = std::env::var("ZERO_DEX_RPC_URL")
        .unwrap_or_else(|_| "https://eth.llamarpc.com".to_string());
    let escrow_address = std::env::var("ZERO_DEX_ESCROW_ADDRESS")
        .map_err(|_| "ZERO_DEX_ESCROW_ADDRESS must be set")?;
    if escrow_address.eq_ignore_ascii_case("0x0000000000000000000000000000000000000000") {
        return Err("ZERO_DEX_ESCROW_ADDRESS cannot be zero address".into());
    }
    let settlement_engine = SettlementEngine::new(&rpc_url, chain_id, &escrow_address, match_rx);

    let settlement_handle = tokio::spawn(async move {
        settlement_engine.run().await;
    });

    info!("Starting 0-dex Matching Engine...");
    let mut matching_engine = MatchingEngine::new(match_tx);

    let api_gossip_tx = gossip_tx.clone();
    let api_chain_id = chain_id;
    let api_escrow_address = escrow_address.clone();
    let api_handle = tokio::spawn(async move {
        api::start_api_server(api_gossip_tx, 8080, api_chain_id, api_escrow_address).await;
    });

    info!("0-dex node is running in serverless P2P mode.");

    let shutdown = CancellationToken::new();

    let ingest_shutdown = shutdown.clone();
    let ingest_handle = tokio::spawn(async move {
        let mut gossip_rx = gossip_node_rx;
        loop {
            tokio::select! {
                _ = ingest_shutdown.cancelled() => {
                    info!("Ingest task shutting down gracefully");
                    break;
                }
                msg = gossip_rx.recv() => {
                    let Some(msg_bytes) = msg else { break };
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
                                let matched = matching_engine.process_incoming_intent(signed_intent).await;
                                if matched {
                                    info!("Matched incoming intent and forwarded proof to settlement");
                                }
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
