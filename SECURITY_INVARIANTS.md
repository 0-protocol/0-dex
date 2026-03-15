# 0-dex Security Invariants

This file is the minimum gate for code reviews and release approvals.

## Contract-Level Invariants

- **AuthInvariant:** each party's intent signature must recover to the claimed owner.
- **ReplayInvariant:** each unique settlement id can execute at most once.
- **ExpiryInvariant:** expired intents are always rejected.
- **DomainInvariant:** signatures are bound to the target chain id and escrow contract.
- **AmountInvariant:** transfer amounts cannot exceed each signer's authorized amounts.
- **CallerInvariant:** execution path is either permissionless with full proofs or relayer-gated with explicit policy.

## Node-Level Invariants

- **IngressInvariant:** API accepts only `SignedIntent` payloads that pass schema and size checks.
- **CryptoInvariant:** Rust and Python derive the same signing payload bytes and digest.
- **MatchInvariant:** matching uses token pair, side, and price bounds, not confidence-only heuristics.
- **DoSInvariant:** payload size, queue pressure, and VM limits are enforced.

## Release Gates

- All high and critical findings have reproducible tests.
- End-to-end golden vectors validate Python signing, Rust verification, and EVM verification compatibility.
- CI must pass lint, unit tests, and security checks before merge.
