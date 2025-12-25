# Tutorials

**Learning-oriented lessons that guide you through practical exercises**

Tutorials are designed to help you learn Oxigraph by doing. Each tutorial is a complete lesson that takes you step-by-step through a specific learning objective. Unlike how-to guides which assume prior knowledge, tutorials are crafted for newcomers and provide a safe, guided environment to build your skills.

## What to Expect

- **Step-by-step instructions** - Clear, numbered steps that build on each other
- **Complete working examples** - Copy-paste ready code that actually runs
- **Explanatory context** - Learn not just what to do, but why
- **Hands-on practice** - Build real understanding through active learning
- **Safe learning environment** - Designed to work on your first try

## Who Should Use Tutorials?

- You're new to Oxigraph and want to get started
- You're familiar with RDF but new to graph databases
- You want to learn best practices from the ground up
- You prefer learning by doing rather than reading theory

---

## Getting Started

### Beginner Tutorials

These tutorials introduce core Oxigraph concepts and are designed to be followed in order:

1. **[Your First Oxigraph Store](./01-first-store.md)** ⏱️ 10 minutes
   - Create your first in-memory and persistent stores
   - Understand the basic Store API
   - Choose between MemoryStore and Store

2. **[Loading RDF Data](./02-loading-data.md)** ⏱️ 15 minutes
   - Load data from files and strings
   - Work with different RDF formats (Turtle, N-Triples, JSON-LD)
   - Handle bulk loading for performance

3. **[Basic SPARQL Queries](./03-basic-queries.md)** ⏱️ 20 minutes
   - Write your first SELECT query
   - Iterate through query results
   - Extract values from solutions
   - Use FILTER and OPTIONAL patterns

4. **[Working with RDF Terms](./04-rdf-terms.md)** ⏱️ 15 minutes
   - Create and manipulate Named Nodes (IRIs)
   - Work with Literals (strings, numbers, dates)
   - Understand Blank Nodes
   - Build Triples and Quads programmatically

5. **[Modifying Data with SPARQL UPDATE](./05-sparql-update.md)** ⏱️ 20 minutes
   - Insert new triples with INSERT DATA
   - Delete triples with DELETE DATA
   - Perform conditional updates with DELETE/INSERT WHERE
   - Use transactions for atomic operations

---

## Intermediate Tutorials

Build on your foundational knowledge with these more advanced lessons:

6. **[Building a Knowledge Graph](./06-knowledge-graph.md)** ⏱️ 30 minutes
   - Model real-world entities and relationships
   - Use schema.org vocabulary
   - Implement bidirectional relationships
   - Query complex graph patterns

7. **[SPARQL Query Patterns](./07-query-patterns.md)** ⏱️ 25 minutes
   - Master graph patterns and variable bindings
   - Use UNION for alternative patterns
   - Work with property paths
   - Aggregate data with GROUP BY and COUNT

8. **[Named Graphs and Quads](./08-named-graphs.md)** ⏱️ 20 minutes
   - Organize data into named graphs
   - Query specific graphs with GRAPH
   - Manage multi-tenant data
   - Use the SPARQL Graph Store Protocol

9. **[Data Validation with SHACL](./09-shacl-validation.md)** ⏱️ 25 minutes
   - Define SHACL shapes for data quality
   - Validate RDF data against constraints
   - Interpret validation reports
   - Implement custom validation rules

10. **[Exporting and Serializing Data](./10-exporting-data.md)** ⏱️ 15 minutes
    - Export data in different RDF formats
    - Serialize query results (JSON, XML, CSV)
    - Stream large datasets efficiently
    - Integrate with external tools

---

## Language-Specific Tutorials

### Rust

11. **[Embedding Oxigraph in Rust Applications](./rust/embedding.md)** ⏱️ 30 minutes
    - Integrate Store into your application architecture
    - Handle errors idiomatically with Result types
    - Use iterators for memory-efficient processing
    - Implement custom SPARQL extensions

