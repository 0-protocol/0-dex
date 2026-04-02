# zk/ — Zero-Knowledge Verification Logic

Demonstrates that a TEE/maker node's reported liquidity tensor is backed by real on-chain funds, without revealing exact balances.

## ZK Statement

```
Public inputs:  commitment_hash, min_liquidity_bound
Private inputs: actual_balances[K], salt

Proof:
  Poseidon(balances[0], ..., balances[K-1], salt) == commitment_hash
  AND  sum(balances) >= min_liquidity_bound
```

## Components

| Path | Description |
|------|-------------|
| `tensor_commitment.py` | Python commitment scheme (SHA-256 stand-in for Poseidon) |
| `circom/tensor_proof.circom` | Functional Circom 2.1 circuit — compile with `circom` + verify with `snarkjs` |
| `halo2/tensor_verify.py` | Annotated Python pseudocode for a Halo2-style IPA circuit |

## Connection to ZK-FL Paper

In ZK-FL (Wang et al., IEEE TBD 2024), ZK proofs verify that gradient aggregation was performed correctly without revealing individual model updates. Here we apply the same principle:

- **ZK-FL**: `Poseidon(gradients, salt) == commitment` proves honest aggregation
- **0-dex**: `Poseidon(balances, salt) == commitment` proves liquidity solvency

The Circom circuit is a simplified but compilable demonstration. A production version would use field-native Poseidon over BN254 and per-balance range checks.

## Integration with Rust Stub

The existing [`src/privacy/zk.rs`](../src/privacy/zk.rs) defines:
- `ZkPlugin` — privacy plugin interface
- `ZkEnvelope` — wire format for ZK-proved intents
- `ZkPublicOutputs` — revealed constraint bounds

This `zk/` module provides the **research-grade verification logic** that the Rust stub references as `TODO: when risc0-zkvm is available`. The mapping is:

| Rust (`src/privacy/zk.rs`) | Python/Circom (`zk/`) |
|----------------------------|----------------------|
| `prove_intent()` placeholder | `tensor_commitment.commit()` + `tensor_proof.circom` |
| `verify()` placeholder | `tensor_commitment.verify()` + `snarkjs groth16 verify` |
| `ZkPublicOutputs` | Circom public inputs: `commitment_hash`, `min_liquidity` |

## Usage (Circom)

```bash
cd zk/circom
npm install circomlib
circom tensor_proof.circom --r1cs --wasm --sym
snarkjs groth16 setup tensor_proof.r1cs pot12_final.ptau circuit.zkey
# Generate witness, prove, verify...
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

@article{lui2024sok,
  title={{SoK}: Blockchain-Based Decentralized {AI} ({DeAI})},
  author={Lui, Eric and Sun, Rui and Shah, Vraj and Xiong, Xihan and Sun, Jiahao and Crapis, Davide and Knottenbelt, William and Wang, Zhipeng},
  journal={arXiv preprint arXiv:2411.17461},
  year={2024}
}
```
