# Explanations

**Understanding-oriented discussions that illuminate concepts and design decisions**

Explanations provide conceptual clarity about Oxigraph, RDF, SPARQL, and graph databases. Unlike tutorials (which teach) or how-to guides (which solve problems), explanations help you understand the "why" behind the "what" and "how." They explore topics in depth, discuss alternatives, and provide context.

## What to Expect

- **Conceptual depth** - Thorough exploration of ideas and principles
- **Context and background** - Historical perspective, design rationale
- **Multiple perspectives** - Trade-offs, alternatives, implications
- **Big picture thinking** - How components fit together
- **No instructions** - Discussion and illumination, not step-by-step tasks

## Who Should Use Explanations?

- You want to understand why things work the way they do
- You need to make informed architectural decisions
- You're curious about the design philosophy
- You want deeper knowledge beyond just using the API
- You're evaluating Oxigraph for your use case

---

## Core Concepts

### Graph Databases

- **[What is a Graph Database?](./concepts/graph-database.md)**
  - Property graphs vs RDF graphs
  - When to use graph databases
  - Graph database patterns

- **[RDF Data Model](./concepts/rdf-model.md)**
  - Triples and quads explained
  - Named nodes, blank nodes, literals
  - Why RDF uses URIs/IRIs
  - The open world assumption

- **[Named Graphs](./concepts/named-graphs.md)**
  - What are named graphs?
  - Use cases: provenance, versioning, access control
  - Default graph vs named graphs
  - Quads as the fundamental unit

- **[RDF vs Property Graphs](./concepts/rdf-vs-property-graphs.md)**
  - Comparing data models
  - Strengths and weaknesses
  - Migration considerations

### SPARQL

- **[What is SPARQL?](./concepts/sparql-intro.md)**
  - SQL for graphs
  - Pattern matching philosophy
  - Query forms and their purposes

- **[SPARQL Graph Patterns](./concepts/graph-patterns.md)**
  - How pattern matching works
  - Variable bindings and solutions
  - Optional patterns and union
  - The difference between AND and OPTIONAL

- **[SPARQL Property Paths](./concepts/property-paths.md)**
  - Transitive closures explained
  - Path expressions vs explicit patterns
  - Performance implications
  - When to use property paths

- **[SPARQL Query Evaluation](./concepts/query-evaluation.md)**
  - How queries are executed
  - Join algorithms
  - Filter pushdown
  - Index selection

- **[SPARQL vs SQL](./concepts/sparql-vs-sql.md)**
  - Similarities and differences
  - When to use each
  - Impedance mismatch

### Linked Data and Semantic Web

- **[Linked Data Principles](./concepts/linked-data.md)**
  - Tim Berners-Lee's four principles
  - URIs as identifiers
  - Dereferencing and content negotiation
  - The web of data

- **[Semantic Web Stack](./concepts/semantic-web-stack.md)**
  - RDF, RDFS, OWL, SPARQL layers
  - Where Oxigraph fits
  - Standards landscape

- **[Vocabularies and Ontologies](./concepts/vocabularies.md)**
  - What are vocabularies?
  - Schema.org, Dublin Core, FOAF
  - Ontology design patterns
  - Reusing vs creating vocabularies

- **[IRIs and Namespaces](./concepts/iris-namespaces.md)**
  - Why IRIs instead of simple strings?
  - Namespace conventions
  - Cool URIs don't change
  - Hash vs slash URIs

---

## Oxigraph Architecture

### Design Philosophy

