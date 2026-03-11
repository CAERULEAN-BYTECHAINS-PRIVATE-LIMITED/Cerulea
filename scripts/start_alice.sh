#!/bin/bash

BASE_PATH="/tmp/alice"
NETWORK_KEY_PATH="$BASE_PATH/chains/cbc_local/network/secret_ed25519"

# Clean up old data for fresh start (optional - comment out if you want to keep data)
# rm -rf "$BASE_PATH"

# Generate network key if it doesn't exist
if [ ! -f "$NETWORK_KEY_PATH" ]; then
    echo "Generating network key for Alice..."
    mkdir -p "$(dirname "$NETWORK_KEY_PATH")"
    ./target/release/cbc-node key generate-node-key --file "$NETWORK_KEY_PATH" > /dev/null 2>&1
    echo "Network key generated"
fi

./target/release/cbc-node \
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
  --lifecycle-trace \
  --lifecycle-trace-format human-readable \
  > node_output_alice.log 2>&1
