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
    allow_simulation: bool,
}

impl SettlementEngine {
    pub fn new(
        rpc_url: &str,
        chain_id: u64,
        escrow_address: &str,
        match_receiver: mpsc::Receiver<MatchProof>,
    ) -> Result<Self, String> {
        let key_provider = EnvKeyProvider::from_env();
        let allow_simulation = std::env::var("ZERO_DEX_ALLOW_SIMULATION")
            .map(|v| v.eq_ignore_ascii_case("true"))
            .unwrap_or(false);
        let parsed = escrow_address
            .parse::<Address>()
            .unwrap_or_else(|_| Address::zero());
        if key_provider.is_none() {
            if allow_simulation {
                warn!("No relayer key provider configured — settlements will be simulated");
            } else {
                return Err(
                    "ZERO_DEX_RELAYER_KEY is required (or set ZERO_DEX_ALLOW_SIMULATION=true for non-production)"
                        .to_string(),
                );
            }
        }
        Ok(Self {
            rpc_url: rpc_url.to_string(),
            chain_id,
            escrow_address: parsed,
            match_receiver,
            key_provider,
            allow_simulation,
        })
    }

    pub async fn run(mut self) {
        info!(
            "Settlement engine listening on rpc={} chain_id={} escrow={:?} mode={}",
            self.rpc_url,
            self.chain_id,
            self.escrow_address,
            self.mode_name()
        );

        while let Some(match_proof) = self.match_receiver.recv().await {
            info!(
                "Received new match proof for settlement: {}",
                match_proof.match_id
            );
            if let Err(e) = self.execute_swap(match_proof).await {
                error!("Settlement failed: {e}");
            }
        }
    }

    pub fn mode_name(&self) -> &'static str {
        if self.key_provider.is_some() {
            "onchain"
        } else if self.allow_simulation {
            "simulation"
        } else {
            "disabled"
        }
    }

    async fn execute_swap(&self, proof: MatchProof) -> Result<(), String> {
        debug!("Preparing settlement tx for match_id={}", proof.match_id);
        let encoded_data = match crate::abi::encode_match_for_evm(&proof) {
            Ok(data) => data,
            Err(e) => {
                return Err(format!("Failed to encode ABI payload for settlement: {e}"));
            }
        };
        info!(
            "ABI-encoded settlement payload: 0x{}",
            hex::encode(&encoded_data)
        );

        let Some(ref key_provider) = self.key_provider else {
            if !self.allow_simulation {
                return Err("Missing relayer key and simulation is disabled".to_string());
            }
            info!(
                "No key provider. Simulating settlement for match_id={}",
                proof.match_id
            );
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
            info!("Trade Settled (Simulated)! match_id={}", proof.match_id);
            return Ok(());
        };

        if self.escrow_address == Address::zero() {
            return Err("Escrow address is zero. Refusing to broadcast.".to_string());
        }

        let wallet = match key_provider.get_wallet(self.chain_id).await {
            Ok(w) => w,
            Err(e) => {
                return Err(format!("Key provider error: {e}"));
            }
        };

        let provider = match Provider::<Http>::try_from(self.rpc_url.as_str()) {
            Ok(p) => p,
            Err(e) => {
                return Err(format!("Failed to connect to RPC provider: {e}"));
            }
        };

        let client = Arc::new(SignerMiddleware::new(provider, wallet));
        let tx = TransactionRequest::new()
            .to(self.escrow_address)
            .data(encoded_data);

        info!("Submitting atomic swap transaction to the blockchain");
        let submission = client.send_transaction(tx, None).await;
        let pending_tx = match submission {
            Ok(pending_tx) => pending_tx,
            Err(e) => {
                return Err(format!("Failed to broadcast transaction: {e}"));
            }
        };
        info!("Trade submitted! Tx Hash: {:?}", pending_tx.tx_hash());
        let receipt = pending_tx
            .await
            .map_err(|e| format!("Failed waiting for transaction receipt: {e}"))?
            .ok_or_else(|| "No transaction receipt returned".to_string())?;
        if receipt.status != Some(U64::from(1u64)) {
            return Err(format!(
                "Settlement transaction reverted or failed. tx_hash={:?} status={:?}",
                receipt.transaction_hash, receipt.status
            ));
        }
        info!(
            "Trade Settled On-Chain in block {} for match_id={}",
            receipt.block_number.unwrap_or_default(),
            proof.match_id
        );
        Ok(())
    }
}
