oxsdatatypes
============

[![Latest Version](https://img.shields.io/crates/v/oxsdatatypes.svg)](https://crates.io/crates/oxsdatatypes)
[![Released API docs](https://docs.rs/oxsdatatypes/badge.svg)](https://docs.rs/oxsdatatypes)
[![Crates.io downloads](https://img.shields.io/crates/d/oxsdatatypes)](https://crates.io/crates/oxsdatatypes)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community)

oxsdatatypes is an implementation of some [XML Schema Definition Language Datatypes](https://www.w3.org/TR/xmlschema11-2/).
Its main aim is to ease the implementation of SPARQL and XPath.

Usage example:

```rust
use std::str::FromStr;
use oxsdatatypes::Decimal;

assert!(Decimal::from_str("22.2").unwrap() > Decimal::from_str("21").unwrap());
```

Each datatype is represented by a Rust struct.

Each datatype provides:
* `FromStr` implementation to parse a datatype string serialization following its [lexical mapping](https://www.w3.org/TR/xmlschema11-2/#dt-lexical-mapping).
* `Display` implementation to serialize a datatype following its [canonical mapping](https://www.w3.org/TR/xmlschema11-2/#dt-canonical-mapping).
* `is_identical_with` method following its [identity relation](https://www.w3.org/TR/xmlschema11-2/#identity).
* `PartialEq`, and `Eq` if possible, implementations following its [equality relation](https://www.w3.org/TR/xmlschema11-2/#equality).
* `PartialOrd`, and `Ord` if possible, implementations following its [order relation](https://www.w3.org/TR/xmlschema11-2/#order).
* `From` and `TryFrom` implementations to implement [XPath casting](https://www.w3.org/TR/xpath-functions-31/#casting).
* Various methods implementing [XPath functions](https://www.w3.org/TR/xpath-functions-31/).
* `from_be_bytes` and `to_be_bytes` methods for serialization.


### `DateTime::now` behavior

The `DateTime::now()` function needs special OS support.
Currently:
- If the `custom-now` feature is enabled, a function computing `now` must be set:
  ```rust
  use oxsdatatypes::Duration;
  
  #[unsafe(no_mangle)]
  fn custom_ox_now() -> Duration {
    unimplemented!("now implementation")
  }
  ```
- For `wasm32-unknown-unknown` if the `js` feature is enabled the `Date.now()` ECMAScript API is used.
- For `zkvm` there is no access to IO functions so the function returns start of unix time (Jan 1, 1970)
- For all other targets `SystemTime::now()` is used.

## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or
  `<http://www.apache.org/licenses/LICENSE-2.0>`)
* MIT license ([LICENSE-MIT](../LICENSE-MIT) or
  `<http://opensource.org/licenses/MIT>`)

at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Oxigraph by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
