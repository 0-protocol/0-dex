#!/usr/bin/env bash
set -euo pipefail

if [[ -f ".env" ]]; then
  # shellcheck disable=SC1091
  source ".env"
fi

: "${ZERO_DEX_CHAIN_ID:?ZERO_DEX_CHAIN_ID is required}"
: "${ZERO_DEX_RPC_URL:?ZERO_DEX_RPC_URL is required}"
: "${ZERO_DEX_ESCROW_ADDRESS:?ZERO_DEX_ESCROW_ADDRESS is required}"

echo "Starting 0-dex node on chain ${ZERO_DEX_CHAIN_ID}"
cargo run
