//! EVM ABI Encoding for ZeroDexEscrow.executeSwap.

use ethabi::{encode, ethereum_types::H256, Token};
use sha3::{Digest, Keccak256};
use std::str::FromStr;

use crate::protocol::{resolve_payload, MatchProof};

pub fn encode_match_for_evm(proof: &MatchProof) -> Result<Vec<u8>, String> {
    let maker = intent_tuple(&proof.maker_intent.payload)?;
    let taker = intent_tuple(&proof.taker_intent.payload)?;
    let amount_a = Token::Uint(ethabi::Uint::from(proof.amount_a));
    let amount_b = Token::Uint(ethabi::Uint::from(proof.amount_b));
    let match_id = parse_h256(&proof.match_id)?;
    let sig_a = parse_bytes(&proof.maker_intent.signature_hex)?;
    let sig_b = parse_bytes(&proof.taker_intent.signature_hex)?;

    let mut out = selector();
    out.extend(encode(&[
        maker,
        taker,
        amount_a,
        amount_b,
        Token::FixedBytes(match_id.as_fixed_bytes().to_vec()),
        Token::Bytes(sig_a),
        Token::Bytes(sig_b),
    ]));
    Ok(out)
}

fn intent_tuple(payload: &crate::protocol::IntentPayload) -> Result<Token, String> {
    let resolved = resolve_payload(payload)?;
    let owner = ethabi::Address::from_str(&resolved.owner)
        .map_err(|_| "Invalid owner address".to_string())?;
    let token_in = ethabi::Address::from_str(&resolved.token_in)
        .map_err(|_| "Invalid token_in address".to_string())?;
    let token_out = ethabi::Address::from_str(&resolved.token_out)
        .map_err(|_| "Invalid token_out address".to_string())?;

    Ok(Token::Tuple(vec![
        Token::Address(owner),
        Token::Address(token_in),
        Token::Address(token_out),
        Token::Uint(resolved.amount_in.into()),
        Token::Uint(resolved.min_amount_out.into()),
        Token::Uint(resolved.nonce.into()),
        Token::Uint(resolved.deadline_unix.into()),
        Token::Uint(resolved.chain_id.into()),
    ]))
}

fn parse_bytes(value: &str) -> Result<Vec<u8>, String> {
    hex::decode(value.trim_start_matches("0x")).map_err(|e| format!("Invalid hex bytes: {e}"))
}

fn parse_h256(value: &str) -> Result<H256, String> {
    H256::from_str(value).map_err(|e| format!("Invalid match_id: {e}"))
}

fn selector() -> Vec<u8> {
    let sig = "executeSwap((address,address,address,uint256,uint256,uint256,uint256,uint256),(address,address,address,uint256,uint256,uint256,uint256,uint256),uint256,uint256,bytes32,bytes,bytes)";
    let mut hasher = Keccak256::new();
    hasher.update(sig.as_bytes());
    hasher.finalize()[..4].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{IntentPayload, MatchProof, OrderSide, SignedIntent, PROTOCOL_VERSION};

    fn intent(owner: &str, side: OrderSide) -> SignedIntent {
        SignedIntent {
            payload: IntentPayload {
                version: PROTOCOL_VERSION.to_string(),
                chain_id: 1,
                nonce: 1,
                deadline_unix: 4_102_444_800,
                owner_address: owner.to_string(),
                verifying_contract: "0x4444444444444444444444444444444444444444".to_string(),
                base_token: "0x1111111111111111111111111111111111111111".to_string(),
                quote_token: "0x2222222222222222222222222222222222222222".to_string(),
                side,
                amount_in: 100,
                min_amount_out: 200,
                graph_content: "graph".to_string(),
            },
            signature_hex: "0x1234".to_string(),
        }
    }

    #[test]
    fn encoded_payload_has_selector_prefix() {
        let proof = MatchProof {
            match_id: "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_string(),
            maker_intent: intent(
                "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                OrderSide::Sell,
            ),
            taker_intent: intent("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", OrderSide::Buy),
            amount_a: 10,
            amount_b: 20,
            matched_at_unix: 1,
            relayer: None,
        };
        let encoded = encode_match_for_evm(&proof).expect("encode");
        assert!(encoded.len() > 4);
        assert_eq!(&encoded[..4], &selector());
    }

    #[test]
    fn buy_side_flips_token_flow() {
        let sell = intent(
            "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            OrderSide::Sell,
        );
        let buy = intent("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb", OrderSide::Buy);
        let sell_tuple = intent_tuple(&sell.payload).expect("sell tuple");
        let buy_tuple = intent_tuple(&buy.payload).expect("buy tuple");
        let sell_fields = match sell_tuple {
            Token::Tuple(v) => v,
            _ => panic!("tuple expected"),
        };
        let buy_fields = match buy_tuple {
            Token::Tuple(v) => v,
            _ => panic!("tuple expected"),
        };
        assert_eq!(sell_fields[1], buy_fields[2]);
        assert_eq!(sell_fields[2], buy_fields[1]);
    }
}
