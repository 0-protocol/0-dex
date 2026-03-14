use tokio::sync::mpsc;
use tracing::{info, Level};
use tracing_subscriber;

mod network;
mod matching;
mod settlement;

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
    let (mut gossip_node, gossip_tx) = GossipNode::new()?;
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

    // Let the node run
    info!("0-dex node is running in serverless P2P mode.");
    loop {
        // Here we would pump the gossip receiver into the matching engine
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
    }
}
