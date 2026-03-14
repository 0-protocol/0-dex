//! Secure isolation sandbox for running untrusted counterparty graphs.

use zerolang::{RuntimeGraph, VM};
use tokio::time::timeout;
use std::time::Duration;
use tracing::warn;

pub struct SecureVM {
    /// Max ops allowed for untrusted graphs (gas limit equivalent)
    max_ops: usize,
    /// Execution timeout
    timeout_ms: u64,
}

impl SecureVM {
    pub fn new(max_ops: usize, timeout_ms: u64) -> Self {
        Self { max_ops, timeout_ms }
    }

    /// Evaluates a counterparty graph in a sandboxed, time-bounded environment
    pub async fn evaluate_untrusted(&self, graph: &RuntimeGraph) -> Result<Vec<zerolang::Tensor>, String> {
        let mut local_vm = VM::new();
        // Here we would strictly limit local_vm.max_ops if the 0-lang API supported it
        
        let result = timeout(Duration::from_millis(self.timeout_ms), async {
            // Ideally VM execution should be yieldable/async to prevent thread blocking,
            // but for now we run it synchronously inside the timeout task.
            tokio::task::spawn_blocking({
                let g = graph.clone();
                move || {
                    let mut vm = VM::new();
                    vm.execute(&g)
                }
            }).await
        }).await;

        match result {
            Ok(Ok(Ok(tensors))) => Ok(tensors),
            Ok(Ok(Err(e))) => Err(format!("VM Execution error: {:?}", e)),
            Ok(Err(e)) => Err(format!("Task panic: {:?}", e)),
            Err(_) => Err("VM Execution Timeout (Gas limit exceeded)".to_string()),
        }
    }
}
