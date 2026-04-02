"""Experiment runner — orchestrates env, agent, and simulation components.

Reads a YAML config, builds the federation (with configurable Byzantine
fraction), trains the GATRouter via PPO, and logs metrics to TensorBoard
and CSV for paper figure generation.

Usage
-----
::

    python -m simulation.runner --config experiments/config/byzantine_20.yaml
"""

from __future__ import annotations

import csv
import os
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np
import yaml

import envs  # noqa: F401  — registers Gymnasium env
from simulation.brier_score import BrierScoreTracker


@dataclass
class RunConfig:
    num_makers: int = 10
    num_levels: int = 5
    decay_lambda: float = 0.1
    max_steps: int = 100
    base_liquidity: float = 100.0
    volatility: float = 5.0
    byzantine_fraction: float = 0.0
    attack_type: str = "spoofer"
    brier_alpha: float = 0.5
    gat_layers: int = 2
    gat_heads: int = 4
    hidden_dim: int = 64
    lr: float = 3e-4
    algorithm: str = "ppo"
    episodes: int = 2000
    seed: int = 42
    log_dir: str = "experiments/results"

    @classmethod
    def from_yaml(cls, path: str | Path) -> "RunConfig":
        with open(path) as f:
            raw = yaml.safe_load(f)
        flat: dict[str, Any] = {}
        for section in raw.values():
            if isinstance(section, dict):
                flat.update(section)
        return cls(**{k: v for k, v in flat.items() if k in cls.__dataclass_fields__})


def build_byzantine_mask(num_makers: int, fraction: float, rng: np.random.Generator) -> np.ndarray:
    n_byz = int(round(num_makers * fraction))
    mask = np.zeros(num_makers, dtype=bool)
    if n_byz > 0:
        indices = rng.choice(num_makers, size=n_byz, replace=False)
        mask[indices] = True
    return mask


def run(config: RunConfig) -> dict[str, Any]:
    """Execute a full training run. Returns summary metrics."""
    import gymnasium as gym
    import torch
    from torch.utils.tensorboard import SummaryWriter

    from agents.gnn_router import GATRouter
    from agents.graph_builder import GraphBuilder
    from agents.ppo_trainer import PPOConfig, PPOTrainer

    rng = np.random.default_rng(config.seed)
    torch.manual_seed(config.seed)

    byz_mask = build_byzantine_mask(config.num_makers, config.byzantine_fraction, rng)

    env = gym.make(
        "ZeroTVLDarkPool-v0",
        num_makers=config.num_makers,
        num_levels=config.num_levels,
        decay_lambda=config.decay_lambda,
        max_steps=config.max_steps,
        base_liquidity=config.base_liquidity,
        volatility=config.volatility,
        byzantine_mask=byz_mask.tolist(),
        brier_alpha=config.brier_alpha,
        seed=config.seed,
    )

    graph_builder = GraphBuilder(config.num_makers, config.num_levels)
    model = GATRouter(
        in_channels=graph_builder.node_feature_dim,
        hidden_channels=config.hidden_dim,
        num_heads=config.gat_heads,
        num_layers=config.gat_layers,
        num_makers=config.num_makers,
    )
    ppo_cfg = PPOConfig(lr=config.lr, algorithm=config.algorithm)
    trainer = PPOTrainer(model, graph_builder, ppo_cfg)

    brier_tracker = BrierScoreTracker(config.num_makers)

    log_path = Path(config.log_dir)
    log_path.mkdir(parents=True, exist_ok=True)
    writer = SummaryWriter(log_dir=str(log_path))

    csv_path = log_path / "metrics.csv"
    csv_file = open(csv_path, "w", newline="")
    csv_writer = csv.writer(csv_file)
    csv_writer.writerow(["episode", "reward", "fill_rate", "brier_penalty"])

    episode_rewards: list[float] = []

    for ep in range(config.episodes):
        obs, info = env.reset(seed=int(rng.integers(0, 2**31)))
        graph_builder.reset()
        brier_tracker.reset()
        ep_reward = 0.0
        last_fill_rate = 0.0

        for _ in range(config.max_steps):
            action, log_prob, value = trainer.select_action(obs)
            next_obs, reward, terminated, truncated, info = env.step(action)

            trainer.store_transition(obs, action, log_prob, value, reward, terminated or truncated)

            if "brier_scores" in info:
                graph_builder.update_stats(brier_scores=info["brier_scores"])
            if "fill_rate" in info:
                last_fill_rate = info["fill_rate"]

            ep_reward += reward
            obs = next_obs
            if terminated or truncated:
                break

        losses = trainer.update(obs)
        episode_rewards.append(ep_reward)

        writer.add_scalar("reward/episode", ep_reward, ep)
        writer.add_scalar("fill_rate", last_fill_rate, ep)
        writer.add_scalar("loss/total", losses["loss"], ep)
        writer.add_scalar("loss/policy", losses["policy_loss"], ep)
        writer.add_scalar("loss/value", losses["value_loss"], ep)

        if "weights" in info:
            for i in range(config.num_makers):
                tag = f"weight/maker_{i}_{'byz' if byz_mask[i] else 'honest'}"
                writer.add_scalar(tag, float(info["weights"][i]), ep)

        csv_writer.writerow([ep, ep_reward, last_fill_rate, losses["loss"]])

        if (ep + 1) % 100 == 0:
            avg = np.mean(episode_rewards[-100:])
            print(f"Episode {ep+1}/{config.episodes}  avg_reward(100)={avg:.4f}")

    csv_file.close()
    writer.close()
    env.close()

    return {
        "final_avg_reward": float(np.mean(episode_rewards[-100:])),
        "total_episodes": config.episodes,
        "log_dir": str(log_path),
    }
