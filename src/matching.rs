//! Graph intersection and matching engine
//!
//! Evaluates incoming graphs against local intents using the 0-lang VM.

use zerolang::{RuntimeGraph, VM};

pub struct MatchingEngine {
    vm: VM,
    local_intents: Vec<RuntimeGraph>,
}

impl MatchingEngine {
    pub fn new() -> Self {
        Self {
            vm: VM::new(),
            local_intents: Vec::new(),
        }
    }

    pub fn register_intent(&mut self, graph: RuntimeGraph) {
        self.local_intents.push(graph);
    }

    pub fn evaluate_counterparty(&mut self, counterparty_graph: &RuntimeGraph) -> bool {
        // TODO: Combine graphs and find valid intersections where output tensors satisfy both parties
        false
    }
}
