//! Integration seams for upstream 0-lang dependencies.
//!
//! These traits make it explicit which features are blocked on 0-lang PRs:
//! - Oracle verification (`Op::OracleRead`, `Op::VerifyPythPrice`)
//! - State locking / nonce exhaustion opcodes

use crate::model::NormalizedIntent;

pub trait OracleVerifier: Send + Sync {
    fn verify_oracle_constraints(&self, intent: &NormalizedIntent) -> Result<bool, String>;
}

pub trait IntentStateLocker: Send + Sync {
    fn can_match(&self, intent_id: &str, nonce: u64) -> Result<bool, String>;
    fn mark_exhausted(&self, intent_id: &str, nonce: u64) -> Result<(), String>;
}

pub struct NoopOracleVerifier;
impl OracleVerifier for NoopOracleVerifier {
    fn verify_oracle_constraints(&self, _intent: &NormalizedIntent) -> Result<bool, String> {
        Ok(true)
    }
}

pub struct InMemoryStateLocker {
    exhausted: std::sync::Mutex<std::collections::HashSet<(String, u64)>>,
}

impl InMemoryStateLocker {
    pub fn new() -> Self {
        Self {
            exhausted: std::sync::Mutex::new(std::collections::HashSet::new()),
        }
    }
}

impl IntentStateLocker for InMemoryStateLocker {
    fn can_match(&self, intent_id: &str, nonce: u64) -> Result<bool, String> {
        let g = self.exhausted.lock().map_err(|_| "lock poisoned".to_string())?;
        Ok(!g.contains(&(intent_id.to_string(), nonce)))
    }

    fn mark_exhausted(&self, intent_id: &str, nonce: u64) -> Result<(), String> {
        let mut g = self.exhausted.lock().map_err(|_| "lock poisoned".to_string())?;
        g.insert((intent_id.to_string(), nonce));
        Ok(())
    }
}
