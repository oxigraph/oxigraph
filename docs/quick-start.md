# Quick Start Guide

Get started with Oxigraph in 5 minutes! This guide provides copy-paste ready examples for each platform.

## What is Oxigraph?

Oxigraph is a fast, compliant graph database implementing the SPARQL standard. It supports:
- SPARQL 1.1 Query, Update, and Federated Query
- Multiple RDF formats: Turtle, TriG, N-Triples, N-Quads, RDF/XML, JSON-LD
- Persistent storage (RocksDB) and in-memory options
- Multiple language bindings: Rust, Python, JavaScript

Choose your platform below to get started:

## Quick Start by Platform

### Rust

#### Installation

```bash
cargo add oxigraph
```

#### Minimal Example (In-Memory)

```rust
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create an in-memory store
    let store = Store::new()?;

    // Create some RDF data
    let ex = NamedNode::new("http://example.com")?;
    let name = NamedNode::new("http://schema.org/name")?;
    let quad = Quad::new(
        ex.clone(),
        name,
        Literal::new_simple_literal("Example"),
        GraphName::DefaultGraph,
    );

    // Insert the quad
    store.insert(&quad)?;

    // Query using SPARQL
    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query("SELECT ?name WHERE { <http://example.com> <http://schema.org/name> ?name }")?
        .on_store(&store)
        .execute()?
    {
        while let Some(solution) = solutions.next() {
            let solution = solution?;
            println!("Name: {}", solution.get("name").unwrap());
        }
    }

    Ok(())
}
```

#### Persistent Store Example

```rust
use oxigraph::store::Store;
use oxigraph::io::RdfFormat;
use oxigraph::sparql::SparqlEvaluator;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a persistent store
    let store = Store::open("./my_database")?;

    // Load an RDF file
    let data = std::fs::read_to_string("data.ttl")?;
    store.load_from_reader(RdfFormat::Turtle, data.as_bytes())?;

    // Query
    let results = SparqlEvaluator::new()
        .parse_query("SELECT * WHERE { ?s ?p ?o } LIMIT 10")?
        .on_store(&store)
        .execute()?;

    Ok(())
}
```

