# experiments/ — Reproducible Experiment Configurations

> Part of the [0-dex research framework](../README.md). The core 0-dex protocol
> is implemented in Rust + 0-lang. See the [root README](../../README.md).

YAML-driven experiment runner for dark-pool routing research.

## Quick Start

```bash
# Single experiment
python -m experiments.run_experiment --config experiments/config/byzantine_20.yaml

# All configs
python -m experiments.run_experiment --config-dir experiments/config/

# Override seed for multiple runs
python -m experiments.run_experiment --config experiments/config/baseline.yaml --seed 123
```

## Config Structure

```yaml
env:
  num_makers: 10          # N federated maker nodes
  num_levels: 5           # K price levels per maker
  decay_lambda: 0.1       # observation staleness rate
  max_steps: 100          # episode length

agent:
  gat_layers: 2           # GATv2Conv depth
  gat_heads: 4            # attention heads
  hidden_dim: 64          # hidden channel width
  lr: 3.0e-4              # Adam learning rate
  algorithm: ppo          # ppo or a3c

simulation:
  byzantine_fraction: 0.2 # fraction of malicious makers
  attack_type: spoofer    # spoofer, sandwich, or freerider
  brier_alpha: 0.5        # Brier penalty weight in reward

training:
  episodes: 2000          # total training episodes
  seed: 42                # reproducibility seed
  log_dir: experiments/results/byzantine_20
```

## Included Configs

| Config | Byzantine % | Purpose |
|--------|-------------|---------|
| `baseline.yaml` | 0% | Control — optimal convergence reference |
| `byzantine_20.yaml` | 20% | Primary evaluation (matches FLock paper setup) |
| `byzantine_30.yaml` | 30% | Stress test beyond 1/3 BFT threshold |

## Outputs

- **TensorBoard logs** → `experiments/results/<name>/`
- **CSV metrics** → `experiments/results/<name>/metrics.csv`
- Columns: `episode, reward, fill_rate, brier_penalty`
