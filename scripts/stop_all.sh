#!/bin/bash
# Stop all running cbc-node processes.

echo "Stopping all CBC Chain nodes..."

if pkill -f "cbc-node"; then
    echo "Sent SIGTERM to all cbc-node processes."
else
    echo "No cbc-node processes found."
    exit 0
fi

# Give nodes a moment for graceful shutdown
sleep 2

# Force-kill anything still alive
if pkill -9 -f "cbc-node" 2>/dev/null; then
    echo "Force-killed remaining cbc-node processes."
fi

echo "Done."