**Next steps:** Read the [full Rust documentation](https://docs.rs/oxigraph)

---

### Python

#### Installation

```bash
pip install pyoxigraph
```

Or with conda:
```bash
conda install -c conda-forge pyoxigraph
```

#### Minimal Example

```python
from pyoxigraph import Store, NamedNode, Literal, Quad, DefaultGraph

# Create an in-memory store
store = Store()

# Create some RDF data
ex = NamedNode("http://example.com")
name = NamedNode("http://schema.org/name")
quad = Quad(ex, name, Literal("Example"), DefaultGraph())

# Insert the quad
store.add(quad)

# Query using SPARQL
for result in store.query("SELECT ?name WHERE { <http://example.com> <http://schema.org/name> ?name }"):
    print(f"Name: {result['name'].value}")
```

#### Persistent Store Example

```python
from pyoxigraph import Store, RdfFormat

# Create a persistent store
store = Store("./my_database")

# Load an RDF file
store.load(path="data.ttl", format=RdfFormat.TURTLE)

# Query
for result in store.query("SELECT * WHERE { ?s ?p ?o } LIMIT 10"):
    print(f"Subject: {result['s']}")
```

#### Loading from URLs

```python
import requests
from pyoxigraph import Store, RdfFormat

store = Store()

# Fetch and load remote RDF data
response = requests.get("https://www.w3.org/People/Berners-Lee/card")
store.load(input=response.content, format=RdfFormat.TURTLE)

# Query the data
for result in store.query("SELECT ?name WHERE { ?person <http://xmlns.com/foaf/0.1/name> ?name } LIMIT 5"):
    print(result["name"].value)
```

**Next steps:** Read the [full Python documentation](https://pyoxigraph.readthedocs.io/)

---

### JavaScript (Node.js & Browser)

#### Installation

```bash
npm install oxigraph
```

#### Node.js Example

```javascript
const oxigraph = require('oxigraph');

// Create an in-memory store
const store = new oxigraph.Store();

// Create some RDF data
const ex = oxigraph.namedNode("http://example.com");
const name = oxigraph.namedNode("http://schema.org/name");
const triple = oxigraph.triple(ex, name, oxigraph.literal("Example"));

// Insert the triple
store.add(triple);

// Query using SPARQL
for (const binding of store.query("SELECT ?name WHERE { <http://example.com> <http://schema.org/name> ?name }")) {
    console.log(`Name: ${binding.get("name").value}`);
}
```

#### ES Module Example

```javascript
import oxigraph from 'oxigraph/node.js';
import { readFileSync } from 'fs';

const store = new oxigraph.Store();
store.add(oxigraph.triple(
    oxigraph.namedNode("http://example.com"),
    oxigraph.namedNode("http://schema.org/name"),
    oxigraph.literal("Example")
));

// Load from file
const data = readFileSync('data.ttl', 'utf-8');
store.load(data, { format: "text/turtle" });
```

#### Browser Example

```html
<!DOCTYPE html>
<html>
<head>
    <title>Oxigraph Browser Example</title>
</head>
<body>
    <h1>Oxigraph in Browser</h1>
    <div id="results"></div>

    <script type="module">
        import init, * as oxigraph from './node_modules/oxigraph/web.js';

        (async function () {
            // Initialize WebAssembly
            await init();

            // Create store
            const store = new oxigraph.Store();

            // Add data
            const ex = oxigraph.namedNode("http://example.com");
            const name = oxigraph.namedNode("http://schema.org/name");
            store.add(oxigraph.triple(ex, name, oxigraph.literal("Example")));

            // Query
            const results = [];
            for (const binding of store.query("SELECT ?name WHERE { ?s ?name }")) {
                results.push(binding.get("name").value);
            }

            // Display results
            document.getElementById('results').innerHTML = results.join('<br>');
        })();
    </script>
</body>
</html>
```

**Next steps:** Read the [JavaScript API documentation](../js/README.md)

---

### CLI Server (Docker)

The fastest way to get a SPARQL endpoint running!

#### Start with Docker

```bash
# Create a data directory
mkdir oxigraph-data

# Start the server
docker run -d \
  --name oxigraph \
  -v $PWD/oxigraph-data:/data \
  -p 7878:7878 \
  ghcr.io/oxigraph/oxigraph:latest \
  serve --location /data --bind 0.0.0.0:7878
```

#### Access the Server

Open your browser: http://localhost:7878

You'll see a SPARQL query interface powered by YASGUI!

#### Load Data via HTTP

```bash
# Load a Turtle file
curl -X POST \
  -H 'Content-Type: text/turtle' \
  -T data.ttl \
  http://localhost:7878/store

# Load into a named graph
curl -X POST \
  -H 'Content-Type: text/turtle' \
  -T data.ttl \
  "http://localhost:7878/store?graph=http://example.com/mygraph"
```

#### Query via HTTP

```bash
# Execute a SELECT query
curl -X POST \
  -H 'Content-Type: application/sparql-query' \
  -H 'Accept: application/sparql-results+json' \
  --data 'SELECT * WHERE { ?s ?p ?o } LIMIT 10' \
  http://localhost:7878/query

# Execute a CONSTRUCT query
curl -X POST \
  -H 'Content-Type: application/sparql-query' \
  -H 'Accept: text/turtle' \
  --data 'CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o } LIMIT 10' \
  http://localhost:7878/query
```

#### Update via HTTP

```bash
# Insert data
curl -X POST \
  -H 'Content-Type: application/sparql-update' \
  --data 'INSERT DATA { <http://example.com/s> <http://example.com/p> "value" }' \
  http://localhost:7878/update

# Delete data
curl -X POST \
  -H 'Content-Type: application/sparql-update' \
  --data 'DELETE WHERE { <http://example.com/s> ?p ?o }' \
  http://localhost:7878/update
```

---

### CLI Server (Native Binary)

#### Installation

```bash
# Via Cargo
cargo install oxigraph-cli

# Via UV (Python package manager)
uvx oxigraph --help

# Via Conda
conda install -c conda-forge oxigraph-server
```

Or download pre-built binaries from [GitHub Releases](https://github.com/oxigraph/oxigraph/releases/latest).

#### Start the Server

```bash
# Start with persistent storage
oxigraph serve --location ./my-database

# Custom port
oxigraph serve --location ./my-database --bind localhost:8080
```

#### Bulk Load Data

```bash
# Load data before starting the server (much faster!)
oxigraph load --location ./my-database --file data.nq
oxigraph load --location ./my-database --file data.ttl
```

#### Convert RDF Formats

```bash
# Convert Turtle to N-Quads
oxigraph convert --from-format ttl --to-format nq < input.ttl > output.nq

# Convert RDF/XML to Turtle
oxigraph convert --from-format rdf --to-format ttl < input.rdf > output.ttl
```

**Next steps:** Run `oxigraph --help` for all options

---

## Common Operations

### Loading RDF Data

<details>
<summary><b>Rust</b></summary>

```rust
use oxigraph::store::Store;
use oxigraph::io::RdfFormat;

let store = Store::open("./db")?;

// From file
let data = std::fs::read_to_string("data.ttl")?;
store.load_from_reader(RdfFormat::Turtle, data.as_bytes())?;

// From string
let turtle_data = r#"<http://example.com/s> <http://example.com/p> "value" ."#;
store.load_from_reader(RdfFormat::Turtle, turtle_data.as_bytes())?;

// Bulk load (faster for large files)
let file_data = std::fs::read_to_string("large-file.nq")?;
let mut loader = store.bulk_loader();
loader.load_from_reader(RdfFormat::NQuads, file_data.as_bytes())?;
loader.commit()?;
```

</details>

<details>
<summary><b>Python</b></summary>

```python
from pyoxigraph import Store, RdfFormat

store = Store("./db")

# From file
store.load(path="data.ttl", format=RdfFormat.TURTLE)

# From string
data = """
<http://ex.org/s> <http://ex.org/p> "value" .
"""
store.load(input=data.encode(), format=RdfFormat.TURTLE)

# Bulk load
store.bulk_load(path="large-file.nq", format=RdfFormat.N_QUADS)
```

</details>

<details>
<summary><b>JavaScript</b></summary>

```javascript
const store = new oxigraph.Store();

// From string
const data = `
<http://ex.org/s> <http://ex.org/p> "value" .
`;
store.load(data, { format: "text/turtle" });

// In Node.js, from file
const fs = require('fs');
const fileData = fs.readFileSync('data.ttl', 'utf-8');
store.load(fileData, { format: "text/turtle" });
```

</details>

### Executing SPARQL Queries

<details>
<summary><b>Rust</b></summary>

```rust
use oxigraph::sparql::{QueryResults, SparqlEvaluator};

// SELECT query
if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
    .parse_query("SELECT ?s ?p ?o WHERE { ?s ?p ?o } LIMIT 10")?
    .on_store(&store)
    .execute()?
{
    while let Some(solution) = solutions.next() {
        let s = solution?;
        println!("{:?}", s.get("s"));
    }
}

// ASK query
if let QueryResults::Boolean(result) = SparqlEvaluator::new()
    .parse_query("ASK { ?s ?p ?o }")?
    .on_store(&store)
    .execute()?
{
    println!("Has triples: {}", result);
}

// CONSTRUCT query
if let QueryResults::Graph(graph) = SparqlEvaluator::new()
    .parse_query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o } LIMIT 10")?
    .on_store(&store)
    .execute()?
{
    for triple in graph {
        println!("{:?}", triple?);
    }
}
```

</details>

<details>
<summary><b>Python</b></summary>

```python
# SELECT query
for solution in store.query("SELECT ?s ?p ?o WHERE { ?s ?p ?o } LIMIT 10"):
    print(solution["s"], solution["p"], solution["o"])

# ASK query
result = store.query("ASK { ?s ?p ?o }")
print(f"Has triples: {result}")

# CONSTRUCT query
for triple in store.query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o } LIMIT 10"):
    print(triple)
```

</details>

<details>
<summary><b>JavaScript</b></summary>

```javascript
// SELECT query
for (const solution of store.query("SELECT ?s ?p ?o WHERE { ?s ?p ?o } LIMIT 10")) {
    console.log(solution.get("s").value);
}

// ASK query
const hasTriples = store.query("ASK { ?s ?p ?o }");
console.log(`Has triples: ${hasTriples}`);

// CONSTRUCT query
const triples = store.query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o } LIMIT 10");
for (const triple of triples) {
    console.log(triple);
}
```

</details>

### SPARQL Updates

<details>
<summary><b>Rust</b></summary>

```rust
use oxigraph::sparql::SparqlEvaluator;

// Insert data
SparqlEvaluator::new()
    .parse_update("INSERT DATA {
        <http://example.com/s> <http://example.com/p> 'value'
    }")?
    .on_store(&store)
    .execute()?;

// Delete data
SparqlEvaluator::new()
    .parse_update("DELETE WHERE {
        <http://example.com/s> ?p ?o
    }")?
    .on_store(&store)
    .execute()?;

// Delete/Insert
SparqlEvaluator::new()
    .parse_update("
        DELETE { ?s <http://example.com/oldProp> ?o }
        INSERT { ?s <http://example.com/newProp> ?o }
        WHERE { ?s <http://example.com/oldProp> ?o }
    ")?
    .on_store(&store)
    .execute()?;
```

</details>

<details>
<summary><b>Python</b></summary>

```python
# Insert data
store.update("INSERT DATA { <http://example.com/s> <http://example.com/p> 'value' }")

# Delete data
store.update("DELETE WHERE { <http://example.com/s> ?p ?o }")

# Delete/Insert
store.update("""
    DELETE { ?s <http://example.com/oldProp> ?o }
    INSERT { ?s <http://example.com/newProp> ?o }
    WHERE { ?s <http://example.com/oldProp> ?o }
""")
```

</details>

<details>
<summary><b>JavaScript</b></summary>

```javascript
// Insert data
store.update("INSERT DATA { <http://example.com/s> <http://example.com/p> 'value' }");

// Delete data
store.update("DELETE WHERE { <http://example.com/s> ?p ?o }");

// Delete/Insert
store.update(`
    DELETE { ?s <http://example.com/oldProp> ?o }
    INSERT { ?s <http://example.com/newProp> ?o }
    WHERE { ?s <http://example.com/oldProp> ?o }
`);
```

</details>

---

## Next Steps

### Learn More
- [Installation Guide](installation.md) - Detailed installation instructions
- [FAQ](faq.md) - Frequently asked questions
- [Architecture](https://github.com/oxigraph/oxigraph/wiki/Architecture) - Internal design

### API Documentation
- [Rust API docs](https://docs.rs/oxigraph)
- [Python API docs](https://pyoxigraph.readthedocs.io/)
- [JavaScript API](../js/README.md)
- [CLI documentation](../cli/README.md)

### Community
- [GitHub Discussions](https://github.com/oxigraph/oxigraph/discussions) - Ask questions
- [Gitter Chat](https://gitter.im/oxigraph/community) - Real-time chat
- [Issue Tracker](https://github.com/oxigraph/oxigraph/issues) - Report bugs

### Examples & Tutorials
- [Example applications](https://github.com/oxigraph/oxigraph/tree/main/examples)
- [Benchmark suite](../bench/README.md)
- [W3C Test Suites](../testsuite/README.md)

---

## Troubleshooting

### Python Installation Issues

If `pip install pyoxigraph` fails:

```bash
# Try with pre-built wheel
pip install --upgrade pip
pip install pyoxigraph

# Or use conda
conda install -c conda-forge pyoxigraph
```

### JavaScript WebAssembly Issues

Make sure your environment supports WebAssembly reference types:
- Node.js 18+ required
- Modern browsers (Chrome 90+, Firefox 89+, Safari 15+)

### Rust Compilation Issues

If building from source fails:

```bash
# Make sure you have Clang installed (required for RocksDB)
# On Ubuntu/Debian:
sudo apt-get install clang

# On macOS:
xcode-select --install

# Clone with submodules
git clone --recursive https://github.com/oxigraph/oxigraph.git
```

### Docker Issues

If the Docker container can't access data:

```bash
# Make sure the volume is mounted correctly
docker run -v $(pwd)/data:/data ...  # Linux/macOS
docker run -v %CD%/data:/data ...     # Windows CMD
docker run -v ${PWD}/data:/data ...   # Windows PowerShell
```

---

## Performance Tips

1. **Use bulk loading** for importing large datasets
2. **Use transactions** for batch operations in Rust/Python
3. **Enable HTTP client** for federated queries (Rust feature flag)
4. **Use appropriate RDF formats**: N-Quads/N-Triples are fastest to parse
5. **Index considerations**: Oxigraph maintains SPO, POS, and OSP indexes automatically

---

## Quick Reference

### Supported RDF Formats

| Format | MIME Type | Extension |
|--------|-----------|-----------|
| Turtle | `text/turtle` | `.ttl` |
| TriG | `application/trig` | `.trig` |
| N-Triples | `application/n-triples` | `.nt` |
| N-Quads | `application/n-quads` | `.nq` |
| RDF/XML | `application/rdf+xml` | `.rdf` |
| JSON-LD | `application/ld+json` | `.jsonld` |
| N3 | `text/n3` | `.n3` |

### Supported SPARQL Result Formats

| Format | MIME Type |
|--------|-----------|
| JSON | `application/sparql-results+json` |
| XML | `application/sparql-results+xml` |
| CSV | `text/csv` |
| TSV | `text/tab-separated-values` |

### Default Server Endpoints

- **SPARQL Query**: `POST /query`
- **SPARQL Update**: `POST /update`
- **Graph Store Protocol**: `GET/POST/PUT/DELETE /store`
- **Web UI**: `GET /` (browser interface)

---

**Ready to build something amazing?** Start with one of the examples above and explore the full documentation!
