Oxigraph Server
===============

[![Latest Version](https://img.shields.io/crates/v/oxigraph_server.svg)](https://crates.io/crates/oxigraph_server)
[![Crates.io downloads](https://img.shields.io/crates/d/oxigraph_server)](https://crates.io/crates/oxigraph_server)
[![Docker Image Version (latest semver)](https://img.shields.io/docker/v/oxigraph/oxigraph?sort=semver)](https://hub.docker.com/repository/docker/oxigraph/oxigraph)
[![Docker Image Size (latest semver)](https://img.shields.io/docker/image-size/oxigraph/oxigraph)](https://hub.docker.com/repository/docker/oxigraph/oxigraph)
[![Docker Pulls](https://img.shields.io/docker/pulls/oxigraph/oxigraph)](https://hub.docker.com/repository/docker/oxigraph/oxigraph)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge)

Oxigraph Server is a standalone HTTP server providing a graph database implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.

Its goal is to provide a compliant, safe, and fast graph database based on the [RocksDB](https://rocksdb.org/) key-value store.
It is written in Rust.
It also provides a set of utility functions for reading, writing, and processing RDF files.

Oxigraph is in heavy development and SPARQL query evaluation has not been optimized yet.

Oxigraph provides three different installation methods for Oxigraph server.
* [`cargo install`](#installation) (multiplatform)
* [A Docker image](#using-a-docker-image)
* [A Homebrew formula](#homebrew)

It is also usable as [a Rust library](https://crates.io/crates/oxigraph) and as [a Python library](https://oxigraph.org/pyoxigraph/).

Oxigraph implements the following specifications:
* [SPARQL 1.1 Query](https://www.w3.org/TR/sparql11-query/), [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/), and [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/).
* [Turtle](https://www.w3.org/TR/turtle/), [TriG](https://www.w3.org/TR/trig/), [N-Triples](https://www.w3.org/TR/n-triples/), [N-Quads](https://www.w3.org/TR/n-quads/), and [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/) RDF serialization formats for both data ingestion and retrieval using the [Rio library](https://github.com/oxigraph/rio).
* [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/), [SPARQL 1.1 Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/) and [SPARQL 1.1 Query Results CSV and TSV Formats](https://www.w3.org/TR/sparql11-results-csv-tsv/).
* [SPARQL 1.1 Protocol](https://www.w3.org/TR/sparql11-protocol/#query-operation) and [SPARQL 1.1 Graph Store HTTP Protocol](https://www.w3.org/TR/sparql11-http-rdf-update/).

A preliminary benchmark [is provided](../bench/README.md).

## Installation

You need to have [a recent stable version of Rust and Cargo installed](https://www.rust-lang.org/tools/install).

To download, build and install the latest released version run `cargo install oxigraph_server`.
There is no need to clone the git repository.

To compile the server from source, clone this git repository, and execute `cargo build --release` in the `server` directory to compile the full server after having downloaded its dependencies.
It will create a fat binary in `target/release/oxigraph_server`.

## Usage

Run `oxigraph_server --location my_data_storage_directory serve` to start the server where `my_data_storage_directory` is the directory where you want Oxigraph data to be stored in. It listens by default on `localhost:7878`.

The server provides an HTML UI, based on [YASGUI](https://yasgui.triply.cc), with a form to execute SPARQL requests.

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
  For example `curl -f -X POST -H 'Content-Type:application/n-quads' --data-binary "@MY_FILE.nq" http://localhost:7878/store` will add the N-Quads file MY_FILE.nq to the server dataset.

Use `oxigraph_server --help` to see the possible options when starting the server.

It is also possible to load RDF data offline using bulk loading:
`oxigraph_server --location my_data_storage_directory load --file my_file.nq`

## Using a Docker image

### Display the help menu
```sh
docker run --rm oxigraph/oxigraph --help
```

### Run the Web server
Expose the server on port `7878` of the host machine, and save data on the local `./data` folder
```sh
docker run --rm -v $PWD/data:/data -p 7878:7878 oxigraph/oxigraph --location /data serve --bind 0.0.0.0:7878
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

### Run the Web server with basic authentication

It can be useful to make Oxigraph SPARQL endpoint available publicly, with a layer of authentication on `/update` to be able to add data.

To quickly use a single user/password you can define them in a `.env` file alongside the `docker-compose.yaml`:

```sh
cat << EOF > .env
OXIGRAPH_USER=oxigraph
OXIGRAPH_PASSWORD=oxigraphy
EOF
```

Start the Oxigraph server and nginx proxy for authentication with `docker-compose`:

```sh
docker-compose up
```

To make an update, first change `$OXIGRAPH_USER` and `$OXIGRAPH_PASSWORD`, or set the environment variables, then run:

```sh
curl -X POST -u $OXIGRAPH_USER:$OXIGRAPH_PASSWORD -H 'Content-Type: application/sparql-update' --data 'INSERT DATA { <http://example.com/s> <http://example.com/p> <http://example.com/o> }' http://localhost:7878/update
```

In case you want to have multiple users, you can comment the `entrypoint:` line in the `docker-compose.yml` file, uncomment the `.htpasswd` volume, then generate each user in the `.htpasswd` file with this command:

```sh
htpasswd -Bbn $OXIGRAPH_USER $OXIGRAPH_PASSWORD >> .htpasswd
```

> You can find the nginx configuration in `server/nginx.conf`

### Build the image

You could easily build your own Docker image by cloning this repository with its submodules, and going to the root folder:

```sh
git clone --recursive https://github.com/oxigraph/oxigraph.git
cd oxigraph
```

Then run this command to build the image locally:

````sh
docker build -t oxigraph/oxigraph .
````

## Homebrew

Oxigraph maintains a [Homebrew](https://brew.sh) formula in [a custom tap](https://github.com/oxigraph/homebrew-oxigraph).

To install Oxigraph server using Homebrew do:
```sh
brew tap oxigraph/oxigraph
brew install oxigraph
```
It installs the `oxigraph_server` binary. [See the usage documentation to know how to use it](#usage).

## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](../LICENSE-MIT) or
  http://opensource.org/licenses/MIT)

at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Futures by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
