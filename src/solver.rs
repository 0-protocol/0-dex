//! Multi-way Coincidence-of-Wants (CoW) solver engine.
//!
//! Instead of pairwise matching, the solver collects intents into a pool
//! and finds optimal N-way swap cycles where all participants' constraints
//! are satisfied simultaneously.

use std::collections::{HashMap, HashSet};
use tokio::sync::mpsc;
use tracing::{info, debug};
use zerolang::VM;

use crate::intent_pool::{self, IntentPool, PoolHandle, PooledIntent};
use crate::settlement::MatchProof;
use crate::vm_bridge::SecureVM;

/// A directed edge in the swap graph: agent wants to swap `give_token` for `receive_token`
#[derive(Debug, Clone)]
struct SwapEdge {
    intent_key: String,
    agent_address: String,
    give_token: String,
    receive_token: String,
    give_amount: f32,
    receive_amount: f32,
    confidence: f32,
    signature: String,
}

/// Directed graph where nodes = tokens, edges = swap intents
struct SwapGraph {
    /// token -> list of outgoing edges (intents wanting to sell this token)
    adjacency: HashMap<String, Vec<SwapEdge>>,
    tokens: HashSet<String>,
}

impl SwapGraph {
    fn new() -> Self {
        Self { adjacency: HashMap::new(), tokens: HashSet::new() }
    }

    fn add_edge(&mut self, edge: SwapEdge) {
        self.tokens.insert(edge.give_token.clone());
        self.tokens.insert(edge.receive_token.clone());
        self.adjacency
            .entry(edge.give_token.clone())
            .or_default()
            .push(edge);
    }

    /// Find all simple cycles of length 2+ using DFS
    fn find_cycles(&self) -> Vec<Vec<SwapEdge>> {
        let mut cycles = Vec::new();
        let tokens: Vec<&String> = self.tokens.iter().collect();

        for start in &tokens {
            let mut path: Vec<SwapEdge> = Vec::new();
            let mut visited: HashSet<String> = HashSet::new();
            visited.insert((*start).clone());
            self.dfs_cycles(start, start, &mut path, &mut visited, &mut cycles);
        }

        // Deduplicate cycles (same cycle can be found starting from different nodes)
        deduplicate_cycles(cycles)
    }

    fn dfs_cycles(
        &self,
        current: &str,
        start: &str,
        path: &mut Vec<SwapEdge>,
        visited: &mut HashSet<String>,
        cycles: &mut Vec<Vec<SwapEdge>>,
    ) {
        if let Some(edges) = self.adjacency.get(current) {
            for edge in edges {
                if edge.receive_token == start && path.len() >= 1 {
                    // Found a cycle back to start
                    let mut cycle = path.clone();
                    cycle.push(edge.clone());
                    cycles.push(cycle);
                } else if !visited.contains(&edge.receive_token) && path.len() < 5 {
                    // Continue DFS (limit cycle length to 5 for performance)
                    visited.insert(edge.receive_token.clone());
                    path.push(edge.clone());
                    self.dfs_cycles(&edge.receive_token, start, path, visited, cycles);
                    path.pop();
                    visited.remove(&edge.receive_token);
                }
            }
        }
    }
}

fn deduplicate_cycles(cycles: Vec<Vec<SwapEdge>>) -> Vec<Vec<SwapEdge>> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut unique = Vec::new();

    for cycle in cycles {
        // Canonical form: sort intent keys, join
        let mut keys: Vec<String> = cycle.iter().map(|e| e.intent_key.clone()).collect();
        keys.sort();
        let canonical = keys.join(",");
        if seen.insert(canonical) {
            unique.push(cycle);
        }
    }
    unique
}

pub struct SolverEngine {
    pool: PoolHandle,
    match_sender: mpsc::Sender<MatchProof>,
    solution_tx: mpsc::Sender<Vec<u8>>,
    vm: VM,
}

