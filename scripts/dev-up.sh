#!/usr/bin/env bash
# Cerulea local mode. One command starts the full platform for one tenant.
#
# Usage: scripts/dev-up.sh [tenant]
#
# The tenant name selects a separate world on this machine. Each tenant holds
# its own data directories, its own database schema, its own process session
# and its own port block. Two tenants run side by side and share nothing.
#
# Processes started, in order:
#   1. PostgreSQL, the platform database, shared by every tenant.
#   2. The tenant schema inside that database, with every migration applied.
#   3. Three validator processes of the Cerulea chain, on the tenant data path.
#   4. Cerulea APIs, the control plane.
#   5. Cerulea Studio, Cerulea Dashboard and Cerulea Explorer.
#   6. The inference server for Cerulea AI, only when no Rust build is running.
#
# Every process writes a log under local/tenants/<tenant>/run/ and a pid file
# beside it. scripts/dev-down.sh <tenant> stops everything this script started
# for that tenant and leaves every other tenant running.
set -uo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CHAIN="$ROOT/chain"
TENANT="${1:-default}"
# The control plane home. A machine whose repository sits on a slow mount points
# this value at a prepared copy on a fast path.
APIS_HOME="${CERULEA_APIS_HOME:-$ROOT/apis}"
NODE_BIN="${CERULEA_NODE_BIN:-${CARGO_TARGET_DIR:-$CHAIN/target}/release/cerulea-node}"
MODEL="${CERULEA_MODEL:-$HOME/models/Qwen3-8B-Q4_K_M.gguf}"
LLAMA="${CERULEA_LLAMA_SERVER:-$HOME/llama.cpp/build/bin/llama-server}"
DB_HOST="${CERULEA_DATABASE_HOST:-127.0.0.1}"
DB_PORT="${CERULEA_DATABASE_PORT:-5544}"
DB_USER="${CERULEA_DATABASE_USER:-postgres}"
DB_NAME="${CERULEA_DATABASE_NAME:-cerulea_apis}"
export CERULEA_MODE="${CERULEA_MODE:-local}"

say() { printf '%s\n' "$*"; }
fail() { printf 'dev-up: %s\n' "$*" >&2; exit 1; }

# The tenant name reaches a directory name, a schema name and a port block, so
# the shape is bounded here.
case "$TENANT" in
  *[!a-z0-9-]*|'' ) fail "the tenant name $TENANT carries a character outside a to z, 0 to 9 and the hyphen. Choose a name from those characters and run this script again." ;;
esac
[ "${#TENANT}" -le 24 ] || fail "the tenant name $TENANT holds more than 24 characters. Choose a shorter name and run this script again."

# The local base holds every tenant world. A machine whose repository sits on a
# slow mount points this value at a fast path.
LOCAL_BASE="${CERULEA_LOCAL_BASE:-$ROOT/local}"
TENANTS="$LOCAL_BASE/tenants"
HOME_DIR="$TENANTS/$TENANT"
RUN="$HOME_DIR/run"
DATA="$HOME_DIR/chain"
PLATFORM="$HOME_DIR/platform"
REGISTRY="$TENANTS/registry.txt"
SCHEMA="tenant_$(printf '%s' "$TENANT" | tr '-' '_')"
mkdir -p "$RUN" "$DATA" "$PLATFORM" || fail "the tenant directory under $HOME_DIR stayed absent. Check the write permission on $ROOT/local and run this script again."

# The port block. A tenant keeps the same block across restarts, so the
# registry records the index the tenant first received.
tenant_index() {
  touch "$REGISTRY"
  local known
  known="$(awk -v name="$TENANT" '$1 == name { print $2 }' "$REGISTRY" | head -1)"
  if [ -n "$known" ]; then
    printf '%s' "$known"
    return 0
  fi
  local next=0
  while awk -v slot="$next" '$2 == slot { found = 1 } END { exit found ? 0 : 1 }' "$REGISTRY"; do
    next=$((next + 1))
  done
  printf '%s\t%s\n' "$TENANT" "$next" >> "$REGISTRY"
  printf '%s' "$next"
}

INDEX="$(tenant_index)"
OFFSET=$((INDEX * 100))
P2P_ONE=$((30333 + OFFSET)); P2P_TWO=$((30334 + OFFSET)); P2P_THREE=$((30335 + OFFSET))
RPC_ONE=$((9944 + OFFSET));  RPC_TWO=$((9945 + OFFSET));  RPC_THREE=$((9946 + OFFSET))
API_PORT=$((4000 + OFFSET))
STUDIO_PORT=$((3000 + OFFSET))
DASHBOARD_PORT=$((3001 + OFFSET))
EXPLORER_PORT=$((3002 + OFFSET))
INFERENCE_PORT=$((8080 + OFFSET))

