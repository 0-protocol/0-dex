//! FHE (Fully Homomorphic Encryption) privacy plugin — research stub.
//!
//! FHE would allow Solver agents to compute the intersection of two
//! encrypted intent graphs without ever decrypting them:
//!   intersect(enc(A), enc(B)) -> enc(result)
//!
//! This is not production-ready. Current FHE libraries (tfhe-rs, concrete)
//! achieve ~10,000x overhead vs plaintext for basic operations.
//! Tensor comparison / intersection would need lattice-based arithmetic
//! that doesn't exist yet at usable performance.
//!
//! This stub exists to:
//!   1. Reserve the interface in the PrivacyPlugin trait
//!   2. Document the theoretical approach
//!   3. Track progress on tfhe-rs / concrete improvements

use super::{PrivacyPlugin, PrivacyError, UnwrappedIntent};
use crate::crypto::SignedIntent;

pub struct FhePlugin;

impl PrivacyPlugin for FhePlugin {
    fn wrap_intent(&self, _intent: &SignedIntent) -> Result<Vec<u8>, PrivacyError> {
        Err(PrivacyError(
            "FHE privacy mode is not yet implemented. \
             Awaiting tfhe-rs lattice operations with <100x overhead for tensor comparison. \
             Use 'tee' or 'zk' mode instead."
            .into()
        ))
    }

    fn unwrap_intent(&self, _payload: &[u8]) -> Result<UnwrappedIntent, PrivacyError> {
        Err(PrivacyError("FHE mode not implemented".into()))
    }

    fn verify(&self, _proof: &[u8], _public_inputs: &[u8]) -> Result<bool, PrivacyError> {
        Err(PrivacyError("FHE mode not implemented".into()))
    }

    fn name(&self) -> &'static str { "fhe" }
}
