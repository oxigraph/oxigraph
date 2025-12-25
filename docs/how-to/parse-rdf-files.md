# How to Parse RDF Files

This guide shows you how to parse RDF data in various formats using Oxigraph in Rust, Python, and JavaScript.

## Basic Parsing

### Rust

```rust
use oxrdfio::{RdfFormat, RdfParser};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let turtle_data = r#"
        @prefix schema: <http://schema.org/> .
        @prefix ex: <http://example.com/> .

        ex:Alice a schema:Person ;
            schema:name "Alice Smith" ;
            schema:age 30 .
    "#;

    // Create a parser for Turtle format
    let parser = RdfParser::from_format(RdfFormat::Turtle);

    // Parse the data
    for quad in parser.for_reader(turtle_data.as_bytes()) {
        let quad = quad?;
        println!("Subject: {}", quad.subject);
        println!("Predicate: {}", quad.predicate);
        println!("Object: {}", quad.object);
        println!("---");
    }

    Ok(())
}
```

### Python

```python
from pyoxigraph import parse, RdfFormat

turtle_data = """
@prefix schema: <http://schema.org/> .
@prefix ex: <http://example.com/> .

ex:Alice a schema:Person ;
    schema:name "Alice Smith" ;
    schema:age 30 .
"""

# Parse Turtle data
for quad in parse(input=turtle_data, format=RdfFormat.TURTLE):
    print(f"Subject: {quad.subject}")
    print(f"Predicate: {quad.predicate}")
    print(f"Object: {quad.object}")
    print("---")
```

### JavaScript

```javascript
import { parse, RdfFormat } from 'oxigraph';

const turtleData = `
  @prefix schema: <http://schema.org/> .
  @prefix ex: <http://example.com/> .

  ex:Alice a schema:Person ;
      schema:name "Alice Smith" ;
      schema:age 30 .
`;

// Parse Turtle data
const quads = parse(turtleData, RdfFormat.TURTLE);

for (const quad of quads) {
  console.log(`Subject: ${quad.subject.value}`);
  console.log(`Predicate: ${quad.predicate.value}`);
  console.log(`Object: ${quad.object.value}`);
  console.log('---');
}
```

## Parsing Different Formats

### Turtle

Turtle is the most human-readable format, great for hand-written RDF data.

**Rust:**
```rust
use oxrdfio::{RdfFormat, RdfParser};

let parser = RdfParser::from_format(RdfFormat::Turtle);
for quad in parser.for_reader(data.as_bytes()) {
    let quad = quad?;
    // Process quad
}
```

**Python:**
```python
from pyoxigraph import parse, RdfFormat

for quad in parse(input=data, format=RdfFormat.TURTLE):
    # Process quad
    pass
```

**JavaScript:**
```javascript
import { parse, RdfFormat } from 'oxigraph';

const quads = parse(data, RdfFormat.TURTLE);
for (const quad of quads) {
    // Process quad
}
```

### N-Triples

N-Triples is a line-oriented format ideal for streaming large files.

**Rust:**
```rust
use oxrdfio::{RdfFormat, RdfParser};

let parser = RdfParser::from_format(RdfFormat::NTriples);
for quad in parser.for_reader(data.as_bytes()) {
    let quad = quad?;
    // Process quad
}
```

**Python:**
```python
from pyoxigraph import parse, RdfFormat

for quad in parse(input=data, format=RdfFormat.N_TRIPLES):
    # Process quad
    pass
```

**JavaScript:**
```javascript
import { parse, RdfFormat } from 'oxigraph';

const quads = parse(data, RdfFormat.N_TRIPLES);
```

### N-Quads (with Named Graphs)

N-Quads extends N-Triples to support named graphs.

**Rust:**
```rust
use oxrdfio::{RdfFormat, RdfParser};

let nquads_data = r#"
<http://example.com/Alice> <http://schema.org/name> "Alice" <http://example.com/graph1> .
<http://example.com/Bob> <http://schema.org/name> "Bob" <http://example.com/graph2> .
"#;

let parser = RdfParser::from_format(RdfFormat::NQuads);
for quad in parser.for_reader(nquads_data.as_bytes()) {
    let quad = quad?;
    println!("Graph: {}", quad.graph_name);
    println!("Subject: {}", quad.subject);
}
```

**Python:**
```python
from pyoxigraph import parse, RdfFormat

nquads_data = """
<http://example.com/Alice> <http://schema.org/name> "Alice" <http://example.com/graph1> .
<http://example.com/Bob> <http://schema.org/name> "Bob" <http://example.com/graph2> .
"""

for quad in parse(input=nquads_data, format=RdfFormat.N_QUADS):
    print(f"Graph: {quad.graph_name}")
    print(f"Subject: {quad.subject}")
```

