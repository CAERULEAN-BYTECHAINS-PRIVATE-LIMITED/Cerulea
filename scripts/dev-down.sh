#!/usr/bin/env bash
# Stops every process that scripts/dev-up.sh started, in reverse order.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
RUN="$ROOT/local/run"
[ -d "$RUN" ] || { echo "nothing is running"; exit 0; }
for name in inference explorer dashboard studio apis charlie bob alice; do
  f="$RUN/$name.pid"
  [ -f "$f" ] || continue
  pid="$(cat "$f")"
  if kill -0 "$pid" 2>/dev/null; then
    kill "$pid" 2>/dev/null
    for _ in 1 2 3 4 5 6 7 8 9 10; do kill -0 "$pid" 2>/dev/null || break; sleep 1; done
    kill -0 "$pid" 2>/dev/null && kill -9 "$pid" 2>/dev/null
    echo "$name stopped"
  fi
  rm -f "$f"
done
echo "Cerulea local mode is down. PostgreSQL keeps running for the next start."
