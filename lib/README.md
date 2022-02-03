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

Oxigraph also provides [a standalone HTTP server](https://crates.io/crates/oxigraph_server) and [a Python library](https://oxigraph.org/pyoxigraph/) based on this library.


Oxigraph implements the following specifications:
* [SPARQL 1.1 Query](https://www.w3.org/TR/sparql11-query/), [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/), and [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/).
* [Turtle](https://www.w3.org/TR/turtle/), [TriG](https://www.w3.org/TR/trig/), [N-Triples](https://www.w3.org/TR/n-triples/), [N-Quads](https://www.w3.org/TR/n-quads/), and [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/) RDF serialization formats for both data ingestion and retrieval using the [Rio library](https://github.com/oxigraph/rio).
* [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/), [SPARQL 1.1 Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/) and [SPARQL 1.1 Query Results CSV and TSV Formats](https://www.w3.org/TR/sparql11-results-csv-tsv/).

A preliminary benchmark [is provided](../bench/README.md). Oxigraph internal design [is described on the wiki](https://github.com/oxigraph/oxigraph/wiki/Architecture).

The main entry point of Oxigraph is the [`Store`](store::Store) struct:
```rust
use oxigraph::store::Store;
use oxigraph::model::*;
use oxigraph::sparql::QueryResults;

let store = Store::new().unwrap();

// insertion
let ex = NamedNode::new("http://example.com").unwrap();
let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), GraphName::DefaultGraph);
store.insert(&quad).unwrap();

// quad filter
let results = store.quads_for_pattern(Some(ex.as_ref().into()), None, None, None).collect::<Result<Vec<Quad>,_>>().unwrap();
assert_eq!(vec![quad], results);

// SPARQL query
if let QueryResults::Solutions(mut solutions) =  store.query("SELECT ?s WHERE { ?s ?p ?o }").unwrap() {
    assert_eq!(solutions.next().unwrap().unwrap().get("s"), Some(&ex.into()));
}
```

Some parts of this library are available as standalone crates:
* [`oxrdf`](https://crates.io/crates/oxrdf) provides datastructures encoding RDF basic concepts (the `oxigraph::model` module).
* [`spargebra`](https://crates.io/crates/spargebra) provides a SPARQL parser.
* [`sparesults`](https://crates.io/crates/sparesults) provides parsers and serializers for SPARQL result formats.

## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or
  `<http://www.apache.org/licenses/LICENSE-2.0>`)
* MIT license ([LICENSE-MIT](../LICENSE-MIT) or
  `<http://opensource.org/licenses/MIT>`)

at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Oxigraph by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
