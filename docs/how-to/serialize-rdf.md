# How to Serialize RDF Data

This guide shows you how to serialize RDF triples and quads to various formats using Oxigraph in Rust, Python, and JavaScript.

## Basic Serialization

### Rust

```rust
use oxrdf::{NamedNodeRef, LiteralRef, QuadRef};
use oxrdfio::{RdfFormat, RdfSerializer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let mut serializer = RdfSerializer::from_format(RdfFormat::Turtle)
        .for_writer(&mut output);

    // Serialize a triple
    serializer.serialize_quad(QuadRef::new(
        NamedNodeRef::new("http://example.com/Alice")?,
        NamedNodeRef::new("http://schema.org/name")?,
        LiteralRef::new_simple_literal("Alice Smith"),
        oxrdf::GraphNameRef::DefaultGraph,
    ))?;

    serializer.finish()?;

    let turtle = String::from_utf8(output)?;
    println!("{}", turtle);

    Ok(())
}
```

### Python

```python
from pyoxigraph import serialize, RdfFormat, Triple, NamedNode, Literal

triple = Triple(
    NamedNode('http://example.com/Alice'),
    NamedNode('http://schema.org/name'),
    Literal('Alice Smith')
)

# Serialize to Turtle
turtle = serialize([triple], format=RdfFormat.TURTLE)
print(turtle.decode('utf-8'))
```

### JavaScript

```javascript
import { serialize, RdfFormat, namedNode, literal, quad } from 'oxigraph';

const q = quad(
  namedNode('http://example.com/Alice'),
  namedNode('http://schema.org/name'),
  literal('Alice Smith')
);

// Serialize to Turtle
const turtle = serialize([q], RdfFormat.TURTLE);
console.log(turtle);
```

## Serializing to Different Formats

### Turtle

**Rust:**
```rust
use oxrdfio::{RdfFormat, RdfSerializer};

let mut output = Vec::new();
let mut serializer = RdfSerializer::from_format(RdfFormat::Turtle)
    .for_writer(&mut output);

// Add quads
for quad in quads {
    serializer.serialize_quad(quad.as_ref())?;
}

serializer.finish()?;
let turtle = String::from_utf8(output)?;
```

**Python:**
```python
from pyoxigraph import serialize, RdfFormat

turtle = serialize(triples, format=RdfFormat.TURTLE)
```

**JavaScript:**
```javascript
import { serialize, RdfFormat } from 'oxigraph';

const turtle = serialize(quads, RdfFormat.TURTLE);
```

### N-Triples

**Rust:**
```rust
use oxrdfio::{RdfFormat, RdfSerializer};

let mut output = Vec::new();
let mut serializer = RdfSerializer::from_format(RdfFormat::NTriples)
    .for_writer(&mut output);

for quad in quads {
    serializer.serialize_quad(quad.as_ref())?;
}

serializer.finish()?;
```

**Python:**
```python
from pyoxigraph import serialize, RdfFormat

ntriples = serialize(triples, format=RdfFormat.N_TRIPLES)
```

**JavaScript:**
```javascript
import { serialize, RdfFormat } from 'oxigraph';

const ntriples = serialize(quads, RdfFormat.N_TRIPLES);
```

### N-Quads (with Named Graphs)

**Rust:**
```rust
use oxrdf::{NamedNodeRef, LiteralRef, QuadRef};
use oxrdfio::{RdfFormat, RdfSerializer};

let mut output = Vec::new();
let mut serializer = RdfSerializer::from_format(RdfFormat::NQuads)
    .for_writer(&mut output);

// Serialize quad with named graph
serializer.serialize_quad(QuadRef::new(
    NamedNodeRef::new("http://example.com/Alice")?,
    NamedNodeRef::new("http://schema.org/name")?,
    LiteralRef::new_simple_literal("Alice"),
    NamedNodeRef::new("http://example.com/graph1")?,
))?;

serializer.finish()?;
```

