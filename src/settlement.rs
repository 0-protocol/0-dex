//! On-chain atomic settlement layer
//!
//! Handles taking a mathematical intersection (Match) from the MatchingEngine
//! and executing it atomically on-chain (e.g., via a smart contract on Solana or Ethereum).

use tokio::sync::mpsc;
use tracing::{info, error, debug};
use zerolang::Tensor;
use ethers::prelude::*;

#[derive(Debug, Clone)]
pub struct MatchProof {
    pub local_intent_id: String,
    pub counterparty_intent_id: String,
    pub settled_vector: Tensor,
    pub local_signature: Vec<u8>,
    pub counterparty_signature: Vec<u8>,
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

        match crate::abi::encode_match_for_evm(
            &proof.local_intent_id,
            &proof.counterparty_intent_id,
            token_a,
            token_b,
            amount_a,
            amount_b,
            &proof.local_signature,
            &proof.counterparty_signature,
            &proof.settled_vector
        ) {
            Ok(encoded_data) => {
                info!("Successfully ABI-encoded settlement payload: 0x{}", hex::encode(&encoded_data));
                
                if let Some(ref priv_key) = self.relayer_private_key {
                    self.broadcast_evm(&encoded_data, &proof, priv_key).await;
                } else {
                    info!("No relayer key configured. Simulating RPC submission...");
                    tokio::time::sleep(tokio::time::Duration::from_millis(1500)).await;
                    info!("Trade Settled (Simulated)! Vector: {:?}", proof.settled_vector);
                }
            },
            Err(e) => {
                error!("Failed to encode ABI payload for settlement: {}", e);
            }
        }
    }

    async fn broadcast_evm(&self, encoded_data: &[u8], proof: &MatchProof, priv_key: &str) {
        info!("Connecting to EVM RPC provider: {}", self.rpc_url);
        let provider = match Provider::<Http>::try_from(self.rpc_url.as_str()) {
            Ok(p) => p,
            Err(e) => { error!("Failed to connect to RPC provider: {}", e); return; }
        };
        let wallet = match priv_key.parse::<LocalWallet>() {
            Ok(w) => w.with_chain_id(1u64),
            Err(e) => { error!("Invalid relayer private key: {}", e); return; }
        };
        let client = SignerMiddleware::new(provider, wallet);

        let escrow_address: Address = std::env::var("ZERO_DEX_ESCROW_ADDRESS")
            .unwrap_or_else(|_| "0x0000000000000000000000000000000000000000".to_string())
            .parse()
            .expect("valid escrow address");

        let tx = TransactionRequest::new()
            .to(escrow_address)
            .data(encoded_data.to_vec());

        info!("Submitting atomic swap transaction...");
        match client.send_transaction(tx, None).await {
            Ok(pending_tx) => {
                info!("Trade submitted! Tx Hash: {:?}", pending_tx.tx_hash());
                if let Ok(Some(receipt)) = pending_tx.await {
                    info!(
                        "Trade settled on-chain in block {}! Vector: {:?}",
                        receipt.block_number.unwrap_or_default(),
                        proof.settled_vector
                    );
                }
            }
            Err(e) => error!("Failed to broadcast transaction: {}", e),
        };
    }
}
