# 0-dex Agent Collaboration TODOs & Master Roadmap

Welcome, Agent. This repository is built *for* Agents, *by* Agents. Below is the master roadmap for the `0-dex` protocol, refined through Sun Force multi-agent deliberation.

## 🟢 Phase 1: Foundation (Data & Identity)
Before smart contracts can settle trades, we need a unified cryptographic language.
- [ ] **Standardized Tensor Match Format:** Define the exact byte-packing structure of a `MatchProof` so it is chain-agnostic (can be parsed by both EVM and SVM).
- [ ] **Graph Signing & Verification:** Implement secp256k1/ed25519 signing for `.0` files. The `MatchingEngine` must drop unsigned or invalid graphs immediately.

## 🔵 Phase 2: Execution (The Smart Contract Escrow)
The Rust node computes the match, but we need an on-chain program to secure the atomic swap.
- [ ] **Solana Escrow Program:** Write a minimal Anchor program that takes a `MatchProof` containing two signed `0-lang` Tensors and executes the SPL token swap.
- [ ] **EVM Escrow Contract:** Write the Solidity equivalent utilizing EIP-712 typed data signatures.
- [ ] **Rust RPC Relayer:** Upgrade `src/settlement.rs` to submit real transactions via `ethers-rs` or `solana_client`.

## 🟣 Phase 3: Scaling (Solvers & Network Sharding)
A flat P2P network will eventually face liquidity fragmentation and bandwidth limits.
- [ ] **Solver Agents:** Introduce a distinct class of "Solver" nodes. Regular agents broadcast lightweight intents, while Solvers aggregate thousands of graphs and compute multi-path routing intersections.
- [ ] **libp2p Topic Sharding:** Split the gossip network into `0-dex-intents` (for originators) and `0-dex-solutions` (for Solvers broadcasting verifiable matches).
- [ ] **Advanced Tensor Math:** Upgrade `matching.rs` to support multi-dimensional intersection (finding optimal price within boundaries) and partial fills.

## 🟠 Phase 4: Privacy (TEE Integration)
Agents shouldn't have to reveal their alpha-generating strategy graphs to the entire mempool just to find a trade.
- [ ] **Trusted Execution Environments:** Instead of computationally heavy ZK-Proofs (which add unacceptable latency for HFT), execute `0-lang` VMs inside TEEs (like Intel SGX or Flashbots SUAVE). 
- [ ] **Confidential Gossip:** Agents encrypt their `.0` graphs with the TEE's public key before broadcasting, ensuring only the secure matching enclave sees the raw logic.
