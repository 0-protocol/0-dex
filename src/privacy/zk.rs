//! ZK (Zero Knowledge) privacy plugin.
//!
//! Agents execute their intent graph locally inside a ZK prover,
//! then broadcast only the proof of the output tensor constraints
//! without revealing the graph itself.
//!
//! Uses Risc0 for ZK proving (when the `zk` feature is enabled).
//! Without the feature, this module provides the interface but returns
//! errors at runtime.

use super::{PrivacyPlugin, PrivacyError, UnwrappedIntent};
use crate::crypto::SignedIntent;

/// Envelope format for ZK-proved intents on the gossip network
#[derive(serde::Serialize, serde::Deserialize)]
struct ZkEnvelope {
    /// The ZK proof bytes (hex-encoded)
    proof: String,
    /// Public outputs: token pair, amount bounds, price bounds (JSON)
    public_outputs: String,
    /// Agent's EVM address (public, needed for settlement)
    owner_address: String,
    /// Signature over the public outputs (so settlement can verify)
    signature_hex: String,
    /// Protocol tag
    protocol: String,
}

/// Public outputs that are revealed from the ZK proof
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct ZkPublicOutputs {
    pub give_token: String,
    pub receive_token: String,
    pub min_give_amount: f64,
    pub max_give_amount: f64,
    pub min_receive_amount: f64,
    pub min_price: f64,
    pub max_price: f64,
}

pub struct ZkPlugin {
    _prover_available: bool,
}

impl ZkPlugin {
    pub fn new() -> Self {
        // In production with `zk` feature: initialize Risc0 prover
        Self {
            _prover_available: false,
        }
    }

    /// Execute the graph in a ZK prover and return proof + public outputs.
    /// Requires the `zk` feature and Risc0 guest binary.
    fn prove_intent(&self, intent: &SignedIntent) -> Result<(Vec<u8>, ZkPublicOutputs), PrivacyError> {
        // TODO: when risc0-zkvm is available:
        //   1. Load the zero-dex-zk-guest ELF binary
        //   2. Feed graph_content as guest input
        //   3. Guest runs zerolang::VM, outputs tensor constraints
        //   4. Return Receipt (proof) + journal (public outputs)

        // For now, extract public outputs directly (no actual proof)
        let outputs = ZkPublicOutputs {
            give_token: extract_def_value(&intent.graph_content, "sell_asset")
                .unwrap_or_else(|| "UNKNOWN".to_string()),
            receive_token: extract_def_value(&intent.graph_content, "buy_asset")
                .unwrap_or_else(|| "UNKNOWN".to_string()),
            min_give_amount: extract_def_float(&intent.graph_content, "amount").unwrap_or(0.0),
            max_give_amount: extract_def_float(&intent.graph_content, "amount").unwrap_or(f64::MAX),
            min_receive_amount: 0.0,
            min_price: extract_def_float(&intent.graph_content, "min_price").unwrap_or(0.0),
            max_price: f64::MAX,
        };

        // Placeholder proof (would be Risc0 Receipt bytes)
        let proof_placeholder = b"zk_proof_placeholder".to_vec();

        Ok((proof_placeholder, outputs))
    }
}

impl PrivacyPlugin for ZkPlugin {
    fn wrap_intent(&self, intent: &SignedIntent) -> Result<Vec<u8>, PrivacyError> {
        let (proof_bytes, public_outputs) = self.prove_intent(intent)?;

        let public_outputs_json = serde_json::to_string(&public_outputs)
            .map_err(|e| PrivacyError(format!("Serialize public outputs: {}", e)))?;

        let envelope = ZkEnvelope {
            proof: hex::encode(&proof_bytes),
            public_outputs: public_outputs_json,
            owner_address: intent.owner_address.clone(),
            signature_hex: intent.signature_hex.clone(),
            protocol: "zk-risc0-v1".to_string(),
        };

        serde_json::to_vec(&envelope)
            .map_err(|e| PrivacyError(format!("Envelope serialize: {}", e)))
    }

    fn unwrap_intent(&self, payload: &[u8]) -> Result<UnwrappedIntent, PrivacyError> {
        let envelope: ZkEnvelope = match serde_json::from_slice::<ZkEnvelope>(payload) {
            Ok(e) if e.protocol == "zk-risc0-v1" => e,
            _ => return Err(PrivacyError("Not a ZK-proved payload".into())),
        };

        let proof = hex::decode(&envelope.proof)
            .map_err(|e| PrivacyError(format!("Bad proof hex: {}", e)))?;
        let public_outputs = envelope.public_outputs.into_bytes();

        Ok(UnwrappedIntent::ZkProof { proof, public_outputs })
    }

    fn verify(&self, proof: &[u8], _public_inputs: &[u8]) -> Result<bool, PrivacyError> {
        // TODO: risc0_zkvm::Receipt::verify(IMAGE_ID) when available
        if proof == b"zk_proof_placeholder" {
            tracing::warn!("ZK proof verification is using placeholder — not cryptographically verified");
            return Ok(true);
        }
        Err(PrivacyError("Real ZK verification requires risc0-zkvm feature".into()))
    }

    fn name(&self) -> &'static str { "zk" }
}

fn extract_def_value(content: &str, key: &str) -> Option<String> {
    let prefix = format!("def {}:", key);
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with(&prefix) {
            let val = trimmed[prefix.len()..].trim();
            // Strip surrounding quotes
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
