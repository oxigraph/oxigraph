## Master

### Added
- [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/) support for Rust, Python and JavaScript.

## Changed
- Fixes evaluation of `MONTH()` and `DAY()` functions on the `xsd:date` values.
- `Variable::new` now validates the variable name.

## [0.1.1] - 2020-08-14

### Added
- The `"sophia"` feature implementing the [`sophia_api`](https://docs.rs/sophia_api/) traits on Oxigraph terms and stores.
- Explicit types for quads iterators returned by stores.

### Changed
- `QueryOptions::with_default_graph` now takes an `impl Into<GraphName>` instead of an `impl Into<NamedNode>`.
- `QueryOptions::with_named_graph` now takes an `impl Into<NamedOrBlankNode>` instead of an `impl Into<NamedNode>`.
- `pyoxigraph` `query` methods now takes two new parameters, `default_graph` and `named_graphs`. `default_graph_uris` and `named_graph_uris` parameters are deprecated.
- Fixes a bug in `xsd:gYear` parsing.


## [0.1.0] - 2020-08-09

### Added
- `QueryOptions` now allows settings the query dataset graph URIs (the SPARQL protocol `default-graph-uri` and `named-graph-uri` parameters).
- `pyoxigraph` store `query` methods allows to provide the dataset graph URIs. It also provides an option to use all graph names as the default graph.
- "default graph as union option" now works with FROM NAMED.
- `pyoxigraph` now exposes and documents `Variable`, `QuerySolution`, `QuerySolutions` and `QueryTriples`


## [0.1.0-rc.1] - 2020-08-08

### Added
- `oxigraph` Rust library with SPARQL 1.1 query support and memory, Sled and RocksDB stores.
- `oxigraph_server` standalone SPARQL server.
- `oxigraph_wikibase` standalone SPARQL server loading data from a Wikibase instance.
- `pyoxigraph` Python library based on Oxigraph.
- `oxigraph` NodeJS library based on Oxigraph.
