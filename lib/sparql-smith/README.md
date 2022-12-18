SPARQL smith
============

[![Latest Version](https://img.shields.io/crates/v/sparql-smith.svg)](https://crates.io/crates/sparql-smith)
[![Released API docs](https://docs.rs/sparql-smith/badge.svg)](https://docs.rs/sparql-smith)
[![Crates.io downloads](https://img.shields.io/crates/d/sparql-smith)](https://crates.io/crates/sparql-smith)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge)

sparql-smith is a test case generator for the [SPARQL](https://www.w3.org/TR/sparql11-overview/) language.

It provides a single struct, `Query` that could be serialized to a SPARQL query using `to_string()`.

The queries generated are sadly not always valid. Variables scopes are not properly handled yet.
All SPARQL features are not supported yet.

The `DATA_TRIG` constant is provided as an example dataset on which queries could be evaluated.

Usage example with [libfuzzer-sys](https://docs.rs/libfuzzer-sys) and [spargebra](https://docs.rs/spargebra):

```rust
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: sparql_smith::Query| {
    spargebra::Query::parse(&data.to_string(), None).unwrap()
});
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
