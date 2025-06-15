Oxigraph
========

[![Latest Version](https://img.shields.io/crates/v/oxigraph.svg)](https://crates.io/crates/oxigraph)
[![Released API docs](https://docs.rs/oxigraph/badge.svg)](https://docs.rs/oxigraph)
[![Crates.io downloads](https://img.shields.io/crates/d/oxigraph)](https://crates.io/crates/oxigraph)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community)

Oxigraph is a graph database library implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.

Its goal is to provide a compliant, safe and fast on-disk graph database.
It also provides a set of utility functions for reading, writing, and processing RDF files.

Oxigraph is in heavy development and SPARQL query evaluation has not been optimized yet.

Oxigraph also provides [a CLI tool](https://crates.io/crates/oxigraph-cli) and [a Python library](https://pyoxigraph.readthedocs.io/) based on this library.


Oxigraph implements the following specifications:
* [SPARQL 1.1 Query](https://www.w3.org/TR/sparql11-query/), [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/), and [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/).
* [Turtle](https://www.w3.org/TR/turtle/), [TriG](https://www.w3.org/TR/trig/), [N-Triples](https://www.w3.org/TR/n-triples/), [N-Quads](https://www.w3.org/TR/n-quads/), and [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/) RDF serialization formats for both data ingestion and retrieval.
* [SPARQL Query Results XML Format](https://www.w3.org/TR/rdf-sparql-XMLres/), [SPARQL 1.1 Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/) and [SPARQL 1.1 Query Results CSV and TSV Formats](https://www.w3.org/TR/sparql11-results-csv-tsv/).

Support for [RDF 1.2](https://www.w3.org/TR/rdf12-concepts/) and [SPARQL 1.2](https://www.w3.org/TR/sparql12-query/) is also available behind the `rdf-12` feature.

A preliminary benchmark [is provided](../bench/README.md). Oxigraph internal design [is described on the wiki](https://github.com/oxigraph/oxigraph/wiki/Architecture).

The main entry point of Oxigraph is the [`Store`](store::Store) struct:
```rust
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

let store = Store::new().unwrap();

// insertion
let ex = NamedNode::new("http://example.com").unwrap();
let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), GraphName::DefaultGraph);
store.insert(&quad).unwrap();

// quad filter
let results = store
    .quads_for_pattern(Some(ex.as_ref().into()), None, None, None)
    .collect::<Result<Vec<Quad>, _>>()
    .unwrap();
assert_eq!(vec![quad], results);

// SPARQL query
if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
    .parse_query("SELECT ?s WHERE { ?s ?p ?o }")
    .unwrap()
    .on_store(&store)
    .execute()
    .unwrap()
{
    assert_eq!(
        solutions.next().unwrap().unwrap().get("s"),
        Some(&ex.into())
    );
}
```

It is based on these crates that can be used separately:
* [`oxrdf`](https://crates.io/crates/oxrdf), datastructures encoding RDF basic concepts (the [`oxigraph::model`](crate::model) module).
* [`oxrdfio`](https://crates.io/crates/oxrdfio), a unified parser and serializer API for RDF formats (the [`oxigraph::io`](crate::io) module). It itself relies on:
  * [`oxttl`](https://crates.io/crates/oxttl), N-Triple, N-Quad, Turtle, TriG and N3 parsing and serialization.
  * [`oxrdfxml`](https://crates.io/crates/oxrdfxml), RDF/XML parsing and serialization.
* [`spargebra`](https://crates.io/crates/spargebra), a SPARQL parser.
* [`sparesults`](https://crates.io/crates/sparesults), parsers and serializers for SPARQL result formats (the [`oxigraph::sparql::results`](crate::sparql::results) module).
* [`sparopt`](https://crates.io/crates/sparesults), a SPARQL optimizer.
* [`oxsdatatypes`](https://crates.io/crates/oxsdatatypes), an implementation of some XML Schema datatypes.

To build the library locally, don't forget to clone the submodules using `git clone --recursive https://github.com/oxigraph/oxigraph.git` to clone the repository including submodules or `git submodule update --init` to add submodules to the already cloned repository.

It is possible to disable the RocksDB storage backend to only use the in-memory fallback by disabling the `rocksdb` default feature:
```toml
oxigraph = { version = "*", default-features = false }
```
This is the default behavior when compiling Oxigraph to WASM.

## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or
  `<http://www.apache.org/licenses/LICENSE-2.0>`)
* MIT license ([LICENSE-MIT](../LICENSE-MIT) or
  `<http://opensource.org/licenses/MIT>`)

at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Oxigraph by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
