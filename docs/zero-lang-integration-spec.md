# 0-dex / 0-lang Integration Spec (Seams and Dependencies)

This document defines what 0-dex can implement locally now, and what requires upstream `0-lang` changes.

## Local Seams Implemented in 0-dex

- `OracleVerifier` trait in `src/upstream.rs`
  - `verify_oracle_constraints(intent) -> Result<bool, String>`
  - Current implementation: `NoopOracleVerifier` (accepts all)
  - Future: plug in proof validation for oracle-fed tensors

- `IntentStateLocker` trait in `src/upstream.rs`
  - `can_match(intent_id, nonce)`
  - `mark_exhausted(intent_id, nonce)`
  - Current implementation: `InMemoryStateLocker`
  - Future: tie to `0-lang` state-lock opcodes and persistent storage

These are wired into the matching path in `src/matching.rs`.

## Upstream 0-lang Requirements

### 1) Oracle Trust

- `Op::OracleRead`: fetch oracle payload into VM
- `Op::VerifyPythPrice` (or equivalent): verify signed oracle payload
- Required to make `OracleVerifier` enforceable with cryptographic guarantees

### 2) Gas Awareness

- `Op::GetGasPrice`: allow graph-level fee guards

### 3) State Locking / Nonce Exhaustion

- Opcode to query nonce / lock state
- Opcode to mark graph exhausted after signature generation
- Required to replace in-memory lock with canonical VM-level semantics

## Ownership Split

### Implement now in 0-dex

- Trait boundaries and call sites (`src/upstream.rs`, `src/matching.rs`)
- Fallback local lock behavior for dev/test
- Operational monitoring and error handling around verification hooks

### Blocked on 0-lang upstream

- Cryptographically sound oracle proof execution in VM
- Canonical nonce/state-lock semantics inside graph execution
- Full trustless anti-replay at graph opcode layer
