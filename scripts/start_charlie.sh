#!/bin/bash

# Wait a moment to ensure Alice has started and generated her peer ID
sleep 2

BASE_PATH="/tmp/charlie"
NETWORK_KEY_PATH="$BASE_PATH/chains/cbc_local/network/secret_ed25519"

# Clean up old data for fresh start (optional - comment out if you want to keep data)
# rm -rf "$BASE_PATH"

# Generate network key if it doesn't exist
if [ ! -f "$NETWORK_KEY_PATH" ]; then
    echo "Generating network key for Charlie..."
    mkdir -p "$(dirname "$NETWORK_KEY_PATH")"
    ./target/release/cbc-node key generate-node-key --file "$NETWORK_KEY_PATH" > /dev/null 2>&1
    echo "Network key generated"
fi

# Extract Alice's peer ID dynamically (as she is the bootnode)
ALICE_PEER_ID=""
MAX_RETRIES=10
RETRY_COUNT=0

while [ -z "$ALICE_PEER_ID" ] && [ $RETRY_COUNT -lt $MAX_RETRIES ]; do
    if [ -f /tmp/alice/chains/cbc_local/network/secret_ed25519 ]; then
        ALICE_PEER_ID=$(./target/release/cbc-node key inspect-node-key --file /tmp/alice/chains/cbc_local/network/secret_ed25519 2>/dev/null | tail -n 1)
    fi
    
    if [ -z "$ALICE_PEER_ID" ]; then
        echo "Waiting for Alice to generate network key... (attempt $((RETRY_COUNT+1))/$MAX_RETRIES)"
        sleep 2
        RETRY_COUNT=$((RETRY_COUNT+1))
    fi
done

if [ -z "$ALICE_PEER_ID" ]; then
    echo "Could not extract Alice's peer ID after $MAX_RETRIES attempts. Please ensure Alice is running."
    exit 1
fi

echo "Connecting Charlie to Alice -> /ip4/127.0.0.1/tcp/30333/p2p/$ALICE_PEER_ID"

./target/release/cbc-node \
  --base-path "$BASE_PATH" \
  --chain local \
  --charlie \
  --port 30335 \
  --rpc-port 9946 \
  --prometheus-port 9617 \
  --unsafe-rpc-external \
  --rpc-cors all \
  --validator \
  --name Charlie \
  --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/$ALICE_PEER_ID \
  --lifecycle-trace \
  --lifecycle-trace-format human-readable \
  > node_output_charlie.log 2>&1
