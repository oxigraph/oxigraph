# Oxigraph Soak Test

A comprehensive production-readiness test for Oxigraph that runs for extended periods to detect memory leaks, performance degradation, and stability issues.

## Overview

The soak test simulates a realistic mixed workload against an Oxigraph store:

- **70% Read Operations**: SELECT queries with various patterns
- **20% Write Operations**: INSERT/DELETE operations
- **10% Complex Queries**: Aggregations, filters, GROUP BY operations

## Running the Test

### Basic Usage

Run for 1 hour (default):
```bash
cargo run -p oxigraph --example soak
```

### Custom Duration

Specify duration in seconds:
```bash
# Run for 10 minutes (600 seconds)
cargo run -p oxigraph --example soak -- --duration 600

# Run for 4 hours (14400 seconds)
cargo run -p oxigraph --example soak -- --duration 14400
```

### Without RocksDB (in-memory only)

For testing without RocksDB dependency:
```bash
cargo run -p oxigraph --example soak --no-default-features -- --duration 3600
```

## What It Monitors

### 1. Throughput
- Queries per second (QPS)
- Total query count
- Tracks stability over time

### 2. Error Rates
- Counts all query errors
- Fails if error rate exceeds 1%

### 3. Latency
- p50 (median), p95, p99 percentiles
- Detects latency degradation
- All latencies in microseconds

### 4. Memory Usage
- Monitors resident memory (RSS) on Linux
- Detects monotonic growth (leak indicator)
- Warns if growth > 10% per hour
- Fails if total growth > 50%

## Output Example

```
=== Oxigraph Soak Test ===
Duration: 3600s (60 minutes)
Workers: 4
Workload: 70% reads, 20% writes, 10% complex queries

Starting soak test... Press Ctrl+C to stop early
Time         Queries         Errors       Memory       QPS          p50          p95
----------------------------------------------------------------------------------------------------
[00:01:00]  150230          0            45.2MB       2503         2856μs       3721μs
[00:02:00]  298456          0            45.8MB       2470         2912μs       3798μs
[00:03:00]  445821          0            46.1MB       2456         2889μs       3765μs
...

=== SOAK TEST COMPLETE ===

Duration:       3600s (60 minutes)
Total Queries:  9012345
Total Errors:   12
Error Rate:     0.00%
Avg QPS:        2503.4

Latency Stats:
  p50:          2901μs
  p95:          3782μs
  p99:          4521μs

Memory Stats:
  Start:        40.5MB
  End:          46.3MB
  Growth:       14.3%
  Trend:        STABLE

VERDICT:        ✓ PASS
```

## Pass/Fail Criteria

The test **PASSES** if:
- Error rate < 1%
- Total memory growth < 50%

The test **FAILS** if:
- Error rate ≥ 1%
- Memory growth ≥ 50%

## Interpreting Results

### Healthy Test Results

- **Memory**: Should stabilize after ~5-10 minutes (warmup period)
- **Latency**: Should remain stable, no gradual increase over time
- **Error Rate**: Should be 0% or very close to 0%
- **QPS**: May vary slightly but should be consistent

### Warning Signs

⚠️ **Memory continuously increasing**
- Indicates potential memory leak
- Check if memory stabilizes after warmup (first 10 minutes)

⚠️ **Latency increasing over time**
- May indicate performance degradation
- Could be related to data growth or inefficient queries

⚠️ **High error rate**
- Indicates stability issues
- Check stderr for error details

## Workload Details

### Read Queries (70%)
1. Get all people with their properties
2. Get products with prices
3. Get social connections
4. ASK queries for data existence

### Write Operations (20%)
- Insert new people with random IDs
- Periodically delete old data (cleanup)
- Mix of triples across different predicates

### Complex Queries (10%)
1. COUNT and AVG aggregations
2. GROUP BY with OPTIONAL patterns
3. FILTER with expressions
4. CONSTRUCT queries

## Configuration

Edit the constants at the top of `soak.rs` to customize:

```rust
const DEFAULT_DURATION_SECS: u64 = 3600;  // Default 1 hour
const WORKER_THREADS: usize = 4;          // Concurrent workers
const STATS_INTERVAL_SECS: u64 = 60;      // Report interval

// Workload distribution
const READ_PERCENTAGE: u64 = 70;
const WRITE_PERCENTAGE: u64 = 20;
const COMPLEX_PERCENTAGE: u64 = 10;
```

## Tips for Long-Running Tests

### For Multi-Hour Tests
```bash
# Run in background, log to file
nohup cargo run -p oxigraph --example soak -- --duration 14400 > soak.log 2>&1 &

# Monitor progress
tail -f soak.log
```

### For Production-Like Testing
```bash
# Run for 24 hours
cargo run -p oxigraph --example soak --release -- --duration 86400
```

**Note**: Use `--release` for performance testing to avoid debug overhead.

### Memory Monitoring

On Linux, memory is read from `/proc/self/status` (VmRSS).
On other platforms, memory monitoring may not be available.

## Troubleshooting

### Test completes but shows "FAIL"

Check the failure reasons in the output:
- If memory growth is the issue, verify it's not just warmup (check if it stabilized)
- If error rate is the issue, check for query errors in stderr

### Test is very slow

- Make sure you're using `--release` build for performance testing
- Adjust `WORKER_THREADS` based on your CPU cores
- Consider reducing the workload complexity

### Out of memory

- Reduce `WORKER_THREADS`
- Reduce test duration
- The cleanup logic removes old data, but may need tuning

## Integration with CI/CD

Example GitHub Actions workflow:

```yaml
- name: Run Oxigraph Soak Test
  run: |
    cargo run -p oxigraph --example soak --release -- --duration 300
  timeout-minutes: 10
```

For CI, use shorter durations (5-10 minutes) to catch obvious issues without excessive runtime.

## What Makes This a Good Soak Test?

✅ **Mixed Workload**: Realistic combination of reads, writes, and complex queries
✅ **Memory Leak Detection**: Monitors for monotonic memory growth
✅ **Performance Tracking**: Latency percentiles detect degradation
✅ **Long-Running**: Can run for hours/days to catch subtle issues
✅ **Clear Pass/Fail**: Objective criteria for CI/CD integration
✅ **Detailed Reporting**: Comprehensive stats for debugging

## Further Reading

- [Oxigraph Documentation](https://docs.rs/oxigraph)
- [SPARQL Query Language](https://www.w3.org/TR/sparql11-query/)
- [Performance Testing Best Practices](https://github.com/oxigraph/oxigraph/blob/main/bench/README.md)