- **[Oxigraph's Design Goals](./architecture/design-goals.md)**
  - Standards compliance first
  - Performance through simplicity
  - Embeddability and portability
  - Zero-configuration usability

- **[Why Rust?](./architecture/why-rust.md)**
  - Memory safety without garbage collection
  - Zero-cost abstractions
  - WebAssembly compilation
  - Ecosystem and tooling

- **[Modular Architecture](./architecture/modularity.md)**
  - Crate organization philosophy
  - Separation of concerns
  - Dependency graph
  - Public vs internal APIs

### Storage Engine

- **[Storage Architecture](./architecture/storage.md)**
  - Why RocksDB?
  - Key-value store for graphs
  - Index design (SPO, POS, OSP)
  - Trade-offs and alternatives

- **[Index Strategies](./architecture/indexes.md)**
  - Why three indexes?
  - Query pattern to index mapping
  - Index selection algorithm
  - Space vs time trade-offs

- **[Transaction Model](./architecture/transactions.md)**
  - ACID properties in Oxigraph
  - Isolation levels
  - Concurrency control
  - RocksDB transactions

- **[Memory vs Persistent Storage](./architecture/memory-vs-persistent.md)**
  - When to use MemoryStore
  - Performance characteristics
  - Memory overhead
  - Hybrid approaches

### Query Engine

- **[Query Optimization](./architecture/query-optimization.md)**
  - Algebraic optimization
  - Join order selection
  - Filter pushdown
  - Statistics and cost estimation

- **[Query Evaluation Pipeline](./architecture/query-pipeline.md)**
  - Parsing to algebra
  - Optimization passes
  - Physical execution
  - Result streaming

- **[Join Algorithms](./architecture/joins.md)**
  - Hash joins vs merge joins
  - Index nested loops
  - When each is used
  - Performance implications

### I/O and Parsing

- **[Parser Architecture](./architecture/parsers.md)**
  - Streaming parsers
  - Error handling and recovery
  - Memory efficiency
  - Format-specific implementations

- **[Serialization Strategies](./architecture/serialization.md)**
  - Streaming output
  - Pretty printing vs compact
  - Format selection
  - Performance considerations

---

## Data Modeling

### RDF Modeling Patterns

- **[Modeling Entities and Relationships](./modeling/entities-relationships.md)**
  - Subject-predicate-object thinking
  - Reification alternatives
  - N-ary relationships
  - Temporal modeling

- **[Schema Design](./modeling/schema-design.md)**
  - Top-down vs bottom-up
  - When to use classes
  - Property vs class hierarchies
  - Open world design

- **[Identifier Strategies](./modeling/identifiers.md)**
  - Choosing IRI schemes
  - Blank nodes vs named nodes
  - Stable identifiers
  - Skolemization

- **[Representing Time and Change](./modeling/temporal.md)**
  - Valid time vs transaction time
  - Versioning strategies
  - Event sourcing with RDF
  - Temporal queries

- **[Multilingual Data](./modeling/multilingual.md)**
  - Language tags
  - Translation patterns
  - Fallback strategies
  - Direction tags (RDF 1.2)

### Validation and Quality

- **[SHACL Explained](./modeling/shacl.md)**
  - Shapes vs ontologies
  - Open vs closed world validation
  - When to validate
  - SHACL vs ShEx

- **[Data Quality Strategies](./modeling/data-quality.md)**
  - Validation at ingestion
  - Continuous quality monitoring
  - Constraint checking
  - Provenance tracking

---

## Performance

### Understanding Performance

- **[Graph Database Performance](./performance/graph-performance.md)**
  - What makes graphs fast (or slow)
  - Query complexity classes
  - Index coverage
  - Data distribution effects

- **[Query Performance Factors](./performance/query-factors.md)**
  - Selectivity and cardinality
  - Join ordering impact
  - Filter placement
  - Property path costs

- **[Bulk Loading Performance](./performance/bulk-loading.md)**
  - Why bulk loading is faster
  - Index maintenance overhead
  - Write amplification
  - Optimal batch sizes

- **[Memory Usage Patterns](./performance/memory-patterns.md)**
  - Memory store overhead
  - Query execution memory
  - Caching strategies
  - Memory vs disk trade-offs

### Scaling

- **[Vertical Scaling](./performance/vertical-scaling.md)**
  - Hardware characteristics
  - SSD vs HDD
  - RAM sizing
  - CPU utilization

- **[Horizontal Scaling Considerations](./performance/horizontal-scaling.md)**
  - Read replicas
  - Sharding challenges
  - Distributed queries
  - Consistency trade-offs

- **[Performance Tuning Philosophy](./performance/tuning-philosophy.md)**
  - Measure before optimizing
  - 80/20 rule for queries
  - Premature optimization
  - Profiling tools

---

## Integration and Ecosystem

### Language Bindings

- **[Why Multiple Language Bindings?](./integration/language-bindings.md)**
  - Rust-first development
  - Python scientific ecosystem
  - JavaScript ubiquity
  - FFI strategies

- **[Python Bindings Architecture](./integration/python-architecture.md)**
  - PyO3 bridge
  - GIL considerations
  - Memory management
  - Performance characteristics

- **[JavaScript/WASM Architecture](./integration/javascript-architecture.md)**
  - wasm-bindgen bridge
  - Browser vs Node.js
  - Memory sharing
  - Performance trade-offs
  - RDF/JS compatibility

### Standards Compliance

- **[W3C Standards Philosophy](./integration/standards-philosophy.md)**
  - Why standards matter
  - Interoperability benefits
  - Test-driven compliance
  - Extension points

- **[SPARQL 1.1 Implementation](./integration/sparql-implementation.md)**
  - Specification coverage
  - Conformance testing
  - Extension functions
  - Known limitations

- **[RDF 1.1 vs 1.2](./integration/rdf-versions.md)**
  - What changed in RDF 1.2
  - Directional language tags
  - Backward compatibility
  - Migration path

### Ecosystem

- **[Oxigraph in the RDF Ecosystem](./ecosystem/rdf-ecosystem.md)**
  - Comparison with other stores (Jena, Virtuoso, GraphDB, Blazegraph)
  - Use case fit
  - Community and adoption
  - Contribution opportunities

- **[SPARQL Endpoints and Linked Data](./ecosystem/linked-data-platforms.md)**
  - Public SPARQL endpoints
  - Linked Open Data cloud
  - Wikidata and DBpedia
  - Federation patterns

- **[Knowledge Graphs](./ecosystem/knowledge-graphs.md)**
  - Enterprise knowledge graphs
  - Open knowledge graphs
  - Graph databases vs knowledge graphs
  - Oxigraph's role

---

## Use Cases

### When to Use Oxigraph

- **[Embedded Applications](./use-cases/embedded.md)**
  - Mobile apps
  - Desktop applications
  - IoT devices
  - Edge computing

- **[Research and Academia](./use-cases/research.md)**
  - Scientific data management
  - Publication metadata
  - Experimental results
  - Reproducibility

- **[Web Applications](./use-cases/web-apps.md)**
  - Content management
  - Social networks
  - Recommendation engines
  - Semantic search

- **[Data Integration](./use-cases/data-integration.md)**
  - ETL pipelines
  - Data lakes
  - Master data management
  - Schema mapping

### When NOT to Use Oxigraph

- **[Alternative Technologies](./use-cases/alternatives.md)**
  - When SQL is better
  - When property graphs fit better
  - When document stores are appropriate
  - When to use enterprise triple stores

- **[Scalability Limits](./use-cases/scalability-limits.md)**
  - Single-node constraints
  - Dataset size considerations
  - Query complexity limits
  - Concurrency bottlenecks

---

## Security and Trust

- **[RDF and Security](./security/rdf-security.md)**
  - Authentication vs authorization
  - Graph-level access control
  - Triple-level security challenges
  - Inference and privacy

- **[SPARQL Injection](./security/sparql-injection.md)**
  - Attack vectors
  - Parameterized queries
  - Input sanitization
  - Best practices

- **[Trusted Data](./security/trust.md)**
  - Provenance tracking
  - Digital signatures
  - Verifiable credentials
  - Trust networks

---

## Advanced Topics

### Reasoning and Inference

- **[RDFS Inference](./advanced/rdfs-inference.md)**
  - Subclass and subproperty
  - Domain and range
  - Materialization vs query rewriting
  - Oxigraph's stance on inference

- **[OWL and Description Logics](./advanced/owl.md)**
  - OWL profiles
  - Reasoning complexity
  - When to use reasoners
  - Integration with Oxigraph

### Extensions

- **[GeoSPARQL](./advanced/geosparql.md)**
  - Spatial queries
  - Geometry functions
  - Use cases
  - Implementation notes

- **[Custom SPARQL Functions](./advanced/custom-functions.md)**
  - Why extend SPARQL?
  - Function implementation (Rust only)
  - Namespace conventions
  - Portability considerations

- **[Full-Text Search](./advanced/full-text-search.md)**
  - Limitations of SPARQL FILTER
  - External indexing strategies
  - Integration patterns
  - Future directions

### Comparative Analysis

- **[Oxigraph vs SQLite](./comparison/sqlite.md)**
  - Embedded database patterns
  - Use case overlap
  - Performance characteristics
  - When to choose each

- **[Oxigraph vs Neo4j](./comparison/neo4j.md)**
  - RDF vs property graph models
  - Query language differences
  - Ecosystem and tooling
  - Migration paths

- **[Oxigraph vs Apache Jena](./comparison/jena.md)**
  - Java vs Rust implementation
  - Feature comparison
  - Performance profiles
  - Community and maturity

---

## History and Evolution

- **[Oxigraph's History](./history/project-history.md)**
  - Origins and motivation
  - Major milestones
  - Design evolution
  - Future roadmap

- **[RDF History](./history/rdf-history.md)**
  - From Semantic Web vision to practical tools
  - Evolution of standards
  - Success stories and lessons learned
  - Where we are today

- **[SPARQL Evolution](./history/sparql-evolution.md)**
  - SPARQL 1.0 to 1.1
  - What didn't make it into the spec
  - SPARQL 1.2 and beyond
  - Alternative query languages

---

## Philosophy and Best Practices

- **[The Graph Way of Thinking](./philosophy/graph-thinking.md)**
  - Relationships as first-class citizens
  - Schema flexibility
  - Emergent structure
  - Network effects

- **[Semantic Precision](./philosophy/semantic-precision.md)**
  - Precise vs fuzzy semantics
  - Vocabulary reuse
  - Semantic interoperability
  - The cost of precision

- **[Open World vs Closed World](./philosophy/open-vs-closed.md)**
  - Assumptions and implications
  - Validation strategies
  - When to close the world
  - Hybrid approaches

- **[Developer Experience Philosophy](./philosophy/developer-experience.md)**
  - API design principles
  - Error messages that help
  - Documentation as code
  - Community building

---

## Research and Future Directions

- **[Query Optimization Research](./research/query-optimization.md)**
  - Current challenges
  - Machine learning approaches
  - Adaptive execution
  - Research papers

- **[Storage Innovation](./research/storage-innovation.md)**
  - Column-oriented storage
  - Compression techniques
  - GPU acceleration
  - Persistent memory

- **[Distributed Graph Databases](./research/distributed-graphs.md)**
  - Partitioning strategies
  - Distributed join algorithms
  - Consistency models
  - Research directions

- **[Future of RDF and SPARQL](./research/future.md)**
  - Standardization efforts
  - Community initiatives
  - Emerging use cases
  - Convergence with property graphs

---

## Next Steps

- **Want to learn by doing?** Start with the [Tutorials](../tutorials/)
- **Need to solve a problem?** Check the [How-To Guides](../how-to/)
- **Looking for specifications?** Browse the [Reference](../reference/)

---

## Contributing

Help deepen our explanations:

- [Suggest topics](https://github.com/oxigraph/oxigraph/issues) that need explanation
- [Write explanations](https://github.com/oxigraph/oxigraph/pulls) for complex topics
- [Share your insights](https://github.com/oxigraph/oxigraph/discussions) from using Oxigraph

Explanations should be:
- Conceptual and illuminating
- Balanced and fair in presenting alternatives
- Well-researched and accurate
- Accessible but not oversimplified
- Connected to related topics
