//! EVM ABI Encoding for 0-dex Settlements
//!
//! Transforms matched Tensors into ABI-encoded calldata for the
//! ZeroDexEscrow.sol `executeSwap` function.

use ethabi::{Token, encode};
use sha3::{Digest, Keccak256};
use zerolang::Tensor;
use std::str::FromStr;

/// 4-byte function selector for:
///   executeSwap(address,address,address,address,uint256,uint256,bytes,bytes)
fn execute_swap_selector() -> [u8; 4] {
    let mut hasher = Keccak256::new();
    hasher.update(b"executeSwap(address,address,address,address,uint256,uint256,bytes,bytes)");
    let hash = hasher.finalize();
    let mut selector = [0u8; 4];
    selector.copy_from_slice(&hash[..4]);
    selector
}

/// Builds the full EVM calldata for `ZeroDexEscrow.executeSwap`.
pub fn encode_match_for_evm(
    local_address: &str,
    counterparty_address: &str,
    token_a: &str,
    token_b: &str,
    amount_a: u64,
    amount_b: u64,
    signature_a: &[u8],
    signature_b: &[u8],
    _settled_tensor: &Tensor,
) -> Result<Vec<u8>, String> {
    let addr_local = ethabi::Address::from_str(local_address)
        .map_err(|_| "Invalid local EVM address")?;
    let addr_cp = ethabi::Address::from_str(counterparty_address)
        .map_err(|_| "Invalid counterparty EVM address")?;
    let addr_token_a = ethabi::Address::from_str(token_a)
        .map_err(|_| "Invalid token A address")?;
    let addr_token_b = ethabi::Address::from_str(token_b)
        .map_err(|_| "Invalid token B address")?;

    let tokens = vec![
        Token::Address(addr_local),
        Token::Address(addr_cp),
        Token::Address(addr_token_a),
        Token::Address(addr_token_b),
        Token::Uint(ethabi::Uint::from(amount_a)),
        Token::Uint(ethabi::Uint::from(amount_b)),
        Token::Bytes(signature_a.to_vec()),
        Token::Bytes(signature_b.to_vec()),
    ];

    let mut calldata = execute_swap_selector().to_vec();
    calldata.extend_from_slice(&encode(&tokens));
    Ok(calldata)
}
