# Oxigraph Observability Infrastructure

## Overview

This document describes the observability infrastructure implemented for Oxigraph to enable production-ready monitoring, health checks, and metrics collection.

## Features Implemented

### 1. Structured Logging

**Location:** `cli/src/main.rs`

Oxigraph now supports structured JSON logging using the `tracing` crate.

#### Usage

```bash
# Enable structured JSON logging
RUST_LOG=info oxigraph serve --bind 127.0.0.1:7878

# Different log levels
RUST_LOG=debug oxigraph serve --bind 127.0.0.1:7878
RUST_LOG=warn oxigraph serve --bind 127.0.0.1:7878
```

#### Example Output

```json
{
  "timestamp": "2025-12-26T10:30:00.123456Z",
  "level": "INFO",
  "fields": {
    "bind": "127.0.0.1:7878",
    "version": "0.5.3",
    "read_only": false,
    "union_default_graph": false
  },
  "target": "oxigraph_cli",
  "message": "Server started and listening for requests"
}
```

#### Key Log Points

- **Server startup**: Logs bind address, version, and configuration
- **Errors**: All internal server errors are logged with structured context
- **Query execution**: (Future enhancement: add query timing)

### 2. Health Check Endpoint

**Location:** `cli/src/health.rs`, route: `GET /health`

Provides a health check endpoint for load balancers and monitoring systems.

#### Usage

```bash
curl http://localhost:7878/health
```

#### Response Format

```json
{
  "status": "healthy",
  "version": "0.5.3",
  "uptime_seconds": 3600,
  "triple_count": 1234567
}
```

#### Fields

- `status`: Always "healthy" if server is responding
- `version`: Oxigraph version
- `uptime_seconds`: Time since server start
- `triple_count`: Total number of triples/quads in store (optional, may be null for read-only stores)

### 3. Prometheus Metrics Endpoint

**Location:** `lib/oxigraph/src/metrics.rs`, route: `GET /metrics`

Exports metrics in Prometheus format for scraping.

#### Usage

```bash
curl http://localhost:7878/metrics
```

#### Response Format

```
# HELP oxigraph_queries_total Total number of queries executed
# TYPE oxigraph_queries_total counter
oxigraph_queries_total 42

# HELP oxigraph_query_errors_total Total number of query errors
# TYPE oxigraph_query_errors_total counter
oxigraph_query_errors_total 2

# HELP oxigraph_query_duration_sum_ms Total query execution time in milliseconds
# TYPE oxigraph_query_duration_sum_ms counter
oxigraph_query_duration_sum_ms 15234

# HELP oxigraph_inserts_total Total number of triples/quads inserted
# TYPE oxigraph_inserts_total counter
oxigraph_inserts_total 10000

# HELP oxigraph_deletes_total Total number of triples/quads deleted
# TYPE oxigraph_deletes_total counter
oxigraph_deletes_total 100
```

#### Metrics Available

1. **oxigraph_queries_total**: Total queries executed
2. **oxigraph_query_errors_total**: Total query errors
3. **oxigraph_query_duration_sum_ms**: Total query execution time (ms)
4. **oxigraph_inserts_total**: Total triples/quads inserted
5. **oxigraph_deletes_total**: Total triples/quads deleted

## Architecture

### Components

```
lib/oxigraph/src/
├── metrics.rs          # Metrics collection module
│   ├── StoreMetrics    # Atomic counters for metrics
│   └── Timer           # Duration measurement utility
└── lib.rs              # Public module export

cli/src/
├── health.rs           # Health check module
│   ├── HealthStatus    # Health response structure
│   ├── init_metrics()  # Global metrics initialization
│   └── get_metrics()   # Access to global metrics
└── main.rs             # Server with /health and /metrics routes
```

### Design Decisions

1. **Lock-free metrics**: Uses `AtomicU64` for lock-free metric updates
2. **Global metrics**: Singleton pattern for metrics accessible across threads
3. **JSON logging**: Structured logs for easy parsing by log aggregators
4. **Prometheus format**: Standard metrics format for ecosystem compatibility

## Testing

### Unit Tests

```bash
# Test metrics module
cargo test -p oxigraph --lib metrics --no-default-features

# Test health module
cargo test -p oxigraph-cli health
```

### Integration Tests

```bash
# Run all observability tests
cargo test -p oxigraph-cli observability
```

### Manual Testing

```bash
# Start server with logging
RUST_LOG=info cargo run -p oxigraph-cli -- serve --bind 127.0.0.1:7878

# In another terminal:
# Check health
curl http://localhost:7878/health | jq

# Check metrics
curl http://localhost:7878/metrics

# Execute a query to generate metrics
curl -X POST http://localhost:7878/query \
  -H "Content-Type: application/sparql-query" \
  -d "SELECT * WHERE { ?s ?p ?o } LIMIT 10"

# Check metrics again
curl http://localhost:7878/metrics
```

## Production Deployment

### Kubernetes Deployment

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: oxigraph
spec:
  containers:
  - name: oxigraph
    image: oxigraph/oxigraph:latest
    env:
    - name: RUST_LOG
      value: "info"
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

### Prometheus Scrape Config

```yaml
scrape_configs:
  - job_name: 'oxigraph'
    static_configs:
      - targets: ['localhost:7878']
    metrics_path: /metrics
    scrape_interval: 15s
```

### Log Aggregation

For JSON log parsing with Elasticsearch/Fluentd/etc:

```bash
# Run with JSON logging
RUST_LOG=info oxigraph serve --bind 0.0.0.0:7878 2>&1 | fluentd
```

## Future Enhancements

### High Priority

1. **Query timing instrumentation**: Add tracing spans to query execution
2. **Storage metrics**: Track RocksDB stats (cache hits, compaction, etc.)
3. **Detailed query metrics**: Track query types (SELECT, CONSTRUCT, ASK, etc.)

### Medium Priority

4. **Distributed tracing**: Add OpenTelemetry support
5. **Custom metrics**: Allow users to define custom metrics via config
6. **Metrics retention**: Add time-windowed metrics (last 5min, 1hr, etc.)

### Low Priority

7. **Grafana dashboard**: Provide pre-built dashboard
8. **Alert rules**: Prometheus alert examples
9. **Performance profiling**: Add CPU/memory profiling endpoints

## Compliance

### 80/20 Principle

This implementation follows the 80/20 principle by focusing on:

- **Health checks** (20%): Covers 80% of uptime monitoring needs
- **Prometheus metrics** (20%): Covers 80% of performance monitoring needs
- **Structured logging** (20%): Covers 80% of debugging needs

These three features provide 80% of production observability value with minimal complexity.

### Production Readiness

✅ **Adequate for production:**
- Health endpoint for load balancers
- Metrics for monitoring dashboards
- Structured logs for debugging
- Zero-overhead when RUST_LOG not set
- Thread-safe metric collection

⚠️ **Known limitations:**
- No distributed tracing (yet)
- No custom metric labels
- Limited storage metrics
- No real-time metric streaming

## Dependencies Added

### lib/oxigraph/Cargo.toml
```toml
tracing = "0.1"
```

### cli/Cargo.toml
```toml
serde = { workspace = true, features = ["derive"] }
serde_json.workspace = true
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["json", "env-filter"] }
```

## References

- [Tracing crate documentation](https://docs.rs/tracing/)
- [Prometheus exposition formats](https://prometheus.io/docs/instrumenting/exposition_formats/)
- [OpenTelemetry specification](https://opentelemetry.io/docs/specs/)
- [12-Factor App: Logs](https://12factor.net/logs)
