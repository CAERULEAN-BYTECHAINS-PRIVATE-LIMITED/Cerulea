# Grafana Dashboard Setup Guide

This guide explains how to set up and test the CBC Grafana dashboard locally.

## Prerequisites

- Docker and Docker Compose installed
- CBC node running with metrics enabled
- Basic understanding of Prometheus and Grafana

## Quick Setup with Docker Compose

### 1. Create Docker Compose Configuration

Create a `docker-compose.monitoring.yml` file in your project root:

```yaml
version: '3.8'

services:
  prometheus:
    image: prom/prometheus:latest
    container_name: cbc-prometheus
    ports:
      - "9090:9090"
    volumes:
      - ./monitoring/prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.console.libraries=/etc/prometheus/console_libraries'
      - '--web.console.templates=/etc/prometheus/consoles'
      - '--storage.tsdb.retention.time=200h'
      - '--web.enable-lifecycle'
    networks:
      - monitoring

  grafana:
    image: grafana/grafana:latest
    container_name: cbc-grafana
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - grafana_data:/var/lib/grafana
      - ./monitoring/grafana/provisioning:/etc/grafana/provisioning
      - ./docs/grafana-dashboard.json:/var/lib/grafana/dashboards/cbc-dashboard.json
    networks:
      - monitoring

volumes:
  prometheus_data:
  grafana_data:

networks:
  monitoring:
    driver: bridge
```

### 2. Create Monitoring Configuration Directory

```bash
mkdir -p monitoring/grafana/provisioning/{dashboards,datasources}
```

### 3. Configure Prometheus

Create `monitoring/prometheus.yml`:

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  - job_name: 'cbc-node'
    static_configs:
      - targets: ['host.docker.internal:9615']  # Default Substrate metrics port
    scrape_interval: 5s
    metrics_path: /metrics
```

### 4. Configure Grafana Datasource

Create `monitoring/grafana/provisioning/datasources/prometheus.yml`:

```yaml
apiVersion: 1

datasources:
  - name: Prometheus
    type: prometheus
    access: proxy
    url: http://prometheus:9090
    isDefault: true
```

### 5. Configure Grafana Dashboard Provisioning

Create `monitoring/grafana/provisioning/dashboards/dashboard.yml`:

```yaml
apiVersion: 1

providers:
  - name: 'CBC Dashboards'
    orgId: 1
    folder: ''
    type: file
    disableDeletion: false
    updateIntervalSeconds: 10
    allowUiUpdates: true
    options:
      path: /var/lib/grafana/dashboards
```

## Running the Setup

### 1. Start Your CBC Node with Metrics

Make sure your CBC node is running with metrics enabled:

```bash
# Build the node first
cargo build --release

# Run with metrics enabled (default port 9615)
./target/release/cbc-node --dev --prometheus-external
```

### 2. Start Monitoring Stack

Using the provided script:
```bash
./start-monitoring.sh
```

Or for a **fresh start** (clears old data/cache):
```bash
./start-monitoring.sh --fresh
```

Manual start:
```bash
# Start Prometheus and Grafana
docker-compose -f docker-compose.monitoring.yml up -d

# Check if services are running
docker-compose -f docker-compose.monitoring.yml ps
```

### 3. Access the Services

- **Grafana**: http://localhost:3000
  - Username: `admin`
  - Password: `admin`
- **Prometheus**: http://localhost:9090

### 4. Import and View Dashboard

1. Open Grafana at http://localhost:3000
2. Login with admin/admin
3. The CBC dashboard should be automatically provisioned
4. Navigate to "Dashboards" → "CBC Consensus Dashboard"

## Manual Dashboard Import (Alternative)

If automatic provisioning doesn't work:

1. In Grafana, click the "+" icon → "Import"
2. Copy the contents of `docs/grafana-dashboard.json`
3. Paste into the import dialog
4. Click "Load"
5. Select "Prometheus" as the datasource
6. Click "Import"

## Troubleshooting

### No Data in Dashboard

1. **Check CBC Node Metrics**:
   ```bash
   curl http://localhost:9615/metrics | grep cbc_
   ```

2. **Check Prometheus Targets**:
   - Go to http://localhost:9090/targets
   - Ensure `cbc-node` target is "UP"

3. **Check Prometheus Queries**:
   - Go to http://localhost:9090
   - Try queries like `cbc_consensus_current_epoch`

### Connection Issues

1. **Docker Network**: Ensure your CBC node is accessible from Docker containers
2. **Firewall**: Check if ports 9615 (metrics) and 3000 (Grafana) are open
3. **Host Access**: On Linux, we use `host.docker.internal` (configured via `extra_hosts` in `docker-compose.monitoring.yml`)
4. **Stale Data**: If you see old chain data after a node restart, use `./start-monitoring.sh --fresh`

### Dashboard Not Loading

1. Check Grafana logs: `docker-compose -f docker-compose.monitoring.yml logs grafana`
2. Verify JSON syntax: `python3 -m json.tool docs/grafana-dashboard.json`
3. Check datasource configuration in Grafana settings

## Customizing the Dashboard

1. **Edit Panels**: Click on any panel title → "Edit"
2. **Add Panels**: Click "Add panel" in dashboard edit mode
3. **Modify Queries**: Update Prometheus queries to match your metrics
4. **Save Changes**: Click "Save dashboard" (disk icon)

## Production Considerations

- Use proper authentication and SSL certificates
- Set up persistent storage for Prometheus data
- Configure alerting rules for critical metrics
- Use environment-specific configuration files
- Set up log aggregation (ELK stack or similar)

## Stopping the Services

```bash
# Stop monitoring stack
docker-compose -f docker-compose.monitoring.yml down

# Remove volumes (optional - will delete all data)
docker-compose -f docker-compose.monitoring.yml down -v
```