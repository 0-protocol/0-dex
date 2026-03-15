# 0-dex Mainnet Scope Freeze (v0)

This document freezes the security scope for the first production release.

## Launch Scope

- Settlement chain: EVM only.
- Solana program: experimental, not in production traffic.
- Order type: limit order only (single fill, no partial fills).
- Asset model: ERC20 to ERC20 only.
- Matching model: deterministic pair and price overlap checks.
- API ingress: signed intents only.

## Non-Goals For v0

- Multi-chain atomic settlement.
- Solver auctions and complex route aggregation.
- Privacy plugins (TEE, ZK, FHE).
- Cross-intent partial fill orchestration.

## Canonical SignedIntent

Signed payload fields:

- `version`: protocol version string.
- `chain_id`: EVM chain id the order is valid on.
- `nonce`: per-owner monotonic or unique nonce.
- `deadline_unix`: unix second expiration.
- `owner_address`: EVM address.
- `verifying_contract`: escrow contract address bound into signature domain.
- `base_token`: market pair base asset (shared by buy and sell orders).
- `quote_token`: market pair quote asset (shared by buy and sell orders).
- `side`: `buy` or `sell`.
- `amount_in`: for `sell`, base amount being sold; for `buy`, quote amount being spent.
- `min_amount_out`: for `sell`, minimum quote out; for `buy`, minimum base out.
- `graph_content`: optional human-readable strategy graph.

Signature format:

- Message: 32-byte keccak hash of ABI-encoded resolved settlement intent fields.
- Scheme: secp256k1 recoverable signature over Ethereum personal-sign hash.

## Canonical MatchProof

- `match_id`: deterministic hash of both intents and resolved amounts.
- `maker_intent` and `taker_intent`: full signed intents.
- `amount_a` and `amount_b`: exact settled transfer amounts.
- `relayer`: optional relayer address for observability.
- `matched_at_unix`: settlement intent timestamp.

## Mandatory Security Invariants

1. A swap is executable only when both signed intents validate.
2. A swap is executable only before both deadlines.
3. A swap is executable only once (`match_id` replay protected).
4. The contract must reject wrong chain id domain usage.
5. Settlement cannot mutate signed amounts outside user-authorized bounds.
6. One-sided transfer success must not leave protocol in inconsistent state.
7. Off-chain components must reject malformed and oversized payloads before matching.
