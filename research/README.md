# 0-dex Research Framework (FLARE)

> **This directory contains the ML/RL evaluation harness for the FLARE paper.**
> The core 0-dex protocol is implemented in Rust + [0-lang](https://github.com/0-protocol/0-lang).
> See the [root README](../README.md) for the protocol architecture.

## Overview

The research framework formalises the zero-TVL federated dark-pool routing problem as a POMDP and evaluates it using Gymnasium RL environments, GNN-based agents (PyTorch Geometric), and Byzantine adversarial simulations. It is designed to produce reproducible experimental results for the FLARE paper.

## Quick Start

```bash
cd research
pip install -e ".[dev]"          # or: pip install -r requirements-research.txt

# Run the 20% Byzantine experiment
python -m research.experiments.run_experiment \
    --config research/experiments/config/byzantine_20.yaml

# View results
tensorboard --logdir research/experiments/results/
```

## Modules

| Directory | Description | Key technique |
|-----------|-------------|---------------|
| `envs/` | Gymnasium `ZeroTVLDarkPool-v0` environment | POMDP with Ephemeral Tensor observations (Def. 3.1) |
| `agents/` | GNN routing policy (GATv2Conv + PPO/A3C) | Graph attention over maker-node features (Eq. 3) |
| `simulation/` | Byzantine adversarial simulation | Brier-score penalty isolates spoofing nodes (Eq. 2) |
| `zk/` | ZK verification logic (Circom + Halo2) | Tensor commitment proves liquidity solvency (Sec. 4.1) |
| `experiments/` | YAML-driven reproducible experiment runner | TensorBoard + CSV logging (Sec. 5) |

## Relationship to the Core Protocol

The research framework **evaluates** 0-dex routing policies but does not replace the core protocol stack:

```
                 ┌──────────────────────────────────────┐
                 │          0-dex Core (Rust)            │
                 │  0-lang VM  ·  libp2p gossip  ·  EVM │
                 │  SecureVM   ·  matching.rs    ·  P2P  │
                 └───────────────┬──────────────────────┘
                                 │ models the behaviour of
                 ┌───────────────▼──────────────────────┐
                 │      research/ (Python)               │
                 │  Gymnasium env  ·  GNN agent  ·  sim  │
                 │  Brier penalty  ·  ZK proofs  ·  A3C  │
                 └──────────────────────────────────────┘
```

- **0-lang** is the production intent language — agents write `.0` graph files
- **Python** is the training/evaluation harness — standard ML tooling for the paper
- Trained routing policies are designed to be deployed as native 0-lang routing graphs

## Citing

If you use this research framework, please cite the relevant papers:

```bibtex
@article{wang2024zkfl,
  title={Zero-Knowledge Proof-Based Gradient Aggregation for Federated Learning},
  author={Wang, Zhipeng and Dong, Nanqing and Sun, Jiahao and Knottenbelt, William and Guo, Yike},
  journal={IEEE Transactions on Big Data},
  volume={11}, number={2}, pages={447--460}, year={2024}
}

@article{dong2024defending,
  title={Defending against poisoning attacks in federated learning with blockchain},
  author={Dong, Nanqing and Wang, Zhipeng and Sun, Jiahao and Kampffmeyer, Michael and Knottenbelt, William and Xing, Eric},
  journal={IEEE Transactions on Artificial Intelligence},
  volume={5}, number={7}, pages={3743--3756}, year={2024}
}

@article{piao2025garnn,
  title={{GARNN}: an interpretable graph attentive recurrent neural network for predicting blood glucose levels via multivariate time series},
  author={Piao, Chengzhe and Zhu, Taiyu and Baldeweg, Stephanie E and Taylor, Paul and Georgiou, Pantelis and Sun, Jiahao and Wang, Jun and Li, Kezhi},
  journal={Neural Networks}, volume={185}, pages={107229}, year={2025}
}

@inproceedings{wang2025aiarena,
  title={{AIArena}: A Blockchain-Based Decentralized {AI} Training Platform},
  author={Wang, Zhipeng and Sun, Rui and Lui, Eric and Zhou, Tao and Wen, Yizhuo and Sun, Jiahao},
  booktitle={Companion Proceedings of the ACM on Web Conference 2025},
  pages={1375--1379}, year={2025}
}

@article{lui2024sok,
  title={{SoK}: Blockchain-Based Decentralized {AI} ({DeAI})},
  author={Lui, Eric and Sun, Rui and Shah, Vraj and Xiong, Xihan and Sun, Jiahao and Crapis, Davide and Knottenbelt, William and Wang, Zhipeng},
  journal={arXiv preprint arXiv:2411.17461}, year={2024}
}
```
