//! Node operational mode: Agent or Solver.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeMode {
    /// Regular agent: broadcasts intents, listens for solutions
    Agent,
    /// Solver: aggregates intents, computes multi-way matches, submits bundles
    Solver,
}

impl std::fmt::Display for NodeMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeMode::Agent => write!(f, "agent"),
            NodeMode::Solver => write!(f, "solver"),
        }
    }
}

impl std::str::FromStr for NodeMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "agent" => Ok(NodeMode::Agent),
            "solver" => Ok(NodeMode::Solver),
            _ => Err(format!("Invalid node mode '{}'. Expected 'agent' or 'solver'.", s)),
        }
    }
}
