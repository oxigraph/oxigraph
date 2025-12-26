# SDK Integration Patterns

**Production-ready architectural patterns for building applications with Oxigraph**

This section provides battle-tested integration patterns for using Oxigraph in real-world applications. Each pattern addresses common architectural challenges with complete, working implementations across Rust, Python, and JavaScript.

## What Are Integration Patterns?

Integration patterns are reusable solutions to common problems when building applications with Oxigraph. Unlike tutorials that teach basics or how-to guides that solve specific tasks, patterns provide architectural blueprints for structuring your application code.

## Pattern Catalog

### [Repository Pattern](./repository-pattern.md)
**Abstract data access behind a clean interface**

Create a repository layer that abstracts Oxigraph Store operations, making your code easier to test, maintain, and swap implementations.

**Use when:**
- Building applications with complex domain logic
- Need to mock data access for unit testing
- Want to isolate SPARQL from business logic
- Planning to support multiple storage backends

**Key benefits:**
- Clean separation of concerns
- Testable without a real database
- Domain-driven design support
- Easy to add caching or logging

---

### [Event Sourcing](./event-sourcing.md)
**Store events as RDF triples for complete audit trails**

Use RDF to model event streams, enabling temporal queries, audit logging, and state reconstruction from events.

**Use when:**
- Need complete audit trails and compliance
- Want to reconstruct state at any point in time
- Building event-driven architectures
- Require temporal queries (who changed what when)

**Key benefits:**
- Immutable event history
- Time-travel queries
- Natural fit for RDF's graph model
- Regulatory compliance support

---

### [Caching](./caching.md)
**Speed up queries with intelligent result caching**

Implement multi-level caching strategies for SPARQL query results with smart invalidation.

**Use when:**
- Query performance is critical
- Same queries run repeatedly
- Read-heavy workloads
- Need to reduce database load

**Key benefits:**
- Dramatic performance improvements
- Reduced server load
- Flexible invalidation strategies
- Integration with Redis, Memcached, etc.

---

### [Multi-Tenancy](./multi-tenancy.md)
**Isolate data for multiple customers in one database**

Use named graphs to partition data by tenant while maintaining efficient queries and strong isolation.

**Use when:**
- Building SaaS applications
- Need data isolation between customers
- Want to share infrastructure efficiently
- Require tenant-specific queries

**Key benefits:**
- Strong data isolation
- Cost-effective infrastructure
- Tenant-aware query patterns
- Security by design

---

## Pattern Selection Guide

### Decision Matrix

| **Your Need** | **Recommended Pattern** | **Why** |
|---------------|-------------------------|---------|
| Clean architecture | Repository | Separates data access from business logic |
| Audit requirements | Event Sourcing | Immutable event log with full history |
| Performance optimization | Caching | Reduces query latency and database load |
| SaaS application | Multi-Tenancy | Efficient data isolation per customer |
| Complex domain model | Repository | Domain objects separate from storage |
| Compliance/regulations | Event Sourcing | Complete audit trail with temporal queries |
| High read volume | Caching | Serves repeated queries from cache |
| Multiple customers | Multi-Tenancy | Shared infrastructure with isolation |

### Combining Patterns

These patterns work well together:

**Repository + Caching**
```
Application → Repository (with cache layer) → Oxigraph Store
```
- Repository provides clean interface
- Caching handles performance
- Easy to test with mocks

**Multi-Tenancy + Event Sourcing**
```
Events stored in tenant-specific named graphs
```
- Each tenant gets isolated event stream
- Audit trail per customer
- Temporal queries per tenant

**Repository + Multi-Tenancy**
```
Repository wraps tenant-aware queries
```
- Business logic doesn't handle tenant filtering
- Repository automatically scopes to current tenant
- Prevents cross-tenant data leaks

**All Four Combined**
```
Application
    ↓
Repository (abstracts tenant context)
    ↓
Cache Layer (tenant-aware caching)
    ↓
Event Sourcing (events in named graphs)
    ↓
Oxigraph Store
```

---

## Cross-Cutting Concerns

### Error Handling

All patterns should handle common Oxigraph errors:

