sparopt
=======

[![Latest Version](https://img.shields.io/crates/v/sparopt.svg)](https://crates.io/crates/sparopt)
[![Released API docs](https://docs.rs/sparopt/badge.svg)](https://docs.rs/sparopt)
[![Crates.io downloads](https://img.shields.io/crates/d/sparopt)](https://crates.io/crates/sparopt)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community)

sparopt is a work in progress [SPARQL Query](https://www.w3.org/TR/sparql11-query/) optimizer.

It relies on the output of [spargebra](https://crates.io/crates/spargebra).

Support for [SPARQL 1.2](https://www.w3.org/TR/sparql12-query/) is also available behind the `sparql-12` feature.

This crate is intended
to be a building piece for SPARQL implementations in Rust like [spareval](https://crates.io/crates/spareval)
used by [oxigraph](https://oxigraph.org).

The optimizer ensures that the rewritten query returns the exact same results as the input query.
However, it might discard some errors.
For example, the query:
```sparql
SELECT * WHERE { SERVICE <http://example.com/not-found> {} }
```
might get rewritten into:
```sparql
SELECT * WHERE {}
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
