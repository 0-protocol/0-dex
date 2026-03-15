# 0-dex Mainnet Gates

A release candidate must satisfy all gates before mainnet deployment.

## Security Gates

- [ ] All critical and high audit findings remediated.
- [ ] Contract replay protection and expiry checks tested.
- [ ] Signed intent verification is consistent across Python and Rust.
- [ ] Solana path remains disabled unless explicitly reviewed and enabled.

## Engineering Gates

- [ ] `cargo fmt -- --check` passes.
- [ ] `cargo clippy --all-targets -- -D warnings` passes.
- [ ] `cargo test --all-targets` passes.
- [ ] Python SDK tests pass.

## Operations Gates

- [ ] Production env values documented and validated.
- [ ] Relayer key custody uses KMS/HSM or equivalent.
- [ ] Monitoring and alerting dashboards exist.
- [ ] Rollback and incident runbooks reviewed.
