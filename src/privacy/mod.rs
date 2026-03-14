//! Pluggable privacy layer for 0-dex.
//!
//! Privacy is opt-in. Agents choose their privacy mode at runtime:
//!   - **Naked** (default): plaintext gossip, zero overhead
//!   - **TEE**: encrypt graphs for TEE-enabled Solver enclaves
//!   - **ZK**: broadcast only a ZK proof of output constraints
//!   - **FHE**: (future) compute intersections on encrypted graphs

mod naked;
mod tee;
mod zk;
mod fhe;

use crate::crypto::SignedIntent;

/// Errors from privacy operations
#[derive(Debug)]
pub struct PrivacyError(pub String);

impl std::fmt::Display for PrivacyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "PrivacyError: {}", self.0)
    }
}

impl std::error::Error for PrivacyError {}

/// Result of unwrapping a received gossip payload
pub enum UnwrappedIntent {
    /// Plaintext graph, ready for local matching
    Plaintext(SignedIntent),
    /// ZK proof of output tensor constraints (graph not revealed)
    ZkProof {
        proof: Vec<u8>,
        public_outputs: Vec<u8>,
    },
    /// Encrypted for TEE processing only
    TeeEncrypted {
        ciphertext: Vec<u8>,
        tee_pubkey: Vec<u8>,
    },
}

/// Trait that all privacy modes implement.
/// The matching engine and gossip layer use this to wrap/unwrap intents.
pub trait PrivacyPlugin: Send + Sync {
    /// Wrap a signed intent before broadcasting (encrypt, prove, or pass-through)
    fn wrap_intent(&self, intent: &SignedIntent) -> Result<Vec<u8>, PrivacyError>;

    /// Unwrap a received gossip payload into a usable form
    fn unwrap_intent(&self, payload: &[u8]) -> Result<UnwrappedIntent, PrivacyError>;

    /// Verify a proof or decrypted result (no-op for naked mode)
    fn verify(&self, proof: &[u8], public_inputs: &[u8]) -> Result<bool, PrivacyError>;

    /// Human-readable name
    fn name(&self) -> &'static str;
}

/// Factory: create the appropriate plugin from a mode string
pub fn create_plugin(mode: &str) -> Box<dyn PrivacyPlugin> {
    match mode.to_lowercase().as_str() {
        "tee" => Box::new(tee::TeePlugin::new()),
        "zk" => Box::new(zk::ZkPlugin::new()),
        "fhe" => Box::new(fhe::FhePlugin),
        _ => Box::new(naked::NakedPlugin),
    }
}
