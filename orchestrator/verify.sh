#!/usr/bin/env bash
# Verification harness, v2.
# Usage: verify.sh <worktree> <domain> <scope>
#   scope: docs | rust | node | full
# Guardrail checks run ONLY on files this task changed. A gate that judges the
# whole repo blocks the very tasks meant to clean the repo.

set -uo pipefail
WORKTREE="${1:?usage: verify.sh <worktree> <domain> <scope>}"
DOMAIN="${2:?}"
SCOPE="${3:-full}"
cd "$WORKTREE" || exit 90

FAILED=0
LOG="$WORKTREE/.verify.log"
: > "$LOG"

run() {
  local name="$1"; shift
  echo "=== $name ===" | tee -a "$LOG"
  if "$@" >>"$LOG" 2>&1; then echo "PASS: $name" | tee -a "$LOG"
  else echo "FAIL: $name" | tee -a "$LOG"; FAILED=1; fi
}

if [ -f "$WORKTREE/BLOCKED.md" ]; then
  echo "AGENT BLOCKED:" | tee -a "$LOG"; cat "$WORKTREE/BLOCKED.md" | tee -a "$LOG"; exit 3
fi

CHANGED=$(git diff --name-only develop...HEAD 2>/dev/null || true)
echo "=== changed files ===" | tee -a "$LOG"
echo "${CHANGED:-none}" | tee -a "$LOG"

changed_matching() { echo "$CHANGED" | grep -E "$1" 2>/dev/null || true; }

case "$SCOPE" in
  docs)
    echo "=== scope docs, skipping build and test ===" | tee -a "$LOG"
    CODE=$(changed_matching '\.(rs|ts|tsx|toml)$')
    if [ -n "$CODE" ]; then
      echo "FAIL: docs task modified code:" | tee -a "$LOG"; echo "$CODE" | tee -a "$LOG"; FAILED=1
    else
      echo "PASS: no code touched" | tee -a "$LOG"
    fi
    ;;
  rust)
    [ -f Cargo.toml ] && { run "cargo fmt check" cargo fmt --all -- --check
      run "cargo clippy" cargo clippy --all-targets -- -D warnings
      run "cargo test" cargo test --all
      run "cargo build" cargo build --release; }
    ;;
  node)
    for dir in platform .; do
      if [ -f "$dir/package.json" ]; then
        echo "=== node checks in $dir ===" | tee -a "$LOG"
        ( cd "$dir" && pnpm install --frozen-lockfile && pnpm exec tsc --noEmit \
          && pnpm lint && pnpm test && pnpm build ) >>"$LOG" 2>&1 \
          && echo "PASS: node checks" | tee -a "$LOG" \
          || { echo "FAIL: node checks" | tee -a "$LOG"; FAILED=1; }
        break
      fi
    done
    ;;
  full)
    [ -f Cargo.toml ] && { run "cargo fmt check" cargo fmt --all -- --check
      run "cargo clippy" cargo clippy --all-targets -- -D warnings
      run "cargo test" cargo test --all
      run "cargo build" cargo build --release; }
    for dir in platform .; do
      if [ -f "$dir/package.json" ]; then
        ( cd "$dir" && pnpm install --frozen-lockfile && pnpm exec tsc --noEmit \
          && pnpm lint && pnpm test && pnpm build ) >>"$LOG" 2>&1 \
          && echo "PASS: node checks" | tee -a "$LOG" \
          || { echo "FAIL: node checks" | tee -a "$LOG"; FAILED=1; }
        break
      fi
    done
    ;;
esac

echo "=== security gate ===" | tee -a "$LOG"
if CHANGED_FILES="$CHANGED" "$(dirname "$0")/security-gate.sh" "$WORKTREE" >>"$LOG" 2>&1; then
  echo "PASS: security gate" | tee -a "$LOG"
else
  echo "FAIL: security gate" | tee -a "$LOG"; FAILED=1
fi

check_changed() {
  local label="$1" pat="$2" fre="$3" files hits
  echo "=== guardrail: $label ===" | tee -a "$LOG"
  files=$(changed_matching "$fre")
  if [ -z "$files" ]; then echo "PASS: $label (nothing relevant changed)" | tee -a "$LOG"; return; fi
  hits=$(echo "$files" | xargs -r grep -nP "$pat" 2>/dev/null | head -10)
  if [ -n "$hits" ]; then
    echo "FAIL: $label" | tee -a "$LOG"; echo "$hits" | tee -a "$LOG"; FAILED=1
  else
    echo "PASS: $label" | tee -a "$LOG"
  fi
}

# Branding: only in user-visible string literals, not comments or doc comments.
check_changed "forbidden branding" '"[^"]*\b(?i:substrate|polkadot|grandpa|parity)\b[^"]*"' '\.(rs|ts|tsx)$'
check_changed "em dash" '\x{2014}' '\.(rs|ts|tsx|md|json|ya?ml)$'
check_changed "placeholders" 'TODO:? *implement|unimplemented!\(|mock data|hardcoded for now' '\.(rs|ts|tsx)$'

if [ "$FAILED" -eq 0 ]; then echo "VERIFY: ALL CHECKS PASSED" | tee -a "$LOG"; exit 0; fi
echo "VERIFY: FAILED" | tee -a "$LOG"; exit 1
