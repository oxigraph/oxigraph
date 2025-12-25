# RDF Serialization Formats: An Introduction

This tutorial introduces the RDF serialization formats supported by Oxigraph and helps you choose the right format for your use case.

## What is RDF Serialization?

RDF (Resource Description Framework) data exists as abstract triples and quads, but to store or transmit this data, we need to serialize it into a text or binary format. Different serialization formats have different strengths and use cases.

## Supported Formats

Oxigraph supports the following W3C standard RDF serialization formats:

### 1. Turtle (Terse RDF Triple Language)

**File Extension:** `.ttl`
**Media Type:** `text/turtle`
**Supports Datasets:** No (only graphs)

Turtle is designed for human readability and hand-editing. It uses a compact syntax with prefixes to make URIs more readable.

**Example:**
```turtle
@prefix schema: <http://schema.org/> .
@prefix ex: <http://example.com/> .

ex:Alice a schema:Person ;
    schema:name "Alice Smith" ;
    schema:age 30 ;
    schema:knows ex:Bob .
```

**Best for:**
- Hand-editing RDF data
- Documentation and examples
- Small to medium datasets
- Situations where readability matters

### 2. N-Triples

**File Extension:** `.nt`
**Media Type:** `application/n-triples`
**Supports Datasets:** No (only graphs)

N-Triples is the simplest RDF format with one triple per line and no abbreviations. Every IRI is written in full.

**Example:**
```ntriples
<http://example.com/Alice> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
<http://example.com/Alice> <http://schema.org/name> "Alice Smith" .
<http://example.com/Alice> <http://schema.org/age> "30"^^<http://www.w3.org/2001/XMLSchema#integer> .
```

**Best for:**
- Streaming large datasets
- Line-by-line processing
- Debugging and testing
- Data exchange where simplicity is critical
- Parallel processing (each line is independent)

### 3. N-Quads

**File Extension:** `.nq`
**Media Type:** `application/n-quads`
**Supports Datasets:** Yes (includes named graphs)

N-Quads extends N-Triples to support RDF datasets with named graphs. Each line contains a quad (subject, predicate, object, graph).

**Example:**
```nquads
<http://example.com/Alice> <http://schema.org/name> "Alice Smith" <http://example.com/graph1> .
<http://example.com/Bob> <http://schema.org/name> "Bob Jones" <http://example.com/graph2> .
```

**Best for:**
- Working with named graphs
- Streaming large datasets with provenance
- Data from multiple sources
- Line-by-line processing with context

### 4. TriG (Turtle with named Graphs)

**File Extension:** `.trig`
**Media Type:** `application/trig`
**Supports Datasets:** Yes

TriG extends Turtle to support named graphs while maintaining readability.

**Example:**
```trig
@prefix schema: <http://schema.org/> .
@prefix ex: <http://example.com/> .

ex:graph1 {
    ex:Alice a schema:Person ;
        schema:name "Alice Smith" .
}

ex:graph2 {
    ex:Bob a schema:Person ;
        schema:name "Bob Jones" .
}
```

**Best for:**
- Human-readable datasets with named graphs
- Organizing RDF data by source or context
- Situations where Turtle's readability is needed for datasets

### 5. RDF/XML

**File Extension:** `.rdf` or `.xml`
**Media Type:** `application/rdf+xml`
**Supports Datasets:** No

RDF/XML was the first standard RDF serialization format. It represents RDF as XML.

**Example:**
```xml
<?xml version="1.0"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"
         xmlns:schema="http://schema.org/">
  <schema:Person rdf:about="http://example.com/Alice">
    <schema:name>Alice Smith</schema:name>
    <schema:age rdf:datatype="http://www.w3.org/2001/XMLSchema#integer">30</schema:age>
  </schema:Person>
</rdf:RDF>
```

**Best for:**
- Legacy systems requiring XML
- Integration with XML-based tools
- Corporate environments with XML infrastructure
- When XML schema validation is needed

**Note:** While fully supported, RDF/XML is generally not recommended for new projects due to complexity and verbosity.

### 6. JSON-LD (JSON for Linking Data)

**File Extension:** `.jsonld`
**Media Type:** `application/ld+json`
**Supports Datasets:** Yes

JSON-LD represents RDF as JSON, making it accessible to web developers and JavaScript applications.

**Example:**
```json
{
  "@context": {
    "schema": "http://schema.org/",
    "name": "schema:name",
    "age": "schema:age"
  },
  "@id": "http://example.com/Alice",
  "@type": "schema:Person",
  "name": "Alice Smith",
  "age": 30
}
```

