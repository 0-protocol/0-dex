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
    args = parser.parse_args(argv)

    from simulation.runner import RunConfig, run

    configs: list[Path] = []
    if args.config:
        configs.append(Path(args.config))
    else:
        configs = sorted(Path(args.config_dir).glob("*.yaml"))

    if not configs:
        print("No config files found.", file=sys.stderr)
        sys.exit(1)

    for cfg_path in configs:
        print(f"\n{'='*60}")
        print(f"  Experiment: {cfg_path.stem}")
        print(f"{'='*60}\n")

        config = RunConfig.from_yaml(cfg_path)
        if args.seed is not None:
            config.seed = args.seed

        summary = run(config)
        print(f"\nDone. avg_reward(last 100) = {summary['final_avg_reward']:.4f}")
        print(f"Logs → {summary['log_dir']}")


if __name__ == "__main__":
    main()
