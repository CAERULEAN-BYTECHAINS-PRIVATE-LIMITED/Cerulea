#!/usr/bin/env bash
# Stops every process that scripts/dev-up.sh started for one tenant.
#
# Usage: scripts/dev-down.sh [tenant]
#        scripts/dev-down.sh --all
#
# The stop reaches the process session of that tenant alone. Every other tenant
# keeps running. PostgreSQL keeps running for the next start.
set -uo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
LOCAL_BASE="${CERULEA_LOCAL_BASE:-$ROOT/local}"
TENANTS="$LOCAL_BASE/tenants"
ORDER=(inference explorer dashboard studio apis charlie bob alice)

stop_tenant() {
  local tenant="$1"
  local run="$TENANTS/$tenant/run"
  if [ ! -d "$run" ]; then
    echo "tenant $tenant is running nothing"
    return 0
  fi
  for name in "${ORDER[@]}"; do
    local file="$run/$name.pid"
    [ -f "$file" ] || continue
    local pid group
    pid="$(cat "$file")"
    group=""
    [ -f "$run/$name.pgid" ] && group="$(cat "$run/$name.pgid")"
    if kill -0 "$pid" 2>/dev/null; then
      if [ -n "$group" ]; then
        kill -TERM "-$group" 2>/dev/null
      else
        kill "$pid" 2>/dev/null
      fi
      for _ in 1 2 3 4 5 6 7 8 9 10; do kill -0 "$pid" 2>/dev/null || break; sleep 1; done
      if kill -0 "$pid" 2>/dev/null; then
        [ -n "$group" ] && kill -9 "-$group" 2>/dev/null
        kill -9 "$pid" 2>/dev/null
      fi
      echo "$tenant: $name stopped"
    fi
    rm -f "$file" "$run/$name.pgid"
  done
  echo "tenant $tenant is down"
}

if [ ! -d "$TENANTS" ]; then
  # A tree from the single tenant shape keeps working.
  LEGACY="$ROOT/local/run"
  if [ -d "$LEGACY" ]; then
    for name in "${ORDER[@]}"; do
      file="$LEGACY/$name.pid"
      [ -f "$file" ] || continue
      pid="$(cat "$file")"
      kill "$pid" 2>/dev/null
      rm -f "$file"
      echo "$name stopped"
    done
  fi
  echo "Cerulea local mode is down. PostgreSQL keeps running for the next start."
  exit 0
fi

if [ "${1:-default}" = "--all" ]; then
  for directory in "$TENANTS"/*/; do
    [ -d "$directory" ] || continue
    stop_tenant "$(basename "$directory")"
  done
else
  stop_tenant "${1:-default}"
fi

echo "Cerulea local mode is down. PostgreSQL keeps running for the next start."
