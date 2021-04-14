Oxigraph Wikibase
=================

[![Latest Version](https://img.shields.io/crates/v/oxigraph_wikibase.svg)](https://crates.io/crates/oxigraph_wikibase)
[![Crates.io](https://img.shields.io/crates/d/oxigraph_wikibase)](https://crates.io/crates/oxigraph_wikibase)
[![Docker Image Version (latest semver)](https://img.shields.io/docker/v/oxigraph/oxigraph-wikibase?sort=semver)](https://hub.docker.com/repository/docker/oxigraph/oxigraph-wikibase)
[![Docker Image Size (latest semver)](https://img.shields.io/docker/image-size/oxigraph/oxigraph-wikibase)](https://hub.docker.com/repository/docker/oxigraph/oxigraph-wikibase)
[![Docker Pulls](https://img.shields.io/docker/pulls/oxigraph/oxigraph-wikibase)](https://hub.docker.com/repository/docker/oxigraph/oxigraph-wikibase)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge)

Oxigraph Wikibase is a [SPARQL](https://www.w3.org/TR/sparql11-overview/) web server able to synchronize with a [Wikibase instance](https://wikiba.se/).
It is based on [Oxigraph](https://crates.io/crates/oxigraph).

Oxigraph and Oxigraph Wikibase are in heavy development and not been optimized yet.

## Installation

You need to have [a recent stable version of Rust and Cargo installed](https://www.rust-lang.org/tools/install).

To download, build and install the latest released version run `cargo install oxigraph_wikibase`.
There is no need to clone the git repository.

To compile the server from source, clone this git repository, and execute `cargo build --release` in the `wikibase` directory to compile the full server after having downloaded its dependencies.
It will create a fat binary in `target/release/oxigraph_wikibase`.

## Usage

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
## Using a Docker image

### Display the help menu
```sh
docker run --rm oxigraph/oxigraph-wikibase --help
```

### Run the Web server
Expose the server on port `7878` of the host machine, and save data on the local `./data` folder
```sh
docker run --init --rm -v $PWD/wikibase_data:/wikibase_data -p 7878:7878 oxigraph/oxigraph-wikibase -b 0.0.0.0:7878 -f /wikibase_data --mediawiki-api http://some.wikibase.instance/w/api.php --mediawiki-base-url http://some.wikibase.instance/wiki/
```

Warning: the Wikibase instance needs to be accessible from within the container.
The clean way to do that could be to have both your wikibase and oxigraph_wikibase in the same [`docker-compose.yml`](https://docs.docker.com/compose/).

You could easily build your own Docker image by running `docker build -t oxigraph-wikibase -f wikibase/Dockerfile .` from the root directory.


## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](../LICENSE-MIT) or
  http://opensource.org/licenses/MIT)

at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Futures by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
