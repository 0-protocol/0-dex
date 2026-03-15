//! Cryptographic primitives for 0-dex EIP-712 signed intents.

use k256::ecdsa::{RecoveryId, Signature, VerifyingKey};
use sha3::{Digest, Keccak256};

/// Recovers the EVM address from a 32-byte digest and a 65-byte secp256k1 signature.
pub fn recover_address_from_digest(
    digest: &[u8; 32],
    signature_hex: &str,
) -> Result<String, String> {
    let sig_bytes = hex::decode(signature_hex.trim_start_matches("0x"))
        .map_err(|e| format!("Invalid hex signature: {e}"))?;
    if sig_bytes.len() != 65 {
        return Err("Invalid signature length, expected 65 bytes".to_string());
    }

    let signature = Signature::from_slice(&sig_bytes[0..64])
        .map_err(|e| format!("Malformed signature: {e}"))?;
    let v = normalize_recovery_id(sig_bytes[64])?;
    let recid = RecoveryId::try_from(v).map_err(|e| format!("Invalid recovery id: {e}"))?;

    let recovered_key = VerifyingKey::recover_from_prehash(digest, &signature, recid)
        .map_err(|e| format!("Failed to recover public key: {e}"))?;
    Ok(public_key_to_address(&recovered_key))
}

fn normalize_recovery_id(v: u8) -> Result<u8, String> {
    match v {
        27 | 28 => Ok(v - 27),
        0 | 1 => Ok(v),
        _ => Err("Unsupported recovery id".to_string()),
    }
}

fn public_key_to_address(key: &VerifyingKey) -> String {
    let encoded_point = key.to_encoded_point(false);
    let mut hasher = Keccak256::new();
    hasher.update(&encoded_point.as_bytes()[1..]);
    let address_hash = hasher.finalize();
    format!("0x{}", hex::encode(&address_hash[12..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_recovery_id_handles_all_formats() {
        assert_eq!(normalize_recovery_id(0).unwrap(), 0);
        assert_eq!(normalize_recovery_id(1).unwrap(), 1);
        assert_eq!(normalize_recovery_id(27).unwrap(), 0);
        assert_eq!(normalize_recovery_id(28).unwrap(), 1);
        assert!(normalize_recovery_id(2).is_err());
    }
}
