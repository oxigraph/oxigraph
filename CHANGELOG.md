## [0.3.22] - 2023-11-29

### Changed
- Allows to compile with more recent `bindgen` and `cc`
- Fixes compatibility with `spin_no_std` feature of `lazy_static`


## [0.3.21] - 2023-11-29

### Changed
- Bulk loader: do not fail when loading empty files.
- Python: fixes source distribution.
- Upgrades RocksDB to 7.8.1.


## [0.3.20] - 2023-10-23

### Changed
- SPARQL: fixes `STR` evaluation on small IRI (less than 16 bytes) that was broken.
- SPARQL update: fixes `WHERE` clause evaluation that was generating too many solutions in some cases.
- Upgrades RocksDB to 8.7.1.


## [0.3.19] - 2023-08-18

### Added
- Python: allows to give `pathlib.Path` argument when a path is expected.
- Cargo.toml: add a documentation field to link to docs.rs documentation.

### Changed
- Upgrades RocksDB to 8.3.2.


## [0.3.18] - 2023-06-13

### Changed
- SPARQL: fixes evaluation of `||` when all alternatives are `false`. The results must be `false` and not an error.
- SPARQL: `xsd:duration` it is now possible to properly parse and serialize all numbers that can be internally represented by the encoding used by Oxigraph.
- Python: fixes `Store.contains_named_graph` return type annotation.


## [0.3.17] - 2023-06-11

### Added
- SPARQL: support of `xsd:duration` to SPARQL `SUM` aggregate function.
- Server: support for systemd "notify" service option.
- Server: uses the target graph name as base URI when `POST`ing and `PUT`ing data.
- Python: `Store.contains_named_graph` method.

### Changed
- SPARQL: `xsd:decimal` multiplication and division have been improved to avoid raising overflow because of internal operations in some cases.
- SPARQL: `xsd:decimal` parser is now able to properly parse all numbers that can be internally represented by the encoding used by Oxigraph.
- SPARQL: `xsd:decimal` avoids bad overflow on unary `-` operator.
- SPARQL: reduces loss of precision when casting `xsd:decimal` to `xsd:float` or `xsd:double`.
- SPARQL: improves a bit the AST in case of a lot of `&&`, `||` or `IN` operators.
- SPARQL: fixes some optimization bugs around `MINUS` inside `OPTIONAL`.
- SPARQL: makes for loop join choice instead of hash join a bit more aggressive.
- Avoids random crashes when building RocksDB for aarch64. 
- Python: fixes support of text I/O with not-ASCII char.
- Python: upgrades PyO3 to 0.19.
- `spargebra`: `GroundTermPattern::Triple` is now properly gated behind the `"rdf-star"` feature.
- `oxsdatatypes`: Deprecates `is_naan` that is renamed to `is_nan` (we like bread but prefer numbers).
- `oxsdatatypes`: Adds `checked_neg` methods on numerical types.
- `oxsdatatypes`: Drops `nom` dependency and uses hand-written parsers instead.


## [0.3.16] - 2023-04-29

### Changed
- Fixes flush and compaction on the GSPO index. It might improve Oxigraph performances and storage space.
- SPARQL: fixes some optimizations in presence quoted triples with nested variables.
- SPARQL profiler: adds EXISTS operation to the explanation and profiling tree.
- Upgrades RocksDB to 8.1.1.


## [0.3.15] - 2023-04-18

### Added
- Rust: adds `Store.explain_query_opt` method that allows to get an explanation of the evaluation with profiling statistics.
- Server: adds explanation and profiling to the `query` action (`--explain`, `--explain-file` and `--stats` options).
- Python: implements the `__match_args__` attribute on terms (literals, triples, quads...) to allow `match` operator usage.
- Server: adds the `--cors` option to the `serve` actions to allow cross-origin resource sharing.

### Changed
- SPARQL: fixes evaluation of empty aggregation without GROUP BY: aggregators now return their default value (0 for COUNT...) and not an empty row.
- SPARQL: fixes parsing of decimal with more than 19 digits in the fractional part.
- Server docker image: switch to the smaller distroless base images.
- Bulk loader: by default only uses 2 concurrent threads and around 2GB of RAM.
- Server load: progress is now printed to stderr much more regularly.


