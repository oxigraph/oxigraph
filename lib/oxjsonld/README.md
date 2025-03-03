OxJSON-LD
=========

[![Latest Version](https://img.shields.io/crates/v/oxjsonld.svg)](https://crates.io/crates/oxjsonld)
[![Released API docs](https://docs.rs/oxjsonld/badge.svg)](https://docs.rs/oxjsonld)
[![Crates.io downloads](https://img.shields.io/crates/d/oxjsonld)](https://crates.io/crates/oxjsonld)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community)

OxJSON-LD is a parser and serializer for [JSON-LD](https://www.w3.org/TR/json-ld/).

The entry points of this library are the two [`JsonLdParser`] and [`JsonLdSerializer`] structs.

TODO: notes

Usage example counting the number of people in a RDF/XML file:

```rust
use oxrdf::{NamedNodeRef, vocab::rdf};
use oxjsonld::RdfXmlParser;

let file = br#"<?xml version="1.0"?>
<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:schema="http://schema.org/">
 <rdf:Description rdf:about="http://example.com/foo">
   <rdf:type rdf:resource="http://schema.org/Person" />
   <schema:name>Foo</schema:name>
 </rdf:Description>
 <schema:Person rdf:about="http://example.com/bar" schema:name="Bar" />
</rdf:RDF>"#;

let schema_person = NamedNodeRef::new("http://schema.org/Person").unwrap();
let mut count = 0;
for triple in RdfXmlParser::new().for_reader(file.as_ref()) {
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
