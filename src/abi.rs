//! EVM ABI Encoding for 0-dex Settlements
//!
//! Transforms matched Tensors into ABI-encoded bytes that can be submitted
//! to the ZeroDexEscrow.sol smart contract.

use ethabi::{Token, encode};
use zerolang::Tensor;
use std::str::FromStr;

/// Simulates extracting EVM parameters from a mathematically matched Tensor.
/// In a real system, the Tensor would contain indices or structural data 
/// mapping to specific token addresses and amounts.
pub fn encode_match_for_evm(
    local_address: &str,
    counterparty_address: &str,
    token_a: &str,
    token_b: &str,
    amount_a: u64,
    amount_b: u64,
    _settled_tensor: &Tensor,
) -> Result<Vec<u8>, String> {
    
    // Parse hex addresses into standard 20-byte arrays
    let addr_local = ethabi::Address::from_str(local_address)
        .map_err(|_| "Invalid local EVM address")?;
    let addr_cp = ethabi::Address::from_str(counterparty_address)
        .map_err(|_| "Invalid counterparty EVM address")?;
    let addr_token_a = ethabi::Address::from_str(token_a)
        .map_err(|_| "Invalid token A address")?;
    let addr_token_b = ethabi::Address::from_str(token_b)
        .map_err(|_| "Invalid token B address")?;

    // Encode parameters based on ZeroDexEscrow.sol `executeSwap` signature:
    // executeSwap(address partyA, address partyB, address tokenA, address tokenB, uint256 amountA, uint256 amountB)
    let tokens = vec![
        Token::Address(addr_local),
        Token::Address(addr_cp),
        Token::Address(addr_token_a),
        Token::Address(addr_token_b),
        Token::Uint(ethabi::Uint::from(amount_a)),
        Token::Uint(ethabi::Uint::from(amount_b)),
    ];

    let encoded = encode(&tokens);
    Ok(encoded)
}
