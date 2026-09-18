#!/usr/bin/env bash
# Cerulea local mode. One command starts the full platform on one machine.
#
# Processes started, in order:
#   1. PostgreSQL, the platform database.
#   2. Three validator processes of the Cerulea chain.
#   3. Cerulea APIs, the control plane.
#   4. Cerulea Studio, Cerulea Dashboard and Cerulea Explorer.
#   5. The inference server for Cerulea AI, only when no Rust build is running.
#
# Every process writes a log under local/run/ and a pid file beside it.
# scripts/dev-down.sh stops everything this script started.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CHAIN="$ROOT/chain"
RUN="$ROOT/local/run"
NODE_BIN="${CERULEA_NODE_BIN:-${CARGO_TARGET_DIR:-$CHAIN/target}/release/cerulea-node}"
MODEL="${CERULEA_MODEL:-$HOME/models/Qwen3-8B-Q4_K_M.gguf}"
LLAMA="${CERULEA_LLAMA_SERVER:-$HOME/llama.cpp/build/bin/llama-server}"
export CERULEA_MODE="${CERULEA_MODE:-local}"

mkdir -p "$RUN"

say() { printf '%s\n' "$*"; }
fail() { printf 'dev-up: %s\n' "$*" >&2; exit 1; }

# Start one process in the background, record its pid, and confirm it is alive.
start() {
  local name="$1"; shift
  if [ -f "$RUN/$name.pid" ] && kill -0 "$(cat "$RUN/$name.pid")" 2>/dev/null; then
    say "$name already running with pid $(cat "$RUN/$name.pid")"
    return 0
  fi
  nohup "$@" > "$RUN/$name.log" 2>&1 &
  local pid=$!
  echo "$pid" > "$RUN/$name.pid"
  sleep 1
  kill -0 "$pid" 2>/dev/null || fail "$name exited at once. Read $RUN/$name.log for the reason."
  say "$name started with pid $pid, log at $RUN/$name.log"
}

# Wait until a TCP port answers, up to a bound in seconds.
wait_port() {
  local port="$1" limit="${2:-60}" i=0
  while ! (exec 3<>"/dev/tcp/127.0.0.1/$port") 2>/dev/null; do
    i=$((i+1)); [ "$i" -ge "$limit" ] && return 1
    sleep 1
  done
  return 0
}

# 1. The platform database.
if command -v pg_isready >/dev/null 2>&1 && ! pg_isready -q -h 127.0.0.1 -p 5432; then
  if command -v service >/dev/null 2>&1; then
    service postgresql start >/dev/null 2>&1 || sudo service postgresql start
  fi
fi
wait_port 5432 30 || fail "PostgreSQL answers on no port. Start the database and run this script again."
say "PostgreSQL answers on 5432"

# 2. Three validator processes. Alice, Bob and Charlie form the local network.
[ -x "$NODE_BIN" ] || fail "node binary missing at $NODE_BIN. Run the release build first."
BASE_ARGS=(--chain local --tmp --enable-cerulea-extensions --rpc-cors all --rpc-methods unsafe --no-telemetry)
start alice   "$NODE_BIN" "${BASE_ARGS[@]}" --alice   --port 30333 --rpc-port 9944 --node-key 0000000000000000000000000000000000000000000000000000000000000001
BOOT="/ip4/127.0.0.1/tcp/30333/p2p/12D3KooWEyoppNCUx8Yx66oV9fJnriXwCcXwDDUA2kj6vnc6iDEp"
start bob     "$NODE_BIN" "${BASE_ARGS[@]}" --bob     --port 30334 --rpc-port 9945 --bootnodes "$BOOT"
start charlie "$NODE_BIN" "${BASE_ARGS[@]}" --charlie --port 30335 --rpc-port 9946 --bootnodes "$BOOT"
wait_port 9944 60 || fail "the first validator answers on no port. Read $RUN/alice.log."
say "three validators started"

# 3. Cerulea APIs.
if [ -d "$ROOT/apis" ]; then
  ( cd "$ROOT/apis" && start apis pnpm start )
  wait_port "${CERULEA_API_PORT:-4000}" 60 || fail "Cerulea APIs answers on no port. Read $RUN/apis.log."
fi

# 4. Studio, Dashboard, Explorer.
for app in studio dashboard explorer; do
  if [ -d "$ROOT/$app" ]; then
    ( cd "$ROOT/$app" && start "$app" pnpm start )
  fi
done

# 5. The inference server. Resource discipline: a running Rust build blocks it.
if pgrep -f 'cargo (build|test|check|clippy)' >/dev/null 2>&1; then
  say "a Rust build is running, so the inference server stays stopped"
elif [ -x "$LLAMA" ] && [ -f "$MODEL" ]; then
  start inference "$LLAMA" -m "$MODEL" --port "${CERULEA_INFERENCE_PORT:-8080}" --host 127.0.0.1
else
  say "inference server or model missing. Cerulea AI stays stopped until both exist."
fi

say "Cerulea local mode is up. Stop it with scripts/dev-down.sh."
