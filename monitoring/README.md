# CBC Monitoring Setup

This directory contains the configuration files for monitoring your CBC node with Prometheus and Grafana.

## Quick Start

1. **Start the monitoring stack:**
   ```bash
   ./scripts/start-monitoring.sh
   ```

2. **Start your CBC node with metrics:**
   ```bash
   cargo build --release
   ./target/release/cerulea-node --dev --prometheus-external
   ```

3. **Test the setup:**
   ```bash
   ./scripts/test-dashboard.sh
   ```

4. **Open Grafana:**
   - URL: http://localhost:3000
   - Username: `admin`
   - Password: `admin`

## Files

- `prometheus.yml` - Prometheus configuration
- `grafana/provisioning/` - Grafana auto-provisioning configs
  - `datasources/prometheus.yml` - Prometheus datasource config
  - `dashboards/dashboard.yml` - Dashboard provisioning config

## Troubleshooting

If you don't see data in the dashboard:

1. Check if your CBC node is running with `--prometheus-external`
2. Verify metrics are available: `curl http://localhost:9615/metrics | grep cbc_`
3. Check Prometheus targets: http://localhost:9090/targets
4. View logs: `docker-compose -f docker-compose.monitoring.yml logs`

## Stopping

```bash
docker-compose -f docker-compose.monitoring.yml down
```

For more detailed instructions, see `docs/grafana-setup-guide.md`.