**Python:**
```python
from pyoxigraph import serialize, RdfFormat, Quad, NamedNode, Literal

quad = Quad(
    NamedNode('http://example.com/Alice'),
    NamedNode('http://schema.org/name'),
    Literal('Alice'),
    NamedNode('http://example.com/graph1')
)

nquads = serialize([quad], format=RdfFormat.N_QUADS)
```

**JavaScript:**
```javascript
import { serialize, RdfFormat, namedNode, literal, quad } from 'oxigraph';

const q = quad(
  namedNode('http://example.com/Alice'),
  namedNode('http://schema.org/name'),
  literal('Alice'),
  namedNode('http://example.com/graph1')
);

const nquads = serialize([q], RdfFormat.N_QUADS);
```

### TriG

**Rust:**
```rust
use oxrdfio::{RdfFormat, RdfSerializer};

let mut output = Vec::new();
let mut serializer = RdfSerializer::from_format(RdfFormat::TriG)
    .for_writer(&mut output);

for quad in quads {
    serializer.serialize_quad(quad.as_ref())?;
}

serializer.finish()?;
```

**Python:**
```python
from pyoxigraph import serialize, RdfFormat

trig = serialize(quads, format=RdfFormat.TRIG)
```

**JavaScript:**
```javascript
import { serialize, RdfFormat } from 'oxigraph';

const trig = serialize(quads, RdfFormat.TRIG);
```

### RDF/XML

**Rust:**
```rust
use oxrdfio::{RdfFormat, RdfSerializer};

let mut output = Vec::new();
let mut serializer = RdfSerializer::from_format(RdfFormat::RdfXml)
    .for_writer(&mut output);

for quad in quads {
    serializer.serialize_quad(quad.as_ref())?;
}

serializer.finish()?;
```

**Python:**
```python
from pyoxigraph import serialize, RdfFormat

rdfxml = serialize(triples, format=RdfFormat.RDF_XML)
```

**JavaScript:**
```javascript
import { serialize, RdfFormat } from 'oxigraph';

const rdfxml = serialize(quads, RdfFormat.RDF_XML);
```

### JSON-LD

**Rust:**
```rust
use oxrdfio::{RdfFormat, RdfSerializer, JsonLdProfileSet};

let mut output = Vec::new();
let format = RdfFormat::JsonLd {
    profile: JsonLdProfileSet::empty(),
};
let mut serializer = RdfSerializer::from_format(format)
    .for_writer(&mut output);

for quad in quads {
    serializer.serialize_quad(quad.as_ref())?;
}

serializer.finish()?;
```

**Python:**
```python
from pyoxigraph import serialize, RdfFormat

jsonld = serialize(triples, format=RdfFormat.JSON_LD)
```

**JavaScript:**
```javascript
import { serialize, RdfFormat } from 'oxigraph';

const jsonld = serialize(quads, RdfFormat.JSON_LD);
```

## Serializing to Files

### Rust

```rust
use oxrdfio::{RdfFormat, RdfSerializer};
use std::fs::File;
use std::io::BufWriter;

fn serialize_to_file() -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create("output.ttl")?;
    let mut serializer = RdfSerializer::from_format(RdfFormat::Turtle)
        .for_writer(BufWriter::new(file));

    // Add quads
    for quad in quads {
        serializer.serialize_quad(quad.as_ref())?;
    }

    serializer.finish()?;
    Ok(())
}
```

### Python

```python
from pyoxigraph import serialize, RdfFormat

# Serialize to file path
serialize(triples, output="output.ttl", format=RdfFormat.TURTLE)

# Or serialize to file object
with open("output.ttl", "wb") as f:
    serialize(triples, output=f, format=RdfFormat.TURTLE)

# Or get bytes and write manually
turtle_bytes = serialize(triples, format=RdfFormat.TURTLE)
with open("output.ttl", "wb") as f:
    f.write(turtle_bytes)
```

