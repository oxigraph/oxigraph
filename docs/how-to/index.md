# How-To Guides

**Task-oriented recipes for solving specific problems with Oxigraph**

How-to guides are practical, focused instructions for accomplishing specific tasks. Unlike tutorials which teach concepts, how-to guides assume you have basic familiarity with Oxigraph and need to solve a particular problem. Each guide is a recipe that gets straight to the solution.

## What to Expect

- **Problem-focused** - Addresses a specific task or challenge
- **Actionable steps** - Clear instructions to achieve your goal
- **Practical solutions** - Production-ready code and configurations
- **Assumes knowledge** - Builds on fundamentals covered in tutorials
- **Multiple approaches** - Often presents alternative solutions

## Who Should Use How-To Guides?

- You have a specific task to accomplish
- You're familiar with Oxigraph basics
- You need practical solutions, not learning material
- You want to solve problems efficiently

---

## Installation and Setup

### Getting Started

- **[Install Oxigraph in Different Environments](./installation/install-environments.md)**
  - Rust, Python, JavaScript, Docker, from source

- **[Set Up a Development Environment](./installation/dev-environment.md)**
  - IDE configuration, debugging tools, test setup

- **[Deploy Oxigraph Server to Production](./installation/production-deployment.md)**
  - Docker, systemd, reverse proxy, monitoring

- **[Configure Oxigraph Server](./installation/server-configuration.md)**
  - Command-line options, environment variables, performance tuning

- **[Migrate Between Oxigraph Versions](./installation/version-migration.md)**
  - Upgrade safely, handle breaking changes

---

## Data Management

### Loading Data

- **[Load Data from Files](./data/load-from-files.md)**
  - Single files, directories, different formats

- **[Import Large Datasets Efficiently](./data/bulk-import.md)**
  - Bulk loader API, performance optimization, progress tracking

- **[Load Data from URLs](./data/load-from-urls.md)**
  - HTTP/HTTPS sources, handling redirects, authentication

- **[Parse and Load Custom RDF Formats](./data/custom-formats.md)**
  - N-Triples, N-Quads, Turtle, TriG, RDF/XML, JSON-LD

- **[Handle Streaming Data](./data/streaming-data.md)**
  - Real-time ingestion, continuous updates

### Exporting Data

- **[Export Data in Different Formats](./data/export-formats.md)**
  - Turtle, N-Triples, RDF/XML, JSON-LD serialization

- **[Dump Entire Store to File](./data/dump-store.md)**
  - Backup strategies, compression, incremental dumps

- **[Export Query Results](./data/export-results.md)**
  - JSON, XML, CSV, TSV formats for SPARQL results

- **[Stream Large Result Sets](./data/stream-results.md)**
  - Memory-efficient iteration, pagination

### Data Validation

- **[Validate Data with SHACL](./data/shacl-validation.md)**
  - Define shapes, run validation, interpret reports

- **[Implement Custom Validation Rules](./data/custom-validation.md)**
  - Application-level constraints, complex business logic

- **[Check Data Integrity](./data/integrity-checks.md)**
  - Verify referential integrity, detect orphaned nodes

---

## Querying

### SPARQL Queries

- **[Execute Basic SELECT Queries](./querying/select-queries.md)**
  - Variable bindings, filters, sorting

- **[Use CONSTRUCT for Graph Transformation](./querying/construct-queries.md)**
  - Create new graphs, schema mapping

- **[Perform ASK Queries for Existence Checks](./querying/ask-queries.md)**
  - Boolean results, conditional logic

- **[Query Multiple Named Graphs](./querying/named-graphs.md)**
  - GRAPH clause, FROM and FROM NAMED

- **[Use SPARQL Property Paths](./querying/property-paths.md)**
  - Transitive relationships, path expressions

### Query Optimization

- **[Optimize Slow Queries](./querying/optimize-queries.md)**
  - Profiling, rewriting patterns, index usage