**JavaScript:**
```javascript
import { parse, RdfFormat } from 'oxigraph';

const nquadsData = `
<http://example.com/Alice> <http://schema.org/name> "Alice" <http://example.com/graph1> .
<http://example.com/Bob> <http://schema.org/name> "Bob" <http://example.com/graph2> .
`;

const quads = parse(nquadsData, RdfFormat.N_QUADS);
for (const quad of quads) {
  console.log(`Graph: ${quad.graphName.value}`);
  console.log(`Subject: ${quad.subject.value}`);
}
```

### TriG (Turtle with Named Graphs)

TriG provides readable syntax for datasets with named graphs.

**Rust:**
```rust
use oxrdfio::{RdfFormat, RdfParser};

let trig_data = r#"
@prefix ex: <http://example.com/> .
@prefix schema: <http://schema.org/> .

ex:graph1 {
    ex:Alice schema:name "Alice" .
}

ex:graph2 {
    ex:Bob schema:name "Bob" .
}
"#;

let parser = RdfParser::from_format(RdfFormat::TriG);
for quad in parser.for_reader(trig_data.as_bytes()) {
    let quad = quad?;
    // Process quad
}
```

**Python:**
```python
from pyoxigraph import parse, RdfFormat

trig_data = """
@prefix ex: <http://example.com/> .
@prefix schema: <http://schema.org/> .

ex:graph1 {
    ex:Alice schema:name "Alice" .
}

ex:graph2 {
    ex:Bob schema:name "Bob" .
}
"""

for quad in parse(input=trig_data, format=RdfFormat.TRIG):
    # Process quad
    pass
```

**JavaScript:**
```javascript
import { parse, RdfFormat } from 'oxigraph';

const trigData = `
@prefix ex: <http://example.com/> .
@prefix schema: <http://schema.org/> .

ex:graph1 {
    ex:Alice schema:name "Alice" .
}
`;

const quads = parse(trigData, RdfFormat.TRIG);
```

### RDF/XML

RDF/XML is useful for legacy systems and XML-based workflows.

**Rust:**
```rust
use oxrdfio::{RdfFormat, RdfParser};

let rdfxml_data = r#"<?xml version="1.0"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
         xmlns:schema="http://schema.org/">
  <schema:Person rdf:about="http://example.com/Alice">
    <schema:name>Alice Smith</schema:name>
  </schema:Person>
</rdf:RDF>"#;

let parser = RdfParser::from_format(RdfFormat::RdfXml);
for quad in parser.for_reader(rdfxml_data.as_bytes()) {
    let quad = quad?;
    // Process quad
}
```

**Python:**
```python
from pyoxigraph import parse, RdfFormat

rdfxml_data = """<?xml version="1.0"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
         xmlns:schema="http://schema.org/">
  <schema:Person rdf:about="http://example.com/Alice">
    <schema:name>Alice Smith</schema:name>
  </schema:Person>
</rdf:RDF>"""

for quad in parse(input=rdfxml_data, format=RdfFormat.RDF_XML):
    # Process quad
    pass
```

**JavaScript:**
```javascript
import { parse, RdfFormat } from 'oxigraph';

const rdfxmlData = `<?xml version="1.0"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
         xmlns:schema="http://schema.org/">
  <schema:Person rdf:about="http://example.com/Alice">
    <schema:name>Alice Smith</schema:name>
  </schema:Person>
</rdf:RDF>`;

const quads = parse(rdfxmlData, RdfFormat.RDF_XML);
```

### JSON-LD

JSON-LD is ideal for web APIs and JavaScript applications.

**Rust:**
```rust
use oxrdfio::{RdfFormat, RdfParser};
use oxrdfio::JsonLdProfileSet;

let jsonld_data = r#"{
  "@context": {
    "schema": "http://schema.org/",
    "name": "schema:name",
    "age": "schema:age"
  },
  "@id": "http://example.com/Alice",
  "@type": "schema:Person",
  "name": "Alice Smith",
  "age": 30
}"#;

let format = RdfFormat::JsonLd {
    profile: JsonLdProfileSet::empty(),
};
let parser = RdfParser::from_format(format);
for quad in parser.for_reader(jsonld_data.as_bytes()) {
    let quad = quad?;
    // Process quad
}
```

**Python:**
```python
from pyoxigraph import parse, RdfFormat

jsonld_data = """{
  "@context": {
    "schema": "http://schema.org/",
    "name": "schema:name",
    "age": "schema:age"
  },
  "@id": "http://example.com/Alice",
  "@type": "schema:Person",
  "name": "Alice Smith",
  "age": 30
}"""

for quad in parse(input=jsonld_data, format=RdfFormat.JSON_LD):
    # Process quad
    pass
```

