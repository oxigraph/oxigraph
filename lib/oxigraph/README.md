# Incroxigraph Rust library

This is the Incroxigraph database engine, derived from
[Oxigraph](https://github.com/oxigraph/oxigraph). It provides RDF stores, ordinary
SPARQL queries and updates, parsing and serialization, and incremental queries.
The public Rust crate is named `incroxigraph`.

The [Python bindings](https://github.com/maartyman/incroxigraph/tree/main/python)
and [JavaScript bindings](https://github.com/maartyman/incroxigraph/tree/main/js)
expose the same engine through their own interfaces.

## Usage

The main entry point is `incroxigraph::store::Store`. `Store::new()` creates an
in-memory store; `Store::open(path)` opens a persistent database.

```rust
use incroxigraph::model::*;
use incroxigraph::sparql::{QueryResults, SparqlEvaluator};
use incroxigraph::store::Store;

let store = Store::new().unwrap();

// insertion
let ex = NamedNode::new("http://example.com").unwrap();
let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), GraphName::DefaultGraph);
store.insert(quad.clone()).unwrap();

// quad filter
let results = store
    .quads_for_pattern(Some(&ex.clone().into()), None, None, None)
    .collect::<Result<Vec<Quad>, _>>()
    .unwrap();
assert_eq!(vec![quad], results);

// SPARQL query
if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
    .parse_query("SELECT ?s WHERE { ?s ?p ?o }")
    .unwrap()
    .on_store(&store)
    .execute()
    .unwrap()
{
    assert_eq!(
        solutions.next().unwrap().unwrap().get("s"),
        Some(&ex.into())
    );
}
```

## Incremental queries

An incremental query keeps track of a `SELECT`, `ASK`, or `CONSTRUCT` result as the store changes.
Bind a parsed query with `on_store(&store)`, then choose complete result snapshots or result deltas.
The query observes committed store changes.

Use `execute_incremental_results()` when you want the current complete result after each change:

```rust
use incroxigraph::model::{GraphName, NamedNode, Quad};
use incroxigraph::sparql::{
    IncrementalQueryResults, SparqlEvaluator, StoreIncrementalQueryResultsState,
};
use incroxigraph::store::Store;

let store = Store::new().unwrap();
let mut query: StoreIncrementalQueryResultsState = SparqlEvaluator::new()
    .parse_query("SELECT ?s WHERE { ?s ?p ?o }")
    .unwrap()
    .on_store(&store)
    .execute_incremental_results()
    .unwrap();

if let IncrementalQueryResults::Solutions(rows) = query.results().unwrap() {
    assert_eq!(rows.count(), 0); // Initial snapshot
}

let ex = NamedNode::new("http://example.com").unwrap();
let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), GraphName::DefaultGraph);
let mut transaction = store.start_transaction().unwrap();
transaction.insert(quad.clone());
transaction.commit().unwrap();
if let IncrementalQueryResults::Solutions(rows) = query.results().unwrap() {
    assert_eq!(rows.count(), 1); // Complete result after the insertion
}
```

Use `execute_incremental_deltas()` when you maintain your own result set and only need changes since the previous call:

```rust
use incroxigraph::model::{GraphName, NamedNode, Quad};
use incroxigraph::sparql::{Delta, QueryResultsDelta, SparqlEvaluator};
use incroxigraph::store::Store;

let store = Store::new().unwrap();
let mut query = SparqlEvaluator::new()
    .parse_query("SELECT ?s WHERE { ?s ?p ?o }").unwrap()
    .on_store(&store)
    .execute_incremental_deltas().unwrap();

if let QueryResultsDelta::Solutions(initial) = query.deltas().unwrap() {
    assert_eq!(initial.count(), 0);
}

let ex = NamedNode::new("http://example.com").unwrap();
let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), GraphName::DefaultGraph);
let mut transaction = store.start_transaction().unwrap();
transaction.insert(quad.clone());
transaction.commit().unwrap();
if let QueryResultsDelta::Solutions(mut changes) = query.deltas().unwrap() {
    assert!(matches!(changes.next(), Some(Ok(Delta::Addition(_)))));
}
```

For `ASK`, complete results are `IncrementalQueryResults::Boolean(bool)`; deltas emit the initial boolean and later values only when it changes.
For `CONSTRUCT`, complete results are `IncrementalQueryResults::Graph`, while deltas contain triple additions and deletions.
Graph results have set semantics. Incremental `DESCRIBE` is not supported.

Both states also offer `iter_results()` or `iter_deltas()` to asynchronously await changes with `next().await`.
Each call drains changes currently ready, so several commits may be combined into one update.
Complete snapshots borrow the state: finish using a snapshot before calling `results()` again.
If you need an explicit type for a store-bound state, use `StoreIncrementalQueryResultsState` or `StoreIncrementalQueryDeltasState`.
For example, a struct can store the results handle in a field of type `StoreIncrementalQueryResultsState`.

The current incremental evaluator supports quad patterns, joins, projection and
VALUES. Optimized plans containing unsupported operators return errors. Ordinary
queries use the broader SPARQL implementation.

## Build from source

Initialize the repository submodules with `git submodule update --init --recursive`.
Native builds with RocksDB require a C++ toolchain and Clang in addition to Rust.
From the repository root:

```sh
cargo build -p incroxigraph
```

To use this checkout from another Rust project, add a path dependency:

```toml
[dependencies]
incroxigraph = { path = "/path/to/incroxigraph/lib/oxigraph" }
```

The `rocksdb` feature is enabled by default for native persistent storage. Set
`default-features = false` on the dependency for an in-memory-only build.
WebAssembly builds use the in-memory backend.

Optional features include `rdf-12` for RDF/SPARQL 1.2 support, `geosparql` for
GeoSPARQL functions, and `http-client` for federated queries and remote loading.
TLS variants of the HTTP client are listed in `Cargo.toml`.

See the [workspace overview](../README.md) for the supporting crates and
[store benchmarks](benches/README.md) for benchmark commands and inputs.

## License

Apache-2.0 or MIT, at your option. See
[LICENSE-APACHE](https://github.com/maartyman/incroxigraph/blob/main/LICENSE-APACHE)
and [LICENSE-MIT](https://github.com/maartyman/incroxigraph/blob/main/LICENSE-MIT).
Upstream Oxigraph copyright and license notices are retained.