### JavaScript (Node.js)

```javascript
import { serialize, RdfFormat } from 'oxigraph';
import { writeFileSync } from 'fs';

const turtle = serialize(quads, RdfFormat.TURTLE);
writeFileSync('output.ttl', turtle);
```

## Using Prefixes

Prefixes make serialized output more readable by abbreviating URIs.

### Rust

```rust
use oxrdfio::{RdfFormat, RdfSerializer};

let mut output = Vec::new();
let mut serializer = RdfSerializer::from_format(RdfFormat::Turtle)
    .with_prefix("schema", "http://schema.org/")?
    .with_prefix("ex", "http://example.com/")?
    .for_writer(&mut output);

// Now URIs like http://schema.org/name will be serialized as schema:name
for quad in quads {
    serializer.serialize_quad(quad.as_ref())?;
}

serializer.finish()?;
```

### Python

```python
from pyoxigraph import serialize, RdfFormat, Triple, NamedNode, Literal

triple = Triple(
    NamedNode('http://example.com/Alice'),
    NamedNode('http://schema.org/name'),
    Literal('Alice')
)

turtle = serialize(
    [triple],
    format=RdfFormat.TURTLE,
    prefixes={
        'schema': 'http://schema.org/',
        'ex': 'http://example.com/'
    }
)

print(turtle.decode('utf-8'))
# Output:
# @prefix ex: <http://example.com/> .
# @prefix schema: <http://schema.org/> .
#
# ex:Alice schema:name "Alice" .
```

### JavaScript

```javascript
import { serialize, RdfFormat, namedNode, literal, quad } from 'oxigraph';

const q = quad(
  namedNode('http://example.com/Alice'),
  namedNode('http://schema.org/name'),
  literal('Alice')
);

const turtle = serialize([q], RdfFormat.TURTLE, {
  prefixes: {
    'schema': 'http://schema.org/',
    'ex': 'http://example.com/'
  }
});

console.log(turtle);
// Output:
// @prefix ex: <http://example.com/> .
// @prefix schema: <http://schema.org/> .
//
// ex:Alice schema:name "Alice" .
```

## Using Base IRI

A base IRI allows relative IRIs in the serialized output.

### Rust

```rust
use oxrdfio::{RdfFormat, RdfSerializer};

let mut output = Vec::new();
let mut serializer = RdfSerializer::from_format(RdfFormat::Turtle)
    .with_base_iri("http://example.com/")?
    .with_prefix("schema", "http://schema.org/")?
    .for_writer(&mut output);

// URIs like http://example.com/Alice can be serialized as <Alice>
for quad in quads {
    serializer.serialize_quad(quad.as_ref())?;
}

serializer.finish()?;
```

### Python

```python
from pyoxigraph import serialize, RdfFormat, Triple, NamedNode, Literal

triple = Triple(
    NamedNode('http://example.com/Alice'),
    NamedNode('http://schema.org/name'),
    Literal('Alice')
)

turtle = serialize(
    [triple],
    format=RdfFormat.TURTLE,
    base_iri='http://example.com/',
    prefixes={'schema': 'http://schema.org/'}
)

print(turtle.decode('utf-8'))
# Output:
# @base <http://example.com/> .
# @prefix schema: <http://schema.org/> .
#
# <Alice> schema:name "Alice" .
```

### JavaScript

```javascript
import { serialize, RdfFormat, namedNode, literal, quad } from 'oxigraph';

const q = quad(
  namedNode('http://example.com/Alice'),
  namedNode('http://schema.org/name'),
  literal('Alice')
);

const turtle = serialize([q], RdfFormat.TURTLE, {
  baseIri: 'http://example.com/',
  prefixes: { 'schema': 'http://schema.org/' }
});

console.log(turtle);
// Output:
// @base <http://example.com/> .
// @prefix schema: <http://schema.org/> .
//
// <Alice> schema:name "Alice" .
```

