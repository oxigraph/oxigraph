# Configuration Reference

This document details all configuration options for Oxigraph, including CLI parameters, environment variables, store configuration, and server settings.

## Table of Contents

- [CLI Configuration](#cli-configuration)
- [Store Configuration](#store-configuration)
- [Server Configuration](#server-configuration)
- [Environment Variables](#environment-variables)
- [Feature Flags](#feature-flags)
- [Performance Tuning](#performance-tuning)

---

## CLI Configuration

The Oxigraph CLI (`oxigraph` or `oxigraph-cli`) provides several commands with their own configuration options.

### Global Options

```bash
oxigraph --version    # Show version information
oxigraph --help       # Show help information
```

---

### serve - HTTP Server (Read-Write)

Start an HTTP SPARQL server in read-write mode.

**Syntax**:
```bash
oxigraph serve [OPTIONS]
```

**Options**:

| Option | Short | Type | Default | Description |
|--------|-------|------|---------|-------------|
| `--location` | `-l` | Path | None | Directory for persistent data storage |
| `--bind` | `-b` | Host:Port | `localhost:7878` | Host and port to listen on |
| `--cors` | | Flag | false | Enable CORS (Cross-Origin Resource Sharing) |
| `--union-default-graph` | | Flag | false | Query all graphs by default (union behavior) |
| `--timeout-s` | | Seconds | None | Query timeout in seconds |

**Examples**:

```bash
# In-memory server (data lost on exit)
oxigraph serve

# Persistent server
oxigraph serve --location /var/lib/oxigraph

# Custom bind address
oxigraph serve --bind 0.0.0.0:8080

# With CORS enabled
oxigraph serve --location ./data --cors

# Union default graph with timeout
oxigraph serve --location ./data --union-default-graph --timeout-s 30
```

**Server Endpoints**:
- `GET /query` - SPARQL Query endpoint
- `POST /query` - SPARQL Query endpoint
- `POST /update` - SPARQL Update endpoint
- `GET|POST|PUT|DELETE /store` - SPARQL Graph Store Protocol

---

### serve-read-only - HTTP Server (Read-Only)

Start an HTTP SPARQL server in read-only mode.

**Syntax**:
```bash
oxigraph serve-read-only [OPTIONS]
```

**Options**:

| Option | Short | Type | Required | Description |
|--------|-------|------|----------|-------------|
| `--location` | `-l` | Path | Yes | Directory with Oxigraph data |
| `--bind` | `-b` | Host:Port | No | Host and port (default: `localhost:7878`) |
| `--cors` | | Flag | No | Enable CORS |
| `--union-default-graph` | | Flag | No | Query all graphs by default |
| `--timeout-s` | | Seconds | No | Query timeout in seconds |

**Example**:
```bash
oxigraph serve-read-only --location /var/lib/oxigraph --bind 0.0.0.0:8080
```

**Warning**: Opening as read-only while another process is writing is undefined behavior.

---

### load - Load RDF Files

Load RDF file(s) into the database.

**Syntax**:
```bash
oxigraph load [OPTIONS]
```

**Options**:

| Option | Short | Type | Description |
|--------|-------|------|-------------|
| `--location` | `-l` | Path | Directory for Oxigraph data (required) |
| `--file` | `-f` | Path(s) | File(s) to load (multiple allowed) |
| `--format` | | String | RDF format (extension or MIME type) |
| `--base` | | IRI | Base IRI for relative IRIs |
| `--graph` | | IRI | Target named graph (graphs only, not datasets) |
| `--non-atomic` | | Flag | Save during loading (reduces disk usage) |
| `--lenient` | | Flag | Attempt to parse invalid RDF |

**Format Detection**:
- Auto-detected from file extension if `--file` provided
- Must specify `--format` when reading from stdin

**Examples**:

```bash
# Load single file
oxigraph load --location ./data --file data.ttl

# Load multiple files in parallel
oxigraph load --location ./data --file file1.ttl --file file2.ttl --file file3.nt

# Load from stdin
cat data.ttl | oxigraph load --location ./data --format text/turtle

# Load into named graph
oxigraph load --location ./data --file data.ttl --graph http://example.org/graph

# Lenient loading (for Wikidata dumps, etc.)
oxigraph load --location ./data --file wikidata.nt.gz --lenient

# Non-atomic loading (saves disk space)
oxigraph load --location ./data --file large.nt --non-atomic
```

**Supported Formats**:
- Turtle (`.ttl`, `text/turtle`)
- N-Triples (`.nt`, `application/n-triples`)
- N-Quads (`.nq`, `application/n-quads`)
- TriG (`.trig`, `application/trig`)
- RDF/XML (`.rdf`, `application/rdf+xml`)
- JSON-LD (`.jsonld`, `application/ld+json`)

---

### dump - Export Database

Dump database content to file.

**Syntax**:
```bash
oxigraph dump [OPTIONS]
```

**Options**:

| Option | Short | Type | Description |
|--------|-------|------|-------------|
| `--location` | `-l` | Path | Directory with Oxigraph data (required) |
| `--file` | `-f` | Path | Output file (uses stdout if omitted) |
| `--format` | | String | Output format (extension or MIME type) |
| `--graph` | | IRI | Dump specific named graph only |

**Examples**:

```bash
# Dump entire database to N-Quads
oxigraph dump --location ./data --file dump.nq

# Dump to stdout
oxigraph dump --location ./data --format application/n-quads

# Dump specific graph
oxigraph dump --location ./data --file graph.ttl --graph http://example.org/graph

# Dump default graph
oxigraph dump --location ./data --file default.ttl --graph default
```

---

### query - Execute SPARQL Query

Execute a SPARQL query against the database.

**Syntax**:
```bash
oxigraph query [OPTIONS]
```

**Options**:

| Option | Type | Description |
|--------|------|-------------|
| `--location` | Path | Directory with Oxigraph data (required) |
| `--query` | String | SPARQL query string |
| `--query-file` | Path | File containing SPARQL query |
| `--query-base` | IRI | Base IRI for query |
| `--results-file` | Path | Output file for results |
| `--results-format` | String | Results format |
| `--explain` | Flag | Print query explanation to stderr |
| `--explain-file` | Path | Write query explanation to file |
| `--stats` | Flag | Include execution statistics in explanation |
| `--union-default-graph` | Flag | Query all graphs by default |

**Examples**:

```bash
# Query from string
oxigraph query --location ./data --query "SELECT * WHERE { ?s ?p ?o } LIMIT 10"

# Query from file
oxigraph query --location ./data --query-file query.sparql

# Save results to file
oxigraph query --location ./data --query-file query.sparql --results-file results.json

# With query explanation
oxigraph query --location ./data --query-file query.sparql --explain --stats

# Union default graph
oxigraph query --location ./data --query "SELECT * { ?s ?p ?o }" --union-default-graph
```

**Results Formats**:
- JSON (`.srj`, `application/sparql-results+json`)
- XML (`.srx`, `application/sparql-results+xml`)
- CSV (`.csv`, `text/csv`)
- TSV (`.tsv`, `text/tab-separated-values`)
- For CONSTRUCT: Any RDF format

**Explanation Formats**:
- `.txt` - Human-readable text
- `.json` - Machine-readable JSON

---

### update - Execute SPARQL Update

Execute a SPARQL update operation.

**Syntax**:
```bash
oxigraph update [OPTIONS]
```

**Options**:

| Option | Type | Description |
|--------|------|-------------|
| `--location` | Path | Directory with Oxigraph data (required) |
| `--update` | String | SPARQL update string |
| `--update-file` | Path | File containing SPARQL update |
| `--update-base` | IRI | Base IRI for update |

**Examples**:

```bash
# Update from string
oxigraph update --location ./data --update "DELETE WHERE { ?s ?p ?o }"

# Update from file
oxigraph update --location ./data --update-file update.sparql

# With base IRI
oxigraph update --location ./data --update-file update.sparql --update-base http://example.org/
```

---

### backup - Create Backup

Create a database backup.

**Syntax**:
```bash
oxigraph backup [OPTIONS]
```

**Options**:

| Option | Short | Type | Description |
|--------|-------|------|-------------|
| `--location` | `-l` | Path | Source database directory (required) |
| `--destination` | `-d` | Path | Backup destination directory (required) |

**Example**:
```bash
oxigraph backup --location ./data --destination ./backup
```

**Features**:
- Creates independent, usable Oxigraph database
- Uses hard links if on same filesystem (cheap, fast)
- Immutable snapshots (safe for concurrent reads)

---

### optimize - Optimize Database

Optimize database storage (compaction, compression).

**Syntax**:
```bash
oxigraph optimize [OPTIONS]
```

**Options**:

| Option | Short | Type | Description |
|--------|-------|------|-------------|
| `--location` | `-l` | Path | Database directory (required) |

**Example**:
```bash
oxigraph optimize --location ./data
```

**Note**: Usually not needed; optimization runs automatically during `serve`.

---

### convert - Convert RDF Formats

Convert RDF files between formats.

**Syntax**:
```bash
oxigraph convert [OPTIONS]
```

**Options**:

| Option | Type | Description |
|--------|------|-------------|
| `--from-file` | Path | Input file (stdin if omitted) |
| `--from-format` | String | Input format |
| `--from-base` | IRI | Base IRI for input |
| `--from-graph` | IRI | Only load specific named graph |
| `--from-default-graph` | Flag | Only load default graph |
| `--to-file` | Path | Output file (stdout if omitted) |
| `--to-format` | String | Output format |
| `--to-base` | IRI | Base IRI for output |
| `--to-graph` | IRI | Map default graph to named graph |
| `--lenient` | Flag | Lenient parsing |

**Examples**:

```bash
# Convert Turtle to N-Triples
oxigraph convert --from-file data.ttl --to-file data.nt

# Convert with format specification
oxigraph convert --from-file data.rdf --from-format application/rdf+xml --to-format text/turtle

# Extract named graph
oxigraph convert --from-file dataset.trig --from-graph http://example.org/graph --to-file graph.ttl

# Stdin/stdout pipeline
cat data.ttl | oxigraph convert --from-format text/turtle --to-format application/n-triples > data.nt
```

---

## Store Configuration

### Rust API

```rust
use oxigraph::store::Store;

// In-memory store
let store = Store::new()?;

// Persistent store
let store = Store::open("path/to/db")?;

// Read-only store
let store = Store::open_read_only("path/to/db")?;
```

**Store Options** (via builder pattern):

Currently, Oxigraph does not expose many low-level RocksDB configuration options. The database is tuned with reasonable defaults.

---

### Python API

```python
from pyoxigraph import Store

# In-memory
store = Store()

# Persistent
store = Store("/path/to/db")

# Read-only
store = Store.read_only("/path/to/db")
```

---

### JavaScript API

```javascript
import { Store } from 'oxigraph';

// In-memory only (browser environment)
const store = new Store();

// Can optionally initialize with quads
const store = new Store([quad1, quad2, quad3]);
```

---

## Server Configuration

### HTTP Endpoints

When running `oxigraph serve`, the following endpoints are available:

#### SPARQL Query Endpoint

```
GET /query?query=...
POST /query
  Content-Type: application/sparql-query
  Content-Type: application/x-www-form-urlencoded
```

**Query Parameters**:
- `query` - SPARQL query string
- `default-graph-uri` - Default graph URI(s)
- `named-graph-uri` - Named graph URI(s)

**Response Formats** (via Accept header or `format` parameter):
- `application/sparql-results+json`
- `application/sparql-results+xml`
- `text/csv`
- `text/tab-separated-values`

---

#### SPARQL Update Endpoint

```
POST /update
  Content-Type: application/sparql-update
  Content-Type: application/x-www-form-urlencoded
```

---

#### SPARQL Graph Store Protocol

```
GET /store?graph=...           # Get graph
POST /store?graph=...          # Add to graph
PUT /store?graph=...           # Replace graph
DELETE /store?graph=...        # Delete graph

GET /store?default             # Get default graph
POST /store?default            # Add to default graph
PUT /store?default             # Replace default graph
DELETE /store?default          # Delete default graph
```

**Content Types**: Any supported RDF format (Turtle, N-Triples, etc.)

---

#### Service Description

```
GET /
```

Returns SPARQL service description (RDF describing the endpoint).

---

### CORS Configuration

Enable Cross-Origin Resource Sharing:

```bash
oxigraph serve --cors
```

**Effect**: Adds CORS headers to all responses:
- `Access-Control-Allow-Origin: *`
- `Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS`
- `Access-Control-Allow-Headers: Content-Type, Authorization`

---

### Union Default Graph

Make SPARQL queries search all named graphs by default:

```bash
oxigraph serve --union-default-graph
```

**Effect**: `{ ?s ?p ?o }` queries across all graphs, equivalent to:
```sparql
{ GRAPH ?g { ?s ?p ?o } }
```

---

### Query Timeout

Set maximum query execution time:

```bash
oxigraph serve --timeout-s 30
```

**Effect**: Queries exceeding 30 seconds are terminated.

---

## Environment Variables

Oxigraph respects the following environment variables:

### RUST_LOG

Control logging output (using `env_logger`).

**Levels**: `error`, `warn`, `info`, `debug`, `trace`

**Examples**:
```bash
# Show all info-level logs
RUST_LOG=info oxigraph serve

# Debug-level logs for Oxigraph only
RUST_LOG=oxigraph=debug oxigraph serve

# Multiple modules
RUST_LOG=oxigraph=debug,spargebra=trace oxigraph serve
```

---

### RUST_BACKTRACE

Enable backtrace on panic.

**Values**: `0` (off), `1` (on), `full`

**Example**:
```bash
RUST_BACKTRACE=1 oxigraph serve
```

---

### OXIGRAPH_MAX_MEMORY

(Not currently implemented - reserved for future use)

Potential future option to limit memory usage.

---

## Feature Flags

Feature flags are compile-time options set in `Cargo.toml`.

### Rust Crate Features

#### oxigraph

| Feature | Default | Description |
|---------|---------|-------------|
| `rocksdb` | Yes | Enable RocksDB persistent storage |
| `http-client` | No | Enable HTTP client for SPARQL SERVICE |
| `rdf-12` | No | Enable RDF 1.2 support |

**Example**:
```toml
[dependencies]
oxigraph = { version = "0.5", features = ["http-client"] }
```

---

#### spareval

| Feature | Default | Description |
|---------|---------|-------------|
| `sparql-12` | No | Enable SPARQL 1.2 features |
| `sep-0002` | No | Enable temporal functions (ADJUST, etc.) |
| `sep-0006` | No | Enable LATERAL joins |
| `calendar-ext` | No | Enable extended calendar arithmetic |

---

#### oxrdfio

| Feature | Default | Description |
|---------|---------|-------------|
| `rdf-12` | No | Enable RDF 1.2 in parsers/serializers |
| `async-tokio` | No | Enable async I/O with Tokio |

---

### JavaScript Build Features

Built into the WASM module:

| Feature | Description |
|---------|-------------|
| `geosparql` | Enable GeoSPARQL functions |
| `rdf-12` | Enable RDF 1.2 features |

These are set when building `oxigraph-js`:
```bash
cd js
wasm-pack build --features geosparql
```

---

## Performance Tuning

### Bulk Loading

For loading large datasets:

**CLI**:
```bash
# Use non-atomic flag to reduce disk usage
oxigraph load --location ./data --file large.nt --non-atomic

# Enable lenient mode to skip validation
oxigraph load --location ./data --file large.nt --lenient
```

**Rust API**:
```rust
use oxigraph::store::Store;
use oxigraph::io::{RdfFormat, RdfParser};

let store = Store::open("./data")?;
let mut loader = store.bulk_loader();

let parser = RdfParser::from_format(RdfFormat::NTriples);
for quad in parser.for_reader(file) {
    loader.load_quad(&quad?)?;
}
loader.finish()?;
```

**Benefits**:
- Optimized batch insertion
- Reduced write amplification
- Better compression
- Lower memory usage

---

### RocksDB Tuning

Oxigraph uses default RocksDB settings tuned for general use. For advanced tuning:

**Currently**: No exposed configuration (reasonable defaults)

**Future**: May expose options like:
- Block cache size
- Write buffer size
- Compression algorithms
- Background threads

---

### Query Performance

**Tips**:
1. Use LIMIT on large result sets
2. Apply filters early in query
3. Avoid `SELECT *` (select only needed variables)
4. Use indexes effectively (SPO, POS, OSP)
5. Property paths can be expensive on large graphs

**Query Explanation**:
```bash
# Analyze query plan
oxigraph query --location ./data --query-file query.sparql --explain --stats
```

---

### Memory Management

**In-memory Store**:
- All data in RAM
- Fast access
- Limited by available memory
- Lost on exit

**Persistent Store**:
- Data on disk (RocksDB)
- Block cache in memory
- Suitable for large datasets
- Survives restarts

---

## Default Values Summary

| Setting | Default Value |
|---------|---------------|
| Server bind address | `localhost:7878` |
| Storage location | In-memory (none) |
| CORS | Disabled |
| Union default graph | Disabled |
| Query timeout | None (unlimited) |
| RDF format detection | By file extension |
| Load mode | Atomic |
| Lenient parsing | Disabled |

---

## Configuration Examples

### Production HTTP Server

```bash
oxigraph serve \
  --location /var/lib/oxigraph \
  --bind 0.0.0.0:7878 \
  --cors \
  --timeout-s 60
```

---

### Read-Only Query Endpoint

```bash
oxigraph serve-read-only \
  --location /var/lib/oxigraph \
  --bind 0.0.0.0:8080 \
  --union-default-graph
```

---

### Bulk Data Import

```bash
# Large dataset (Wikidata, DBpedia, etc.)
oxigraph load \
  --location /var/lib/oxigraph \
  --file wikidata-latest-all.nt.gz \
  --lenient \
  --non-atomic
```

---

### Development Setup

```bash
# In-memory server for testing
oxigraph serve --cors

# Or with sample data
oxigraph serve --location ./dev-data --cors
```

---

## Troubleshooting

### Database Locked Error

**Cause**: Another process has the database open in write mode.

**Solution**: Ensure only one write instance is running. Use read-only mode for additional processes:
```bash
oxigraph serve-read-only --location ./data
```

---

### Out of Memory

**Cause**: Query or dataset too large for available RAM.

**Solutions**:
1. Use LIMIT in queries
2. Use persistent store (not in-memory)
3. Increase system memory
4. Use streaming/chunked processing

---

### Slow Queries

**Cause**: Complex query or large dataset.

**Solutions**:
1. Add filters to reduce result set
2. Use LIMIT to paginate results
3. Optimize query with `--explain`
4. Consider indexing strategies
5. Use more specific patterns

---

### Parse Errors

**Cause**: Invalid RDF syntax.

**Solutions**:
1. Validate RDF with external tools
2. Use `--lenient` flag (with caution)
3. Check format specification matches content
4. Verify encoding (must be UTF-8)

---

## Further Reading

- [Oxigraph GitHub](https://github.com/oxigraph/oxigraph)
- [RocksDB Documentation](https://rocksdb.org/)
- [SPARQL 1.1 Protocol](https://www.w3.org/TR/sparql11-protocol/)
- [SPARQL 1.1 Graph Store Protocol](https://www.w3.org/TR/sparql11-http-rdf-update/)
