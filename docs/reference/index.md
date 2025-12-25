# Reference

**Information-oriented technical documentation for lookup and specification**

Reference documentation provides precise, technical descriptions of Oxigraph's APIs, configurations, and specifications. This is where you come when you know what you're looking for and need exact details, parameters, return types, or specifications.

## What to Expect

- **Technical accuracy** - Precise, authoritative information
- **Comprehensive coverage** - Complete API documentation
- **Structured information** - Organized for quick lookup
- **Specification details** - Standards compliance, formats, protocols
- **No explanation** - Facts and specifications, not conceptual discussion

## Who Should Use Reference Documentation?

- You know what feature or API you need
- You need exact syntax, parameters, or return types
- You're looking up configuration options
- You need to verify standards compliance
- You're integrating Oxigraph with other systems

---

## API Documentation

### Rust API

Complete API documentation for all Rust crates:

#### Core Crates

- **[oxigraph](./rust/oxigraph/)** - Main database library
  - [Store](./rust/oxigraph/store.md) - Persistent RocksDB-backed store
  - [MemoryStore](./rust/oxigraph/memory-store.md) - In-memory store
  - [Query API](./rust/oxigraph/query.md) - SPARQL query execution
  - [Update API](./rust/oxigraph/update.md) - SPARQL update execution
  - [Transaction API](./rust/oxigraph/transaction.md) - Transactional operations
  - [Bulk Loader](./rust/oxigraph/bulk-loader.md) - Efficient bulk import

- **[oxrdf](./rust/oxrdf/)** - RDF data model
  - [NamedNode](./rust/oxrdf/named-node.md) - IRI references
  - [BlankNode](./rust/oxrdf/blank-node.md) - Blank nodes with unique IDs
  - [Literal](./rust/oxrdf/literal.md) - Typed and language-tagged literals
  - [Triple](./rust/oxrdf/triple.md) - Subject-predicate-object statements
  - [Quad](./rust/oxrdf/quad.md) - Triples with graph context
  - [Graph](./rust/oxrdf/graph.md) - In-memory triple collection
  - [Dataset](./rust/oxrdf/dataset.md) - In-memory quad collection

#### I/O Crates

- **[oxrdfio](./rust/oxrdfio/)** - RDF parsing and serialization
  - [RdfFormat](./rust/oxrdfio/format.md) - Format enumeration
  - [RdfParser](./rust/oxrdfio/parser.md) - Parsing interface
  - [RdfSerializer](./rust/oxrdfio/serializer.md) - Serialization interface

- **[oxttl](./rust/oxttl/)** - Turtle family parsers
  - [TurtleParser](./rust/oxttl/turtle-parser.md) - Turtle (.ttl)
  - [TriGParser](./rust/oxttl/trig-parser.md) - TriG (.trig)
  - [N-TriplesParser](./rust/oxttl/ntriples-parser.md) - N-Triples (.nt)
  - [N-QuadsParser](./rust/oxttl/nquads-parser.md) - N-Quads (.nq)

- **[oxrdfxml](./rust/oxrdfxml/)** - RDF/XML parser
  - [RdfXmlParser](./rust/oxrdfxml/parser.md) - RDF/XML (.rdf, .xml)

- **[oxjsonld](./rust/oxjsonld/)** - JSON-LD support
  - [JsonLdParser](./rust/oxjsonld/parser.md) - JSON-LD parsing
  - [JsonLdSerializer](./rust/oxjsonld/serializer.md) - JSON-LD serialization

#### SPARQL Crates

- **[spargebra](./rust/spargebra/)** - SPARQL algebra
  - [Query](./rust/spargebra/query.md) - Query AST
  - [Update](./rust/spargebra/update.md) - Update AST
  - [Algebra](./rust/spargebra/algebra.md) - Algebraic representation

- **[spareval](./rust/spareval/)** - Query evaluation
  - [QueryEvaluator](./rust/spareval/evaluator.md) - Evaluation engine
  - [Functions](./rust/spareval/functions.md) - Built-in SPARQL functions

- **[sparopt](./rust/sparopt/)** - Query optimization
  - [Optimizer](./rust/sparopt/optimizer.md) - Query optimizer

- **[sparesults](./rust/sparesults/)** - Results formats
  - [QueryResults](./rust/sparesults/results.md) - Results enumeration
  - [QuerySolution](./rust/sparesults/solution.md) - Variable bindings
  - [ResultsFormat](./rust/sparesults/format.md) - JSON, XML, CSV, TSV

#### Extension Crates

- **[sparshacl](./rust/sparshacl/)** - SHACL validation
  - [ShaclValidator](./rust/sparshacl/validator.md) - Validation engine
  - [ValidationReport](./rust/sparshacl/report.md) - Validation results

