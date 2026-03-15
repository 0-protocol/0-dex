#!/usr/bin/env bash
set -euo pipefail

echo "[preflight] running rust format check"
cargo fmt -- --check

echo "[preflight] running clippy"
cargo clippy --all-targets -- -D warnings

echo "[preflight] running tests"
cargo test --all-targets

if command -v forge >/dev/null 2>&1; then
  echo "[preflight] running foundry contract tests"
  forge test
else
  echo "[preflight] skipping foundry tests (forge not installed)"
fi

if command -v pytest >/dev/null 2>&1; then
  echo "[preflight] running python sdk tests"
  PYTHONPATH=python pytest -q python/tests
else
  echo "[preflight] skipping python tests (pytest not installed)"
fi

echo "[preflight] all checks passed"
