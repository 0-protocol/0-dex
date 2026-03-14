# 0-dex Production Roadmap: Path to Mainnet

To evolve `0-dex` from a Devnet playground to a production-ready, highly liquid, and secure DEX, we must cross four major engineering chasms. This document outlines the explicit technical roadmap and the dependencies required from the underlying `0-lang` protocol.

## 1. The Oracle Trust Challenge
**Problem:** Currently, agents fetch prices via local HTTP execution. A malicious counterparty can spoof the local HTTP response (e.g., reporting 1 ETH = $1) during their VM execution to trick your graph into signing a bad trade.
**Solution:** Cryptographic Oracle Proofs.
- [ ] **0-dex:** Implement `OracleVerifier` in the Rust Matching Engine. The engine must reject any counterparty graph that relies on unverified external price feeds.
- [ ] **0-lang (Dependency):** Extend `0-lang` to support signed data ingestion (e.g., `Op::VerifyPythPrice`). The VM must natively validate Ed25519/Secp256k1 signatures on incoming oracle tensors before allowing them into the execution path.

## 2. The Concurrency & Double Spend Dilemma
**Problem:** In a pure P2P broadcast, an intent to sell 1 ETH might match with 10 different counterparties simultaneously. Your agent signs 10 match proofs. All 10 race to the blockchain; 1 succeeds, 9 fail, wasting massive amounts of Gas.
**Solution:** Solvers and Strict Sequencing.
- [ ] **0-dex:** Implement `Solver Nodes`. Regular agents only broadcast intents. High-compute Solver agents collect intents, compute the optimal multi-way intersections (Coincidence of Wants), and submit the bundled transaction.
- [ ] **0-dex:** Integrate with Flashbots (EVM) or Jito (Solana) to ensure transactions revert off-chain if the bundle is invalid, guaranteeing 0 Gas loss for failed races.
- [ ] **0-lang (Dependency):** Add nonce-management and state-locking opcodes so a graph can mark itself as "exhausted" after generating a signature.

## 3. The Liquidity Bootstrapping (Cold Start)
**Problem:** On day one, the P2P dark pool will be empty. Two agents will rarely find a perfect overlapping mathematical intersection if there are only 5 agents online.
**Solution:** The AMM Bridge Graph.
- [ ] **0-dex:** Write official "Bridge Intents" (`graphs/pools/uniswap_bridge.0`). These are permanently running graphs that listen to `0-dex` intents and automatically fulfill them by routing the backend trade through Uniswap V3 (EVM) or Raydium (Solana).
- [ ] **0-dex:** Build the `LiquidityRelayer` module in Rust to execute the smart contract leg of the bridged AMM trade.

## 4. Smart Contract Hardening
**Problem:** The current Escrow contracts are conceptual skeletons. They lack cross-chain replay protection and reentrancy guards.
**Solution:** Institutional-grade contract architecture.
- [ ] **0-dex (EVM):** Implement EIP-712 Domain Separators. The signature must bind the intent strictly to the Chain ID and the specific Escrow contract address.
- [ ] **0-dex (Solana):** Implement strict token account validation and PDA (Program Derived Address) vaults to hold locked intent collateral.
- [ ] **0-dex:** Integrate Reentrancy Guards and rigorous bounds checking on the submitted Tensors.
- [ ] **0-dex:** Complete Tier-1 Security Audits (Zellic, OtterSec, Trail of Bits).

---
*If you are an agent reading this, coordinate with your orchestrator. Do not attempt to submit production intent proofs until the Oracle Trust Challenge (Phase 1) is resolved.*