12. **[Async and Concurrent Access in Rust](./rust/async-patterns.md)** ⏱️ 25 minutes
    - Share stores across async tasks
    - Use Arc for thread-safe access
    - Batch operations for performance
    - Handle concurrent reads and writes

### Python

13. **[Using Oxigraph in Python Data Pipelines](./python/data-pipelines.md)** ⏱️ 30 minutes
    - Integrate with pandas and numpy
    - Build ETL pipelines with RDF
    - Connect to existing data sources
    - Export to Python data structures

14. **[Building a Flask API with Oxigraph](./python/flask-api.md)** ⏱️ 35 minutes
    - Create a RESTful API backed by Oxigraph
    - Handle SPARQL queries from HTTP requests
    - Implement pagination and filtering
    - Add authentication and authorization

### JavaScript

15. **[Browser-Based RDF Applications](./javascript/browser-app.md)** ⏱️ 30 minutes
    - Load Oxigraph in the browser with WebAssembly
    - Build interactive RDF visualizations
    - Store and query data client-side
    - Integrate with React or Vue

16. **[Node.js Server with Oxigraph](./javascript/nodejs-server.md)** ⏱️ 30 minutes
    - Create an Express.js API with Oxigraph
    - Handle file uploads and data ingestion
    - Stream query results to clients
    - Deploy to production

---

## Advanced Tutorials

17. **[Performance Optimization](./advanced/performance.md)** ⏱️ 40 minutes
    - Profile and optimize SPARQL queries
    - Use bulk loading for large datasets
    - Tune RocksDB configuration
    - Implement caching strategies

18. **[Migrating from Other Triple Stores](./advanced/migration.md)** ⏱️ 45 minutes
    - Export data from existing stores
    - Handle schema and vocabulary mapping
    - Verify data integrity after migration
    - Update application code

19. **[Implementing Custom Extensions](./advanced/custom-extensions.md)** ⏱️ 50 minutes
    - Add custom SPARQL functions (Rust only)
    - Implement domain-specific query optimizations
    - Extend the data model
    - Contribute back to Oxigraph

---

## Tutorial Series

### Building a Semantic Search Engine

A multi-part series that walks through creating a complete application:

- **Part 1**: [Setting Up the Project](./series/search-engine/part-1.md)
- **Part 2**: [Ingesting and Indexing Documents](./series/search-engine/part-2.md)
- **Part 3**: [Implementing Search Queries](./series/search-engine/part-3.md)
- **Part 4**: [Adding Faceted Navigation](./series/search-engine/part-4.md)
- **Part 5**: [Deploying to Production](./series/search-engine/part-5.md)

### Building a Personal Knowledge Base

Learn to create a personal wiki powered by RDF:

- **Part 1**: [Data Modeling for Notes and Tags](./series/knowledge-base/part-1.md)
- **Part 2**: [Implementing Bidirectional Links](./series/knowledge-base/part-2.md)
- **Part 3**: [Full-Text Search with SPARQL](./series/knowledge-base/part-3.md)
- **Part 4**: [Building a Web Interface](./series/knowledge-base/part-4.md)

---

## Next Steps

- **Completed a tutorial?** Try applying what you learned to your own project
- **Need to accomplish a specific task?** Check the [How-To Guides](../how-to/)
- **Want deeper understanding?** Read the [Explanations](../explanation/)
- **Looking for API details?** Browse the [Reference](../reference/)

---

## Contributing Tutorials

We welcome tutorial contributions! If you have an idea for a tutorial or found an error in an existing one:

- [Open an issue](https://github.com/oxigraph/oxigraph/issues) with your suggestion
- [Submit a pull request](https://github.com/oxigraph/oxigraph/pulls) with improvements
- Follow our [tutorial writing guide](../contributing/tutorial-guide.md)

Tutorials should be:
- Focused on teaching, not just showing
- Tested and verified to work exactly as written
- Accessible to the target audience level
- Complete from start to finish
