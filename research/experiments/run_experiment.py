"""CLI entry point for reproducible dark-pool routing experiments.

Usage
-----
::

    # Run a single experiment
    python -m experiments.run_experiment --config experiments/config/byzantine_20.yaml

    # Run all configs in a directory
    python -m experiments.run_experiment --config-dir experiments/config/

    # Override seed for multiple runs
    python -m experiments.run_experiment --config experiments/config/baseline.yaml --seed 123

    # Eta sweep for Theorem 4.1 validation
    python -m experiments.run_experiment --config experiments/config/eta_sweep.yaml \\
        --eta-override 0.01 0.05 0.1 0.2 0.5 1.0 2.0 5.0
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path


def main(argv: list[str] | None = None) -> None:
    parser = argparse.ArgumentParser(
        description="Run a 0-dex dark-pool routing experiment."
    )
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument(
        "--config", type=str, help="Path to a single YAML config file."
    )
    group.add_argument(
        "--config-dir", type=str, help="Directory of YAML configs to run sequentially."
    )
    parser.add_argument(
        "--seed", type=int, default=None, help="Override the seed in the config."
    )
    parser.add_argument(
        "--eta-override",
        type=float,
        nargs="+",
        default=None,
        help="Sweep over multiple eta values (Theorem 4.1 validation).",
    )
    args = parser.parse_args(argv)

    from research.simulation.runner import RunConfig, run
    from research.simulation.metrics import convergence_episode

    configs: list[Path] = []
    if args.config:
        configs.append(Path(args.config))
    else:
        configs = sorted(Path(args.config_dir).glob("*.yaml"))

    if not configs:
        print("No config files found.", file=sys.stderr)
        sys.exit(1)

    for cfg_path in configs:
        config = RunConfig.from_yaml(cfg_path)
        if args.seed is not None:
            config.seed = args.seed

        if args.eta_override:
            import numpy as np
            print(f"\n{'='*60}")
            print(f"  Eta sweep: {cfg_path.stem}")
            print(f"{'='*60}")
            sweep_results = []
            for eta_val in args.eta_override:
                config.eta = eta_val
                config.log_dir = f"{cfg_path.stem}_eta_{eta_val}"
                print(f"\n--- eta = {eta_val} ---")
                summary = run(config)
                byz_mask = np.array([False] * config.num_makers)
                n_byz = int(round(config.num_makers * config.byzantine_fraction))
                byz_mask[:n_byz] = True
                conv_ep = convergence_episode(
                    summary.get("weight_history", []), byz_mask
                )
                sweep_results.append({
                    "eta": eta_val,
                    "convergence_episode": conv_ep,
                    "final_reward": summary["final_avg_reward"],
                    "final_byz_resilience": summary.get("final_byzantine_resilience", 0),
                })
                print(f"    convergence_episode={conv_ep}, reward={summary['final_avg_reward']:.4f}")

            print(f"\n{'='*60}")
            print("  Eta Sweep Summary (Theorem 4.1)")
            print(f"{'='*60}")
            print(f"  {'eta':>8s}  {'conv_ep':>8s}  {'reward':>10s}  {'byz_res':>8s}")
            for r in sweep_results:
                conv = str(r["convergence_episode"]) if r["convergence_episode"] is not None else "N/A"
                print(f"  {r['eta']:8.3f}  {conv:>8s}  {r['final_reward']:10.4f}  {r['final_byz_resilience']:8.2f}")
        else:
            print(f"\n{'='*60}")
            print(f"  Experiment: {cfg_path.stem}")
            print(f"{'='*60}\n")

            summary = run(config)
            print(f"\nDone. avg_reward(last 100) = {summary['final_avg_reward']:.4f}")
            print(f"Logs -> {summary['log_dir']}")


if __name__ == "__main__":
    main()
