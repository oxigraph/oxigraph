# Oxigraph Crates Reference

This document provides a complete overview of all Oxigraph crates, their purposes, and dependency relationships.

## Core Crates

### oxigraph
[![Crates.io](https://img.shields.io/crates/v/oxigraph.svg)](https://crates.io/crates/oxigraph)
[![Documentation](https://docs.rs/oxigraph/badge.svg)](https://docs.rs/oxigraph)

**Purpose**: Main graph database library implementing the SPARQL standard.

**Use Cases**:
- Embedded RDF database in Rust applications
- SPARQL query and update execution
- On-disk persistent storage with RocksDB
- In-memory graph database

**Key Features**:
- SPARQL 1.1 Query, Update, and Federated Query support
- Multiple RDF format support (Turtle, N-Triples, N-Quads, TriG, RDF/XML, JSON-LD)
- RocksDB-backed persistent storage
- In-memory fallback option
- Transaction support with "repeatable read" isolation

**Dependencies**:
- `oxrdf` - RDF data model
- `oxrdfio` - RDF I/O operations
- `spargebra` - SPARQL parsing
- `spareval` - SPARQL evaluation
- `sparopt` - SPARQL optimization
- `sparesults` - SPARQL results serialization
- `oxsdatatypes` - XSD datatypes

**Feature Flags**:
- `rocksdb` (default) - Enable RocksDB backend
- `http-client` - Enable HTTP client for SERVICE queries
- `rdf-12` - Enable RDF 1.2 features

---

## RDF Data Model

### oxrdf
[![Crates.io](https://img.shields.io/crates/v/oxrdf.svg)](https://crates.io/crates/oxrdf)
[![Documentation](https://docs.rs/oxrdf/badge.svg)](https://docs.rs/oxrdf)

**Purpose**: Core RDF 1.1 data structures and concepts.

**Use Cases**:
- Building block for RDF applications
- In-memory RDF graphs and datasets
- RDF term manipulation

**Key Types**:
- `NamedNode` - IRI references
- `BlankNode` - Blank nodes
- `Literal` - RDF literals with datatypes/language tags
- `Term` - Union of all node types
- `Triple` - RDF statements (subject, predicate, object)
- `Quad` - RDF statements with graph name
- `Graph` - In-memory triple collection
- `Dataset` - In-memory quad collection

**Feature Flags**:
- `rdf-12` - Enable RDF 1.2 support (directional language tags)
- `rdfc-10` - Enable RDF Dataset Canonicalization

**Inspiration**: Based on [RDF/JS](https://rdf.js.org/data-model-spec/) and [Apache Commons RDF](http://commons.apache.org/proper/commons-rdf/)

---

## RDF I/O Stack

### oxrdfio
[![Crates.io](https://img.shields.io/crates/v/oxrdfio.svg)](https://crates.io/crates/oxrdfio)
[![Documentation](https://docs.rs/oxrdfio/badge.svg)](https://docs.rs/oxrdfio)

**Purpose**: Unified parser and serializer API for RDF formats.

**Use Cases**:
- Converting between RDF formats
- Parsing RDF from files or streams
- Serializing RDF data

**Supported Formats**:
- Turtle, TriG, N-Triples, N-Quads, N3 (via `oxttl`)
- RDF/XML (via `oxrdfxml`)
- JSON-LD 1.0 (via `oxjsonld`)

**Dependencies**:
- `oxrdf` - RDF data model
- `oxttl` - Turtle family parsers
- `oxrdfxml` - RDF/XML parser
- `oxjsonld` - JSON-LD parser

**Feature Flags**:
- `rdf-12` - Enable RDF 1.2 support
- `async-tokio` - Enable asynchronous I/O

**Entry Points**: `RdfParser` and `RdfSerializer` structs

---

### oxttl
[![Crates.io](https://img.shields.io/crates/v/oxttl.svg)](https://crates.io/crates/oxttl)
[![Documentation](https://docs.rs/oxttl/badge.svg)](https://docs.rs/oxttl)

**Purpose**: Parser and serializer for Turtle family formats.

**Use Cases**:
- Parsing Turtle, TriG, N-Triples, N-Quads, N3
- Serializing to Turtle family formats
- Low-level streaming parsers

**Supported Formats**:
- [Turtle](https://www.w3.org/TR/turtle/)
- [TriG](https://www.w3.org/TR/trig/)
- [N-Triples](https://www.w3.org/TR/n-triples/)
- [N-Quads](https://www.w3.org/TR/n-quads/)
- [N3](https://w3c.github.io/N3/spec/)

**Feature Flags**:
- `rdf-12` - Enable RDF 1.2 support (all formats except N3)

**Design**: Low-level parser compatible with both synchronous and asynchronous I/O

---

### oxrdfxml
[![Crates.io](https://img.shields.io/crates/v/oxrdfxml.svg)](https://crates.io/crates/oxrdfxml)
[![Documentation](https://docs.rs/oxrdfxml/badge.svg)](https://docs.rs/oxrdfxml)

**Purpose**: Parser and serializer for RDF/XML.

**Use Cases**:
- Parsing RDF/XML documents
- Serializing RDF to XML format
- Legacy RDF format support

**Supported Format**:
- [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/)

**Entry Points**: `RdfXmlParser` and `RdfXmlSerializer` structs

---

### oxjsonld
[![Crates.io](https://img.shields.io/crates/v/oxjsonld.svg)](https://crates.io/crates/oxjsonld)
[![Documentation](https://docs.rs/oxjsonld/badge.svg)](https://docs.rs/oxjsonld)

**Purpose**: Parser and serializer for JSON-LD.

**Use Cases**:
- Parsing JSON-LD documents
- Serializing RDF to JSON-LD
- Web-friendly RDF representation

**Supported Format**:
- [JSON-LD 1.0](https://www.w3.org/TR/json-ld/) (JSON-LD 1.1 not yet supported)

**Parsing Modes**:
- Regular JSON-LD parsing (buffers full file)
- [Streaming JSON-LD](https://www.w3.org/TR/json-ld11-streaming/) (avoids buffering in some cases)

**Entry Points**: `JsonLdParser` and `JsonLdSerializer` structs

**Note**: Work in progress - JSON-LD 1.1 support planned

---

## SPARQL Stack

### spargebra
[![Crates.io](https://img.shields.io/crates/v/spargebra.svg)](https://crates.io/crates/spargebra)
[![Documentation](https://docs.rs/spargebra/badge.svg)](https://docs.rs/spargebra)

**Purpose**: SPARQL parser and algebra representation.

**Use Cases**:
- Parsing SPARQL queries and updates
- SPARQL syntax tree manipulation
- Building SPARQL tools

**Supported Standards**:
- [SPARQL 1.1 Query](https://www.w3.org/TR/sparql11-query/)
- [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/)

**Feature Flags**:
- `sparql-12` - Enable SPARQL 1.2 support
- `standard-unicode-escaping` - Allow `\uXXXX` escape sequences everywhere (not just in IRIs/strings)

**Entry Points**: `Query` and `Update` structs

**Design**: Based on [SPARQL 1.1 Query Algebra](https://www.w3.org/TR/sparql11-query/#sparqlQuery)

---

### spareval
[![Crates.io](https://img.shields.io/crates/v/spareval.svg)](https://crates.io/crates/spareval)
[![Documentation](https://docs.rs/spareval/badge.svg)](https://docs.rs/spareval)

**Purpose**: SPARQL Query evaluation engine.

**Use Cases**:
- Executing SPARQL queries
- Building custom SPARQL implementations
- Query result processing

**Dependencies**:
- `spargebra` - SPARQL parsing
- `sparopt` - SPARQL optimization

**Feature Flags**:
- `sparql-12` - Enable SPARQL 1.2 changes
- `sep-0002` - Enable `ADJUST` function and arithmetic on date/time types
- `sep-0006` - Enable `LATERAL` keyword
- `calendar-ext` - Arithmetic on `xsd:gYear`, `xsd:gYearMonth`, etc.

**Entry Point**: `QueryEvaluator` struct

---

### sparopt
[![Crates.io](https://img.shields.io/crates/v/sparopt.svg)](https://crates.io/crates/sparopt)
[![Documentation](https://docs.rs/sparopt/badge.svg)](https://docs.rs/sparopt)

**Purpose**: SPARQL Query optimizer (work in progress).

**Use Cases**:
- Optimizing SPARQL queries
- Query rewriting
- Performance improvement

**Dependencies**:
- `spargebra` - SPARQL algebra

**Feature Flags**:
- `sparql-12` - Enable SPARQL 1.2 support

**Note**: Optimizer ensures rewritten query returns exact same results as input, but may discard some errors

---

### sparesults
[![Crates.io](https://img.shields.io/crates/v/sparesults.svg)](https://crates.io/crates/sparesults)
[![Documentation](https://docs.rs/sparesults/badge.svg)](https://docs.rs/sparesults)

**Purpose**: Parsers and serializers for SPARQL query results formats.

**Use Cases**:
- Serializing SPARQL query results
- Parsing SPARQL results from other endpoints
- Format conversion

**Supported Formats**:
- [SPARQL Query Results XML Format](https://www.w3.org/TR/rdf-sparql-XMLres/)
- [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/)
- [SPARQL Query Results CSV Format](https://www.w3.org/TR/sparql11-results-csv-tsv/)
- [SPARQL Query Results TSV Format](https://www.w3.org/TR/sparql11-results-csv-tsv/)

**Feature Flags**:
- `sparql-12` - Enable SPARQL 1.2 support

**Entry Points**: `QueryResultsParser` and `QueryResultsSerializer` structs

---

## Extensions and Utilities

### sparshacl
[![Crates.io](https://img.shields.io/crates/v/sparshacl.svg)](https://crates.io/crates/sparshacl)
[![Documentation](https://docs.rs/sparshacl/badge.svg)](https://docs.rs/sparshacl)

**Purpose**: SHACL (Shapes Constraint Language) implementation for RDF validation.

**Use Cases**:
- Validating RDF graphs against shape constraints
- Data quality checking
- Ontology validation

**Features**:
- SHACL Core constraint components
- Property paths (predicate, sequence, alternative, inverse)
- W3C-compliant validation reports
- Target declarations (targetClass, targetNode, targetSubjectsOf, targetObjectsOf)
- Logical constraints (sh:and, sh:or, sh:not, sh:xone)

**Feature Flags**:
- `sparql` - Enable SPARQL-based constraints (sh:sparql)

**Standard**: [W3C SHACL](https://www.w3.org/TR/shacl/)

**Entry Point**: `ShaclValidator` struct

---

### spargeo
[![Crates.io](https://img.shields.io/crates/v/spargeo.svg)](https://crates.io/crates/spargeo)
[![Documentation](https://docs.rs/spargeo/badge.svg)](https://docs.rs/spargeo)

**Purpose**: GeoSPARQL extension functions for geospatial queries.

**Use Cases**:
- Geospatial SPARQL queries
- Geographic data analysis
- Location-based filtering

**Standard**: [GeoSPARQL](https://docs.ogc.org/is/22-047r1/22-047r1.html)

**Status**: Partial, slow, work in progress

**Entry Point**: `GEOSPARQL_EXTENSION_FUNCTIONS` constant

---

### oxsdatatypes
[![Crates.io](https://img.shields.io/crates/v/oxsdatatypes.svg)](https://crates.io/crates/oxsdatatypes)
[![Documentation](https://docs.rs/oxsdatatypes/badge.svg)](https://docs.rs/oxsdatatypes)

**Purpose**: Implementation of XML Schema Definition Language Datatypes.

**Use Cases**:
- XSD datatype parsing and validation
- SPARQL/XPath function implementation
- Type conversion and arithmetic

**Supported Datatypes**:
- Numeric types (decimal, integer, float, double, etc.)
- Date/time types (dateTime, date, time, duration, etc.)
- String and boolean types

**Features**:
- `FromStr` and `Display` implementations
- XPath casting functions
- Identity, equality, and order relations
- Binary serialization (from_be_bytes/to_be_bytes)

**Standard**: [XML Schema Definition Language Datatypes](https://www.w3.org/TR/xmlschema11-2/)

---

### oxowl
[![Crates.io](https://img.shields.io/crates/v/oxowl.svg)](https://crates.io/crates/oxowl)
[![Documentation](https://docs.rs/oxowl/badge.svg)](https://docs.rs/oxowl)

**Purpose**: OWL 2 ontology support for Oxigraph.

**Use Cases**:
- OWL ontology parsing and manipulation
- Ontological reasoning
- Knowledge graph applications

**Feature Flags**:
- `reasoner-rl` (default) - Enable OWL 2 RL reasoning
- `reasoner-el` - Enable OWL 2 EL reasoning
- `reasoner-rdfs` - Enable RDFS reasoning

**Dependencies**:
- `oxrdf` - RDF data model
- `oxiri` - IRI parsing

**Status**: Version 0.1.0 (early development)

---

## Testing and Fuzzing

### sparql-smith
[![Crates.io](https://img.shields.io/crates/v/sparql-smith.svg)](https://crates.io/crates/sparql-smith)
[![Documentation](https://docs.rs/sparql-smith/badge.svg)](https://docs.rs/sparql-smith)

**Purpose**: Test case generator for SPARQL (fuzzing).

**Use Cases**:
- Fuzzing SPARQL parsers
- Generating test queries
- Finding parser bugs

**Entry Point**: `Query` struct (serializable to SPARQL)

**Note**: Generated queries not always valid; variable scopes not fully handled yet

---

## Language Bindings

### oxigraph-cli
[![Crates.io](https://img.shields.io/crates/v/oxigraph-cli.svg)](https://crates.io/crates/oxigraph-cli)

**Purpose**: Command-line tool and HTTP SPARQL server.

**Use Cases**:
- Running standalone SPARQL endpoint
- CLI database operations (load, dump, query, update)
- Format conversion
- Database backup

**Commands**:
- `serve` - HTTP server (read-write)
- `serve-read-only` - HTTP server (read-only)
- `load` - Load RDF files
- `dump` - Export database
- `query` - Execute SPARQL queries
- `update` - Execute SPARQL updates
- `backup` - Create database backup
- `optimize` - Optimize storage
- `convert` - Convert RDF formats

---

### pyoxigraph
[![PyPI](https://img.shields.io/pypi/v/pyoxigraph)](https://pypi.org/project/pyoxigraph/)
[![Documentation](https://img.shields.io/badge/docs-readthedocs-blue)](https://pyoxigraph.readthedocs.io/)

**Purpose**: Python bindings for Oxigraph.

**Use Cases**:
- RDF processing in Python
- SPARQL queries from Python
- Embedded graph database in Python applications

**Technology**: Built with [PyO3](https://pyo3.rs/) and [maturin](https://www.maturin.rs/)

**Location**: `/python` directory

---

### oxigraph-js
[![npm](https://img.shields.io/npm/v/oxigraph)](https://www.npmjs.com/package/oxigraph)

**Purpose**: JavaScript/WebAssembly bindings for Oxigraph.

**Use Cases**:
- RDF processing in browsers
- SPARQL in Node.js
- Client-side graph database

**Technology**: Built with [wasm-bindgen](https://rustwasm.github.io/wasm-bindgen/)

**Location**: `/js` directory

**Feature Flags**:
- `geosparql` - Enable GeoSPARQL functions
- `rdf-12` - Enable RDF 1.2 features

---

## Dependency Graph

```
oxigraph
├── oxrdf
├── oxrdfio
│   ├── oxrdf
│   ├── oxttl
│   │   └── oxrdf
│   ├── oxrdfxml
│   │   └── oxrdf
│   └── oxjsonld
│       └── oxrdf
├── spargebra
│   └── oxrdf
├── spareval
│   ├── spargebra
│   └── sparopt
│       └── spargebra
├── sparopt
│   └── spargebra
├── sparesults
│   └── oxrdf
└── oxsdatatypes

Extensions:
sparshacl
└── oxrdf

spargeo
└── oxrdf

oxowl
└── oxrdf

Testing:
sparql-smith
└── (arbitrary)
```

---

## Version Information

All crates follow semantic versioning. As of the current workspace version:

- Workspace Version: 0.5.3
- Edition: 2024
- Minimum Rust Version: 1.87
- License: MIT OR Apache-2.0

See individual crate documentation on [docs.rs](https://docs.rs/) for specific API details and examples.