**JavaScript:**
```javascript
import { parse, RdfFormat } from 'oxigraph';

const jsonldData = `{
  "@context": {
    "schema": "http://schema.org/",
    "name": "schema:name",
    "age": "schema:age"
  },
  "@id": "http://example.com/Alice",
  "@type": "schema:Person",
  "name": "Alice Smith",
  "age": 30
}`;

const quads = parse(jsonldData, RdfFormat.JSON_LD);
```

## Parsing Files

### Rust

```rust
use oxrdfio::{RdfFormat, RdfParser};
use std::fs::File;

fn parse_file() -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open("data.ttl")?;
    let parser = RdfParser::from_format(RdfFormat::Turtle);

    for quad in parser.for_reader(file) {
        let quad = quad?;
        // Process quad
    }

    Ok(())
}
```

### Python

```python
from pyoxigraph import parse, RdfFormat

# Parse from file path
for quad in parse(path="data.ttl", format=RdfFormat.TURTLE):
    # Process quad
    pass

# Or parse from file object
with open("data.ttl", "rb") as f:
    for quad in parse(input=f, format=RdfFormat.TURTLE):
        # Process quad
        pass
```

### JavaScript (Node.js)

```javascript
import { parse, RdfFormat } from 'oxigraph';
import { readFileSync } from 'fs';

const data = readFileSync('data.ttl', 'utf-8');
const quads = parse(data, RdfFormat.TURTLE);
```

## Parsing with Base IRI

Use a base IRI to resolve relative IRIs in the data.

### Rust

```rust
use oxrdfio::{RdfFormat, RdfParser};

let data = r#"
@prefix ex: <> .
<Alice> a ex:Person .
"#;

let parser = RdfParser::from_format(RdfFormat::Turtle)
    .with_base_iri("http://example.com/")?;

for quad in parser.for_reader(data.as_bytes()) {
    let quad = quad?;
    // <Alice> expands to <http://example.com/Alice>
}
```

### Python

```python
from pyoxigraph import parse, RdfFormat

data = """
@prefix ex: <> .
<Alice> a ex:Person .
"""

for quad in parse(input=data, format=RdfFormat.TURTLE,
                  base_iri="http://example.com/"):
    # <Alice> expands to <http://example.com/Alice>
    pass
```

### JavaScript

```javascript
import { parse, RdfFormat } from 'oxigraph';

const data = `
@prefix ex: <> .
<Alice> a ex:Person .
`;

const quads = parse(data, RdfFormat.TURTLE, {
  baseIri: 'http://example.com/'
});
// <Alice> expands to <http://example.com/Alice>
```

## Extracting Prefixes and Base IRI

After parsing, you can access the prefixes and base IRI discovered in the document.

### Python

```python
from pyoxigraph import parse, RdfFormat

data = """
@prefix schema: <http://schema.org/> .
@prefix ex: <http://example.com/> .
@base <http://example.com/> .

ex:Alice a schema:Person .
"""

parser = parse(input=data, format=RdfFormat.TURTLE)

# Access prefixes
print(parser.prefixes)  # {'schema': 'http://schema.org/', 'ex': 'http://example.com/'}

# Access base IRI
print(parser.base_iri)  # 'http://example.com/'

# Iterate through quads
for quad in parser:
    print(quad)
```

### JavaScript

```javascript
import { parseWithMetadata, RdfFormat } from 'oxigraph';

const data = `
@prefix schema: <http://schema.org/> .
@prefix ex: <http://example.com/> .
@base <http://example.com/> .

ex:Alice a schema:Person .
`;

const result = parseWithMetadata(data, RdfFormat.TURTLE);

console.log(result.prefixes);  // { schema: 'http://schema.org/', ex: 'http://example.com/' }
console.log(result.baseIri);   // 'http://example.com/'
console.log(result.quads);     // Array of quads
```

## Parsing Options

### Renaming Blank Nodes

Rename blank nodes to avoid conflicts when merging data from multiple sources.

**Rust:**
```rust
let parser = RdfParser::from_format(RdfFormat::Turtle)
    .rename_blank_nodes();
```

**Python:**
```python
for quad in parse(input=data, format=RdfFormat.TURTLE, rename_blank_nodes=True):
    # Blank node IDs are randomized
    pass
```

**JavaScript:**
```javascript
const quads = parse(data, RdfFormat.TURTLE, {
  renameBlankNodes: true
});
```

### Rejecting Named Graphs

Fail when parsing named graphs if you expect only triples.

**Rust:**
```rust
let parser = RdfParser::from_format(RdfFormat::NQuads)
    .without_named_graphs();
```

**Python:**
```python
for quad in parse(input=data, format=RdfFormat.N_QUADS,
                  without_named_graphs=True):
    # Fails if data contains named graphs
    pass
```

**JavaScript:**
```javascript
const quads = parse(data, RdfFormat.N_QUADS, {
  withoutNamedGraphs: true
});
```