```rust
// Rust
use oxigraph::store::StorageError;

match store.insert(&quad) {
    Ok(_) => println!("Success"),
    Err(StorageError::Io(e)) => eprintln!("I/O error: {}", e),
    Err(StorageError::Corruption(e)) => eprintln!("Corruption: {}", e),
    Err(e) => eprintln!("Other error: {}", e),
}
```

```python
# Python
from pyoxigraph import Store, StoreError

try:
    store.add(quad)
except OSError as e:
    print(f"I/O error: {e}")
except StoreError as e:
    print(f"Storage error: {e}")
```

```javascript
// JavaScript
try {
    store.add(quad);
} catch (error) {
    if (error.message.includes('I/O')) {
        console.error('I/O error:', error);
    } else {
        console.error('Storage error:', error);
    }
}
```

### Transaction Management

Patterns should use transactions for consistency:

```rust
// Rust - Atomic updates
store.transaction(|transaction| {
    transaction.insert(quad1)?;
    transaction.insert(quad2)?;
    Ok(())
})?;
```

```python
# Python - Context manager
with store.begin_write() as tx:
    tx.add(quad1)
    tx.add(quad2)
    # Commits on exit or rolls back on exception
```

```javascript
// JavaScript - Manual transaction
const tx = store.beginTransaction();
try {
    tx.add(quad1);
    tx.add(quad2);
    tx.commit();
} catch (error) {
    tx.rollback();
    throw error;
}
```

### Logging and Observability

Add logging to patterns for debugging:

```rust
// Rust - tracing
use tracing::{info, debug, error};

debug!("Querying repository with pattern: {}", pattern);
let results = repository.find_all()?;
info!("Found {} results", results.len());
```

```python
# Python - logging
import logging

logger = logging.getLogger(__name__)

logger.debug(f"Querying repository with pattern: {pattern}")
results = repository.find_all()
logger.info(f"Found {len(results)} results")
```

```javascript
// JavaScript - console or logger
console.debug(`Querying repository with pattern: ${pattern}`);
const results = repository.findAll();
console.info(`Found ${results.length} results`);
```

### Performance Monitoring

Track pattern performance:

```rust
// Rust - timing
use std::time::Instant;

let start = Instant::now();
let results = repository.find_all()?;
let duration = start.elapsed();
info!("Query completed in {:?}", duration);
```

```python
# Python - timing
import time

start = time.time()
results = repository.find_all()
duration = time.time() - start
logger.info(f"Query completed in {duration:.3f}s")
```

```javascript
// JavaScript - timing
const start = performance.now();
const results = repository.findAll();
const duration = performance.now() - start;
console.info(`Query completed in ${duration.toFixed(3)}ms`);
```

### Testing Strategy

Each pattern includes testing approaches:

1. **Unit Tests** - Test pattern logic in isolation with mocks
2. **Integration Tests** - Test with real Oxigraph instance
3. **Performance Tests** - Verify pattern performance characteristics
4. **Contract Tests** - Ensure pattern interface contracts

---

## Implementation Guidelines

### Code Organization

Structure your codebase to separate concerns:

```
project/
├── domain/          # Business logic (pattern-agnostic)
├── repositories/    # Repository implementations
├── caching/         # Cache layers
├── events/          # Event sourcing infrastructure
├── tenancy/         # Multi-tenancy utilities
└── storage/         # Oxigraph Store configuration
```

### Configuration Management

Externalize pattern configuration:

```rust
// Rust - config.toml
[store]
path = "/data/oxigraph"

[cache]
enabled = true
ttl_seconds = 300
max_size_mb = 100

[tenancy]
isolation_mode = "named_graphs"
```

```python
# Python - config.yaml
store:
  path: /data/oxigraph

cache:
  enabled: true
  ttl_seconds: 300
  max_size_mb: 100

tenancy:
  isolation_mode: named_graphs
```

```javascript
// JavaScript - config.json
{
  "store": {
    "path": "/data/oxigraph"
  },
  "cache": {
    "enabled": true,
    "ttl_seconds": 300,
    "max_size_mb": 100
  },
  "tenancy": {
    "isolation_mode": "named_graphs"
  }
}
```

### Dependency Injection

Use DI for testability:

```rust
// Rust - trait-based DI
trait Repository {
    fn find(&self, id: &str) -> Result<Option<Entity>>;
}

struct Service<R: Repository> {
    repo: R,
}
```

```python
# Python - constructor injection
class Service:
    def __init__(self, repository: Repository):
        self.repository = repository
```

```javascript
// JavaScript - constructor injection
class Service {
    constructor(repository) {
        this.repository = repository;
    }
}
```

---

## Performance Considerations

### Pattern Performance Characteristics

| **Pattern** | **Read Overhead** | **Write Overhead** | **Memory** | **Best For** |
|-------------|-------------------|--------------------| ----------|--------------|
| Repository | Low | Low | Low | All workloads |
| Event Sourcing | Medium | High | Medium | Write-heavy, audit |
| Caching | Very Low (cached) | Low | High | Read-heavy |
| Multi-Tenancy | Low | Low | Low | SaaS |

### Optimization Tips

1. **Repository Pattern**
   - Use bulk operations where possible
   - Implement query result streaming for large datasets
   - Consider read-through caching

2. **Event Sourcing**
   - Use bulk loader for event replay
   - Snapshot current state periodically
   - Archive old events to secondary storage

3. **Caching**
   - Cache at appropriate granularity (query vs entity)
   - Use cache warming for predictable patterns
   - Monitor hit rates and adjust TTL

4. **Multi-Tenancy**
   - Index tenant ID fields
   - Use GRAPH clause efficiently
   - Consider tenant sharding for very large deployments

---

## Security Best Practices

### Input Validation

Always validate inputs before constructing SPARQL:

```rust
// Rust - validate before use
fn validate_iri(iri: &str) -> Result<NamedNode, Error> {
    NamedNode::new(iri).map_err(|e| Error::InvalidIri(e))
}
```

### SPARQL Injection Prevention

Use parameterized queries:

```javascript
// JavaScript - NEVER concatenate user input
// BAD: const query = `SELECT * WHERE { ?s ?p "${userInput}" }`;

// GOOD: Use query substitutions
const results = store.query(
    'SELECT * WHERE { ?s ?p ?value }',
    { substitutions: { value: literal(userInput) } }
);
```

### Tenant Isolation

Enforce tenant boundaries in multi-tenancy:

```python
# Python - always filter by tenant
def get_tenant_graph(tenant_id: str) -> NamedNode:
    # Validate tenant_id first
    if not is_valid_tenant_id(tenant_id):
        raise ValueError("Invalid tenant ID")
    return NamedNode(f"http://example.com/tenant/{tenant_id}")
```

---

## Migration and Evolution

### Adding Patterns to Existing Code

1. **Start Small** - Pick one pattern, implement for one module
2. **Measure Impact** - Track before/after metrics
3. **Iterate** - Expand to other modules based on learnings
4. **Document** - Keep team aligned on pattern usage

### Pattern Evolution

As your application grows:

- **Repository** → Add caching layer when performance matters
- **Simple queries** → Event sourcing when audit trail needed
- **Single tenant** → Multi-tenancy when adding customers
- **In-memory** → Distributed cache when scaling horizontally

---

## Next Steps

Choose a pattern based on your needs:

1. **Building a new application?** Start with [Repository Pattern](./repository-pattern.md)
2. **Need audit compliance?** Implement [Event Sourcing](./event-sourcing.md)
3. **Facing performance issues?** Add [Caching](./caching.md)
4. **Building SaaS?** Design for [Multi-Tenancy](./multi-tenancy.md)

Each pattern guide includes:
- When to use (and when not to)
- Complete implementation in Rust, Python, and JavaScript
- Real-world examples and use cases
- Testing strategies
- Performance optimization tips
- Common pitfalls and anti-patterns

---

## Contributing

Have you implemented a useful pattern not listed here?

- [Open an issue](https://github.com/oxigraph/oxigraph/issues) to discuss
- [Submit a pull request](https://github.com/oxigraph/oxigraph/pulls) with your pattern
- Follow the pattern documentation template
- Include working code examples in all three languages

Pattern documentation should:
- Solve a real, recurring problem
- Include complete, tested code examples
- Explain trade-offs clearly
- Provide production-ready implementations
- Cover error handling and edge cases