- **[spargeo](./rust/spargeo/)** - GeoSPARQL
  - [Geometry Functions](./rust/spargeo/functions.md) - Spatial functions

- **[oxsdatatypes](./rust/oxsdatatypes/)** - XSD datatypes
  - [Datatype Implementations](./rust/oxsdatatypes/types.md) - XSD type system

#### Generated Documentation

- **[docs.rs/oxigraph](https://docs.rs/oxigraph)** - Complete Rust API docs
- **[docs.rs/oxrdf](https://docs.rs/oxrdf)** - RDF model docs
- **[docs.rs/spargebra](https://docs.rs/spargebra)** - SPARQL algebra docs

### Python API

Complete API documentation for PyOxigraph:

#### Core Classes

- **[Store](./python/store.md)** - Graph database operations
  - Methods: `add`, `remove`, `load`, `dump`, `query`, `update`
  - Properties: `__len__`, `__iter__`, `__contains__`

- **[MemoryStore](./python/memory-store.md)** - In-memory variant
  - Same interface as Store without persistence

#### RDF Model

- **[NamedNode](./python/named-node.md)** - IRI references
  - Constructor: `NamedNode(value: str)`
  - Properties: `value`

- **[BlankNode](./python/blank-node.md)** - Blank nodes
  - Constructor: `BlankNode(value: str = None)`
  - Properties: `value`

- **[Literal](./python/literal.md)** - Literal values
  - Constructors: typed, language-tagged, simple
  - Properties: `value`, `datatype`, `language`

- **[DefaultGraph](./python/default-graph.md)** - Default graph singleton

- **[Triple](./python/triple.md)** - RDF triples
  - Constructor: `Triple(subject, predicate, object)`

- **[Quad](./python/quad.md)** - RDF quads
  - Constructor: `Quad(subject, predicate, object, graph=None)`

#### Query Results

- **[QuerySolutions](./python/query-solutions.md)** - SELECT results iterator
  - Methods: `__iter__`, `__next__`
  - Properties: `variables`

- **[QuerySolution](./python/query-solution.md)** - Single solution
  - Methods: `__getitem__`, `get`
  - Properties: mapping interface

- **[QueryTriples](./python/query-triples.md)** - CONSTRUCT/DESCRIBE results

- **[QueryBoolean](./python/query-boolean.md)** - ASK results

#### Generated Documentation

- **[PyPI Documentation](https://pypi.org/project/pyoxigraph/)** - Package info
- **[Python API Reference](./python/api.md)** - Complete reference

### JavaScript API

Complete API documentation for the oxigraph npm package:

#### Core Classes

- **[Store](./javascript/store.md)** - Graph database with Array-like API
  - Methods: `add`, `delete`, `has`, `match`, `query`, `update`, `load`, `dump`
  - Collection methods: `forEach`, `map`, `filter`, `reduce`, `find`, `some`, `every`
  - Properties: `size`, `[Symbol.iterator]`

- **[MemoryStore](./javascript/memory-store.md)** - In-memory variant

- **[Dataset](./javascript/dataset.md)** - In-memory quad collection
  - Same Array-like interface as Store

#### RDF Model

- **[NamedNode](./javascript/named-node.md)** - IRI references
  - Constructor: `new NamedNode(value: string)`
  - Properties: `value`, `termType`

- **[BlankNode](./javascript/blank-node.md)** - Blank nodes
  - Constructor: `new BlankNode(value?: string)`
  - Properties: `value`, `termType`

- **[Literal](./javascript/literal.md)** - Literal values
  - Static methods: `Literal.typed()`, `Literal.languageTagged()`
  - Properties: `value`, `datatype`, `language`, `termType`

- **[DefaultGraph](./javascript/default-graph.md)** - Default graph singleton

- **[Triple](./javascript/triple.md)** - RDF triples
  - Constructor: `new Triple(subject, predicate, object)`

- **[Quad](./javascript/quad.md)** - RDF quads
  - Constructor: `new Quad(subject, predicate, object, graph?)`

#### Query Results

- **[QueryResultsIterator](./javascript/query-results.md)** - Query results
  - Implements iterator protocol
  - Methods: `next()`, `[Symbol.iterator]()`

- **[QuerySolution](./javascript/query-solution.md)** - Variable bindings
  - Implements Map-like interface
  - Methods: `get()`, `has()`, `keys()`, `values()`, `entries()`

#### Formats

- **[RdfFormat](./javascript/rdf-format.md)** - RDF format constants
  - Static properties: `TURTLE`, `N_TRIPLES`, `N_QUADS`, `TRIG`, `RDF_XML`, `JSON_LD`

- **[QueryResultsFormat](./javascript/results-format.md)** - Results format constants
  - Static properties: `JSON`, `XML`, `CSV`, `TSV`

#### SHACL

- **[ShaclValidator](./javascript/shacl-validator.md)** - Validation
  - Constructor: `new ShaclValidator(shapes)`
  - Methods: `validate(data)`

#### TypeScript Definitions

- **[index.d.ts](./javascript/typescript.md)** - Complete TypeScript definitions
- **[NPM Documentation](https://www.npmjs.com/package/oxigraph)** - Package info

---

## Standards Compliance

### W3C Standards

- **[RDF 1.1 Compliance](./standards/rdf.md)**
  - RDF Concepts, Turtle, N-Triples, N-Quads, TriG, RDF/XML
  - Supported datatypes and language tags

- **[SPARQL 1.1 Compliance](./standards/sparql.md)**
  - Query Language, Update, Protocol, Service Description
  - Supported features and extensions
  - Known limitations

- **[SPARQL Results Formats](./standards/results-formats.md)**
  - JSON, XML, CSV, TSV specifications

- **[SHACL Compliance](./standards/shacl.md)**
  - Supported constraint components
  - Validation algorithm

- **[JSON-LD Compliance](./standards/jsonld.md)**
  - Version support, context handling

- **[SPARQL Graph Store Protocol](./standards/graph-store.md)**
  - RESTful graph operations

### RDF/JS Specification

- **[RDF/JS Data Model](./standards/rdfjs-datamodel.md)** (JavaScript only)
  - Term interfaces, factory functions
  - Compliance with @rdfjs/types

- **[RDF/JS Dataset](./standards/rdfjs-dataset.md)** (JavaScript only)
  - Dataset interface compliance

---

## Configuration

### Oxigraph Server

- **[Command-Line Options](./config/cli-options.md)**
  - `--location`, `--bind`, `--cors`, etc.
  - Complete option reference

- **[Environment Variables](./config/environment.md)**
  - Configuration via env vars

- **[Server Configuration File](./config/config-file.md)**
  - TOML/YAML configuration (if supported)

### Store Configuration

- **[RocksDB Options](./config/rocksdb.md)**
  - Memory limits, cache sizes, compaction
  - Performance tuning parameters

- **[Memory Store Options](./config/memory-store.md)**
  - Initial capacity, growth strategies

### Feature Flags

- **[Cargo Features](./config/cargo-features.md)** (Rust)
  - `rocksdb`, `http-client`, etc.

- **[Build Features](./config/build-features.md)** (JavaScript)
  - `geosparql`, `rdf-12`

---

## File Formats

### RDF Formats

- **[Turtle (.ttl)](./formats/turtle.md)**
  - Syntax specification, media type, extensions

- **[N-Triples (.nt)](./formats/ntriples.md)**
  - Line-based format, escaping rules

- **[N-Quads (.nq)](./formats/nquads.md)**
  - Quad serialization format

- **[TriG (.trig)](./formats/trig.md)**
  - Named graph serialization

- **[RDF/XML (.rdf, .xml)](./formats/rdfxml.md)**
  - XML-based RDF format

- **[JSON-LD (.jsonld)](./formats/jsonld.md)**
  - JSON-based RDF format

### Query Results Formats

- **[SPARQL Query Results JSON](./formats/results-json.md)**
  - JSON structure specification

- **[SPARQL Query Results XML](./formats/results-xml.md)**
  - XML schema

- **[SPARQL Query Results CSV](./formats/results-csv.md)**
  - CSV format specification

- **[SPARQL Query Results TSV](./formats/results-tsv.md)**
  - Tab-separated format

---

## SPARQL Reference

### Query Forms

- **[SELECT](./sparql/select.md)**
  - Syntax, projection, modifiers (DISTINCT, REDUCED, LIMIT, OFFSET, ORDER BY)

- **[CONSTRUCT](./sparql/construct.md)**
  - Template syntax, graph construction

- **[ASK](./sparql/ask.md)**
  - Boolean queries

- **[DESCRIBE](./sparql/describe.md)**
  - Resource description

### Graph Patterns

- **[Basic Graph Patterns](./sparql/bgp.md)**
  - Triple patterns, variable bindings

- **[Group Graph Patterns](./sparql/group-patterns.md)**
  - Nested patterns, scope

- **[Optional Patterns](./sparql/optional.md)**
  - OPTIONAL keyword

- **[Union Patterns](./sparql/union.md)**
  - UNION alternatives

- **[Named Graphs](./sparql/graph.md)**
  - GRAPH keyword, FROM, FROM NAMED

### Filters and Expressions

- **[Filter Expressions](./sparql/filter.md)**
  - FILTER syntax, operators

- **[Built-in Functions](./sparql/functions.md)**
  - Complete function reference: str(), lang(), datatype(), etc.

- **[Operators](./sparql/operators.md)**
  - Arithmetic, comparison, logical operators

### Solution Modifiers

- **[ORDER BY](./sparql/order-by.md)**
  - Sorting results

- **[LIMIT and OFFSET](./sparql/limit-offset.md)**
  - Result pagination

- **[DISTINCT and REDUCED](./sparql/distinct-reduced.md)**
  - Duplicate elimination

### Aggregates

- **[Aggregate Functions](./sparql/aggregates.md)**
  - COUNT, SUM, AVG, MIN, MAX, GROUP_CONCAT, SAMPLE

- **[GROUP BY](./sparql/group-by.md)**
  - Grouping solutions

- **[HAVING](./sparql/having.md)**
  - Filtering groups

### Property Paths

- **[Property Path Syntax](./sparql/property-paths.md)**
  - `^`, `|`, `/`, `*`, `+`, `?`, `{n,m}`

### Updates

- **[INSERT DATA](./sparql/insert-data.md)**
  - Adding triples/quads

- **[DELETE DATA](./sparql/delete-data.md)**
  - Removing triples/quads

- **[DELETE/INSERT WHERE](./sparql/delete-insert.md)**
  - Conditional modifications

- **[LOAD](./sparql/load.md)**
  - Loading from URLs

- **[CLEAR](./sparql/clear.md)**
  - Clearing graphs

- **[DROP](./sparql/drop.md)**
  - Dropping graphs

### Federated Queries

- **[SERVICE](./sparql/service.md)**
  - Querying remote endpoints

---

## Error Codes and Messages

- **[Rust Error Types](./errors/rust.md)**
  - Complete error enumeration, causes

- **[Python Exceptions](./errors/python.md)**
  - Exception hierarchy, error messages

- **[JavaScript Errors](./errors/javascript.md)**
  - Error types, error codes

- **[HTTP Status Codes](./errors/http.md)** (Server)
  - 200, 400, 404, 500, etc. meanings

---

## Performance Characteristics

- **[Time Complexity](./performance/time-complexity.md)**
  - Query patterns, operations

- **[Space Complexity](./performance/space-complexity.md)**
  - Storage overhead, index sizes

- **[Index Architecture](./performance/indexes.md)**
  - SPO, POS, OSP index descriptions

---

## Protocol Specifications

- **[SPARQL Protocol](./protocols/sparql-protocol.md)**
  - HTTP endpoints, content negotiation

- **[SPARQL Graph Store Protocol](./protocols/graph-store.md)**
  - RESTful graph operations

- **[Content Negotiation](./protocols/content-negotiation.md)**
  - Supported media types, quality values

---

## Vocabulary Support

- **[XSD Datatypes](./vocabularies/xsd.md)**
  - Complete list of supported XML Schema datatypes

- **[RDF Vocabulary](./vocabularies/rdf.md)**
  - Built-in RDF terms

- **[RDFS Vocabulary](./vocabularies/rdfs.md)**
  - RDFS inference (if supported)

- **[OWL Vocabulary](./vocabularies/owl.md)**
  - OWL support level

---

## Limits and Constraints

- **[Size Limits](./limits/size-limits.md)**
  - Maximum triple count, literal sizes, IRI lengths

- **[Query Limits](./limits/query-limits.md)**
  - Maximum query complexity, timeout defaults

- **[Resource Limits](./limits/resource-limits.md)**
  - Memory usage, file handles, connections

---

## Changelog

- **[Version History](./changelog.md)**
  - Release notes for all versions

- **[Breaking Changes](./breaking-changes.md)**
  - Migration guides between major versions

- **[Deprecations](./deprecations.md)**
  - Deprecated APIs and alternatives

---

## Next Steps

- **Learning how to use these APIs?** See the [Tutorials](../tutorials/)
- **Solving a specific problem?** Check the [How-To Guides](../how-to/)
- **Want conceptual understanding?** Read the [Explanations](../explanation/)

---

## Contributing

Help improve the reference documentation:

- [Report inaccuracies](https://github.com/oxigraph/oxigraph/issues)
- [Submit corrections](https://github.com/oxigraph/oxigraph/pulls)
- [Generate API docs](../contributing/api-docs.md) - For Rust crate maintainers

Reference documentation should be:
- Technically accurate and precise
- Complete and comprehensive
- Well-structured for quick lookup
- Version-specific when relevant
