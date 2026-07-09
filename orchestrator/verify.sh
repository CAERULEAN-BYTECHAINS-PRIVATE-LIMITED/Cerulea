#!/usr/bin/env bash
# Verification harness. Exit 0 only if every check passes.
# Usage: verify.sh <worktree-path> <domain>
# Domains: chain, studio, backend, frontend, sdk, billing, intelligence, devops, testing

set -uo pipefail

WORKTREE="${1:?usage: verify.sh <worktree> <domain>}"
DOMAIN="${2:?usage: verify.sh <worktree> <domain>}"
cd "$WORKTREE" || exit 90

FAILED=0
LOG="$WORKTREE/.verify.log"
: > "$LOG"

run() {
  local name="$1"; shift
  echo "=== $name ===" | tee -a "$LOG"
  if "$@" >>"$LOG" 2>&1; then
    echo "PASS: $name" | tee -a "$LOG"
  else
    echo "FAIL: $name" | tee -a "$LOG"
    FAILED=1
  fi
}

# Anything an agent wrote that blocks itself is an immediate fail.
if [ -f "$WORKTREE/BLOCKED.md" ]; then
  echo "AGENT BLOCKED:" | tee -a "$LOG"
  cat "$WORKTREE/BLOCKED.md" | tee -a "$LOG"
  exit 3
fi

case "$DOMAIN" in
  chain)
    if [ -f Cargo.toml ]; then
      run "cargo fmt check" cargo fmt --all -- --check
      run "cargo clippy"    cargo clippy --all-targets --all-features -- -D warnings
      run "cargo test"      cargo test --all --release
      run "cargo build"     cargo build --release
    else
      echo "no Cargo.toml, skipping rust checks" | tee -a "$LOG"
    fi
    ;;
  studio|backend|frontend|sdk|billing|intelligence)
    if [ -f package.json ]; then
      run "install"   pnpm install --frozen-lockfile
      run "typecheck" pnpm exec tsc --noEmit
      run "lint"      pnpm lint
      run "test"      pnpm test
      run "build"     pnpm build
    else
      echo "no package.json, skipping node checks" | tee -a "$LOG"
    fi
    ;;
  devops|testing)
    [ -f Cargo.toml ]    && run "cargo test" cargo test --all
    [ -f package.json ]  && run "test" pnpm test
    ;;
  *)
    echo "unknown domain: $DOMAIN" | tee -a "$LOG"
    exit 91
    ;;
esac

# Security gate runs on every domain without exception.
run "security gate" "$(dirname "$0")/security-gate.sh" "$WORKTREE"

# Guardrail: forbidden strings in customer-facing surfaces.
echo "=== guardrail: forbidden branding ===" | tee -a "$LOG"
BAD=$(grep -rniE '\b(substrate|polkadot|parity|grandpa|\baura\b|\bbabe\b)\b' \
  --include=*.rs --include=*.ts --include=*.tsx --include=*.md \
  --exclude-dir=node_modules --exclude-dir=target --exclude-dir=.git \
  --exclude=Cargo.toml --exclude=Cargo.lock --exclude=CLAUDE.md \
  . 2>/dev/null | grep -viE '^\s*(//|#)?\s*(use |extern crate|import )' | head -20)
if [ -n "$BAD" ]; then
  echo "FAIL: forbidden branding found:" | tee -a "$LOG"
  echo "$BAD" | tee -a "$LOG"
  FAILED=1
else
  echo "PASS: no forbidden branding" | tee -a "$LOG"
fi

# Guardrail: em dashes.
echo "=== guardrail: em dash ===" | tee -a "$LOG"
if grep -rlP '\x{2014}' --include=*.rs --include=*.ts --include=*.tsx \
   --include=*.md --exclude-dir=node_modules --exclude-dir=target \
   --exclude-dir=.git . 2>/dev/null | head -5 | grep -q .; then
  echo "FAIL: em dash present" | tee -a "$LOG"
  FAILED=1
else
  echo "PASS: no em dash" | tee -a "$LOG"
fi

# Guardrail: no placeholder or fake-success code.
echo "=== guardrail: placeholders ===" | tee -a "$LOG"
PH=$(grep -rniE 'TODO:? *implement|not implemented|unimplemented!\(|placeholder|FIXME|mock data|hardcoded for now' \
  --include=*.rs --include=*.ts --include=*.tsx \
  --exclude-dir=node_modules --exclude-dir=target --exclude-dir=.git \
  . 2>/dev/null | head -10)
if [ -n "$PH" ]; then
  echo "FAIL: placeholder code:" | tee -a "$LOG"
  echo "$PH" | tee -a "$LOG"
  FAILED=1
else
  echo "PASS: no placeholders" | tee -a "$LOG"
fi

if [ "$FAILED" -eq 0 ]; then
  echo "VERIFY: ALL CHECKS PASSED" | tee -a "$LOG"
  exit 0
fi
echo "VERIFY: FAILED" | tee -a "$LOG"
exit 1