## [0.3.14] - 2023-03-19

### Added
- Read only and secondary RocksDB storage. Allows to open the data as read-only or to follow a primary instance.
- Server: adds multiple commands:
  - `serve-secondary` and `serve-read-only` to serve the HTTP server in secondary and read-only modes.
  - `dump` to dump the database content to a file.
  - `query` and `update` to execute SPARQL query and updates.
  - `backup` to do a database backup.
  - `optimize` to optimize the database storage.
- Server: adds `format` and `graph` options to the `load` command in order to select the loaded data format and the target graph.
- Python: `Store.extend` and `Store.bulk_extend` methods.
- Python: allows to pickle basic data model classes.

### Changed
- Upgrades RocksDB to 8.0.0.
- Server: listening to localhost now properly listen to both IPv4 and IPv6.
- RDF/XML and XML parser results: avoid an ever growing buffer.
- JSON SPARQL results: allows the "head" key to be at the end of the document.
- TSV SPARQL results: properly quote `\t` and print trailing line jumps.
- `Term::from_str`: fixes parsing of blank nodes in object position of quoted triples.
- `QuerySolution`: implements `Eq` and `Debug`.
- JS: Reduces WASM build size.
- OxRDF: fixes `Graph` and `Dataset` serialization to output proper N-Triples and N-Quads.


## [0.3.13] - 2023-02-23

### Changed
- Fixes serialization of times with a decimal number of seconds lower than 10 (now `01:01:01.01` and not `01:01:1.01`).
- Turtle and TriG: fixes parsing for prefixes named after keywords (`prefix`, `base`, `graph`, `true` and `false`).


## [0.3.12] - 2023-02-18

