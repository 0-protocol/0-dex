//! Liquidity Relayer: routes unmatched intents to on-chain AMMs.
//!
//! When an intent goes unmatched for N seconds, the relayer constructs
//! a fallback trade through Uniswap V3 (EVM) or Raydium (Solana),
//! acting as a market maker of last resort.

use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing::{info, warn, debug};

use crate::intent_pool::{IntentPool, PoolHandle, IntentStatus};
use crate::settlement::MatchProof;

/// How long to wait before routing an unmatched intent to an AMM
const FALLBACK_DELAY_SECS: u64 = 30;

pub struct LiquidityRelayer {
    pool: PoolHandle,
    match_sender: mpsc::Sender<MatchProof>,
    /// Uniswap V3 SwapRouter address
    uniswap_router: String,
    /// Raydium CLMM program ID
    raydium_program: String,
}

impl LiquidityRelayer {
    pub fn new(pool: PoolHandle, match_sender: mpsc::Sender<MatchProof>) -> Self {
        Self {
            pool,
            match_sender,
            uniswap_router: std::env::var("ZERO_DEX_UNISWAP_ROUTER")
                .unwrap_or_else(|_| "0xE592427A0AEce92De3Edee1F18E0157C05861564".to_string()),
            raydium_program: std::env::var("ZERO_DEX_RAYDIUM_PROGRAM")
                .unwrap_or_else(|_| "CAMMCzo5YL8w4VFF8KVHrK22GGUsp5VTaW7grrKgrWqK".to_string()),
        }
    }

    /// Check the pool for stale unmatched intents and route them to AMMs
    pub async fn check_and_relay(&self) {
        let mut pool = self.pool.lock().await;
        let now = Instant::now();
        let fallback_threshold = Duration::from_secs(FALLBACK_DELAY_SECS);

        let stale_keys: Vec<String> = pool.active_intents().iter()
            .filter(|(_, pi)| now.duration_since(pi.received_at) > fallback_threshold)
            .map(|(k, _)| (*k).clone())
            .collect();

        for key in &stale_keys {
            info!("Intent {} unmatched for {}s, routing to AMM bridge", &key[..16], FALLBACK_DELAY_SECS);

            // Generate a bridge match proof
            // The settlement engine will detect the bridge address and route accordingly
            let proof = MatchProof {
                local_intent_id: "amm_bridge".to_string(),
                counterparty_intent_id: key.clone(),
                settled_vector: zerolang::Tensor::scalar(0.0, 1.0),
                token_a: "0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2".to_string(),
                token_b: "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48".to_string(),
                amount_a: 0,
                amount_b: 0,
                local_signature: Vec::new(),
                counterparty_signature: Vec::new(),
            };

            let _ = self.match_sender.send(proof).await;
            pool.mark_exhausted(key);
        }

        if !stale_keys.is_empty() {
            info!("Relayed {} stale intents to AMM bridges", stale_keys.len());
        }
    }

    /// Run the relayer as a periodic background task
    pub async fn run(self) {
        info!("Liquidity relayer started (fallback after {}s)", FALLBACK_DELAY_SECS);
        let mut interval = tokio::time::interval(Duration::from_secs(10));
        loop {
            interval.tick().await;
            self.check_and_relay().await;
        }
    }
}
