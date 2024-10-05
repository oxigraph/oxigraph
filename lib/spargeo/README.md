spargeo
=======

[![Latest Version](https://img.shields.io/crates/v/spargeo.svg)](https://crates.io/crates/spargeo)
[![Released API docs](https://docs.rs/spargeo/badge.svg)](https://docs.rs/spargeo)
[![Crates.io downloads](https://img.shields.io/crates/d/spargeo)](https://crates.io/crates/spargeo)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community)

spargeo is a very partial, slow and WIP [GeoSPARQL](https://docs.ogc.org/is/22-047r1/22-047r1.html) implementation for Oxigraph.

Its entry point is the [`register_geosparql_functions`] function that allows to register GeoSPARQL extension function into Oxigraph [`QueryOptions`](oxigraph::sparql::QueryOptions).

## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or
  `<http://www.apache.org/licenses/LICENSE-2.0>`)
* MIT license ([LICENSE-MIT](../LICENSE-MIT) or
  `<http://opensource.org/licenses/MIT>`)

at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Oxigraph by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