say "tenant $TENANT holds index $INDEX, port offset $OFFSET, schema $SCHEMA and data path $HOME_DIR"

# Start one process in its own session, record its pid and its process group,
# and confirm it is alive. The separate session is the process boundary between
# tenants. A stop of one tenant reaches that group alone.
start() {
  local name="$1"; shift
  if [ -f "$RUN/$name.pid" ] && kill -0 "$(cat "$RUN/$name.pid")" 2>/dev/null; then
    say "$name already running with pid $(cat "$RUN/$name.pid")"
    return 0
  fi
  setsid nohup "$@" > "$RUN/$name.log" 2>&1 &
  local pid=$!
  echo "$pid" > "$RUN/$name.pid"
  sleep 1
  kill -0 "$pid" 2>/dev/null || fail "$name exited at once. Read $RUN/$name.log for the reason."
  ps -o pgid= -p "$pid" 2>/dev/null | tr -d ' ' > "$RUN/$name.pgid"
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

# 1. The platform database, shared by every tenant.
if command -v pg_isready >/dev/null 2>&1 && ! pg_isready -q -h "$DB_HOST" -p "$DB_PORT"; then
  if command -v service >/dev/null 2>&1; then
    service postgresql start >/dev/null 2>&1 || sudo service postgresql start
  fi
fi
wait_port "$DB_PORT" 30 || fail "PostgreSQL answers on no port $DB_PORT. Start the database and run this script again."
say "PostgreSQL answers on $DB_PORT"

# 2. The tenant schema. Each tenant reads and writes one schema alone, so a
# query in one tenant reaches no row of another.
DATABASE_URL="postgres://$DB_USER@$DB_HOST:$DB_PORT/$DB_NAME?options=-c%20search_path%3D$SCHEMA"
if command -v psql >/dev/null 2>&1; then
  psql -w -h "$DB_HOST" -p "$DB_PORT" -U "$DB_USER" -d "$DB_NAME" \
    -c "CREATE SCHEMA IF NOT EXISTS $SCHEMA" >/dev/null 2>&1 \
    || fail "the schema $SCHEMA stayed absent in $DB_NAME. Grant the create right on $DB_NAME to $DB_USER and run this script again."
  say "schema $SCHEMA is present in $DB_NAME"
fi
if [ -f "$APIS_HOME/dist/db/migrate-entry.js" ]; then
  ( cd "$APIS_HOME" && DATABASE_URL="$DATABASE_URL" CERULEA_MODE=local \
      CERULEA_LOCAL_ROOT="$PLATFORM" CERULEA_PUBLIC_URL="http://127.0.0.1:$API_PORT" \
      node dist/db/migrate-entry.js ) > "$RUN/migrate.log" 2>&1 \
    && say "every migration is applied inside $SCHEMA" \
    || fail "the migration run inside $SCHEMA stopped. Read $RUN/migrate.log, then run this script again."
elif [ -d "$APIS_HOME" ]; then
  say "Cerulea APIs holds no build at $APIS_HOME/dist. Run pnpm build inside apis, then run this script again."
fi

# 3. Three validator processes. Alice, Bob and Charlie form the tenant network.
[ -x "$NODE_BIN" ] || fail "node binary missing at $NODE_BIN. Run the release build first."
if [ ! -f "$DATA/node-key-one" ]; then
  "$NODE_BIN" key generate-node-key --file "$DATA/node-key-one" > "$DATA/peer-one.txt" 2>&1 \
    || fail "the node key for tenant $TENANT stayed absent. Check the write permission on $DATA and run this script again."
fi
"$NODE_BIN" key inspect-node-key --file "$DATA/node-key-one" > "$DATA/peer-one.txt" 2>&1
PEER_ONE="$(grep -oE '12D3Koo[A-Za-z0-9]+' "$DATA/peer-one.txt" | head -1)"
[ -n "$PEER_ONE" ] || fail "the peer name of the first validator stayed empty. Remove $DATA/node-key-one and $DATA/peer-one.txt, then run this script again."

# Local peer discovery stays off. Two tenants on one machine share a genesis, so
# a discovery broadcast would join their networks. Each validator reaches the
# tenant bootnode by address alone.
BASE_ARGS=(--chain local --enable-cerulea-extensions --rpc-cors all --rpc-methods unsafe --no-telemetry --no-mdns)
start alice   "$NODE_BIN" "${BASE_ARGS[@]}" --alice   --base-path "$DATA/alice"   --port "$P2P_ONE"   --rpc-port "$RPC_ONE"   --node-key-file "$DATA/node-key-one"
BOOT="/ip4/127.0.0.1/tcp/$P2P_ONE/p2p/$PEER_ONE"
start bob     "$NODE_BIN" "${BASE_ARGS[@]}" --bob     --base-path "$DATA/bob"     --port "$P2P_TWO"   --rpc-port "$RPC_TWO"   --bootnodes "$BOOT"
start charlie "$NODE_BIN" "${BASE_ARGS[@]}" --charlie --base-path "$DATA/charlie" --port "$P2P_THREE" --rpc-port "$RPC_THREE" --bootnodes "$BOOT"
wait_port "$RPC_ONE" 60 || fail "the first validator answers on no port. Read $RUN/alice.log."
say "three validators started on rpc ports $RPC_ONE, $RPC_TWO and $RPC_THREE"

# 4. Cerulea APIs. The service runs from source, so a tenant starts the code the
# repository holds now.
if [ -f "$APIS_HOME/dist/main.js" ]; then
  ( cd "$APIS_HOME" && \
    DATABASE_URL="$DATABASE_URL" \
    CERULEA_LOCAL_ROOT="$PLATFORM" \
    CERULEA_HTTP_HOST=127.0.0.1 \
    CERULEA_HTTP_PORT="$API_PORT" \
    CERULEA_PUBLIC_URL="http://127.0.0.1:$API_PORT" \
    CERULEA_CHAIN_WEBSOCKET="ws://127.0.0.1:$RPC_ONE" \
    CERULEA_INFERENCE_ENDPOINT="http://127.0.0.1:$INFERENCE_PORT" \
    start apis node dist/main.js )
  wait_port "$API_PORT" "${CERULEA_API_START_LIMIT:-300}" \
    || say "Cerulea APIs answers on no port $API_PORT yet. Read $RUN/apis.log."
elif [ -d "$APIS_HOME" ]; then
  say "Cerulea APIs holds no build at $APIS_HOME/dist/main.js. Run pnpm build inside apis, then run this script again."
fi

# 5. Studio, Dashboard, Explorer. Each app starts once its build is present.
start_app() {
  local app="$1" port="$2"
  [ -d "$ROOT/$app" ] || return 0
  local home="$ROOT/$app"
  [ -d "$home/apps/frontend" ] && home="$home/apps/frontend"
  local runner="$home/node_modules/next/dist/bin/next"
  [ -f "$runner" ] || runner="$ROOT/$app/node_modules/next/dist/bin/next"
  if [ ! -d "$home/.next" ]; then
    say "$app holds no build at $home/.next. Run pnpm build inside $app, then run this script again."
    return 0
  fi
  if [ ! -f "$runner" ]; then
    say "$app holds no frontend runner. Run pnpm install inside $app, then run this script again."
    return 0
  fi
  ( cd "$home" && \
    NEXT_PUBLIC_CERULEA_API="http://127.0.0.1:$API_PORT" \
    start "$app" node "$runner" start --port "$port" --hostname 127.0.0.1 )
}
start_app studio "$STUDIO_PORT"
start_app dashboard "$DASHBOARD_PORT"
start_app explorer "$EXPLORER_PORT"

# 6. The inference server. Resource discipline: a running Rust build blocks it.
if pgrep -f 'cargo (build|test|check|clippy)' >/dev/null 2>&1; then
  say "a Rust build is running, so the inference server stays stopped"
elif [ -x "$LLAMA" ] && [ -f "$MODEL" ]; then
  start inference "$LLAMA" -m "$MODEL" --port "$INFERENCE_PORT" --host 127.0.0.1
else
  say "inference server or model missing. Cerulea AI stays stopped until both exist."
fi

# The tenant record. Another script reads this file rather than guessing.
cat > "$HOME_DIR/tenant.env" <<RECORD
CERULEA_TENANT=$TENANT
CERULEA_TENANT_INDEX=$INDEX
CERULEA_TENANT_HOME=$HOME_DIR
CERULEA_TENANT_SCHEMA=$SCHEMA
CERULEA_LOCAL_ROOT=$PLATFORM
CERULEA_CHAIN_DATA=$DATA
DATABASE_URL=$DATABASE_URL
CERULEA_RPC_PORTS=$RPC_ONE,$RPC_TWO,$RPC_THREE
CERULEA_P2P_PORTS=$P2P_ONE,$P2P_TWO,$P2P_THREE
CERULEA_API_PORT=$API_PORT
CERULEA_STUDIO_PORT=$STUDIO_PORT
CERULEA_DASHBOARD_PORT=$DASHBOARD_PORT
CERULEA_EXPLORER_PORT=$EXPLORER_PORT
CERULEA_INFERENCE_PORT=$INFERENCE_PORT
CERULEA_PEER_ONE=$PEER_ONE
RECORD

say "Cerulea local mode is up for tenant $TENANT. Stop it with scripts/dev-down.sh $TENANT."
