# 0-dex Swarm Dark Pool (Phase 27)

- Deprecated linear AMM routing.
- Integrated `WASM spawn` & `Op::QuantumJitter`.
- **Mechanism:** On receiving an intent, the Dex Engine spawns 100 micro-agents. They explore liquidity graphs simultaneously and stream results back via `stream<tensor>`.
- **Security:** `Op::VerifyInvariant` guarantees no micro-agent can execute a path that violates the user's initial slippage tolerance.
