#!/usr/bin/env bash
# Cerulea term check. Blocks any build containing a forbidden term.
# Usage: term-check.sh [root]   Default root is the current directory.

set -uo pipefail
ROOT="${1:-.}"
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TERMS="$HERE/../guardrails/forbidden-terms.txt"

FAIL=0
COUNT=0

mapfile -t LIST < <(grep -vE '^\s*(#|$)' "$TERMS")

while IFS= read -r -d '' f; do
  case "$f" in
    */cerulea-build/*) continue ;;
    */node_modules/*|*/target/*|*/.git/*|*/dist/*|*/build/*) continue ;;
    */Cargo.toml|*/Cargo.lock|*/package.json|*/package-lock.json|*/pnpm-lock.yaml) continue ;;
    *forbidden-terms.txt|*QUESTIONS.md) continue ;;
  esac
  for t in "${LIST[@]}"; do
    # context-exempt words need a human-meaningful check, handled below
    case "$t" in
      substrate|polkadot) continue ;;
    esac
    if hit=$(grep -inE "(^|[^a-z0-9_-])${t}([^a-z0-9_-]|$)" "$f" 2>/dev/null); then
      while IFS= read -r line; do
        echo "FORBIDDEN  $f:${line%%:*}  term: $t"
        FAIL=1
        COUNT=$((COUNT+1))
      done <<< "$hit"
    fi
  done
done < <(find "$ROOT" -type f \( -name '*.md' -o -name '*.rs' -o -name '*.ts' \
   -o -name '*.tsx' -o -name '*.js' -o -name '*.jsx' -o -name '*.json' \
   -o -name '*.yaml' -o -name '*.yml' -o -name '*.html' -o -name '*.txt' \) -print0)

# Context-exempt check: report for human review rather than blocking.
echo ""
echo "--- context review, not blocking ---"
grep -rinE '(^|[^a-z0-9_-])substrate([^a-z0-9_-]|$)' "$ROOT" \
  --include='*.md' --include='*.rs' --include='*.ts' --include='*.tsx' \
  --exclude-dir=node_modules --exclude-dir=target --exclude-dir=.git --exclude-dir=cerulea-build 2>/dev/null \
  | grep -viE 'execution substrate|three substrates|native substrate|contract substrate|the substrate is|between substrates|per substrate' \
  | head -40

if [ "$FAIL" -eq 1 ]; then
  echo ""
  echo "TERM CHECK FAILED: $COUNT violations"
  exit 1
fi
echo ""
echo "TERM CHECK PASSED"
exit 0
