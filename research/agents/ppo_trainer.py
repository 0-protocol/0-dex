"""PPO trainer for the GATRouter agent.

Implements single-process Proximal Policy Optimisation (Schulman et al.,
2017) with Generalised Advantage Estimation (GAE).  This serves as the
primary training algorithm for FLARE ablation experiments.

For the asynchronous multi-worker variant described in Section 4.2 of
the FLARE paper, see :mod:`research.agents.a3c_trainer`.
"""

from __future__ import annotations

import copy
from dataclasses import dataclass, field
from typing import Any

import numpy as np
import torch
import torch.nn as nn
import torch.optim as optim
from torch import Tensor

from research.agents.gnn_router import GATRouter
from research.agents.graph_builder import GraphBuilder


@dataclass
class PPOConfig:
    lr: float = 3e-4
    gamma: float = 0.99
    gae_lambda: float = 0.95
    clip_eps: float = 0.2
    entropy_coef: float = 0.01
    value_coef: float = 0.5
    max_grad_norm: float = 0.5
    ppo_epochs: int = 4
    mini_batch_size: int = 32
    algorithm: str = "ppo"


@dataclass
class Transition:
    obs: np.ndarray
    action: np.ndarray
    log_prob: float
    value: float
    reward: float
    done: bool


class RolloutBuffer:
    """Stores transitions for one rollout and computes GAE returns."""

    def __init__(self) -> None:
        self.transitions: list[Transition] = []

    def append(self, t: Transition) -> None:
        self.transitions.append(t)

    def clear(self) -> None:
        self.transitions.clear()

    def compute_returns(
        self,
        last_value: float,
        gamma: float,
        gae_lambda: float,
    ) -> tuple[list[float], list[float]]:
        """Compute GAE advantages and discounted returns."""
        advantages: list[float] = []
        returns: list[float] = []
        gae = 0.0
        next_value = last_value

        for t in reversed(self.transitions):
            mask = 0.0 if t.done else 1.0
            delta = t.reward + gamma * next_value * mask - t.value
            gae = delta + gamma * gae_lambda * mask * gae
            advantages.insert(0, gae)
            returns.insert(0, gae + t.value)
            next_value = t.value

        return advantages, returns

    def __len__(self) -> int:
        return len(self.transitions)


class PPOTrainer:
    """Trains a :class:`GATRouter` with PPO or A3C.

    Parameters
    ----------
    model : GATRouter
    graph_builder : GraphBuilder
    config : PPOConfig
    device : str
    """

    def __init__(
        self,
        model: GATRouter,
        graph_builder: GraphBuilder,
        config: PPOConfig | None = None,
        device: str = "cpu",
    ) -> None:
        self.model = model.to(device)
        self.graph_builder = graph_builder
        self.cfg = config or PPOConfig()
        self.device = torch.device(device)
        self.optimizer = optim.Adam(self.model.parameters(), lr=self.cfg.lr)
        self.buffer = RolloutBuffer()

    def select_action(
        self, obs: np.ndarray, deterministic: bool = False
    ) -> tuple[np.ndarray, float, float]:
        """Pick an action given observation; returns (action, log_prob, value)."""
        data = self.graph_builder.build(obs).to(self.device)
        with torch.no_grad():
            action, log_prob, value = self.model.act(
                data.x, data.edge_index, deterministic=deterministic
            )
        return (
            action.cpu().numpy(),
            float(log_prob.cpu()),
            float(value.cpu()),
        )

    def store_transition(
        self,
        obs: np.ndarray,
        action: np.ndarray,
        log_prob: float,
        value: float,
        reward: float,
        done: bool,
    ) -> None:
        self.buffer.append(
            Transition(obs=obs, action=action, log_prob=log_prob,
                       value=value, reward=reward, done=done)
        )

    def update(self, last_obs: np.ndarray) -> dict[str, float]:
        """Run PPO (or A3C) update on collected rollout. Returns loss dict."""
        data = self.graph_builder.build(last_obs).to(self.device)
        with torch.no_grad():
            _, _, last_value = self.model.act(data.x, data.edge_index, deterministic=True)

        advantages, returns = self.buffer.compute_returns(
            float(last_value.cpu()), self.cfg.gamma, self.cfg.gae_lambda
        )

        adv_t = torch.tensor(advantages, dtype=torch.float32, device=self.device)
        ret_t = torch.tensor(returns, dtype=torch.float32, device=self.device)
        adv_t = (adv_t - adv_t.mean()) / (adv_t.std() + 1e-8)

        old_log_probs = torch.tensor(
            [t.log_prob for t in self.buffer.transitions],
            dtype=torch.float32,
            device=self.device,
        )

        total_loss = 0.0
        policy_loss_sum = 0.0
        value_loss_sum = 0.0

        n_epochs = self.cfg.ppo_epochs if self.cfg.algorithm == "ppo" else 1

        for _ in range(n_epochs):
            for idx in range(len(self.buffer)):
                t = self.buffer.transitions[idx]
                graph = self.graph_builder.build(t.obs).to(self.device)
                action_t = torch.from_numpy(t.action).to(self.device)

                logits, value = self.model(graph.x, graph.edge_index)
                probs = torch.softmax(logits, dim=0)

                concentration = probs * self.model.num_makers + 1e-2
                dist = torch.distributions.Dirichlet(concentration)
                log_prob = dist.log_prob(action_t)
                entropy = dist.entropy()

                if self.cfg.algorithm == "ppo":
                    ratio = torch.exp(log_prob - old_log_probs[idx])
                    surr1 = ratio * adv_t[idx]
                    surr2 = torch.clamp(ratio, 1 - self.cfg.clip_eps, 1 + self.cfg.clip_eps) * adv_t[idx]
                    policy_loss = -torch.min(surr1, surr2)
                else:
                    policy_loss = -log_prob * adv_t[idx]

                value_loss = nn.functional.mse_loss(value.squeeze().mean(), ret_t[idx])
                loss = (
                    policy_loss
                    + self.cfg.value_coef * value_loss
                    - self.cfg.entropy_coef * entropy
                )

                self.optimizer.zero_grad()
                loss.backward()
                nn.utils.clip_grad_norm_(self.model.parameters(), self.cfg.max_grad_norm)
                self.optimizer.step()

                total_loss += float(loss)
                policy_loss_sum += float(policy_loss)
                value_loss_sum += float(value_loss)

        n_updates = n_epochs * len(self.buffer)
        self.buffer.clear()

        return {
            "loss": total_loss / max(n_updates, 1),
            "policy_loss": policy_loss_sum / max(n_updates, 1),
            "value_loss": value_loss_sum / max(n_updates, 1),
        }
