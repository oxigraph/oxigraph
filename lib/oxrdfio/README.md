OxRDF I/O
=========

[![Latest Version](https://img.shields.io/crates/v/oxrdfio.svg)](https://crates.io/crates/oxrdfio)
[![Released API docs](https://docs.rs/oxrdfio/badge.svg)](https://docs.rs/oxrdfio)
[![Crates.io downloads](https://img.shields.io/crates/d/oxrdfio)](https://crates.io/crates/oxrdfio)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community)

OxRDF I/O is a set of parsers and serializers for RDF.

It supports:
* [N3](https://w3c.github.io/N3/spec/) using [`oxttl`](https://crates.io/crates/oxttl)
* [N-Quads](https://www.w3.org/TR/n-quads/) using [`oxttl`](https://crates.io/crates/oxttl)
* [N-Triples](https://www.w3.org/TR/n-triples/) using [`oxttl`](https://crates.io/crates/oxttl)
* [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/) using [`oxrdfxml`](https://crates.io/crates/oxrdfxml)
* [TriG](https://www.w3.org/TR/trig/) using [`oxttl`](https://crates.io/crates/oxttl)
* [Turtle](https://www.w3.org/TR/turtle/) using [`oxttl`](https://crates.io/crates/oxttl)

Support for [SPARQL-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html) is also available behind the `rdf-star`feature for [Turtle-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#turtle-star), [TriG-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#trig-star), [N-Triples-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#n-triples-star) and [N-Quads-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#n-quads-star).

It is designed as a low level parser compatible with both synchronous and asynchronous I/O (behind the `async-tokio` feature).

The entry points of this library are the two [`RdfParser`] and [`RdfSerializer`] structs.

Usage example converting a Turtle file to a N-Triples file:
```rust
use oxrdfio::{RdfFormat, RdfParser, RdfSerializer};

let turtle_file = b"@base <http://example.com/> .
@prefix schema: <http://schema.org/> .
<foo> a schema:Person ;
    schema:name \"Foo\" .
<bar> a schema:Person ;
    schema:name \"Bar\" .";

let ntriples_file = b"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
<http://example.com/foo> <http://schema.org/name> \"Foo\" .
<http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
<http://example.com/bar> <http://schema.org/name> \"Bar\" .
";

let mut writer = RdfSerializer::from_format(RdfFormat::NTriples).serialize_to_write(Vec::new());
for quad in RdfParser::from_format(RdfFormat::Turtle).parse_read(turtle_file.as_ref()) {
    writer.write_quad(&quad.unwrap()).unwrap();
}
assert_eq!(writer.finish().unwrap(), ntriples_file);
```

Parsers for other RDF formats exists in Rust like [graph-rdfa-processor](https://github.com/nbittich/graph-rdfa-processor) for RDFa and [json-ld](https://github.com/timothee-haudebourg/json-ld) for JSON-LD.


## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or
  `<http://www.apache.org/licenses/LICENSE-2.0>`)
* MIT license ([LICENSE-MIT](../LICENSE-MIT) or
  `<http://opensource.org/licenses/MIT>`)

at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Oxigraph by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
