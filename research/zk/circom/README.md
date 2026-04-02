# Circom Circuit: Tensor Liquidity Proof

`tensor_proof.circom` implements a Groth16-compatible ZK circuit proving that a maker's committed liquidity tensor is backed by balances exceeding a public threshold.

## Prerequisites

```bash
npm install -g circom snarkjs
npm install circomlib
```

## Compile & Verify

```bash
circom tensor_proof.circom --r1cs --wasm --sym
snarkjs powersoftau new bn128 12 pot12_0000.ptau
snarkjs powersoftau contribute pot12_0000.ptau pot12_final.ptau --name="dev"
snarkjs groth16 setup tensor_proof.r1cs pot12_final.ptau circuit.zkey
snarkjs zkey export verificationkey circuit.zkey verification_key.json
```

The circuit uses 5 price levels by default. Adjust the template parameter `TensorProof(K)` for different configurations.