### Lenient Parsing

Skip some validation for faster parsing (use with caution).

**Rust:**
```rust
let parser = RdfParser::from_format(RdfFormat::Turtle)
    .lenient();
```

**Python:**
```python
for quad in parse(input=data, format=RdfFormat.TURTLE, lenient=True):
    # Faster parsing with less validation
    pass
```

**JavaScript:**
```javascript
const quads = parse(data, RdfFormat.TURTLE, {
  lenient: true
});
```

## Error Handling

### Rust

```rust
use oxrdfio::{RdfFormat, RdfParser, RdfParseError};

fn parse_with_errors() {
    let invalid_data = "@prefix ex: <invalid iri> .";
    let parser = RdfParser::from_format(RdfFormat::Turtle);

    for result in parser.for_reader(invalid_data.as_bytes()) {
        match result {
            Ok(quad) => println!("Parsed: {:?}", quad),
            Err(e) => {
                eprintln!("Parse error: {}", e);
                if let RdfParseError::Syntax(err) = e {
                    if let Some(location) = err.location() {
                        eprintln!("At line {}, column {}",
                                location.start.line,
                                location.start.column);
                    }
                }
            }
        }
    }
}
```

### Python

```python
from pyoxigraph import parse, RdfFormat

invalid_data = "@prefix ex: <invalid iri> ."

try:
    for quad in parse(input=invalid_data, format=RdfFormat.TURTLE):
        print(f"Parsed: {quad}")
except SyntaxError as e:
    print(f"Parse error: {e}")
    # Python SyntaxError includes line and column information
except Exception as e:
    print(f"Error: {e}")
```

### JavaScript

```javascript
import { parse, RdfFormat } from 'oxigraph';

const invalidData = "@prefix ex: <invalid iri> .";

try {
  const quads = parse(invalidData, RdfFormat.TURTLE);
  console.log('Parsed:', quads);
} catch (error) {
  console.error('Parse error:', error.message);
}
```

## Streaming Large Files

For large files, process quads incrementally to reduce memory usage.

### Rust

```rust
use oxrdfio::{RdfFormat, RdfParser};
use std::fs::File;

fn stream_large_file() -> Result<(), Box<dyn std::error::Error>> {
    let file = File::open("large_dataset.nt")?;
    let parser = RdfParser::from_format(RdfFormat::NTriples);

    let mut count = 0;
    for quad in parser.for_reader(file) {
        let quad = quad?;
        count += 1;

        // Process quad immediately without storing all in memory
        if count % 10000 == 0 {
            println!("Processed {} quads", count);
        }
    }

    println!("Total: {} quads", count);
    Ok(())
}
```

### Python

```python
from pyoxigraph import parse, RdfFormat

count = 0
for quad in parse(path="large_dataset.nt", format=RdfFormat.N_TRIPLES):
    count += 1
    # Process quad immediately without storing all in memory
    if count % 10000 == 0:
        print(f"Processed {count} quads")

print(f"Total: {count} quads")
```

### JavaScript (Async)

```javascript
import { parseAsync, RdfFormat } from 'oxigraph';
import { readFileSync } from 'fs';

const data = readFileSync('large_dataset.nt', 'utf-8');

// Use parseAsync for large datasets to avoid blocking
const quads = await parseAsync(data, RdfFormat.N_TRIPLES);
console.log(`Parsed ${quads.length} quads`);
```

## Auto-detecting Format

You can detect the format from file extension or media type.

### Rust

```rust
use oxrdfio::RdfFormat;

// From file extension
if let Some(format) = RdfFormat::from_extension("ttl") {
    println!("Format: {}", format.name());
}

// From media type
if let Some(format) = RdfFormat::from_media_type("text/turtle") {
    println!("Format: {}", format.name());
}
```

### Python

```python
from pyoxigraph import RdfFormat

# From file extension
format = RdfFormat.from_extension("ttl")
if format:
    print(f"Format: {format.name}")

# From media type
format = RdfFormat.from_media_type("text/turtle")
if format:
    print(f"Format: {format.name}")

# Auto-detect from file path
from pyoxigraph import parse

for quad in parse(path="data.ttl"):  # Format auto-detected from .ttl extension
    pass
```

### JavaScript

```javascript
import { RdfFormat } from 'oxigraph';

// From file extension
const format = RdfFormat.fromExtension('ttl');
if (format) {
  console.log(`Format: ${format.name}`);
}

// From media type
const format2 = RdfFormat.fromMediaType('text/turtle');
if (format2) {
  console.log(`Format: ${format2.name}`);
}
```

## Next Steps

- Learn how to [serialize RDF data](serialize-rdf.md)
- See the [format comparison table](../reference/format-comparison.md)
- Read the [RDF formats introduction](../tutorials/rdf-formats-intro.md)
