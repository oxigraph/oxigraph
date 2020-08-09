## Master

### Added
- `QueryOptions` now allows settings the query dataset graph URIs (the SPARQL protocol `default-graph-uri` and `named-graph-uri` parameters).
- `pyoxigraph` store `query` methods allows to provide the dataset graph URIs. It also provides an option to use all graph names as the default graph.
- "default graph as union option" now works with FROM NAMED.

## [0.1.0-rc.1] - 2020-08-01

### Added
- `oxigraph` Rust library with SPARQL 1.1 query support and memory, Sled and RocksDB stores.
- `oxigraph_server` standalone SPARQL server.
- `oxigraph_wikibase` standalone SPARQL server loading data from a Wikibase instance.
- `pyoxigraph` Python library based on Oxigraph.
- `oxigraph` NodeJS library based on Oxigraph.