## Format Conversion

Convert between different RDF formats by parsing one format and serializing to another.

### Rust

```rust
use oxrdfio::{RdfFormat, RdfParser, RdfSerializer};

fn convert_format() -> Result<(), Box<dyn std::error::Error>> {
    let turtle_data = r#"
        @prefix ex: <http://example.com/> .
        ex:Alice ex:knows ex:Bob .
    "#;

    // Parse Turtle
    let parser = RdfParser::from_format(RdfFormat::Turtle);
    let quads: Vec<_> = parser
        .for_reader(turtle_data.as_bytes())
        .collect::<Result<_, _>>()?;

    // Serialize to N-Triples
    let mut output = Vec::new();
    let mut serializer = RdfSerializer::from_format(RdfFormat::NTriples)
        .for_writer(&mut output);

    for quad in &quads {
        serializer.serialize_quad(quad.as_ref())?;
    }

    serializer.finish()?;

    let ntriples = String::from_utf8(output)?;
    println!("{}", ntriples);

    Ok(())
}
```

### Python

```python
from pyoxigraph import parse, serialize, RdfFormat

turtle_data = """
@prefix ex: <http://example.com/> .
ex:Alice ex:knows ex:Bob .
"""

# Parse Turtle
quads = list(parse(input=turtle_data, format=RdfFormat.TURTLE))

# Serialize to N-Triples
ntriples = serialize(quads, format=RdfFormat.N_TRIPLES)
print(ntriples.decode('utf-8'))

# Or convert to JSON-LD
jsonld = serialize(quads, format=RdfFormat.JSON_LD)
print(jsonld.decode('utf-8'))
```

### JavaScript

```javascript
import { parse, serialize, RdfFormat } from 'oxigraph';

const turtleData = `
@prefix ex: <http://example.com/> .
ex:Alice ex:knows ex:Bob .
`;

// Parse Turtle
const quads = parse(turtleData, RdfFormat.TURTLE);

// Serialize to N-Triples
const ntriples = serialize(quads, RdfFormat.N_TRIPLES);
console.log(ntriples);

// Or convert to JSON-LD
const jsonld = serialize(quads, RdfFormat.JSON_LD);
console.log(jsonld);
```

## Preserving Prefixes During Conversion

Extract prefixes from input and use them in output.

### Python

```python
from pyoxigraph import parse, serialize, RdfFormat

turtle_data = """
@prefix schema: <http://schema.org/> .
@prefix ex: <http://example.com/> .

ex:Alice a schema:Person ;
    schema:name "Alice" .
"""

# Parse and extract prefixes
parser = parse(input=turtle_data, format=RdfFormat.TURTLE)
prefixes = parser.prefixes
quads = list(parser)

# Serialize with the same prefixes
output = serialize(quads, format=RdfFormat.TURTLE, prefixes=prefixes)
print(output.decode('utf-8'))
```

### JavaScript

```javascript
import { parseWithMetadata, serialize, RdfFormat } from 'oxigraph';

const turtleData = `
@prefix schema: <http://schema.org/> .
@prefix ex: <http://example.com/> .

ex:Alice a schema:Person ;
    schema:name "Alice" .
`;

// Parse and extract prefixes
const result = parseWithMetadata(turtleData, RdfFormat.TURTLE);

// Serialize with the same prefixes
const output = serialize(result.quads, RdfFormat.TURTLE, {
  prefixes: result.prefixes
});
console.log(output);
```

## Serializing from Store

Serialize RDF data directly from an Oxigraph store.

### Rust

```rust
use oxigraph::store::Store;
use oxrdfio::{RdfFormat, RdfSerializer};

fn serialize_store() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    // Add data to store
    // ...

    // Serialize all quads from store
    let mut output = Vec::new();
    let mut serializer = RdfSerializer::from_format(RdfFormat::Turtle)
        .for_writer(&mut output);

    for quad in store.iter() {
        let quad = quad?;
        serializer.serialize_quad(quad.as_ref())?;
    }

    serializer.finish()?;

    let turtle = String::from_utf8(output)?;
    Ok(())
}
```

