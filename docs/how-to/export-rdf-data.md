# How to Export RDF Data

This guide explains how to export RDF data from Oxigraph in various formats and contexts.

## Supported Export Formats

### Graph Formats (Triples)
- **Turtle** (`text/turtle`)
- **N-Triples** (`application/n-triples`)
- **RDF/XML** (`application/rdf+xml`)

### Dataset Formats (Quads)
- **N-Quads** (`application/n-quads`)
- **TriG** (`application/trig`)

## Using the CLI

### Dump Entire Database

```bash
# Dump to N-Quads (includes all graphs)
oxigraph dump --location /path/to/store --file dump.nq

# Dump to TriG
oxigraph dump --location /path/to/store --file dump.trig

# Dump to stdout
oxigraph dump --location /path/to/store --format nquads > dump.nq

# Specify format explicitly
oxigraph dump --location /path/to/store \
  --file output.txt --format turtle
```

### Export Specific Graph

```bash
# Export default graph as Turtle
oxigraph dump --location /path/to/store \
  --file default.ttl --graph default

# Export named graph
oxigraph dump --location /path/to/store \
  --file mygraph.ttl --graph http://example.com/mygraph
```

### Export via HTTP (Server)

```bash
# Get default graph
curl -H 'Accept: text/turtle' \
  http://localhost:7878/store?default > default.ttl

# Get named graph
curl -H 'Accept: text/turtle' \
  "http://localhost:7878/store?graph=http://example.com/mygraph" > graph.ttl

# Get entire dataset as N-Quads
curl -H 'Accept: application/n-quads' \
  http://localhost:7878/store > dataset.nq

# Get entire dataset as TriG
curl -H 'Accept: application/trig' \
  http://localhost:7878/store > dataset.trig
```

## Using Rust API

### Export to File

```rust
use oxigraph::store::Store;
use oxigraph::io::RdfFormat;
use std::fs::File;

let store = Store::open("data")?;

// Dump entire dataset to N-Quads
store.dump_to_writer(RdfFormat::NQuads, File::create("dump.nq")?)?;

// Dump to Turtle (default graph only)
store.dump_graph_to_writer(
    GraphNameRef::DefaultGraph,
    RdfFormat::Turtle,
    File::create("output.ttl")?
)?;
```

### Export to String

```rust
use oxigraph::store::Store;
use oxigraph::io::RdfFormat;

let store = Store::open("data")?;

// Serialize to string
let mut buffer = Vec::new();
store.dump_to_writer(RdfFormat::NQuads, &mut buffer)?;
let data = String::from_utf8(buffer)?;

println!("{}", data);
```

### Export Named Graph

```rust
use oxigraph::model::{GraphNameRef, NamedNodeRef};
use oxigraph::io::RdfFormat;

let graph = NamedNodeRef::new("http://example.com/mygraph")?;

store.dump_graph_to_writer(
    graph.into(),
    RdfFormat::Turtle,
    File::create("graph.ttl")?
)?;
```

### Export with Serialization Options

```rust
use oxigraph::io::RdfSerializer;
use oxigraph::model::GraphNameRef;

let mut serializer = RdfSerializer::from_format(RdfFormat::Turtle)
    .for_writer(File::create("output.ttl")?);

// Iterate and serialize quads from default graph
for quad in store.quads_for_pattern(None, None, None, Some(GraphNameRef::DefaultGraph)) {
    let quad = quad?;
    serializer.serialize_triple(quad.as_ref().into())?;
}

serializer.finish()?;
```

### Custom Export Logic

```rust
use oxigraph::io::RdfSerializer;
use oxigraph::store::Store;
use oxigraph::model::*;

let store = Store::open("data")?;

// Export only specific subjects
let subject = NamedNodeRef::new("http://example.com/resource")?;
let mut serializer = RdfSerializer::from_format(RdfFormat::Turtle)
    .for_writer(File::create("filtered.ttl")?);

for quad in store.quads_for_pattern(Some(subject.into()), None, None, None) {
    serializer.serialize_quad(&quad?)?;
}

serializer.finish()?;
```

## Using Python API

### Basic Export

