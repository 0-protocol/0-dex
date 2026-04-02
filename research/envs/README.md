# envs/ — Gymnasium RL Environments

> Part of the [0-dex research framework](../README.md). The core 0-dex protocol
> is implemented in Rust + 0-lang. See the [root README](../../README.md).

Standard [Gymnasium](https://gymnasium.farama.org/) environments for dark-pool order routing research.

## `ZeroTVLDarkPool-v0`

| Attribute | Spec |
|-----------|------|
| **Observation** | `Box(N, K)` — Ephemeral probability tensor per maker, decayed by `exp(-λ·Δt)` |
| **Action** | `Box(N,)` — Continuous routing-weight logits (softmax-normalised) |
| **Reward** | `fill_rate × price_improvement − slippage − α·brier_penalty` |
| **State (hidden)** | Full order book across N makers — strictly invisible to agent |

### Quick start

```python
import gymnasium as gym
import envs  # registers ZeroTVLDarkPool-v0

env = gym.make(
    "ZeroTVLDarkPool-v0",
    num_makers=10,
    num_levels=5,
    decay_lambda=0.1,
    byzantine_mask=[False]*8 + [True]*2,
)
obs, info = env.reset(seed=42)
action = env.action_space.sample()
obs, reward, terminated, truncated, info = env.step(action)
```

### Design rationale

The **ephemeral tensor** observation models real-world information staleness in dark pools: maker reports arrive asynchronously and degrade over time. The agent must learn to discount stale information — a signal that also correlates with Byzantine behaviour since honest makers report more frequently.

The **Brier-score penalty** in the reward directly implements the reputation mechanism from the FLock framework, applied to DeFi routing rather than FL gradient aggregation.

## Related Publications

```bibtex
@article{dong2024defending,
  title={Defending against poisoning attacks in federated learning with blockchain},
  author={Dong, Nanqing and Wang, Zhipeng and Sun, Jiahao and Kampffmeyer, Michael and Knottenbelt, William and Xing, Eric},
  journal={IEEE Transactions on Artificial Intelligence},
  volume={5},
  number={7},
  pages={3743--3756},
  year={2024}
}

@article{piao2025garnn,
  title={{GARNN}: an interpretable graph attentive recurrent neural network for predicting blood glucose levels via multivariate time series},
  author={Piao, Chengzhe and Zhu, Taiyu and Baldeweg, Stephanie E and Taylor, Paul and Georgiou, Pantelis and Sun, Jiahao and Wang, Jun and Li, Kezhi},
  journal={Neural Networks},
  volume={185},
  pages={107229},
  year={2025}
}
```
