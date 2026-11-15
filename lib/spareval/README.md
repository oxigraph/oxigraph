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
let evaluator = QueryEvaluator::new();
let results = evaluator.prepare(&query).execute(&dataset);
if let QueryResults::Solutions(solutions) = results.unwrap() {
    let solutions = solutions.collect::<Result<Vec<_>, _>>().unwrap();
    assert_eq!(solutions.len(), 1);
    assert_eq!(solutions[0]["s"], ex);
}
```

## Incremental evaluation

Incroxigraph extends prepared queries with incremental evaluation for SELECT,
ASK and CONSTRUCT. After `QueryEvaluator::prepare()`, choose one of these methods:

| Method | State returned | Reading results |
| --- | --- | --- |
| `execute_incremental_results(dataset)` | `IncrementalQueryResultsState` | `results()` returns the complete current answer |
| `execute_incremental_deltas(dataset)` | `IncrementalQueryDeltasState` | `deltas()` consumes additions and deletions since the previous read |

SELECT deltas contain `Delta<QuerySolution>` values and preserve duplicate rows.
CONSTRUCT deltas contain `Delta<Triple>` values with set semantics. ASK emits its
initial boolean, including false, and later values only when the answer changes.
Initial SELECT rows and CONSTRUCT triples are additions.

Snapshots borrow their state, so finish using them before requesting another
snapshot. Delta batches own their data and can outlive the state. Both state
objects also provide asynchronous access through `iter_results()` or
`iter_deltas()` and `next().await`. Reads process the changes currently available;
several updates can be combined into one result. Drop the state after an
evaluation error.

### Dataset integration

Incremental execution requires `IncrementalQueryableDataset`, which extends
`QueryableDataset`. Its default implementation exposes existing quads as additions
and then ends. The implementation for `&oxrdf::Dataset` therefore provides an
initial result rather than a live subscription.

A custom live dataset must implement `internal_quad_deltas_for_pattern()` to
provide matching additions and deletions. Its iterator returns
`StreamingItem::Pending` when no change is ready and `None` only when permanently
exhausted. Use the supplied weak `IncrementalQueryNotifier` to notify waiting
consumers when new changes become available.

The `oxigraph` store adapter provides this integration for committed store
changes. See its [incremental query examples](../oxigraph/README.md#incremental-queries)
for complete snapshots, result deltas and atomic transactions.

Advanced SELECT integrations can use `execute_incremental_driver()` for the
lower-level polling interface.

### Current limits

The incremental evaluator currently supports quad patterns, joins, projection
and VALUES. Optimized plans retaining unsupported operators, such as FILTER,
OPTIONAL, UNION, aggregates or property paths, return an evaluation error.
Incremental DESCRIBE is unsupported. Ordinary `execute()` retains the broader
SPARQL evaluator.

## Cargo features
- `sparql-12`: enables [SPARQL 1.2](https://www.w3.org/TR/sparql12-query/) changes.
- `sep-0002`: enables the [`SEP-0002`](https://github.com/w3c/sparql-dev/blob/main/SEP/SEP-0002/sep-0002.md) (`ADJUST` function and a lot of arithmetic on `xsd:date`, `xsd:time`, `xsd:yearMonthDuration` and `xsd:dayTimeDuration`).
- `sep-0006`: enables the [`SEP-0006`](https://github.com/w3c/sparql-dev/blob/main/SEP/SEP-0006/sep-0006.md) (`LATERAL` keyword). 
- `calendar-ext`: arithmetic on `xsd:gYear`, `xsd:gYearMonth`, `xsd:gMonth`, `xsd:gMonthDay` and `xsd:gDay`.
- `geosparql`: pre-registers the [GeoSPARQL 1.1](https://docs.ogc.org/is/22-047r1/22-047r1.html) extension functions from the [`spargeo`](https://crates.io/crates/spargeo) crate so `QueryEvaluator::new()` evaluates `geof:` predicates without an explicit `with_custom_function` call per function.

## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](https://github.com/maartyman/incroxigraph/blob/main/LICENSE-APACHE) or
  `<http://www.apache.org/licenses/LICENSE-2.0>`)
* MIT license ([LICENSE-MIT](https://github.com/maartyman/incroxigraph/blob/main/LICENSE-MIT) or
  `<http://opensource.org/licenses/MIT>`)

at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Oxigraph by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
