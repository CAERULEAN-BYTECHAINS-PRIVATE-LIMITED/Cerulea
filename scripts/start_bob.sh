#!/bin/bash
# Start Bob — connects to Alice as bootnode.
# No manual key generation or directory setup needed; the node handles everything automatically.

set -e

BINARY="${BINARY:-./target/release/cbc-node}"
BASE_PATH="${BASE_PATH:-$HOME/.local/share/cbc-node/bob}"
LOG_FILE="${LOG_FILE:-node_output_bob.log}"
ALICE_BASE_PATH="${ALICE_BASE_PATH:-$HOME/.local/share/cbc-node/alice}"

if [ ! -f "$BINARY" ]; then
    echo "ERROR: Binary not found at $BINARY. Run: cargo build --release"
    exit 1
fi

# Derive Alice's peer ID from her network key (written by the node on first start).
ALICE_NET_KEY="$ALICE_BASE_PATH/chains/cbc_local/network/secret_ed25519"
MAX_RETRIES=15
RETRY=0

echo "Waiting for Alice's network key at $ALICE_NET_KEY..."
while [ ! -f "$ALICE_NET_KEY" ] && [ $RETRY -lt $MAX_RETRIES ]; do
    sleep 2
    RETRY=$((RETRY + 1))
    echo "  ...attempt $RETRY/$MAX_RETRIES"
done

if [ ! -f "$ALICE_NET_KEY" ]; then
    echo "ERROR: Alice's network key not found after ${MAX_RETRIES} attempts."
    echo "Make sure Alice is running: bash scripts/start_alice.sh"
    exit 1
fi

ALICE_PEER_ID=$("$BINARY" key inspect-node-key --file "$ALICE_NET_KEY" 2>/dev/null | tail -n 1)
if [ -z "$ALICE_PEER_ID" ]; then
    echo "ERROR: Could not derive Alice's peer ID from $ALICE_NET_KEY"
    exit 1
fi

echo "Starting Bob..."
echo "  Data dir   : $BASE_PATH"
echo "  RPC port   : 9945"
echo "  Bootnode   : /ip4/127.0.0.1/tcp/30333/p2p/$ALICE_PEER_ID"
echo "  Log file   : $LOG_FILE"

exec "$BINARY" \
    --base-path "$BASE_PATH" \
    --chain local \
    --bob \
    --port 30334 \
    --rpc-port 9945 \
    --prometheus-port 9616 \
    --unsafe-rpc-external \
    --rpc-cors all \
    --validator \
    --name Bob \
    --bootnodes "/ip4/127.0.0.1/tcp/30333/p2p/$ALICE_PEER_ID" \
    >> "$LOG_FILE" 2>&1
