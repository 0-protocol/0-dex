# 0-dex Mainnet Gates

A release candidate must satisfy all gates before mainnet deployment.

## Security Gates

- [ ] All critical and high audit findings remediated.
- [x] Contract replay protection and expiry checks tested.
- [x] Signed intent verification is consistent across Python and Rust.
- [ ] Solana path remains disabled unless explicitly reviewed and enabled.

## Engineering Gates

- [x] `cargo fmt -- --check` passes.
- [x] `cargo clippy --all-targets -- -D warnings` passes.
- [x] `cargo test --all-targets` passes.
- [x] Python SDK tests pass.
- [x] Foundry contract tests pass.

## Operations Gates

- [x] Production env values documented and validated.
- [ ] Relayer key custody uses KMS/HSM or equivalent.
- [ ] Monitoring and alerting dashboards exist.
- [ ] Rollback and incident runbooks reviewed.

## Evidence Log

- `python3 -m pytest -q python/tests` -> `6 passed`.
- `forge test -q` -> all contract tests pass.
- Added cross-language signing golden vector at `testdata/signing_vector.json` with Python fixture test.
- `cargo fmt -- --check` passes.
- `cargo clippy --all-targets -- -D warnings` passes.
- `cargo test --all-targets` passes (`12 passed`).
- Toolchain environment fix applied: `/usr/local/include/stdint.h` now uses `#include_next <stdint.h>` to avoid recursive shadowing of SDK `stdint.h`, which unblocked Rust native dependency compilation (`ring`).
