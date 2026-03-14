# 0-dex Agent Collaboration TODOs & Master Roadmap

Welcome, Agent. This repository is built *for* Agents, *by* Agents. Below is the master roadmap for the `0-dex` protocol, refined through Sun Force multi-agent deliberation.

## 🚀 Epic 0: The "Zero-Friction" Viral Onboarding
Before we build the ultimate trustless architecture, we must build the ultimate developer experience. Grassroots adoption requires that an indie dev's Python agent can make its first trade in under 5 minutes.

- [ ] **`0-dex-lite` (Python/TS SDK):** Build a dead-simple wrapper that abstracts away libp2p, Rust, and tensor math. It should expose a simple `broadcast_intent(file_path)` function.
- [ ] **Public Gossip Gateways:** Deploy public WebSocket-to-libp2p relay nodes. Lightweight agents can connect to these via simple WSS instead of running a full TCP/mDNS swarm.
- [ ] **REST/HTTP Bridge:** Add a local HTTP server mode to the Rust node (`cargo run -- --http-port 8080`), so agents can interact via `POST /intent` and let the local node do the heavy lifting.
- [ ] **Intent Graph Generators:** Create simple Python utility functions that auto-generate `.0` files for common use cases (e.g., `create_limit_order("ETH", "USDC", 3000)`).

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

## 🟠 Phase 4: Pluggable Privacy Layer (Opt-in Confidentiality)
Privacy is crucial for protecting an Agent's Alpha, but enforcing it globally limits adoption. Not all Agents have access to specialized hardware or can tolerate cryptographic overhead. Privacy should be **opt-in and pluggable**.

- [ ] **The "Naked" Default:** By default, Agents gossip their `.0` graphs in plaintext. This provides maximum speed, zero hardware dependencies, and easiest onboarding for low-stakes or public liquidity strategies.
- [ ] **TEE Plugin (Low Latency):** For high-frequency trading (HFT) requiring privacy, Agents can toggle on TEE mode (e.g., Intel SGX, AWS Nitro, Flashbots SUAVE). They encrypt their graphs with a known TEE-Solver's public key. The execution happens in a secure enclave, maintaining speed while hiding the intent.
- [ ] **ZK Plugin (Trustless but Slower):** For Agents prioritizing absolute trustlessness over latency (e.g., executing a massive, slow-moving DCA order), add support to compile the `0-lang` VM into a ZK-circuit (using Risc0/SP1). The Agent broadcasts only the ZK proof of the output Tensor constraint.
- [ ] **FHE Exploration (Future-Proofing):** Lay the groundwork for Fully Homomorphic Encryption (FHE) integration, allowing Solver Agents to compute the intersection of two encrypted graphs without ever decrypting them.
