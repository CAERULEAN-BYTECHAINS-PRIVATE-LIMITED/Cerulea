#!/bin/bash
set -e

# NODE_ROLE must be: alice | bob | charlie
NODE_ROLE="${NODE_ROLE:-alice}"

BASE_PATH="/data"
NETWORK_KEY_PATH="$BASE_PATH/chains/cbc_local/network/secret_ed25519"
P2P_PORT="${P2P_PORT:-30333}"
RPC_PORT="${RPC_PORT:-9944}"
PROMETHEUS_PORT="${PROMETHEUS_PORT:-9615}"

# Generate a deterministic network key if one doesn't exist
if [ ! -f "$NETWORK_KEY_PATH" ]; then
    echo "Generating network key for $NODE_ROLE..."
    mkdir -p "$(dirname "$NETWORK_KEY_PATH")"
    cbc-node key generate-node-key --file "$NETWORK_KEY_PATH" 2>/dev/null
    echo "Network key generated."
fi

# Base args shared by all nodes
BASE_ARGS=(
    --base-path "$BASE_PATH"
    --chain local
    --port "$P2P_PORT"
    --rpc-port "$RPC_PORT"
    --prometheus-port "$PROMETHEUS_PORT"
    --unsafe-rpc-external
    --rpc-cors all
    --validator
    --lifecycle-trace
    --lifecycle-trace-format human-readable
)

# Role-specific args
case "$NODE_ROLE" in
    alice)
        echo "Starting Alice (bootnode)..."
        exec cbc-node \
            "${BASE_ARGS[@]}" \
            --alice \
            --name Alice
        ;;

    bob)
        # ALICE_BOOTNODE must be set: /dns/alice-host/tcp/30333/p2p/<peer-id>
        if [ -z "$ALICE_BOOTNODE" ]; then
            echo "ERROR: ALICE_BOOTNODE env var is required for Bob"
            exit 1
        fi
        echo "Starting Bob, bootnode: $ALICE_BOOTNODE"
        exec cbc-node \
            "${BASE_ARGS[@]}" \
            --bob \
            --name Bob \
            --bootnodes "$ALICE_BOOTNODE"
        ;;

    charlie)
        if [ -z "$ALICE_BOOTNODE" ]; then
            echo "ERROR: ALICE_BOOTNODE env var is required for Charlie"
            exit 1
        fi
        echo "Starting Charlie, bootnode: $ALICE_BOOTNODE"
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