### Python

```python
from pyoxigraph import Store, serialize, RdfFormat

store = Store()

# Add data to store
# ...

# Serialize all quads from store
quads = list(store)
turtle = serialize(quads, format=RdfFormat.TURTLE)
```

### JavaScript

```javascript
import { Store, serialize, RdfFormat } from 'oxigraph';

const store = new Store();

// Add data to store
// ...

// Serialize all quads from store
const quads = Array.from(store);
const turtle = serialize(quads, RdfFormat.TURTLE);
```

## Streaming Serialization

For large datasets, serialize data incrementally.

### Rust

```rust
use oxrdfio::{RdfFormat, RdfSerializer};
use std::fs::File;
use std::io::BufWriter;

fn stream_serialize() -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create("large_output.nt")?;
    let mut serializer = RdfSerializer::from_format(RdfFormat::NTriples)
        .for_writer(BufWriter::new(file));

    // Stream quads without loading all in memory
    for i in 0..1_000_000 {
        // Generate or fetch quad
        let quad = generate_quad(i)?;
        serializer.serialize_quad(quad.as_ref())?;

        if i % 100_000 == 0 {
            println!("Serialized {} quads", i);
        }
    }

    serializer.finish()?;
    Ok(())
}
```

### Python

```python
from pyoxigraph import serialize, RdfFormat, Quad, NamedNode, Literal

def generate_quads():
    """Generator that yields quads one at a time"""
    for i in range(1_000_000):
        yield Quad(
            NamedNode(f'http://example.com/subject{i}'),
            NamedNode('http://example.com/predicate'),
            Literal(str(i))
        )

# Serialize generator output to file
serialize(generate_quads(), output="large_output.nt", format=RdfFormat.N_TRIPLES)
```

### JavaScript (Async)

```javascript
import { serializeAsync, RdfFormat, namedNode, literal, quad } from 'oxigraph';

function* generateQuads() {
  for (let i = 0; i < 1000000; i++) {
    yield quad(
      namedNode(`http://example.com/subject${i}`),
      namedNode('http://example.com/predicate'),
      literal(String(i))
    );
  }
}

// Serialize asynchronously to avoid blocking
const result = await serializeAsync(generateQuads(), RdfFormat.N_TRIPLES);
```

## Pretty Printing

Some formats support pretty printing options.

### Turtle Pretty Printing

Turtle automatically uses pretty printing with prefixes and multi-line objects:

**Python:**
```python
from pyoxigraph import serialize, RdfFormat, Triple, NamedNode, Literal

triples = [
    Triple(
        NamedNode('http://example.com/Alice'),
        NamedNode('http://schema.org/name'),
        Literal('Alice')
    ),
    Triple(
        NamedNode('http://example.com/Alice'),
        NamedNode('http://schema.org/age'),
        Literal('30', datatype=NamedNode('http://www.w3.org/2001/XMLSchema#integer'))
    ),
    Triple(
        NamedNode('http://example.com/Alice'),
        NamedNode('http://schema.org/knows'),
        NamedNode('http://example.com/Bob')
    ),
]

turtle = serialize(
    triples,
    format=RdfFormat.TURTLE,
    prefixes={'schema': 'http://schema.org/', 'ex': 'http://example.com/'}
)

print(turtle.decode('utf-8'))
# Output (pretty-printed):
# @prefix ex: <http://example.com/> .
# @prefix schema: <http://schema.org/> .
#
# ex:Alice schema:age 30 ;
#     schema:knows ex:Bob ;
#     schema:name "Alice" .
```

## Error Handling

### Rust

```rust
use oxrdfio::{RdfFormat, RdfSerializer};

