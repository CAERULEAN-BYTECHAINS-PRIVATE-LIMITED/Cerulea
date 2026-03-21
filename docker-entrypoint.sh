#!/bin/bash
set -e

# NODE_ROLE must be: alice | bob | charlie
NODE_ROLE="${NODE_ROLE:-alice}"

BASE_PATH="/data"
NETWORK_KEY_PATH="$BASE_PATH/chains/cbc_local/network/secret_ed25519"
P2P_PORT="${P2P_PORT:-30333}"
RPC_PORT="${RPC_PORT:-9944}"
PROMETHEUS_PORT="${PROMETHEUS_PORT:-9615}"

# Alice's network key is baked into the image at this path.
# Bob and Charlie derive Alice's peer-id from it — no manual config needed.
ALICE_BAKED_KEY="/etc/cbc/alice_network_key"

# ── Alice: install her fixed key so her peer-id is always the same ──────────
if [ "$NODE_ROLE" = "alice" ]; then
    if [ ! -f "$NETWORK_KEY_PATH" ]; then
        echo "Installing Alice's fixed network key..."
        mkdir -p "$(dirname "$NETWORK_KEY_PATH")"
        cp "$ALICE_BAKED_KEY" "$NETWORK_KEY_PATH"
        chmod 600 "$NETWORK_KEY_PATH"
    fi
fi

# ── Bob / Charlie: generate their own key if missing ────────────────────────
if [ "$NODE_ROLE" != "alice" ] && [ ! -f "$NETWORK_KEY_PATH" ]; then
    echo "Generating network key for $NODE_ROLE..."
    mkdir -p "$(dirname "$NETWORK_KEY_PATH")"
    cbc-node key generate-node-key --file "$NETWORK_KEY_PATH" 2>/dev/null
fi

# ── Derive Alice's peer-id from the baked key (same trick as start_bob.sh) ──
ALICE_PEER_ID=$(cbc-node key inspect-node-key --file "$ALICE_BAKED_KEY" 2>/dev/null | tail -n 1)

# Alice's internal Render hostname — matches the service name in render.yaml
ALICE_HOST="${ALICE_HOST:-cbc-alice}"
ALICE_BOOTNODE="/dns/${ALICE_HOST}/tcp/30333/p2p/${ALICE_PEER_ID}"

echo "======================================================"
echo "  NODE_ROLE      : $NODE_ROLE"
echo "  ALICE_PEER_ID  : $ALICE_PEER_ID"
echo "  ALICE_BOOTNODE : $ALICE_BOOTNODE"
echo "======================================================"

# Base args shared by all nodes
BASE_ARGS=(
    --base-path "$BASE_PATH"
    --chain local
    --port "$P2P_PORT"
    --rpc-port "$RPC_PORT"
    --prometheus-port "$PROMETHEUS_PORT"
    --validator
    --lifecycle-trace
    --lifecycle-trace-format human-readable
)

case "$NODE_ROLE" in
    alice)
        echo "Starting Alice (bootnode + public RPC)..."
        # --unsafe-rpc-external  : bind RPC to 0.0.0.0 so Render can reach it
        # --rpc-cors all         : allow any frontend origin (lock this down in prod)
        # --rpc-methods unsafe   : expose all RPC methods (use Safe in prod)
        exec cbc-node \
            "${BASE_ARGS[@]}" \
            --unsafe-rpc-external \
            --rpc-cors all \
            --rpc-methods unsafe \
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
