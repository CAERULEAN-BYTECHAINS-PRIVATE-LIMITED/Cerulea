#!/bin/bash

# Ensure Alice's keys exist for extraction
if [ ! -f /tmp/alice/chains/cbc_local/network/secret_ed25519 ]; then
    echo "Alice's keys not found. Please start Alice first."
    exit 1
fi

BASE_PATH="/tmp/bob"
NETWORK_KEY="$BASE_PATH/chains/cbc_local/network/secret_ed25519"

# Ensure own keys exist or generate them
if [ ! -f "$NETWORK_KEY" ]; then
    echo "Bob's network key not found. Generating..."
    mkdir -p "$(dirname "$NETWORK_KEY")"
    ./target/release/cbc-node key generate-node-key --file "$NETWORK_KEY" > /dev/null
fi

ALICE_PEER_ID=$(./target/release/cbc-node key inspect-node-key --file /tmp/alice/chains/cbc_local/network/secret_ed25519 | tail -n 1)

if [ -z "$ALICE_PEER_ID" ]; then
    echo "Alice's peer ID could not be dynamically extracted."
    exit 1
fi

echo "Connecting Bob to Alice -> /ip4/127.0.0.1/tcp/30333/p2p/$ALICE_PEER_ID"

./target/release/cbc-node \
  --base-path "$BASE_PATH" \
  --chain local \
  --bob \
  --port 30334 \
  --rpc-port 9945 \
  --unsafe-rpc-external \
  --rpc-cors all \
  --validator \
  --name Bob \
  --bootnodes /ip4/127.0.0.1/tcp/30333/p2p/$ALICE_PEER_ID
