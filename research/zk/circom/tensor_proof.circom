/*
 * tensor_proof.circom — ZK circuit proving a liquidity tensor is backed by
 * real on-chain funds.
 *
 * ZK Statement
 * ------------
 *   Public inputs:  commitment_hash, min_liquidity_bound
 *   Private inputs: balances[K], salt
 *
 *   Proof:
 *     Poseidon(balances[0], ..., balances[K-1], salt) == commitment_hash
 *     AND  sum(balances) >= min_liquidity_bound
 *
 * This extends the ZK gradient-commitment paradigm from:
 *   Wang et al., "Zero-Knowledge Proof-Based Gradient Aggregation for
 *   Federated Learning", IEEE Transactions on Big Data 11(2), 2024.
 *
 * The circuit is simplified for research demonstration.  A production
 * version would use field-native Poseidon over BN254 scalars and include
 * per-balance range checks (e.g., 0 <= balance_i < 2^64).
 *
 * Compile:  circom tensor_proof.circom --r1cs --wasm --sym
 * Setup:    snarkjs groth16 setup tensor_proof.r1cs pot12_final.ptau circuit.zkey
 * Prove:    snarkjs groth16 prove circuit.zkey witness.wtns proof.json public.json
 * Verify:   snarkjs groth16 verify verification_key.json public.json proof.json
 */

pragma circom 2.1.6;

include "node_modules/circomlib/circuits/poseidon.circom";
include "node_modules/circomlib/circuits/comparators.circom";

/*
 * TensorProof(K)
 *   K = number of price levels (balance slots)
 *
 * Public:
 *   commitment_hash   — Poseidon hash of (balances, salt)
 *   min_liquidity      — lower bound on sum(balances)
 *
 * Private:
 *   balances[K]        — actual token balances per level
 *   salt               — blinding factor
 */
template TensorProof(K) {
    // --- Public inputs ---
    signal input commitment_hash;
    signal input min_liquidity;

    // --- Private inputs ---
    signal input balances[K];
    signal input salt;

    // 1. Compute Poseidon hash of (balances || salt)
    //    Poseidon with K+1 inputs
    component hasher = Poseidon(K + 1);
    for (var i = 0; i < K; i++) {
        hasher.inputs[i] <== balances[i];
    }
    hasher.inputs[K] <== salt;

    // Constrain: hash output must equal the public commitment
    commitment_hash === hasher.out;

    // 2. Sum all balances
    signal cumsum[K + 1];
    cumsum[0] <== 0;
    for (var i = 0; i < K; i++) {
        cumsum[i + 1] <== cumsum[i] + balances[i];
    }

    // 3. Prove sum(balances) >= min_liquidity
    //    Using GreaterEqThan with 64-bit operands
    component geq = GreaterEqThan(64);
    geq.in[0] <== cumsum[K];
    geq.in[1] <== min_liquidity;
    geq.out === 1;
}

// Default instantiation: 5 price levels
component main {public [commitment_hash, min_liquidity]} = TensorProof(5);
