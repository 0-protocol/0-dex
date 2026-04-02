"""0-dex research framework -- FLARE paper evaluation harness.

This package contains the ML/RL experiment infrastructure for
studying adversarial dark-pool routing.  The core 0-dex protocol
is implemented in Rust + 0-lang; this Python package serves as
the academic training and evaluation companion.

Modules
-------
envs        Gymnasium RL environments (ZeroTVLDarkPool-v0)
agents      GNN-based routing policy networks (GATv2Conv + PPO/A3C)
simulation  Byzantine adversarial simulation with Brier-score penalty
zk          Zero-knowledge verification logic (Circom + Halo2)
experiments Reproducible YAML-driven experiment runner
"""