- **[Use Query Hints and Optimizations](./querying/query-hints.md)**
  - Service-specific optimizations

- **[Implement Pagination for Large Result Sets](./querying/pagination.md)**
  - LIMIT and OFFSET, cursor-based pagination

- **[Cache Query Results](./querying/caching.md)**
  - Application-level caching, cache invalidation

### Federated Queries

- **[Query External SPARQL Endpoints](./querying/federated-queries.md)**
  - SERVICE clause, combining data sources

- **[Handle SERVICE Query Timeouts](./querying/service-timeouts.md)**
  - Configuration, error handling, fallbacks

---

## Data Modification

### SPARQL Updates

- **[Insert Triples with INSERT DATA](./updates/insert-data.md)**
  - Add new facts, programmatic insertion

- **[Delete Triples with DELETE DATA](./updates/delete-data.md)**
  - Remove specific triples, patterns

- **[Update Data with DELETE/INSERT WHERE](./updates/conditional-updates.md)**
  - Conditional modifications, transformations

- **[Use Transactions for Atomic Updates](./updates/transactions.md)**
  - ACID guarantees, rollback on error

- **[Batch Multiple Updates](./updates/batch-updates.md)**
  - Performance optimization, bulk modifications

### Programmatic API

- **[Insert Triples Using the API](./api/insert-triples.md)**
  - Language-specific methods (Rust, Python, JS)

- **[Delete Triples Using the API](./api/delete-triples.md)**
  - Pattern matching, bulk deletion

- **[Update Literals and Values](./api/update-values.md)**
  - Modify existing data, replace values

---

## Performance

### Storage Optimization

- **[Tune RocksDB Configuration](./performance/rocksdb-tuning.md)**
  - Memory limits, compaction, caching

- **[Optimize Disk I/O](./performance/disk-io.md)**
  - File system choices, SSD vs HDD

- **[Manage Database Size](./performance/database-size.md)**
  - Compaction, cleanup, archival

- **[Monitor Storage Metrics](./performance/storage-metrics.md)**
  - Size tracking, growth projections

### Query Performance

- **[Profile Query Execution](./performance/query-profiling.md)**
  - Identify bottlenecks, execution plans

- **[Use Indexes Effectively](./performance/index-usage.md)**
  - Understand SPO, POS, OSP indexes

- **[Optimize Memory Usage](./performance/memory-optimization.md)**
  - Memory store vs persistent store tradeoffs

### Concurrency

- **[Handle Concurrent Reads](./performance/concurrent-reads.md)**
  - Thread safety, clone vs reference

- **[Manage Concurrent Writes](./performance/concurrent-writes.md)**
  - Locking strategies, transaction isolation

- **[Scale with Multiple Instances](./performance/horizontal-scaling.md)**
  - Read replicas, sharding strategies

---

## Integration

### Web Frameworks

- **[Integrate with Flask (Python)](./integration/flask.md)**
  - Request handling, JSON responses

- **[Integrate with Django (Python)](./integration/django.md)**
  - Models, views, ORM interop

- **[Integrate with Express.js (Node.js)](./integration/express.md)**
  - Middleware, routing, error handling

- **[Integrate with Actix/Axum (Rust)](./integration/rust-web.md)**
  - Async handlers, state management

### Data Pipelines

- **[Connect to SQL Databases](./integration/sql-databases.md)**
  - R2RML mapping, ETL patterns

- **[Integrate with Apache Kafka](./integration/kafka.md)**
  - Stream processing, event sourcing

- **[Use with Apache Spark](./integration/spark.md)**
  - Distributed processing, RDF DataFrames

- **[Export to CSV/JSON for Analysis](./integration/export-analysis.md)**
  - Pandas, data science workflows

### APIs and Protocols

- **[Implement SPARQL Protocol Endpoint](./integration/sparql-protocol.md)**
  - HTTP API, content negotiation

- **[Use Graph Store Protocol](./integration/graph-store-protocol.md)**
  - RESTful graph operations

