# API Overview Reference

This document provides an overview of Oxigraph APIs across different programming languages, highlighting common patterns and key differences.

## Table of Contents

- [Rust API](#rust-api)
- [Python API](#python-api)
- [JavaScript API](#javascript-api)
- [API Comparison](#api-comparison)
- [Common Patterns](#common-patterns)

---

## Rust API

### Core Modules

The Rust API is organized into several modules:

```rust
oxigraph::model      // RDF data model (re-export of oxrdf)
oxigraph::io         // RDF parsing/serialization (re-export of oxrdfio)
oxigraph::sparql     // SPARQL query/results
oxigraph::store      // Database store
```

### Key Types

#### RDF Model (`oxigraph::model`)

```rust
use oxigraph::model::*;

// RDF terms
let iri = NamedNode::new("http://example.org/Alice")?;
let blank = BlankNode::default();
let literal = Literal::new_simple_literal("Alice");
let typed_literal = Literal::new_typed_literal("42", xsd::INTEGER);
let lang_literal = Literal::new_language_tagged_literal("hello", "en")?;

// RDF statements
let triple = Triple::new(
    iri.clone(),
    iri.clone(),
    literal.clone(),
);

let quad = Quad::new(
    iri.clone(),
    iri.clone(),
    literal.clone(),
    GraphName::DefaultGraph,
);

// Collections
let mut graph = Graph::new();
graph.insert(&triple);

let mut dataset = Dataset::new();
dataset.insert(&quad);
```

#### Store (`oxigraph::store`)

```rust
use oxigraph::store::Store;

// Create store
let store = Store::new()?;                    // In-memory
let store = Store::open("path/to/db")?;       // Persistent

// Open read-only
let store = Store::open_read_only("path/to/db")?;

// Add data
store.insert(&quad)?;

// Query data
for quad in store.quads_for_pattern(Some(&subject), None, None, None) {
    println!("{:?}", quad?);
}

// SPARQL query
use oxigraph::sparql::{QueryResults, SparqlEvaluator};

let evaluator = SparqlEvaluator::new();
let results = evaluator
    .parse_query("SELECT * WHERE { ?s ?p ?o }")?
    .on_store(&store)
    .execute()?;

match results {
    QueryResults::Solutions(solutions) => { /* ... */ }
    QueryResults::Graph(graph) => { /* ... */ }
    QueryResults::Boolean(result) => { /* ... */ }
}

// SPARQL update
store.update("INSERT DATA { <http://example.org/s> <http://example.org/p> <http://example.org/o> }")?;
```

#### I/O (`oxigraph::io`)

```rust
use oxigraph::io::{RdfFormat, RdfParser, RdfSerializer};

// Parsing
let parser = RdfParser::from_format(RdfFormat::Turtle)
    .with_base_iri("http://example.org/")?;

for quad in parser.for_reader(input_file) {
    store.insert(&quad?)?;
}

// Serialization
let mut serializer = RdfSerializer::from_format(RdfFormat::NTriples)
    .for_writer(output_file);

for quad in store.iter() {
    serializer.serialize_quad(&quad?)?;
}
```

### API Patterns

**Builder Pattern**:
```rust
let results = SparqlEvaluator::new()
    .parse_query(query_str)?
    .with_base_iri("http://example.org/")?
    .with_default_graph(&graph_name)
    .on_store(&store)
    .execute()?;
```

**Error Handling**:
```rust
use oxigraph::store::StorageError;

match store.insert(&quad) {
    Ok(_) => println!("Success"),
    Err(StorageError::Io(e)) => eprintln!("I/O error: {}", e),
    Err(e) => eprintln!("Other error: {}", e),
}
```

**Iterators**:
```rust
// Lazy iteration
for quad in store.iter() {
    // Process quad
}

// Collect into Vec
let quads: Vec<Quad> = store.iter()
    .collect::<Result<_, _>>()?;
```

### Documentation

- [docs.rs/oxigraph](https://docs.rs/oxigraph)
- [docs.rs/oxrdf](https://docs.rs/oxrdf)
- [docs.rs/spargebra](https://docs.rs/spargebra)

---

## Python API

### Installation

```bash
pip install pyoxigraph
```

### Core Classes

#### RDF Model

```python
from pyoxigraph import NamedNode, BlankNode, Literal, Triple, Quad, DefaultGraph

# RDF terms
iri = NamedNode("http://example.org/Alice")
blank = BlankNode()
literal = Literal("Alice")
typed_literal = Literal("42", datatype=NamedNode("http://www.w3.org/2001/XMLSchema#integer"))
lang_literal = Literal("hello", language="en")

# RDF statements
triple = Triple(iri, iri, literal)
quad = Quad(iri, iri, literal, DefaultGraph())
```

#### Store

```python
from pyoxigraph import Store

# Create store
store = Store()                          # In-memory
store = Store("path/to/db")              # Persistent

# Open read-only
store = Store.read_only("path/to/db")

# Add data
store.add(quad)

# Bulk load
store.load(input_data, format="text/turtle", base_iri="http://example.org/")

# Query patterns
for quad in store.quads_for_pattern(subject=iri, predicate=None, object=None, graph_name=None):
    print(quad)

# SPARQL query
results = store.query("SELECT * WHERE { ?s ?p ?o }")
for solution in results:
    print(solution["s"], solution["p"], solution["o"])

# SPARQL update
store.update("INSERT DATA { <http://example.org/s> <http://example.org/p> <http://example.org/o> }")
```

#### I/O

```python
# Parse from string
store.load(
    data="<http://example.org/s> <http://example.org/p> <http://example.org/o> .",
    format="text/turtle"
)

# Parse from file
with open("data.ttl") as f:
    store.load(f, format="text/turtle")

# Serialize
output = store.dump(format="application/n-triples")
print(output)
```

### API Patterns

**Context Managers**: Not used (Store manages resources automatically)

**Error Handling**:
```python
try:
    store.add(quad)
except OSError as e:
    print(f"Storage error: {e}")
except SyntaxError as e:
    print(f"Parse error: {e}")
```

**Iteration**:
```python
# Direct iteration
for quad in store:
    print(quad)

# Pattern iteration
for quad in store.quads_for_pattern(subject=iri):
    print(quad)
```

**Type Hints**:
```python
from typing import Optional
from pyoxigraph import Store, Quad, QuerySolutions

def query_store(store: Store, query: str) -> QuerySolutions:
    return store.query(query)
```

### Documentation

- [PyPI: pyoxigraph](https://pypi.org/project/pyoxigraph/)
- [ReadTheDocs](https://pyoxigraph.readthedocs.io/)

---

## JavaScript API

### Installation

```bash
npm install oxigraph
```

### Core Classes

#### RDF Model

```javascript
import { NamedNode, BlankNode, Literal, Triple, Quad, DefaultGraph } from 'oxigraph';

// RDF terms
const iri = new NamedNode("http://example.org/Alice");
const blank = new BlankNode();
const literal = new Literal("Alice");
const typedLiteral = new Literal("42", new NamedNode("http://www.w3.org/2001/XMLSchema#integer"));
const langLiteral = new Literal("hello", "en");

// RDF statements
const triple = new Triple(iri, iri, literal);
const quad = new Quad(iri, iri, literal, new DefaultGraph());
```

#### Store

```javascript
import { Store } from 'oxigraph';

// Create store (in-memory only in browser)
const store = new Store();

// Add data
store.add(quad);

// Load from string
store.load(
    `<http://example.org/s> <http://example.org/p> <http://example.org/o> .`,
    { format: "text/turtle", baseIri: "http://example.org/" }
);

// Query patterns
const quads = store.match(iri, null, null, null);
for (const quad of quads) {
    console.log(quad);
}

// SPARQL query
const results = store.query("SELECT * WHERE { ?s ?p ?o }");
for (const solution of results) {
    console.log(solution.get("s"), solution.get("p"), solution.get("o"));
}

// SPARQL update
store.update("INSERT DATA { <http://example.org/s> <http://example.org/p> <http://example.org/o> }");
```

#### Async Operations

```javascript
// Async query
const results = await store.queryAsync("SELECT * WHERE { ?s ?p ?o }");

// Async update
await store.updateAsync("INSERT DATA { ... }");
```

#### I/O

```javascript
// Serialize
const ntriples = store.dump({ format: "application/n-triples" });
console.log(ntriples);
```

### API Patterns

**Array-like Interface**:
```javascript
// Size
console.log(store.size);
console.log(store.length);

// Iteration
for (const quad of store) {
    console.log(quad);
}

// Array methods
store.forEach((quad) => console.log(quad));

const subjects = store.map((quad) => quad.subject);

const filtered = store.filter((quad) => quad.predicate.equals(someNode));
```

**RDF/JS Compatibility**:
```javascript
// Implements RDF/JS Dataset interface
// https://rdf.js.org/dataset-spec/

store.add(quad);
store.delete(quad);
store.has(quad);
store.match(subject, predicate, object, graph);
```

**Error Handling**:
```javascript
try {
    store.add(quad);
} catch (error) {
    console.error("Error:", error.message);
}
```

**Promises**:
```javascript
// Async operations return Promises
store.queryAsync(query)
    .then(results => {
        for (const solution of results) {
            console.log(solution);
        }
    })
    .catch(error => console.error(error));
```

### TypeScript Support

Full TypeScript definitions included:

```typescript
import { Store, NamedNode, Quad, QueryResults } from 'oxigraph';

const store: Store = new Store();
const node: NamedNode = new NamedNode("http://example.org/");
const quad: Quad = new Quad(node, node, node);

store.add(quad);

const results: QueryResults = store.query("SELECT * WHERE { ?s ?p ?o }");
```

### Documentation

- [npm: oxigraph](https://www.npmjs.com/package/oxigraph)
- TypeScript definitions in package

---

## API Comparison

### Store Creation

| Language | In-Memory | Persistent | Read-Only |
|----------|-----------|------------|-----------|
| Rust | `Store::new()` | `Store::open(path)` | `Store::open_read_only(path)` |
| Python | `Store()` | `Store(path)` | `Store.read_only(path)` |
| JavaScript | `new Store()` | N/A (browser) | N/A |

### Adding Quads

| Language | Syntax |
|----------|--------|
| Rust | `store.insert(&quad)?` |
| Python | `store.add(quad)` |
| JavaScript | `store.add(quad)` |

### Pattern Matching

| Language | Syntax |
|----------|--------|
| Rust | `store.quads_for_pattern(s, p, o, g)` |
| Python | `store.quads_for_pattern(s, p, o, g)` |
| JavaScript | `store.match(s, p, o, g)` |

### SPARQL Query

| Language | Sync | Async |
|----------|------|-------|
| Rust | `evaluator.parse_query(q)?.on_store(&store).execute()?` | N/A |
| Python | `store.query(query)` | N/A |
| JavaScript | `store.query(query)` | `store.queryAsync(query)` |

### Loading RDF

| Language | Syntax |
|----------|--------|
| Rust | `RdfParser::from_format(fmt).for_reader(r)` |
| Python | `store.load(data, format=fmt)` |
| JavaScript | `store.load(data, {format: fmt})` |

### Serializing RDF

| Language | Syntax |
|----------|--------|
| Rust | `RdfSerializer::from_format(fmt).for_writer(w)` |
| Python | `store.dump(format=fmt)` |
| JavaScript | `store.dump({format: fmt})` |

---

## Common Patterns

### Creating and Populating a Store

**Rust**:
```rust
let store = Store::new()?;
let quad = Quad::new(subject, predicate, object, graph);
store.insert(&quad)?;
```

**Python**:
```python
store = Store()
quad = Quad(subject, predicate, object, graph)
store.add(quad)
```

**JavaScript**:
```javascript
const store = new Store();
const quad = new Quad(subject, predicate, object, graph);
store.add(quad);
```

---

### Loading from File

**Rust**:
```rust
use std::fs::File;

let file = File::open("data.ttl")?;
let parser = RdfParser::from_format(RdfFormat::Turtle);
for quad in parser.for_reader(file) {
    store.insert(&quad?)?;
}
```

**Python**:
```python
with open("data.ttl") as f:
    store.load(f, format="text/turtle")
```

**JavaScript**:
```javascript
// Browser: fetch
const response = await fetch("data.ttl");
const text = await response.text();
store.load(text, { format: "text/turtle" });

// Node.js: fs
const fs = require('fs');
const data = fs.readFileSync("data.ttl", "utf8");
store.load(data, { format: "text/turtle" });
```

---

### Executing SPARQL SELECT

**Rust**:
```rust
let evaluator = SparqlEvaluator::new();
let results = evaluator
    .parse_query("SELECT ?s WHERE { ?s ?p ?o }")?
    .on_store(&store)
    .execute()?;

if let QueryResults::Solutions(solutions) = results {
    for solution in solutions {
        let solution = solution?;
        println!("{:?}", solution.get("s"));
    }
}
```

**Python**:
```python
for solution in store.query("SELECT ?s WHERE { ?s ?p ?o }"):
    print(solution["s"])
```

**JavaScript**:
```javascript
const results = store.query("SELECT ?s WHERE { ?s ?p ?o }");
for (const solution of results) {
    console.log(solution.get("s"));
}
```

---

### Executing SPARQL CONSTRUCT

**Rust**:
```rust
let results = evaluator
    .parse_query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }")?
    .on_store(&store)
    .execute()?;

if let QueryResults::Graph(quads) = results {
    for quad in quads {
        println!("{:?}", quad?);
    }
}
```

**Python**:
```python
for quad in store.query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }"):
    print(quad)
```

**JavaScript**:
```javascript
const results = store.query("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }");
for (const quad of results) {
    console.log(quad);
}
```

---

### Executing SPARQL ASK

**Rust**:
```rust
let results = evaluator
    .parse_query("ASK { ?s ?p ?o }")?
    .on_store(&store)
    .execute()?;

if let QueryResults::Boolean(result) = results {
    println!("Result: {}", result);
}
```

**Python**:
```python
result = store.query("ASK { ?s ?p ?o }")
print(result)  # True or False
```

**JavaScript**:
```javascript
const result = store.query("ASK { ?s ?p ?o }");
console.log(result);  // true or false
```

---

### Iterating Over All Quads

**Rust**:
```rust
for quad in store.iter() {
    let quad = quad?;
    println!("{:?}", quad);
}
```

**Python**:
```python
for quad in store:
    print(quad)
```

**JavaScript**:
```javascript
for (const quad of store) {
    console.log(quad);
}
```

---

### Filtering Quads by Pattern

**Rust**:
```rust
let subject = NamedNode::new("http://example.org/Alice")?;
for quad in store.quads_for_pattern(Some(subject.as_ref()), None, None, None) {
    println!("{:?}", quad?);
}
```

**Python**:
```python
subject = NamedNode("http://example.org/Alice")
for quad in store.quads_for_pattern(subject=subject):
    print(quad)
```

**JavaScript**:
```javascript
const subject = new NamedNode("http://example.org/Alice");
for (const quad of store.match(subject, null, null, null)) {
    console.log(quad);
}
```

---

### Serializing Store to String

**Rust**:
```rust
let mut output = Vec::new();
let mut serializer = RdfSerializer::from_format(RdfFormat::NTriples)
    .for_writer(&mut output);

for quad in store.iter() {
    serializer.serialize_quad(&quad?)?;
}

let output_str = String::from_utf8(output)?;
```

**Python**:
```python
output = store.dump(format="application/n-triples")
print(output)
```

**JavaScript**:
```javascript
const output = store.dump({ format: "application/n-triples" });
console.log(output);
```

---

## Type Mapping

### RDF Terms

| Concept | Rust | Python | JavaScript |
|---------|------|--------|------------|
| IRI | `NamedNode` | `NamedNode` | `NamedNode` |
| Blank Node | `BlankNode` | `BlankNode` | `BlankNode` |
| Literal | `Literal` | `Literal` | `Literal` |
| Variable | `Variable` | N/A | N/A |
| Any Term | `Term` | Union type | Union type |

### RDF Statements

| Concept | Rust | Python | JavaScript |
|---------|------|--------|------------|
| Triple | `Triple` | `Triple` | `Triple` |
| Quad | `Quad` | `Quad` | `Quad` |
| Graph Name | `GraphName` | `NamedNode\|BlankNode\|DefaultGraph` | `NamedNode\|BlankNode\|DefaultGraph` |

### Collections

| Concept | Rust | Python | JavaScript |
|---------|------|--------|------------|
| Graph | `Graph` | N/A | N/A |
| Dataset | `Dataset` | N/A | N/A |
| Store | `Store` | `Store` | `Store` |

---

## Feature Availability

| Feature | Rust | Python | JavaScript |
|---------|------|--------|------------|
| Persistent Storage | Yes | Yes | No (in-memory only) |
| Read-only Mode | Yes | Yes | No |
| Bulk Loading | Yes | Yes | No |
| Transactions | Yes | No (atomic operations) | No |
| HTTP Client (SERVICE) | Yes* | Yes* | No |
| GeoSPARQL | No | No | Yes* |
| Async I/O | Yes* | No | Yes |
| RDF/JS Compatibility | N/A | N/A | Yes |

*Requires feature flag

---

## Best Practices

### Rust

1. Use `?` operator for error propagation
2. Prefer iterator chains over collecting into Vec
3. Use `as_ref()` when passing owned values to pattern matching
4. Enable `rocksdb` feature for persistence
5. Use bulk loader for large imports

### Python

1. Use context managers for file I/O
2. Catch specific exceptions (OSError, SyntaxError)
3. Use generator expressions for large datasets
4. Specify format explicitly for load/dump operations
5. Type hint function signatures for clarity

### JavaScript

1. Use async/await for query operations
2. Leverage TypeScript for type safety
3. Use Array methods (map, filter, forEach) on Store
4. Handle Promise rejections properly
5. Use `try...catch` for error handling

---

## Further Reading

- [Rust API Documentation](https://docs.rs/oxigraph)
- [Python API Documentation](https://pyoxigraph.readthedocs.io/)
- [JavaScript Package](https://www.npmjs.com/package/oxigraph)
- [RDF/JS Specification](https://rdf.js.org/)
- [W3C RDF Concepts](https://www.w3.org/TR/rdf11-concepts/)
