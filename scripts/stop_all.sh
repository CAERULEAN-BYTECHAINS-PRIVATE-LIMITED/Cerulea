#!/bin/bash
# Stop all running cerulea-node processes.

echo "Stopping all Cerulea nodes..."

if pkill -f "cerulea-node"; then
    echo "Sent SIGTERM to all cerulea-node processes."
else
    echo "No cerulea-node processes found."
    exit 0
fi

# Give nodes a moment for graceful shutdown
sleep 2

# Force-kill anything still alive
if pkill -9 -f "cerulea-node" 2>/dev/null; then
    echo "Force-killed remaining cerulea-node processes."
fi

echo "Done."
