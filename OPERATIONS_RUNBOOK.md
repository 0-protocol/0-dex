# 0-dex Operations Runbook

This runbook defines the minimum operational process before any public deployment.

## 1) Environment Promotion

- `dev`: local testing only.
- `staging`: public testnet plus synthetic traffic.
- `mainnet`: only after all gates pass.

Promotions require:

1. All CI checks green.
2. Security checklist complete.
3. Dry-run deployment and rollback rehearsal.
4. Sign-off from protocol and security owners.

## 2) Deployment

1. Build and test:
   - `cargo test --all-targets`
   - `cargo clippy --all-targets -- -D warnings`
2. Verify environment variables from `.env.example`.
3. Confirm contract address and chain id match the deployment target.
4. Start node with explicit environment values.

## 3) Rollback

1. Stop relayer processes.
2. Revert runtime to previous tagged release.
3. Restore previous known-good environment values.
4. Re-enable service only after health checks pass.

## 4) Key Management

- Development can use local env vars.
- Staging and mainnet must use managed key custody (KMS/HSM/remote signer).
- Rotate relayer keys after incident or on fixed cadence.

## 5) Monitoring Baseline

- API request rate and failure rate.
- Intent verification failures.
- Match creation rate.
- Settlement submission failures and revert reasons.
- RPC latency and provider error spikes.

## 6) Incident Response

1. Triage severity and blast radius.
2. Freeze settlement submissions if fund safety is uncertain.
3. Capture logs, tx hashes, and reproduction steps.
4. Publish internal postmortem with fix and regression tests.
