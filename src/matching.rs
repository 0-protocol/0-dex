//! Graph intersection and matching engine
//!
//! Evaluates incoming graphs against local intents using the 0-lang VM.

use zerolang::{RuntimeGraph, VM};
use tokio::sync::mpsc;
use crate::settlement::MatchProof;

pub struct MatchingEngine {
    vm: VM,
    local_intents: Vec<RuntimeGraph>,
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
            local_intents: Vec::new(),
            match_sender,
            local_wallet_address: wallet_address,
            local_signature: local_sig,
        }
    }

    pub fn register_intent(&mut self, graph: RuntimeGraph) {
        self.local_intents.push(graph);
    }

    /// Evaluates if any of our local intents intersect with the counterparty's intent.
    /// An intersection is valid if both graphs can execute successfully and their
    /// output tensors signify a mathematically sound exchange rate (price overlap).
    pub async fn evaluate_counterparty(
        &mut self, 
        counterparty_graph: RuntimeGraph,
        counterparty_address: &str,
        counterparty_sig: &str,
    ) -> bool {
        let secure_vm = crate::vm_bridge::SecureVM::new(1_000_000, 100);
        let counterparty_result = secure_vm.evaluate_untrusted(counterparty_graph).await;
        
        match counterparty_result {
            Ok(cp_tensors) => {
                if cp_tensors.is_empty() {
                    tracing::debug!("Counterparty graph executed but yielded no outputs.");
                    return false;
                }
                
                // For this MVP, we assume the first tensor output represents the desired swap vector
                // e.g. [amount_in, amount_out, confidence]
                let cp_vector = &cp_tensors[0];
                
                for local_intent in &self.local_intents {
                    if let Ok(local_tensors) = self.vm.execute(local_intent) {
                        if local_tensors.is_empty() { continue; }
                        
                        let local_vector = &local_tensors[0];
                        
                        // Cross-evaluate swap vectors. 
                        // If my local output allows the counterparty's minimum requirement
                        // and vice versa, we have a match.
                        if self.is_valid_intersection(local_vector, cp_vector) {
                            tracing::info!(
                                "MATCH FOUND! Local tensor {:?} intersects with Counterparty tensor {:?}",
                                local_vector, cp_vector
                            );
                            
                            // Send to settlement layer
                            let proof = MatchProof {
                                local_intent_id: self.local_wallet_address.clone(),
                                counterparty_intent_id: counterparty_address.to_string(),
                                settled_vector: local_vector.clone(),
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
                tracing::warn!("Counterparty graph failed to execute in local VM: {:?}", e);
                false
            }
        }
    }

    /// Cryptographic intersection math:
    /// In a real DEX, this evaluates if the vectors overlap in multi-dimensional space
    /// (e.g. Price bounds, Token ID, Amount limits).
    fn is_valid_intersection(&self, local: &zerolang::Tensor, counterparty: &zerolang::Tensor) -> bool {
        // Placeholder math: Just require both to have > 0.8 confidence
        // and represent opposing sides of the trade.
        let local_conf = local.confidence;
        let cp_conf = counterparty.confidence;
        
        local_conf > 0.8 && cp_conf > 0.8
    }
}