fn serialize_with_errors() -> Result<(), Box<dyn std::error::Error>> {
    let mut output = Vec::new();
    let mut serializer = RdfSerializer::from_format(RdfFormat::Turtle)
        .for_writer(&mut output);

    for quad in quads {
        // Check if format supports datasets
        if !quad.graph_name.is_default_graph() && !RdfFormat::Turtle.supports_datasets() {
            return Err("Turtle format doesn't support named graphs".into());
        }

        serializer.serialize_quad(quad.as_ref())?;
    }

    serializer.finish()?;
    Ok(())
}
```

### Python

```python
from pyoxigraph import serialize, RdfFormat, Quad, NamedNode, Literal

quad_with_graph = Quad(
    NamedNode('http://example.com/Alice'),
    NamedNode('http://schema.org/name'),
    Literal('Alice'),
    NamedNode('http://example.com/graph1')  # Named graph
)

try:
    # This will fail because Turtle doesn't support named graphs
    turtle = serialize([quad_with_graph], format=RdfFormat.TURTLE)
except ValueError as e:
    print(f"Error: {e}")
    # Use TriG instead for quads with named graphs
    trig = serialize([quad_with_graph], format=RdfFormat.TRIG)
```

### JavaScript

```javascript
import { serialize, RdfFormat, namedNode, literal, quad } from 'oxigraph';

const quadWithGraph = quad(
  namedNode('http://example.com/Alice'),
  namedNode('http://schema.org/name'),
  literal('Alice'),
  namedNode('http://example.com/graph1')
);

try {
  // This will fail because Turtle doesn't support named graphs
  const turtle = serialize([quadWithGraph], RdfFormat.TURTLE);
} catch (error) {
  console.error('Error:', error.message);
  // Use TriG instead for quads with named graphs
  const trig = serialize([quadWithGraph], RdfFormat.TRIG);
  console.log(trig);
}
```

## Complete Example: Round-Trip Conversion

Parse, modify, and serialize RDF data.

### Python

```python
from pyoxigraph import parse, serialize, RdfFormat, NamedNode, Literal, Quad

# Parse input data
input_data = """
@prefix schema: <http://schema.org/> .
@prefix ex: <http://example.com/> .

ex:Alice schema:name "Alice" ;
    schema:age 30 .
"""

quads = list(parse(input=input_data, format=RdfFormat.TURTLE))

# Modify data: add new triple
new_quad = Quad(
    NamedNode('http://example.com/Alice'),
    NamedNode('http://schema.org/email'),
    Literal('alice@example.com')
)
quads.append(new_quad)

# Serialize back to Turtle
output = serialize(
    quads,
    format=RdfFormat.TURTLE,
    prefixes={
        'schema': 'http://schema.org/',
        'ex': 'http://example.com/'
    }
)

print(output.decode('utf-8'))
```

### JavaScript

```javascript
import { parse, serialize, RdfFormat, namedNode, literal, quad } from 'oxigraph';

// Parse input data
const inputData = `
@prefix schema: <http://schema.org/> .
@prefix ex: <http://example.com/> .

ex:Alice schema:name "Alice" ;
    schema:age 30 .
`;

const quads = parse(inputData, RdfFormat.TURTLE);

// Modify data: add new quad
const newQuad = quad(
  namedNode('http://example.com/Alice'),
  namedNode('http://schema.org/email'),
  literal('alice@example.com')
);
quads.push(newQuad);

// Serialize back to Turtle
const output = serialize(quads, RdfFormat.TURTLE, {
  prefixes: {
    'schema': 'http://schema.org/',
    'ex': 'http://example.com/'
  }
});

console.log(output);
```

## Next Steps

- Learn how to [parse RDF files](parse-rdf-files.md)
- See the [format comparison table](../reference/format-comparison.md)
- Read the [RDF formats introduction](../tutorials/rdf-formats-intro.md)
