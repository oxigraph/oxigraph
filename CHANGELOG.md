## [0.2.2] - 2021-03-18

### Added
- Support of XML entities to the RDF/XML parser

### Changed
- Serve: Allows unsupported query parameters in HTTP SPARQL requests. 
- Fixes WASM compilation bug and optimises WASM release packages.
- Fixes named graph creation inside of a SledStore transaction.

## [0.2.1] - 2021-01-16

### Changed
- Fixes `pyoxigraph` build by enforcing a given `maturin` version.
- Adds code to build Python wheels for MacOS and Windows.


## [0.2.0] - 2021-01-07

### Added
- [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/) support for Rust, Python and JavaScript. All store-like classes now provide an `update` method.
- [SPARQL 1.1 Query Results CSV and TSV Formats](https://www.w3.org/TR/sparql11-results-csv-tsv/) serializers and TSV format parser.
- [SPARQL 1.1 Graph Store HTTP Protocol](https://www.w3.org/TR/sparql11-http-rdf-update/) partial support in `oxigraph_server`. This protocol is accessible under the `/store` path.
- The SPARQL Query and Update algebra is now public.
- The stores are now "graph aware" i.e. it is possible to create and keep empty named graphs.
- A simple built-in HTTP client. In the Rust library, is disabled by default behind the `http_client` feature. It powers SPARQL federation and SPARQL UPDATE `LOAD` operations.
- `std::str::FromStr` implementations to `NamedNode`, `BlankNode`, `Literal`, `Term` and `Variable` allowing to easily parse Turtle/SPARQL serialization of these terms.
- Optional Sled storage for `oxigraph_server`.

### Removed
- The `default_graph_uris` and `named_graph_uris` parameters from `pyoxigraph` `query` methods.
- Python 3.5 support.
- `(Memory|RocksDB|Sled)Store::prepare_query` methods. It is possible to cache SPARQL query parsing using the `Query::parse` function and give the parsed query to the `query` method.

### Changed
- Loading data into `oxigraph_server` is now possible using `/store` and not anymore using `/`.
  For example, you should use now `curl -f -X POST -H 'Content-Type:application/n-quads' --data-binary "@MY_FILE.nq" http://localhost:7878/store` to add the N-Quads file MY_FILE.nt to the server dataset.
- Fixes evaluation of `MONTH()` and `DAY()` functions on the `xsd:date` values.
- `Variable::new` now validates the variable name.
- `(Memory|RocksDB|Sled)Store::query` does not have an option parameter anymore. There is now a new `query_opt` method that allows giving options.
- `xsd:boolean` SPARQL function now properly follows XPath specification.
- Fixes SPARQL `DESCRIBE` evaluation.

### Disk data format

The disk data format has been changed between Oxigraph 0.1 (version 0) and Oxigraph 0.2 (version 1). Data is automatically migrated from the version 0 format to the version 1 format when opened with Oxigraph 0.2.


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
