# Oxigraph Observability - Quick Start Guide

## 🚀 5-Minute Production Setup

### 1. Start Server with Observability

```bash
# Enable structured JSON logging
RUST_LOG=info oxigraph serve --bind 0.0.0.0:7878 --location /data/oxigraph
```

### 2. Verify Health Endpoint

```bash
curl http://localhost:7878/health

# Expected response:
{
  "status": "healthy",
  "version": "0.5.3",
  "uptime_seconds": 10,
  "triple_count": 1000000
}
```

### 3. Check Metrics

```bash
curl http://localhost:7878/metrics

# Expected output:
# HELP oxigraph_queries_total Total number of queries executed
# TYPE oxigraph_queries_total counter
oxigraph_queries_total 42
...
```

### 4. View Structured Logs

Logs are written to stderr in JSON format when `RUST_LOG` is set:

```json
{"timestamp":"2025-12-26T10:30:00Z","level":"INFO","message":"Server started..."}
```

---

## 📊 Kubernetes Integration

### Deployment with Health Probes

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: oxigraph
spec:
  template:
    spec:
      containers:
      - name: oxigraph
        image: oxigraph/oxigraph:latest
        env:
        - name: RUST_LOG
          value: "info"
        ports:
        - containerPort: 7878
        livenessProbe:
          httpGet:
            path: /health
            port: 7878
          initialDelaySeconds: 10
          periodSeconds: 30
        readinessProbe:
          httpGet:
            path: /health
            port: 7878
          initialDelaySeconds: 5
          periodSeconds: 10
```

---

## 📈 Prometheus Integration

### Scrape Configuration

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'oxigraph'
    static_configs:
      - targets: ['oxigraph:7878']
    metrics_path: /metrics
    scrape_interval: 15s
```

### Docker Compose with Prometheus

```yaml
version: '3.8'
services:
  oxigraph:
    image: oxigraph/oxigraph:latest
    environment:
      - RUST_LOG=info
    ports:
      - "7878:7878"
    command: serve --bind 0.0.0.0:7878

  prometheus:
    image: prom/prometheus:latest
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
```

---

## 🔍 Log Aggregation

### Fluentd Configuration

```conf
<source>
  @type tail
  path /var/log/oxigraph/app.log
  pos_file /var/log/oxigraph/app.log.pos
  tag oxigraph
  format json
  time_key timestamp
  time_format %Y-%m-%dT%H:%M:%S.%NZ
</source>

<match oxigraph>
  @type elasticsearch
  host elasticsearch
  port 9200
  index_name oxigraph
  type_name _doc
</match>
```

---

## 🎯 Key Metrics to Monitor

### Essential Metrics

| Metric | Purpose | Alert Threshold |
|--------|---------|----------------|
| `oxigraph_queries_total` | Query volume | Sudden drops/spikes |
| `oxigraph_query_errors_total` | Error rate | > 5% of total queries |
| `oxigraph_query_duration_sum_ms` | Performance | Increasing trend |
| `oxigraph_inserts_total` | Write activity | Expected vs actual |
| `oxigraph_deletes_total` | Delete activity | Unexpected spikes |

### Derived Metrics (PromQL)

```promql
# Query error rate
rate(oxigraph_query_errors_total[5m]) / rate(oxigraph_queries_total[5m])

# Average query duration
rate(oxigraph_query_duration_sum_ms[5m]) / rate(oxigraph_queries_total[5m])

# Queries per second
rate(oxigraph_queries_total[1m])
```

---

## 🚨 Sample Alerts

### High Error Rate

```yaml
- alert: OxigraphHighErrorRate
  expr: rate(oxigraph_query_errors_total[5m]) / rate(oxigraph_queries_total[5m]) > 0.05
  for: 5m
  labels:
    severity: warning
  annotations:
    summary: "Oxigraph query error rate is above 5%"
```

### Service Down

```yaml
- alert: OxigraphDown
  expr: up{job="oxigraph"} == 0
  for: 1m
  labels:
    severity: critical
  annotations:
    summary: "Oxigraph instance is down"
```

