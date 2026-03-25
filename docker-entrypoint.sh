#!/bin/bash
set -e

# NODE_ROLE must be: alice | bob | charlie
NODE_ROLE="${NODE_ROLE:-alice}"

BASE_PATH="/data"
NETWORK_KEY_PATH="$BASE_PATH/chains/cbc_local/network/secret_ed25519"
P2P_PORT="${P2P_PORT:-30333}"
# Render sets PORT to tell its proxy which port to forward to.
# We use it as the RPC port so both are in sync.
RPC_PORT="${PORT:-${RPC_PORT:-9944}}"
PROMETHEUS_PORT="${PROMETHEUS_PORT:-9615}"

# Alice's network key — baked into the image for a deterministic peer-id.
# If not baked (e.g. local dev without the keys/ dir), one is generated.
ALICE_BAKED_KEY="/etc/cbc/alice_network_key"

# ── Alice: install her fixed key so her peer-id is always the same ──────────
if [ "$NODE_ROLE" = "alice" ]; then
    if [ ! -f "$NETWORK_KEY_PATH" ]; then
        mkdir -p "$(dirname "$NETWORK_KEY_PATH")"
        if [ -f "$ALICE_BAKED_KEY" ]; then
            echo "Installing Alice's fixed network key..."
            cp "$ALICE_BAKED_KEY" "$NETWORK_KEY_PATH"
        else
            echo "No baked key found — generating Alice's network key..."
            cbc-node key generate-node-key --file "$NETWORK_KEY_PATH" 2>/dev/null
        fi
        chmod 600 "$NETWORK_KEY_PATH"
    fi
fi

# ── Bob / Charlie: generate their own key if missing ────────────────────────
if [ "$NODE_ROLE" != "alice" ] && [ ! -f "$NETWORK_KEY_PATH" ]; then
    echo "Generating network key for $NODE_ROLE..."
    mkdir -p "$(dirname "$NETWORK_KEY_PATH")"
    cbc-node key generate-node-key --file "$NETWORK_KEY_PATH" 2>/dev/null
fi

# ── Derive Alice's peer-id ───────────────────────────────────────────────────
# Prefer the baked key (always available); fall back to Alice's live key
# (useful in docker-compose where all nodes share a network volume).
if [ -f "$ALICE_BAKED_KEY" ]; then
    ALICE_PEER_ID=$(cbc-node key inspect-node-key --file "$ALICE_BAKED_KEY" 2>/dev/null | tail -n 1)
else
    # docker-compose local mode: wait for Alice's key to appear on the shared volume
    ALICE_LIVE_KEY="${ALICE_DATA_PATH:-/data-alice}/chains/cbc_local/network/secret_ed25519"
    MAX_RETRIES=15
    RETRY=0
    echo "Waiting for Alice's network key at $ALICE_LIVE_KEY..."
    while [ ! -f "$ALICE_LIVE_KEY" ] && [ $RETRY -lt $MAX_RETRIES ]; do
        sleep 2
        RETRY=$((RETRY + 1))
        echo "  ...attempt $RETRY/$MAX_RETRIES"
    done
    if [ ! -f "$ALICE_LIVE_KEY" ]; then
        echo "ERROR: Alice's network key not found after $MAX_RETRIES attempts."
        exit 1
    fi
    ALICE_PEER_ID=$(cbc-node key inspect-node-key --file "$ALICE_LIVE_KEY" 2>/dev/null | tail -n 1)
fi

if [ -z "$ALICE_PEER_ID" ]; then
    echo "ERROR: Could not derive Alice's peer ID."
    exit 1
fi

# Alice's hostname — matches the Render service name or docker-compose service name
ALICE_HOST="${ALICE_HOST:-cbc-alice}"
ALICE_BOOTNODE="/dns/${ALICE_HOST}/tcp/30333/p2p/${ALICE_PEER_ID}"

echo "======================================================"
echo "  NODE_ROLE      : $NODE_ROLE"
echo "  BASE_PATH      : $BASE_PATH"
echo "  RPC_PORT       : $RPC_PORT"
echo "  P2P_PORT       : $P2P_PORT"
echo "  ALICE_PEER_ID  : $ALICE_PEER_ID"
echo "  ALICE_BOOTNODE : $ALICE_BOOTNODE"
echo "======================================================"

# Base args shared by all nodes — mirrors the local start_*.sh scripts exactly
BASE_ARGS=(
    --base-path "$BASE_PATH"
    --chain local
    --port "$P2P_PORT"
    --rpc-port "$RPC_PORT"
    --prometheus-port "$PROMETHEUS_PORT"
    --unsafe-rpc-external
    --rpc-cors all
    --validator
)

case "$NODE_ROLE" in
    alice)
        echo "Starting Alice (bootnode + public RPC)..."
        exec cbc-node \
            "${BASE_ARGS[@]}" \
            --alice \
            --name Alice
        ;;

    bob)
        echo "Starting Bob -> $ALICE_BOOTNODE"
        exec cbc-node \
            "${BASE_ARGS[@]}" \
            --bob \
            --name Bob \
            --bootnodes "$ALICE_BOOTNODE"
        ;;

    charlie)
        echo "Starting Charlie -> $ALICE_BOOTNODE"
        exec cbc-node \
            "${BASE_ARGS[@]}" \
            --charlie \
            --name Charlie \
            --bootnodes "$ALICE_BOOTNODE"
        ;;

    *)
        echo "ERROR: Unknown NODE_ROLE '$NODE_ROLE'. Must be alice, bob, or charlie."
        exit 1
        ;;
esac
