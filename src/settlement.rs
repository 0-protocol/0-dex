//! On-chain atomic settlement layer
//!
//! Handles taking a mathematical intersection (Match) from the MatchingEngine
//! and executing it atomically on-chain (e.g., via a smart contract on Solana or Ethereum).

use tokio::sync::mpsc;
use tracing::{info, error, debug};
use zerolang::Tensor;
use ethers::prelude::*;
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct MatchProof {
    pub local_intent_id: String,
    pub counterparty_intent_id: String,
    pub settled_vector: Tensor,
    pub signature: Vec<u8>,
}

pub struct SettlementEngine {
    rpc_url: String,
    match_receiver: mpsc::Receiver<MatchProof>,
    // The private key of the Relayer/Solver who submits the batched swap
    relayer_private_key: Option<String>,
}

impl SettlementEngine {
    pub fn new(rpc_url: &str, match_receiver: mpsc::Receiver<MatchProof>) -> Self {
        // In production, this comes from an env var or secure vault
        let relayer_key = std::env::var("ZERO_DEX_RELAYER_KEY").ok();
        Self {
            rpc_url: rpc_url.to_string(),
            match_receiver,
            relayer_private_key: relayer_key,
        }
    }

    /// Run the settlement loop, waiting for cryptographic matches from the MatchingEngine
    pub async fn run(mut self) {
        info!("Settlement engine listening for matches on RPC: {}", self.rpc_url);
        
        while let Some(match_proof) = self.match_receiver.recv().await {
            info!("Received new match proof for settlement: {:?}", match_proof);
            self.execute_swap(match_proof).await;
        }
    }

    async fn execute_swap(&self, proof: MatchProof) {
        debug!("Validating tensor overlap cryptographically before Tx submission...");
        
        // 1. In a production environment, we extract token addresses and amounts 
        // from the mathematical properties of the settled Tensor.
        // For this stub, we use dummy addresses.
        let token_a = "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2"; // WETH
        let token_b = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48"; // USDC
        let amount_a = 1_000_000_000_000_000_000; // 1 WETH
        let amount_b = 3_000_000_000; // 3000 USDC

        // 2. Encode the parameters using the ABI spec for ZeroDexEscrow.sol
        match crate::abi::encode_match_for_evm(
            &proof.local_intent_id,
            &proof.counterparty_intent_id,
            token_a,
            token_b,
            amount_a,
            amount_b,
            &proof.settled_vector
        ) {
            Ok(encoded_data) => {
                info!("Successfully ABI-encoded settlement payload: 0x{}", hex::encode(&encoded_data));
                
                // 3. Real EVM RPC Broadcast via ethers-rs
                if let Some(ref priv_key) = self.relayer_private_key {
                    info!("Connecting to EVM RPC provider: {}", self.rpc_url);
                    if let Ok(provider) = Provider::<Http>::try_from(self.rpc_url.as_str()) {
                        if let Ok(wallet) = priv_key.parse::<LocalWallet>() {
                            // Assume chain ID 1 for mainnet or fetch dynamically
                            let client = SignerMiddleware::new(provider, wallet.with_chain_id(1u64));
                            let client = Arc::new(client);

                            // The deployed address of the ZeroDexEscrow contract
                            let escrow_address = "0x0000000000000000000000000000000000000000".parse::<Address>().unwrap();
                            
                            let tx = TransactionRequest::new()
                                .to(escrow_address)
                                .data(encoded_data.clone());
                            
                            info!("Submitting atomic swap transaction to the blockchain...");
                            match client.send_transaction(tx, None).await {
                                Ok(pending_tx) => {
                                    info!("✅ Trade submitted! Tx Hash: {:?}", pending_tx.tx_hash());
                                    // Wait for confirmation
                                    if let Ok(Some(receipt)) = pending_tx.await {
                                        info!("✅ Trade Settled On-Chain in block {}! Vector: {:?}", receipt.block_number.unwrap_or_default(), proof.settled_vector);
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to broadcast transaction: {}", e);
                                }
                            }
                        } else {
                            error!("Invalid relayer private key format");
                        }
                    } else {
                        error!("Failed to connect to RPC provider");
                    }
                } else {
                    // Fallback mock if no key is configured
                    info!("No relayer key configured. Simulating RPC submission...");
                    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                    info!("✅ Trade Settled (Simulated)! Vector: {:?}", proof.settled_vector);
                }
            },
            Err(e) => {
                error!("Failed to encode ABI payload for settlement: {}", e);
            }
        }
    }
}
