#![allow(dead_code)]

//! Secure isolation sandbox for running untrusted counterparty graphs.
//!
//! Defence layers:
//! 1. Serialized size budget.
//! 2. Execution timeout.
//! 3. Panic isolation.
//! 4. Output sanity limits.

use std::time::Duration;
use tokio::time::timeout;
use zerolang::{RuntimeGraph, VM};

const DEFAULT_MAX_SERIALIZED_BYTES: usize = 256 * 1024;
const DEFAULT_TIMEOUT_MS: u64 = 200;

pub struct SecureVM {
    max_serialized_bytes: usize,
    timeout_ms: u64,
}

impl SecureVM {
    pub fn new(max_serialized_bytes: usize, timeout_ms: u64) -> Self {
        Self {
            max_serialized_bytes,
            timeout_ms,
        }
    }

    pub fn default_limits() -> Self {
        Self::new(DEFAULT_MAX_SERIALIZED_BYTES, DEFAULT_TIMEOUT_MS)
    }

    pub async fn evaluate_untrusted(
        &self,
        graph: &RuntimeGraph,
    ) -> Result<Vec<zerolang::Tensor>, String> {
        let serialized = format!("{graph:?}");
        if serialized.len() > self.max_serialized_bytes {
            return Err(format!(
                "Graph serialized size {}B exceeds limit {}B",
                serialized.len(),
                self.max_serialized_bytes
            ));
        }

        let timeout_dur = Duration::from_millis(self.timeout_ms);
        let result = timeout(timeout_dur, async {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut vm = VM::new();
                vm.execute(graph)
            }))
        })
        .await;

        match result {
            Ok(Ok(Ok(tensors))) => {
                if tensors.len() > 1024 {
                    return Err(format!(
                        "VM produced {} output tensors — exceeds sanity limit",
                        tensors.len()
                    ));
                }
                Ok(tensors)
            }
            Ok(Ok(Err(e))) => Err(format!("VM execution error: {e:?}")),
            Ok(Err(_)) => Err("VM thread panicked".to_string()),
            Err(_) => Err(format!(
                "VM execution timed out after {}ms",
                self.timeout_ms
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_limits_are_sane() {
        let vm = SecureVM::default_limits();
        assert_eq!(vm.max_serialized_bytes, 256 * 1024);
        assert_eq!(vm.timeout_ms, 200);
    }
}
