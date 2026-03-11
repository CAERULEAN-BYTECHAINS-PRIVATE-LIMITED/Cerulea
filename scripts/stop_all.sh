#!/bin/bash

# Script to stop all running CBC Chain nodes

echo "Stopping all CBC Chain nodes..."

# Find and kill all cbc-node processes
pkill -f "cbc-node.*--alice" && echo "Stopped Alice"
pkill -f "cbc-node.*--bob" && echo "Stopped Bob"
pkill -f "cbc-node.*--charlie" && echo "Stopped Charlie"

# Wait a moment for graceful shutdown
sleep 2

# Force kill if still running
pkill -9 -f "cbc-node" 2>/dev/null && echo "Force stopped remaining nodes"

echo "All nodes stopped."
