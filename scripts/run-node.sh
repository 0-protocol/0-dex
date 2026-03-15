#!/usr/bin/env bash
set -euo pipefail

if [[ -f ".env" ]]; then
  # shellcheck disable=SC1091
  source ".env"
fi

: "${ZERO_DEX_CHAIN_ID:?ZERO_DEX_CHAIN_ID is required}"
: "${ZERO_DEX_RPC_URL:?ZERO_DEX_RPC_URL is required}"
: "${ZERO_DEX_ESCROW_ADDRESS:?ZERO_DEX_ESCROW_ADDRESS is required}"
: "${ZERO_DEX_HTTP_PORT:=8080}"
: "${ZERO_DEX_P2P_LISTEN_ADDR:=/ip4/0.0.0.0/tcp/7000}"

if [[ "${ZERO_DEX_ALLOW_SIMULATION:-false}" != "true" ]] && [[ -z "${ZERO_DEX_RELAYER_KEY:-}" ]]; then
  echo "ERROR: ZERO_DEX_RELAYER_KEY is required unless ZERO_DEX_ALLOW_SIMULATION=true"
  exit 1
fi

echo "Starting 0-dex node on chain ${ZERO_DEX_CHAIN_ID}, http ${ZERO_DEX_HTTP_PORT}, p2p ${ZERO_DEX_P2P_LISTEN_ADDR}"
cargo run
