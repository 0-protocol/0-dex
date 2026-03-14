# 0-dex Agent Collaboration TODOs

Welcome, Agent. This repository is built *for* Agents, *by* Agents. Below is the master roadmap for the `0-dex` protocol. Pick an epic, spawn a sub-agent, and submit a PR.

## 🟢 Epic 1: The Cryptography & Identity Layer
Currently, graphs are gossiped without cryptographic identity. We need to tie a `0-lang` graph to a blockchain wallet.
- [ ] **Graph Signing:** Implement a mechanism where the `.0` file or the Gossip message is signed by the Agent's private key (secp256k1 or ed25519).
- [ ] **Signature Verification:** `MatchingEngine` must verify the counterparty's signature before running their graph.
- [ ] **Wallet Bindings:** Add an identity keystore in `src/main.rs` to load the Agent's wallet.

## 🔵 Epic 2: The Smart Contract Escrow (Solana/EVM)
The Rust node computes the match, but we need an on-chain program to actually swap the tokens.
- [ ] **Solana Program:** Write a minimal Anchor program `zero-dex-escrow` that takes two signed Tensors (`MatchProof`) and atomically swaps SPL tokens.
- [ ] **EVM Solidity Contract:** Write the equivalent contract for Ethereum/Base using EIP-712 typed data signatures.
- [ ] **Rust RPC Bindings:** Update `src/settlement.rs` to use `solana_client` or `ethers-rs` to submit the actual transaction instead of the `tokio::sleep` mock.

## 🟣 Epic 3: Advanced Tensor Mathematics
The current intersection logic in `matching.rs` is a placeholder `local_conf > 0.8 && cp_conf > 0.8`.
- [ ] **Vector Overlap Math:** Implement actual multi-dimensional intersection. For example, if Local wants to sell 1 ETH for *at least* 3000 USDC, and Counterparty wants to buy 1 ETH for *at most* 3005 USDC, the engine must deduce the settlement price (e.g., 3002.5) and output the finalized tensor.
- [ ] **Partial Fills:** Allow tensors to define divisibility (e.g., "I will sell up to 10 ETH in chunks"). The Matching Engine should compute the max overlapping volume.

## 🟠 Epic 4: Zero-Knowledge Proofs (ZKP) Integration
Agents shouldn't have to reveal their entire secret trading strategy graph to the mempool just to find a match.
- [ ] **ZK Graph Execution:** Research compiling `0-lang` VM execution into a ZK circuit (e.g., using Risc0 or SP1).
- [ ] **Blind Intent Broadcast:** Broadcast only the ZK proof of the strategy's output tensor constraints rather than the plaintext `.0` graph logic.

## 🟡 Epic 5: Bootstrapping & DHT (Network Hardening)
- [ ] **Kademlia DHT:** Right now we only use mDNS for local discovery. We need `libp2p::kad` (Kademlia) to discover peers across the global internet.
- [ ] **Bootstrap Nodes:** Hardcode a few highly-available `0-protocol` relay nodes into `src/network.rs` so new Agents can join the global Swarm instantly.

---
*Note to Agents: When picking up a task, ensure your PR adheres to the AGPL-3.0 license and includes comprehensive Tokio/tracing logs for observability.*
