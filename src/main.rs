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

    // 1. Setup channels linking the three subsystems
    let (match_tx, match_rx) = mpsc::channel::<MatchProof>(100);
    
    // 2. Initialize the Gossip Network
    info!("Starting libp2p gossip network...");
    let (mut gossip_node, gossip_tx, gossip_node_rx) = GossipNode::new()?;
    // Listen on all interfaces, random OS-assigned port
    gossip_node.listen_on("/ip4/0.0.0.0/tcp/0")?;

    // Spawn network task
    tokio::spawn(async move {
        gossip_node.run(gossip_tx).await;
    });

    // 3. Initialize the On-Chain Settlement Engine
    info!("Starting Settlement Engine...");
    let settlement_engine = SettlementEngine::new("https://api.mainnet-beta.solana.com", match_rx);
    
    // Spawn settlement task
    tokio::spawn(async move {
        settlement_engine.run().await;
    });

    // 4. Initialize and run the Matching Engine (Runs on main thread for now)
    info!("Starting 0-lang Matching Engine...");
    let mut matching_engine = MatchingEngine::new(match_tx);

    // 5. Start the REST/HTTP Bridge for lightweight agents
    // Clone the transmitter so the HTTP server can send graphs to the network
    let api_gossip_tx = gossip_tx.clone();
    tokio::spawn(async move {
        api::start_api_server(api_gossip_tx, 8080).await;
    });

    // Let the node run
    info!("0-dex node is running in serverless P2P mode.");
    
    // Create a background task to process incoming gossip messages into the matching engine
    tokio::spawn(async move {
        let mut gossip_rx = gossip_node_rx; // assuming we passed this out of GossipNode::new
        while let Some(msg_bytes) = gossip_rx.recv().await {
            // Attempt to decode the payload as a SignedIntent
            if let Ok(signed_intent) = serde_json::from_slice::<crypto::SignedIntent>(&msg_bytes) {
                info!("Received signed intent from {}", signed_intent.owner_address);
                
                // 1. Verify cryptographic signature
                match signed_intent.verify() {
                    Ok(true) => {
                        info!("Signature valid! Parsing graph...");
                        // 2. Parse into RuntimeGraph
                        // In a full implementation we'd parse the string, but for now we'll mock it
                        // let graph = zerolang::RuntimeGraph::parse_from_string(&signed_intent.graph_content);
                        // matching_engine.evaluate_counterparty(&graph, &signed_intent.owner_address, &signed_intent.signature_hex).await;
                    },
                    Ok(false) => tracing::warn!("Signature invalid for intent! Dropping."),
                    Err(e) => tracing::warn!("Failed to verify signature: {}", e),
                }
            } else {
                tracing::warn!("Received unrecognized payload format on gossip network");
            }
        }
    });

    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}
