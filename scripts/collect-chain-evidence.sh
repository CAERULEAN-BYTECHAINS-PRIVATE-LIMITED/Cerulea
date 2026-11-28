#!/usr/bin/env bash
# Collect chain security evidence, SBOM, and optional reproducible build digest proof.
# Usage: bash scripts/collect-chain-evidence.sh [--verify-reproducible]

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

COMMIT="$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
STAMP="$(date -u +%Y-%m-%dT%H:%M:%SZ)"

run_control() {
  local id="$1"
  local cmd="$2"
  local dir="$ROOT/security/evidence/$id"
  mkdir -p "$dir"
  echo "[evidence] Running $id: $cmd"
  {
    echo "Control: $id"
    echo "Commit: $COMMIT"
    echo "Timestamp: $STAMP"
    echo "Command: $cmd"
    echo "---"
    eval "$cmd"
  } > "$dir/last-run.txt" 2>&1 || {
    echo "[evidence] FAILED: $id (see $dir/last-run.txt)"
    tail -n 30 "$dir/last-run.txt"
    exit 1
  }
  echo "[evidence] PASSED: $id"
}

run_control "P10-001" "cargo test -p cerulea-node --test equivocation_test"
run_control "P10-002" "cargo test -p cerulea-node --test checkpoint_test"
run_control "P10-003" "cargo test -p cerulea-node --test eclipse_test"
run_control "P10-004" "cargo test -p cerulea-node --test network_bounds_test"
run_control "P10-005" "cargo test -p cerulea-blueprint"

# SEC-008 / P10-022: Chain software bill of materials
mkdir -p "$ROOT/security/evidence/P10-022"
cargo metadata --format-version 1 --locked > "$ROOT/security/evidence/P10-022/sbom-chain.json"
echo "Generated SBOM at security/evidence/P10-022/sbom-chain.json ($STAMP, commit $COMMIT)" > "$ROOT/security/evidence/P10-022/last-run.txt"
echo "[evidence] SBOM generated: security/evidence/P10-022/sbom-chain.json"

# CHN-076 / P10-021: Optional dual-build reproducible digest check
if [ "${1:-}" = "--verify-reproducible" ]; then
  mkdir -p "$ROOT/security/evidence/P10-021"
  TMP1="$(mktemp -d)"
  TMP2="$(mktemp -d)"
  trap 'rm -rf "$TMP1" "$TMP2"' EXIT
  CARGO_TARGET_DIR="$TMP1" cargo build --release --locked -p cerulea-node
  CARGO_TARGET_DIR="$TMP2" cargo build --release --locked -p cerulea-node
  H1="$(sha256sum "$TMP1/release/cerulea-node" | awk '{print $1}')"
  H2="$(sha256sum "$TMP2/release/cerulea-node" | awk '{print $1}')"
  {
    echo "Commit: $COMMIT"
    echo "Timestamp: $STAMP"
    echo "Build 1 SHA256: $H1"
    echo "Build 2 SHA256: $H2"
  } > "$ROOT/security/evidence/P10-021/last-run.txt"
  if [ "$H1" != "$H2" ]; then
    echo "[evidence] FAILED P10-021: binary digests differ ($H1 vs $H2)"
    exit 1
  fi
  echo "[evidence] PASSED P10-021: matching digests $H1"
fi

echo "[evidence] All chain evidence checks complete."
