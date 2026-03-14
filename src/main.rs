//! 0-dex: Agent-native decentralized exchange powered by 0-lang tensor graphs.

use clap::Parser;
use tokio::sync::mpsc;
use tracing::{info, Level};

mod node_mode;
mod network;
mod matching;
mod settlement;
mod vm_bridge;
mod api;
mod crypto;
mod abi;
mod intent_watcher;
mod metrics;
mod intent_pool;
mod solver;
mod liquidity_relayer;
mod privacy;

use node_mode::NodeMode;
use network::GossipNode;
use matching::MatchingEngine;
use settlement::{SettlementEngine, MatchProof};

#[derive(Parser)]
#[command(name = "zero-dex", about = "Agent-native DEX powered by 0-lang")]
struct Cli {
    /// Node mode: agent (default) or solver
    #[arg(long, default_value = "agent")]
    mode: NodeMode,

    /// HTTP API port
    #[arg(long, default_value_t = 8080)]
    http_port: u16,

    /// libp2p listen address
    #[arg(long, default_value = "/ip4/0.0.0.0/tcp/0")]
    listen_addr: String,

    /// Graph directories to watch (comma-separated)
    #[arg(long, default_value = "graphs/intents,graphs/pools")]
    graphs_dir: String,

    /// Privacy mode: naked (default), tee, or zk
    #[arg(long, default_value = "naked")]
    privacy: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .init();

    let cli = Cli::parse();

    info!("Initializing 0-dex node in {} mode...", cli.mode);

    let privacy_plugin = privacy::create_plugin(&cli.privacy);

    let (match_tx, match_rx) = mpsc::channel::<MatchProof>(100);

    info!("Starting libp2p gossip network...");
    let (mut gossip_node, outbound_tx, mut inbound_rx) = GossipNode::new(cli.mode)?;
    gossip_node.listen_on(&cli.listen_addr)?;

    let api_gossip_tx = outbound_tx.clone();

    tokio::spawn(async move {
        gossip_node.run().await;
    });

    info!("Starting Settlement Engine...");
    let settlement_engine = SettlementEngine::new("https://api.mainnet-beta.solana.com", match_rx);
    tokio::spawn(async move {
        settlement_engine.run().await;
    });

    info!("Starting REST/HTTP Bridge on port {}...", cli.http_port);
    let http_port = cli.http_port;
    let api_privacy = privacy::create_plugin(&cli.privacy);
    tokio::spawn(async move {
        api::start_api_server(api_gossip_tx, http_port, api_privacy).await;
    });

    match cli.mode {
        NodeMode::Agent => run_agent_loop(&mut inbound_rx, match_tx, privacy_plugin, cli.graphs_dir).await,
        NodeMode::Solver => run_solver_loop(&mut inbound_rx, match_tx, outbound_tx, privacy_plugin).await,
    }

    Ok(())
}

/// Agent mode: receive intents from gossip, verify, match pairwise against local intents
async fn run_agent_loop(
    inbound_rx: &mut mpsc::Receiver<Vec<u8>>,
    match_tx: mpsc::Sender<MatchProof>,
    privacy_plugin: Box<dyn privacy::PrivacyPlugin>,
    graphs_dir: String,
) {
    info!("0-dex AGENT node is running.");
    let mut matching_engine = MatchingEngine::new(match_tx);

    let (watcher_tx, mut watcher_rx) = tokio::sync::mpsc::channel(10);
    let directories = graphs_dir.split(",").map(|s| s.trim().to_string()).collect();
    let watcher = crate::intent_watcher::IntentWatcher::new(directories, watcher_tx);
    tokio::spawn(watcher.run());


    loop {
        tokio::select! {
            Some(msg_bytes) = inbound_rx.recv() => {
                let payload = match privacy_plugin.unwrap_intent(&msg_bytes) {
                    Ok(privacy::UnwrappedIntent::Plaintext(intent)) => intent,
                    Ok(_) => { tracing::debug!("Agent received non-plaintext intent, skipping"); continue; }
                    Err(e) => { tracing::warn!("Failed to unwrap intent: {}", e); continue; }
                };

                match payload.verify() {
                    Ok(true) => {
                        info!("Signature valid from {}. Evaluating...", payload.owner_address);
                        matching_engine.evaluate_counterparty(
                            &payload.graph_content,
                            &payload.owner_address,
                            &payload.signature_hex,
                        ).await;
                    }
                    Ok(false) => tracing::warn!("Signature invalid! Dropping."),
                    Err(e) => tracing::warn!("Failed to verify signature: {}", e),
                }
            }
            Some(event) = watcher_rx.recv() => {
                match event {
                    crate::intent_watcher::IntentEvent::Updated(path, graph, source) => {
                        matching_engine.register_intent(path, graph, source);
                    }
                    crate::intent_watcher::IntentEvent::Removed(path) => {
                        matching_engine.remove_intent(&path);
                    }
                }
            }
            else => break,
        }
    }
}

/// Solver mode: aggregate intents into pool, run CoW matching cycles, publish solutions
async fn run_solver_loop(
    inbound_rx: &mut mpsc::Receiver<Vec<u8>>,
    match_tx: mpsc::Sender<MatchProof>,
    solution_tx: mpsc::Sender<Vec<u8>>,
    privacy_plugin: Box<dyn privacy::PrivacyPlugin>,
) {
    info!("0-dex SOLVER node is running.");
    let mut solver_engine = solver::SolverEngine::new(match_tx, solution_tx);

    // Spawn periodic matching cycle (every 500ms)
    let pool_handle = solver_engine.pool_handle();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
        loop {
            interval.tick().await;
            solver_engine.run_matching_cycle().await;
        }
    });

    while let Some(msg_bytes) = inbound_rx.recv().await {
        let intent = match privacy_plugin.unwrap_intent(&msg_bytes) {
            Ok(privacy::UnwrappedIntent::Plaintext(intent)) => intent,
            Ok(privacy::UnwrappedIntent::TeeEncrypted { ciphertext, .. }) => {
                tracing::debug!("Received TEE-encrypted intent ({} bytes)", ciphertext.len());
                continue;
            }
            Ok(privacy::UnwrappedIntent::ZkProof { .. }) => {
                tracing::debug!("Received ZK proof intent");
                continue;
            }
            Err(e) => { tracing::warn!("Failed to unwrap intent: {}", e); continue; }
        };

        match intent.verify() {
            Ok(true) => {
                if let Ok(graph) = zerolang::RuntimeGraph::from_reader(intent.graph_content.as_bytes()) {
                    pool_handle.lock().await.add_intent(intent, graph);
                }
            }
            Ok(false) => tracing::warn!("Invalid signature on intent, dropping."),
            Err(e) => tracing::warn!("Verification error: {}", e),
        }
    }
}
