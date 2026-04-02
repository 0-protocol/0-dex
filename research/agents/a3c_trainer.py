"""Asynchronous Advantage Actor-Critic (A3C) trainer for GATRouter.

Implements the asynchronous policy gradient algorithm from Section 4.2
of the FLARE paper (Eq. 3):

    nabla_theta J(theta) ~ E_{pi_theta}[
        nabla_theta log pi_theta(a_t | O_t) *
        (R_t + gamma * V_phi(O_{t+1}) - V_phi(O_t))
    ]

Each worker process runs its own environment instance, computes local
n-step returns and gradients, and applies them to a shared global model
via Hogwild-style updates (no explicit parameter server lock).  This
mirrors the federated asynchronous gradient aggregation in:

  Wang et al., "Zero-Knowledge Proof-Based Gradient Aggregation for
  Federated Learning", IEEE Transactions on Big Data, 2024.

Usage
-----
::

    from research.agents.a3c_trainer import A3CTrainer, A3CConfig
    trainer = A3CTrainer(model_fn, env_fn, A3CConfig(num_workers=4))
    trainer.train(total_episodes=2000)
"""

from __future__ import annotations

import os
from dataclasses import dataclass
from typing import Any, Callable

import numpy as np
import torch
import torch.multiprocessing as mp
import torch.nn as nn
import torch.nn.functional as F
import torch.optim as optim

from research.agents.gnn_router import GATRouter
from research.agents.graph_builder import GraphBuilder


@dataclass
class A3CConfig:
    num_workers: int = 4
    lr: float = 3e-4
    gamma: float = 0.99
    n_steps: int = 20
    entropy_coef: float = 0.01
    value_coef: float = 0.5
    max_grad_norm: float = 0.5
    max_episodes_per_worker: int = 500


def ensure_shared_grads(local_model: nn.Module, shared_model: nn.Module) -> None:
    """Copy local gradients into the shared model's grad buffers."""
    for local_param, shared_param in zip(
        local_model.parameters(), shared_model.parameters()
    ):
        if shared_param.grad is not None:
            return
        shared_param._grad = local_param.grad


class SharedAdam(optim.Adam):
    """Adam optimizer with shared state tensors for multiprocessing."""

    def __init__(self, params, **kwargs):
        super().__init__(params, **kwargs)
        for group in self.param_groups:
            for p in group["params"]:
                state = self.state[p]
                state["step"] = torch.zeros(1)
                state["exp_avg"] = torch.zeros_like(p.data)
                state["exp_avg_sq"] = torch.zeros_like(p.data)
                state["step"].share_memory_()
                state["exp_avg"].share_memory_()
                state["exp_avg_sq"].share_memory_()


