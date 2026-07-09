#!/usr/bin/env bash
# Security gate. Runs on every task, every domain, no exceptions.
# Exit 0 = clean. Exit 1 = blocked.

set -uo pipefail
WORKTREE="${1:?usage: security-gate.sh <worktree>}"
cd "$WORKTREE" || exit 90

FAIL=0
say() { echo "[security] $*"; }

EXCL="--exclude-dir=node_modules --exclude-dir=target --exclude-dir=.git --exclude-dir=dist --exclude-dir=.next"

# 1. Committed secrets. This is what let the Alice ed25519 key into the chain repo.
say "scanning for secrets"
SECRET_PATTERNS='-----BEGIN (RSA |EC |OPENSSH |PGP )?PRIVATE KEY|sk-ant-api[0-9]{2}-[A-Za-z0-9_-]{20,}|sk_live_[A-Za-z0-9]{20,}|rk_live_[A-Za-z0-9]{20,}|AKIA[0-9A-Z]{16}|ghp_[A-Za-z0-9]{36}|[0-9]{8,10}:AA[A-Za-z0-9_-]{33}'
HITS=$(grep -rniE "$SECRET_PATTERNS" $EXCL . 2>/dev/null | head -10)
if [ -n "$HITS" ]; then
  say "BLOCKED: credential material in tree"
  echo "$HITS" | cut -c1-160
  FAIL=1
fi

# Key files by name, regardless of content.
KEYFILES=$(find . -type f \( -name "*.pem" -o -name "*.key" -o -name "secret_*" -o -name "id_rsa" -o -name "id_ed25519" -o -name ".env" \) \
  -not -path "./node_modules/*" -not -path "./target/*" -not -path "./.git/*" \
  -not -name "*.example" -not -name ".env.example" 2>/dev/null | head -10)
if [ -n "$KEYFILES" ]; then
  say "BLOCKED: key or env files committed"
  echo "$KEYFILES"
  FAIL=1
fi

# 2. Rust memory and panic safety on untrusted paths.
if [ -f Cargo.toml ]; then
  say "rust unsafe patterns"
  UNSAFE=$(grep -rn "unsafe *{" --include=*.rs $EXCL . 2>/dev/null | head -5)
  if [ -n "$UNSAFE" ]; then
    say "WARNING: unsafe blocks present, review required"
    echo "$UNSAFE" | cut -c1-140
  fi

  # unwrap/expect/panic inside pallet and consensus code is a real availability risk
  PANIC=$(grep -rnE '\.unwrap\(\)|\.expect\(|panic!\(' --include=*.rs \
    $EXCL ./cerulea-pallets ./cerulea-node 2>/dev/null \
    | grep -vE '(#\[cfg\(test\)\]|/tests?/|_test\.rs|mock\.rs|benchmark)' | head -8)
  if [ -n "$PANIC" ]; then
    say "BLOCKED: panic path in consensus or pallet code"
    echo "$PANIC" | cut -c1-140
    FAIL=1
  fi

  if command -v cargo-audit >/dev/null 2>&1; then
    say "cargo audit"
    cargo audit --deny warnings || { say "BLOCKED: vulnerable rust dependency"; FAIL=1; }
  fi
fi

# 3. Node dependency vulnerabilities and TS escapes.
if [ -f package.json ]; then
  say "pnpm audit"
  if ! pnpm audit --audit-level high >/dev/null 2>&1; then
    say "BLOCKED: high or critical severity npm dependency"
    pnpm audit --audit-level high 2>&1 | head -20
    FAIL=1
  fi

  ANY=$(grep -rn ': *any\b' --include=*.ts --include=*.tsx $EXCL . 2>/dev/null \
    | grep -v 'justified:' | head -8)
  if [ -n "$ANY" ]; then
    say "BLOCKED: untyped any without justification comment"
    echo "$ANY" | cut -c1-140
    FAIL=1
  fi

  # Obvious injection and XSS sinks.
  SINK=$(grep -rnE 'dangerouslySetInnerHTML|eval\(|new Function\(|child_process' \
    --include=*.ts --include=*.tsx $EXCL . 2>/dev/null | head -5)
  if [ -n "$SINK" ]; then
    say "BLOCKED: dangerous sink introduced"
    echo "$SINK" | cut -c1-140
    FAIL=1
  fi

  # Raw SQL string interpolation.
  SQLI=$(grep -rnE '(query|execute)\(`[^`]*\$\{' --include=*.ts $EXCL . 2>/dev/null | head -5)
  if [ -n "$SQLI" ]; then
    say "BLOCKED: interpolated SQL, use parameterized queries"
    echo "$SQLI" | cut -c1-140
    FAIL=1
  fi
fi

# 4. Wallets, keys, gas must never reach the user surface.
say "checking user-surface leakage"
LEAK=$(grep -rniE 'gas ?(price|fee|limit)|private ?key|seed ?phrase|mnemonic' \
  --include=*.tsx $EXCL ./apps 2>/dev/null | head -5)
if [ -n "$LEAK" ]; then
  say "BLOCKED: gas, key, or wallet concept exposed in UI"
  echo "$LEAK" | cut -c1-140
  FAIL=1
fi

[ "$FAIL" -eq 0 ] && { say "clean"; exit 0; }
say "GATE FAILED"
exit 1
