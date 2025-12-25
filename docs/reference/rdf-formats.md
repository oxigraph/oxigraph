# RDF Formats Reference

This document details all RDF serialization formats supported by Oxigraph.

## Supported Formats Overview

Oxigraph supports 7 RDF serialization formats through the `oxrdfio` crate:

| Format | Datasets | Graphs | W3C Spec | Status |
|--------|----------|--------|----------|--------|
| Turtle | No | Yes | [TR](https://www.w3.org/TR/turtle/) | Full |
| TriG | Yes | Yes | [TR](https://www.w3.org/TR/trig/) | Full |
| N-Triples | No | Yes | [TR](https://www.w3.org/TR/n-triples/) | Full |
| N-Quads | Yes | Yes | [TR](https://www.w3.org/TR/n-quads/) | Full |
| RDF/XML | No | Yes | [TR](https://www.w3.org/TR/rdf-syntax-grammar/) | Full |
| JSON-LD | Yes | Yes | [TR](https://www.w3.org/TR/json-ld/) | Partial (1.0 only) |
| N3 | Yes | Yes | [Spec](https://w3c.github.io/N3/spec/) | Full |

## Format Details

### Turtle (text/turtle)

**File Extension**: `.ttl`

**MIME Type**: `text/turtle`

**Format IRI**: `http://www.w3.org/ns/formats/Turtle`

**Description**: A compact, human-readable RDF format based on the Terse RDF Triple Language. Supports prefixes and abbreviated syntax for common patterns.

**Graph Support**: Triples only (no named graphs)

**Dataset Support**: No

**RDF-star**: Yes (with `rdf-12` feature)

**Example**:
```turtle
@prefix ex: <http://example.org/> .
@prefix schema: <http://schema.org/> .

ex:Alice a schema:Person ;
    schema:name "Alice Smith" ;
    schema:age 30 .
```

**Parser**: `oxttl::TurtleParser`

**Serializer**: `oxttl::TurtleSerializer`

---

### TriG (application/trig)

**File Extension**: `.trig`

**MIME Type**: `application/trig`

**Format IRI**: `http://www.w3.org/ns/formats/TriG`

**Description**: Extension of Turtle for RDF datasets. Allows grouping triples into named graphs.

**Graph Support**: Yes

**Dataset Support**: Yes

**RDF-star**: Yes (with `rdf-12` feature)

**Example**:
```trig
@prefix ex: <http://example.org/> .

ex:graph1 {
    ex:Alice ex:knows ex:Bob .
}

ex:graph2 {
    ex:Bob ex:knows ex:Charlie .
}
```

**Parser**: `oxttl::TriGParser`

**Serializer**: `oxttl::TriGSerializer`

---

### N-Triples (application/n-triples)

**File Extension**: `.nt`

**MIME Type**: `application/n-triples`

**Format IRI**: `http://www.w3.org/ns/formats/N-Triples`

**Description**: Line-based, plain text format for RDF triples. One triple per line with no abbreviations or prefixes.

**Graph Support**: Triples only (no named graphs)

**Dataset Support**: No

**RDF-star**: Yes (with `rdf-12` feature)

**Example**:
```ntriples
<http://example.org/Alice> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
<http://example.org/Alice> <http://schema.org/name> "Alice Smith" .
<http://example.org/Alice> <http://schema.org/age> "30"^^<http://www.w3.org/2001/XMLSchema#integer> .
```

**Parser**: `oxttl::NTriplesParser`

**Serializer**: `oxttl::NTriplesSerializer`

**Use Cases**:
- Large file streaming
- Simple parsing requirements
- Log files
- Testing and debugging

---

### N-Quads (application/n-quads)

**File Extension**: `.nq`

**MIME Type**: `application/n-quads`

**Format IRI**: `http://www.w3.org/ns/formats/N-Quads`

**Description**: Extension of N-Triples for RDF datasets. One quad per line with optional graph name.

**Graph Support**: Yes

**Dataset Support**: Yes

**RDF-star**: Yes (with `rdf-12` feature)

**Example**:
```nquads
<http://example.org/Alice> <http://example.org/knows> <http://example.org/Bob> <http://example.org/graph1> .
<http://example.org/Bob> <http://example.org/knows> <http://example.org/Charlie> <http://example.org/graph2> .
<http://example.org/Alice> <http://schema.org/name> "Alice" .
```

**Parser**: `oxttl::NQuadsParser`

**Serializer**: `oxttl::NQuadsSerializer`

**Use Cases**:
- Dataset dumps (e.g., Wikidata)
- Large-scale data exchange
- Streaming processing
- Database backups

---

### RDF/XML (application/rdf+xml)

**File Extension**: `.rdf`

**MIME Type**: `application/rdf+xml`

**Format IRI**: `http://www.w3.org/ns/formats/RDF_XML`

**Description**: XML-based RDF serialization. Legacy format with complex syntax.

**Graph Support**: Triples only (no named graphs)

**Dataset Support**: No

**RDF-star**: No

**Example**:
```xml
<?xml version="1.0"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
         xmlns:schema="http://schema.org/">
  <rdf:Description rdf:about="http://example.org/Alice">
    <rdf:type rdf:resource="http://schema.org/Person"/>
    <schema:name>Alice Smith</schema:name>
    <schema:age rdf:datatype="http://www.w3.org/2001/XMLSchema#integer">30</schema:age>
  </rdf:Description>
</rdf:RDF>
```

**Parser**: `oxrdfxml::RdfXmlParser`

**Serializer**: `oxrdfxml::RdfXmlSerializer`

**Use Cases**:
- Legacy systems
- XML toolchain integration
- Historical data

**Note**: Less commonly used in modern applications; Turtle or JSON-LD preferred

---

### JSON-LD (application/ld+json)

**File Extension**: `.jsonld`

**MIME Type**: `application/ld+json`

**Format IRI**: `https://www.w3.org/ns/formats/data/JSON-LD`

**Description**: JSON-based RDF format with context for mapping JSON to RDF.

**Graph Support**: Yes

**Dataset Support**: Yes

**RDF-star**: No

**JSON-LD Version**: 1.0 (JSON-LD 1.1 not yet supported)

**Example**:
```json
{
  "@context": {
    "schema": "http://schema.org/",
    "name": "schema:name",
    "age": "schema:age"
  },
  "@id": "http://example.org/Alice",
  "@type": "schema:Person",
  "name": "Alice Smith",
  "age": 30
}
```

**Parser**: `oxjsonld::JsonLdParser`

**Serializer**: `oxjsonld::JsonLdSerializer`

**Parsing Modes**:
- **Regular**: Buffers full file into memory
- **Streaming**: Avoids buffering in certain cases (enable with `JsonLdProfile::Streaming`)

**Use Cases**:
- Web APIs
- JavaScript applications
- JSON-native environments
- Linked Data on the web

**Status**: Work in progress; JSON-LD 1.1 support planned

---

### N3 (text/n3)

**File Extension**: `.n3`

**MIME Type**: `text/n3`

**Format IRI**: `http://www.w3.org/ns/formats/N3`

**Description**: Notation3 - superset of Turtle with additional features like formulas and rules.

**Graph Support**: Yes

**Dataset Support**: Yes

**RDF-star**: No (RDF 1.2 not supported for N3)

**Example**:
```n3
@prefix ex: <http://example.org/> .
@prefix log: <http://www.w3.org/2000/10/swap/log#> .

ex:Alice ex:knows ex:Bob .

{?x ex:knows ?y} => {?y ex:knownBy ?x} .
```

**Parser**: `oxttl::N3Parser`

**Use Cases**:
- Rules and reasoning
- Logic programming
- Complex knowledge representation

**Note**: Full N3 reasoning not implemented in Oxigraph; primarily parsing/serialization support

---

## Format Comparison Table

### MIME Types

| Format | Canonical MIME Type | Alternative MIME Types |
|--------|---------------------|------------------------|
| Turtle | `text/turtle` | `application/turtle`, `application/x-turtle` |
| TriG | `application/trig` | `application/x-trig` |
| N-Triples | `application/n-triples` | `text/plain`, `application/ntriples` |
| N-Quads | `application/n-quads` | `application/nquads` |
| RDF/XML | `application/rdf+xml` | `application/xml`, `text/xml` |
| JSON-LD | `application/ld+json` | `application/json`, `application/activity+json` |
| N3 | `text/n3` | - |

### File Extensions

| Format | Primary Extension | Alternative Extensions |
|--------|-------------------|------------------------|
| Turtle | `.ttl` | - |
| TriG | `.trig` | - |
| N-Triples | `.nt` | `.txt` |
| N-Quads | `.nq` | - |
| RDF/XML | `.rdf` | `.xml` |
| JSON-LD | `.jsonld` | `.json` |
| N3 | `.n3` | - |

### Feature Support

| Format | Prefixes | Blank Nodes | Datatypes | Lang Tags | Named Graphs | RDF-star* |
|--------|----------|-------------|-----------|-----------|--------------|-----------|
| Turtle | Yes | Yes | Yes | Yes | No | Yes |
| TriG | Yes | Yes | Yes | Yes | Yes | Yes |
| N-Triples | No | Yes | Yes | Yes | No | Yes |
| N-Quads | No | Yes | Yes | Yes | Yes | Yes |
| RDF/XML | Yes | Yes | Yes | Yes | No | No |
| JSON-LD | Context | Yes | Yes | Yes | Yes | No |
| N3 | Yes | Yes | Yes | Yes | Yes | No |

*With `rdf-12` feature flag

### Performance Characteristics

| Format | Parse Speed | File Size | Human Readable | Streaming |
|--------|-------------|-----------|----------------|-----------|
| Turtle | Medium | Small | High | Yes |
| TriG | Medium | Small | High | Yes |
| N-Triples | Fast | Large | Medium | Yes |
| N-Quads | Fast | Large | Medium | Yes |
| RDF/XML | Slow | Medium | Low | Yes |
| JSON-LD | Medium | Medium | High | Partial |
| N3 | Medium | Small | High | Yes |

---

## Format Selection Guide

### When to Use Each Format

**Turtle**:
- Human authoring
- Configuration files
- Small to medium datasets
- When readability is important

**TriG**:
- Multiple named graphs
- Dataset authoring
- When you need both readability and graph separation

**N-Triples**:
- Large datasets (millions of triples)
- Streaming processing
- Simple parsing requirements
- Log files and debugging

**N-Quads**:
- Large datasets with named graphs
- Dataset dumps (Wikidata, DBpedia)
- Bulk loading
- When you need maximum compatibility

**RDF/XML**:
- Legacy system integration
- XML-based toolchains
- Historical compatibility
- Not recommended for new projects

**JSON-LD**:
- Web APIs
- JavaScript/TypeScript applications
- JSON-native systems
- Linked Data publishing

**N3**:
- Rules and reasoning systems
- Advanced logic applications
- When Turtle syntax is insufficient

---

## RDF Version Support

### RDF 1.1 (Default)

All formats support RDF 1.1 by default.

### RDF 1.2 (Feature Flag)

Enable with the `rdf-12` feature flag in `oxrdfio`:

```toml
[dependencies]
oxrdfio = { version = "*", features = ["rdf-12"] }
```

**RDF 1.2 Additions**:
- Directional language tags
- Enhanced RDF-star support
- Updated semantics

**Formats Supporting RDF 1.2**:
- Turtle
- TriG
- N-Triples
- N-Quads

**Formats NOT Supporting RDF 1.2**:
- N3 (not yet implemented)
- JSON-LD (requires JSON-LD 1.1 support first)

---

## Usage Examples

### Format Detection from Extension

```rust
use oxrdfio::RdfFormat;

let format = RdfFormat::from_extension("ttl");
assert_eq!(format, Some(RdfFormat::Turtle));

let format = RdfFormat::from_extension("jsonld");
assert_eq!(format, Some(RdfFormat::JsonLd { profile: Default::default() }));
```

### Format Detection from MIME Type

```rust
use oxrdfio::RdfFormat;

let format = RdfFormat::from_media_type("text/turtle; charset=utf-8");
assert_eq!(format, Some(RdfFormat::Turtle));

let format = RdfFormat::from_media_type("application/n-quads");
assert_eq!(format, Some(RdfFormat::NQuads));
```

### Parsing with Format Auto-Detection

```rust
use oxrdfio::{RdfFormat, RdfParser};
use std::path::Path;

let path = Path::new("data.ttl");
let format = RdfFormat::from_extension(
    path.extension().unwrap().to_str().unwrap()
).unwrap();

let parser = RdfParser::from_format(format);
for quad in parser.for_reader(std::fs::File::open(path)?) {
    println!("{:?}", quad?);
}
```

### Format Conversion

```rust
use oxrdfio::{RdfFormat, RdfParser, RdfSerializer};

// Convert Turtle to N-Triples
let turtle_data = b"@prefix ex: <http://example.org/> . ex:s ex:p ex:o .";
let mut output = Vec::new();

let mut serializer = RdfSerializer::from_format(RdfFormat::NTriples)
    .for_writer(&mut output);

for quad in RdfParser::from_format(RdfFormat::Turtle)
    .for_reader(turtle_data.as_ref())
{
    serializer.serialize_quad(&quad?)?;
}

println!("{}", String::from_utf8(output)?);
```

---

## Advanced Features

### Async I/O Support

Enable async support with the `async-tokio` feature:

```toml
[dependencies]
oxrdfio = { version = "*", features = ["async-tokio"] }
```

Provides async versions of parsers and serializers for Tokio-based applications.

### Lenient Parsing

Some parsers support lenient mode for handling slightly invalid RDF:

```rust
// Via CLI
oxigraph load --lenient data.ttl

// Via API (varies by format)
// Check specific parser documentation
```

Useful for:
- Loading real-world data (e.g., Wikidata dumps)
- Recovering from minor syntax errors
- Compatibility with other systems

---

## References

- [RDF 1.1 Concepts](https://www.w3.org/TR/rdf11-concepts/)
- [RDF 1.2 Concepts](https://www.w3.org/TR/rdf12-concepts/)
- [W3C File Formats Registry](https://www.w3.org/ns/formats/)
- [IANA Media Types](https://www.iana.org/assignments/media-types/media-types.xhtml)
