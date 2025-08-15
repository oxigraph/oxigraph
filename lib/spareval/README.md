spareval
========

[![Latest Version](https://img.shields.io/crates/v/spareval.svg)](https://crates.io/crates/spareval)
[![Released API docs](https://docs.rs/spareval/badge.svg)](https://docs.rs/spareval)
[![Crates.io downloads](https://img.shields.io/crates/d/spareval)](https://crates.io/crates/spareval)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community)

spareval is a [SPARQL Query](https://www.w3.org/TR/sparql11-query/) evaluator.

It relies on the [spargebra](https://crates.io/crates/spargebra) and [sparopt](https://crates.io/crates/sparopt) crates.

This crate is intended
to be a building piece for SPARQL implementations like [oxigraph](https://oxigraph.org).

```rust
use oxrdf::{Dataset, GraphName, NamedNode, Quad};
use spareval::{QueryEvaluator, QueryResults};
use spargebra::SparqlParser;

let ex = NamedNode::new("http://example.com").unwrap();
let dataset = Dataset::from_iter([Quad::new(
    ex.clone(),
    ex.clone(),
    ex.clone(),
    GraphName::DefaultGraph,
)]);
let query = SparqlParser::new().parse_query("SELECT * WHERE { ?s ?p ?o }").unwrap();
let results = QueryEvaluator::new().execute(&dataset, &query);
if let QueryResults::Solutions(solutions) = results.unwrap() {
    let solutions = solutions.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(solutions.len(), 1);
    assert_eq!(solutions[0]["s"], ex.into());
}
```

## Cargo features
- `sparql-12`: enables [SPARQL 1.2](https://www.w3.org/TR/sparql12-query/) changes.
- `sep-0002`: enables the [`SEP-0002`](https://github.com/w3c/sparql-dev/blob/main/SEP/SEP-0002/sep-0002.md) (`ADJUST` function and a lot of arithmetic on `xsd:date`, `xsd:time`, `xsd:yearMonthDuration` and `xsd:dayTimeDuration`).
- `sep-0006`: enables the [`SEP-0006`](https://github.com/w3c/sparql-dev/blob/main/SEP/SEP-0006/sep-0006.md) (`LATERAL` keyword). 
- `calendar-ext`: arithmetic on `xsd:gYear`, `xsd:gYearMonth`, `xsd:gMonth`, `xsd:gMonthDay` and `xsd:gDay`.

## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or
  `<http://www.apache.org/licenses/LICENSE-2.0>`)
* MIT license ([LICENSE-MIT](../LICENSE-MIT) or
  `<http://opensource.org/licenses/MIT>`)

at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Oxigraph by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
