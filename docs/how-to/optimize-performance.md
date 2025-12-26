# How to Optimize Oxigraph Performance

This guide covers best practices for maximizing Oxigraph performance across different use cases.

## Bulk Loading Optimization

### Use the Bulk Loader

**Always use the bulk loader for large datasets** - it's 10-100x faster than individual inserts:

```rust
// DON'T: Slow for large datasets
for quad in quads {
    store.insert(&quad)?;
}

// DO: Use bulk loader
let mut loader = store.bulk_loader();
for quad in quads {
    loader.load_quad(quad)?;
}
loader.commit()?;
```

### Non-Atomic Loading

For maximum speed when safety is not critical:

```bash
# CLI: Non-atomic mode
oxigraph load --location data --file huge.nq --non-atomic
```

```rust
// Rust: Non-atomic bulk loading
let mut loader = store.bulk_loader()
    .without_atomicity();
loader.load_from_path("huge.nq")?;
loader.commit()?;
```

**Warning**: Non-atomic mode can leave the database in an inconsistent state if the operation fails.

### Parallel Loading

The CLI automatically uses parallel loading for multiple files:

```bash
# Loads files in parallel (uses half of available CPU cores)
oxigraph load --location data \
  --file data1.nq \
  --file data2.nq \
  --file data3.nq \
  --file data4.nq
```

### Choose Fast Formats

**N-Triples and N-Quads are fastest** for parsing (line-oriented, no complex syntax):

```bash
# Fastest
oxigraph load --location data --file data.nq

# Slower (requires parsing complex syntax)
oxigraph load --location data --file data.ttl
```

### Progress Monitoring

Monitor progress without impacting performance:

```rust
let mut loader = store.bulk_loader()
    .on_progress(|count| {
        if count % 1_000_000 == 0 {
            eprintln!("{} triples loaded", count);
        }
    });
```

### Error Handling in Bulk Load

Use lenient mode to skip malformed data:

```bash
# Skip parse errors
oxigraph load --location data --file dirty-data.nq --lenient
```

```rust
let mut loader = store.bulk_loader()
    .on_parse_error(|e| {
        eprintln!("Skipping error: {}", e);
        Ok(()) // Continue on error
    });
```

## Query Optimization

### Index Usage

Oxigraph maintains three indexes: SPO, POS, OSP. Structure queries to leverage them:

```sparql
# GOOD: Uses SPO index
SELECT ?p ?o WHERE {
  <http://example.com/subject> ?p ?o .
}

# GOOD: Uses POS index
SELECT ?s ?o WHERE {
  ?s <http://example.com/predicate> ?o .
}

# GOOD: Uses OSP index
SELECT ?s ?p WHERE {
  ?s ?p "literal value" .
}

# LESS OPTIMAL: Requires full scan
SELECT ?s ?p ?o WHERE {
  ?s ?p ?o .
  FILTER(CONTAINS(STR(?o), "text"))
}
```

### Filter Placement

Place filters early to reduce intermediate results:

```sparql
# GOOD: Filter early
SELECT ?person ?name WHERE {
  ?person a :Person .
  ?person :birthYear ?year .
  FILTER(?year > 1990)
  ?person :name ?name .
}

# LESS OPTIMAL: Filter late
SELECT ?person ?name WHERE {
  ?person a :Person .
  ?person :birthYear ?year .
  ?person :name ?name .
  FILTER(?year > 1990)
}
```

### Avoid OPTIONAL When Possible

OPTIONAL clauses are expensive. Use them only when necessary:

```sparql
# GOOD: If email is required
SELECT ?person ?email WHERE {
  ?person a :Person .
  ?person :email ?email .
}

# EXPENSIVE: Only use if email is truly optional
SELECT ?person ?email WHERE {
  ?person a :Person .
  OPTIONAL { ?person :email ?email }
}
```

### LIMIT Results

Always use LIMIT when appropriate to avoid processing unnecessary results:

```sparql
# GOOD: Get sample results
SELECT ?s ?p ?o WHERE {
  ?s ?p ?o .
} LIMIT 100

# GOOD: Check for existence
ASK { ?s a :Person }

# LESS OPTIMAL: Counting all results when you just need to know if any exist
SELECT (COUNT(*) as ?count) WHERE { ?s a :Person }
```

### Query Timeouts

