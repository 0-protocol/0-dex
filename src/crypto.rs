//! Cryptographic Identity and Signature Verification for 0-dex
//!
//! Binds 0-lang intent graphs to on-chain identities (EVM wallets via secp256k1).

use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use k256::ecdsa::signature::Verifier;
use sha3::{Digest, Keccak256};
use serde::{Deserialize, Serialize};

/// Represents a cryptographically signed intent graph payload.
/// This is what agents actually broadcast over the Gossipsub network.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SignedIntent {
    /// The raw 0-lang graph code (e.g. `.0` content)
    pub graph_content: String,
    /// The EVM public address (0x...) of the agent
    pub owner_address: String,
    /// The hex-encoded secp256k1 signature of the graph content
    pub signature_hex: String,
}

impl SignedIntent {
    /// Compute the Keccak256 hash of the graph payload, similar to Ethereum signed messages.
    pub fn compute_hash(&self) -> [u8; 32] {
        let prefix = format!("\x190-dex Intent:\n{}", self.graph_content.len());
        let mut hasher = Keccak256::new();
        hasher.update(prefix.as_bytes());
        hasher.update(self.graph_content.as_bytes());
        let result = hasher.finalize();
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&result);
        hash
    }

    /// Verifies the signature against the claimed owner address.
    pub fn verify(&self) -> Result<bool, String> {
        // 1. Decode hex signature
        let sig_bytes = hex::decode(self.signature_hex.trim_start_matches("0x"))
            .map_err(|e| format!("Invalid hex signature: {}", e))?;
            
        if sig_bytes.len() != 65 {
            return Err("Invalid signature length (expected 65 bytes with recovery id)".into());
        }

        // 2. Parse signature and recovery ID
        let signature = Signature::from_slice(&sig_bytes[0..64])
            .map_err(|e| format!("Malformed signature: {}", e))?;
        let recid = RecoveryId::try_from(sig_bytes[64] % 27)
            .map_err(|e| format!("Invalid recovery ID: {}", e))?;

        // 3. Recover the public key from the hash
        let hash = self.compute_hash();
        let recovered_key = VerifyingKey::recover_from_prehash(&hash, &signature, recid)
            .map_err(|e| format!("Failed to recover public key: {}", e))?;

        // 4. Derive EVM address from recovered public key
        let encoded_point = recovered_key.to_encoded_point(false);
        let uncompressed_pubkey = encoded_point.as_bytes(); // starts with 0x04
        
        let mut address_hasher = Keccak256::new();
        address_hasher.update(&uncompressed_pubkey[1..]); // strip 0x04 prefix
        let address_hash = address_hasher.finalize();
        
        let mut recovered_address = [0u8; 20];
        recovered_address.copy_from_slice(&address_hash[12..32]); // take last 20 bytes
        let recovered_address_hex = format!("0x{}", hex::encode(recovered_address));

        // 5. Check if recovered address matches claimed owner
        let matches = recovered_address_hex.eq_ignore_ascii_case(&self.owner_address);
        if !matches {
            tracing::warn!(
                "Signature verification failed: Recovered {} != Claimed {}", 
                recovered_address_hex, 
                self.owner_address
            );
        }
        
        Ok(matches)
    }
}
