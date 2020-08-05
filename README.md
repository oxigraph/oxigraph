Oxigraph
========

[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge)

Oxigraph is a work in progress graph database implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.

There is no released version yet.
The storage format is not stable yet and may be brocken at any time.

Its goal is to provide a compliant, safe and fast graph database based on the [RocksDB](https://rocksdb.org/) and [Sled](https://sled.rs/) key-value stores.
It is written in Rust.
It also provides a set of utility functions for reading, writing and processing RDF files.

It is split into multiple parts:
* The `lib` directory contains the database written as a Rust library.
* The `python` directory contains bindings to use Oxigraph in Python. See [its README](https://github.com/oxigraph/oxigraph/blob/master/python/README.md) for the Python bindings documentation.
* The `js` directory contains bindings to use Oxigraph in JavaScript with the help of WebAssembly. See [its README](https://github.com/oxigraph/oxigraph/blob/master/js/README.md) for the JS bindings documentation.
* The `server` directory contains a stand-alone binary of a web server implementing the [SPARQL 1.1 Protocol](https://www.w3.org/TR/sparql11-protocol/). It uses the [RocksDB](https://rocksdb.org/) key-value store.
* The `wikibase` directory contains a stand-alone binary of a web server able to synchronize with a [Wikibase instance](https://wikiba.se/).

Are currently implemented:
* [SPARQL 1.1 Query](https://www.w3.org/TR/sparql11-query/) except `FROM` and `FROM NAMED`.
* [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/).
* [Turtle](https://www.w3.org/TR/turtle/), [TriG](https://www.w3.org/TR/trig/), [N-Triples](https://www.w3.org/TR/n-triples/), [N-Quads](https://www.w3.org/TR/n-quads/) and [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/) RDF serialization formats for both data ingestion and retrieval using the [Rio library](https://github.com/oxigraph/rio).
* [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/) and [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/).

A preliminary benchmark [is provided](bench/README.md).

## Run the web server

### Build

You need to have [a recent stable version of Rust and Cargo installed](https://www.rust-lang.org/tools/install). You also need [clang](https://clang.llvm.org/) to build RocksDB.

If it's done, executing `cargo build --release` in the root directory of this repository should compile the full server after having downloaded its dependencies.
It will create a fat binary in `target/release/oxigraph_server`.

### Usage

Run `./oxigraph_server` to start the server. It listen by default on `localhost:7878`.

The server provides an HTML UI with a form to execute SPARQL requests.

It provides the following REST actions:
* `/` allows to `POST` data to the server.
  For example `curl -f -X POST -H 'Content-Type:application/n-triples' --data-binary "@MY_FILE.nt" http://localhost:7878/`
  will add the N-Triples file MY_FILE.nt to the server repository. [Turtle](https://www.w3.org/TR/turtle/), [TriG](https://www.w3.org/TR/trig/), [N-Triples](https://www.w3.org/TR/n-triples/), [N-Quads](https://www.w3.org/TR/n-quads/) and [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/) are supported.
* `/query` allows to evaluate SPARQL queries against the server repository following the [SPARQL 1.1 Protocol](https://www.w3.org/TR/sparql11-protocol/#query-operation).
  For example `curl -X POST -H 'Content-Type:application/sparql-query' --data 'SELECT * WHERE { ?s ?p ?o } LIMIT 10' http://localhost:7878/query`.
  This action supports content negotiation and could return [Turtle](https://www.w3.org/TR/turtle/), [N-Triples](https://www.w3.org/TR/n-triples/), [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/), [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/) and [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/).


Use `oxigraph_server --help` to see the possible options when starting the server.

### Using a Docker image

#### Display the help menu
```sh
docker run --rm oxigraph/oxigraph --help
```

#### Run the web server
Expose the server on port `7878` of the host machine, and save data on the local `./data` folder
```sh
docker run --init --rm -v $PWD/data:/data -p 7878:7878 oxigraph/oxigraph -b 0.0.0.0:7878 -f /data
```

You can then access it from your machine on port `7878`:
```sh
# Open the GUI in a browser
firefox http://localhost:7878

# Post some data
curl http://localhost:7878 -H 'Content-Type: application/x-turtle' -d@./data.ttl

# Make a query
curl -H 'Accept: application/sparql-results+json' 'http://localhost:7878/query?query=SELECT%20*%20%7B%20%3Fs%20%3Fp%20%3Fo%20%7D%20LIMIT%2010'
```

You could easily build your own Docker image by running `docker build -t oxigraph server -f server/Dockerfile .` from the root directory.

### Build

You need to have [a recent stable version of Rust and Cargo installed](https://www.rust-lang.org/tools/install).

If it's done, executing `cargo build --release` in the root directory of this repository should compile the full server after having downloaded its dependencies.
It will create a fat binary in `target/release/oxigraph_wikibase`.

### Usage

To start a server that is synchronized with [test.wikidata.org](https://test.wikidata.org) you should run:
```bash
./oxigraph_wikibase --mediawiki-api=https://test.wikidata.org/w/api.php --mediawiki-base-url=https://test.wikidata.org/wiki/ --namespaces=0,120 --file=test.wikidata
```

It creates a SPARQL endpoint listening to `localhost:7878/query` that could be queried just like Blazegraph.

The configuration parameters are:
* `mediawiki_api` URL of the MediaWiki API to use
* `mediawiki_base_url` Base URL of MediaWiki pages like `https://test.wikidata.org/wiki/` for test.wikidata.org or `http://localhost/w/index.php?title=` for "vanilla" installations.
* `namespaces` The ids of the Wikibase namespaces to synchronize with, separated by `,`.
* `file` Path of where Oxigraph should store its data.

### Using a Docker image

#### Display the help menu
```sh
docker run --rm oxigraph/oxigraph-wikibase --help
```

#### Run the web server
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
