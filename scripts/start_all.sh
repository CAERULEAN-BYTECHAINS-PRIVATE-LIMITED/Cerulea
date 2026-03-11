#!/bin/bash

# Script to start all three validator nodes for DVF testing
# This will start Alice, Bob, and Charlie in the background

echo "Starting CBC Chain Multi-Validator Network..."
echo "=============================================="

# Clean up old logs
rm -f node_output_alice.log node_output_bob.log node_output_charlie.log

# Start Alice (bootnode)
echo "Starting Alice (bootnode)..."
bash scripts/start_alice.sh &
ALICE_PID=$!
echo "Alice started with PID: $ALICE_PID"

# Wait for Alice to initialize and generate network key
echo "Waiting for Alice to initialize and generate network key..."
sleep 8

# Start Bob
echo "Starting Bob..."
bash scripts/start_bob.sh &
BOB_PID=$!
echo "Bob started with PID: $BOB_PID"

# Wait a moment
sleep 3

# Start Charlie
echo "Starting Charlie..."
bash scripts/start_charlie.sh &
CHARLIE_PID=$!
echo "Charlie started with PID: $CHARLIE_PID"

echo ""
echo "=============================================="
echo "All nodes started!"
echo "=============================================="
echo "Alice PID:   $ALICE_PID (RPC: 9944, Prometheus: 9615)"
echo "Bob PID:     $BOB_PID (RPC: 9945, Prometheus: 9616)"
echo "Charlie PID: $CHARLIE_PID (RPC: 9946, Prometheus: 9617)"
echo ""
echo "Logs:"
echo "  Alice:   node_output_alice.log"
echo "  Bob:     node_output_bob.log"
echo "  Charlie: node_output_charlie.log"
echo ""
echo "To stop all nodes, run: scripts/stop_all.sh"
echo "To view logs: tail -f node_output_alice.log"
echo ""
echo "Waiting a few seconds for network to stabilize..."
sleep 5
echo "Network should be ready now!"
