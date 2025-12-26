# Oxigraph Documentation

Welcome to the Oxigraph documentation! This guide will help you navigate all available resources and find exactly what you need.

## Quick Start

**New to Oxigraph?** Start here:

1. **[Onboarding Guide](onboarding.md)** - Set up your environment and verify installation
2. **[Quick Start](quick-start.md)** - Get started in 5 minutes with copy-paste examples
3. **[Learning Path](learning-path.md)** - Structured learning from beginner to advanced
4. **[Cheatsheet](cheatsheet.md)** - Quick reference for common operations

## Documentation Structure

This documentation follows the [Diataxis framework](https://diataxis.fr/), organizing content by purpose:

### 📚 [Tutorials](tutorials/) - Learning-Oriented
**"Teach me by doing"**

Step-by-step lessons for learning Oxigraph fundamentals:
- Getting started guides (Rust, Python, JavaScript)
- SPARQL introduction
- RDF formats overview
- Working with RDF data

[Browse all tutorials →](tutorials/index.md)

### 🔧 [How-To Guides](how-to/) - Task-Oriented
**"Show me how to solve this problem"**

Practical solutions for specific tasks:
- Import and export RDF data
- Execute SPARQL queries
- Optimize performance
- Validate with SHACL
- Run SPARQL server
- SPARQL updates

[Browse all how-to guides →](how-to/index.md)

### 💡 [Explanations](explanation/) - Understanding-Oriented
**"Help me understand concepts"**

Conceptual discussions about RDF, SPARQL, and Oxigraph:
- RDF fundamentals
- SPARQL explained
- Semantic web concepts
- Architecture and design

[Browse all explanations →](explanation/index.md)

### 📖 [Reference](reference/) - Information-Oriented
**"Give me the technical details"**

Technical specifications and API documentation:
- Crates overview
- RDF formats
- SPARQL support and functions
- API documentation (Rust, Python, JavaScript)
- Configuration options

[Browse all reference docs →](reference/index.md)

---

## Additional Resources

### Examples & Patterns

- **[Integration Examples](examples/)** - Production-ready code examples
  - Rust integration patterns
  - Python integration patterns
  - JavaScript integration patterns
  - Real-world project examples

- **[Design Patterns](patterns/)** - Architectural patterns
  - Repository pattern
  - Event sourcing
  - Caching strategies
  - Multi-tenancy

- **[Integration Scenarios](scenarios/)** - Complete use cases
  - Migrate from Apache Jena
  - Migrate from RDFLib
  - Integrate with Wikidata
  - Build a knowledge graph
  - Microservices architecture

### Support & Troubleshooting

- **[FAQ](faq.md)** - Frequently asked questions
- **[Troubleshooting](troubleshooting/)** - Diagnose and fix issues
  - Common errors
  - Performance problems
  - Data issues
  - Deployment challenges
- **[Installation Guide](installation.md)** - Detailed setup instructions

### Development

- **[Contributing Guide](CONTRIBUTING.md)** - How to contribute to Oxigraph
- **[Development Documentation](development/)** - For contributors
  - Testing guide
  - Documentation guide
  - Release process
  - API stability policy

---

## API Documentation

### Official API Docs

- **Rust**: [docs.rs/oxigraph](https://docs.rs/oxigraph) - Complete Rust API
- **Python**: Built-in help - `help(pyoxigraph.Store)`
- **JavaScript**: TypeScript definitions included in package

### Quick API References

By language:
- [Rust API Overview](reference/api-overview.md)
- Python API (see package documentation)
- JavaScript API (see TypeScript definitions)

---

## Learning Paths

### For Beginners

**Goal**: Understand RDF and create your first store

1. [Onboarding Guide](onboarding.md) - Install and verify setup
2. [RDF Fundamentals](explanation/rdf-fundamentals.md) - Learn the basics
3. Language-specific tutorial:
   - [Rust Getting Started](tutorials/rust-getting-started.md)
   - [Python Getting Started](tutorials/python-getting-started.md)
   - [JavaScript Getting Started](tutorials/javascript-getting-started.md)
4. [SPARQL Introduction](tutorials/sparql-introduction.md) - Query your data
5. [Cheatsheet](cheatsheet.md) - Reference for common operations

**Time**: 1-2 hours

### For Intermediate Users

**Goal**: Build real applications with Oxigraph

1. Complete beginner path above
2. [RDF Formats](tutorials/rdf-formats-intro.md) - Work with different formats
3. Language-specific SPARQL guide:
   - [Rust SPARQL Queries](tutorials/rust-sparql-queries.md)
   - [Python SPARQL](tutorials/python-sparql.md)
   - [JavaScript SPARQL](tutorials/javascript-sparql.md)
4. [Integration Examples](examples/) - See production patterns
5. [Design Patterns](patterns/) - Architectural approaches

**Time**: 3-4 hours

### For Advanced Users

**Goal**: Deploy, optimize, and scale Oxigraph

1. [Run SPARQL Server](how-to/run-sparql-server.md) - Deploy to production
2. [Optimize Performance](how-to/optimize-performance.md) - Tune for your workload
3. [SPARQL Advanced Queries](how-to/sparql-advanced-queries.md) - Complex patterns
4. [Architecture Explained](explanation/architecture.md) - Deep dive
5. [Integration Scenarios](scenarios/) - Real-world use cases

**Time**: 6-8 hours

### For Contributors

**Goal**: Contribute code, documentation, or tests

1. [Contributing Guide](CONTRIBUTING.md) - Get started
2. [Development Documentation](development/) - Development workflow
3. [Testing Guide](development/testing-guide.md) - Write tests
4. [Documentation Guide](development/documentation-guide.md) - Improve docs

---

## Documentation by Use Case

### I want to...

**...get started quickly**
→ [Quick Start Guide](quick-start.md)

**...understand RDF and SPARQL**
→ [RDF Fundamentals](explanation/rdf-fundamentals.md) + [SPARQL Explained](explanation/sparql-explained.md)

**...load data into Oxigraph**
→ [Import RDF Data](how-to/import-rdf-data.md)

**...query my data**
→ [SPARQL Introduction](tutorials/sparql-introduction.md)

**...deploy to production**
→ [Run SPARQL Server](how-to/run-sparql-server.md)

**...optimize performance**
→ [Optimize Performance](how-to/optimize-performance.md)

**...migrate from another triplestore**
→ [Migration Scenarios](scenarios/) (Jena, RDFLib, etc.)

**...build an application**
→ [Integration Examples](examples/)

**...validate my data**
→ [Validate with SHACL](how-to/validate-with-shacl.md)

**...troubleshoot an issue**
→ [Troubleshooting Guide](troubleshooting/) + [FAQ](faq.md)

**...contribute to Oxigraph**
→ [Contributing Guide](CONTRIBUTING.md)

---

## Platform-Specific Documentation

### Rust

- [Getting Started](tutorials/rust-getting-started.md)
- [RDF Basics](tutorials/rust-rdf-basics.md)
- [SPARQL Queries](tutorials/rust-sparql-queries.md)
- [Integration Patterns](examples/rust-integration.md)
- [Crates Reference](reference/crates.md)

### Python

- [Getting Started](tutorials/python-getting-started.md)
- [Working with RDF Data](tutorials/python-rdf-data.md)
- [SPARQL Queries](tutorials/python-sparql.md)
- [Integration Patterns](examples/python-integration.md)

### JavaScript

- [Getting Started](tutorials/javascript-getting-started.md)
- [RDF Model](tutorials/javascript-rdf-model.md)
- [SPARQL Queries](tutorials/javascript-sparql.md)
- [Integration Patterns](examples/javascript-integration.md)

### CLI Server

- [Installation](installation.md#cli-server)
- [Quick Start](quick-start.md#cli-server-docker)
- [Run SPARQL Server](how-to/run-sparql-server.md)
- [Configuration](reference/configuration.md)

---

## External Resources

### Official Links

- **Website**: [oxigraph.org](https://oxigraph.org)
- **GitHub**: [github.com/oxigraph/oxigraph](https://github.com/oxigraph/oxigraph)
- **API Docs**: [docs.rs/oxigraph](https://docs.rs/oxigraph)

### Community

- **Discussions**: [GitHub Discussions](https://github.com/oxigraph/oxigraph/discussions)
- **Chat**: [Gitter](https://gitter.im/oxigraph/community)
- **Issues**: [Bug Reports](https://github.com/oxigraph/oxigraph/issues)
- **Stack Overflow**: Tag [`oxigraph`](https://stackoverflow.com/questions/tagged/oxigraph)

### Standards

- **RDF 1.1**: [W3C RDF Primer](https://www.w3.org/TR/rdf11-primer/)
- **SPARQL 1.1**: [W3C SPARQL Spec](https://www.w3.org/TR/sparql11-query/)
- **Turtle**: [W3C Turtle](https://www.w3.org/TR/turtle/)
- **JSON-LD**: [W3C JSON-LD](https://www.w3.org/TR/json-ld11/)

---

## Documentation Status

### Core Documentation (Complete)

- ✅ Onboarding guide
- ✅ Quick start guide
- ✅ Learning path
- ✅ Cheatsheet
- ✅ FAQ
- ✅ Installation guide
- ✅ Contributing guide
- ✅ Basic tutorials (getting started for each language)
- ✅ Integration examples
- ✅ Troubleshooting guides
- ✅ Design patterns
- ✅ Integration scenarios

### Planned Documentation

The section index pages (tutorials/index.md, how-to/index.md, etc.) outline comprehensive documentation that is planned for future releases. Current documentation focuses on essential getting-started materials and common use cases.

**Contributing**: If you'd like to help write any of the planned documentation, please see the [Contributing Guide](CONTRIBUTING.md) and [Documentation Guide](development/documentation-guide.md).

---

## How to Use This Documentation

### First Time Here?

1. Start with [Onboarding](onboarding.md) to set up your environment
2. Follow the [Quick Start](quick-start.md) for your platform
3. Use the [Learning Path](learning-path.md) for structured learning
4. Keep the [Cheatsheet](cheatsheet.md) handy as reference

### Looking for Something Specific?

- **Search**: Use GitHub's search or Ctrl+F in your browser
- **Browse by topic**: Use the [Documentation Structure](#documentation-structure) above
- **Browse by use case**: See [Documentation by Use Case](#documentation-by-use-case)
- **Browse by platform**: See [Platform-Specific Documentation](#platform-specific-documentation)

### Need Help?

1. Check the [FAQ](faq.md) for common questions
2. Search [GitHub Discussions](https://github.com/oxigraph/oxigraph/discussions)
3. Browse [Troubleshooting](troubleshooting/) guides
4. Ask in [Gitter chat](https://gitter.im/oxigraph/community)
5. Open a [GitHub Discussion](https://github.com/oxigraph/oxigraph/discussions/new)

---

## Contributing to Documentation

Found a typo? Want to add an example? Have a tutorial idea?

See the [Documentation Guide](development/documentation-guide.md) for:
- Documentation standards
- Writing guidelines
- How to contribute
- Style conventions

**All contributions welcome!** Documentation improvements help everyone in the community.

---

## License

This documentation is part of the Oxigraph project and is dual-licensed under Apache 2.0 and MIT.

---

**Happy learning!** If you have suggestions for improving this documentation, please [open an issue](https://github.com/oxigraph/oxigraph/issues) or submit a pull request.
