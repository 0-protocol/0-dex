use ethabi::{encode, Token};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Keccak256};
use std::str::FromStr;

use crate::crypto;

pub const PROTOCOL_VERSION: &str = "0-dex-v1";
pub const MAX_GRAPH_BYTES: usize = 32 * 1024;

pub const EIP712_DOMAIN_TYPE: &[u8] =
    b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)";
pub const INTENT_TYPE: &[u8] =
    b"Intent(address owner,address tokenIn,address tokenOut,uint256 amountIn,uint256 minAmountOut,uint256 nonce,uint256 deadline)";
pub const DOMAIN_NAME: &[u8] = b"ZeroDexEscrow";
pub const DOMAIN_VERSION: &[u8] = b"1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum OrderSide {
    Buy,
    Sell,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntentPayload {
    pub version: String,
    pub chain_id: u64,
    pub nonce: u64,
    pub deadline_unix: u64,
    pub owner_address: String,
    pub verifying_contract: String,
    pub base_token: String,
    pub quote_token: String,
    pub side: OrderSide,
    pub amount_in: u128,
    pub min_amount_out: u128,
    pub graph_content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedIntent {
    #[serde(flatten)]
    pub payload: IntentPayload,
    pub signature_hex: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchProof {
    pub match_id: String,
    pub maker_intent: SignedIntent,
    pub taker_intent: SignedIntent,
    pub amount_a: u128,
    pub amount_b: u128,
    pub matched_at_unix: u64,
    pub relayer: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedIntent {
    pub owner: String,
    pub token_in: String,
    pub token_out: String,
    pub amount_in: u128,
    pub min_amount_out: u128,
    pub nonce: u64,
    pub deadline_unix: u64,
    pub chain_id: u64,
}

impl SignedIntent {
    /// Computes the EIP-712 digest for this intent.
    /// Returns the 32-byte hash that was signed.
    pub fn eip712_digest(&self) -> Result<[u8; 32], String> {
        let resolved = self.resolved_intent()?;
        let verifying_contract =
            parse_address(&self.payload.verifying_contract, "invalid verifying_contract")?;
        let owner = parse_address(&resolved.owner, "invalid owner_address")?;
        let token_in = parse_address(&resolved.token_in, "invalid token_in")?;
        let token_out = parse_address(&resolved.token_out, "invalid token_out")?;

        let domain_separator = compute_domain_separator(verifying_contract, resolved.chain_id);

        let struct_hash = keccak(&encode(&[
            Token::FixedBytes(keccak(INTENT_TYPE).to_vec()),
            Token::Address(owner),
            Token::Address(token_in),
            Token::Address(token_out),
            Token::Uint(resolved.amount_in.into()),
            Token::Uint(resolved.min_amount_out.into()),
            Token::Uint(resolved.nonce.into()),
            Token::Uint(resolved.deadline_unix.into()),
        ]));

        let mut eip712_input = Vec::with_capacity(66);
        eip712_input.extend_from_slice(&[0x19, 0x01]);
        eip712_input.extend_from_slice(&domain_separator);
        eip712_input.extend_from_slice(&struct_hash);
        Ok(keccak(&eip712_input))
    }

    pub fn validate_basic(
        &self,
        now_unix: u64,
        expected_chain_id: u64,
        expected_verifying_contract: &str,
    ) -> Result<(), String> {
        if self.payload.version != PROTOCOL_VERSION {
            return Err("Unsupported protocol version".to_string());
        }
        if self.payload.owner_address.is_empty()
            || self.payload.verifying_contract.is_empty()
            || self.payload.base_token.is_empty()
            || self.payload.quote_token.is_empty()
        {
            return Err("Address fields cannot be empty".to_string());
        }
        if self
            .payload
            .base_token
            .eq_ignore_ascii_case(&self.payload.quote_token)
        {
            return Err("base_token and quote_token cannot match".to_string());
        }
        if self.payload.deadline_unix < now_unix {
            return Err("Intent expired".to_string());
        }
        if self.payload.amount_in == 0 || self.payload.min_amount_out == 0 {
            return Err("Intent amounts must be non-zero".to_string());
        }
        if self.payload.graph_content.len() > MAX_GRAPH_BYTES {
            return Err("Graph payload exceeds maximum size".to_string());
        }
        if self.payload.chain_id != expected_chain_id {
            return Err("Intent chain_id mismatch".to_string());
        }
        if self
            .payload
            .verifying_contract
            .eq_ignore_ascii_case("0x0000000000000000000000000000000000000000")
        {
            return Err("verifying_contract cannot be zero".to_string());
        }
        if !self
            .payload
            .verifying_contract
            .eq_ignore_ascii_case(expected_verifying_contract)
        {
            return Err("verifying_contract mismatch".to_string());
        }
        Ok(())
    }

    pub fn verify_signature(&self) -> Result<bool, String> {
        let digest = self.eip712_digest()?;
        crypto::recover_address_from_digest(&digest, &self.signature_hex)
            .map(|recovered| recovered.eq_ignore_ascii_case(&self.payload.owner_address))
    }

    pub fn resolved_intent(&self) -> Result<ResolvedIntent, String> {
        resolve_payload(&self.payload)
    }
}

pub fn compute_match_id(
    maker: &SignedIntent,
    taker: &SignedIntent,
    amount_a: u128,
    amount_b: u128,
) -> String {
    let maker_addr = normalize_address(&maker.payload.owner_address);
    let taker_addr = normalize_address(&taker.payload.owner_address);
    let mut hasher = Keccak256::new();
    hasher.update(maker_addr.as_bytes());
    hasher.update(maker.payload.nonce.to_le_bytes());
    hasher.update(taker_addr.as_bytes());
    hasher.update(taker.payload.nonce.to_le_bytes());
    hasher.update(amount_a.to_le_bytes());
    hasher.update(amount_b.to_le_bytes());
    format!("0x{}", hex::encode(hasher.finalize()))
}

pub fn resolve_payload(payload: &IntentPayload) -> Result<ResolvedIntent, String> {
    let (token_in, token_out) = match payload.side {
        OrderSide::Sell => (&payload.base_token, &payload.quote_token),
        OrderSide::Buy => (&payload.quote_token, &payload.base_token),
    };
    Ok(ResolvedIntent {
        owner: payload.owner_address.clone(),
        token_in: token_in.clone(),
        token_out: token_out.clone(),
        amount_in: payload.amount_in,
        min_amount_out: payload.min_amount_out,
        nonce: payload.nonce,
        deadline_unix: payload.deadline_unix,
        chain_id: payload.chain_id,
    })
}

pub fn compute_domain_separator(verifying_contract: ethabi::Address, chain_id: u64) -> [u8; 32] {
    keccak(&encode(&[
        Token::FixedBytes(keccak(EIP712_DOMAIN_TYPE).to_vec()),
        Token::FixedBytes(keccak(DOMAIN_NAME).to_vec()),
        Token::FixedBytes(keccak(DOMAIN_VERSION).to_vec()),
        Token::Uint(chain_id.into()),
        Token::Address(verifying_contract),
    ]))
}

fn normalize_address(addr: &str) -> String {
    addr.to_ascii_lowercase()
}

fn parse_address(value: &str, err: &str) -> Result<ethabi::Address, String> {
    ethabi::Address::from_str(value).map_err(|_| err.to_string())
}

fn keccak(bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Keccak256::new();
    hasher.update(bytes);
    let output = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&output);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ethers::signers::{LocalWallet, Signer};

    fn sample_payload() -> IntentPayload {
        IntentPayload {
            version: PROTOCOL_VERSION.to_string(),
            chain_id: 1,
            nonce: 1,
            deadline_unix: 4_102_444_800,
            owner_address: String::new(),
            verifying_contract: "0x4444444444444444444444444444444444444444".to_string(),
            base_token: "0x1111111111111111111111111111111111111111".to_string(),
            quote_token: "0x2222222222222222222222222222222222222222".to_string(),
            side: OrderSide::Sell,
            amount_in: 100,
            min_amount_out: 200,
            graph_content: "graph".to_string(),
        }
    }

    #[tokio::test]
    async fn verifies_eip712_sign_flow() {
        let wallet: LocalWallet =
            "0x59c6995e998f97a5a0044976f8f2b8d2f22ebf0c6f0f4f7f3afccf4d7ed2d1a5"
                .parse()
                .expect("wallet");
        let mut payload = sample_payload();
        payload.owner_address = format!("{:?}", wallet.address());

        let intent = SignedIntent {
            payload: payload.clone(),
            signature_hex: String::new(),
        };
        let digest = intent.eip712_digest().expect("digest");

        let signature = wallet.sign_hash(digest.into()).expect("sign");
        let signed = SignedIntent {
            payload,
            signature_hex: format!("0x{}", signature),
        };
        assert!(signed.verify_signature().expect("verify"));
    }

    #[test]
    fn eip712_digest_is_deterministic() {
        let mut payload = sample_payload();
        payload.owner_address = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();
        let intent = SignedIntent {
            payload,
            signature_hex: "0x".to_string(),
        };
        let d1 = intent.eip712_digest().expect("d1");
        let d2 = intent.eip712_digest().expect("d2");
        assert_eq!(d1, d2);
    }

    #[test]
    fn buy_side_produces_different_digest() {
        let mut sell_payload = sample_payload();
        sell_payload.owner_address = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();
        sell_payload.side = OrderSide::Sell;
        let sell = SignedIntent {
            payload: sell_payload,
            signature_hex: "0x".to_string(),
        };

        let mut buy_payload = sample_payload();
        buy_payload.owner_address = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_string();
        buy_payload.side = OrderSide::Buy;
        let buy = SignedIntent {
            payload: buy_payload,
            signature_hex: "0x".to_string(),
        };

        assert_ne!(
            sell.eip712_digest().expect("sell"),
            buy.eip712_digest().expect("buy")
        );
    }

    #[test]
    fn wrong_signer_rejected() {
        let mut payload = sample_payload();
        payload.owner_address = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
        let intent = SignedIntent {
            payload,
            signature_hex: "0x0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ff".to_string(),
        };
        let result = intent.verify_signature();
        assert!(result.is_err() || result == Ok(false));
    }
}
