Oxigraph
========

[![actions status](https://github.com/Tpt/oxigraph/workflows/build/badge.svg)](https://github.com/Tpt/oxigraph/actions)
[![dependency status](https://deps.rs/repo/github/Tpt/oxigraph/status.svg)](https://deps.rs/repo/github/Tpt/oxigraph)


Oxigraph is a work in progress graph database implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.

There is no released version yet.

Its goal is to provide a compliant, safe and fast graph database based on the [RocksDB](https://rocksdb.org/) key-value store.
It is written in Rust.

It is split into multiple parts:
* The `lib` directory contains the database written as a Rust library
* The `server` directory contains a stand-alone binary of a web server implementing the [SPARQL 1.1 Protocol](https://www.w3.org/TR/sparql11-protocol/).
* The `wikibase` directory contains a stand-alone binary of a web server able to synchronize with a [Wikibase instance](https://wikiba.se/).

Are currently implemented:
* [SPARQL 1.1 Query](https://www.w3.org/TR/sparql11-query/) except `FROM` and `FROM NAMED`.
* [Turtle](https://www.w3.org/TR/turtle/), [TriG](https://www.w3.org/TR/trig/), [N-Triples](https://www.w3.org/TR/n-triples/), [N-Quads](https://www.w3.org/TR/n-quads/) and [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/) RDF serialization formats for both data ingestion and retrieval using the [Rio library](https://github.com/Tpt/rio).
* [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/) and [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/).

## Run the web server

### Build
You need to have [a recent stable version of Rust and Cargo installed](https://www.rust-lang.org/tools/install).

If it's done, executing `cargo build --release` in the root directory of this repository should compile the full server after having downloaded its dependencies.
It will create a fat binary in `target/release/oxigraph_server`.

### Usage

Run `./oxigraph_server` to start the server. It listen by default on `localhost:7878`.

The server provides an HTML UI with a form to execute SPARQL requests.

It provides the following routes:
* `/` allows to `POST` data to the server.
  For example `curl -f -X POST -H 'Content-Type:application/n-triples' --data-binary "@MY_FILE.nt" http://localhost:7878/`
  will add the N-Triples file MY_FILE.nt to the server repository. [Turtle](https://www.w3.org/TR/turtle/), [TriG](https://www.w3.org/TR/trig/), [N-Triples](https://www.w3.org/TR/n-triples/), [N-Quads](https://www.w3.org/TR/n-quads/) and [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/) are supported.
* `/query` allows to evaluate SPARQL queries against the server repository following the [SPARQL 1.1 Protocol](https://www.w3.org/TR/sparql11-protocol/#query-operation).
  For example `curl -f -X POST -H 'Content-Type:application/sparql-query' --data 'SELECT * WHERE { ?s ?p ?o } LIMIT 10' http://localhost:7878/query`.
  This route supports content negotiation and could return [Turtle](https://www.w3.org/TR/turtle/), [N-Triples](https://www.w3.org/TR/n-triples/), [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/), [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/) and [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/).


Use `oxigraph_server --help` to see the possible options when starting the server.


## Run the web server for Wikibase

### Build
You need to have [a recent stable version of Rust and Cargo installed](https://www.rust-lang.org/tools/install).

If it's done, executing `cargo build --release` in the root directory of this repository should compile the full server after having downloaded its dependencies.
It will create a fat binary in `target/release/oxigraph_wikibase`.

### Usage

To start a server that is synchronized with [test.wikidata.org](https://test.wikidata.org) you should run:
```bash
./oxigraph_wikibase --mediawiki_api=https://test.wikidata.org/w/api.php --mediawiki_base_url=https://test.wikidata.org/wiki/ --namespaces=0,120 --file=test.wikidata
```

It creates a SPARQL endpoint listening to `localhost:7878/query` that could be queried just like Blazegraph.

The configuration parameters are:
* `mediawiki_api` URL of the MediaWiki API to use
* `mediawiki_base_url` Base URL of MediaWiki pages like `https://test.wikidata.org/wiki/` for test.wikidata.org or `http://localhost/w/index.php?title=` for "vanilla" installations.
* `namespaces` The ids of the Wikibase namespaces to synchronize with, separated by `,`.
* `file` Path of where Oxigraph should store its data.


## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)
   
at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Futures by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
