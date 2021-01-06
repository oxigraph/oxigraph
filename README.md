Oxigraph
========

[![Latest Version](https://img.shields.io/crates/v/oxigraph.svg)](https://crates.io/crates/oxigraph) 
[![Released API docs](https://docs.rs/oxigraph/badge.svg)](https://docs.rs/oxigraph)
[![PyPI](https://img.shields.io/pypi/v/pyoxigraph)](https://pypi.org/project/pyoxigraph/)
[![npm](https://img.shields.io/npm/v/oxigraph)](https://www.npmjs.com/package/oxigraph)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge)

Oxigraph is a graph database implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.

Its goal is to provide a compliant, safe, and fast graph database based on the [RocksDB](https://rocksdb.org/) and [Sled](https://sled.rs/) key-value stores.
It is written in Rust.
It also provides a set of utility functions for reading, writing, and processing RDF files.

Oxigraph is in heavy development and SPARQL query evaluation has not been optimized yet.

It is split into multiple parts:
* The `lib` directory contains the database written as a Rust library.
[![Latest Version](https://img.shields.io/crates/v/oxigraph.svg)](https://crates.io/crates/oxigraph) 
[![Released API docs](https://docs.rs/oxigraph/badge.svg)](https://docs.rs/oxigraph)
* The `python` directory contains Pyoxigraph. Pyoxigraph allows using Oxigraph in Python. See the [Pyoxigraph website](https://oxigraph.org/pyoxigraph/) for Pyoxigraph documentation. [![PyPI](https://img.shields.io/pypi/v/pyoxigraph)](https://pypi.org/project/pyoxigraph/)
* The `js` directory contains bindings to use Oxigraph in JavaScript with the help of WebAssembly. See [its README](https://github.com/oxigraph/oxigraph/blob/master/js/README.md) for the JS bindings documentation.
[![npm](https://img.shields.io/npm/v/oxigraph)](https://www.npmjs.com/package/oxigraph)
* The `server` directory contains a stand-alone binary of a web server implementing the [SPARQL 1.1 Protocol](https://www.w3.org/TR/sparql11-protocol/) and the [SPARQL 1.1 Graph Store Protocol](https://www.w3.org/TR/sparql11-http-rdf-update/). It uses the [RocksDB](https://rocksdb.org/) key-value store.
[![Latest Version](https://img.shields.io/crates/v/oxigraph_server.svg)](https://crates.io/crates/oxigraph_server) 
[![Docker Image Version (latest semver)](https://img.shields.io/docker/v/oxigraph/oxigraph?sort=semver)](https://hub.docker.com/repository/docker/oxigraph/oxigraph)
* The `wikibase` directory contains a stand-alone binary of a web server able to synchronize with a [Wikibase instance](https://wikiba.se/).
[![Latest Version](https://img.shields.io/crates/v/oxigraph_wikibase.svg)](https://crates.io/crates/oxigraph_wikibase)
[![Docker Image Version (latest semver)](https://img.shields.io/docker/v/oxigraph/oxigraph-wikibase?sort=semver)](https://hub.docker.com/repository/docker/oxigraph/oxigraph-wikibase)

Oxigraph implements the following specifications:
* [SPARQL 1.1 Query](https://www.w3.org/TR/sparql11-query/), [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/), and [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/).
* [Turtle](https://www.w3.org/TR/turtle/), [TriG](https://www.w3.org/TR/trig/), [N-Triples](https://www.w3.org/TR/n-triples/), [N-Quads](https://www.w3.org/TR/n-quads/), and [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/) RDF serialization formats for both data ingestion and retrieval using the [Rio library](https://github.com/oxigraph/rio).
* [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/), [SPARQL 1.1 Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/) and [SPARQL 1.1 Query Results CSV and TSV Formats](https://www.w3.org/TR/sparql11-results-csv-tsv/).

A preliminary benchmark [is provided](bench/README.md).

## Run the Web server

### Installation

You need to have [a recent stable version of Rust and Cargo installed](https://www.rust-lang.org/tools/install). You also need [clang](https://clang.llvm.org/) to build RocksDB.

To download, build and install the latest released version run `cargo install oxigraph_server`.
There is no need to clone this git repository.

To compile the server from source, clone this git repository, and execute `cargo build --release` in the `server` directory to compile the full server after having downloaded its dependencies.
It will create a fat binary in `target/release/oxigraph_server`.

### Usage

Run `oxigraph_server -f my_data_storage_directory` to start the server where `my_data_storage_directory` is the directory where you want Oxigraph data to be stored in. It listens by default on `localhost:7878`.

The server provides an HTML UI with a form to execute SPARQL requests.

It provides the following REST actions:
* `/query` allows to evaluate SPARQL queries against the server repository following the [SPARQL 1.1 Protocol](https://www.w3.org/TR/sparql11-protocol/#query-operation).
  For example `curl -X POST -H 'Content-Type:application/sparql-query' --data 'SELECT * WHERE { ?s ?p ?o } LIMIT 10' http://localhost:7878/query`.
  This action supports content negotiation and could return [Turtle](https://www.w3.org/TR/turtle/), [N-Triples](https://www.w3.org/TR/n-triples/), [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/), [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/) and [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/).
* `/update` allows to execute SPARQL updates against the server repository following the [SPARQL 1.1 Protocol](https://www.w3.org/TR/sparql11-protocol/#update-operation).
  For example `curl -X POST -H 'Content-Type: application/sparql-update' --data 'DELETE WHERE { <http://example.com/s> ?p ?o }' http://localhost:7878/update`.
* `/store` allows to retrieve and change the server content using the [SPARQL 1.1 Graph Store HTTP Protocol](https://www.w3.org/TR/sparql11-http-rdf-update/).
  For example `curl -f -X POST -H 'Content-Type:application/n-triples' --data-binary "@MY_FILE.nt" http://localhost:7878/store?graph=http://example.com/g` will add the N-Triples file MY_FILE.nt to the server dataset inside of the `http://example.com/g` named graph.
  [Turtle](https://www.w3.org/TR/turtle/), [N-Triples](https://www.w3.org/TR/n-triples/) and [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/) are supported.
  It is also possible to `POST`, `PUT` and `GET` the complete RDF dataset on the server using RDF dataset formats ([TriG](https://www.w3.org/TR/trig/) and [N-Quads](https://www.w3.org/TR/n-quads/)) against the `/store` endpoint.
  For example `curl -f -X POST -H 'Content-Type:application/n-quads' --data-binary "@MY_FILE.nq" http://localhost:7878/store` will add the N-Quads file MY_FILE.nt to the server dataset.

Use `oxigraph_server --help` to see the possible options when starting the server.

### Using a Docker image

#### Display the help menu
```sh
docker run --rm oxigraph/oxigraph --help
```

#### Run the Web server
Expose the server on port `7878` of the host machine, and save data on the local `./data` folder
```sh
docker run --init --rm -v $PWD/data:/data -p 7878:7878 oxigraph/oxigraph -b 0.0.0.0:7878 -f /data
```

You can then access it from your machine on port `7878`:
```sh
# Open the GUI in a browser
firefox http://localhost:7878

# Post some data
curl http://localhost:7878/store?default -H 'Content-Type: text/turtle' -d@./data.ttl

# Make a query
curl -X POST -H 'Accept: application/sparql-results+json' -H 'Content-Type: application/sparql-query' --data 'SELECT * WHERE { ?s ?p ?o } LIMIT 10' http://localhost:7878/query

# Make an UPDATE
curl -X POST -H 'Content-Type: application/sparql-update' --data 'DELETE WHERE { <http://example.com/s> ?p ?o }' http://localhost:7878/update
```

You could easily build your own Docker image by running `docker build -t oxigraph server -f server/Dockerfile .` from the root directory.

## Run the Wikibase server

### Installation

You need to have [a recent stable version of Rust and Cargo installed](https://www.rust-lang.org/tools/install). You also need [clang](https://clang.llvm.org/) to build RocksDB.

To download, build and install the latest released version run `cargo install oxigraph_wikibase`.
There is no need to clone this git repository.

To compile the server from source, clone this git repository, and execute `cargo build --release` in the `wikibase` directory to compile the full server after having downloaded its dependencies.
It will create a fat binary in `target/release/oxigraph_wikibase`.

### Usage

To start a server that is synchronized with [test.wikidata.org](https://test.wikidata.org) you should run:
```bash
./oxigraph_wikibase --mediawiki-api https://test.wikidata.org/w/api.php --mediawiki-base-url https://test.wikidata.org/wiki/ --namespaces 0,120 --file test.wikidata
```

It creates a SPARQL endpoint listening to `localhost:7878/query` that could be queried just like Blazegraph.

The configuration parameters are:
* `mediawiki_api` URL of the MediaWiki API to use
* `mediawiki_base_url` Base URL of MediaWiki pages like `https://test.wikidata.org/wiki/` for test.wikidata.org or `http://localhost/w/index.php?title=` for "vanilla" installations.
* `namespaces` The ids of the Wikibase namespaces to synchronize with, separated by `,`.
* `file` Path of where Oxigraph should store its data.


You can then access it from your machine on port `7878`. No GUI is provided.
```sh
# Make a query
curl -X POST -H 'Accept: application/sparql-results+json' -H 'Content-Type: application/sparql-query' --data 'SELECT * WHERE { ?s ?p ?o } LIMIT 10' http://localhost:7878/query
```
### Using a Docker image

#### Display the help menu
```sh
docker run --rm oxigraph/oxigraph-wikibase --help
```

#### Run the Web server
Expose the server on port `7878` of the host machine, and save data on the local `./data` folder
```sh
docker run --init --rm -v $PWD/wikibase_data:/wikibase_data -p 7878:7878 oxigraph/oxigraph-wikibase -b 0.0.0.0:7878 -f /wikibase_data --mediawiki-api http://some.wikibase.instance/w/api.php --mediawiki-base-url http://some.wikibase.instance/wiki/
```

Warning: the Wikibase instance needs to be accessible from within the container.
The clean way to do that could be to have both your wikibase and oxigraph_wikibase in the same [`docker-compose.yml`](https://docs.docker.com/compose/).

You could easily build your own Docker image by running `docker build -t oxigraph-wikibase -f wikibase/Dockerfile .` from the root directory.

## License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)
   
at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Futures by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
