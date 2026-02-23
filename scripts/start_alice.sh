#!/bin/bash

BASE_PATH="/tmp/alice"
NETWORK_KEY="$BASE_PATH/chains/cbc_local/network/secret_ed25519"

# Ensure keys exist or generate them
if [ ! -f "$NETWORK_KEY" ]; then
    echo "Alice's network key not found. Generating..."
    mkdir -p "$(dirname "$NETWORK_KEY")"
    ./target/release/cbc-node key generate-node-key --file "$NETWORK_KEY" > /dev/null
fi

./target/release/cbc-node \
  --base-path "$BASE_PATH" \
  --chain local \
  --alice \
  --port 30333 \
  --rpc-port 9944 \
  --unsafe-rpc-external \
  --rpc-cors all \
  --validator \
  --name Alice \
  --lifecycle-trace \
  --lifecycle-trace-format human-readable \
  > node_output_alice.log 2>&1
