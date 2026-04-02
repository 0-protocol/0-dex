"""Experiment runner -- orchestrates env, agent, and simulation components.

Reads a YAML config, builds the federation (with configurable Byzantine
fraction and attack type), trains the GATRouter via PPO, and logs all
paper metrics (Section 5) to TensorBoard and CSV.

Usage
-----
::

    python -m research.simulation.runner --config research/experiments/config/byzantine_20.yaml
"""

from __future__ import annotations

import csv
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import numpy as np
import yaml

import research.envs  # noqa: F401  -- registers Gymnasium env
from research.simulation.brier_score import BrierScoreTracker
from research.simulation.metrics import (
    byzantine_resilience,
    capital_efficiency,
    effective_slippage,
)


@dataclass
class RunConfig:
    num_makers: int = 10
    num_levels: int = 5
    decay_lambda: float = 0.1
    max_steps: int = 100
    base_liquidity: float = 100.0
    volatility: float = 5.0
    order_size: float = 50.0
    byzantine_fraction: float = 0.0
    attack_type: str = "spoofer"
    alpha: float = 1.0
    eta: float = 0.5
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


def build_byzantine_mask(
    num_makers: int,
    fraction: float,
    attack_type: str,
    rng: np.random.Generator,
) -> tuple[np.ndarray, list[str]]:
    """Build mask and per-node attack type list.

    Returns (mask, attack_types) where attack_types[i] is the attack
    strategy for Byzantine node i, or 'honest' for honest nodes.
    """
    n_byz = int(round(num_makers * fraction))
    mask = np.zeros(num_makers, dtype=bool)
    attack_types = ["honest"] * num_makers
    if n_byz > 0:
        indices = rng.choice(num_makers, size=n_byz, replace=False)
        mask[indices] = True
        for idx in indices:
            attack_types[idx] = attack_type
    return mask, attack_types


def run(config: RunConfig) -> dict[str, Any]:
    """Execute a full training run. Returns summary metrics."""
    import gymnasium as gym
    import torch
    from torch.utils.tensorboard import SummaryWriter

    from research.agents.gnn_router import GATRouter
    from research.agents.graph_builder import GraphBuilder
    from research.agents.ppo_trainer import PPOConfig, PPOTrainer

    rng = np.random.default_rng(config.seed)
    torch.manual_seed(config.seed)

    byz_mask, attack_types = build_byzantine_mask(
        config.num_makers, config.byzantine_fraction, config.attack_type, rng
    )

    env = gym.make(
        "ZeroTVLDarkPool-v0",
        num_makers=config.num_makers,
        num_levels=config.num_levels,
        decay_lambda=config.decay_lambda,
        max_steps=config.max_steps,
        order_size=config.order_size,
        base_liquidity=config.base_liquidity,
        volatility=config.volatility,
        byzantine_mask=byz_mask.tolist(),
        alpha=config.alpha,
        eta=config.eta,
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
    csv_writer.writerow([
        "episode", "reward", "fill_rate", "brier_penalty",
        "capital_efficiency", "effective_slippage", "byzantine_resilience",
    ])

    episode_rewards: list[float] = []
    weight_history: list[np.ndarray] = []

    for ep in range(config.episodes):
        obs, info = env.reset(seed=int(rng.integers(0, 2**31)))
        graph_builder.reset()
        brier_tracker.reset()
        ep_reward = 0.0
        last_fill_rate = 0.0
        ep_volume = 0.0
        ep_filled = 0.0
        last_weights = np.ones(config.num_makers, dtype=np.float32) / config.num_makers

        for _ in range(config.max_steps):
            action, log_prob, value = trainer.select_action(obs)
            next_obs, reward, terminated, truncated, step_info = env.step(action)

            trainer.store_transition(obs, action, log_prob, value, reward, terminated or truncated)

            if "fill_prob" in step_info and "actual_filled" in step_info:
                brier_tracker.update(step_info["fill_prob"], step_info["actual_filled"])
            if "brier_scores" in step_info:
                graph_builder.update_stats(brier_scores=step_info["brier_scores"])
            if "fill_rate" in step_info:
                last_fill_rate = step_info["fill_rate"]
            if "weights" in step_info:
                last_weights = step_info["weights"]
            if "total_filled" in step_info:
                ep_filled += step_info["total_filled"]

            ep_volume += config.order_size
            ep_reward += reward
            obs = next_obs
            if terminated or truncated:
                break

        losses = trainer.update(obs)
        episode_rewards.append(ep_reward)
        weight_history.append(last_weights.copy())

        # Paper metrics (Section 5)
        ep_cap_eff = capital_efficiency(ep_volume, tvl_locked=0.0)
        ep_eff_slip = effective_slippage(ep_volume, ep_filled)
        ep_byz_res = byzantine_resilience(last_weights, byz_mask)
        ep_brier = brier_tracker.penalty(last_weights)

        writer.add_scalar("reward/episode", ep_reward, ep)
        writer.add_scalar("fill_rate", last_fill_rate, ep)
        writer.add_scalar("loss/total", losses["loss"], ep)
        writer.add_scalar("loss/policy", losses["policy_loss"], ep)
        writer.add_scalar("loss/value", losses["value_loss"], ep)
        writer.add_scalar("metric/capital_efficiency", ep_cap_eff if np.isfinite(ep_cap_eff) else 0, ep)
        writer.add_scalar("metric/effective_slippage", ep_eff_slip, ep)
        writer.add_scalar("metric/byzantine_resilience", ep_byz_res, ep)
        writer.add_scalar("metric/brier_penalty", ep_brier, ep)

        for i in range(config.num_makers):
            label = attack_types[i]
            tag = f"weight/maker_{i}_{label}"
            writer.add_scalar(tag, float(last_weights[i]), ep)

        csv_writer.writerow([
            ep, ep_reward, last_fill_rate, ep_brier,
            ep_cap_eff if np.isfinite(ep_cap_eff) else "inf",
            ep_eff_slip, ep_byz_res,
        ])

        if (ep + 1) % 100 == 0:
            avg = np.mean(episode_rewards[-100:])
            print(
                f"Episode {ep+1}/{config.episodes}  "
                f"avg_reward(100)={avg:.4f}  "
                f"byz_resilience={ep_byz_res:.2f}  "
                f"eff_slippage={ep_eff_slip:.4f}"
            )

    csv_file.close()
    writer.close()
    env.close()

    return {
        "final_avg_reward": float(np.mean(episode_rewards[-100:])),
        "total_episodes": config.episodes,
        "log_dir": str(log_path),
        "final_byzantine_resilience": byzantine_resilience(last_weights, byz_mask),
        "weight_history": weight_history,
    }