def _worker(
    rank: int,
    shared_model: GATRouter,
    optimizer: SharedAdam,
    env_fn: Callable,
    graph_builder_fn: Callable,
    config: A3CConfig,
    global_episode_counter: mp.Value,
    result_queue: mp.Queue,
) -> None:
    """A3C worker process — runs env, computes n-step returns, updates shared model."""
    torch.manual_seed(config.gamma * 1000 + rank)

    env = env_fn()
    graph_builder = graph_builder_fn()

    local_model = GATRouter(
        in_channels=shared_model.convs[0].in_channels,
        hidden_channels=shared_model.critic_head[0].in_features
        // len(shared_model.convs[0].heads if hasattr(shared_model.convs[0], 'heads') else [1]),
        num_heads=shared_model.convs[0].heads if isinstance(shared_model.convs[0].heads, int) else 4,
        num_layers=len(shared_model.convs),
        num_makers=shared_model.num_makers,
    )
    local_model.load_state_dict(shared_model.state_dict())

    while True:
        with global_episode_counter.get_lock():
            if global_episode_counter.value >= config.max_episodes_per_worker * config.num_workers:
                break
            global_episode_counter.value += 1
            ep_num = global_episode_counter.value

        local_model.load_state_dict(shared_model.state_dict())
        graph_builder.reset()

        obs, info = env.reset()
        done = False
        ep_reward = 0.0
        step = 0

        while not done:
            values, log_probs, rewards, entropies = [], [], [], []

            for _ in range(config.n_steps):
                data = graph_builder.build(obs)
                logits, value = local_model(data.x, data.edge_index)
                probs = F.softmax(logits, dim=0)

                concentration = probs * local_model.num_makers + 1e-2
                dist = torch.distributions.Dirichlet(concentration)
                action = dist.sample()
                log_prob = dist.log_prob(action)
                entropy = dist.entropy()

                next_obs, reward, terminated, truncated, info = env.step(
                    action.detach().numpy()
                )
                done = terminated or truncated

                if "brier_scores" in info:
                    graph_builder.update_stats(brier_scores=info["brier_scores"])

                values.append(value.squeeze())
                log_probs.append(log_prob)
                rewards.append(reward)
                entropies.append(entropy)

                ep_reward += reward
                obs = next_obs
                step += 1
                if done:
                    break

            # n-step return bootstrap
            R = torch.zeros(1)
            if not done:
                data = graph_builder.build(obs)
                _, R = local_model(data.x, data.edge_index)
                R = R.squeeze().detach()

            policy_loss = torch.tensor(0.0)
            value_loss = torch.tensor(0.0)
            ent_sum = torch.tensor(0.0)

            for i in reversed(range(len(rewards))):
                R = config.gamma * R + rewards[i]
                advantage = R - values[i]
                value_loss = value_loss + 0.5 * advantage.pow(2)
                policy_loss = policy_loss - log_probs[i] * advantage.detach()
                ent_sum = ent_sum + entropies[i]

            loss = (
                policy_loss
                + config.value_coef * value_loss
                - config.entropy_coef * ent_sum
            )

            optimizer.zero_grad()
            loss.backward()
            nn.utils.clip_grad_norm_(local_model.parameters(), config.max_grad_norm)
            ensure_shared_grads(local_model, shared_model)
            optimizer.step()

        result_queue.put({
            "rank": rank,
            "episode": ep_num,
            "reward": ep_reward,
            "steps": step,
            "fill_rate": info.get("fill_rate", 0.0),
        })

    env.close()


class A3CTrainer:
    """Orchestrates parallel A3C workers with a shared GATRouter model.

    Parameters
    ----------
    model_fn : callable
        Returns a fresh GATRouter instance.
    env_fn : callable
        Returns a fresh Gymnasium env instance.
    graph_builder_fn : callable
        Returns a fresh GraphBuilder instance.
    config : A3CConfig
    """

    def __init__(
        self,
        model_fn: Callable[[], GATRouter],
        env_fn: Callable,
        graph_builder_fn: Callable[[], GraphBuilder],
        config: A3CConfig | None = None,
    ) -> None:
        self.config = config or A3CConfig()
        self.shared_model = model_fn()
        self.shared_model.share_memory()
        self.optimizer = SharedAdam(self.shared_model.parameters(), lr=self.config.lr)
        self.env_fn = env_fn
        self.graph_builder_fn = graph_builder_fn

    def train(self, total_episodes: int | None = None) -> list[dict[str, Any]]:
        """Launch workers and collect results.

        Returns list of per-episode result dicts.
        """
        if total_episodes is not None:
            self.config.max_episodes_per_worker = (
                total_episodes // self.config.num_workers + 1
            )

        mp.set_start_method("spawn", force=True)
        global_ep = mp.Value("i", 0)
        result_queue = mp.Queue()

        workers = []
        for rank in range(self.config.num_workers):
            p = mp.Process(
                target=_worker,
                args=(
                    rank,
                    self.shared_model,
                    self.optimizer,
                    self.env_fn,
                    self.graph_builder_fn,
                    self.config,
                    global_ep,
                    result_queue,
                ),
            )
            p.start()
            workers.append(p)

        for p in workers:
            p.join()

        results = []
        while not result_queue.empty():
            results.append(result_queue.get())

        return sorted(results, key=lambda r: r["episode"])
