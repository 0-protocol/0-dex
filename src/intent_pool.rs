//! Intent pool for Solver nodes.
//!
//! Collects and manages signed intents with TTL, deduplication,
//! and status tracking to prevent double-matching.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Mutex;
use zerolang::RuntimeGraph;
use sha3::{Digest, Keccak256};

use crate::crypto::SignedIntent;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IntentStatus {
    Active,
    Matched,
    Expired,
    Exhausted,
}

pub struct PooledIntent {
    pub graph: RuntimeGraph,
    pub signed_intent: SignedIntent,
    pub received_at: Instant,
    pub status: IntentStatus,
}

pub struct IntentPool {
    intents: HashMap<String, PooledIntent>,
    expiry_ms: u64,
}

impl IntentPool {
    pub fn new(expiry_ms: u64) -> Self {
        Self {
            intents: HashMap::new(),
            expiry_ms,
        }
    }

    /// Add an intent to the pool. Returns the intent hash key.
    pub fn add_intent(&mut self, signed_intent: SignedIntent, graph: RuntimeGraph) -> String {
        let key = Self::intent_hash(&signed_intent);
        if self.intents.contains_key(&key) {
            tracing::debug!("Duplicate intent {}, skipping", &key[..16]);
            return key;
        }

        tracing::info!("Pooled intent {} from {}", &key[..16], signed_intent.owner_address);
        self.intents.insert(key.clone(), PooledIntent {
            graph,
            signed_intent,
            received_at: Instant::now(),
            status: IntentStatus::Active,
        });
        key
    }

    /// Get all active (non-expired, non-matched) intents
    pub fn active_intents(&self) -> Vec<(&String, &PooledIntent)> {
        let now = Instant::now();
        self.intents.iter()
            .filter(|(_, pi)| {
                pi.status == IntentStatus::Active
                    && now.duration_since(pi.received_at).as_millis() < self.expiry_ms as u128
            })
            .collect()
    }

    /// Mark a set of intents as matched (prevents re-matching)
    pub fn mark_matched(&mut self, keys: &[String]) {
        for key in keys {
            if let Some(pi) = self.intents.get_mut(key) {
                pi.status = IntentStatus::Matched;
            }
        }
    }

    /// Mark an intent as exhausted (permanently used)
    pub fn mark_exhausted(&mut self, key: &str) {
        if let Some(pi) = self.intents.get_mut(key) {
            pi.status = IntentStatus::Exhausted;
        }
    }

    /// Remove expired intents
    pub fn prune_expired(&mut self) -> usize {
        let now = Instant::now();
        let expiry = self.expiry_ms;
        let before = self.intents.len();
        self.intents.retain(|_, pi| {
            if pi.status == IntentStatus::Active
                && now.duration_since(pi.received_at).as_millis() >= expiry as u128
            {
                false
            } else {
                // Also remove already-matched/exhausted intents older than 2x TTL
                !(pi.status != IntentStatus::Active
                    && now.duration_since(pi.received_at).as_millis() >= (expiry * 2) as u128)
            }
        });
        before - self.intents.len()
    }

    pub fn len(&self) -> usize {
        self.intents.len()
    }

    fn intent_hash(intent: &SignedIntent) -> String {
        let mut hasher = Keccak256::new();
        hasher.update(intent.graph_content.as_bytes());
        hasher.update(intent.owner_address.as_bytes());
        hasher.update(intent.signature_hex.as_bytes());
        hex::encode(hasher.finalize())
    }
}

/// Thread-safe handle to the intent pool
pub type PoolHandle = Arc<Mutex<IntentPool>>;

pub fn new_pool_handle(expiry_ms: u64) -> PoolHandle {
    Arc::new(Mutex::new(IntentPool::new(expiry_ms)))
}
