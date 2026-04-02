"""GNN-based RL agents for dark-pool routing.

Reviewer entry points
---------------------
* :mod:`research.agents.gnn_a3c`     — combined GNN + A3C (Section 4.2)
* :mod:`research.agents.gnn_router`  — GATv2Conv backbone
* :mod:`research.agents.a3c_trainer` — async A3C trainer
* :mod:`research.agents.ppo_trainer` — PPO ablation baseline
"""

from research.agents.gnn_router import GATRouter
from research.agents.graph_builder import GraphBuilder

__all__ = ["GATRouter", "GraphBuilder"]
