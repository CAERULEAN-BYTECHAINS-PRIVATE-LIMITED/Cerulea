#!/usr/bin/env bash
# Chain verification runner for Cerulea.
# Usage: bash scripts/verify.sh

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

echo "[verify] 1/4 Term check on chain crates, tools, docs, and security"
bash scripts/term-check.sh cerulea-blueprint
bash scripts/term-check.sh cerulea-pallets
bash scripts/term-check.sh cerulea-node
bash scripts/term-check.sh cerulea-runtime
bash scripts/term-check.sh cerulea-primitives
bash scripts/term-check.sh cerulea-platform-api
bash scripts/term-check.sh tools
bash scripts/term-check.sh docs
bash scripts/term-check.sh security

echo "[verify] 2/4 Format check"
cargo fmt --all -- --check

echo "[verify] 3/4 Security evidence collection (P10-001..P10-005 + SBOM)"
bash scripts/collect-chain-evidence.sh

echo "[verify] 4/4 Chain unit tests"
cargo test -p cerulea-blueprint -p pallet-cerulea-relayer -p pallet-cerulea-custody -p cerulea-tools --bin seed-etl

echo "[verify] Chain verification complete."
