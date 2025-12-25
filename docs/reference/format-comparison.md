# RDF Format Comparison

This reference provides detailed comparison of all RDF serialization formats supported by Oxigraph.

## Quick Reference Table

| Feature | Turtle | N-Triples | N-Quads | TriG | RDF/XML | JSON-LD | N3 |
|---------|--------|-----------|---------|------|---------|---------|-----|
| **File Extension** | .ttl | .nt | .nq | .trig | .rdf, .xml | .jsonld | .n3 |
| **Media Type** | text/turtle | application/n-triples | application/n-quads | application/trig | application/rdf+xml | application/ld+json | text/n3 |
| **Supports Datasets** | No | No | Yes | Yes | No | Yes | Yes |
| **Human Readable** | High | Low | Low | High | Medium | High | High |
| **File Size** | Small | Large | Large | Small | Large | Medium | Small |
| **Parse Speed** | Medium | Fast | Fast | Medium | Slow | Medium | Medium |
| **Streaming** | No | Yes | Yes | No | Partial | No | No |
| **Prefixes** | Yes | No | No | Yes | Yes | Yes | Yes |
| **Base IRI** | Yes | No | No | Yes | Yes | Yes | Yes |
| **RDF 1.2** | Yes* | Yes* | Yes* | Yes* | Yes* | No** | No |

\* With `rdf-12` feature flag
\** JSON-LD 1.0 only, not JSON-LD 1.1 yet

## Detailed Format Information

### Turtle (Terse RDF Triple Language)