```python
from pyoxigraph import Store

store = Store("data")

# Dump entire dataset
output = store.dump(format="application/n-quads")
with open("dump.nq", "w") as f:
    f.write(output)

# Dump to file directly
with open("dump.nq", "wb") as f:
    store.dump(f, mime_type="application/n-quads")
```

### Export Specific Graph

```python
from pyoxigraph import Store, NamedNode

store = Store("data")

# Export default graph
output = store.dump(
    format="text/turtle",
    from_graph=None  # Default graph
)

# Export named graph
graph = NamedNode("http://example.com/mygraph")
output = store.dump(
    format="text/turtle",
    from_graph=graph
)

with open("graph.ttl", "w") as f:
    f.write(output)
```

### Iterate and Serialize

```python
from pyoxigraph import Store, serialize

store = Store("data")

# Manually serialize quads
quads = list(store)
serialized = serialize(quads, mime_type="application/n-quads")

with open("output.nq", "w") as f:
    f.write(serialized)
```

### Export with Filtering

```python
from pyoxigraph import Store, NamedNode

store = Store("data")
subject = NamedNode("http://example.com/resource")

# Get quads matching pattern
quads = list(store.quads_for_pattern(subject=subject))

# Serialize filtered quads
from pyoxigraph import serialize
output = serialize(quads, mime_type="text/turtle")

with open("filtered.ttl", "w") as f:
    f.write(output)
```

## Using JavaScript API

### Basic Export

```javascript
import { Store } from 'oxigraph';

const store = new Store();

// Dump to string
const data = store.dump({
    format: 'application/n-quads'
});

console.log(data);
```

### Export with Options

```javascript
// Export with prefixes
const turtle = store.dump({
    format: 'text/turtle',
    prefixes: {
        'ex': 'http://example.com/',
        'rdf': 'http://www.w3.org/1999/02/22-rdf-syntax-ns#'
    }
});

// Export with base IRI
const output = store.dump({
    format: 'text/turtle',
    baseIri: 'http://example.com/'
});
```

### Export Specific Graph

```javascript
import { Store, NamedNode, DefaultGraph } from 'oxigraph';

const store = new Store();

// Export default graph
const defaultGraph = store.dump({
    format: 'text/turtle',
    fromGraphName: new DefaultGraph()
});

// Export named graph
const graph = new NamedNode('http://example.com/mygraph');
const graphData = store.dump({
    format: 'text/turtle',
    fromGraphName: graph
});
```

## SPARQL CONSTRUCT Queries

You can also export data using SPARQL CONSTRUCT queries to transform and filter during export.

### Via CLI/HTTP

```bash
# Execute CONSTRUCT query
curl -X POST http://localhost:7878/query \
  -H 'Content-Type: application/sparql-query' \
  -H 'Accept: text/turtle' \
  --data 'CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }' \
  > result.ttl
```

### Rust API

```rust
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::io::RdfSerializer;

let evaluator = SparqlEvaluator::new();
let query = "CONSTRUCT { ?s ?p ?o } WHERE { ?s a <http://example.com/Person> }";

if let QueryResults::Graph(triples) = evaluator
    .parse_query(query)?
    .on_store(&store)
    .execute()?
{
    let mut serializer = RdfSerializer::from_format(RdfFormat::Turtle)
        .for_writer(File::create("persons.ttl")?);

    for triple in triples {
        serializer.serialize_triple(&triple?)?;
    }

    serializer.finish()?;
}
```

### Python API

```python
from pyoxigraph import Store

store = Store("data")

query = """
CONSTRUCT { ?s ?p ?o }
WHERE { ?s a <http://example.com/Person> . ?s ?p ?o }
"""

result = store.query(query)

# Result is iterable of triples
output = store.dump(result, mime_type="text/turtle")
```

### JavaScript API

