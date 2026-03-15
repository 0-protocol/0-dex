# 0-dex Development Plan

> **Status note (Mainnet v0):** Historical sections below include exploratory roadmap items and completed-check claims that are not part of the current mainnet launch surface. For launch truth, use `MAINNET_SCOPE.md` and `MAINNET_GATES.md` as the source of record.

## Current Docs Task
- [x] Study `0-ads/README.md` disclaimer style and tone
- [x] Add an explicit experimental-risk disclaimer to `0-dex/README.md`
- [x] Review wording for consistency with the repo's agent-native voice
- [x] Add a top-level status box to `0-dex/README.md`
- [x] Tighten the opening product description and reduce repeated manifesto copy
- [x] Make devnet / mock / experimental status more explicit in the quick-start narrative

## Codebase Assessment

**0-dex** is an agent-native decentralized exchange where AI agents trade directly via P2P gossip + 0-lang tensor graph intersections. Status: **early devnet / skeleton**.

### What Exists (Working Skeletons)
- [x] Rust node scaffold (`main.rs` wires subsystems via Tokio channels)
- [x] libp2p Gossipsub + mDNS peer discovery (`network.rs`)
- [x] REST/HTTP bridge: `GET /health`, `POST /intent` via Axum (`api.rs`)
- [x] Matching engine skeleton with 0-lang VM integration (`matching.rs`)
- [x] Sandboxed VM execution with timeout (`vm_bridge.rs`)
- [x] secp256k1 signature verification, EIP-191-style hashing (`crypto.rs`)
- [x] EVM ABI encoding for `executeSwap` params (`abi.rs`)
- [x] Settlement engine with ethers-rs RPC + mock fallback (`settlement.rs`)
- [x] EVM escrow contract skeleton (`ZeroDexEscrow.sol`)
- [x] Solana Anchor escrow skeleton (`programs/zero_dex_escrow`)
- [x] Python SDK: `LiteClient`, `create_limit_order` generator
- [x] Example `.0` graph files (`limit.0`, `amm.0`)

### Critical Bugs & Gaps (Must Fix)

1. **Gossip → API broadcast is broken**: `GossipNode::run()` only reads swarm events. The `gossip_tx` channel from `api.rs` feeds into a receiver that `GossipNode` never reads — intents submitted via REST are silently dropped.
2. **Matching engine is disconnected**: In `main.rs:74-77`, the actual `RuntimeGraph` parsing and `evaluate_counterparty()` call are commented out. No matching actually happens.
3. **ABI encoding is missing function selector**: `abi.rs` encodes params via `ethabi::encode()` but omits the 4-byte `executeSwap(...)` function selector — EVM calls would fail.
4. **EVM contract has no signature verification**: `ZeroDexEscrow.sol:52-53` has `_verifySignature` calls commented out. No EIP-712 domain separator defined. Anyone can call `executeSwap`.
5. **Solana program has no Ed25519 verification**: `lib.rs:27-28` has Ed25519 sysvar check commented out. Match proof hash is ignored.
6. **Python SDK signature mismatch**: `client.py` uses `encode_defunct(text=payload)` which adds Ethereum's `\x19Ethereum Signed Message:\n` prefix on top of the custom `\x190-dex Intent:\n` prefix — double-prefixed, won't verify on the Rust side.
7. **`main.rs` ownership bug**: `gossip_tx` is moved into the gossip node spawn (line 36), then cloned for the API server (line 54) — this won't compile as-is because the move happens first.

---

## Plan: Priority-Ordered Workstreams

### P0 — Wire the Core Loop (Make It Actually Work End-to-End)

- [x] **P0.1** Fix `GossipNode` to use `tokio::select!` for concurrent swarm + outbound channel processing
- [x] **P0.2** Fix `main.rs` ownership: clone `outbound_tx` before moving `gossip_node`
- [x] **P0.3** Wire `RuntimeGraph::from_reader` + `evaluate_counterparty` in the main intent loop
- [x] **P0.4** Add 4-byte function selector + `signatureA`/`signatureB` params to `abi.rs`
- [x] **P0.5** Fix Python SDK signing: raw Keccak256 without `encode_defunct` double-wrapping
- [x] **P0.6** `MatchProof` now carries both `local_signature` and `counterparty_signature`
- [x] **P0.7** Verified: `cargo check` passes (0 errors, 2 benign dead-code warnings)
- [ ] **P0.8** Write an integration smoke test: 2 nodes discover each other, one broadcasts a limit order, the other matches against an AMM pool, settlement engine receives a `MatchProof`

