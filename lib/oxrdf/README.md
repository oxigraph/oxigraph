OxRDF
=====

[![Latest Version](https://img.shields.io/crates/v/oxrdf.svg)](https://crates.io/crates/oxrdf)
[![Released API docs](https://docs.rs/oxrdf/badge.svg)](https://docs.rs/oxrdf)
[![Crates.io downloads](https://img.shields.io/crates/d/oxrdf)](https://crates.io/crates/oxrdf)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge)

OxRDF is a simple library providing datastructures encoding [RDF 1.1 concepts](https://www.w3.org/TR/rdf11-concepts/).

This crate is intended to be a basic building block of other crates like [Oxigraph](https://crates.io/crates/oxigraph) or [Spargebra](https://crates.io/crates/spargebra).

Support for [RDF-star](https://w3c.github.io/rdf-star/cg-spec/) is available behind the `rdf-star` feature.

OxRDF is inspired by [RDF/JS](https://rdf.js.org/data-model-spec/) and [Apache Commons RDF](http://commons.apache.org/proper/commons-rdf/).

Usage example:

```rust
use oxrdf::*;

let mut graph = Graph::default();

// insertion
let ex = NamedNodeRef::new("http://example.com").unwrap();
let triple = TripleRef::new(ex, ex, ex);
graph.insert(triple);

// simple filter
let results: Vec<_> = graph.triples_for_subject(ex).collect();
assert_eq!(vec![triple], results);
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
