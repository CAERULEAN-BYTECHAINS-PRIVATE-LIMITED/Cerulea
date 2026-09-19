#!/usr/bin/env bash
# Cerulea term check. Blocks any build containing a forbidden term.
# Usage: term-check.sh [root]   Default root is the current directory.
# One grep pass carries every term, so the check stays fast on large trees.

set -uo pipefail
ROOT="${1:-.}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TERMS="${TERMS_FILE:-$HERE/../guardrails/forbidden-terms.txt}"

mapfile -t LIST < <(grep -vE '^\s*(#|$)' "$TERMS" | grep -vxE 'substrate|polkadot')

# Build one alternation. Every term is matched case-insensitively at a
# word boundary that treats letters, digits, underscore and hyphen as
# word characters, so a crate name like pallet-cerulea-custody passes.
ALT=""
for t in "${LIST[@]}"; do
  esc=$(printf '%s' "$t" | sed 's/[][\.*^$/]/\&/g')
  ALT="${ALT:+$ALT|}$esc"
done
PATTERN="(^|[^a-zA-Z0-9_-])(${ALT})([^a-zA-Z0-9_-]|$)"

INCLUDES=(--include='*.md' --include='*.rs' --include='*.ts' --include='*.tsx' \
  --include='*.js' --include='*.jsx' --include='*.json' --include='*.yaml' \
  --include='*.yml' --include='*.html' --include='*.txt' --include='*.mjs' --include='*.cjs')
EXCLUDE_DIRS=(--exclude-dir=node_modules --exclude-dir=target --exclude-dir=.git \
  --exclude-dir=dist --exclude-dir=build --exclude-dir=cerulea-build --exclude-dir=.next)
EXCLUDE_FILES=(--exclude=Cargo.toml --exclude=Cargo.lock --exclude=package.json \
  --exclude=package-lock.json --exclude=pnpm-lock.yaml --exclude=forbidden-terms.txt \
  --exclude=QUESTIONS.md --exclude=TERM_VIOLATIONS.md)

COUNT=0
while IFS= read -r line; do
  [ -z "$line" ] && continue
  file="${line%%:*}"; rest="${line#*:}"; num="${rest%%:*}"; text="${rest#*:}"
  term=$(printf '%s' "$text" | grep -oiE "$PATTERN" | head -1 | sed -E 's/^[^a-zA-Z0-9_-]//; s/[^a-zA-Z0-9_-]$//')
  echo "FORBIDDEN  $file:$num  term: $term"
  COUNT=$((COUNT+1))
done < <(grep -rinE "${INCLUDES[@]}" "${EXCLUDE_DIRS[@]}" "${EXCLUDE_FILES[@]}" "$PATTERN" "$ROOT" 2>/dev/null)

echo ""
echo "--- context review, not blocking ---"
grep -rinE "${INCLUDES[@]}" "${EXCLUDE_DIRS[@]}" '(^|[^a-zA-Z0-9_-])substrate([^a-zA-Z0-9_-]|$)' "$ROOT" 2>/dev/null \
  | grep -viE 'execution substrate|three substrates|native substrate|contract substrate|the substrate is|between substrates|per substrate|substrate selection|substrates' \
  | head -40

if [ "$COUNT" -gt 0 ]; then
  echo ""
  echo "TERM CHECK FAILED: $COUNT violations"
  exit 1
fi
echo ""
echo "TERM CHECK PASSED"
exit 0
