# agents/ — GNN-Based Routing Agents

> Part of the [0-dex research framework](../README.md). The core 0-dex protocol
> is implemented in Rust + 0-lang. See the [root README](../../README.md).

Graph Neural Network policy networks for learning dark-pool order routing under adversarial conditions.

## Architecture

```
Observation (N,K)
      │
      ▼
 GraphBuilder ──► PyG Data(x, edge_index)
      │
      ▼
 GATv2Conv × 2  (multi-head attention, 4 heads)
      │
      ├──► Actor head  → routing logits (N,) → softmax → action
      │
      └──► Critic head → state value (scalar)
```

The **GATv2Conv** attention mechanism learns *which makers to trust* based on their reported liquidity, historical fill rates, and Brier scores. This is directly analogous to the attention-weighted aggregation in the GARNN architecture (Piao et al., Neural Networks 2025).

## Components

| File | Purpose |
|------|---------|
| `gnn_router.py` | `GATRouter` — actor-critic with GATv2Conv backbone |
| `graph_builder.py` | `GraphBuilder` — converts (N,K) observations into PyG `Data` |
| `ppo_trainer.py` | `PPOTrainer` — PPO/A3C with GAE, rollout buffer |

## Training

The PPO trainer runs multiple environment workers that collect rollouts and aggregate gradients — mirroring the federated gradient aggregation pattern in ZK-FL (Wang et al., IEEE TBD 2024).

```python
from agents import GATRouter, GraphBuilder
from agents.ppo_trainer import PPOTrainer, PPOConfig

graph_builder = GraphBuilder(num_makers=10, num_levels=5)
model = GATRouter(in_channels=graph_builder.node_feature_dim, num_makers=10)
trainer = PPOTrainer(model, graph_builder, PPOConfig(algorithm="ppo"))
```

## Related Publications

```bibtex
@article{wang2024zkfl,
  title={Zero-Knowledge Proof-Based Gradient Aggregation for Federated Learning},
  author={Wang, Zhipeng and Dong, Nanqing and Sun, Jiahao and Knottenbelt, William and Guo, Yike},
  journal={IEEE Transactions on Big Data},
  volume={11},
  number={2},
  pages={447--460},
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
