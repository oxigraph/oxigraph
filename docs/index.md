# Oxigraph Documentation

**A fast, standards-compliant graph database implementing SPARQL**

Oxigraph is a graph database that implements the SPARQL standard, providing a robust and efficient solution for managing RDF data. Whether you're building semantic web applications, knowledge graphs, or linked data platforms, Oxigraph offers the performance and compliance you need.

## Available as Multiple Targets

- **Rust Library** - Embed graph database capabilities in your Rust applications
- **Standalone Server** - Production-ready HTTP SPARQL endpoint
- **Python Bindings** - Integrate with Python data science and web ecosystems
- **JavaScript/WebAssembly** - Run in browsers and Node.js environments

---

## Documentation Navigation

This documentation follows the [Diataxis framework](https://diataxis.fr/), organizing content into four distinct sections based on your needs:

<table>
<thead>
<tr>
<th></th>
<th>🎓 Learning-Oriented</th>
<th>🎯 Task-Oriented</th>
</tr>
</thead>
<tbody>
<tr>
<td><strong>Practical</strong></td>
<td>
<h3><a href="tutorials/">📚 Tutorials</a></h3>
<p>Step-by-step lessons to learn Oxigraph fundamentals through hands-on exercises.</p>
<ul>
<li>Getting started with your first store</li>
<li>Loading and querying RDF data</li>
<li>Building a knowledge graph</li>
</ul>
</td>
<td>
<h3><a href="how-to/">🔧 How-To Guides</a></h3>
<p>Problem-focused recipes for accomplishing specific tasks with Oxigraph.</p>
<ul>
<li>Optimize query performance</li>
<li>Deploy a production server</li>
<li>Integrate with existing systems</li>
</ul>
</td>
</tr>
<tr>
<td><strong>Theoretical</strong></td>
<td>
<h3><a href="explanation/">💡 Explanations</a></h3>
<p>Conceptual discussions that deepen your understanding of Oxigraph and RDF.</p>
<ul>
<li>Architecture and design decisions</li>
<li>RDF and SPARQL concepts</li>
<li>Performance characteristics</li>
</ul>
</td>
<td>
<h3><a href="reference/">📖 Reference</a></h3>
<p>Technical specifications, API documentation, and comprehensive details.</p>
<ul>
<li>API documentation (Rust, Python, JS)</li>
<li>SPARQL compliance</li>
<li>Configuration options</li>
</ul>
</td>
</tr>
</tbody>
</table>

---

## Quick Start

### Installation

Choose your preferred environment:

**Rust:**
```bash
cargo add oxigraph
```

**Python:**
```bash
pip install pyoxigraph
```

**JavaScript/Node.js:**
```bash
npm install oxigraph
```

**Standalone Server:**
```bash
cargo install oxigraph-cli
oxigraph serve --location ./data
```

### Your First Query

**Rust:**
```rust
use oxigraph::store::Store;
use oxigraph::sparql::QueryResults;

let store = Store::new()?;
store.load_from_reader(
    r#"<http://example.org/Alice> <http://schema.org/name> "Alice" ."#.as_bytes(),
    oxigraph::io::RdfFormat::Turtle,
    None,
)?;

if let QueryResults::Solutions(solutions) = store.query("SELECT ?name WHERE { ?person <http://schema.org/name> ?name }")? {
    for solution in solutions {
        println!("{}", solution?.get("name").unwrap());
    }
}
```

**Python:**
```python
from pyoxigraph import Store

store = Store()
store.load(b'<http://example.org/Alice> <http://schema.org/name> "Alice" .', "text/turtle")

for solution in store.query('SELECT ?name WHERE { ?person <http://schema.org/name> ?name }'):
    print(solution["name"].value)
```

**JavaScript:**
```javascript
import oxigraph from 'oxigraph';

const store = new oxigraph.Store();
store.load('<http://example.org/Alice> <http://schema.org/name> "Alice" .', 'text/turtle');

for (const solution of store.query('SELECT ?name WHERE { ?person <http://schema.org/name> ?name }')) {
    console.log(solution.get('name').value);
}
```

---

## Key Features

- **SPARQL 1.1 Compliant** - Full implementation of SPARQL Query, Update, and Protocol
- **Multiple RDF Formats** - Turtle, N-Triples, N-Quads, RDF/XML, JSON-LD, TriG
- **High Performance** - Efficient indexing with RocksDB, optimized query execution
- **Standards-Based** - W3C-compliant RDF, SPARQL, and SHACL support
- **Transactional** - ACID guarantees for data integrity
- **Embeddable** - Zero-configuration in-memory or persistent stores
- **Cross-Platform** - Rust-native with Python and JavaScript bindings

---

## Community and Support

- **GitHub**: [https://github.com/oxigraph/oxigraph](https://github.com/oxigraph/oxigraph)
- **Website**: [https://oxigraph.org](https://oxigraph.org)
- **Issues**: [Report bugs or request features](https://github.com/oxigraph/oxigraph/issues)

---

## Contributing

Oxigraph is open source and welcomes contributions! See the [contributing guide](https://github.com/oxigraph/oxigraph/blob/main/CONTRIBUTING.md) to get started.

---

## Where to Go Next

- **New to Oxigraph?** Start with the [Tutorials](tutorials/) to learn through hands-on examples
- **Have a specific task?** Check the [How-To Guides](how-to/) for practical solutions
- **Need technical details?** Browse the [Reference](reference/) documentation
- **Want to understand more?** Read the [Explanations](explanation/) for conceptual depth

Welcome to the Oxigraph documentation. We're excited to have you here!
