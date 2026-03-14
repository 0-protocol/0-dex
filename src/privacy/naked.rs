//! Naked privacy plugin: zero-overhead plaintext pass-through.
//! This is the default mode — graphs are broadcasted and evaluated in plaintext.

use super::{PrivacyPlugin, PrivacyError, UnwrappedIntent};
use crate::crypto::SignedIntent;

pub struct NakedPlugin;

impl PrivacyPlugin for NakedPlugin {
    fn wrap_intent(&self, intent: &SignedIntent) -> Result<Vec<u8>, PrivacyError> {
        serde_json::to_vec(intent)
            .map_err(|e| PrivacyError(format!("Serialization failed: {}", e)))
    }

    fn unwrap_intent(&self, payload: &[u8]) -> Result<UnwrappedIntent, PrivacyError> {
        let intent: SignedIntent = serde_json::from_slice(payload)
            .map_err(|e| PrivacyError(format!("Deserialization failed: {}", e)))?;
        Ok(UnwrappedIntent::Plaintext(intent))
    }

    fn verify(&self, _proof: &[u8], _public_inputs: &[u8]) -> Result<bool, PrivacyError> {
        // Verification is handled by crypto.rs for plaintext intents
        Ok(true)
    }

    fn name(&self) -> &'static str { "naked" }
}
