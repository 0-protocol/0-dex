//! Graph intersection and matching engine
//!
//! Evaluates incoming graphs against local intents using the 0-lang VM.

use zerolang::{RuntimeGraph, VM};
use tokio::sync::mpsc;
use std::collections::HashMap;
use crate::settlement::MatchProof;

pub struct LocalIntent {
    pub graph: RuntimeGraph,
    pub source: String,
    pub give_token: String,
    pub receive_token: String,
    pub min_give: f64,
    pub max_give: f64,
    pub min_price: f64, // receive / give
}

pub struct MatchingEngine {
    vm: VM,
    local_intents: HashMap<String, LocalIntent>,
    match_sender: mpsc::Sender<MatchProof>,
    local_wallet_address: String,
    local_signature: Vec<u8>,
}

impl MatchingEngine {
    pub fn new(match_sender: mpsc::Sender<MatchProof>) -> Self {
        let wallet_address = std::env::var("ZERO_DEX_WALLET_ADDRESS")
            .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string());
        let local_sig = std::env::var("ZERO_DEX_WALLET_SIG")
            .ok()
            .and_then(|s| hex::decode(s.trim_start_matches("0x")).ok())
            .unwrap_or_default();

        Self {
            vm: VM::new(),
            local_intents: HashMap::new(),
            match_sender,
            local_wallet_address: wallet_address,
            local_signature: local_sig,
        }
    }

    /// Register a local intent from its raw source code and compiled graph
    pub fn remove_intent(pub fn register_intent(&mut self, id: String, graph: RuntimeGraph, source: String) {mut self, id: pub fn register_intent(&mut self, id: String, graph: RuntimeGraph, source: String) {str) {
        self.local_intents.remove(id);
    }

    pub fn register_intent(&mut self, id: String, graph: RuntimeGraph, source: String) {
        let give_token = extract_def_value(&source, "sell_asset").unwrap_or_default();
        let receive_token = extract_def_value(&source, "buy_asset").unwrap_or_default();
        let min_give = extract_def_float(&source, "min_amount").unwrap_or(0.0);
        let max_give = extract_def_float(&source, "amount").unwrap_or(f64::MAX);
        let min_price = extract_def_float(&source, "min_price").unwrap_or(0.0);

        self.local_intents.insert(id, LocalIntent {
            graph,
            source,
            give_token,
            receive_token,
            min_give,
            max_give,
            min_price,
        });
    }

    /// Evaluates if any of our local intents intersect with the counterparty's intent.
    pub async fn evaluate_counterparty(
        &mut self,
        counterparty_source: &str,
        counterparty_address: &str,
        counterparty_sig: &str,
    ) -> bool {
        let cp_give_token = extract_def_value(counterparty_source, "sell_asset").unwrap_or_default();
        let cp_receive_token = extract_def_value(counterparty_source, "buy_asset").unwrap_or_default();
        let cp_min_give = extract_def_float(counterparty_source, "min_amount").unwrap_or(0.0);
        let cp_max_give = extract_def_float(counterparty_source, "amount").unwrap_or(f64::MAX);
        let cp_min_price = extract_def_float(counterparty_source, "min_price").unwrap_or(0.0);

        let cp_graph = match RuntimeGraph::from_reader(counterparty_source.as_bytes()) {
            Ok(g) => g,
            Err(_) => return false,
        };

        let secure_vm = crate::vm_bridge::SecureVM::new(1_000_000, 100);
        let counterparty_result = secure_vm.evaluate_untrusted(cp_graph).await;
        
        match counterparty_result {
            Ok(cp_tensors) => {
                if cp_tensors.is_empty() { return false; }
                let cp_vector = &cp_tensors[0];
                
                for (local_id, local) in &self.local_intents {
                    // Check token pair match (inverse)
                    if local.give_token != cp_receive_token || local.receive_token != cp_give_token {
                        continue;
                    }

                    // Calculate Price Overlap
                    // Local wants at least local.min_price (Receive/Give)
                    // CP wants at least cp.min_price (Local_Give/Local_Receive) => maximum price for Local is 1 / cp.min_price
                    let local_max_price = if cp_min_price > 0.0 { 1.0 / cp_min_price } else { f64::MAX };
                    
                    if local.min_price > local_max_price {
                        tracing::debug!("Price bounds do not overlap");
                        continue;
                    }

                    // Settlement Price is the midpoint of the overlapping region
                    let settled_price = (local.min_price + local_max_price) / 2.0;

                    // Calculate Amount Overlap (Partial Fills)
                    // We measure everything in terms of "Local Give" token (amount_a)
                    // CP Give (Local Receive) bounds converted to Local Give bounds:
                    let cp_min_local_give = cp_min_give / settled_price;
                    let cp_max_local_give = cp_max_give / settled_price;

                    let overlap_min = local.min_give.max(cp_min_local_give);
                    let overlap_max = local.max_give.min(cp_max_local_give);

                    if overlap_min > overlap_max {
                        tracing::debug!("Amount bounds do not overlap");
                        continue;
                    }

                    // We fill the maximum mutually agreeable amount
                    let amount_a = overlap_max;
                    let amount_b = amount_a * settled_price;

                    if let Ok(local_tensors) = self.vm.execute(&local.graph) {
                        if local_tensors.is_empty() { continue; }
                        let local_vector = &local_tensors[0];

                        if local_vector.confidence > 0.5 && cp_vector.confidence > 0.5 {
                            tracing::info!(
                                "MATCH FOUND! {} <=> {} | Settled {} {} for {} {}",
                                local_id, counterparty_address,
                                amount_a, local.give_token,
                                amount_b, local.receive_token
                            );
                            
                            let proof = MatchProof {
                                local_intent_id: self.local_wallet_address.clone(),
                                counterparty_intent_id: counterparty_address.to_string(),
                                settled_vector: zerolang::Tensor::scalar(0.0, 1.0),
                                token_a: local.give_token.clone(),
                                token_b: local.receive_token.clone(),
                                amount_a: amount_a as u64,
                                amount_b: amount_b as u64,
                                local_signature: self.local_signature.clone(),
                                counterparty_signature: hex::decode(
                                    counterparty_sig.trim_start_matches("0x")
                                ).unwrap_or_default(),
                            };
                            
                            let _ = self.match_sender.send(proof).await;
                            return true;
                        }
                    }
                }
                false
            },
            Err(e) => {
                tracing::warn!("Counterparty graph failed: {:?}", e);
                false
            }
        }
    }
}

// Helpers for extracting defs
fn extract_def_value(content: &str, key: &str) -> Option<String> {
    let prefix = format!("def {}:", key);
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&prefix) {
            let val = trimmed[prefix.len()..].trim();
            if val.starts_with('"') && val.ends_with('"') {
                return Some(val[1..val.len()-1].to_string());
            }
            return Some(val.to_string());
        }
    }
    None
}

fn extract_def_float(content: &str, key: &str) -> Option<f64> {
    extract_def_value(content, key)?.parse().ok()
}