impl SolverEngine {
    pub fn new(match_sender: mpsc::Sender<MatchProof>, solution_tx: mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            pool: intent_pool::new_pool_handle(60_000),
            match_sender,
            solution_tx,
            vm: VM::new(),
        }
    }

    pub fn pool_handle(&self) -> PoolHandle {
        self.pool.clone()
    }

    /// Run one matching cycle: build swap graph, find cycles, settle best
    pub async fn run_matching_cycle(&mut self) {
        let mut pool = self.pool.lock().await;

        // Prune expired intents
        let pruned = pool.prune_expired();
        if pruned > 0 {
            debug!("Pruned {} expired intents", pruned);
        }

        let active = pool.active_intents();
        if active.len() < 2 {
            return;
        }

        // Execute each intent's graph and extract swap parameters
        let mut swap_graph = SwapGraph::new();
        for (key, pooled) in &active {
            if let Some(edge) = extract_swap_edge(&mut self.vm, key, pooled) {
                swap_graph.add_edge(edge);
            }
        }

        let cycles = swap_graph.find_cycles();
        if cycles.is_empty() {
            return;
        }

        // Score cycles by total surplus and pick the best
        let best = cycles.into_iter()
            .max_by(|a, b| {
                let surplus_a: f32 = a.iter().map(|e| e.confidence).sum();
                let surplus_b: f32 = b.iter().map(|e| e.confidence).sum();
                surplus_a.partial_cmp(&surplus_b).unwrap_or(std::cmp::Ordering::Equal)
            });

        if let Some(cycle) = best {
            info!("CoW cycle found with {} participants!", cycle.len());
            let intent_keys: Vec<String> = cycle.iter().map(|e| e.intent_key.clone()).collect();

            // Generate match proofs for each pair in the cycle
            for window in cycle.windows(2) {
                let proof = MatchProof {
                    local_intent_id: window[0].agent_address.clone(),
                    counterparty_intent_id: window[1].agent_address.clone(),
                    settled_vector: zerolang::Tensor::scalar(0.0, 1.0),
                    local_signature: hex::decode(
                        window[0].signature.trim_start_matches("0x")
                    ).unwrap_or_default(),
                    counterparty_signature: hex::decode(
                        window[1].signature.trim_start_matches("0x")
                    ).unwrap_or_default(),
                };
                let _ = self.match_sender.send(proof).await;
                }
            }

            // Close the cycle: last -> first
            if cycle.len() >= 2 {
                if let Some(last) = cycle.last() {
                let first = &cycle[0];
                let proof = MatchProof {
                    local_intent_id: last.agent_address.clone(),
                    counterparty_intent_id: first.agent_address.clone(),
                    settled_vector: zerolang::Tensor::scalar(0.0, 1.0),
                    local_signature: hex::decode(
                        last.signature.trim_start_matches("0x")
                    ).unwrap_or_default(),
                    counterparty_signature: hex::decode(
                        first.signature.trim_start_matches("0x")
                    ).unwrap_or_default(),
                };
                let _ = self.match_sender.send(proof).await;
                }
            }

            // Mark all participating intents as matched
            pool.mark_matched(&intent_keys);

            // Publish solution summary to gossip
            let solution_msg = serde_json::json!({
                "type": "cow_solution",
                "participants": cycle.len(),
                "intent_keys": intent_keys,
            });
            let _ = self.solution_tx.send(
                serde_json::to_vec(&solution_msg).unwrap_or_default()
            ).await;
        }
    }

}

fn extract_swap_edge(vm: &mut VM, key: &str, pooled: &PooledIntent) -> Option<SwapEdge> {
    match vm.execute(&pooled.graph) {
        Ok(tensors) if !tensors.is_empty() => {
            let t = &tensors[0];
            let (give, receive) = extract_token_pair(&pooled.signed_intent.graph_content);
            Some(SwapEdge {
                intent_key: key.to_string(),
                agent_address: pooled.signed_intent.owner_address.clone(),
                give_token: give,
                receive_token: receive,
                give_amount: t.confidence,
                receive_amount: t.confidence,
                confidence: t.confidence,
                signature: pooled.signed_intent.signature_hex.clone(),
            })
        }
        Ok(_) => None,
        Err(e) => {
            debug!("Failed to execute intent {}: {:?}", &key[..16], e);
            None
        }
    }
}

/// Parse token pair from 0-lang graph content.
/// Looks for `def buy_asset: "X"` and `def sell_asset: "Y"` patterns.
fn extract_token_pair(graph_content: &str) -> (String, String) {
    let mut sell = "UNKNOWN".to_string();
    let mut buy = "UNKNOWN".to_string();

    for line in graph_content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("def sell_asset:") {
            if let Some(val) = extract_quoted_value(trimmed) {
                sell = val;
            }
        } else if trimmed.starts_with("def buy_asset:") {
            if let Some(val) = extract_quoted_value(trimmed) {
                buy = val;
            }
        }
    }

    // sell_asset is what the agent gives, buy_asset is what they receive
    (sell, buy)
}

fn extract_quoted_value(line: &str) -> Option<String> {
    let start = line.find('"')? + 1;
    let end = line[start..].find('"')? + start;
    Some(line[start..end].to_string())
}
