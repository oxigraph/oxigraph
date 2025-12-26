# Oxigraph Quick Reference Cheatsheet

Your one-page reference for common operations. Keep this handy while developing!

---

## Table of Contents

- [Installation](#installation)
- [Store Operations](#store-operations)
- [RDF Data Model](#rdf-data-model)
- [Loading Data](#loading-data)
- [SPARQL Queries](#sparql-queries)
- [SPARQL Updates](#sparql-updates)
- [Query Result Handling](#query-result-handling)
- [RDF Formats](#rdf-formats)
- [CLI Commands](#cli-commands)
- [Environment Variables](#environment-variables)
- [Common SPARQL Patterns](#common-sparql-patterns)

---

## Installation

### Quick Install

| Platform | Command |
|----------|---------|
| **Rust** | `cargo add oxigraph` |
| **Python** | `pip install pyoxigraph` |
| **JavaScript** | `npm install oxigraph` |
| **Docker** | `docker pull ghcr.io/oxigraph/oxigraph:latest` |
| **CLI** | `cargo install oxigraph-cli` |

### Imports

<table>
<tr><th>Rust</th><th>Python</th><th>JavaScript</th></tr>
<tr>
<td>

```rust
use oxigraph::store::Store;
use oxigraph::model::*;
use oxigraph::sparql::QueryResults;
```

</td>
<td>

```python
from pyoxigraph import (
    Store, NamedNode, Literal,
    Quad, DefaultGraph
)
```

</td>
<td>

```javascript
const oxigraph = require('oxigraph');
// or ES module:
import oxigraph from 'oxigraph/node.js';
```

</td>
</tr>
</table>

---

## Store Operations

### Create Store

<table>
<tr><th>Operation</th><th>Rust</th><th>Python</th><th>JavaScript</th></tr>

<tr>
<td><b>In-Memory</b></td>
<td>

```rust
let store = Store::new()?;
```

</td>
<td>

```python
store = Store()
```

</td>
<td>

```javascript
const store = new oxigraph.Store();
```

</td>
</tr>

<tr>
<td><b>Persistent</b></td>
<td>

```rust
let store = Store::open("./db")?;
```

</td>
<td>

```python
store = Store("./db")
```

</td>
<td>

```javascript
// Not available in JavaScript
// (always in-memory)
```

</td>
</tr>

<tr>
<td><b>Read-Only</b></td>
<td>

```rust
let store = Store::open_read_only(
    "./db"
)?;
```

</td>
<td>

```python
# Not directly supported
# Use read-only filesystem
```

</td>
<td>

```javascript
// Not available
```

</td>
</tr>
</table>

### Store Statistics

<table>
<tr><th>Operation</th><th>Rust</th><th>Python</th><th>JavaScript</th></tr>

<tr>
<td><b>Count Quads</b></td>
<td>

```rust
let count = store.len()?;
```

</td>
<td>

```python
count = len(store)
```

</td>
<td>

```javascript
const count = store.size;
```

</td>
</tr>

<tr>
<td><b>Check Empty</b></td>
<td>

```rust
let empty = store.is_empty()?;
```

</td>
<td>

```python
empty = len(store) == 0
```

</td>
<td>

```javascript
const empty = store.size === 0;
```

</td>
</tr>

<tr>
<td><b>Contains Quad</b></td>
<td>

```rust
if store.contains(&quad)? {
    // exists
}
```

</td>
<td>

```python
if quad in store:
    # exists
```

</td>
<td>

```javascript
// Use query instead
```

</td>
</tr>
</table>

---

## RDF Data Model

### Creating Nodes

<table>
<tr><th>Node Type</th><th>Rust</th><th>Python</th><th>JavaScript</th></tr>

<tr>
<td><b>Named Node (IRI)</b></td>
<td>

```rust
let node = NamedNode::new(
    "http://example.org/alice"
)?;
```

</td>
<td>

```python
node = NamedNode(
    "http://example.org/alice"
)
```

</td>
<td>

```javascript
const node = oxigraph.namedNode(
    "http://example.org/alice"
);
```

</td>
</tr>

<tr>
<td><b>Blank Node</b></td>
<td>

```rust
let blank = BlankNode::default();
// or with ID:
let blank = BlankNode::new("b1")?;
```

</td>
<td>

```python
blank = BlankNode()
# or with ID:
blank = BlankNode("b1")
```

</td>
<td>

```javascript
const blank = oxigraph.blankNode();
// or with ID:
const blank = oxigraph.blankNode("b1");
```

</td>
</tr>

<tr>
<td><b>Simple Literal</b></td>
<td>

```rust
let lit = Literal::new_simple_literal(
    "Alice"
);
```

</td>
<td>

```python
lit = Literal("Alice")
```

</td>
<td>

```javascript
const lit = oxigraph.literal("Alice");
```

</td>
</tr>

<tr>
<td><b>Literal with Language</b></td>
<td>

```rust
let lit = Literal::new_language_tagged_literal(
    "Alice", "en"
)?;
```

</td>
<td>

```python
lit = Literal("Alice", language="en")
```

</td>
<td>

```javascript
const lit = oxigraph.literal(
    "Alice",
    { language: "en" }
);
```

</td>
</tr>

<tr>
<td><b>Typed Literal</b></td>
<td>

```rust
let num = Literal::new_typed_literal(
    "42",
    xsd::INTEGER
);
```

</td>
<td>

```python
from pyoxigraph import xsd
num = Literal("42", datatype=xsd.INTEGER)
```

</td>
<td>

```javascript
const num = oxigraph.literal(
    "42",
    { datatype: oxigraph.xsd.integer }
);
```

</td>
</tr>
</table>

### Creating Triples/Quads

<table>
<tr><th>Rust</th><th>Python</th><th>JavaScript</th></tr>

<tr>
<td>

```rust
// Triple (default graph)
let triple = Triple::new(
    subject,
    predicate,
    object
);

// Quad (with graph)
let quad = Quad::new(
    subject,
    predicate,
    object,
    GraphName::DefaultGraph
);

// Named graph
let quad = Quad::new(
    subject,
    predicate,
    object,
    NamedNode::new("http://ex.org/g")?
);
```

</td>
<td>

```python
# Triple (default graph)
triple = Quad(
    subject,
    predicate,
    object,
    DefaultGraph()
)

# Named graph
from pyoxigraph import NamedNode
graph = NamedNode("http://ex.org/g")
quad = Quad(
    subject,
    predicate,
    object,
    graph
)
```

</td>
<td>

```javascript
// Triple (default graph)
const triple = oxigraph.triple(
    subject,
    predicate,
    object
);

// Quad with named graph
const quad = oxigraph.quad(
    subject,
    predicate,
    object,
    oxigraph.namedNode("http://ex.org/g")
);
```

</td>
</tr>
</table>

### Adding/Removing Data

<table>
<tr><th>Operation</th><th>Rust</th><th>Python</th><th>JavaScript</th></tr>

<tr>
<td><b>Insert Quad</b></td>
<td>

```rust
store.insert(&quad)?;
```

</td>
<td>

```python
store.add(quad)
```

</td>
<td>

```javascript
store.add(quad);
```

</td>
</tr>

<tr>
<td><b>Remove Quad</b></td>
<td>

```rust
store.remove(&quad)?;
```

</td>
<td>

```python
store.remove(quad)
```

</td>
<td>

```javascript
store.delete(quad);
```

</td>
</tr>

<tr>
<td><b>Clear All</b></td>
<td>

```rust
store.clear()?;
```

</td>
<td>

```python
store.clear()
```

</td>
<td>

```javascript
store.clear();
```

</td>
</tr>
</table>

---

## Loading Data

### From Files

<table>
<tr><th>Rust</th><th>Python</th><th>JavaScript</th></tr>

<tr>
<td>

```rust
use oxigraph::io::RdfFormat;

// Auto-detect from extension
store.load_from_path("data.ttl")?;

// Explicit format
store.load_from_reader(
    RdfFormat::Turtle,
    std::fs::File::open("data.ttl")?
)?;
```

</td>
<td>

```python
# Auto-detect from extension
store.load(path="data.ttl")

# Explicit format
store.load(
    "data.ttl",
    format=RdfFormat.TURTLE
)

# From bytes
data = open("data.ttl", "rb").read()
store.load(input=data, format=RdfFormat.TURTLE)
```

</td>
<td>

```javascript
const fs = require('fs');

// Read file
const data = fs.readFileSync(
    'data.ttl',
    'utf-8'
);

// Load into store
store.load(input=data, {
    format: "text/turtle"
});
```

</td>
</tr>
</table>

### From Strings

<table>
<tr><th>Rust</th><th>Python</th><th>JavaScript</th></tr>

<tr>
<td>

```rust
use oxigraph::io::RdfFormat;

let data = r#"
    <http://ex.org/s>
    <http://ex.org/p>
    "value" .
"#;

store.load_from_reader(
    RdfFormat::NTriples,
    data.as_bytes()
)?;
```

</td>
<td>

```python
data = """
    <http://ex.org/s>
    <http://ex.org/p>
    "value" .
"""

store.load(
    data.encode(),
    format=RdfFormat.N_TRIPLES
)
```

</td>
<td>

```javascript
const data = `
    <http://ex.org/s>
    <http://ex.org/p>
    "value" .
`;

store.load(input=data, {
    format: "application/n-triples"
});
```

</td>
</tr>
</table>

### Bulk Loading

<table>
<tr><th>Rust</th><th>Python</th><th>JavaScript</th></tr>

<tr>
<td>

```rust
// Much faster for large files
let mut loader = store.bulk_loader();
loader.load_from_path("huge.nq")?;
```

</td>
<td>

```python
# Much faster for large files
store.bulk_load(
    "huge.nq",
    format=RdfFormat.N_QUADS
)
```

</td>
<td>

```javascript
// Not available
// Use regular load()
```

</td>
</tr>
</table>

---

## SPARQL Queries

### Basic Query

<table>
<tr><th>Rust</th><th>Python</th><th>JavaScript</th></tr>

<tr>
<td>

```rust
let results = store.query(
    "SELECT * WHERE { ?s ?p ?o }"
)?;

match results {
    QueryResults::Solutions(mut sols) => {
        while let Some(sol) = sols.next() {
            let s = sol?;
            println!("{:?}", s);
        }
    }
    _ => {}
}
```

</td>
<td>

```python
query = "SELECT * WHERE { ?s ?p ?o }"

for solution in store.query(query):
    print(solution["s"])
    print(solution["p"])
    print(solution["o"])
```

</td>
<td>

```javascript
const query = "SELECT * WHERE { ?s ?p ?o }";

for (const solution of store.query(query)) {
    console.log(solution.get("s"));
    console.log(solution.get("p"));
    console.log(solution.get("o"));
}
```

</td>
</tr>
</table>

### Query Types

<table>
<tr><th>Type</th><th>Example</th><th>Returns</th></tr>

<tr>
<td><b>SELECT</b></td>
<td>

```sparql
SELECT ?name ?age WHERE {
    ?person foaf:name ?name ;
            foaf:age ?age .
}
```

</td>
<td>Bindings (solution mappings)</td>
</tr>

<tr>
<td><b>ASK</b></td>
<td>

```sparql
ASK {
    ?person foaf:name "Alice" .
}
```

</td>
<td>Boolean (true/false)</td>
</tr>

<tr>
<td><b>CONSTRUCT</b></td>
<td>

```sparql
CONSTRUCT {
    ?person a foaf:Person .
} WHERE {
    ?person foaf:name ?name .
}
```

</td>
<td>Triples/Graph</td>
</tr>

<tr>
<td><b>DESCRIBE</b></td>
<td>

```sparql
DESCRIBE <http://example.org/alice>
```

</td>
<td>Triples/Graph about resource</td>
</tr>
</table>

### Handling Results by Type

<table>
<tr><th>Rust</th><th>Python</th><th>JavaScript</th></tr>

<tr>
<td>

```rust
match store.query(q)? {
    QueryResults::Solutions(s) => {
        // SELECT query
    }
    QueryResults::Boolean(b) => {
        // ASK query
        println!("Result: {}", b);
    }
    QueryResults::Graph(g) => {
        // CONSTRUCT/DESCRIBE
        for triple in g {
            println!("{:?}", triple?);
        }
    }
}
```

</td>
<td>

```python
result = store.query(query)

# For SELECT
for sol in result:
    print(sol)

# For ASK
bool_result = store.query("ASK {...}")
print(bool_result)  # True/False

# For CONSTRUCT
for triple in store.query("CONSTRUCT {...}"):
    print(triple)
```

</td>
<td>

```javascript
// For SELECT
for (const sol of store.query("SELECT...")) {
    console.log(sol);
}

// For ASK
const result = store.query("ASK {...}");
console.log(result);  // true/false

// For CONSTRUCT
for (const triple of store.query("CONSTRUCT...")) {
    console.log(triple);
}
```

</td>
</tr>
</table>

---

## SPARQL Updates

### Insert Data

<table>
<tr><th>Rust</th><th>Python</th><th>JavaScript</th></tr>

<tr>
<td>

```rust
store.update(
    "INSERT DATA {
        <http://ex.org/s>
        <http://ex.org/p>
        'value' .
    }"
)?;
```

</td>
<td>

```python
store.update("""
    INSERT DATA {
        <http://ex.org/s>
        <http://ex.org/p>
        'value' .
    }
""")
```

</td>
<td>

```javascript
store.update(`
    INSERT DATA {
        <http://ex.org/s>
        <http://ex.org/p>
        'value' .
    }
`);
```

</td>
</tr>
</table>

### Delete Data

<table>
<tr><th>Rust</th><th>Python</th><th>JavaScript</th></tr>

<tr>
<td>

```rust
store.update(
    "DELETE WHERE {
        ?s ?p ?o .
        FILTER(?o = 'old')
    }"
)?;
```

</td>
<td>

```python
store.update("""
    DELETE WHERE {
        ?s ?p ?o .
        FILTER(?o = 'old')
    }
""")
```

</td>
<td>

```javascript
store.update(`
    DELETE WHERE {
        ?s ?p ?o .
        FILTER(?o = 'old')
    }
`);
```

</td>
</tr>
</table>

### Modify Data

<table>
<tr><th>Rust</th><th>Python</th><th>JavaScript</th></tr>

<tr>
<td>

```rust
store.update(
    "DELETE { ?s ?p 'old' }
     INSERT { ?s ?p 'new' }
     WHERE { ?s ?p 'old' }"
)?;
```

</td>
<td>

```python
store.update("""
    DELETE { ?s ?p 'old' }
    INSERT { ?s ?p 'new' }
    WHERE { ?s ?p 'old' }
""")
```

</td>
<td>

```javascript
store.update(`
    DELETE { ?s ?p 'old' }
    INSERT { ?s ?p 'new' }
    WHERE { ?s ?p 'old' }
`);
```

</td>
</tr>
</table>

---

## Query Result Handling

### Accessing Bindings

<table>
<tr><th>Rust</th><th>Python</th><th>JavaScript</th></tr>

<tr>
<td>

```rust
if let QueryResults::Solutions(mut sols) =
    store.query("SELECT ?x WHERE {}")?
{
    while let Some(sol) = sols.next() {
        let solution = sol?;

        // Get value
        if let Some(x) = solution.get("x") {
            println!("{}", x);
        }

        // Iterate all variables
        for (var, value) in solution.iter() {
            println!("{}: {}", var, value);
        }
    }
}
```

</td>
<td>

```python
for solution in store.query("SELECT ?x WHERE {}"):
    # Get value
    x = solution["x"]
    print(x)

    # Or use .get() for optional
    y = solution.get("y")

    # Iterate all variables
    for var, value in solution.items():
        print(f"{var}: {value}")
```

</td>
<td>

```javascript
for (const solution of store.query("SELECT ?x WHERE {}")) {
    // Get value
    const x = solution.get("x");
    console.log(x);

    // Iterate all variables
    for (const [var, value] of solution) {
        console.log(`${var}: ${value}`);
    }
}
```

</td>
</tr>
</table>

### Converting Results to JSON

<table>
<tr><th>Rust</th><th>Python</th><th>JavaScript</th></tr>

<tr>
<td>

```rust
use oxigraph::sparql::QueryResultsFormat;
use std::io::Cursor;

if let QueryResults::Solutions(sols) =
    store.query(q)?
{
    let mut buffer = Vec::new();
    sols.write(
        &mut buffer,
        QueryResultsFormat::Json
    )?;
    let json = String::from_utf8(buffer)?;
    println!("{}", json);
}
```

</td>
<td>

```python
import json

results = []
for solution in store.query(query):
    row = {
        var: str(value)
        for var, value in solution.items()
    }
    results.append(row)

json_output = json.dumps(results, indent=2)
print(json_output)
```

</td>
<td>

```javascript
const results = [];
for (const solution of store.query(query)) {
    const row = {};
    for (const [var, value] of solution) {
        row[var] = value.value;
    }
    results.push(row);
}

console.log(JSON.stringify(results, null, 2));
```

</td>
</tr>
</table>

---

## RDF Formats

### Format Reference

| Format | MIME Type | Extension | Use Case |
|--------|-----------|-----------|----------|
| **Turtle** | `text/turtle` | `.ttl` | Human-readable, compact |
| **TriG** | `application/trig` | `.trig` | Turtle + named graphs |
| **N-Triples** | `application/n-triples` | `.nt` | Simple, line-based |
| **N-Quads** | `application/n-quads` | `.nq` | N-Triples + named graphs |
| **RDF/XML** | `application/rdf+xml` | `.rdf`, `.owl` | Legacy XML format |
| **JSON-LD** | `application/ld+json` | `.jsonld` | JSON with @context |
| **N3** | `text/n3` | `.n3` | Turtle superset with rules |

### Format Examples

**Turtle:**
```turtle
@prefix ex: <http://example.org/> .
@prefix foaf: <http://xmlns.com/foaf/0.1/> .

ex:alice a foaf:Person ;
    foaf:name "Alice" ;
    foaf:knows ex:bob .
```

**N-Triples:**
```ntriples
<http://example.org/alice> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://xmlns.com/foaf/0.1/Person> .
<http://example.org/alice> <http://xmlns.com/foaf/0.1/name> "Alice" .
<http://example.org/alice> <http://xmlns.com/foaf/0.1/knows> <http://example.org/bob> .
```

**JSON-LD:**
```json
{
  "@context": {
    "foaf": "http://xmlns.com/foaf/0.1/",
    "name": "foaf:name",
    "knows": "foaf:knows"
  },
  "@id": "http://example.org/alice",
  "@type": "foaf:Person",
  "name": "Alice",
  "knows": { "@id": "http://example.org/bob" }
}
```

---

## CLI Commands

### Server Commands

```bash
# Start server
oxigraph serve --location ./data --bind 127.0.0.1:7878

# Custom port
oxigraph serve --location ./data --bind 0.0.0.0:8080

# Read-only mode
oxigraph serve --location ./data --read-only
```

### Data Loading

```bash
# Load data (before starting server - fastest)
oxigraph load --location ./data --file data.ttl

# Multiple files
oxigraph load --location ./data --file file1.ttl --file file2.nq

# From stdin
cat data.ttl | oxigraph load --location ./data
```

### Format Conversion

```bash
# Convert Turtle to N-Quads
oxigraph convert --from-format ttl --to-format nq < input.ttl > output.nq

# Convert RDF/XML to Turtle
oxigraph convert --from-format rdf --to-format ttl < input.rdf > output.ttl
```

### Backup

```bash
# Dump database to N-Quads
oxigraph dump --location ./data > backup.nq

# Restore from backup
oxigraph load --location ./new-data --file backup.nq
```

---

## Environment Variables

### RocksDB Tuning

```bash
# Increase write buffer (default: ~128MB)
export ROCKSDB_TOTAL_WRITE_BUFFER_SIZE=2147483648  # 2GB

# Background jobs (default: 2)
export ROCKSDB_MAX_BACKGROUND_JOBS=8

# Compression type
export ROCKSDB_COMPRESSION_TYPE=lz4  # or snappy, zstd, none
```

### Rust Debugging

```bash
# Enable backtraces
export RUST_BACKTRACE=1
export RUST_BACKTRACE=full  # More detailed

# Enable logging
export RUST_LOG=debug
export RUST_LOG=oxigraph=trace  # Oxigraph-specific
export RUST_LOG=oxigraph::store=debug  # Module-specific
```

### Build Configuration

```bash
# Parallel build jobs
export CARGO_BUILD_JOBS=4

# Target directory
export CARGO_TARGET_DIR=/tmp/cargo-target
```

---

## Common SPARQL Patterns

### Filter Patterns

```sparql
# String contains
FILTER(CONTAINS(?name, "alice"))

# Regex match
FILTER(REGEX(?email, "@example\\.org$"))

# Numeric comparison
FILTER(?age >= 18 && ?age < 65)

# Language filter
FILTER(LANG(?label) = "en")

# Datatype filter
FILTER(DATATYPE(?value) = xsd:integer)

# Existence check
FILTER(BOUND(?optionalVar))
FILTER(!BOUND(?optionalVar))
```

### Optional Patterns

```sparql
SELECT ?person ?name ?email WHERE {
    ?person foaf:name ?name .
    OPTIONAL { ?person foaf:email ?email }
}
```

### Union Patterns

```sparql
SELECT ?contact WHERE {
    {
        ?person foaf:email ?contact .
    } UNION {
        ?person foaf:phone ?contact .
    }
}
```

### Property Paths

```sparql
# One or more
?person foaf:knows+ ?connection .

# Zero or more
?person foaf:knows* ?connection .

# Exactly one
?person foaf:knows ?friend .

# Alternative paths
?person foaf:knows|foaf:worksWith ?connection .

# Inverse
?person ^foaf:knows ?knower .  # Who knows ?person

# Sequence
?person foaf:knows/foaf:knows ?friendOfFriend .
```

### Aggregations

```sparql
SELECT ?category (COUNT(?item) AS ?count) (AVG(?price) AS ?avgPrice)
WHERE {
    ?item a ?category ;
          schema:price ?price .
}
GROUP BY ?category
HAVING (COUNT(?item) > 10)
ORDER BY DESC(?count)
```

### Subqueries

```sparql
SELECT ?person ?name WHERE {
    ?person foaf:name ?name .

    {
        SELECT ?person WHERE {
            ?person foaf:age ?age .
        }
        ORDER BY DESC(?age)
        LIMIT 10
    }
}
```

### Negation

```sparql
# NOT EXISTS
SELECT ?person WHERE {
    ?person a foaf:Person .
    FILTER NOT EXISTS {
        ?person foaf:email ?email .
    }
}

# MINUS
SELECT ?person WHERE {
    ?person a foaf:Person .
    MINUS {
        ?person foaf:email ?email .
    }
}
```

### VALUES

```sparql
# Inline data
SELECT ?person ?name WHERE {
    VALUES ?person {
        <http://example.org/alice>
        <http://example.org/bob>
    }
    ?person foaf:name ?name .
}

# Multiple variables
VALUES (?person ?age) {
    (<http://example.org/alice> 30)
    (<http://example.org/bob> 25)
}
```

### Federated Queries

```sparql
# Query remote SPARQL endpoint
SELECT ?name WHERE {
    SERVICE <http://dbpedia.org/sparql> {
        ?person foaf:name ?name .
        FILTER(LANG(?name) = "en")
    }
}
LIMIT 10
```

---

## HTTP API Endpoints (CLI Server)

### Query Endpoint

```bash
# SELECT query
curl -X POST http://localhost:7878/query \
  -H 'Content-Type: application/sparql-query' \
  -H 'Accept: application/sparql-results+json' \
  --data 'SELECT * WHERE { ?s ?p ?o } LIMIT 10'

# CONSTRUCT query
curl -X POST http://localhost:7878/query \
  -H 'Content-Type: application/sparql-query' \
  -H 'Accept: text/turtle' \
  --data 'CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o } LIMIT 10'
```

### Update Endpoint

```bash
curl -X POST http://localhost:7878/update \
  -H 'Content-Type: application/sparql-update' \
  --data 'INSERT DATA { <http://ex.org/s> <http://ex.org/p> "value" }'
```

### Graph Store Protocol

```bash
# Upload to default graph
curl -X POST http://localhost:7878/store \
  -H 'Content-Type: text/turtle' \
  -T data.ttl

# Upload to named graph
curl -X PUT http://localhost:7878/store?graph=http://example.org/graph \
  -H 'Content-Type: text/turtle' \
  -T data.ttl

# Get graph
curl http://localhost:7878/store?graph=http://example.org/graph

# Delete graph
curl -X DELETE http://localhost:7878/store?graph=http://example.org/graph
```

---

## Common Prefixes

```sparql
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>
PREFIX owl: <http://www.w3.org/2002/07/owl#>
PREFIX foaf: <http://xmlns.com/foaf/0.1/>
PREFIX dcterms: <http://purl.org/dc/terms/>
PREFIX schema: <http://schema.org/>
PREFIX skos: <http://www.w3.org/2004/02/skos/core#>
```

---

## Quick Debugging

### Check Store Contents

```sparql
# Count all triples
SELECT (COUNT(*) AS ?count) WHERE { ?s ?p ?o }

# Sample triples
SELECT * WHERE { ?s ?p ?o } LIMIT 10

# All predicates
SELECT DISTINCT ?p WHERE { ?s ?p ?o }

# All classes
SELECT DISTINCT ?class WHERE { ?s a ?class }

# Graph names
SELECT DISTINCT ?g WHERE { GRAPH ?g { ?s ?p ?o } }
```

### Performance Testing

```bash
# Time a query
time curl -X POST http://localhost:7878/query \
  -H 'Content-Type: application/sparql-query' \
  --data 'SELECT * WHERE { ?s ?p ?o } LIMIT 10000'

# Benchmark with Apache Bench
ab -n 100 -c 10 -p query.txt -T 'application/sparql-query' \
  http://localhost:7878/query
```

---

## Troubleshooting Quick Fixes

| Problem | Quick Fix |
|---------|-----------|
| **LLVM not found** | `sudo apt-get install clang` (Linux) / `xcode-select --install` (macOS) |
| **Port in use** | Change port: `--bind localhost:8080` |
| **Out of memory** | Use persistent store instead of in-memory |
| **Slow queries** | Add `LIMIT`, use indexes efficiently |
| **Can't import pyoxigraph** | `pip install --upgrade pip && pip install pyoxigraph` |
| **WASM not loading** | `await init()` before using in browser |
| **Submodule errors** | `git submodule update --init --recursive` |

---

## Additional Resources

- **Documentation:** [oxigraph.org](https://oxigraph.org/)
- **GitHub:** [github.com/oxigraph/oxigraph](https://github.com/oxigraph/oxigraph)
- **Discussions:** [github.com/oxigraph/oxigraph/discussions](https://github.com/oxigraph/oxigraph/discussions)
- **Gitter Chat:** [gitter.im/oxigraph/community](https://gitter.im/oxigraph/community)
- **SPARQL Spec:** [w3.org/TR/sparql11-query](https://www.w3.org/TR/sparql11-query/)

---

**Keep this cheatsheet handy for quick reference!**

**For detailed guides, see:**
- [Onboarding Guide](onboarding.md)
- [Learning Path](learning-path.md)
- [Quick Start](quick-start.md)
