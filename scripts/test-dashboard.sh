#!/bin/bash

# CBC Dashboard Test Script

set -e

echo "Testing CBC Grafana Dashboard..."

# Check if services are running
if ! docker-compose -f docker-compose.monitoring.yml ps | grep -q "Up"; then
    echo "ERROR: Monitoring stack is not running. Start it first with:"
    echo "   ./scripts/start-monitoring.sh"
    exit 1
fi

echo "Checking service health..."

# Test Prometheus
echo "   Testing Prometheus..."
if curl -s http://localhost:9090/-/healthy > /dev/null; then
    echo "   SUCCESS: Prometheus is healthy"
else
    echo "   ERROR: Prometheus is not responding"
    exit 1
fi

# Test Grafana
echo "   Testing Grafana..."
if curl -s http://localhost:3000/api/health | grep -q "ok"; then
    echo "   SUCCESS: Grafana is healthy"
else
    echo "   ERROR: Grafana is not responding"
    exit 1
fi

# Check if CBC node metrics are available
echo "Checking CBC node metrics..."
if curl -s http://localhost:9615/metrics > /dev/null 2>&1; then
    echo "   SUCCESS: CBC node metrics endpoint is accessible"
    
    # Check for specific CBC metrics
    if curl -s http://localhost:9615/metrics | grep -q "cerulea_"; then
        echo "   SUCCESS: CBC metrics are being exported"
    else
        echo "   WARNING: CBC metrics not found - node might not be running with --prometheus-external"
    fi
else
    echo "   WARNING: CBC node metrics endpoint not accessible"
    echo "      Make sure your CBC node is running with: ./target/release/cerulea-node --dev --prometheus-external"
fi

# Test Prometheus targets
echo "Checking Prometheus targets..."
if curl -s http://localhost:9090/api/v1/targets | grep -q "cerulea-node"; then
    echo "   SUCCESS: CBC node target is configured in Prometheus"
else
    echo "   WARNING: CBC node target not found in Prometheus"
fi

echo ""
echo "Dashboard test complete!"
echo ""
echo "Open your dashboard at: http://localhost:3000"
echo "   Username: admin"
echo "   Password: admin"
echo ""
echo "Look for the 'CBC Consensus Dashboard' in the dashboard list"