use tokio::sync::mpsc;
use tracing::{info, Level};
use tracing_subscriber;

mod network;
mod matching;
mod settlement;
mod vm_bridge;
mod api;
mod crypto;
mod abi;

use network::GossipNode;
use matching::MatchingEngine;
use settlement::{SettlementEngine, MatchProof};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    info!("Initializing 0-dex agent node...");

    let (match_tx, match_rx) = mpsc::channel::<MatchProof>(100);

    info!("Starting libp2p gossip network...");
    let (mut gossip_node, outbound_tx, mut inbound_rx) = GossipNode::new()?;
    gossip_node.listen_on("/ip4/0.0.0.0/tcp/0")?;

    let api_gossip_tx = outbound_tx.clone();

    tokio::spawn(async move {
        gossip_node.run().await;
    });

    info!("Starting Settlement Engine...");
    let settlement_engine = SettlementEngine::new("https://api.mainnet-beta.solana.com", match_rx);
    tokio::spawn(async move {
        settlement_engine.run().await;
    });

    info!("Starting 0-lang Matching Engine...");
    let mut matching_engine = MatchingEngine::new(match_tx);

    info!("Starting REST/HTTP Bridge on port 8080...");
    tokio::spawn(async move {
        api::start_api_server(api_gossip_tx, 8080).await;
    });

    info!("0-dex node is running in serverless P2P mode.");

    // Main intent processing loop: gossip → verify → match → settle
    while let Some(msg_bytes) = inbound_rx.recv().await {
        match serde_json::from_slice::<crypto::SignedIntent>(&msg_bytes) {
            Ok(signed_intent) => {
                info!("Received signed intent from {}", signed_intent.owner_address);

                match signed_intent.verify() {
                    Ok(true) => {
                        info!("Signature valid. Evaluating graph against local intents...");
                        match zerolang::RuntimeGraph::from_reader(signed_intent.graph_content.as_bytes()) {
                            Ok(graph) => {
                                matching_engine.evaluate_counterparty(
                                    graph,
                                    &signed_intent.owner_address,
                                    &signed_intent.signature_hex,
                                ).await;
                            }
                            Err(e) => tracing::warn!("Failed to parse counterparty graph: {:?}", e),
                        }
                    }
                    Ok(false) => tracing::warn!("Signature invalid for intent! Dropping."),
                    Err(e) => tracing::warn!("Failed to verify signature: {}", e),
                }
            }
            Err(_) => {
                tracing::warn!("Received unrecognized payload format on gossip network");
            }
        }
    }

    Ok(())
}