### Added
- `From<NamedOrBlankNode>` for `GraphName` (and similarly for `*Ref`).
- Prebuilt Python wheels for Linux with [musl libc](https://www.musl-libc.org/).

### Changed
- Fixes TriG serialization.
- `QueryDataset` is now properly exposed in the public Rust API.


## [0.3.11] - 2023-01-18

### Added
- SPARQL: Implementation of the `LATERAL` operator following [SPARQL SEP-0006](https://github.com/w3c/sparql-12/blob/main/SEP/SEP-0006/sep-0006.md).
  Support is behind the `sep-0006` feature in `spargebra` and enabled by default in Oxigraph. 
- SPARQL: Implementation of the `ADJUST` function following [SPARQL SEP-0002](https://github.com/w3c/sparql-12/blob/main/SEP/SEP-0002/sep-0002.md).
  Support is behind the `sep-0002` feature in `spargebra` and enabled by default in Oxigraph.
- Rust: There is a new stand-alone crate `oxsdatatypes` implementing Rust structs for the common XML schema datatypes.
  It was part of the Oxigraph crate and it might be useful for other SPARQL or XPath implementations.
- Rust: The `oxigraph` crate can now be compiled for `wasm32-wasi` with the basic in-memory backend but without RocksDB.

### Changed
- SPARQL: The property path evaluator was sometime emitting duplicates when evaluating disjunctive patterns (`a|b`). It is now fixed.
- SPARQL: If written explicitly in the SPARQL query, the regular expressions are now compiled once and not for each row.
- SPARQL: Property path evaluation with both start and end variables bound has been optimized.
- SPARQL: Casts to `xsd:decimal` from `xsd:float` and `xsd:double` now properly fails on overflow instead of overflowing the internal 128-bits representation.
  This follows [XPath casting rules](https://www.w3.org/TR/xpath-functions-31/#casting-to-decimal).
- Rust: The minimal supported Rust version is set at 1.60 and enforced using the CI.
- Python: Local builds will now target the specific Python version instead of [abi3](https://docs.python.org/3/c-api/stable.html).
  abi3 wheels are still released on Pypi alongside new Python-version specific builds for Linux+GNU.
- SPARQL: Fixes a panic when the estimated upper bound of the results size was overflowing a `usize`.
- Python: Uses `typing.IO` in Python stubs instead of narrower interfaces. 
- Upgrades RocksDB to 7.9.2, `quick-xml` to 0.27 and `pyo3` to 0.18.


## [0.3.10] - 2022-12-21

### Added
- SPARQL: Property path with unbound graph are now evaluated.
  For example, `SELECT * WHERE { GRAPH ?g { ?s ex:p+ ?o } }` now works.
- SPARQL: The query optimizer is now fuzzed for better testing.

### Changed
- SPARQL: Evaluation of zero-length paths with both start and end unbounded now return only terms defined in the current graph but not terms only defined in the query.
  For example, in `SELECT * WHERE { VALUES ?s { 1 } { ?s ex:p? ?o } }` the literal `1` won't be returned anymore if not in the queried graph.
- Python: type stubs are now compatible with Python 3.7 and Mypy strict mode.
- RDF/XML: allows entities declared using other entities.
- Upgrades `quick-xml` to 0.26.


## [0.3.9] - 2022-12-07

### Added
- Server: The `/store` endpoints now has a `no_transaction` HTTP option for `POST` and `PUT` request to get better performances at the cost of transactional guarantees.
- Server: The `/store` endpoints now has a `lenient` HTTP option for `POST` and `PUT` request to ignore syntax errors (requires the  `no_transaction` option).
- Server: allows path that are not valid UTF-8 in file path CLI arguments.
- Rust: `From<spargebra::Query>` to `oxigraph::Query` (thanks to @hobofan).

### Changed
- SPARQL: `NOW()` function properly returns the current time and not 1970-01-01
- SPARQL: fixes serialization of SPARQL queries (property path and STRSTARTS function).
- SPARQL: slightly optimize aggregates by avoiding an unneeded projection.
- SPARQL: the parser now cleanly fails if invalid `VALUES` clauses are provided.
- SPARQL: In DELETE/INSERT UPDATE the currently written values can't be read anymore ("Halloween problem").
- `oxrdf`: makes Clippy run without warnings when `rdf-star` is disable.
- Python: makes type annotations compatible with Python 3.7.
- Python: makes sure the parameter default value is always included in the type annotation.


## [0.3.8] - 2022-10-22

### Changed
- Python: avoid uploading debug wheels to pypi.


## [0.3.7] - 2022-10-22

### Added
- Python type stubs are now generated and distributed alongside wheels.

### Changed
- SPARQL: Fixes evaluation of sub queries in which some variables might not be always bound.
- Python: Uses https://pyoxigraph.readthedocs.io instead of https://oxigraph.org/pyoxigraph/ to store pyoxigraph documentation.
- Server: Migration to Clap v4. Slight changes to the console textual outputs.


## [0.3.6] - 2022-08-16

### Changed
- Server: Makes sure body is always sent for all HTTP response codes that are not explicitly to not do that according to the HTTP specification.
- Python: Adds support of reading from `io.TextIOBase`.


## [0.3.5] - 2022-08-02

### Changed
- Fixes a bug in the SPARQL parser with nested anonymous blank nodes (`[]`).
- Migrates the Python documentation skin to [Furo](https://pradyunsg.me/furo/).


## [0.3.4] - 2022-07-17

### Changed
- Allows multiple value to be given to `oxigraph_server` `--file` parameter.
- Limits parallelism of `oxigraph_server` file loader to avoid stack overflows.


## [0.3.3] - 2022-06-12

### Added
- `QueryResults::with_http_redirection_limit` that allows to set a limit on the number of HTTP redirection followed (`0` by default).

### Changed
- Fixes a bug in SPARQL parser that was always enabling the `SILENT` option of the `SERVICE` operator.
- Allows unknown keys in the objects present in the SPARQL JSON query results. It allows to parse e.g. Virtuoso query results.
- Allows `"typed-literal"` as an alias of `"literal"` in SPARQL JSON query results. 
- Improves the HTTP client error reporting: no silent failures on 4XX and 5XX responses.
- Upgrades RocksDB to 7.3.1.
- Upgrades quick-xml to 0.23.


## [0.3.2] - 2022-04-24

### Changed
- Fixes a bug in the server bulk loader that crashed on very small files.
- Upgrades RocksDB to v7.1.
- Removes some dead code in the bulk loader.


## [0.3.1] - 2022-04-02

### Changed
- The default git branch is now `main` and not `master` (thanks to @nyurik).
- Upgrades RocksDB to v7.0.4.
- Limits the number of bulk loader threads to at most 4 (allows bigger BTree chunks and a better data layout).
- Limits the number of files opened by RocksDB to the soft file descriptor limit minus 48.


## [0.3.0] - 2022-03-19

### Changed
- Fixes compilation on ARM.
- Moves some lints from errors to warnings in order to avoid compilation failures on different Rust versions.


## [0.3.0-rc.1] - 2022-03-14

### Added
- The bulk loader now checks available memory and tries to increase its batch size to make use of it.
- The Bulk loader provides now a `--lenient` option to keep loading a file even if invalid data is found (works only with N-Triples and N-Quads). 
  This behavior can be customised in the Rust API using the `BulkLoader::on_parse_error` method.

### Changed
- Rocksdb has been upgrade to 7.0.2. It now requires a C++17 compatible compiler. This required dropping support of macOS 10.9 to 10.13.

## [0.3.0-beta.4] - 2022-02-27

### Added
- JS: Oxigraph NPM package is now also supporting web browsers and WebPack.
- JS: RDF term related classes now overrides the `toString` method.
- Python: It is now possible to directly give a file path to the
  `parse`, `serialize`, `Store.load`, `Store.bulk_load` and `Store.dump` functions.
- Python: New `Store.clear_graph`, `Store.clear`, `Store.optimize` and `Store.flush` methods.

### Removed
- `sophia_api` traits implementation following a request of Sophia maintainer.

### Changed
- SPARQL: fixes evaluation of SPARQL queries with no results but an `ORDER BY` clause.
  There should be no group in the output instead of one empty group.
  This behavior has been changed following [this discussion](https://github.com/w3c/rdf-tests/pull/61).
- SPARQL: fixes SPARQL-star evaluation of nested triples with both variables and constants.
- SPARQL: if results are sorted, literals are now ordered by value, then datatype, then language tag.
  This ordering is considered as "implementation defined" by the SPARQL specification and is very likely to change in the future.
- Python: all costly methods now release the python GIL allowing multithreaded usages of pyoxigraph.
- Rust: SPARQL results writer now flushes the buffer at the end of the results writes. This makes their API less error-prone.
- Rust: the bulk loader API has been rewritten to allow hooking a progress indicator and set parallelism limit.
- Server: it is now possible to bulk load gzipped files.

## [0.3.0-beta.3] - 2022-02-02

### Changed
- Fixes a bug in the `bulk_load_dataset` method that was creating an invalid database.
- Server: Takes into account also URL query parameters if the send SPARQL request body is using form-urlencoded.
- Upgrades RocksDB to v0.28.2.
- Generate clean python sdist files compiling Oxigraph from scratch with the proper `Cargo.lock`.
- Do not push beta releases to homebrew and python stable documentation.
- Moves RocksDB binding directory to `oxrocksdb-sys`.

## [0.3.0-beta.2] - 2022-01-29

### Changed
- Fixes release on crates.io of the RocksDB bindings.

## [0.3.0-beta.1] - 2022-01-29

### Added
- [RDF-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html) support. `Triple` is now a possible `Term`. Serialization formats and SPARQL support have been updated to match the [latest version of the specification draft](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html).
- Fast data bulk load with the `Store` `bulk_load_dataset` and `bulk_load_graph` methods and a special command line option of the server.
- It is now possible to quickly backup the database using the `backup` method.
- Rust: `*Syntax::from_extension` to easy guess a graph/dataset/sparql result format from a file extension.
- Rust: Custom SPARQL functions are now supported using `QueryOptions::with_custom_function`.
- Rust: Simple in-memory graph (`Graph`) and dataset (`Dataset`) data structures with canonicalization.
- Nightly build of the server binary and docker image, and of pyoxigraph wheels.
- `Store` operations are now transactional using the "repeatable read" isolation level:
  the store only exposes changes that have been "committed" (i.e. no partial writes) and the exposed state does not change for the complete duration of a read operation (e.g. a SPARQL query) or a read/write operation (e.g. a SPARQL update).
  the `Store` `transaction` method now allows to do read/write transactions.
-`RDF-star <https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html>`_ is now supported (including serialization formats and SPARQL-star). :py:class:`.Triple` can now be used in :py:attr:`.Triple.object`, :py:attr:`.Triple.object`, :py:attr:`.Quad.subject` and :py:attr:`.Quad.object`.

### Changed
- SPARQL: It is now possible to compare `rdf:langString` literals with the same language tag.
- SPARQL: The parser now validates more carefully the inputs following the SPARQL specification and test suite.
- SPARQL: Variable scoping was buggy with "FILTER EXISTS". It is now fixed.
- Rust: RDF model, SPARQL parser and SPARQL result parsers have been moved to stand-alone reusable libraries.
- Rust: HTTPS is not supported by default with the `http_client` option. You need to enable the `native-tls` or the `rustls` feature of the `oxhttp` crate to enable a TSL layer.
- Rust: The error types have been cleaned.
  Most of the `Store` methods now return a `StorageError` that is more descriptive than the previous `std::io::Error`.
  The new error type all implements `Into<std::io::Error>` for easy conversion.
- Rust: There is now a `Subject` struct that is the union of `NamedNode`, `BlankNode` and `Triple`.
  It is The used type of the `subject` field of the `Triple` and `Quad` structs.
- Rust: The SPARQL algebra is not anymore publicly exposed in the `oxigraph` crate. The new `oxalgebra` crate exposes it.
- Rust: `UpdateOptions` API have been rewritten. It can now be built using `From<QueryOptions>` or `Default`.
- Server: The command line API has been redesign. See the [server README](server/README.md) for more information.
- Server: The HTTP implementation is now provided by [`oxhttp`](https://github.com/oxigraph/oxhttp).
- Server: The HTTP response bodies are now generated on the fly instead of being buffered.
- Python: The `SledStore` and `MemoryStore` classes have been removed in favor of the `Store` class.
- JS: The `MemoryStore` class has been renamed to `Store`.
- JS: The [RDF/JS `DataFactory` interface](http://rdf.js.org/data-model-spec/#datafactory-interface) is now implemented by the `oxigraph` module itself and the `MemoryStore.dataFactory` property has been removed.
- The implementation of SPARQL evaluation has been improved for better performances (especially joins).
- The TLS implementation used in SPARQL HTTP calls is now [rustls](https://github.com/rustls/rustls) and not [native-tls](https://github.com/sfackler/rust-native-tls). The host system certificate registry is still used.
- Spargebra: The basic RDF terms are now the ones of the `oxrdf` crate.

### Removed
- `SledStore` and `MemoryStore`. There is only the `RocksDbStore` anymore that is renamed to `Store`.
- `oxigraph_wikibase` is now stored in [its own repository](https://github.com/oxigraph/oxigraph-wikibase).
- Rust: `From` implementations between `oxigraph` terms and `rio_api` terms.

Many thanks to [Thad Guidry](https://github.com/thadguidry), [James Overton](https://github.com/jamesaoverton) and [Jeremiah](https://github.com/jeremiahpslewis) who sponsored the project during the development of this version.


## [0.2.5] - 2021-07-11

### Added
- [SPARQL 1.1 Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/) parser.
- Python wheels for macOS are now universal2 binaries.

### Changed
- The `Cargo.lock` file is now provided with releases to avoid compilation failures because of changes in dependencies.
- Uses clap instead of argh for the server arguments parsing.
- Upgrades PyO3 to v0.14.


## [0.2.4] - 2021-04-28

### Changed
- The HTTP server allows to query the union of all graphs using the `union-default-graph` query parameter and to use the union graph for update `WHERE` clauses using the `using-union-graph` parameter.
- Exposes Sled flush operation (useful for platforms without auto-flush like Windows or Android).
- Fixes a possible out of bound panic in SPARQL query evaluation.
- Upgrades RocksDB to 6.17.3.


## [0.2.3] - 2021-04-11

### Changed
- Server: Fixes HTTP content negotiation (charset constraints, failure to properly handle `*/*`...).
- Makes Clippy 1.51 happy.


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
