"""GNN-based Asynchronous Actor-Critic for dark-pool routing (Section 4.2).

Reviewer-facing entry point that composes the GATv2Conv routing backbone
(:class:`GATRouter`) with the asynchronous A3C trainer into a single
readable module.

Architecture (Eq. 3)
--------------------
The central router maintains an implicit liquidity graph G.  Graph
Attention Networks extract per-maker features, which are fed into an
Asynchronous Advantage Actor-Critic (A3C) network::

    nabla_theta J(theta) ~ E_{pi_theta}[
        nabla_theta log pi_theta(a_t | O_t)
        * (R_t + gamma * V_phi(O_{t+1}) - V_phi(O_t))
    ]

* **Backbone**: 2-layer GATv2Conv with multi-head attention (4 heads).
  Learns which makers to trust from reported liquidity, historical fill
  rates, and Brier scores.
* **Actor**: Outputs routing-weight logits -> softmax -> Dirichlet
  concentration.  The sampled action lives on the probability simplex
  (continuous routing split).
* **Critic**: Separate value head sharing the GAT backbone; advantage
  estimated via n-step returns in each async worker.
* **Async training**: Multiple workers each run their own env, compute
  local gradients, and apply Hogwild-style updates to a shared model
  via ``torch.multiprocessing``.

Quick start::

    from research.agents.gnn_a3c import GNNAsyncRouter, train_gnn_a3c

    router = GNNAsyncRouter(num_makers=10, num_levels=5)
    results = train_gnn_a3c(router, total_episodes=500, num_workers=4)

References
----------
Wang et al., "Zero-Knowledge Proof-Based Gradient Aggregation for
Federated Learning", IEEE Transactions on Big Data, 2024.

Piao et al., "GARNN: an interpretable graph attentive recurrent neural
network ...", Neural Networks 185, 107229, 2025.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Any, Callable

from research.agents.gnn_router import GATRouter
from research.agents.graph_builder import GraphBuilder
from research.agents.a3c_trainer import A3CConfig, A3CTrainer


@dataclass
class GNNAsyncRouter:
    """Convenience wrapper: GATRouter model + GraphBuilder + A3C config.

    Parameters
    ----------
    num_makers : int
        N — number of federated maker nodes.
    num_levels : int
        K — price levels per maker.
    hidden_channels : int
        GATv2Conv hidden dimension.
    num_heads : int
        Number of attention heads per GAT layer.
    num_layers : int
        Number of GATv2Conv layers.
    lr : float
        Adam learning rate for shared model.
    gamma : float
        Discount factor.
    n_steps : int
        Steps per A3C rollout before gradient push.
    """

    num_makers: int = 10
    num_levels: int = 5
    hidden_channels: int = 64
    num_heads: int = 4
    num_layers: int = 2
    lr: float = 3e-4
    gamma: float = 0.99
    n_steps: int = 20

    def build_model(self) -> GATRouter:
        gb = self.build_graph_builder()
        return GATRouter(
            in_channels=gb.node_feature_dim,
            hidden_channels=self.hidden_channels,
            num_heads=self.num_heads,
            num_layers=self.num_layers,
            num_makers=self.num_makers,
        )

    def build_graph_builder(self) -> GraphBuilder:
        return GraphBuilder(self.num_makers, self.num_levels)

    def build_a3c_config(self, num_workers: int = 4) -> A3CConfig:
        return A3CConfig(
            num_workers=num_workers,
            lr=self.lr,
            gamma=self.gamma,
            n_steps=self.n_steps,
        )


def train_gnn_a3c(
    router: GNNAsyncRouter,
    env_kwargs: dict[str, Any] | None = None,
    total_episodes: int = 2000,
    num_workers: int = 4,
) -> list[dict[str, Any]]:
    """Train the GNN-A3C routing policy.

    Parameters
    ----------
    router : GNNAsyncRouter
        Model and hyperparameter configuration.
    env_kwargs : dict
        Extra kwargs passed to ``gym.make("ZeroTVLDarkPool-v0", ...)``.
    total_episodes : int
        Total training episodes across all workers.
    num_workers : int
        Number of parallel A3C worker processes.

    Returns
    -------
    list[dict]
        Per-episode results from all workers, sorted by episode number.
    """
    import gymnasium as gym
    import research.envs  # noqa: F401 — registers ZeroTVLDarkPool-v0

    _env_kwargs = dict(
        num_makers=router.num_makers,
        num_levels=router.num_levels,
    )
    if env_kwargs:
        _env_kwargs.update(env_kwargs)

    def env_fn():
        return gym.make("ZeroTVLDarkPool-v0", **_env_kwargs)

    trainer = A3CTrainer(
        model_fn=router.build_model,
        env_fn=env_fn,
        graph_builder_fn=router.build_graph_builder,
        config=router.build_a3c_config(num_workers),
    )

    return trainer.train(total_episodes=total_episodes)


__all__ = ["GNNAsyncRouter", "train_gnn_a3c"]
