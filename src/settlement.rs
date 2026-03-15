//! On-chain atomic settlement layer for canonical match proofs.

use ethers::prelude::*;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};

use crate::protocol::MatchProof;

#[async_trait::async_trait]
pub trait KeyProvider: Send + Sync {
    async fn get_wallet(&self, chain_id: u64) -> Result<LocalWallet, String>;
}

pub struct EnvKeyProvider {
    env_var: String,
}

impl EnvKeyProvider {
    pub fn new(env_var: &str) -> Self {
        Self {
            env_var: env_var.to_string(),
        }
    }

    pub fn from_env() -> Option<Box<dyn KeyProvider>> {
        if std::env::var("ZERO_DEX_RELAYER_KEY").is_ok() {
            Some(Box::new(Self::new("ZERO_DEX_RELAYER_KEY")))
        } else {
            None
        }
    }
}

#[async_trait::async_trait]
impl KeyProvider for EnvKeyProvider {
    async fn get_wallet(&self, chain_id: u64) -> Result<LocalWallet, String> {
        let key = std::env::var(&self.env_var)
            .map_err(|_| format!("Environment variable {} not set", self.env_var))?;
        let wallet: LocalWallet = key
            .parse()
            .map_err(|_| "Invalid relayer private key format".to_string())?;
        Ok(wallet.with_chain_id(chain_id))
    }
}

pub struct SettlementEngine {
    rpc_url: String,
    chain_id: u64,
    escrow_address: Address,
    match_receiver: mpsc::Receiver<MatchProof>,
    key_provider: Option<Box<dyn KeyProvider>>,
}

impl SettlementEngine {
    pub fn new(
        rpc_url: &str,
        chain_id: u64,
        escrow_address: &str,
        match_receiver: mpsc::Receiver<MatchProof>,
    ) -> Self {
        let key_provider = EnvKeyProvider::from_env();
        let parsed = escrow_address
            .parse::<Address>()
            .unwrap_or_else(|_| Address::zero());
        if key_provider.is_none() {
            warn!("No relayer key provider configured — settlements will be simulated");
        }
        Self {
            rpc_url: rpc_url.to_string(),
            chain_id,
            escrow_address: parsed,
            match_receiver,
            key_provider,
        }
    }

    pub fn with_key_provider(mut self, provider: Box<dyn KeyProvider>) -> Self {
        self.key_provider = Some(provider);
        self
    }

    pub async fn run(mut self) {
        info!(
            "Settlement engine listening on rpc={} chain_id={} escrow={:?}",
            self.rpc_url, self.chain_id, self.escrow_address
        );

        while let Some(match_proof) = self.match_receiver.recv().await {
            info!(
                "Received new match proof for settlement: {}",
                match_proof.match_id
            );
            self.execute_swap(match_proof).await;
        }
    }

    async fn execute_swap(&self, proof: MatchProof) {
        debug!("Preparing settlement tx for match_id={}", proof.match_id);
        let encoded_data = match crate::abi::encode_match_for_evm(&proof) {
            Ok(data) => data,
            Err(e) => {
                error!("Failed to encode ABI payload for settlement: {}", e);
                return;
            }
        };
        info!(
            "ABI-encoded settlement payload: 0x{}",
            hex::encode(&encoded_data)
        );

        let Some(ref key_provider) = self.key_provider else {
            info!("No key provider. Simulating settlement for match_id={}", proof.match_id);
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            info!("Trade Settled (Simulated)! match_id={}", proof.match_id);
            return;
        };

        if self.escrow_address == Address::zero() {
            error!("Escrow address is zero. Refusing to broadcast.");
            return;
        }

        let wallet = match key_provider.get_wallet(self.chain_id).await {
            Ok(w) => w,
            Err(e) => {
                error!("Key provider error: {}", e);
                return;
            }
        };

        let provider = match Provider::<Http>::try_from(self.rpc_url.as_str()) {
            Ok(p) => p,
            Err(e) => {
                error!("Failed to connect to RPC provider: {}", e);
                return;
            }
        };

        let client = Arc::new(SignerMiddleware::new(provider, wallet));
        let tx = TransactionRequest::new()
            .to(self.escrow_address)
            .data(encoded_data);

        info!("Submitting atomic swap transaction to the blockchain");
        match client.send_transaction(tx, None).await {
            Ok(pending_tx) => {
                info!("Trade submitted! Tx Hash: {:?}", pending_tx.tx_hash());
                if let Ok(Some(receipt)) = pending_tx.await {
                    info!(
                        "Trade Settled On-Chain in block {} for match_id={}",
                        receipt.block_number.unwrap_or_default(),
                        proof.match_id
                    );
                }
            }
            Err(e) => {
                error!("Failed to broadcast transaction: {}", e);
            }
        }
    }
}
