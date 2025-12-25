# How to Import RDF Data

This guide explains different ways to import RDF data into Oxigraph across different platforms and use cases.

## Supported Formats

Oxigraph supports the following RDF formats for import:
- **Turtle** (`.ttl`)
- **N-Triples** (`.nt`)
- **N-Quads** (`.nq`)
- **TriG** (`.trig`)
- **RDF/XML** (`.rdf`, `.owl`)
- **N3** (`.n3`)
- **JSON-LD** (`.jsonld`)

## Using the CLI Server

### Basic Import via HTTP

Upload data to the server using the Graph Store HTTP Protocol:

```bash
# Import into default graph
curl -X POST -H 'Content-Type: text/turtle' \
  -T data.ttl http://localhost:7878/store?default

# Import into named graph
curl -X POST -H 'Content-Type: text/turtle' \
  -T data.ttl "http://localhost:7878/store?graph=http://example.com/mygraph"
```

### Import Complete Datasets

Use dataset formats (N-Quads, TriG) to import data into multiple graphs:

```bash
# Import N-Quads dataset
curl -X POST -H 'Content-Type: application/n-quads' \
  -T dataset.nq http://localhost:7878/store

# Import TriG dataset
curl -X POST -H 'Content-Type: application/trig' \
  -T dataset.trig http://localhost:7878/store
```

### Bulk Loading (Offline)

For large datasets, use the bulk loader for optimal performance:

```bash
# Load single file
oxigraph load --location /path/to/store --file data.nq

# Load multiple files in parallel
oxigraph load --location /path/to/store \
  --file data1.nq --file data2.ttl --file data3.rdf

# Load from stdin
cat data.ttl | oxigraph load --location /path/to/store --format ttl

# Load into specific graph
oxigraph load --location /path/to/store \
  --file data.ttl --graph http://example.com/mygraph
```

### Bulk Loading with Options

```bash
# Lenient mode (skip parsing errors)
oxigraph load --location /path/to/store \
  --file data.ttl --lenient

# Non-atomic loading (faster but less safe)
oxigraph load --location /path/to/store \
  --file large-data.nq --non-atomic

# Specify base IRI
oxigraph load --location /path/to/store \
  --file data.ttl --base http://example.com/

# Specify format explicitly
oxigraph load --location /path/to/store \
  --file data.txt --format ttl
```

### Loading Compressed Files

Oxigraph automatically handles gzipped files:

```bash
oxigraph load --location /path/to/store --file data.nq.gz
```

## Using Rust API

### Basic Import

```rust
use oxigraph::store::Store;
use oxrdfio::RdfFormat;

let store = Store::open("data")?;

// Load from file
let file_data = std::fs::read_to_string("data.ttl")?;
store.load_from_reader(RdfFormat::Turtle, file_data.as_bytes())?;

// Load from string
let data = r#"
@prefix ex: <http://example.com/> .
ex:subject ex:predicate "object" .
"#;
store.load_from_reader(RdfFormat::Turtle, data.as_bytes())?;
```

### Import into Named Graph

```rust
use oxrdfio::RdfParser;
use oxigraph::model::NamedNodeRef;

// Create a parser that loads into a specific named graph
let parser = RdfParser::from_format(RdfFormat::Turtle)
    .with_default_graph(NamedNodeRef::new("http://example.com/mygraph")?);

let file_data = std::fs::read_to_string("data.ttl")?;
store.load_from_reader(parser, file_data.as_bytes())?;
```

### Bulk Loading

For optimal performance when importing large amounts of data:

```rust
use oxigraph::store::Store;
use oxrdfio::RdfFormat;
use std::fs::File;
use std::io::BufReader;

let store = Store::open("data")?;

// Create bulk loader with progress tracking
let mut loader = store.bulk_loader()
    .on_progress(|size| {
        if size % 100000 == 0 {
            eprintln!("Loaded {} triples", size);
        }
    });

// Load data from file
let file = File::open("large-data.nq")?;
loader.load_from_reader(RdfFormat::NQuads, BufReader::new(file))?;

// Commit all changes
loader.commit()?;
```

### Bulk Loading with Error Handling

```rust
use std::fs::File;
use std::io::BufReader;

let mut loader = store.bulk_loader()
    .on_parse_error(|error| {
        eprintln!("Parse error: {}", error);
        Ok(()) // Continue on error
    })
    .on_progress(|size| {
        if size % 100000 == 0 {
            eprintln!("{} triples loaded", size);
        }
    });

let file = File::open("data.nq")?;
loader.load_from_reader(RdfFormat::NQuads, BufReader::new(file))?;
loader.commit()?;
```

### Non-Atomic Bulk Loading

For maximum speed (trades safety for performance):

```rust
use std::fs::File;
use std::io::BufReader;

let mut loader = store.bulk_loader()
    .without_atomicity()
    .on_progress(|size| {
        if size % 1000000 == 0 {
            eprintln!("{} triples", size);
        }
    });

let file = File::open("huge-data.nq")?;
loader.load_from_reader(RdfFormat::NQuads, BufReader::new(file))?;
loader.commit()?;
```

