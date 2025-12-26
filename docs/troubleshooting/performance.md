# Performance Troubleshooting

This guide helps identify and resolve performance issues in Oxigraph including slow queries, memory problems, and disk I/O bottlenecks.

## Table of Contents

- [Identifying Slow Queries](#identifying-slow-queries)
- [Query Analysis and Optimization](#query-analysis-and-optimization)
- [Memory Profiling](#memory-profiling)
- [Disk I/O Issues](#disk-io-issues)
- [Bulk Loading Optimization](#bulk-loading-optimization)
- [Benchmark Comparison](#benchmark-comparison)

---

## Identifying Slow Queries

### Enable Query Logging

**Symptom:**
Queries take longer than expected, but you don't know which ones.

**Cause:**
No query performance monitoring in place.

**Solution:**

#### Rust/CLI

```rust
use oxigraph::store::Store;
use std::time::Instant;

let store = Store::open("data")?;
let query = "SELECT * WHERE { ?s ?p ?o } LIMIT 100";

let start = Instant::now();
let results = store.query(query)?;
let count = results.count();
let duration = start.elapsed();

println!("Query returned {} results in {:?}", count, duration);

// With logging
env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("spareval=debug")).init();
```

```bash
# Enable query timing in CLI
RUST_LOG=spareval=debug oxigraph query --location /data --query query.sparql
```

**Output:**
```
[DEBUG spareval] Query parse time: 12ms
[DEBUG spareval] Query optimization time: 45ms
[DEBUG spareval] Query execution time: 1234ms
[DEBUG spareval] Total time: 1291ms
```

#### Python

```python
import pyoxigraph as ox
import time
import logging

logging.basicConfig(level=logging.DEBUG)

store = ox.Store("data")

def timed_query(store, query):
    start = time.time()
    results = list(store.query(query))
    duration = time.time() - start
    print(f"Query returned {len(results)} results in {duration:.3f}s")
    return results

query = "SELECT * WHERE { ?s ?p ?o } LIMIT 100"
results = timed_query(store, query)
```

#### JavaScript

```javascript
const oxigraph = require('oxigraph');

const store = new oxigraph.Store();

function timedQuery(store, query) {
  const start = performance.now();
  const results = Array.from(store.query(query));
  const duration = performance.now() - start;
  console.log(`Query returned ${results.length} results in ${duration.toFixed(2)}ms`);
  return results;
}

const query = "SELECT * WHERE { ?s ?p ?o } LIMIT 100";
timedQuery(store, query);
```

**Prevention:**
- Always enable query logging in production
- Set performance thresholds and alert on slow queries
- Log queries that exceed 1 second

---

### Setting Query Timeouts

**Symptom:**
Queries run indefinitely, consuming resources.

**Cause:**
No timeout set for complex queries.

**Solution:**

```rust
use std::time::Duration;

// Set timeout for specific query
let query = "SELECT * WHERE { ?s ?p ?o }";
match store.query_with_timeout(query, Duration::from_secs(30)) {
    Ok(results) => { /* process results */ },
    Err(e) if e.to_string().contains("timeout") => {
        eprintln!("Query exceeded 30 second timeout");
    },
    Err(e) => return Err(e),
}
```

**Prevention:**
- Set default timeouts in production (30-60 seconds)
- Use progressive timeouts: shorter for user queries, longer for analytics
- Implement query queue to prevent resource exhaustion

---

## Query Analysis and Optimization

### Understanding Query Plans

**Symptom:**
Query is slow but you don't understand why.

**Cause:**
Query plan creates inefficient execution (cartesian products, missing indexes).

**Solution:**

Enable query plan logging:

```bash
RUST_LOG=spareval=trace oxigraph query --location /data --query query.sparql
```

Look for these warning patterns in logs:

#### 1. Cartesian Product Warning

```
[WARN spareval] Query plan contains cartesian product, may be slow
```

**Example problem query:**
```sparql
SELECT ?person1 ?person2 WHERE {
  ?person1 a foaf:Person .
  ?person2 a foaf:Person .
}
# Returns N² results!
```

**Fix:** Add join condition
```sparql
SELECT ?person1 ?person2 WHERE {
  ?person1 a foaf:Person .
  ?person2 a foaf:Person .
  FILTER(?person1 != ?person2)  # Still N², but better
  ?person1 foaf:knows ?person2 .  # Best: actual relationship
}
```

#### 2. Large Intermediate Results

```
[DEBUG spareval] Intermediate result set: 1,000,000 bindings
```

**Fix:** Reorder triple patterns to filter early

```sparql
# ❌ Slow - gets all people first (1M), then filters
SELECT ?person ?email WHERE {
  ?person a foaf:Person .           # 1M results
  ?person foaf:mbox ?email .        # Filters to 100K
  ?person ex:country "USA"^^xsd:string .  # Filters to 1K
}

# ✅ Fast - filter first
SELECT ?person ?email WHERE {
  ?person ex:country "USA"^^xsd:string .  # 1K results
  ?person a foaf:Person .                  # Still ~1K
  ?person foaf:mbox ?email .              # Final ~1K
}
```

#### 3. Optional Chains

```
[WARN spareval] Multiple chained OPTIONAL blocks may be inefficient
```

**Fix:** Combine or rewrite

```sparql
# ❌ Slow - cascading OPTIONALs
SELECT ?person ?name ?email ?phone WHERE {
  ?person a foaf:Person .
  OPTIONAL { ?person foaf:name ?name }
  OPTIONAL { ?person foaf:mbox ?email }
  OPTIONAL { ?person foaf:phone ?phone }
}

# ✅ Better - same semantics, better performance
SELECT ?person ?name ?email ?phone WHERE {
  ?person a foaf:Person .
  OPTIONAL {
    ?person foaf:name ?name .
    ?person foaf:mbox ?email .
    ?person foaf:phone ?phone .
  }
}
```

**Prevention:**
- Review query plans before deploying queries to production
- Test queries on representative data sizes
- Use LIMIT during development to prevent accidents

---

### Query Optimization Techniques

#### Use LIMIT and OFFSET Wisely

**Symptom:**
Pagination queries get slower as offset increases.

**Cause:**
Database must scan and skip all offset rows.

**Solution:**

```sparql
# ❌ Slow for large offsets
SELECT ?person ?name WHERE {
  ?person foaf:name ?name .
}
LIMIT 10 OFFSET 100000
# Must process 100,010 results to return 10

# ✅ Better - use cursor-based pagination
SELECT ?person ?name WHERE {
  ?person foaf:name ?name .
  FILTER(?person > <last-seen-iri>)
}
ORDER BY ?person
LIMIT 10
```

**Prevention:**
- Document pagination limits (max offset)
- Use cursor-based pagination for deep pages
- Cache expensive query results

---

#### Use EXISTS Instead of COUNT

**Symptom:**
Query just checking existence is slow.

**Cause:**
COUNT(*) counts all matches when you only need to know if any exist.

**Solution:**

```sparql
# ❌ Slow - counts all matches
SELECT ?person WHERE {
  ?person a foaf:Person .
  FILTER(COUNT(?friend) > 0)
  { ?person foaf:knows ?friend }
}

# ✅ Fast - stops at first match
SELECT ?person WHERE {
  ?person a foaf:Person .
  FILTER EXISTS { ?person foaf:knows ?friend }
}
```

**Prevention:**
- Use EXISTS/NOT EXISTS for boolean checks
- Only use COUNT when you need the actual count

---

#### Minimize UNION

**Symptom:**
Queries with many UNION clauses are slow.

**Cause:**
Each UNION branch is executed separately.

**Solution:**

```sparql
# ❌ Slow - multiple unions
SELECT ?resource WHERE {
  { ?resource a ex:Type1 }
  UNION
  { ?resource a ex:Type2 }
  UNION
  { ?resource a ex:Type3 }
}

# ✅ Fast - use VALUES
SELECT ?resource WHERE {
  VALUES ?type { ex:Type1 ex:Type2 ex:Type3 }
  ?resource a ?type .
}

# ✅ Or property paths
SELECT ?resource WHERE {
  ?resource a ?type .
  FILTER(?type IN (ex:Type1, ex:Type2, ex:Type3))
}
```

**Prevention:**
- Prefer VALUES over UNION when possible
- Use FILTER IN for small sets of values

---

## Memory Profiling

### Identifying Memory Issues

**Symptom:**
High memory usage or out-of-memory errors.

**Cause:**
Large result sets, inefficient queries, or memory leaks.

**Solution:**

#### Monitor Memory Usage

**Linux:**
```bash
# Real-time monitoring
top -p $(pgrep oxigraph)

# Detailed memory breakdown
pmap -x $(pgrep oxigraph)

# Memory over time
while true; do
  date >> memory.log
  ps -p $(pgrep oxigraph) -o rss= >> memory.log
  sleep 10
done
```

**Rust code:**
```rust
// Track memory allocations
#[global_allocator]
static ALLOC: jemallocator::Jemalloc = jemallocator::Jemalloc;

use jemalloc_ctl::{stats, epoch};

// Get memory stats
epoch::mib().unwrap().advance().unwrap();
let allocated = stats::allocated::mib().unwrap().read().unwrap();
let resident = stats::resident::mib().unwrap().read().unwrap();
println!("Memory: allocated={}MB resident={}MB",
    allocated / 1024 / 1024,
    resident / 1024 / 1024);
```

**Python:**
```python
import tracemalloc
import pyoxigraph as ox

tracemalloc.start()

store = ox.Store()
# ... operations ...

current, peak = tracemalloc.get_traced_memory()
print(f"Current memory: {current / 1024 / 1024:.2f} MB")
print(f"Peak memory: {peak / 1024 / 1024:.2f} MB")

tracemalloc.stop()
```

**JavaScript/Node.js:**
```javascript
const v8 = require('v8');

function getMemoryUsage() {
  const heapStats = v8.getHeapStatistics();
  return {
    totalHeapSize: (heapStats.total_heap_size / 1024 / 1024).toFixed(2) + ' MB',
    usedHeapSize: (heapStats.used_heap_size / 1024 / 1024).toFixed(2) + ' MB',
    heapLimit: (heapStats.heap_size_limit / 1024 / 1024).toFixed(2) + ' MB'
  };
}

console.log('Before:', getMemoryUsage());
// ... operations ...
console.log('After:', getMemoryUsage());
```

**Prevention:**
- Set up continuous memory monitoring
- Alert when memory usage exceeds 80% of available
- Profile memory in staging before production

---

### Reducing Memory Usage

**Symptom:**
Memory usage grows during operations.

**Cause:**
Loading all results into memory at once.

**Solution:**

#### Use Streaming/Iteration

```rust
// ❌ Memory intensive - loads all into Vec
let query = "SELECT * WHERE { ?s ?p ?o }";
let results: Vec<_> = store.query(query)?.collect();
// All results in memory!

// ✅ Memory efficient - process one at a time
for result in store.query(query)? {
    let solution = result?;
    process_solution(solution);
    // Previous results can be freed
}
```

```python
# ❌ Memory intensive
results = list(store.query("SELECT * WHERE { ?s ?p ?o }"))

# ✅ Memory efficient
for result in store.query("SELECT * WHERE { ?s ?p ?o }"):
    process_result(result)
```

#### Use Query Result Limits

```sparql
-- For development/testing
SELECT * WHERE {
  ?s ?p ?o
}
LIMIT 1000  -- Prevent accidents
```

#### Batch Processing for Large Exports

```rust
use oxigraph::io::RdfSerializer;

// Export in batches
let batch_size = 100_000;
let mut offset = 0;

loop {
    let query = format!(
        "SELECT * WHERE {{ ?s ?p ?o }} LIMIT {} OFFSET {}",
        batch_size, offset
    );

    let results: Vec<_> = store.query(&query)?.collect();
    if results.is_empty() {
        break;
    }

    // Process batch
    for result in results {
        // ... write to file ...
    }

    offset += batch_size;
}
```

**Prevention:**
- Always use iterators/generators for large result sets
- Implement pagination for user-facing queries
- Set application-level memory limits

---

## Disk I/O Issues

### Identifying I/O Bottlenecks

**Symptom:**
Operations are slow, CPU is not maxed out.

**Cause:**
Disk I/O is the bottleneck.

**Solution:**

#### Monitor Disk I/O

```bash
# Real-time I/O stats (Linux)
iostat -x 1

# Look for:
# - High %util (>80% = bottleneck)
# - High await (>10ms = slow disk)
# - High r/s or w/s (operations per second)

# Disk usage by process
iotop -o

# File access patterns
strace -e trace=open,read,write -p $(pgrep oxigraph)
```

#### Check Disk Type

```bash
# Identify disk type
lsblk -d -o name,rota
# rota=1: HDD (slow)
# rota=0: SSD (fast)

# For best performance, use SSD for Oxigraph store
```

**Prevention:**
- Use SSD for Oxigraph data directories
- Monitor disk I/O metrics
- Separate OS, logs, and data on different disks if possible

---

### Optimizing Disk Performance

**Symptom:**
Slow writes during bulk loading or updates.

**Cause:**
RocksDB write amplification, filesystem overhead.

**Solution:**

#### Use Bulk Loader

```rust
use oxigraph::io::RdfParser;
use oxigraph::store::Store;

let store = Store::new()?;

// ❌ Slow - individual inserts
for quad in parser {
    store.insert(quad?)?;
}

// ✅ Fast - bulk loader (optimized for large imports)
use oxigraph::store::BulkLoader;
let mut loader = store.bulk_loader()?;
for quad in parser {
    loader.add_quad(quad?)?;
}
loader.finish()?;
```

**Performance difference:**
- Individual inserts: ~10K quads/second
- Bulk loader: ~100K+ quads/second

#### Optimize RocksDB Settings

```rust
// For write-heavy workloads
use oxigraph::store::StoreOptions;

let options = StoreOptions::new()
    .write_buffer_size(128 * 1024 * 1024)  // 128MB write buffer
    .max_write_buffer_number(4)             // More buffers
    .target_file_size_base(64 * 1024 * 1024); // Larger files

let store = Store::open_with_options("data", options)?;
```

#### Use Faster Filesystem

```bash
# ext4 with optimizations
sudo mkfs.ext4 -O ^has_journal /dev/sdb1  # Disable journal for more speed (less safety)
sudo mount -o noatime,nodiratime /dev/sdb1 /data  # Disable access time updates

# Or XFS (often faster for large files)
sudo mkfs.xfs /dev/sdb1
sudo mount -o noatime /dev/sdb1 /data
```

**Prevention:**
- Use SSDs for production
- Tune RocksDB for your workload (read-heavy vs write-heavy)
- Monitor I/O wait time

---

### Compaction

**Symptom:**
Store size grows larger than expected, performance degrades over time.

**Cause:**
RocksDB hasn't compacted old/deleted data.

**Solution:**

```bash
# Manual compaction
oxigraph compact --location /data/oxigraph

# Or programmatically
```

```rust
store.compact()?;  // Triggers manual compaction
```

**When to compact:**
- After large bulk deletes
- After many updates to same data
- When store size is significantly larger than data size
- Performance has degraded over time

**Prevention:**
- Schedule periodic compaction (e.g., weekly during low-traffic hours)
- Monitor store size vs. expected size
- Enable auto-compaction in RocksDB (default, but verify settings)

---

## Bulk Loading Optimization

### Best Practices for Large Imports

**Symptom:**
Importing large RDF files (>1GB) is very slow or runs out of memory.

**Cause:**
Inefficient import strategy.

**Solution:**

#### 1. Use Bulk Loader

```rust
use oxigraph::io::RdfParser;
use oxigraph::store::Store;

let store = Store::new()?;
let mut loader = store.bulk_loader()?;

// Parse and load
let parser = RdfParser::from_format(RdfFormat::NTriples)
    .parse_read(std::fs::File::open("large.nt")?);

for quad in parser {
    loader.add_quad(quad?)?;
}

loader.finish()?;
```

#### 2. Optimize for Format

**Speed ranking (fastest to slowest):**
1. N-Triples / N-Quads (fastest - no parsing overhead)
2. Turtle / TriG (fast - relatively simple)
3. RDF/XML (moderate - complex parsing)
4. JSON-LD (slowest - complex parsing + JSON overhead)

**Recommendation:** Convert to N-Quads for bulk loading:

```bash
# Convert Turtle to N-Quads (faster parsing)
rapper -i turtle -o nquads input.ttl > input.nq

# Then load
oxigraph load --location /data input.nq
```

#### 3. Batch Processing

```python
import pyoxigraph as ox

store = ox.Store()

# Process file in chunks
def load_large_file(filename, chunk_size=100000):
    parser = ox.parse(open(filename, 'rb'), "application/n-quads")

    batch = []
    for quad in parser:
        batch.append(quad)

        if len(batch) >= chunk_size:
            store.extend(batch)
            batch = []
            print(f"Loaded {chunk_size} quads...")

    # Load remaining
    if batch:
        store.extend(batch)

load_large_file("large.nq")
```

#### 4. Parallel Loading (Multiple Files)

```rust
use rayon::prelude::*;

let files = vec!["part1.nq", "part2.nq", "part3.nq"];

// Load files in parallel (if independent)
files.par_iter().for_each(|file| {
    let store = Store::open("data").unwrap();
    let mut loader = store.bulk_loader().unwrap();

    let parser = RdfParser::from_format(RdfFormat::NQuads)
        .parse_read(std::fs::File::open(file).unwrap());

    for quad in parser {
        loader.add_quad(quad.unwrap()).unwrap();
    }

    loader.finish().unwrap();
});
```

**Prevention:**
- Split very large files into smaller chunks for parallel loading
- Use appropriate RDF format for your use case
- Monitor progress with logging

---

### Import Performance Benchmarks

**Expected performance (on modern hardware):**

| Format | Speed (quads/sec) | 10M quads | 100M quads |
|--------|------------------|-----------|------------|
| N-Quads (bulk) | 100K-200K | ~1-2 min | ~10-20 min |
| N-Quads (single) | 10K-20K | ~10-20 min | ~2-3 hours |
| Turtle (bulk) | 50K-100K | ~2-3 min | ~20-30 min |
| RDF/XML (bulk) | 30K-60K | ~3-6 min | ~30-60 min |
| JSON-LD (bulk) | 20K-40K | ~5-10 min | ~45-90 min |

**Hardware specs for benchmarks:**
- CPU: 8 cores, 3GHz
- RAM: 32GB
- Disk: NVMe SSD

**If your performance is worse:**
1. Check disk type (HDD vs SSD)
2. Check available memory
3. Check CPU usage (should be high during import)
4. Verify using bulk loader
5. Check for disk I/O bottleneck

---

## Benchmark Comparison

### Creating Reproducible Benchmarks

**Symptom:**
Need to compare performance across versions or configurations.

**Solution:**

```rust
use std::time::Instant;
use oxigraph::store::Store;

fn benchmark_query(store: &Store, name: &str, query: &str, iterations: usize) {
    let mut times = Vec::new();

    for _ in 0..iterations {
        let start = Instant::now();
        let count = store.query(query).unwrap().count();
        let duration = start.elapsed();
        times.push(duration.as_secs_f64());
    }

    let avg = times.iter().sum::<f64>() / times.len() as f64;
    let min = times.iter().cloned().fold(f64::INFINITY, f64::min);
    let max = times.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

    println!("{}: avg={:.3}s min={:.3}s max={:.3}s",
        name, avg, min, max);
}

// Usage
let store = Store::open("data")?;
benchmark_query(&store, "Count all", "SELECT (COUNT(*) AS ?c) WHERE { ?s ?p ?o }", 10);
benchmark_query(&store, "Simple pattern", "SELECT * WHERE { ?s a foaf:Person } LIMIT 100", 10);
```

**Output:**
```
Count all: avg=1.234s min=1.201s max=1.289s
Simple pattern: avg=0.045s min=0.042s max=0.051s
```

### Standard Benchmark Queries

```sparql
-- Q1: Count all triples
SELECT (COUNT(*) AS ?count) WHERE {
  ?s ?p ?o
}

-- Q2: Count by type
SELECT ?type (COUNT(?s) AS ?count) WHERE {
  ?s a ?type
}
GROUP BY ?type
ORDER BY DESC(?count)

-- Q3: Find resources with many properties
SELECT ?s (COUNT(?p) AS ?propCount) WHERE {
  ?s ?p ?o
}
GROUP BY ?s
HAVING (COUNT(?p) > 10)
ORDER BY DESC(?propCount)
LIMIT 100

-- Q4: Join performance
SELECT ?person ?friend ?friendOfFriend WHERE {
  ?person foaf:knows ?friend .
  ?friend foaf:knows ?friendOfFriend .
  FILTER(?person != ?friendOfFriend)
}
LIMIT 1000

-- Q5: OPTIONAL performance
SELECT ?person ?name ?email ?phone WHERE {
  ?person a foaf:Person .
  ?person foaf:name ?name .
  OPTIONAL { ?person foaf:mbox ?email }
  OPTIONAL { ?person foaf:phone ?phone }
}
LIMIT 1000
```

### Comparing Versions

```bash
#!/bin/bash
# benchmark.sh - Compare two Oxigraph versions

QUERIES="queries/*.sparql"
STORE="/data/oxigraph"

for query in $QUERIES; do
    echo "Testing $query..."

    # Version 1
    echo -n "v0.3.0: "
    /usr/local/oxigraph-0.3.0 query --location $STORE --query $query

    # Version 2
    echo -n "v0.4.0: "
    /usr/local/oxigraph-0.4.0 query --location $STORE --query $query

    echo "---"
done
```

**Prevention:**
- Establish baseline benchmarks before changes
- Run benchmarks on representative data
- Test with various query types (simple, complex, aggregations)
- Document hardware specifications

---

## Performance Checklist

Before reporting performance issues, verify:

- [ ] Using latest Oxigraph version
- [ ] Enabled debug logging to identify bottleneck
- [ ] Checked query plan for cartesian products
- [ ] Using appropriate RDF format for operation
- [ ] Using bulk loader for large imports
- [ ] Data stored on SSD (not HDD)
- [ ] Sufficient RAM for dataset size (estimate 5-10x raw data)
- [ ] Disk has free space (>20% free)
- [ ] Not hitting disk I/O limits (checked with iostat)
- [ ] Tried query optimization techniques
- [ ] Created minimal reproducible benchmark
- [ ] Compared with expected performance numbers

---

**Still experiencing performance issues?** Report with:
1. Benchmark results from this guide
2. Query plans (with RUST_LOG=spareval=debug)
3. System specifications (CPU, RAM, disk type)
4. Dataset size and characteristics
5. Oxigraph version

See [troubleshooting index](index.md) for where to get help.
