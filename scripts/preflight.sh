#!/usr/bin/env bash
set -euo pipefail

echo "[preflight] running rust format check"
cargo fmt -- --check

echo "[preflight] running clippy"
cargo clippy --all-targets -- -D warnings

echo "[preflight] running tests"
cargo test --all-targets

echo "[preflight] all checks passed"