### P1 — Smart Contract Hardening (From Skeleton to Secure)

- [x] **P1.1** EVM: Implement EIP-712 typed data signatures in `ZeroDexEscrow.sol` (domain separator, struct hash, `ecrecover`)
- [x] **P1.2** EVM: Add reentrancy guard (`ReentrancyGuard` or CEI pattern), nonce tracking to prevent replay
- [x] **P1.3** EVM: Add chain ID binding and deployed-address binding per EIP-712
- [x] **P1.4** Solana: Implement Ed25519 instruction sysvar verification in `execute_swap`
- [x] **P1.5** Solana: Add PDA vaults for escrowed collateral instead of direct `transferFrom`
- [x] **P1.6** Solana: Add strict token account ownership & mint validation
- [x] **P1.7** Update `abi.rs` to include `signatureA`/`signatureB` in the encoded calldata (matches the Solidity function signature)

### P2 — Matching Engine Maturity

- [x] **P2.1** Real intersection math: replace the placeholder `confidence > 0.8` check with proper multi-dimensional tensor overlap (price bounds, amounts, token IDs)
- [x] **P2.2** Partial fills: allow graphs to match on a subset of the requested amount
- [x] **P2.3** Inject local wallet identity into `MatchProof.local_intent_id` (currently hardcoded `"local_id"`)
- [x] **P2.4** Graph loading: on startup, load all `.0` files from `graphs/intents/` and `graphs/pools/` as local intents
- [x] **P2.5** Hot-reload: watch the `graphs/` directory and register new intents without restart

### P3 — Network Robustness

- [x] **P3.1** Make `GossipNode::run()` use `tokio::select!` to concurrently process swarm events AND outbound publish requests
- [x] **P3.2** Add configurable CLI args: `--http-port`, `--listen-addr`, `--bootstrap-peers`, `--graphs-dir`
- [ ] **P3.3** Add WebSocket-to-libp2p gateway mode for lightweight browser/Python agents
- [ ] **P3.4** Implement peer scoring / rate limiting to prevent gossip spam
- [x] **P3.5** Add metrics endpoint (Prometheus) for monitoring intent throughput, match rate, settlement latency

### P4 — Production Settlement

- [x] **P4.1** Extract real token addresses and amounts from the settled `Tensor` instead of hardcoded WETH/USDC
- [ ] **P4.2** Support configurable chain: mainnet, testnet, devnet via env vars / config file
- [ ] **P4.3** Implement Solana settlement path (currently only EVM is wired)
- [ ] **P4.4** Add retry logic and gas estimation for failed transactions
- [ ] **P4.5** Integrate with Flashbots (EVM) / Jito (Solana) for MEV protection

### P5 — Solver Architecture

- [x] **P5.1** Define Solver node role: `--mode solver` CLI flag, `NodeMode` enum, branching main loop
- [x] **P5.2** Implement libp2p topic sharding: `0-dex-intents` vs `0-dex-solutions` + legacy `0-dex-mempool`
- [x] **P5.3** Intent pool with TTL, dedup, status tracking (Active/Matched/Expired/Exhausted)
- [x] **P5.4** CoW solver engine: directed swap graph, DFS cycle detection, multi-way settlement
- [x] **P5.5** Flashbots (EVM) / Jito (Solana) bundle submission in settlement engine
- [x] **P5.6** AMM bridge graphs: `uniswap_bridge.0`, `raydium_bridge.0`
- [x] **P5.7** `LiquidityRelayer` module for routing stale intents through on-chain AMMs

### P6 — 0-lang Extensions (Upstream PRs Needed)

