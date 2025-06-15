Spargebra
=========

[![Latest Version](https://img.shields.io/crates/v/spargebra.svg)](https://crates.io/crates/spargebra)
[![Released API docs](https://docs.rs/spargebra/badge.svg)](https://docs.rs/spargebra)
[![Crates.io downloads](https://img.shields.io/crates/d/spargebra)](https://crates.io/crates/spargebra)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community)

Spargebra is a [SPARQL](https://www.w3.org/TR/sparql11-overview/) parser.

It supports both [SPARQL 1.1 Query](https://www.w3.org/TR/sparql11-query/) and [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/).

The emitted tree is based on [SPARQL 1.1 Query Algebra](https://www.w3.org/TR/sparql11-query/#sparqlQuery) objects.

The API entry point for SPARQL queries is the [`Query`] struct and the API entry point for SPARQL updates is the [`Update`] struct.

Support for [SPARQL 1.2](https://www.w3.org/TR/sparql12-query/) is also available behind the `sparql-12` feature.

This crate is intended to be a building piece for SPARQL implementations in Rust like [Oxigraph](https://oxigraph.org).

Usage example:

```rust
use spargebra::SparqlParser;

let query_str = "SELECT ?s ?p ?o WHERE { ?s ?p ?o . }";
let query = SparqlParser::new().parse_query(query_str).unwrap();
assert_eq!(query.to_string(), query_str);
```

## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or
  `<http://www.apache.org/licenses/LICENSE-2.0>`)
* MIT license ([LICENSE-MIT](../LICENSE-MIT) or
  `<http://opensource.org/licenses/MIT>`)

at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Oxigraph by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