### Slow Queries

```yaml
- alert: OxigraphSlowQueries
  expr: rate(oxigraph_query_duration_sum_ms[5m]) / rate(oxigraph_queries_total[5m]) > 1000
  for: 10m
  labels:
    severity: warning
  annotations:
    summary: "Average query duration exceeds 1 second"
```

---

## 📋 Operational Checklist

### Pre-Deployment
- [ ] Set `RUST_LOG=info` in environment
- [ ] Configure health check in load balancer
- [ ] Add Prometheus scrape target
- [ ] Set up log aggregation pipeline
- [ ] Define alert rules

### Post-Deployment
- [ ] Verify `/health` returns 200 OK
- [ ] Verify `/metrics` is scrapeable
- [ ] Check logs appear in aggregator
- [ ] Test alert rules trigger correctly
- [ ] Validate Grafana dashboards

### Monitoring
- [ ] Check error rate daily
- [ ] Review query performance trends weekly
- [ ] Audit log volume monthly
- [ ] Test health checks in chaos engineering
- [ ] Update alert thresholds based on SLOs

---

## 🔧 Troubleshooting

### Health Check Returns 503

**Cause:** Metrics not initialized

**Fix:** Ensure server is fully started before health check

**Workaround:** Increase `initialDelaySeconds` in K8s probe

### No Logs Appearing

**Cause:** `RUST_LOG` not set

**Fix:** Set environment variable:
```bash
export RUST_LOG=info
```

### Metrics Show Zero

**Cause:** No queries executed yet

**Fix:** Execute a test query:
```bash
curl -X POST http://localhost:7878/query \
  -H "Content-Type: application/sparql-query" \
  -d "SELECT * WHERE { ?s ?p ?o } LIMIT 1"
```

### Prometheus Can't Scrape

**Cause:** Network/firewall issue

**Fix:** Verify connectivity:
```bash
curl http://oxigraph:7878/metrics
```

---

## 📚 Next Steps

1. **Read Full Documentation:** See [OBSERVABILITY.md](OBSERVABILITY.md)
2. **Review Implementation:** See [OBSERVABILITY_VERIFICATION.md](OBSERVABILITY_VERIFICATION.md)
3. **Check Examples:** Run `cargo run --example observability_demo`
4. **Deploy to Staging:** Test in staging environment first
5. **Configure Grafana:** Import metrics dashboard
6. **Set Alert Rules:** Define SLO-based alerts

---

## 🎓 Best Practices

### Log Levels

- `ERROR`: Unexpected errors requiring investigation
- `WARN`: Recoverable issues, unusual conditions
- `INFO`: Normal operational messages (default)
- `DEBUG`: Detailed diagnostic information
- `TRACE`: Very detailed, high-volume diagnostics

### Log Volume Management

```bash
# Production: INFO level
RUST_LOG=info oxigraph serve

# Debugging specific issue: DEBUG for short period
RUST_LOG=debug oxigraph serve

# High-volume debugging: DEBUG with filters
RUST_LOG=oxigraph=debug,tower_http=info oxigraph serve
```

### Metric Retention

- **Short-term (15 days):** Full resolution for debugging
- **Medium-term (90 days):** 5-minute aggregates for trends
- **Long-term (1 year):** 1-hour aggregates for capacity planning

---

**For detailed information, see:**
- User Guide: [OBSERVABILITY.md](OBSERVABILITY.md)
- Technical Details: [OBSERVABILITY_VERIFICATION.md](OBSERVABILITY_VERIFICATION.md)
- Implementation Summary: [AGENT_9_SUMMARY.md](AGENT_9_SUMMARY.md)

---

**Quick Reference Card - Keep This Handy!**

```
Health:    GET /health
Metrics:   GET /metrics
Logging:   RUST_LOG=info
K8s Probe: path=/health, port=7878
Prometheus: metrics_path=/metrics, port=7878
```
