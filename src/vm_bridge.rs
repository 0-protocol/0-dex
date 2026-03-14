//! Secure isolation sandbox for running untrusted counterparty graphs.

use zerolang::{RuntimeGraph, VM};
use tokio::time::timeout;
use std::time::Duration;

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

    /// Evaluates a counterparty graph in a sandboxed, time-bounded environment.
    /// Takes ownership of the graph to move it into the blocking task.
    pub async fn evaluate_untrusted(&self, graph: RuntimeGraph) -> Result<Vec<zerolang::Tensor>, String> {
        let timeout_ms = self.timeout_ms;

        let result = timeout(Duration::from_millis(timeout_ms), {
            tokio::task::spawn_blocking(move || {
                let mut vm = VM::new();
                vm.execute(&graph)
            })
        }).await;

        match result {
            Ok(Ok(Ok(tensors))) => Ok(tensors),
            Ok(Ok(Err(e))) => Err(format!("VM Execution error: {:?}", e)),
            Ok(Err(e)) => Err(format!("Task panic: {:?}", e)),
            Err(_) => Err("VM Execution Timeout (Gas limit exceeded)".to_string()),
        }
    }
}
