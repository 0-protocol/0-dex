//! TEE (Trusted Execution Environment) privacy plugin.
//!
//! Agent-side: encrypts intent graphs with the TEE Solver's X25519 public key
//! using ChaCha20-Poly1305 AEAD.
//!
//! Solver-side (inside enclave): decrypts and runs matching in the secure enclave.
//!
//! For MVP, targets AWS Nitro Enclaves. The TEE Solver's public key is
//! distributed via the attestation document.

use chacha20poly1305::{
    aead::{Aead, KeyInit, OsRng},
    ChaCha20Poly1305, Nonce,
};
use x25519_dalek::{EphemeralSecret, PublicKey, StaticSecret};
use sha3::{Digest, Sha3_256};
use rand::RngCore;

use super::{PrivacyPlugin, PrivacyError, UnwrappedIntent};
use crate::crypto::SignedIntent;

/// Envelope format for TEE-encrypted intents on the gossip network
#[derive(serde::Serialize, serde::Deserialize)]
struct TeeEnvelope {
    /// Ephemeral X25519 public key (32 bytes, hex-encoded)
    ephemeral_pubkey: String,
    /// ChaCha20-Poly1305 nonce (12 bytes, hex-encoded)
    nonce: String,
    /// Encrypted SignedIntent JSON (hex-encoded)
    ciphertext: String,
    /// The TEE Solver's static public key this was encrypted for
    tee_pubkey: String,
    /// Protocol tag for routing
    protocol: String,
}

pub struct TeePlugin {
    /// The TEE Solver's X25519 public key (agents encrypt to this)
    tee_solver_pubkey: Option<PublicKey>,
    /// The TEE Solver's private key (only available inside the enclave)
    tee_solver_secret: Option<StaticSecret>,
}

impl TeePlugin {
    pub fn new() -> Self {
        // Agent side: load the TEE solver's public key from env
        let pubkey = std::env::var("ZERO_DEX_TEE_PUBKEY")
            .ok()
            .and_then(|hex_str| {
                let bytes = hex::decode(hex_str.trim_start_matches("0x")).ok()?;
                if bytes.len() == 32 {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    Some(PublicKey::from(arr))
                } else {
                    None
                }
            });

        // Solver side: load private key (only available in TEE enclave)
        let secret = std::env::var("ZERO_DEX_TEE_SECRET")
            .ok()
            .and_then(|hex_str| {
                let bytes = hex::decode(hex_str.trim_start_matches("0x")).ok()?;
                if bytes.len() == 32 {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(&bytes);
                    Some(StaticSecret::from(arr))
                } else {
                    None
                }
            });

        Self {
            tee_solver_pubkey: pubkey,
            tee_solver_secret: secret,
        }
    }

    fn derive_symmetric_key(shared_secret: &[u8]) -> [u8; 32] {
        let mut hasher = Sha3_256::new();
        hasher.update(b"0-dex-tee-v1");
        hasher.update(shared_secret);
        let result = hasher.finalize();
        let mut key = [0u8; 32];
        key.copy_from_slice(&result);
        key
    }
}

impl PrivacyPlugin for TeePlugin {
    fn wrap_intent(&self, intent: &SignedIntent) -> Result<Vec<u8>, PrivacyError> {
        let tee_pubkey = self.tee_solver_pubkey
            .ok_or_else(|| PrivacyError("TEE solver public key not configured (set ZERO_DEX_TEE_PUBKEY)".into()))?;

        let plaintext = serde_json::to_vec(intent)
            .map_err(|e| PrivacyError(format!("Serialize failed: {}", e)))?;

        // X25519 ECDH: ephemeral secret + TEE solver's static pubkey
        let ephemeral_secret = EphemeralSecret::random_from_rng(OsRng);
        let ephemeral_pubkey = PublicKey::from(&ephemeral_secret);
        let shared_secret = ephemeral_secret.diffie_hellman(&tee_pubkey);

        // Derive symmetric key from shared secret
        let sym_key = Self::derive_symmetric_key(shared_secret.as_bytes());
        let cipher = ChaCha20Poly1305::new_from_slice(&sym_key)
            .map_err(|e| PrivacyError(format!("Cipher init failed: {}", e)))?;

        // Random nonce
        let mut nonce_bytes = [0u8; 12];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher.encrypt(nonce, plaintext.as_ref())
            .map_err(|e| PrivacyError(format!("Encryption failed: {}", e)))?;

        let envelope = TeeEnvelope {
            ephemeral_pubkey: hex::encode(ephemeral_pubkey.as_bytes()),
            nonce: hex::encode(nonce_bytes),
            ciphertext: hex::encode(&ciphertext),
            tee_pubkey: hex::encode(tee_pubkey.as_bytes()),
            protocol: "tee-x25519-chacha20".to_string(),
        };

        serde_json::to_vec(&envelope)
            .map_err(|e| PrivacyError(format!("Envelope serialize failed: {}", e)))
    }

    fn unwrap_intent(&self, payload: &[u8]) -> Result<UnwrappedIntent, PrivacyError> {
        // First try to parse as TEE envelope
        let envelope: TeeEnvelope = match serde_json::from_slice::<TeeEnvelope>(payload) {
            Ok(e) if e.protocol == "tee-x25519-chacha20" => e,
            _ => {
                return Err(PrivacyError("Not a TEE-encrypted payload".into()));
            }
        };

        // If we have the TEE secret key, decrypt
        if let Some(ref secret) = self.tee_solver_secret {
            let ephemeral_pub_bytes = hex::decode(&envelope.ephemeral_pubkey)
                .map_err(|e| PrivacyError(format!("Bad ephemeral pubkey hex: {}", e)))?;
            let mut epk = [0u8; 32];
            epk.copy_from_slice(&ephemeral_pub_bytes);
            let ephemeral_pubkey = PublicKey::from(epk);

            let shared_secret = secret.diffie_hellman(&ephemeral_pubkey);
            let sym_key = Self::derive_symmetric_key(shared_secret.as_bytes());

            let cipher = ChaCha20Poly1305::new_from_slice(&sym_key)
                .map_err(|e| PrivacyError(format!("Cipher init: {}", e)))?;

            let nonce_bytes = hex::decode(&envelope.nonce)
                .map_err(|e| PrivacyError(format!("Bad nonce hex: {}", e)))?;
            let nonce = Nonce::from_slice(&nonce_bytes);

            let ciphertext = hex::decode(&envelope.ciphertext)
                .map_err(|e| PrivacyError(format!("Bad ciphertext hex: {}", e)))?;

            let plaintext = cipher.decrypt(nonce, ciphertext.as_ref())
                .map_err(|e| PrivacyError(format!("Decryption failed: {}", e)))?;

            let intent: SignedIntent = serde_json::from_slice(&plaintext)
                .map_err(|e| PrivacyError(format!("Deserialize decrypted intent: {}", e)))?;

            Ok(UnwrappedIntent::Plaintext(intent))
        } else {
            // We don't have the secret — return as opaque encrypted blob
            let ciphertext = hex::decode(&envelope.ciphertext).unwrap_or_default();
            let tee_pubkey = hex::decode(&envelope.tee_pubkey).unwrap_or_default();
            Ok(UnwrappedIntent::TeeEncrypted { ciphertext, tee_pubkey })
        }
    }

    fn verify(&self, _proof: &[u8], _public_inputs: &[u8]) -> Result<bool, PrivacyError> {
        // TEE attestation verification would go here
        // For MVP, trust the TEE enclave
        Ok(true)
    }

    fn name(&self) -> &'static str { "tee" }
}
