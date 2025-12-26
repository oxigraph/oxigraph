# Integration Scenarios

This section provides complete, real-world integration scenarios for Oxigraph. Each scenario includes code examples, deployment instructions, and best practices.

## Available Scenarios

### Migration Guides

- **[Migrate from Apache Jena](migrate-from-jena.md)** - Complete guide for migrating from Apache Jena to Oxigraph
  - API mapping table
  - Data migration steps
  - Query compatibility notes
  - Performance optimization tips

- **[Migrate from RDFLib](migrate-from-rdflib.md)** - Transition from Python RDFLib to pyoxigraph
  - API equivalence guide
  - Plugin replacement strategies
  - Namespace handling differences
  - Complete migration example

### Integration Patterns

- **[Integrate with Wikidata](integrate-wikidata.md)** - Query and cache Wikidata effectively
  - SPARQL federation patterns
  - Local caching strategies
  - Rate limiting and best practices
  - Complete working examples

- **[Build a Knowledge Graph](build-knowledge-graph.md)** - End-to-end knowledge graph construction
  - Ontology design
  - Multi-source data ingestion
  - Entity linking and reconciliation
  - Query interface design

### Architecture Patterns

- **[Microservices Architecture](microservices.md)** - Deploy Oxigraph in modern microservices
  - Service discovery patterns
  - Event-driven updates
  - API gateway integration
  - Docker Compose examples

## Scenario Selection Guide

Choose the right scenario based on your needs:

| Your Situation | Recommended Scenario |
|----------------|---------------------|
| Currently using Apache Jena (Java) | [Migrate from Jena](migrate-from-jena.md) |
| Currently using RDFLib (Python) | [Migrate from RDFLib](migrate-from-rdflib.md) |
| Need to query Wikidata or DBpedia | [Wikidata Integration](integrate-wikidata.md) |
| Building a new knowledge graph | [Build Knowledge Graph](build-knowledge-graph.md) |
| Need distributed/scalable architecture | [Microservices](microservices.md) |

## Common Patterns Across Scenarios

### Data Loading Pattern

All scenarios follow a similar data loading pattern:

```rust
use oxigraph::store::Store;
use oxigraph::io::RdfFormat;

fn load_data(store: &Store, path: &str) -> Result<(), Box<dyn std::error::Error>> {
    store.bulk_loader().load_from_path(path, RdfFormat::NTriples)?;
    Ok(())
}
```

### Query Execution Pattern

```rust
use oxigraph::sparql::QueryResults;

fn query_data(store: &Store, sparql: &str) -> Result<(), Box<dyn std::error::Error>> {
    if let QueryResults::Solutions(solutions) = store.query(sparql)? {
        for solution in solutions {
            let solution = solution?;
            println!("{:?}", solution);
        }
    }
    Ok(())
}
```

### Transaction Pattern

```rust
fn transactional_update(store: &Store) -> Result<(), Box<dyn std::error::Error>> {
    let mut transaction = store.transaction()?;

    // Perform multiple operations
    transaction.insert(...)?;
    transaction.remove(...)?;

    // Commit atomically
    transaction.commit()?;
    Ok(())
}
```

## Performance Considerations

All scenarios include performance tips:

1. **Bulk Loading**: Use `bulk_loader()` for initial data import
2. **Batch Transactions**: Group related operations in transactions
3. **Query Optimization**: Let the query planner optimize your SPARQL
4. **Index Selection**: Store automatically maintains optimal indexes
5. **Memory Management**: Consider dataset size when choosing Store vs MemoryStore

## Support and Community

- **Documentation**: https://oxigraph.org/
- **GitHub Issues**: https://github.com/oxigraph/oxigraph/issues
- **Discussions**: https://github.com/oxigraph/oxigraph/discussions

## Contributing Scenarios

Have a useful integration scenario? We welcome contributions:

1. Fork the repository
2. Create your scenario in `docs/scenarios/`
3. Follow the existing format
4. Include complete, working code examples
5. Submit a pull request

See [CONTRIBUTING.md](../CONTRIBUTING.md) for details.
