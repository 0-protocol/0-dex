# simulation/ — Byzantine Adversarial Simulation

> Part of the [0-dex research framework](../README.md). The core 0-dex protocol
> is implemented in Rust + 0-lang. See the [root README](../../README.md).

Simulates a federated dark-pool with configurable fractions of adversarial maker nodes. Directly extends the FLock Byzantine-resilient FL framework to DeFi order routing.

## Threat Model

In the FLock paper, malicious FL participants submit poisoned gradients. Here the analogy is:

| FL (FLock) | Dark-pool routing (0-dex) |
|-----------|--------------------------|
| Poisoned gradient | Spoofed liquidity tensor |
| Gradient aggregation | Order routing weights |
| Accuracy degradation | Fill-rate degradation |
| Brier score on predictions | Brier score on reported vs realised liquidity |

## Attack Strategies

| Class | Behaviour |
|-------|-----------|
| `SpooferMaker` | Reports 2–5× actual liquidity; fills only up to true capacity |
| `SandwichMaker` | Reports truthfully but applies adverse slippage on execution |
| `FreeRiderMaker` | Copies honest reports, never fills — pure intent extraction |

## Brier Score Penalty

The `BrierScoreTracker` maintains per-node scores:

```
BS_i = (1/T) Σ_t ((reported_{i,t} − actual_{i,t}) / max_liq)²
```

Integrated into the RL reward as `reward -= α · Σ_i w_i · BS_i`, driving the PPO agent to assign near-zero weight to high-BS (Byzantine) nodes over time.

## Expected Results

| Scenario | Byzantine % | Convergence | Final fill rate |
|----------|-------------|-------------|-----------------|
| Baseline | 0% | ~500 episodes | Optimal |
| Moderate | 20% | ~1000 episodes | Within 5% of baseline |
| Stress | 30% | ~1500 episodes | Robust; Byzantine weights < 0.01 |

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

@article{dong2022flock,
  title={Flock: Defending malicious behaviors in federated learning with blockchain},
  author={Dong, Nanqing and Sun, Jiahao and Wang, Zhipeng and Zhang, Shuai and Zheng, Siqi},
  journal={arXiv preprint arXiv:2211.04344},
  year={2022}
}

@inproceedings{wang2025aiarena,
  title={{AIArena}: A Blockchain-Based Decentralized {AI} Training Platform},
  author={Wang, Zhipeng and Sun, Rui and Lui, Eric and Zhou, Tao and Wen, Yizhuo and Sun, Jiahao},
  booktitle={Companion Proceedings of the ACM on Web Conference 2025},
  pages={1375--1379},
  year={2025}
}
```
