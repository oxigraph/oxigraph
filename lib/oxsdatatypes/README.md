oxsdatatypes
============

[![Latest Version](https://img.shields.io/crates/v/oxsdatatypes.svg)](https://crates.io/crates/oxsdatatypes)
[![Released API docs](https://docs.rs/oxsdatatypes/badge.svg)](https://docs.rs/oxsdatatypes)
[![Crates.io downloads](https://img.shields.io/crates/d/oxsdatatypes)](https://crates.io/crates/oxsdatatypes)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge)

oxsdatatypes is an implementation of some [XML Schema Definition Language Datatypes](https://www.w3.org/TR/xmlschema11-2/).
Its main aim is to ease the implementation of SPARQL or XPath.

Usage example:

```rust
use std::str::FromStr;
use oxsdatatypes::Decimal;

assert!(Decimal::from_str("22.2").unwrap() > Decimal::from_str("21").unwrap());
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