```javascript
const store = new Store();

const query = `
CONSTRUCT { ?s ?p ?o }
WHERE { ?s a <http://example.com/Person> . ?s ?p ?o }
`;

const result = store.query(query);
// Result is array of Quads

// Manually serialize or add to new store
const filtered = new Store(result);
const output = filtered.dump({ format: 'text/turtle' });
```

## Export Query Results

### Export SPARQL SELECT Results

```bash
# As JSON
curl -X POST http://localhost:7878/query \
  -H 'Content-Type: application/sparql-query' \
  -H 'Accept: application/sparql-results+json' \
  --data 'SELECT * WHERE { ?s ?p ?o } LIMIT 10' \
  > results.json

# As CSV
curl -X POST http://localhost:7878/query \
  -H 'Accept: text/csv' \
  --data 'SELECT * WHERE { ?s ?p ?o }' \
  > results.csv

# As TSV
curl -X POST http://localhost:7878/query \
  -H 'Accept: text/tab-separated-values' \
  --data 'SELECT * WHERE { ?s ?p ?o }' \
  > results.tsv

# As XML
curl -X POST http://localhost:7878/query \
  -H 'Accept: application/sparql-results+xml' \
  --data 'SELECT * WHERE { ?s ?p ?o }' \
  > results.xml
```

### Rust: Export Results to File

```rust
use oxigraph::sparql::results::{QueryResultsFormat, QueryResultsSerializer};
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use std::fs::File;

let query = "SELECT * WHERE { ?s ?p ?o } LIMIT 100";
let results = SparqlEvaluator::new()
    .parse_query(query)?
    .on_store(&store)
    .execute()?;

let mut serializer = QueryResultsSerializer::from_format(QueryResultsFormat::Json)
    .for_writer(File::create("results.json")?);

serializer.serialize(results)?;
```

### Python: Export Results

```python
from pyoxigraph import Store

store = Store("data")

# Query returns solutions
results = store.query("SELECT * WHERE { ?s ?p ?o } LIMIT 10")

# Convert to JSON
import json
solutions = []
for solution in results:
    solutions.append({
        var: str(solution[var]) for var in solution.variables()
    })

with open("results.json", "w") as f:
    json.dump(solutions, f)
```

## Format Conversion

### Convert Between Formats

```bash
# Turtle to N-Triples
oxigraph dump --location store --file output.nt --graph default

# N-Quads to TriG
oxigraph dump --location store --format trig > output.trig

# Via load/dump cycle for format conversion
oxigraph load --location temp_store --file input.rdf
oxigraph dump --location temp_store --file output.ttl
```

### Using Rust for Conversion

```rust
use oxigraph::io::{RdfFormat, RdfParser, RdfSerializer};
use std::fs::File;

// Convert Turtle to N-Triples
let input = File::open("input.ttl")?;
let output = File::create("output.nt")?;

let parser = RdfParser::from_format(RdfFormat::Turtle)
    .for_reader(input);

let mut serializer = RdfSerializer::from_format(RdfFormat::NTriples)
    .for_writer(output);

for quad in parser {
    serializer.serialize_quad(&quad?)?;
}

serializer.finish()?;
```

## Performance Tips

1. **Use streaming** for large exports to minimize memory usage
2. **Choose line-oriented formats** (N-Triples, N-Quads) for fastest serialization
3. **Compress output** with gzip for storage/transfer
4. **Use CONSTRUCT queries** to export only what you need
5. **Export graphs separately** rather than the entire dataset when possible
6. **Avoid pretty-printing** for large exports (use N-Triples/N-Quads)

## Common Patterns

### Incremental Export

```rust
// Export data added after a certain time
for quad in store.quads_for_pattern(None, None, None, None) {
    let quad = quad?;
    // Check timestamp metadata and serialize if recent
    serializer.serialize_quad(&quad)?;
}
```

### Export with Compression

```bash
# Direct compression
oxigraph dump --location store --format nquads | gzip > dump.nq.gz

# Or dump to file and compress
oxigraph dump --location store --file dump.nq
gzip dump.nq
```

### Backup and Export

```bash
# Create backup (RocksDB format)
oxigraph backup --location store --destination backup/

# Export as portable RDF
oxigraph dump --location store --file portable-backup.nq
```

## Next Steps

- Learn about [importing RDF data](import-rdf-data.md)
- Query your data with [SPARQL](run-sparql-server.md)
- Optimize exports with [performance tips](optimize-performance.md)
