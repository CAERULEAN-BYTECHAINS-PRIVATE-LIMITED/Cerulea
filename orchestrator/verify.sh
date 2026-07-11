#!/usr/bin/env bash
# Verification harness v3.
# Usage: verify.sh <worktree> <domain> <scope>
#   scope: docs | rust-build | rust-test | node | full
#
# Principles learned the hard way:
#  - Fail fast. Cheapest check first. Never grind an hour to report a fmt error.
#  - Release profile only. Debug and release share no cache.
#  - Never --all-targets. It drags in binaryen and compiles C++ for an hour.
#  - Guardrails judge only what this task changed.
#  - Pre-existing conditions warn. Newly introduced problems block.

set -uo pipefail
export LC_ALL=C
WORKTREE="${1:?usage: verify.sh <worktree> <domain> <scope>}"
DOMAIN="${2:?}"
SCOPE="${3:-rust-build}"
cd "$WORKTREE" || exit 90

LOG="$WORKTREE/.verify.log"
: > "$LOG"
say()  { echo "$*" | tee -a "$LOG"; }
die()  { say "FAIL: $*"; say "VERIFY: FAILED"; exit 1; }
pass() { say "PASS: $*"; }

if [ -f "$WORKTREE/BLOCKED.md" ]; then
  say "AGENT BLOCKED:"; cat "$WORKTREE/BLOCKED.md" | tee -a "$LOG"; exit 3
fi

CHANGED=$(git diff --name-only develop...HEAD 2>/dev/null || true)
say "=== changed files: $(echo "$CHANGED" | grep -c . || echo 0) ==="

if [ "$SCOPE" != "docs" ] && [ -z "$CHANGED" ]; then
  die "agent modified no files. The task was not attempted. Check the environment."
fi

changed_matching() { echo "$CHANGED" | grep -E "$1" 2>/dev/null || true; }

# ---------------------------------------------------------------- guardrails
# Cheap. Run first. Only on changed files.

check() {
  local label="$1" pat="$2" fre="$3" mode="${4:-block}" files hits
  files=$(changed_matching "$fre")
  [ -z "$files" ] && { pass "$label (nothing relevant changed)"; return; }
  hits=$(echo "$files" | xargs -r grep -nP -e "$pat" 2>/dev/null | head -8)
  if [ -n "$hits" ]; then
    if [ "$mode" = "warn" ]; then
      say "WARN: $label"; echo "$hits" >> "$LOG"
    else
      say "$hits"; die "$label"
    fi
  else
    pass "$label"
  fi
}

# Secrets are always a blocker. Nothing else is worth blocking a whole task for.
SECRETS='-----BEGIN (RSA |EC |OPENSSH |PGP )?PRIVATE KEY|sk-ant-api[0-9]{2}-[A-Za-z0-9_-]{20}|sk_live_[A-Za-z0-9]{20}|AKIA[0-9A-Z]{16}|ghp_[A-Za-z0-9]{36}|[0-9]{8,10}:AA[A-Za-z0-9_-]{33}'
check "no secrets" "$SECRETS" '.' block

KEYFILES=$(echo "$CHANGED" | grep -E '\.(pem|key)$|(^|/)(secret_|id_rsa|id_ed25519|\.env$)' || true)
[ -n "$KEYFILES" ] && { say "$KEYFILES"; die "key or env file committed"; }
pass "no key files"

check "no dangerous sinks"  'dangerouslySetInnerHTML|\beval\(|new Function\(' '\.(ts|tsx)$'  block
check "no interpolated SQL" '(query|execute)\(`[^`]*\$\{'                     '\.(ts)$'      block
check "forbidden branding"  '"[^"]*\b(?i:substrate|polkadot|grandpa|parity)\b[^"]*"' '\.(rs|ts|tsx)$' block
# The em dash is three bytes in UTF-8. Matching bytes works in any locale.
check "no em dash"          '\xe2\x80\x94'                                    '\.(rs|ts|tsx|md|ya?ml|json)$' block
check "no placeholders"     'TODO:? *implement|unimplemented!\(|mock data|hardcoded for now' '\.(rs|ts|tsx)$' block

# Pre-existing conditions. Recorded, never blocking. The audit owns these.
check "panic paths"         '\.unwrap\(\)|\.expect\(|panic!\('               '^cerulea-(pallets|node)/.*\.rs$' warn
check "untyped any"         ': *any\b'                                       '\.(ts|tsx)$' warn

# ---------------------------------------------------------------- build

RUST_CHANGED=$(changed_matching '\.(rs|toml)$|^Cargo\.lock$')
NODE_CHANGED=$(changed_matching '^platform/.*\.(ts|tsx|json)$')

case "$SCOPE" in
  docs)
    # Planning and audit tasks may write markdown and plan files, never code.
    CODE=$(changed_matching '\.(rs|ts|tsx|toml)$')
    [ -n "$CODE" ] && { say "$CODE"; die "documentation task modified code"; }
    pass "no code touched"
    ;;

  rust-build|rust-test|full)
    if [ -n "$RUST_CHANGED" ] || [ "$SCOPE" = "rust-test" ]; then
      say "=== cargo fmt check ==="
      cargo fmt --all -- --check >>"$LOG" 2>&1 || die "cargo fmt (run: cargo fmt --all)"
      pass "cargo fmt"

      # --release reuses the warm cache. No --all-targets: it builds binaryen.
      say "=== cargo check --release ==="
      cargo check --release --workspace >>"$LOG" 2>&1 || die "cargo check"
      pass "cargo check"

      say "=== cargo clippy --release (correctness only) ==="
      cargo clippy --release --workspace -- -D clippy::correctness >>"$LOG" 2>&1 \
        || die "clippy correctness lint"
      pass "clippy"

      say "=== cargo build --release ==="
      cargo build --release >>"$LOG" 2>&1 || die "cargo build"
      pass "cargo build"
    fi

    if [ "$SCOPE" = "rust-test" ] || [ "$SCOPE" = "full" ]; then
      say "=== cargo test --release ==="
      cargo test --release --workspace >>"$LOG" 2>&1 || die "cargo test"
      pass "cargo test"
    fi
    ;;&

  node|full)
    if [ -f platform/package.json ] && { [ -n "$NODE_CHANGED" ] || [ "$SCOPE" = "node" ]; }; then
      cd platform || die "platform missing"
      say "=== pnpm install ==="
      pnpm install --frozen-lockfile >>"$LOG" 2>&1 || die "pnpm install"
      say "=== tsc ==="
      pnpm exec tsc --noEmit >>"$LOG" 2>&1 || die "typecheck"
      say "=== lint ==="
      pnpm lint >>"$LOG" 2>&1 || die "lint"
      say "=== build ==="
      pnpm build >>"$LOG" 2>&1 || die "build"
      pass "node checks"
      cd "$WORKTREE" || exit 90
    fi
    ;;
esac

# ------------------------------------------------- advisory scans, non-blocking
say "=== dependency advisories (advisory only) ==="
if [ -n "$RUST_CHANGED" ] && command -v cargo-audit >/dev/null 2>&1; then
  cargo audit >>"$LOG" 2>&1 || say "WARN: rust dependency advisories present"
fi
if [ -n "$NODE_CHANGED" ] && [ -f platform/package.json ]; then
  ( cd platform && pnpm audit --audit-level high ) >>"$LOG" 2>&1 \
    || say "WARN: npm dependency advisories present"
fi

say "VERIFY: ALL CHECKS PASSED"
exit 0
