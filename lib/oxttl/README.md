OxTTL
=====

[![Latest Version](https://img.shields.io/crates/v/oxttl.svg)](https://crates.io/crates/oxttl)
[![Released API docs](https://docs.rs/oxttl/badge.svg)](https://docs.rs/oxttl)
[![Crates.io downloads](https://img.shields.io/crates/d/oxttl)](https://crates.io/crates/oxttl)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community)

Oxttl is a set of parsers and serializers for [Turtle](https://www.w3.org/TR/turtle/), [TriG](https://www.w3.org/TR/trig/), [N-Triples](https://www.w3.org/TR/n-triples/), [N-Quads](https://www.w3.org/TR/n-quads/) and [N3](https://w3c.github.io/N3/spec/).

Support for [RDF 1.2](https://www.w3.org/TR/rdf12-concepts/) is available behind the `rdf-12` feature for all languages but N3.

It is designed as a low level parser compatible with both synchronous and asynchronous I/O.

Usage example counting the number of people in a Turtle file:
```rust
use oxrdf::{NamedNodeRef, vocab::rdf};
use oxttl::TurtleParser;

let file = b"@base <http://example.com/> .
@prefix schema: <http://schema.org/> .
<foo> a schema:Person ;
    schema:name \"Foo\" .
<bar> a schema:Person ;
    schema:name \"Bar\" .";

let schema_person = NamedNodeRef::new("http://schema.org/Person").unwrap();
let mut count = 0;
for triple in TurtleParser::new().for_reader(file.as_ref()) {
    let triple = triple.unwrap();
    if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
        count += 1;
    }
}
assert_eq!(2, count);
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