Set timeouts to prevent runaway queries:

```bash
# Server with 30-second timeout
oxigraph serve --location data --timeout-s 30
```

```rust
use std::time::Duration;
use oxigraph::sparql::CancellationToken;

let token = CancellationToken::new();
token.set_timeout(Duration::from_secs(30));

let results = SparqlEvaluator::new()
    .parse_query(query)?
    .on_store(&store)
    .with_cancellation_token(token)
    .execute()?;
```

## Memory Management

### Transaction Batching

Batch operations in transactions for better performance:

```rust
// DON'T: Many small transactions
for quad in quads {
    store.insert(&quad)?; // Each insert is a transaction
}

// DO: Batch in transaction
store.extend(quads)?; // Single transaction
```

```python
# DON'T
for quad in quads:
    store.add(quad)

# DO
store.extend(quads)
```

### Streaming Large Results

Stream results instead of collecting everything into memory:

```rust
// DON'T: Collect all results
let all_quads: Vec<_> = store
    .quads_for_pattern(None, None, None, None)
    .collect::<Result<Vec<_>, _>>()?;

// DO: Process as stream
for quad in store.quads_for_pattern(None, None, None, None) {
    process_quad(&quad?)?;
}
```

### Limit Query Result Sets

Use SPARQL LIMIT and OFFSET for pagination:

```sparql
# Page 1
SELECT ?s ?p ?o WHERE {
  ?s ?p ?o .
} LIMIT 100

# Page 2
SELECT ?s ?p ?o WHERE {
  ?s ?p ?o .
} LIMIT 100 OFFSET 100
```

## Storage Optimization

### Optimize After Bulk Load

After bulk loading, optimize the database:

```bash
oxigraph optimize --location data
```

This compacts the RocksDB storage and improves read performance.

### Regular Backups

Regular backups prevent database bloat:

```bash
# Create backup
oxigraph backup --location data --destination backup/

# Restore from backup (cleaner storage)
rm -rf data
cp -r backup/ data/
```

### Disk I/O Optimization

Use fast storage (SSD) for the database:

```bash
# Good: SSD storage
oxigraph serve --location /ssd/oxigraph-data

# Less optimal: HDD storage
oxigraph serve --location /hdd/oxigraph-data
```

### Read-Only Mode

For read-heavy workloads, use read-only mode:

```bash
# Read-only server (allows multiple instances)
oxigraph serve-read-only --location data
```

Benefits:
- Multiple read-only instances can run simultaneously
- Slightly faster queries (no write locks)
- Safe for concurrent access

## Application-Level Optimization

### Connection Pooling

In Rust, clone the Store instance (cheap - shares underlying storage):

```rust
let store = Store::open("data")?;

// Clone for use in different threads (shares storage)
let store_clone = store.clone();
thread::spawn(move || {
    // Use store_clone
});
```

### Caching Results

Cache frequently-used query results:

```rust
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

struct QueryCache {
    cache: Arc<Mutex<HashMap<String, Vec<QuerySolution>>>>,
}

impl QueryCache {
    fn get_or_compute<F>(&self, query: &str, f: F) -> Vec<QuerySolution>
    where
        F: FnOnce() -> Vec<QuerySolution>,
    {
        let mut cache = self.cache.lock().unwrap();
        cache.entry(query.to_string())
            .or_insert_with(f)
            .clone()
    }
}
```

### Prepared Queries

Reuse parsed queries:

```rust
let evaluator = SparqlEvaluator::new();
let query = evaluator.parse_query("SELECT * WHERE { ?s ?p ?o }")?;

// Reuse parsed query multiple times
for _ in 0..100 {
    let results = query.on_store(&store).execute()?;
    // Process results
}
```

## Network Optimization

### Compression

Enable compression for HTTP responses:

```nginx
# Nginx configuration
gzip on;
gzip_types application/sparql-results+json text/turtle application/n-triples;
```

### CDN for Static Content

Serve the YASGUI interface via CDN in production.

### Connection Keep-Alive

Use HTTP keep-alive for multiple requests:

```python
import requests

session = requests.Session()
for query in queries:
    response = session.post(
        'http://localhost:7878/query',
        data=query,
        headers={'Content-Type': 'application/sparql-query'}
    )
```

## Monitoring and Profiling

