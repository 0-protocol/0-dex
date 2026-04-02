#!/usr/bin/env python3
"""Byzantine spoofing resilience experiment — standalone reproducible script.

Injects a configurable fraction (default 30%) of "ghost" maker nodes that
report extremely high liquidity but **refuse 100% of fills**.  Trains a
GATv2Conv routing agent with PPO and plots how the Brier Score penalty
(Eq. 2) drives the routing weights of malicious nodes to zero.

This script is designed to be the first thing an ICML/NeurIPS reviewer
runs after cloning the repository:

    python -m research.simulation.byzantine_spoofing

It produces:
  - A publication-quality matplotlib figure (PDF + PNG)
  - A terminal summary table with convergence statistics

The figure shows routing weight trajectories: honest nodes (blue) retain
weight while Byzantine nodes (red) are isolated within a few hundred
episodes — the core experimental claim of the FLARE paper.

References
----------
Dong et al., "Defending against poisoning attacks in federated learning
with blockchain", IEEE Transactions on Artificial Intelligence, 2024.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

import numpy as np
import torch

import research.envs  # noqa: F401 — registers ZeroTVLDarkPool-v0


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    p = argparse.ArgumentParser(
        description="Byzantine spoofing resilience experiment (FLARE paper)."
    )
    p.add_argument("--episodes", type=int, default=300,
                   help="Training episodes (default: 300)")
    p.add_argument("--num-makers", type=int, default=10,
                   help="Total maker nodes N (default: 10)")
    p.add_argument("--byzantine-fraction", type=float, default=0.3,
                   help="Fraction of Byzantine ghost nodes (default: 0.3)")
    p.add_argument("--max-steps", type=int, default=50,
                   help="Steps per episode (default: 50)")
    p.add_argument("--seed", type=int, default=42,
                   help="Random seed for reproducibility")
    p.add_argument("--alpha", type=float, default=1.0,
                   help="Gain coefficient alpha in Eq. 2")
    p.add_argument("--eta", type=float, default=0.5,
                   help="Brier penalty coefficient eta in Eq. 2")
    p.add_argument("--output-dir", type=str,
                   default="research/simulation/figures",
                   help="Directory for output figures")
    p.add_argument("--no-plot", action="store_true",
                   help="Skip figure generation (CI mode)")
    return p.parse_args(argv)


def build_byzantine_mask(
    num_makers: int, fraction: float, rng: np.random.Generator
) -> np.ndarray:
    n_byz = int(round(num_makers * fraction))
    mask = np.zeros(num_makers, dtype=bool)
    if n_byz > 0:
        indices = rng.choice(num_makers, size=n_byz, replace=False)
        mask[indices] = True
    return mask


def run_experiment(args: argparse.Namespace) -> dict:
    """Train and collect per-episode weight trajectories."""
    import gymnasium as gym
    from research.agents.gnn_router import GATRouter
    from research.agents.graph_builder import GraphBuilder
    from research.agents.ppo_trainer import PPOConfig, PPOTrainer

    rng = np.random.default_rng(args.seed)
    torch.manual_seed(args.seed)

    byz_mask = build_byzantine_mask(args.num_makers, args.byzantine_fraction, rng)

    env = gym.make(
        "ZeroTVLDarkPool-v0",
        num_makers=args.num_makers,
        num_levels=5,
        max_steps=args.max_steps,
        order_size=50.0,
        decay_lambda=0.1,
        base_liquidity=100.0,
        volatility=5.0,
        byzantine_mask=byz_mask.tolist(),
        alpha=args.alpha,
        eta=args.eta,
        seed=args.seed,
    )

    gb = GraphBuilder(args.num_makers, num_levels=5)
    model = GATRouter(
        in_channels=gb.node_feature_dim,
        hidden_channels=64,
        num_heads=4,
        num_layers=2,
        num_makers=args.num_makers,
    )
    trainer = PPOTrainer(model, gb, PPOConfig(lr=3e-4, ppo_epochs=4))

    weight_history = np.zeros((args.episodes, args.num_makers), dtype=np.float32)
    reward_history = np.zeros(args.episodes, dtype=np.float64)
    fill_rate_history = np.zeros(args.episodes, dtype=np.float64)
    brier_history = np.zeros((args.episodes, args.num_makers), dtype=np.float64)

    for ep in range(args.episodes):
        obs, info = env.reset(seed=int(rng.integers(0, 2**31)))
        gb.reset()
        ep_reward = 0.0
        last_weights = np.ones(args.num_makers, dtype=np.float32) / args.num_makers
        last_fill_rate = 0.0
        last_brier = np.zeros(args.num_makers, dtype=np.float64)

        for _ in range(args.max_steps):
            action, log_prob, value = trainer.select_action(obs)
            next_obs, reward, terminated, truncated, step_info = env.step(action)
            trainer.store_transition(obs, action, log_prob, value, reward,
                                    terminated or truncated)

            if "brier_scores" in step_info:
                gb.update_stats(brier_scores=step_info["brier_scores"])
                last_brier = step_info["brier_scores"]
            if "weights" in step_info:
                last_weights = step_info["weights"]
            if "fill_rate" in step_info:
                last_fill_rate = step_info["fill_rate"]

            ep_reward += reward
            obs = next_obs
            if terminated or truncated:
                break

        trainer.update(obs)
        weight_history[ep] = last_weights
        reward_history[ep] = ep_reward
        fill_rate_history[ep] = last_fill_rate
        brier_history[ep] = last_brier

        if (ep + 1) % 50 == 0:
            avg_r = reward_history[max(0, ep - 49):ep + 1].mean()
            byz_w = weight_history[ep][byz_mask].mean()
            honest_w = weight_history[ep][~byz_mask].mean()
            print(f"  Episode {ep+1:4d}/{args.episodes}  "
                  f"avg_reward={avg_r:+.3f}  "
                  f"honest_w={honest_w:.3f}  byz_w={byz_w:.3f}")

    env.close()

    return {
        "weight_history": weight_history,
        "reward_history": reward_history,
        "fill_rate_history": fill_rate_history,
        "brier_history": brier_history,
        "byz_mask": byz_mask,
        "num_makers": args.num_makers,
        "episodes": args.episodes,
    }


def generate_figure(data: dict, output_dir: str) -> list[str]:
    """Generate publication-quality matplotlib figure.

    Returns list of saved file paths.
    """
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    try:
        plt.style.use("seaborn-v0_8-paper")
    except OSError:
        pass

    weight_history = data["weight_history"]
    byz_mask = data["byz_mask"]
    episodes = data["episodes"]
    num_makers = data["num_makers"]

    fig, (ax1, ax2) = plt.subplots(2, 1, figsize=(8, 7), sharex=True,
                                    gridspec_kw={"height_ratios": [3, 1]})

    episodes_x = np.arange(1, episodes + 1)

    for i in range(num_makers):
        if byz_mask[i]:
            ax1.plot(episodes_x, weight_history[:, i],
                     color="#d62728", alpha=0.7, linewidth=1.2,
                     label="Byzantine" if i == np.where(byz_mask)[0][0] else None)
        else:
            ax1.plot(episodes_x, weight_history[:, i],
                     color="#1f77b4", alpha=0.5, linewidth=0.9,
                     label="Honest" if i == np.where(~byz_mask)[0][0] else None)

    ax1.set_ylabel("Routing Weight $w_i$", fontsize=12)
    ax1.set_title(
        "Byzantine Isolation via Brier Score Penalty (Eq. 2)\n"
        f"{int(byz_mask.sum())}/{num_makers} ghost nodes "
        f"({100 * byz_mask.mean():.0f}% Byzantine)",
        fontsize=13,
    )
    ax1.legend(loc="upper right", fontsize=10, framealpha=0.9)
    ax1.set_ylim(-0.02, None)
    ax1.axhline(y=0.01, color="gray", linestyle="--", linewidth=0.8, alpha=0.6)
    ax1.text(episodes * 0.02, 0.015, "isolation threshold (0.01)",
             fontsize=8, color="gray")

    window = min(20, episodes // 5)
    if window > 1:
        smoothed = np.convolve(data["reward_history"],
                               np.ones(window) / window, mode="valid")
        ax2.plot(np.arange(window, episodes + 1), smoothed,
                 color="#2ca02c", linewidth=1.5)
    else:
        ax2.plot(episodes_x, data["reward_history"],
                 color="#2ca02c", linewidth=1.0)

    ax2.set_xlabel("Training Episode", fontsize=12)
    ax2.set_ylabel("Reward (Eq. 2)", fontsize=12)

    plt.tight_layout()

    out = Path(output_dir)
    out.mkdir(parents=True, exist_ok=True)
    paths = []
    for ext in ("pdf", "png"):
        p = out / f"byzantine_isolation.{ext}"
        fig.savefig(str(p), dpi=200, bbox_inches="tight")
        paths.append(str(p))

    plt.close(fig)
    return paths


def print_summary(data: dict) -> None:
    """Print convergence summary table to stdout."""
    byz_mask = data["byz_mask"]
    weight_history = data["weight_history"]
    episodes = data["episodes"]

    print("\n" + "=" * 64)
    print("  Byzantine Spoofing Resilience — Summary")
    print("=" * 64)

    final_w = weight_history[-1]
    print(f"\n  {'Node':>6s}  {'Type':>10s}  {'Final Weight':>13s}  {'Isolated?':>10s}")
    print(f"  {'----':>6s}  {'----':>10s}  {'------------':>13s}  {'---------':>10s}")

    for i in range(data["num_makers"]):
        ntype = "BYZANTINE" if byz_mask[i] else "honest"
        isolated = "YES" if final_w[i] < 0.01 else "no"
        print(f"  {i:>6d}  {ntype:>10s}  {final_w[i]:>13.4f}  {isolated:>10s}")

    n_byz = int(byz_mask.sum())
    n_isolated = int((final_w[byz_mask] < 0.01).sum())

    convergence_ep = None
    for ep in range(episodes):
        if all(weight_history[ep][byz_mask] < 0.01):
            convergence_ep = ep + 1
            break

    print(f"\n  Byzantine nodes isolated: {n_isolated}/{n_byz}")
    if convergence_ep is not None:
        print(f"  Full isolation achieved at episode: {convergence_ep}")
    else:
        print(f"  Full isolation: NOT achieved in {episodes} episodes")

    avg_reward_last = data["reward_history"][-50:].mean()
    print(f"  Avg reward (last 50 episodes): {avg_reward_last:+.4f}")
    print(f"  Avg fill rate (last 50): {data['fill_rate_history'][-50:].mean():.4f}")
    print("=" * 64 + "\n")


def main(argv: list[str] | None = None) -> None:
    args = parse_args(argv)
    print(f"\nFLARE Byzantine Spoofing Experiment")
    print(f"  Makers: {args.num_makers}, Byzantine: {args.byzantine_fraction:.0%}, "
          f"Episodes: {args.episodes}, Seed: {args.seed}")
    print(f"  Reward: R = sum_i w_i * ({args.alpha}*Gain_i - {args.eta}*B_i)\n")

    data = run_experiment(args)

    if not args.no_plot:
        paths = generate_figure(data, args.output_dir)
        print(f"\nFigures saved:")
        for p in paths:
            print(f"  {p}")

    print_summary(data)


if __name__ == "__main__":
    main()
