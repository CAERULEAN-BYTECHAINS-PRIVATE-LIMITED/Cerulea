#!/bin/bash
set -e

# NODE_ROLE must be explicitly set to: alice | bob | charlie
# No default — fail loudly rather than silently start a second Alice.
NODE_ROLE="${NODE_ROLE:?ERROR: NODE_ROLE must be set to alice, bob, or charlie}"

BASE_PATH="/data"
NETWORK_KEY_PATH="$BASE_PATH/chains/cbc_local/network/secret_ed25519"
P2P_PORT="${P2P_PORT:-30333}"
# Render sets PORT to tell its proxy which port to forward to.
# We use it as the RPC port so both are in sync.
RPC_PORT="${PORT:-${RPC_PORT:-9944}}"
PROMETHEUS_PORT="${PROMETHEUS_PORT:-9615}"

# Alice's network key — determines her peer-id, which Bob and Charlie dial.
#
# Three sources, in order of preference:
#   1. ALICE_NODE_KEY   — 64 hex chars in the environment. THIS is what a hosted
#                         deployment uses: keys/ is gitignored (correctly — a key does
#                         not belong in git), so on Render there is no baked key and
#                         Alice would otherwise generate a RANDOM identity on every
#                         boot. Bob and Charlie dial a peer-id derived from this same
#                         value, so a random one silently breaks the network: Alice
#                         comes up alone, the other two never find her, and the
#                         2-of-3 quorum claim quietly becomes a 1-of-1 chain.
#   2. the baked key    — present only when keys/ existed at image build time.
#   3. generated        — single-node local dev, where nothing dials Alice anyway.
#
# ALICE_PEER_ID may also be set directly, which skips derivation entirely; useful when
# the bootnode is not managed by this compose/blueprint at all.
ALICE_BAKED_KEY="/etc/cbc/alice_network_key"

if [ -n "${ALICE_NODE_KEY:-}" ] && [ ! -f "$ALICE_BAKED_KEY" ]; then
    mkdir -p "$(dirname "$ALICE_BAKED_KEY")"
    printf '%s' "$ALICE_NODE_KEY" > "$ALICE_BAKED_KEY"
    chmod 600 "$ALICE_BAKED_KEY"
    echo "Installed Alice's network key from ALICE_NODE_KEY."
fi

# ── Alice: install her fixed key so her peer-id is always the same ──────────
if [ "$NODE_ROLE" = "alice" ]; then
    if [ ! -f "$NETWORK_KEY_PATH" ]; then
        mkdir -p "$(dirname "$NETWORK_KEY_PATH")"
        if [ -f "$ALICE_BAKED_KEY" ]; then
            echo "Installing Alice's fixed network key..."
            cp "$ALICE_BAKED_KEY" "$NETWORK_KEY_PATH"
        else
            echo "No baked key found — generating Alice's network key..."
            cerulea-node key generate-node-key --file "$NETWORK_KEY_PATH" 2>/dev/null
        fi
        chmod 600 "$NETWORK_KEY_PATH"
    fi
fi

# ── Bob / Charlie: generate their own key if missing ────────────────────────
if [ "$NODE_ROLE" != "alice" ] && [ ! -f "$NETWORK_KEY_PATH" ]; then
    echo "Generating network key for $NODE_ROLE..."
    mkdir -p "$(dirname "$NETWORK_KEY_PATH")"
    cerulea-node key generate-node-key --file "$NETWORK_KEY_PATH" 2>/dev/null
fi

# ── Derive Alice's peer-id ───────────────────────────────────────────────────
# Prefer the baked key (always available); fall back to Alice's live key
# (useful in docker-compose where all nodes share a network volume).
if [ -n "${ALICE_PEER_ID:-}" ]; then
    echo "Using ALICE_PEER_ID supplied by the environment."
elif [ -f "$ALICE_BAKED_KEY" ]; then
    ALICE_PEER_ID=$(cerulea-node key inspect-node-key --file "$ALICE_BAKED_KEY" 2>/dev/null | tail -n 1)
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
    ALICE_PEER_ID=$(cerulea-node key inspect-node-key --file "$ALICE_LIVE_KEY" 2>/dev/null | tail -n 1)
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
#
# Listen on BOTH IPv4 and IPv6.
#
# Railway's private network (`<service>.railway.internal`) is IPv6-only, so a node
# bound to 0.0.0.0 alone is unreachable from its peers there and the validators never
# form a network -- the same silent single-validator degradation the ALICE_NODE_KEY
# problem causes, from a different direction. Binding both is harmless on Render,
# docker-compose and locally, so it is unconditional rather than platform-gated.
# The bootnode address uses /dns/ (above), which resolves A and AAAA alike.
BASE_ARGS=(
    --base-path "$BASE_PATH"
    --chain local
    --port "$P2P_PORT"
    --listen-addr "/ip4/0.0.0.0/tcp/$P2P_PORT"
    --listen-addr "/ip6/::/tcp/$P2P_PORT"
    --rpc-port "$RPC_PORT"
    --prometheus-port "$PROMETHEUS_PORT"
    --unsafe-rpc-external
    --rpc-cors all
    --validator
    # Keep ALL state and ALL blocks. This is an audit chain: the whole premise is that a
    # decision's block can be looked up and its state re-derived long after the fact, so
    # discarding historical state defeats the point. It also stops "State already
    # discarded" errors when a reader queries a block the default pruning window has
    # already dropped -- which on a sub-second chain is only a few minutes of history.
    --state-pruning archive
    --blocks-pruning archive
)

case "$NODE_ROLE" in
    alice)
        echo "Starting Alice (bootnode + public RPC)..."
        exec cerulea-node \
            "${BASE_ARGS[@]}" \
            --alice \
            --name Alice
        ;;

    bob)
        echo "Starting Bob -> $ALICE_BOOTNODE"
        exec cerulea-node \
            "${BASE_ARGS[@]}" \
            --bob \
            --name Bob \
            --bootnodes "$ALICE_BOOTNODE"
        ;;

    charlie)
        echo "Starting Charlie -> $ALICE_BOOTNODE"
        exec cerulea-node \
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