### Query Performance Analysis

Use EXPLAIN to understand query execution:

```bash
# Get query explanation
oxigraph query --location data \
  --query "SELECT * WHERE { ?s ?p ?o }" \
  --explain
```

### Resource Monitoring

Monitor system resources:

```bash
# CPU and memory usage
htop

# Disk I/O
iotop

# Database size
du -sh /path/to/oxigraph-data
```

### Enable Statistics

Track query performance in production:

```bash
oxigraph query --location data \
  --query "SELECT * WHERE { ?s ?p ?o }" \
  --stats
```

## Platform-Specific Optimizations

### Linux

```bash
# Increase file descriptor limits
ulimit -n 65536

# Optimize disk I/O scheduler
echo deadline > /sys/block/sda/queue/scheduler
```

### Docker

```yaml
# docker-compose.yml
services:
  oxigraph:
    image: ghcr.io/oxigraph/oxigraph:latest
    volumes:
      - ./data:/data
    environment:
      - ROCKSDB_MAX_OPEN_FILES=1000
    deploy:
      resources:
        limits:
          cpus: '4'
          memory: 8G
```

### Kubernetes

```yaml
apiVersion: v1
kind: Pod
metadata:
  name: oxigraph
spec:
  containers:
  - name: oxigraph
    image: ghcr.io/oxigraph/oxigraph:latest
    resources:
      limits:
        memory: "8Gi"
        cpu: "4"
      requests:
        memory: "4Gi"
        cpu: "2"
    volumeMounts:
    - name: data
      mountPath: /data
  volumes:
  - name: data
    persistentVolumeClaim:
      claimName: oxigraph-data
```

## Benchmarking

### Measure Loading Performance

```bash
time oxigraph load --location data --file dataset.nq
```

### Measure Query Performance

```bash
# Run query and measure time
time curl -X POST http://localhost:7878/query \
  -H 'Content-Type: application/sparql-query' \
  --data @query.sparql
```

### Automated Benchmarking

```rust
use std::time::Instant;

let start = Instant::now();
let results = SparqlEvaluator::new()
    .parse_query(query)?
    .on_store(&store)
    .execute()?;

let count = results.count();
let duration = start.elapsed();

println!("Query returned {} results in {:?}", count, duration);
```

## Performance Checklist

### For Bulk Loading
- [ ] Use `bulk_loader()` instead of individual inserts
- [ ] Use N-Quads or N-Triples format
- [ ] Load files in parallel when possible
- [ ] Consider non-atomic mode for trusted data
- [ ] Enable lenient mode for dirty data
- [ ] Run `oxigraph optimize` after loading

### For Queries
- [ ] Use appropriate indexes (check query patterns)
- [ ] Add filters early in query
- [ ] Use LIMIT when possible
- [ ] Avoid unnecessary OPTIONAL clauses
- [ ] Set query timeouts
- [ ] Use ASK instead of COUNT for existence checks

### For Storage
- [ ] Use SSD storage
- [ ] Run regular backups and optimization
- [ ] Monitor disk space
- [ ] Use read-only mode for read-heavy workloads

### For Applications
- [ ] Batch operations in transactions
- [ ] Stream large results
- [ ] Cache frequently-used results
- [ ] Reuse parsed queries
- [ ] Clone Store instances instead of reopening

## Common Performance Pitfalls

### Anti-Pattern: Frequent Small Transactions

```rust
// SLOW
for quad in quads {
    store.insert(&quad)?;
}

// FAST
store.extend(quads)?;
```

### Anti-Pattern: Collecting Large Result Sets

```rust
// MEMORY INTENSIVE
let all: Vec<_> = store.quads_for_pattern(None, None, None, None)
    .collect::<Result<Vec<_>, _>>()?;

// EFFICIENT
for quad in store.quads_for_pattern(None, None, None, None) {
    process(&quad?)?;
}
```

### Anti-Pattern: Reopening Store

```rust
// SLOW
for query in queries {
    let store = Store::open("data")?;
    // ... query
}

// FAST
let store = Store::open("data")?;
for query in queries {
    // ... query using same store
}
```

## Next Steps

- Learn about [importing data efficiently](import-rdf-data.md)
- Set up a [production SPARQL server](run-sparql-server.md)
- Validate data with [SHACL](validate-with-shacl.md)
