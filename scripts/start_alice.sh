#!/bin/bash
# Start Alice — bootnode and primary RPC endpoint.
# No manual key generation or directory setup needed; the node handles everything automatically.

set -e

BINARY="${BINARY:-./target/release/cbc-node}"
BASE_PATH="${BASE_PATH:-$HOME/.local/share/cbc-node/alice}"
LOG_FILE="${LOG_FILE:-node_output_alice.log}"

if [ ! -f "$BINARY" ]; then
    echo "ERROR: Binary not found at $BINARY. Run: cargo build --release"
    exit 1
fi

echo "Starting Alice (bootnode)..."
echo "  Data dir  : $BASE_PATH"
echo "  RPC port  : 9944"
echo "  Log file  : $LOG_FILE"

exec "$BINARY" \
    --base-path "$BASE_PATH" \
    --chain local \
    --alice \
    --port 30333 \
    --rpc-port 9944 \
    --prometheus-port 9615 \
    --unsafe-rpc-external \
    --rpc-cors all \
    --validator \
    --name Alice \
    >> "$LOG_FILE" 2>&1