**Specification:** [W3C Turtle](https://www.w3.org/TR/turtle/)

**Format Details:**
- **IRI:** `http://www.w3.org/ns/formats/Turtle`
- **Media Type:** `text/turtle`
- **File Extensions:** `.ttl`
- **Charset:** UTF-8

**Capabilities:**
- Supports RDF graphs (triples only)
- Prefix declarations with `@prefix`
- Base IRI with `@base`
- Compact syntax with semicolons and commas
- Blank node abbreviations `[]` and `()`
- Predicate-object lists
- Collection syntax for RDF lists

**Pros:**
- Most human-readable format
- Compact representation with prefixes
- Excellent for documentation and examples
- Wide tool support

**Cons:**
- Cannot represent named graphs
- Requires full document parsing (not streamable)
- Slower parsing than N-Triples

**Best Use Cases:**
- Hand-editing RDF data
- Configuration files
- Documentation and tutorials
- Small to medium datasets
- Version control friendly data

**Example:**
```turtle
@base <http://example.com/> .
@prefix schema: <http://schema.org/> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

<Alice> a schema:Person ;
    schema:name "Alice Smith"@en ;
    schema:age 30 ;
    schema:knows <Bob> , <Carol> ;
    schema:address [
        a schema:PostalAddress ;
        schema:streetAddress "123 Main St"
    ] .
```

---

### N-Triples

**Specification:** [W3C N-Triples](https://www.w3.org/TR/n-triples/)

**Format Details:**
- **IRI:** `http://www.w3.org/ns/formats/N-Triples`
- **Media Type:** `application/n-triples`
- **File Extensions:** `.nt`
- **Charset:** UTF-8

**Capabilities:**
- Supports RDF graphs (triples only)
- One triple per line
- No abbreviations or syntactic sugar
- Full IRIs required (no prefixes)
- Blank nodes with `_:` labels

**Pros:**
- Simplest RDF format
- Line-oriented (perfect for streaming)
- Fast parsing
- Easy debugging
- Suitable for parallel processing
- Stable format for diffs

**Cons:**
- Verbose (large file sizes)
- Not human-friendly
- No prefix support
- Cannot represent named graphs

**Best Use Cases:**
- Streaming large datasets
- Line-by-line processing (grep, sed, etc.)
- Parallel data processing
- Debugging RDF issues
- Data pipelines
- Machine-to-machine exchange

**Example:**
```ntriples
<http://example.com/Alice> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
<http://example.com/Alice> <http://schema.org/name> "Alice Smith"@en .
<http://example.com/Alice> <http://schema.org/age> "30"^^<http://www.w3.org/2001/XMLSchema#integer> .
<http://example.com/Alice> <http://schema.org/knows> <http://example.com/Bob> .
```

**Performance Characteristics:**
- Parse speed: ~1-2 million triples/second (typical)
- Memory: Constant (line-by-line processing)
- File size: ~3-5x larger than Turtle

---

### N-Quads

**Specification:** [W3C N-Quads](https://www.w3.org/TR/n-quads/)

**Format Details:**
- **IRI:** `http://www.w3.org/ns/formats/N-Quads`
- **Media Type:** `application/n-quads`
- **File Extensions:** `.nq`
- **Charset:** UTF-8

**Capabilities:**
- Supports RDF datasets (quads with named graphs)
- One quad per line
- Extension of N-Triples syntax
- Fourth component for graph name
- Default graph when fourth component omitted

**Pros:**
- Supports named graphs
- Line-oriented streaming
- Fast parsing
- Simple debugging
- Parallel processing friendly

**Cons:**
- Very verbose
- No prefix support
- Not human-friendly

**Best Use Cases:**
- Streaming datasets with provenance
- Processing multi-source RDF data
- Named graph management
- Data integration from multiple sources
- Line-by-line dataset processing

**Example:**
```nquads
<http://example.com/Alice> <http://schema.org/name> "Alice" <http://example.com/graph1> .
<http://example.com/Bob> <http://schema.org/name> "Bob" <http://example.com/graph1> .
<http://example.com/Carol> <http://schema.org/name> "Carol" <http://example.com/graph2> .
<http://example.com/Alice> <http://schema.org/knows> <http://example.com/Bob> .
```

**Note:** When the graph component is omitted, the triple belongs to the default graph.

---

### TriG (Turtle + Named Graphs)

**Specification:** [W3C TriG](https://www.w3.org/TR/trig/)

**Format Details:**
- **IRI:** `http://www.w3.org/ns/formats/TriG`
- **Media Type:** `application/trig`
- **File Extensions:** `.trig`
- **Charset:** UTF-8

**Capabilities:**
- Supports RDF datasets with named graphs
- Extends Turtle syntax
- Graph blocks with `{ }` syntax
- All Turtle features within graph blocks
- Default graph for statements outside blocks

**Pros:**
- Human-readable datasets
- Combines Turtle readability with named graphs
- Compact with prefixes
- Clear graph organization

**Cons:**
- Cannot stream (requires full parsing)
- More complex than Turtle
- Slower parsing than N-Quads

**Best Use Cases:**
- Human-readable multi-source data
- Datasets with clear graph boundaries
- Configuration with provenance
- Documentation of complex datasets

**Example:**
```trig
@prefix schema: <http://schema.org/> .
@prefix ex: <http://example.com/> .

# Default graph
ex:Alice a schema:Person .

# Named graph 1
ex:graph1 {
    ex:Alice schema:name "Alice" ;
        schema:age 30 .
}

# Named graph 2
ex:graph2 {
    ex:Bob schema:name "Bob" ;
        schema:knows ex:Alice .
}
```

---

### RDF/XML

**Specification:** [W3C RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/)

**Format Details:**
- **IRI:** `http://www.w3.org/ns/formats/RDF_XML`
- **Media Type:** `application/rdf+xml`
- **File Extensions:** `.rdf`, `.owl`, `.xml`
- **Charset:** UTF-8

**Capabilities:**
- Supports RDF graphs (triples only)
- XML-based representation
- Namespace prefixes via XML namespaces
- Multiple serialization patterns
- xml:base for base IRI

**Pros:**
- XML ecosystem compatibility
- XML schema validation possible
- Corporate XML infrastructure
- Historic W3C standard

**Cons:**
- Very verbose
- Complex parsing
- Multiple valid serializations for same graph
- Slow parse/serialize
- Not recommended for new projects

**Best Use Cases:**
- Legacy system integration
- XML-based workflows
- OWL ontologies (historical)
- Systems requiring XML validation

**Example:**
```xml
<?xml version="1.0" encoding="UTF-8"?>
<rdf:RDF
    xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
    xmlns:schema="http://schema.org/"
    xml:base="http://example.com/">

  <schema:Person rdf:about="Alice">
    <schema:name xml:lang="en">Alice Smith</schema:name>
    <schema:age rdf:datatype="http://www.w3.org/2001/XMLSchema#integer">30</schema:age>
    <schema:knows rdf:resource="Bob"/>
  </schema:Person>

</rdf:RDF>
```

**Performance Characteristics:**
- Parse speed: Slowest (XML parsing overhead)
- File size: Largest (XML verbosity)
- Memory: High (DOM/SAX parsing)

---

### JSON-LD (JSON for Linking Data)

**Specification:** [W3C JSON-LD 1.0](https://www.w3.org/TR/json-ld/)

**Format Details:**
- **IRI:** `http://www.w3.org/ns/formats/JSON-LD`
- **Media Type:** `application/ld+json`
- **File Extensions:** `.jsonld`
- **Charset:** UTF-8

**Capabilities:**
- Supports RDF datasets (with `@graph`)
- JSON syntax with `@context` for mapping
- Compaction and expansion algorithms
- Streaming support (limited)
- Frame-based querying

**Implementation Status:**
- JSON-LD 1.0: Fully supported
- JSON-LD 1.1: Not yet supported
- Streaming JSON-LD: Supported with profile setting

**Pros:**
- Native JSON format
- Web developer friendly
- Excellent for APIs
- SEO/structured data (schema.org)
- JavaScript integration

**Cons:**
- More complex than other formats
- Requires context processing
- Not fully compact without context
- JSON-LD 1.1 not yet available

**Best Use Cases:**
- Web APIs and REST services
- JavaScript/Node.js applications
- Embedding in HTML (`<script>` tags)
- Schema.org markup for SEO
- JSON-based systems

**Example:**
```json
{
  "@context": {
    "schema": "http://schema.org/",
    "name": "schema:name",
    "age": {
      "@id": "schema:age",
      "@type": "http://www.w3.org/2001/XMLSchema#integer"
    },
    "knows": {
      "@id": "schema:knows",
      "@type": "@id"
    }
  },
  "@id": "http://example.com/Alice",
  "@type": "schema:Person",
  "name": "Alice Smith",
  "age": 30,
  "knows": "http://example.com/Bob"
}
```

**With Named Graphs:**
```json
{
  "@context": {
    "schema": "http://schema.org/"
  },
  "@graph": [
    {
      "@id": "http://example.com/graph1",
      "@graph": [
        {
          "@id": "http://example.com/Alice",
          "@type": "schema:Person",
          "schema:name": "Alice"
        }
      ]
    }
  ]
}
```

---

### N3 (Notation3)

**Specification:** [W3C N3](https://w3c.github.io/N3/spec/)

**Format Details:**
- **IRI:** `http://www.w3.org/ns/formats/N3`
- **Media Type:** `text/n3`
- **File Extensions:** `.n3`
- **Charset:** UTF-8

**Capabilities:**
- Superset of Turtle
- Supports formulas and rules (not in Oxigraph)
- Variables and quantification (not in Oxigraph)
- All Turtle features

**Implementation Note:**
Oxigraph supports N3 as a Turtle superset for compatibility but does not support N3-specific features (formulas, rules, logic).

**Pros:**
- Turtle-compatible
- Future-proof for N3 features

**Cons:**
- Limited N3-specific features in Oxigraph
- Most use cases better served by Turtle/TriG

**Best Use Cases:**
- N3 tool compatibility
- Most users should use Turtle/TriG instead

**Example:**
```n3
@prefix schema: <http://schema.org/> .
@prefix ex: <http://example.com/> .

ex:Alice a schema:Person ;
    schema:name "Alice Smith" .
```

---

## Format Support Matrix

### Dataset vs Graph Support

| Format | Graphs | Datasets (Named Graphs) |
|--------|--------|------------------------|
| Turtle | Yes | No |
| N-Triples | Yes | No |
| N-Quads | Yes (default graph) | Yes |
| TriG | Yes | Yes |
| RDF/XML | Yes | No |
| JSON-LD | Yes | Yes |
| N3 | Yes | Yes |

### Syntax Features

| Feature | Turtle | N-Triples | N-Quads | TriG | RDF/XML | JSON-LD | N3 |
|---------|--------|-----------|---------|------|---------|---------|-----|
| Prefix declarations | Yes | No | No | Yes | Yes (xmlns) | Yes (@context) | Yes |
| Base IRI | Yes | No | No | Yes | Yes (xml:base) | Yes (@base) | Yes |
| Blank node labels | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Anonymous blank nodes | Yes (`[]`) | No | No | Yes (`[]`) | Yes (rdf:nodeID) | Yes | Yes |
| Collection syntax | Yes (`()`) | No | No | Yes (`()`) | Yes (rdf:List) | Yes (@list) | Yes |
| Language tags | Yes | Yes | Yes | Yes | Yes (xml:lang) | Yes (@language) | Yes |
| Typed literals | Yes | Yes | Yes | Yes | Yes (rdf:datatype) | Yes (@type) | Yes |
| Comments | Yes (`#`) | Yes (`#`) | Yes (`#`) | Yes (`#`) | Yes (XML `<!-- -->`) | No | Yes (`#`) |

### MIME Types and Extensions

| Format | Primary MIME Type | Alternative MIME Types | Extensions |
|--------|------------------|----------------------|------------|
| Turtle | text/turtle | application/x-turtle | .ttl |
| N-Triples | application/n-triples | text/plain | .nt |
| N-Quads | application/n-quads | text/x-nquads | .nq |
| TriG | application/trig | - | .trig |
| RDF/XML | application/rdf+xml | application/xml, text/xml | .rdf, .owl, .xml |
| JSON-LD | application/ld+json | application/json | .jsonld |
| N3 | text/n3 | text/rdf+n3 | .n3 |

### API Access

All formats accessible via:

**Rust:**
```rust
use oxrdfio::RdfFormat;

RdfFormat::Turtle
RdfFormat::NTriples
RdfFormat::NQuads
RdfFormat::TriG
RdfFormat::RdfXml
RdfFormat::JsonLd { profile }
RdfFormat::N3
```

**Python:**
```python
from pyoxigraph import RdfFormat

RdfFormat.TURTLE
RdfFormat.N_TRIPLES
RdfFormat.N_QUADS
RdfFormat.TRIG
RdfFormat.RDF_XML
RdfFormat.JSON_LD
RdfFormat.N3
```

**JavaScript:**
```javascript
import { RdfFormat } from 'oxigraph';

RdfFormat.TURTLE
RdfFormat.N_TRIPLES
RdfFormat.N_QUADS
RdfFormat.TRIG
RdfFormat.RDF_XML
RdfFormat.JSON_LD
RdfFormat.N3
```

## Performance Comparison

### Parse Speed (Relative)

Based on typical implementations:

| Format | Speed | Notes |
|--------|-------|-------|
| N-Triples | Fast (1.0x) | Baseline |
| N-Quads | Fast (1.0x) | Same as N-Triples |
| Turtle | Medium (0.6x) | Prefix expansion overhead |
| TriG | Medium (0.6x) | Similar to Turtle |
| N3 | Medium (0.6x) | Similar to Turtle |
| JSON-LD | Medium (0.5x) | Context processing |
| RDF/XML | Slow (0.3x) | XML parsing overhead |

### File Size (Relative)

Assuming same RDF graph:

| Format | Size | Notes |
|--------|------|-------|
| Turtle | Small (1.0x) | Baseline with prefixes |
| TriG | Small (1.1x) | Slightly larger with graph syntax |
| N3 | Small (1.0x) | Same as Turtle |
| JSON-LD | Medium (1.5x) | JSON overhead, context-dependent |
| N-Triples | Large (3.0x) | Full IRIs |
| N-Quads | Large (3.5x) | Full IRIs + graph component |
| RDF/XML | Large (4.0x) | XML verbosity |

### Memory Usage

| Format | Memory | Notes |
|--------|--------|-------|
| N-Triples | Low | Line-oriented, constant memory |
| N-Quads | Low | Line-oriented, constant memory |
| Turtle | Medium | Requires prefix table |
| TriG | Medium | Requires prefix table |
| N3 | Medium | Requires prefix table |
| JSON-LD | High | Context processing and expansion |
| RDF/XML | High | XML DOM/SAX parsing |

## Choosing the Right Format

### Decision Tree

```
Need datasets (named graphs)?
├─ Yes
│  ├─ Human readable? → Use TriG
│  ├─ Streaming/processing? → Use N-Quads
│  └─ Web/JavaScript? → Use JSON-LD
└─ No (graphs only)
   ├─ Human readable? → Use Turtle
   ├─ Streaming/processing? → Use N-Triples
   ├─ Web/JavaScript? → Use JSON-LD
   ├─ Legacy XML system? → Use RDF/XML
   └─ N3 compatibility? → Use N3
```

### By Use Case

| Use Case | Recommended Format | Alternative |
|----------|-------------------|-------------|
| Hand editing | Turtle, TriG | N3 |
| Large file processing | N-Triples, N-Quads | - |
| Web APIs | JSON-LD | Turtle |
| JavaScript apps | JSON-LD | - |
| Legacy systems | RDF/XML | - |
| Documentation | Turtle, TriG | - |
| Debugging | N-Triples, N-Quads | - |
| Version control | Turtle, N-Triples | - |
| SEO/Schema.org | JSON-LD | - |
| Data integration | N-Quads, TriG | JSON-LD |

## Format Conversion Recommendations

### Lossless Conversions

These conversions preserve all RDF information:

- Turtle ↔ N-Triples
- TriG ↔ N-Quads
- Turtle → RDF/XML (graphs only)
- Any format → N-Quads (universal)

### Lossy Conversions

These conversions may lose formatting information:

- **Prefix loss:** Any format → N-Triples/N-Quads
- **Graph loss:** N-Quads/TriG/JSON-LD → Turtle/N-Triples/RDF/XML (named graphs become default graph)
- **Base IRI loss:** Any format → N-Triples/N-Quads

## RDF 1.2 Support

RDF 1.2 features available with `rdf-12` feature flag:

| Feature | Turtle | N-Triples | N-Quads | TriG | RDF/XML | JSON-LD | N3 |
|---------|--------|-----------|---------|------|---------|---------|-----|
| Directional language tags | Yes | Yes | Yes | Yes | Yes | No | No |
| Triple terms (RDF-star) | Yes | Yes | Yes | Yes | No | No | No |

**Note:** JSON-LD 1.1 support (with RDF 1.2) is not yet implemented.

## Further Reading

- [RDF 1.1 Concepts](https://www.w3.org/TR/rdf11-concepts/)
- [RDF 1.2 Concepts](https://www.w3.org/TR/rdf12-concepts/)
- [Turtle Specification](https://www.w3.org/TR/turtle/)
- [N-Triples Specification](https://www.w3.org/TR/n-triples/)
- [N-Quads Specification](https://www.w3.org/TR/n-quads/)
- [TriG Specification](https://www.w3.org/TR/trig/)
- [RDF/XML Specification](https://www.w3.org/TR/rdf-syntax-grammar/)
- [JSON-LD Specification](https://www.w3.org/TR/json-ld/)
- [N3 Specification](https://w3c.github.io/N3/spec/)
- [Unique URIs for File Formats](https://www.w3.org/ns/formats/)

## See Also

- [RDF Formats Introduction](../tutorials/rdf-formats-intro.md)
- [How to Parse RDF Files](../how-to/parse-rdf-files.md)
- [How to Serialize RDF](../how-to/serialize-rdf.md)
