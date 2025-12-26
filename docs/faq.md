# Frequently Asked Questions (FAQ)

Common questions about Oxigraph, organized by topic.

## Table of Contents

- [General Questions](#general-questions)
- [Performance](#performance)
- [Features & Compatibility](#features--compatibility)
- [Data Loading & Management](#data-loading--management)
- [SPARQL Queries](#sparql-queries)
- [Storage & Persistence](#storage--persistence)
- [Deployment](#deployment)
- [Migration & Integration](#migration--integration)
- [Troubleshooting](#troubleshooting)
- [Development](#development)

---

## General Questions

### What is Oxigraph?

Oxigraph is a graph database that implements the SPARQL standard. It's written in Rust and provides:
- A high-performance RDF/SPARQL database
- Persistent storage (RocksDB) and in-memory options
- Multiple language bindings (Rust, Python, JavaScript)
- A standalone SPARQL server
- Full compliance with W3C standards

### Why choose Oxigraph over other triple stores?

**Advantages:**
- **Performance**: Fast query execution with optimized indexing (SPO, POS, OSP)
- **Portability**: Written in Rust, runs anywhere (desktop, server, browser via WASM)
- **Standards compliance**: Passes W3C test suites for SPARQL 1.1
- **Easy deployment**: Single binary, Docker image, or library
- **Multiple interfaces**: Use as library or HTTP server
- **Active development**: Regular updates and improvements

**Best for:**
- Embedded databases in applications
- Medium-sized datasets (millions to billions of triples)
- Projects needing Rust/Python/JavaScript integration
- Scenarios requiring portability and ease of deployment

### Is Oxigraph production-ready?

Oxigraph is stable for many use cases, but consider:

**Production-ready aspects:**
- SPARQL 1.1 compliance
- ACID transactions
- Persistent storage
- Active maintenance

**Development aspects:**
- Query optimizer is still being improved
- Some advanced features are experimental
- Performance tuning is ongoing

**Recommendation**: Use in production with testing, monitor performance, and have a backup strategy.

### What license is Oxigraph under?

Dual-licensed under Apache 2.0 and MIT. You can choose either license.

This means you can use Oxigraph in commercial projects, modify it, and distribute it freely.

### How does Oxigraph compare to...

#### vs. Apache Jena

- **Oxigraph**: Written in Rust, lightweight, good for embedding
- **Jena**: Written in Java, mature ecosystem, extensive tooling
- **Choose Oxigraph if**: You need a lightweight, embeddable solution
- **Choose Jena if**: You need mature Java integration and extensive plugins

#### vs. Virtuoso

- **Oxigraph**: Open-source, lightweight, modern codebase
- **Virtuoso**: Enterprise-grade, highly optimized, more features
- **Choose Oxigraph if**: You need simplicity and easy deployment
- **Choose Virtuoso if**: You need maximum performance for huge datasets (100B+ triples)

#### vs. Blazegraph

- **Oxigraph**: Active development, modern Rust codebase
- **Blazegraph**: Mature but no longer actively maintained
- **Choose Oxigraph if**: You want active development and support
- **Choose Blazegraph if**: You need specific features it provides

#### vs. RDFLib (Python)

- **Oxigraph**: Much faster, persistent storage, full SPARQL 1.1
- **RDFLib**: Pure Python, easier to extend, more Python-native
- **Choose Oxigraph if**: You need performance and persistence
- **Choose RDFLib if**: You need pure Python and extensive customization

---

## Performance

### How fast is Oxigraph?

Performance varies by dataset and query, but generally:

- **Data loading**: 10K-100K triples/second (bulk load faster)
- **Simple queries**: Sub-millisecond for small result sets
- **Complex queries**: Depends on optimizer, typically seconds
- **Dataset size**: Tested with billions of triples

See [benchmarks](../bench/README.md) for detailed comparisons.

### How can I improve query performance?

1. **Use bulk loading** for initial data import:
   ```bash
   oxigraph load --location ./db --file data.nq
   ```

2. **Optimize SPARQL queries**:
   - Put most selective patterns first
   - Use FILTER after other patterns
   - Avoid OPTIONAL when possible
   - Use LIMIT when you don't need all results

3. **Use appropriate RDF formats**:
   - N-Triples/N-Quads: Fastest to parse
   - Turtle/TriG: Good balance
   - RDF/XML: Slowest to parse

4. **Consider data modeling**:
   - Avoid extremely large literals
   - Use appropriate datatypes
   - Consider graph organization

5. **System resources**:
   - More RAM = better caching
   - SSD storage for better I/O
   - Increase RocksDB cache size

### How can I improve data loading speed?

Use bulk loading instead of transactional inserts:

```bash
# Fast: Bulk load
oxigraph load --location ./db --file huge-dataset.nq

# Slow: Transactional insert
cat huge-dataset.nq | while read line; do
  curl -X POST ... # Individual inserts
done
```

In code, batch your operations:

```python
# Python
store = Store("./db")
store.bulk_load(path="data.nq", format=RdfFormat.N_QUADS)
```

### Does Oxigraph support parallel queries?

Yes! Oxigraph uses thread-safe data structures and supports concurrent queries.

Multiple threads/processes can query simultaneously. Write operations are serialized through transactions.

### What's the maximum dataset size?

**Theoretical limit**: Limited by disk space (RocksDB can handle petabytes)

**Practical limits**:
- **Tested**: Billions of triples
- **Recommended**: Millions to low billions for optimal performance
- **Memory**: ~2-3x raw data size on disk for RocksDB

For very large datasets (100B+ triples), consider:
- Partitioning data across multiple stores
- Using more powerful hardware
- Enterprise solutions like Virtuoso

---

## Features & Compatibility

### Which SPARQL features are supported?

**Fully supported:**
- SPARQL 1.1 Query
- SPARQL 1.1 Update
- SPARQL 1.1 Federated Query (with `http-client` feature)
- All standard functions and operators
- Property paths
- Aggregates (COUNT, SUM, AVG, etc.)
- Subqueries

**Experimental:**
- SPARQL 1.2 (with `rdf-12` feature flag)
- GeoSPARQL (in JavaScript with `geosparql` feature)

**Not supported:**
- SPARQL 1.1 LOAD operation in UPDATE (security concerns)

### Are RDF 1.2 and SPARQL 1.2 supported?

Yes, with the `rdf-12` feature flag:

```toml
oxigraph = { version = "0.4", features = ["rdf-12"] }
```

This enables:
- Directional language tags
- Triple terms
- SPARQL 1.2 features

### Does Oxigraph support RDF-star/SPARQL-star?

Yes! RDF-star (quoted triples) is supported by default:

```sparql
# Create a statement about a statement
INSERT DATA {
  << :Bob :age 42 >> :source :Census2020 .
}

# Query quoted triples
SELECT * WHERE {
  << ?s ?p ?o >> :source ?source .
}
```

### What RDF formats are supported?

**Reading and writing:**
- Turtle (.ttl)
- TriG (.trig)
- N-Triples (.nt)
- N-Quads (.nq)
- RDF/XML (.rdf)
- JSON-LD (.jsonld)
- N3 (.n3)

**Query results:**
- SPARQL JSON
- SPARQL XML
- SPARQL CSV
- SPARQL TSV
- RDF formats (for CONSTRUCT/DESCRIBE)

### Does Oxigraph support inference/reasoning?

No built-in reasoning/inference engine. Oxigraph is a database, not a reasoner.

**Alternatives:**
- Pre-materialize inferences before loading
- Use external reasoners (e.g., EYE, Jena) and load results
- Implement reasoning in your application layer

### Does Oxigraph support named graphs?

Yes! Full support for named graphs (quads):

```rust
use oxigraph::model::*;

let graph = NamedNode::new("http://example.com/graph1")?;
let quad = Quad::new(subject, predicate, object, graph);
store.insert(&quad)?;
```

```sparql
# Query specific graph
SELECT * FROM <http://example.com/graph1> WHERE { ?s ?p ?o }

# Insert into graph
INSERT DATA {
  GRAPH <http://example.com/graph1> {
    :subject :predicate "object"
  }
}
```

### Does Oxigraph support full-text search?

Not built-in. SPARQL `FILTER` with `regex()` works for simple cases:

```sparql
SELECT * WHERE {
  ?s rdfs:label ?label .
  FILTER(regex(?label, "search term", "i"))
}
```

For advanced full-text search:
- Extract text to dedicated full-text search engine (Elasticsearch, etc.)
- Use external index and join results
- Consider extensions (community contributions welcome!)

### Can I use Oxigraph in the browser?

Yes! JavaScript bindings compile to WebAssembly:

```javascript
import init, * as oxigraph from 'oxigraph/web.js';

await init();
const store = new oxigraph.Store();
// Now use the store...
```

**Limitations:**
- In-memory only (no persistent storage in browser)
- Dataset size limited by browser memory
- Performance may be slower than native

---

## Data Loading & Management

### How do I load data into Oxigraph?

**CLI (bulk load, fastest):**
```bash
oxigraph load --location ./db --file data.nq
```

**HTTP API:**
```bash
curl -X POST -H 'Content-Type: text/turtle' \
  -T data.ttl http://localhost:7878/store
```

**Rust:**
```rust
store.load_from_path("data.ttl")?;
// Or bulk load
store.bulk_loader().load_from_path("data.nq")?;
```

**Python:**
```python
store.load(path="data.ttl", format=RdfFormat.TURTLE)
# Or bulk load
store.bulk_load(path="data.nq", format=RdfFormat.N_QUADS)
```

### Can I load data from URLs?

**Python** (easiest):
```python
import requests
from pyoxigraph import Store, RdfFormat

store = Store()
response = requests.get("https://example.com/data.ttl")
store.load(input=response.content, format=RdfFormat.TURTLE)
```

**JavaScript:**
```javascript
const response = await fetch("https://example.com/data.ttl");
const data = await response.text();
store.load(input=data, { format: "text/turtle" });
```

**CLI with SPARQL UPDATE:**
```bash
# Via SERVICE (requires http-client feature in custom build)
curl -X POST http://localhost:7878/update \
  -H 'Content-Type: application/sparql-update' \
  --data 'LOAD <https://example.com/data.ttl>'
```

Note: LOAD in SPARQL UPDATE is disabled by default in the server for security.

### How do I back up my database?

**Export to N-Quads** (preserves everything):
```bash
# CLI
oxigraph dump --location ./db --format nq > backup.nq

# HTTP API
curl http://localhost:7878/store > backup.nq
```

**File system backup** (while server is stopped):
```bash
# Stop the server first!
systemctl stop oxigraph
# or docker stop oxigraph

# Copy the data directory
tar -czf backup.tar.gz ./db-directory/

# Restart
systemctl start oxigraph
```

**Continuous backup** with RocksDB snapshots (advanced):
See RocksDB documentation for checkpoint API.

### How do I migrate data between Oxigraph instances?

**Method 1: Export/Import**
```bash
# Export from source
oxigraph dump --location ./source-db --format nq > export.nq

# Import to destination
oxigraph load --location ./dest-db --file export.nq
```

**Method 2: Direct copy** (if versions match):
```bash
# Stop both instances
# Copy data directory
cp -r ./source-db ./dest-db
```

### Can I update the database while queries are running?

Yes! Oxigraph uses "repeatable read" isolation:
- Queries see a consistent snapshot
- Updates don't block queries (mostly)
- Multiple queries can run concurrently
- Updates are serialized and atomic

### How do I delete all data?

**Delete everything:**
```bash
# SPARQL Update
curl -X POST http://localhost:7878/update \
  -H 'Content-Type: application/sparql-update' \
  --data 'DELETE WHERE { ?s ?p ?o }'
```

**Delete specific graph:**
```sparql
DROP GRAPH <http://example.com/graph>
```

**Delete and recreate database:**
```bash
# Stop server, delete directory, restart
rm -rf ./db-directory
oxigraph serve --location ./db-directory
```

---

## SPARQL Queries

### Why is my query slow?

**Common issues:**

1. **Missing selectivity**: Put selective patterns first
   ```sparql
   # Slow: Broad pattern first
   SELECT * WHERE {
     ?s a ?type .              # Matches everything
     ?s :specificProperty 42 . # Selective
   }

   # Fast: Selective pattern first
   SELECT * WHERE {
     ?s :specificProperty 42 . # Selective
     ?s a ?type .              # Now filtered
   }
   ```

2. **Complex OPTIONAL**: Use only when necessary
   ```sparql
   # Slow: Multiple OPTIONALs
   SELECT * WHERE {
     ?s ?p ?o .
     OPTIONAL { ?s :prop1 ?v1 }
     OPTIONAL { ?s :prop2 ?v2 }
     OPTIONAL { ?s :prop3 ?v3 }
   }
   ```

3. **Inefficient FILTERs**: Use property paths instead
   ```sparql
   # Slow
   SELECT * WHERE {
     ?s ?p ?o .
     FILTER(?p = :prop1 || ?p = :prop2)
   }

   # Fast
   SELECT * WHERE {
     ?s :prop1|:prop2 ?o .
   }
   ```

4. **Large result sets**: Use LIMIT
   ```sparql
   SELECT * WHERE { ?s ?p ?o } LIMIT 1000
   ```

### How do I debug SPARQL queries?

**Simplify incrementally:**
```sparql
# Start simple
SELECT * WHERE { ?s a :Person } LIMIT 10

# Add one pattern at a time
SELECT * WHERE {
  ?s a :Person ;
     :name ?name .
} LIMIT 10

# Add more complexity gradually
```

**Check intermediate results:**
```sparql
# See what each variable binds to
SELECT ?s ?p ?o WHERE {
  ?s ?p ?o .
  FILTER(?s = :specificSubject)
}
```

**Use COUNT for testing:**
```sparql
# How many results?
SELECT (COUNT(*) AS ?count) WHERE {
  ?s a :Person .
}
```

**Enable query logging** (server):
```bash
RUST_LOG=debug oxigraph serve --location ./db
```

### How do I query across multiple named graphs?

**Union of specific graphs:**
```sparql
SELECT * WHERE {
  GRAPH ?g {
    ?s ?p ?o .
    FILTER(?g IN (<http://graph1>, <http://graph2>))
  }
}
```

**Query all graphs:**
```sparql
SELECT * WHERE {
  GRAPH ?g { ?s ?p ?o }
}
```

**Use default graph as union** (server option):
```bash
curl -X POST http://localhost:7878/query \
  -H 'Content-Type: application/sparql-query' \
  --data 'SELECT * WHERE { ?s ?p ?o }' \
  --data-urlencode 'default-graph-uri=urn:sparql:use-default-graph-as-union'
```

### Can I use custom SPARQL functions?

Not directly. SPARQL functions are hardcoded.

**Workarounds:**
- Use built-in functions creatively
- Pre-process data to include computed values
- Post-process query results in application code
- Contribute a PR to add functions!

### Does Oxigraph support federated queries (SERVICE)?

Yes, with the `http-client` feature in Rust:

```toml
oxigraph = { version = "0.4", features = ["http-client"] }
```

Then:
```sparql
SELECT * WHERE {
  ?s ?p ?o .
  SERVICE <http://dbpedia.org/sparql> {
    ?s rdfs:label ?label .
  }
}
```

**Note**: Not enabled by default in pre-built CLI server for security.

---

## Storage & Persistence

### Where is data stored?

**CLI Server:**
```bash
oxigraph serve --location /path/to/database
# Data stored in: /path/to/database/
```

**Rust:**
```rust
Store::open("./my-database")?;
// Data stored in: ./my-database/
```

**Python:**
```python
Store("./my-database")
# Data stored in: ./my-database/
```

**JavaScript:**
In-memory only, no persistence in WASM.

### What's the on-disk format?

RocksDB (LSM-tree key-value store) with custom indexing:
- 3 indexes: SPO, POS, OSP
- Optimized for RDF quad patterns
- Binary serialization for efficiency

**Important**: Format is specific to Oxigraph version. Use export/import for migration.

### Can I use an in-memory store?

**Rust:**
```rust
// Without RocksDB feature
Store::new()?  // Always in-memory

// With RocksDB feature (default)
Store::new()?  // In-memory
Store::open("path")?  // Persistent
```

**Python:**
```python
Store()  # In-memory
Store("path")  # Persistent
```

**JavaScript:**
Always in-memory (WASM limitation).

### How much disk space will my data use?

Rule of thumb: **2-3x the size of raw N-Quads**

Example:
- 10M triples in N-Quads: ~1.5 GB
- Oxigraph storage: ~3-4 GB

Factors affecting size:
- RocksDB overhead (indexes, logs)
- Literal length (long strings take more space)
- Compression (RocksDB uses Snappy)
- Named graphs (quads vs triples)

### Can I use a different storage backend?

Not currently. Oxigraph is tightly coupled to RocksDB.

Future possibilities:
- Other key-value stores (community contributions welcome)
- Custom backends via abstraction layer

For now: RocksDB or in-memory only.

### How do I optimize storage size?

1. **Compact the database:**
   RocksDB compaction happens automatically, but you can trigger it by reopening:
   ```bash
   # Stop server
   # Restart server (triggers compaction)
   ```

2. **Use bulk loading:**
   Faster and creates more compact storage:
   ```bash
   oxigraph load --location ./db --file data.nq
   ```

3. **Avoid redundant data:**
   - Don't duplicate triples across graphs unnecessarily
   - Use blank nodes sparingly (they're stored as full URIs internally)

---

## Deployment

### How do I deploy Oxigraph in production?

**Docker (recommended):**
```bash
docker run -d \
  --name oxigraph \
  --restart always \
  -v /path/to/data:/data \
  -p 7878:7878 \
  ghcr.io/oxigraph/oxigraph:latest \
  serve --location /data --bind 0.0.0.0:7878
```

**Systemd service:**
```ini
[Unit]
Description=Oxigraph SPARQL Server
After=network.target

[Service]
Type=notify
ExecStart=/usr/local/bin/oxigraph serve --location /var/lib/oxigraph
Restart=always
User=oxigraph

[Install]
WantedBy=multi-user.target
```

**Kubernetes:**
See community examples in discussions.

### How do I secure the Oxigraph server?

**1. Use a reverse proxy** (nginx, Apache, Caddy):
```nginx
server {
    listen 80;
    server_name sparql.example.com;

    location /query {
        # Public queries
        proxy_pass http://localhost:7878/query;
    }

    location /update {
        # Protected updates
        auth_basic "Restricted";
        auth_basic_user_file /etc/nginx/.htpasswd;
        proxy_pass http://localhost:7878/update;
    }
}
```

**2. Firewall rules:**
```bash
# Only allow localhost
oxigraph serve --bind 127.0.0.1:7878

# Let reverse proxy handle external access
```

**3. Container isolation:**
```bash
# Docker with network isolation
docker run --network internal oxigraph ...
```

**4. Rate limiting** (at reverse proxy level)

**Note**: Oxigraph itself has no authentication/authorization built-in.

### How do I monitor Oxigraph?

**Logs:**
```bash
# Enable debug logging
RUST_LOG=info oxigraph serve --location ./db

# Or with systemd
journalctl -u oxigraph -f
```

**Metrics** (not built-in yet):
- Monitor disk I/O
- Monitor memory usage
- Monitor HTTP response times (at proxy level)
- Monitor query count (at proxy level)

**Health checks:**
```bash
# Simple check
curl http://localhost:7878/query \
  -H 'Content-Type: application/sparql-query' \
  --data 'ASK { ?s ?p ?o }'
```

### Should I use one Oxigraph instance per tenant?

**Multi-tenant strategies:**

1. **One store, multiple graphs:**
   ```sparql
   # Tenant 1 data
   GRAPH <http://tenant1/data> { ... }

   # Tenant 2 data
   GRAPH <http://tenant2/data> { ... }
   ```
   - Pros: Simple, shared resources
   - Cons: No isolation, shared query load

2. **Separate Oxigraph instances:**
   - Pros: Complete isolation, independent scaling
   - Cons: More resource overhead

3. **Hybrid**: Separate instances for large tenants, shared for small ones

### How do I scale Oxigraph horizontally?

**Current limitations**: No built-in clustering/replication.

**Workarounds:**

1. **Read replicas** (manual):
   - Export/import to create copies
   - Route reads to replicas
   - Route writes to primary
   - Periodically re-sync replicas

2. **Sharding by graph**:
   - Different graphs on different instances
   - Application-level routing
   - Federation with SERVICE keyword

3. **Load balancer for read-only**:
   - Multiple identical copies
   - Read-only workloads
   - Shared updates via batch exports

**Future**: Clustering/replication may be added later.

---

## Migration & Integration

### How do I migrate from Apache Jena?

**Export from Jena:**
```bash
# Using tdbdump
tdbdump --loc=/path/to/jena/db > export.nq

# Or via SPARQL
curl -X POST http://localhost:3030/dataset/query \
  -H 'Accept: application/n-quads' \
  --data 'CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }' \
  > export.nq
```

**Import to Oxigraph:**
```bash
oxigraph load --location ./oxigraph-db --file export.nq
```

### How do I migrate from Virtuoso?

**Export from Virtuoso:**
```sql
-- In Virtuoso iSQL
DUMP_NQUADS_TO_FILE('/path/to/export.nq');
```

**Import to Oxigraph:**
```bash
oxigraph load --location ./oxigraph-db --file export.nq
```

### Can I use Oxigraph with RDFLib?

Yes! Use [oxrdflib](https://github.com/oxigraph/oxrdflib):

```python
from oxrdflib import OxigraphStore
from rdflib import Graph

# Create RDFLib graph with Oxigraph backend
g = Graph(store=OxigraphStore())

# Use like normal RDFLib
g.parse("data.ttl")
for s, p, o in g:
    print(s, p, o)
```

### Can I use Oxigraph as an RDF/JS store?

Yes! Oxigraph implements the [RDF/JS DataFactory interface](http://rdf.js.org/data-model-spec/):

```javascript
import oxigraph from 'oxigraph';

// Create terms using RDF/JS interface
const quad = oxigraph.quad(
    oxigraph.namedNode("http://example.com/s"),
    oxigraph.namedNode("http://example.com/p"),
    oxigraph.literal("value")
);

// Terms are RDF/JS compatible
console.log(quad.subject.termType); // "NamedNode"
console.log(quad.subject.value);    // "http://example.com/s"
```

### How do I integrate Oxigraph with my application?

**Rust** (embedded):
```rust
// Add to Cargo.toml
use oxigraph::store::Store;

let store = Store::open("./db")?;
// Use directly in your app
```

**Python** (embedded):
```python
from pyoxigraph import Store, RdfFormat

store = Store("./db")
# Use in your app
```

**JavaScript** (embedded):
```javascript
const oxigraph = require('oxigraph');
const store = new oxigraph.Store();
// Use in your app
```

**Any language** (via HTTP):
```bash
# Start server
oxigraph serve --location ./db

# Call from any HTTP client
curl http://localhost:7878/query ...
```

---

## Troubleshooting

### "RocksDB: IO error" when opening database

**Causes:**
- Permission issues
- Disk full
- Corrupted database
- Database created by different Oxigraph version

**Solutions:**
```bash
# Check permissions
ls -la ./db-directory
chmod -R 755 ./db-directory

# Check disk space
df -h

# Try to recover (backup first!)
oxigraph serve --location ./db-directory 2>&1 | tee error.log

# Last resort: export if possible, recreate
```

### "Out of memory" errors

**Solutions:**

1. **Increase system memory**

2. **Reduce query result size**:
   ```sparql
   SELECT * WHERE { ?s ?p ?o } LIMIT 1000
   ```

3. **Use streaming** (Rust):
   ```rust
   // Don't collect all results
   for quad in store.quads_for_pattern(None, None, None, None) {
       let quad = quad?;
       // Process one at a time
   }
   ```

4. **Batch operations**:
   Instead of loading huge files at once, split them.

### Query returns no results but data exists

**Check:**

1. **Exact IRIs**:
   ```sparql
   # Wrong: :name instead of :Name
   SELECT * WHERE { ?s :name ?o }

   # Right
   SELECT * WHERE { ?s :Name ?o }
   ```

2. **Correct graph**:
   ```sparql
   # If data is in named graph
   SELECT * FROM <http://example.com/graph> WHERE { ?s ?p ?o }

   # Or
   SELECT * WHERE {
     GRAPH <http://example.com/graph> { ?s ?p ?o }
   }
   ```

3. **Prefixes**:
   ```sparql
   PREFIX ex: <http://example.com/>
   SELECT * WHERE { ?s ex:property ?o }
   ```

4. **Query the default graph vs named graphs**:
   ```sparql
   # See what graphs exist
   SELECT DISTINCT ?g WHERE {
     GRAPH ?g { ?s ?p ?o }
   }
   ```

### Python: "ImportError: cannot import name 'Store'"

**Solutions:**

```bash
# Reinstall
pip uninstall pyoxigraph
pip install --no-cache-dir pyoxigraph

# Check Python version (3.8+ required)
python --version

# Try conda instead
conda install -c conda-forge pyoxigraph
```

### JavaScript: "WebAssembly module is not available"

**Solution:**

```javascript
// Browser: initialize WASM first
import init, * as oxigraph from 'oxigraph/web.js';

await init(); // Required!
const store = new oxigraph.Store();
```

### Server won't start: "Address already in use"

**Solution:**

```bash
# Find what's using the port
lsof -i :7878  # Linux/macOS
netstat -ano | findstr :7878  # Windows

# Use different port
oxigraph serve --location ./db --bind localhost:8080

# Or kill the process
kill -9 <PID>  # Linux/macOS
taskkill /PID <PID> /F  # Windows
```

---

## Development

### How can I contribute to Oxigraph?

See [CONTRIBUTING.md](CONTRIBUTING.md) for details!

**Ways to contribute:**
- Report bugs
- Improve documentation
- Add tests
- Optimize performance
- Add features
- Fix issues

### How do I add a new SPARQL function?

1. Add to `lib/spareval/src/function.rs`
2. Register in function registry
3. Add tests
4. Update documentation
5. Submit PR

See existing functions for examples.

### How do I add a new RDF format?

1. Create parser in `lib/ox<format>/`
2. Register in `lib/oxrdfio/src/format.rs`
3. Update `RdfFormat` enum
4. Add W3C test suite if available
5. Update documentation
6. Submit PR

### Can I use Oxigraph in commercial projects?

**Yes!** Oxigraph is dual-licensed under Apache 2.0 and MIT.

Both licenses allow commercial use, modification, and distribution.

**Requirements:**
- Include license notice
- That's it!

No attribution required (but appreciated).

### Where can I get help?

- [GitHub Discussions](https://github.com/oxigraph/oxigraph/discussions) - Best for questions
- [Gitter Chat](https://gitter.im/oxigraph/community) - Real-time chat
- [GitHub Issues](https://github.com/oxigraph/oxigraph/issues) - Bug reports
- [Stack Overflow](https://stackoverflow.com/questions/tagged/oxigraph) - Tag: `oxigraph`

For commercial support: Contact [@Tpt](https://github.com/Tpt)

---

## Still Have Questions?

If your question isn't answered here:

1. Search [GitHub Issues](https://github.com/oxigraph/oxigraph/issues)
2. Search [GitHub Discussions](https://github.com/oxigraph/oxigraph/discussions)
3. Ask in [Gitter Chat](https://gitter.im/oxigraph/community)
4. Start a new [Discussion](https://github.com/oxigraph/oxigraph/discussions/new)

We're here to help!
