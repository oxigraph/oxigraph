# Oxigraph

[![Latest Version](https://img.shields.io/crates/v/oxigraph.svg)](https://crates.io/crates/oxigraph)
[![Released API docs](https://docs.rs/oxigraph/badge.svg)](https://docs.rs/oxigraph)
[![PyPI](https://img.shields.io/pypi/v/pyoxigraph)](https://pypi.org/project/pyoxigraph/)
[![npm](https://img.shields.io/npm/v/oxigraph)](https://www.npmjs.com/package/oxigraph)
[![tests status](https://github.com/oxigraph/oxigraph/actions/workflows/tests.yml/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![artifacts status](https://github.com/oxigraph/oxigraph/actions/workflows/artifacts.yml/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![dependency status](https://deps.rs/repo/github/oxigraph/oxigraph/status.svg)](https://deps.rs/repo/github/oxigraph/oxigraph)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community)
[![Twitter URL](https://img.shields.io/twitter/url?style=social&url=https%3A%2F%2Ftwitter.com%2Foxigraph)](https://twitter.com/oxigraph)

**A fast, embeddable graph database implementing the SPARQL standard.**

Oxigraph is a graph database written in Rust that provides:
- **High performance** - Fast query execution with optimized indexing
- **Standards compliance** - Full SPARQL 1.1 support, passes W3C test suites
- **Easy deployment** - Single binary, Docker image, or embeddable library
- **Multiple interfaces** - Rust, Python, JavaScript bindings + HTTP server
- **Persistent storage** - RocksDB backend with in-memory option

Perfect for embedding in applications, building SPARQL endpoints, or processing RDF data at scale.

---

## Quick Start

Get up and running in 60 seconds:

### Rust
```bash
cargo add oxigraph
```
```rust
use oxigraph::store::Store;
use oxigraph::io::RdfFormat;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;
    let data = std::fs::read_to_string("data.ttl")?;
    store.load_from_reader(RdfFormat::Turtle, data.as_bytes())?;

    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query("SELECT * WHERE { ?s ?p ?o } LIMIT 10")?
        .on_store(&store)
        .execute()?
    {
        while let Some(solution) = solutions.next() {
            println!("{:?}", solution?);
        }
    }
    Ok(())
}
```

### Python
```bash
pip install pyoxigraph
```
```python
from pyoxigraph import Store, RdfFormat

store = Store()
store.load(path="data.ttl", format=RdfFormat.TURTLE)
for result in store.query("SELECT * WHERE { ?s ?p ?o } LIMIT 10"):
    print(result)
```

### JavaScript
```bash
npm install oxigraph
```
```javascript
const oxigraph = require('oxigraph');
const fs = require('fs');

const store = new oxigraph.Store();
const data = fs.readFileSync('data.ttl', 'utf-8');
store.load(data, { format: "text/turtle" });

for (const binding of store.query("SELECT * WHERE { ?s ?p ?o } LIMIT 10")) {
    console.log(binding);
}
```

### CLI Server (Docker)
```bash
docker run -d -p 7878:7878 -v $PWD/data:/data \
  ghcr.io/oxigraph/oxigraph:latest serve --location /data --bind 0.0.0.0:7878
```

Then open http://localhost:7878 in your browser!

**See the [Quick Start Guide](docs/quick-start.md) for complete examples.**

---

## Documentation

- **[Quick Start Guide](docs/quick-start.md)** - Get started in 5 minutes
- **[Installation Guide](docs/installation.md)** - Detailed installation for all platforms
- **[FAQ](docs/faq.md)** - Frequently asked questions
- **[Contributing](docs/CONTRIBUTING.md)** - How to contribute

### API Documentation
- [Rust API docs](https://docs.rs/oxigraph)
- [Python API docs](https://pyoxigraph.readthedocs.io/)
- [JavaScript README](./js/README.md)
- [CLI documentation](./cli/README.md)

### Technical Resources
- [Architecture](https://github.com/oxigraph/oxigraph/wiki/Architecture) - Internal design
- [Benchmarks](bench/README.md) - Performance benchmarks
- [W3C Test Suites](testsuite/README.md)

---

## Features

Oxigraph is in active development and SPARQL query evaluation is continuously being optimized.
The development roadmap is tracked using [GitHub milestones](https://github.com/oxigraph/oxigraph/milestones?direction=desc&sort=completeness&state=open).

### Standards Implemented

**SPARQL:**
- [SPARQL 1.1 Query](https://www.w3.org/TR/sparql11-query/), [Update](https://www.w3.org/TR/sparql11-update/), and [Federated Query](https://www.w3.org/TR/sparql11-federated-query/)
- [SPARQL 1.2](https://www.w3.org/TR/sparql12-query/) (experimental, via `rdf-12` feature)
- RDF-star / SPARQL-star (quoted triples)

**RDF Serialization Formats:**
- [Turtle](https://www.w3.org/TR/turtle/), [TriG](https://www.w3.org/TR/trig/), [N-Triples](https://www.w3.org/TR/n-triples/), [N-Quads](https://www.w3.org/TR/n-quads/)
- [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/), [JSON-LD](https://www.w3.org/TR/json-ld/), [N3](https://w3c.github.io/N3/spec/)

**Query Results Formats:**
- [JSON](https://www.w3.org/TR/sparql11-results-json/), [XML](https://www.w3.org/TR/rdf-sparql-XMLres/), [CSV/TSV](https://www.w3.org/TR/sparql11-results-csv-tsv/)

---

## Available Packages

Oxigraph is available in multiple forms to fit your needs:

### 🦀 Rust Library
**[`oxigraph`](./lib/oxigraph)** - Embeddable database library
[![Crates.io](https://img.shields.io/crates/v/oxigraph.svg)](https://crates.io/crates/oxigraph)
[![docs.rs](https://docs.rs/oxigraph/badge.svg)](https://docs.rs/oxigraph)

```toml
[dependencies]
oxigraph = "0.4"
```

### 🐍 Python
**[`pyoxigraph`](./python)** - Python bindings
[![PyPI](https://img.shields.io/pypi/v/pyoxigraph)](https://pypi.org/project/pyoxigraph/)
[![Conda](https://img.shields.io/conda/vn/conda-forge/pyoxigraph)](https://anaconda.org/conda-forge/pyoxigraph)

```bash
pip install pyoxigraph
```

### 📦 JavaScript/TypeScript
**[`oxigraph`](./js)** - WebAssembly bindings for Node.js and browsers
[![npm](https://img.shields.io/npm/v/oxigraph)](https://www.npmjs.com/package/oxigraph)

```bash
npm install oxigraph
```

### 🖥️ CLI Server
**[`oxigraph-cli`](./cli)** - Standalone SPARQL server
[![Crates.io](https://img.shields.io/crates/v/oxigraph-cli.svg)](https://crates.io/crates/oxigraph-cli)
[![Docker](https://img.shields.io/badge/docker-ghcr.io-blue)](https://github.com/oxigraph/oxigraph/pkgs/container/oxigraph)

Implements [SPARQL 1.1 Protocol](https://www.w3.org/TR/sparql11-protocol/) and [Graph Store Protocol](https://www.w3.org/TR/sparql11-http-rdf-update/).

```bash
# Docker
docker pull ghcr.io/oxigraph/oxigraph:latest

# Cargo
cargo install oxigraph-cli

# Pre-built binaries
# Download from GitHub Releases
```

### 📚 Standalone Rust Crates

Lower-level crates for building custom RDF/SPARQL tools:

- **[`oxrdf`](./lib/oxrdf)** - RDF data model (NamedNode, Literal, Triple, Quad)
- **[`oxrdfio`](./lib/oxrdfio)** - Unified RDF parsing/serialization
  - [`oxttl`](./lib/oxttl) - Turtle, TriG, N-Triples, N-Quads, N3
  - [`oxrdfxml`](./lib/oxrdfxml) - RDF/XML
  - [`oxjsonld`](./lib/oxjsonld) - JSON-LD
- **[`spargebra`](./lib/spargebra)** - SPARQL parser and algebra
- **[`spareval`](./lib/spareval)** - SPARQL query evaluator
- **[`sparopt`](./lib/sparopt)** - SPARQL optimizer
- **[`sparesults`](./lib/sparesults)** - SPARQL results parsing/serialization
- **[`oxsdatatypes`](./lib/oxsdatatypes)** - XSD datatypes implementation

The library layers in Oxigraph. The elements above depend on the elements below:
![Oxigraph libraries architecture diagram](./docs/arch-diagram.svg)

---

## Building from Source

Clone with submodules:
```bash
git clone --recursive https://github.com/oxigraph/oxigraph.git
cd oxigraph
```

If already cloned:
```bash
git submodule update --init --recursive
```

Build all components:
```bash
cargo build --all --release
cargo test --all
```

See the [Installation Guide](docs/installation.md) for detailed build instructions.

---

## Community & Support

### Get Help

- **[GitHub Discussions](https://github.com/oxigraph/oxigraph/discussions)** - Ask questions, share ideas
- **[Gitter Chat](https://gitter.im/oxigraph/community)** - Real-time community chat
- **[FAQ](docs/faq.md)** - Frequently asked questions
- **[Stack Overflow](https://stackoverflow.com/questions/tagged/oxigraph)** - Tag: `oxigraph`

### Report Issues

Found a bug? [Open an issue](https://github.com/oxigraph/oxigraph/issues/new)

### Contribute

We welcome contributions! See [CONTRIBUTING.md](docs/CONTRIBUTING.md) for guidelines.

### Commercial Support

For advanced support or custom features, contact [@Tpt](https://github.com/Tpt).


---

## Sponsors

Oxigraph development is supported by:

- **[Zazuko](https://zazuko.com/)** - Knowledge graph consulting
- **[RelationLabs](https://relationlabs.ai/)** - Building [Relation-Graph](https://github.com/relationlabs/Relation-Graph) on Substrate
- **[Magnus Bakken](https://github.com/magbak)** - Creator of [Data Treehouse](https://www.data-treehouse.com/) and [chrontext](https://github.com/magbak/chrontext)
- **[DeciSym.AI](https://www.decisym.ai/)** - RDF-based cybersecurity software
- **[ACE IoT Solutions](https://aceiotsolutions.com/)** - Building IoT platform
- **[Albin Larsson](https://byabbe.se/)** - Creator of [GovDirectory](https://www.govdirectory.org/)
- **[Field 33](https://field33.com)** - Ontology management platform

And [many others](https://github.com/sponsors/Tpt). Thank you! ❤️

[**Become a sponsor**](https://github.com/sponsors/Tpt) to support Oxigraph development.

---

## License

Dual-licensed under either:

- **Apache License 2.0** ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- **MIT License** ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

You may choose either license for your use.

### Contribution

Unless explicitly stated otherwise, any contribution intentionally submitted for inclusion in Oxigraph shall be dual-licensed as above, without additional terms or conditions.

---

**Ready to get started?** Check out the [Quick Start Guide](docs/quick-start.md)!