- **[Build GraphQL API on Oxigraph](./integration/graphql.md)**
  - Resolver implementation, schema mapping

- **[Expose as Linked Data Platform](./integration/ldp.md)**
  - LDP containers, resources

---

## Deployment

### Containerization

- **[Run Oxigraph in Docker](./deployment/docker.md)**
  - Dockerfile, docker-compose, volumes

- **[Deploy to Kubernetes](./deployment/kubernetes.md)**
  - Deployments, services, persistent volumes

- **[Use Docker Compose for Development](./deployment/docker-compose.md)**
  - Multi-service setup, local development

### Cloud Platforms

- **[Deploy to AWS](./deployment/aws.md)**
  - EC2, ECS, Lambda options

- **[Deploy to Google Cloud Platform](./deployment/gcp.md)**
  - Compute Engine, Cloud Run

- **[Deploy to Azure](./deployment/azure.md)**
  - VMs, App Service, Container Instances

- **[Deploy to Heroku](./deployment/heroku.md)**
  - Buildpacks, process types

### Monitoring and Operations

- **[Set Up Health Checks](./deployment/health-checks.md)**
  - Liveness and readiness probes

- **[Monitor Performance Metrics](./deployment/monitoring.md)**
  - Prometheus, Grafana integration

- **[Implement Logging](./deployment/logging.md)**
  - Structured logs, log levels, aggregation

- **[Back Up and Restore Data](./deployment/backup-restore.md)**
  - Backup strategies, disaster recovery

- **[Upgrade Without Downtime](./deployment/zero-downtime-upgrade.md)**
  - Rolling updates, blue-green deployment

---

## Security

### Access Control

- **[Implement Authentication](./security/authentication.md)**
  - API keys, JWT tokens, OAuth

- **[Add Authorization Rules](./security/authorization.md)**
  - Role-based access, graph-level permissions

- **[Secure SPARQL Endpoint](./security/secure-endpoint.md)**
  - Rate limiting, query complexity limits

### Data Protection

- **[Encrypt Data at Rest](./security/encryption-rest.md)**
  - Disk encryption, RocksDB encryption

- **[Encrypt Data in Transit](./security/encryption-transit.md)**
  - TLS/SSL configuration, HTTPS

- **[Sanitize User Inputs](./security/input-sanitization.md)**
  - Prevent SPARQL injection, validation

---

## Troubleshooting

### Common Issues

- **[Debug Connection Problems](./troubleshooting/connection-issues.md)**
  - Network errors, timeouts, DNS

- **[Fix Memory Leaks](./troubleshooting/memory-leaks.md)**
  - Profiling, reference cycles

- **[Resolve Parsing Errors](./troubleshooting/parsing-errors.md)**
  - Invalid RDF, encoding issues

- **[Handle Corrupt Data](./troubleshooting/corrupt-data.md)**
  - Recovery strategies, validation

### Performance Issues

- **[Debug Slow Queries](./troubleshooting/slow-queries.md)**
  - Identify causes, optimization techniques

- **[Fix High Memory Usage](./troubleshooting/high-memory.md)**
  - Memory profiling, optimization

- **[Resolve Disk Space Issues](./troubleshooting/disk-space.md)**
  - Cleanup, compaction, archival

---

## Next Steps

- **New to these topics?** Start with the [Tutorials](../tutorials/) for foundational learning
- **Want to understand why?** Read the [Explanations](../explanation/) for conceptual depth
- **Need technical specifications?** Check the [Reference](../reference/) documentation

---

## Contributing

Found a solution to a problem not listed here? We'd love to add it!

- [Submit an issue](https://github.com/oxigraph/oxigraph/issues) describing the problem and solution
- [Create a pull request](https://github.com/oxigraph/oxigraph/pulls) with a new how-to guide
- Follow our [how-to writing guide](../contributing/howto-guide.md)

How-to guides should be:
- Focused on solving a specific, real-world problem
- Practical and actionable
- Production-ready when possible
- Tested and verified to work