## Using Python API

### Basic Import

```python
from pyoxigraph import Store, RdfFormat

store = Store("data")

# Load from file
store.load(input=open("data.ttl", "rb"), format=RdfFormat.TURTLE)

# Load from string
data = """
@prefix ex: <http://example.com/> .
ex:subject ex:predicate "object" .
"""
store.load(input=data.encode(), format=RdfFormat.TURTLE)
```

### Import into Named Graph

```python
from pyoxigraph import Store, NamedNode, RdfFormat

store = Store("data")
graph = NamedNode("http://example.com/mygraph")

# Load into named graph
store.load(
    input=open("data.ttl", "rb"),
    format=RdfFormat.TURTLE,
    to_graph=graph
)
```

### Bulk Loading

```python
from pyoxigraph import Store, parse, RdfFormat

store = Store("data")

# Read quads from file
quads = parse(
    input=open("large-data.nq", "rb"),
    format=RdfFormat.N_QUADS
)

# Bulk extend (faster for large datasets)
store.bulk_extend(quads)
```

### Import with Base IRI

```python
store.load(
    input=open("data.ttl", "rb"),
    format=RdfFormat.TURTLE,
    base_iri="http://example.com/"
)
```

## Using JavaScript API

### Browser/Node.js Import

```javascript
import { Store } from 'oxigraph';

const store = new Store();

// Load from string
const data = `
@prefix ex: <http://example.com/> .
ex:subject ex:predicate "object" .
`;

store.load(data, {
    format: 'text/turtle',
    baseIri: 'http://example.com/'
});
```

### Import into Named Graph

```javascript
import { Store, NamedNode } from 'oxigraph';

const store = new Store();
const graph = new NamedNode('http://example.com/mygraph');

store.load(data, {
    format: 'text/turtle',
    toGraphName: graph
});
```

### Bulk Loading

```javascript
const store = new Store();

// Load large dataset without transaction (faster)
store.bulkLoad(largeData, {
    format: 'application/n-quads',
    lenient: true  // Skip parse errors
});
```

### Loading with Error Handling

```javascript
const store = new Store();

try {
    store.load(data, {
        format: 'text/turtle',
        lenient: false  // Strict parsing
    });
} catch (error) {
    console.error('Parse error:', error);
}

// Lenient mode
store.load(data, {
    format: 'text/turtle',
    lenient: true  // Skip malformed triples
});
```

## Streaming Import

### Rust Streaming

```rust
use oxrdfio::{RdfFormat, RdfParser};
use oxigraph::store::Store;
use std::fs::File;
use std::io::BufReader;

let store = Store::open("data")?;

// Stream large file
let file = File::open("large-data.nt")?;
let parser = RdfParser::from_format(RdfFormat::NTriples)
    .for_reader(BufReader::new(file));

for quad in parser {
    let quad = quad?;
    store.insert(&quad)?;
}
```

### Python Streaming

```python
from pyoxigraph import Store

store = Store("data")

# Parse and insert one by one
for quad in parse(input=open("large-data.nq", "rb"),
                        format=RdfFormat.N_QUADS):
    store.add(quad)
```

## Error Handling Best Practices

### Rust

```rust
use oxigraph::store::LoaderError;

let file_data = std::fs::read_to_string("data.ttl")?;
match store.load_from_reader(RdfFormat::Turtle, file_data.as_bytes()) {
    Ok(_) => println!("Successfully loaded"),
    Err(LoaderError::Parsing(e)) => eprintln!("Parse error: {}", e),
    Err(LoaderError::Storage(e)) => eprintln!("Storage error: {}", e),
    Err(e) => eprintln!("Error: {}", e),
}
```

### Python

```python
from pyoxigraph import Store, ParseError, RdfFormat

try:
    store.load(data, format=RdfFormat.TURTLE)
except ParseError as e:
    print(f"Parse error: {e}")
except OSError as e:
    print(f"I/O error: {e}")
```

## Performance Tips

1. **Use bulk loading** for large datasets (10x-100x faster than individual inserts)
2. **Load files in parallel** when using the CLI
3. **Use N-Quads or N-Triples** for fastest parsing (line-oriented formats)
4. **Enable lenient mode** for dirty data to skip errors
5. **Use non-atomic mode** for maximum speed when safety is not critical
6. **Avoid transactions** for bulk operations (use bulk loader instead)
7. **Compress files** with gzip to reduce I/O time

## Common Issues

### Invalid UTF-8

```bash
# Convert file to UTF-8 first
iconv -f ISO-8859-1 -t UTF-8 data.ttl > data-utf8.ttl
oxigraph load --location store --file data-utf8.ttl
```

### Base IRI Required

Some formats require a base IRI. Provide one explicitly:

```bash
oxigraph load --location store --file data.ttl \
  --base http://example.com/
```

### Memory Issues

For very large files, use the CLI bulk loader instead of the API to minimize memory usage.

## Next Steps

- Learn about [exporting RDF data](export-rdf-data.md)
- Optimize loading with [performance tips](optimize-performance.md)
- Query your data using the [SPARQL server](run-sparql-server.md)
