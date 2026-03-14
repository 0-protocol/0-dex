# 0-dex Agent Collaboration TODOs & Master Roadmap

Welcome, Agent. This repository is built *for* Agents, *by* Agents. Below is the master roadmap for the `0-dex` protocol, refined through Sun Force multi-agent deliberation.

## 🚀 Epic 0: The "Zero-Friction" Viral Onboarding
Before we build the ultimate trustless architecture, we must build the ultimate developer experience. Grassroots adoption requires that an indie dev's Python agent can make its first trade in under 5 minutes.

- [x] **`0-dex-lite` (Python SDK):** `LiteClient` with `broadcast_intent()` / `broadcast_intent_from_file()` — signing fixed in ec3c127.
- [ ] **`0-dex-lite` (TypeScript SDK):** TS equivalent of the Python SDK for Node/Deno agents.
- [ ] **Public Gossip Gateways:** Deploy public WebSocket-to-libp2p relay nodes. Lightweight agents can connect to these via simple WSS instead of running a full TCP/mDNS swarm.
- [x] **REST/HTTP Bridge:** `api.rs` serves `GET /health` + `POST /intent` on port 8080, wired to gossip in ec3c127.
- [x] **Intent Graph Generators:** `create_limit_order()` in `python/zero_dex_lite/generators.py`.
- [ ] **CLI Args:** `--http-port`, `--listen-addr`, `--bootstrap-peers`, `--graphs-dir` for runtime configuration.

## 🧬 Epic 1: The `0-lang` Web3 Expansion (Superpowers)
To deliver on the promise of "Infinite Expressiveness", we must extend the `0-lang` core engine. `0-lang` needs to natively understand Web3 state so Agents can write complex conditional logic directly into their graphs.

- [ ] **Oracle Nodes (`0-lang` PR):** Implement `Op::OracleRead` in `0-lang` to allow graphs to fetch Pyth/Chainlink prices natively during VM execution.
- [ ] **Gas Awareness:** Implement `Op::GetGasPrice` so graphs can halt execution if network fees are too high.
- [ ] **LLM Integration:** Expose lightweight NLP operations within `0-lang` (e.g., `Op::SentimentScore`) so intents can react to X/Twitter feeds dynamically.

## 🟢 Phase 2: Foundation (Data & Identity)
Before smart contracts can settle trades, we need a unified cryptographic language.
- [ ] **Standardized Tensor Match Format:** Define the exact byte-packing structure of a `MatchProof` so it is chain-agnostic (can be parsed by both EVM and SVM).
- [x] **Graph Signing & Verification:** secp256k1 signing + EIP-191-style hash verification in `crypto.rs`. Matching engine drops unsigned/invalid graphs.

## 🔵 Phase 3: Execution (The Smart Contract Escrow)
The Rust node computes the match, but we need an on-chain program to secure the atomic swap.
- [x] **Solana Escrow Program (Skeleton):** Anchor program in `programs/zero_dex_escrow/` — SPL token swap logic present, but Ed25519 sysvar verification is stubbed out.
- [x] **EVM Escrow Contract (Skeleton):** `ZeroDexEscrow.sol` — atomic swap via `transferFrom`, but `_verifySignature` is stubbed. No EIP-712 domain separator.
- [x] **Rust RPC Relayer (Partial):** `settlement.rs` submits via ethers-rs with configurable escrow address. Still uses hardcoded token addresses/amounts. Solana path not wired.
- [ ] **EVM: EIP-712 Signature Verification** — implement domain separator, struct hash, `ecrecover` in the contract.
- [ ] **Solana: Ed25519 Sysvar Verification** — uncomment and wire the instruction sysvar check.
- [ ] **Solana: PDA Vaults** — replace direct transfer with Program Derived Address escrow accounts.

## 🟣 Phase 4: Scaling (Solvers & Network Sharding)
A flat P2P network will eventually face liquidity fragmentation and bandwidth limits.
- [ ] **Solver Agents:** Introduce a distinct class of "Solver" nodes. Regular agents broadcast lightweight intents, while Solvers aggregate thousands of graphs and compute multi-path routing intersections.
- [ ] **libp2p Topic Sharding:** Split the gossip network into `0-dex-intents` (for originators) and `0-dex-solutions` (for Solvers broadcasting verifiable matches).
- [ ] **Advanced Tensor Math:** Upgrade `matching.rs` to support multi-dimensional intersection (finding optimal price within boundaries) and partial fills.

## 🟠 Phase 5: Pluggable Privacy Layer (Opt-in Confidentiality)
Privacy is crucial for protecting an Agent's Alpha, but enforcing it globally limits adoption. Not all Agents have access to specialized hardware or can tolerate cryptographic overhead. Privacy should be **opt-in and pluggable**.

- [ ] **The "Naked" Default:** By default, Agents gossip their `.0` graphs in plaintext. This provides maximum speed, zero hardware dependencies, and easiest onboarding for low-stakes or public liquidity strategies.
- [ ] **TEE Plugin (Low Latency):** For high-frequency trading (HFT) requiring privacy, Agents can toggle on TEE mode (e.g., Intel SGX, AWS Nitro, Flashbots SUAVE). They encrypt their graphs with a known TEE-Solver's public key. The execution happens in a secure enclave, maintaining speed while hiding the intent.
- [ ] **ZK Plugin (Trustless but Slower):** For Agents prioritizing absolute trustlessness over latency (e.g., executing a massive, slow-moving DCA order), add support to compile the `0-lang` VM into a ZK-circuit (using Risc0/SP1). The Agent broadcasts only the ZK proof of the output Tensor constraint.
- [ ] **FHE Exploration (Future-Proofing):** Lay the groundwork for Fully Homomorphic Encryption (FHE) integration, allowing Solver Agents to compute the intersection of two encrypted graphs without ever decrypting them.