**Best for:**
- Web APIs and REST services
- JavaScript applications
- Embedding RDF in HTML (with `<script type="application/ld+json">`)
- Integration with JSON-based systems
- SEO and structured data on websites

### 7. N3 (Notation3)

**File Extension:** `.n3`
**Media Type:** `text/n3`
**Supports Datasets:** Yes

N3 is a superset of Turtle that adds formulas and rules. Oxigraph supports N3 parsing for compatibility.

**Example:**
```n3
@prefix schema: <http://schema.org/> .
@prefix ex: <http://example.com/> .

ex:Alice a schema:Person ;
    schema:name "Alice Smith" .
```

**Best for:**
- Working with N3-specific features
- Compatibility with N3 tools
- Most users should prefer Turtle or TriG

## Quick Format Comparison

| Format | Readable | Compact | Datasets | Streaming | Best Use Case |
|--------|----------|---------|----------|-----------|---------------|
| Turtle | High | High | No | No | Human editing, docs |
| N-Triples | Low | Low | No | Yes | Processing, debugging |
| N-Quads | Low | Low | Yes | Yes | Dataset processing |
| TriG | High | High | Yes | No | Human-readable datasets |
| RDF/XML | Medium | Low | No | No | Legacy systems |
| JSON-LD | High | Medium | Yes | No | Web APIs, JavaScript |
| N3 | High | High | Yes | No | N3 compatibility |

## Choosing a Format

### For Human Readability
Choose **Turtle** for graphs or **TriG** for datasets. These formats are designed to be readable and editable.

### For Processing Large Files
Choose **N-Triples** for graphs or **N-Quads** for datasets. Their line-oriented structure enables streaming and parallel processing.

### For Web Applications
Choose **JSON-LD**. It integrates seamlessly with JavaScript and modern web stacks.

### For Legacy Systems
Choose **RDF/XML** if required by existing tools or corporate standards.

### For Debugging
Choose **N-Triples** or **N-Quads**. The simple, unabbreviated format makes it easy to see exactly what's in your data.

## Format Features

### Prefixes and Base IRIs

Turtle, TriG, N3, RDF/XML, and JSON-LD support prefixes to abbreviate URIs:

```turtle
@prefix ex: <http://example.com/> .
@base <http://example.com/> .

ex:Alice a ex:Person .  # Expands to http://example.com/Alice
<Bob> a ex:Person .      # Expands to http://example.com/Bob
```

N-Triples and N-Quads require full URIs and don't support prefixes.

### Named Graphs

Only N-Quads, TriG, N3, and JSON-LD support named graphs (RDF datasets):

```trig
# TriG example
@prefix ex: <http://example.com/> .

ex:graph1 {
    ex:Alice ex:knows ex:Bob .
}
```

```nquads
# N-Quads example
<http://example.com/Alice> <http://example.com/knows> <http://example.com/Bob> <http://example.com/graph1> .
```

### Blank Nodes

All formats support blank nodes, but represent them differently:

```turtle
# Turtle
_:b1 a ex:Person .
[ a ex:Person ; ex:name "Anonymous" ] .
```

```ntriples
# N-Triples
_:b1 <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://example.com/Person> .
```

### Literals and Datatypes

All formats support typed literals and language tags:

```turtle
ex:Alice
    ex:name "Alice"@en ;
    ex:age 30 ;
    ex:height "1.75"^^xsd:decimal .
```

## Performance Characteristics

### Parsing Speed
N-Triples and N-Quads are fastest to parse due to their simple structure. Turtle and TriG require more processing for prefix expansion and syntax features.

### File Size
Turtle and TriG produce the most compact files due to prefixes and abbreviations. N-Triples and N-Quads produce larger files but are more efficient for streaming.

### Memory Usage
N-Triples and N-Quads can be processed with minimal memory as they're line-oriented. Other formats may require buffering for parsing complex structures.

## Next Steps

- Learn how to [parse RDF files](../how-to/parse-rdf-files.md) in Oxigraph
- Learn how to [serialize RDF data](../how-to/serialize-rdf.md) to different formats
- See the [format comparison table](../reference/format-comparison.md) for detailed specifications

## Further Reading

- [RDF 1.1 Primer](https://www.w3.org/TR/rdf11-primer/) - W3C introduction to RDF
- [Turtle Specification](https://www.w3.org/TR/turtle/)
- [JSON-LD Specification](https://www.w3.org/TR/json-ld/)
- [RDF/XML Specification](https://www.w3.org/TR/rdf-syntax-grammar/)
