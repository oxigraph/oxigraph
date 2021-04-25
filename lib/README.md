Oxigraph
========

[![Latest Version](https://img.shields.io/crates/v/oxigraph.svg)](https://crates.io/crates/oxigraph)
[![Released API docs](https://docs.rs/oxigraph/badge.svg)](https://docs.rs/oxigraph)
[![Crates.io downloads](https://img.shields.io/crates/d/oxigraph)](https://crates.io/crates/oxigraph)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge)

Oxigraph is a graph database library implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.

Its goal is to provide a compliant, safe and fast on-disk graph database.
It also provides a set of utility functions for reading, writing, and processing RDF files.

Oxigraph is in heavy development and SPARQL query evaluation has not been optimized yet.

The disabled by default `"sophia"` feature provides [`sophia_api`](https://docs.rs/sophia_api/) traits implementation on Oxigraph terms and stores.

Oxigraph also provides [a standalone HTTP server](https://crates.io/crates/oxigraph_server) and [a Python library](https://oxigraph.org/pyoxigraph/) based on this library.


Oxigraph implements the following specifications:
* [SPARQL 1.1 Query](https://www.w3.org/TR/sparql11-query/), [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/), and [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/).
* [Turtle](https://www.w3.org/TR/turtle/), [TriG](https://www.w3.org/TR/trig/), [N-Triples](https://www.w3.org/TR/n-triples/), [N-Quads](https://www.w3.org/TR/n-quads/), and [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/) RDF serialization formats for both data ingestion and retrieval using the [Rio library](https://github.com/oxigraph/rio).
* [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/), [SPARQL 1.1 Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/) and [SPARQL 1.1 Query Results CSV and TSV Formats](https://www.w3.org/TR/sparql11-results-csv-tsv/).

A preliminary benchmark [is provided](../bench/README.md).

Usage example:

```rust
use oxigraph::store::Store;
use oxigraph::sparql::QueryResults;
use oxigraph::model::*;

let store = Store::open("example.db")?;

// insertion
let ex = NamedNode::new("http://example.com")?;
let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), GraphName::DefaultGraph);
store.insert(&quad)?;

// quad filter
let results: Result<Vec<Quad>,_> = store.quads_for_pattern(None, None, None, None).collect();
assert_eq!(vec![quad], results?);

// SPARQL query
if let QueryResults::Solutions(mut solutions) = store.query("SELECT ?s WHERE { ?s ?p ?o }")? {
assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
};
```

## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](../LICENSE-MIT) or
   http://opensource.org/licenses/MIT)
   
at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Futures by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
