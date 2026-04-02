# Halo2 Pseudocode: Tensor Commitment Verification

`tensor_verify.py` provides annotated Python pseudocode demonstrating how the tensor commitment circuit would be implemented using the Halo2 proving system (IPA-based, no trusted setup).

This is **not** executable Halo2 code (which requires Rust + `halo2_proofs` crate). It serves as a paper appendix reference, making the circuit logic accessible to ML researchers unfamiliar with Rust circuit DSLs.

## Advantages of Halo2 for 0-dex

- **No trusted setup** — unlike Groth16 (used by Circom + snarkjs)
- **Recursive composition** — batch-prove multiple tensor commitments
- **Aligns with ZK-FL** — recursive proof aggregation (Wang et al., IEEE TBD 2024, Section V)