- [x] **P6.1** `Op::OracleRead` — native Pyth/Chainlink price feed ingestion in the VM
- [x] **P6.2** `Op::VerifyPythPrice` — Ed25519/secp256k1 signature verification on oracle data
- [x] **P6.3** `Op::GetGasPrice` — halt graph if network fees exceed threshold
- [x] **P6.4** `Op::SentimentScore` — lightweight NLP for reactive intents
- [ ] **P6.5** Nonce/state-lock opcodes for preventing double-spend at the graph level

### P7 — Privacy Layer

- [x] **P7.1** `PrivacyPlugin` trait: `wrap_intent()`, `unwrap_intent()`, `verify()` with `UnwrappedIntent` enum
- [x] **P7.2** Naked plugin (default, zero-overhead pass-through)
- [x] **P7.3** TEE plugin: X25519 + ChaCha20-Poly1305 AEAD encryption, env-based key config
- [x] **P7.4** ZK plugin: Risc0-style proof envelope with public output extraction (placeholder prover)
- [x] **P7.5** FHE stub: trait impl with documented research direction
- [x] **P7.6** Privacy wired into main.rs (both agent/solver loops), api.rs, and CLI (`--privacy naked|tee|zk`)

---

## Recommended Execution Order

```
P0 (wire core loop)  →  P1 (contracts)  →  P2 (matching)
        ↓                                       ↓
   P3 (network)  →  P4 (settlement)  →  P5 (solvers)
                                              ↓
                                    P6 (0-lang)  →  P7 (privacy)
```

**Current status**: Core loop (P0), Contracts (P1), Matching (P2), Metrics/Network (P3), Solvers (P5), 0-lang extensions (P6), and Privacy (P7) are implemented. Remaining technical debt: Solana Settlement wiring (P4.3) and Websocket Gateway (P3.3).

### P8 — Cross-Chain Atomic Swaps (HTLCs)
To fulfill the promise of a unified agent economy, `0-dex` must allow agents to swap assets across disparate blockchains without a centralized bridge.
- [ ] **P8.1** Design `0-lang` graph constraints for cross-chain Hash Time-Locked Contracts (HTLCs)
- [ ] **P8.2** Implement HTLC Escrow Contracts on EVM and Solana
- [ ] **P8.3** Add cross-chain state proof verification to `0-lang` (e.g., SPV or Light Client verifiers)
- [ ] **P8.4** Solver logic for routing cross-chain atomic transactions securely

### P9 — LP-Agents (Liquidity Provisioning Automation)
Independent nodes should be able to run automatically to capture spread and yield.
- [ ] **P9.1** Build `lp-agent-core` binary to auto-generate intents based on external pool yields
- [ ] **P9.2** Introduce dynamic spread configuration to automatically adjust `min_price`
- [ ] **P9.3** Implement inventory management (halt intents if exposure is too high on one token)
- [ ] **P9.4** On-chain Rebalancing: trigger AMM bridge swaps to balance token inventories

### P10 — Intent Standardization (ERC-XXXX)
To make `0-lang` tensor graphs an industry standard, we must formalize them.
- [ ] **P10.1** Draft ERC specification for `.0` Intent Graphs
- [ ] **P10.2** Formalize the mathematical definitions for Tensor Overlap and Settlement Math
- [ ] **P10.3** Publish `0-lang` format standard specifically for EVM wallets (MetaMask/Rabby integration)

### P11 — ZK-Prover Network Decentralization
The current ZK privacy plugin uses local proving. It needs a decentralized proving marketplace.
- [ ] **P11.1** Abstract `prove_intent()` to outsource to a distributed prover network
- [ ] **P11.2** Implement fee-market for proof generation
- [ ] **P11.3** Risc0 verifier smart contract deployment on Base Sepolia / Mainnet

### P12 — Moltbook Agent-Social Integration
AI Agents shouldn't just trade in the dark—they should build reputations and gossip on social layers.
- [ ] **P12.1** Moltbook SDK Integration: solvers auto-post "Mega CoW Match!" to `submolt_name: builds`
- [ ] **P12.2** Agent Reputation: factor Moltbook Karma into Peer Scoring (`P3.4`)
- [ ] **P12.3** Web UI Widget: "Trade via 0-dex" button directly embedded in Moltbook profiles
