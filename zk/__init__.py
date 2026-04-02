"""Zero-knowledge verification logic for liquidity-tensor commitments.

Demonstrates that a TEE/maker node's reported liquidity tensor is backed
by real on-chain funds without revealing exact balances.

Extends the ZK-FL verification paradigm to DeFi dark-pool routing:
  Wang et al., "Zero-Knowledge Proof-Based Gradient Aggregation for
  Federated Learning", IEEE Transactions on Big Data 11(2), 2024.
"""

from zk.tensor_commitment import TensorCommitment

__all__ = ["TensorCommitment"]
