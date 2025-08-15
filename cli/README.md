Oxigraph CLI
============

[![Latest Version](https://img.shields.io/crates/v/oxigraph-cli.svg)](https://crates.io/crates/oxigraph-cli)
[![Crates.io downloads](https://img.shields.io/crates/d/oxigraph-cli)](https://crates.io/crates/oxigraph-cli)
[![Conda](https://img.shields.io/conda/vn/conda-forge/oxigraph-server)](https://anaconda.org/conda-forge/oxigraph-server)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community)

Oxigraph CLI is a graph database implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.
It is packaged as a command-line tool allowing to manipulate RDF files, query them using SPARQL...
It also allows spawning an HTTP server on top of the database.

Oxigraph is in heavy development, and SPARQL query evaluation has not been optimized yet.

Oxigraph provides different installation methods for Oxigraph CLI:
* [`cargo install`](#installation) (multiplatform)
* [A Docker image](#using-a-docker-image)
* [A Pypi package](https://pypi.org/project/oxigraph): with [UV](https://docs.astral.sh/uv/) just run `uvx oxigraph`
* [A conda-forge package](https://anaconda.org/conda-forge/oxigraph-server)
* [Pre-built binaries](https://github.com/oxigraph/oxigraph/releases/latest)

It is also usable as [a Rust library](https://crates.io/crates/oxigraph) and as [a Python library](https://pyoxigraph.readthedocs.io/).

Oxigraph implements the following specifications:
* [SPARQL 1.1 Query](https://www.w3.org/TR/sparql11-query/), [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/), and [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/).
* [Turtle](https://www.w3.org/TR/turtle/), [TriG](https://www.w3.org/TR/trig/), [N-Triples](https://www.w3.org/TR/n-triples/), [N-Quads](https://www.w3.org/TR/n-quads/), and [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/) RDF serialization formats for both data ingestion and retrieval.
* [SPARQL Query Results XML Format](https://www.w3.org/TR/rdf-sparql-XMLres/), [SPARQL 1.1 Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/) and [SPARQL 1.1 Query Results CSV and TSV Formats](https://www.w3.org/TR/sparql11-results-csv-tsv/).
* [SPARQL 1.1 Protocol](https://www.w3.org/TR/sparql11-protocol/#query-operation) and [SPARQL 1.1 Graph Store HTTP Protocol](https://www.w3.org/TR/sparql11-http-rdf-update/).

A preliminary benchmark [is provided](../bench/README.md).

Note that Oxigraph CLI was previously named Oxigraph Server before version 0.4. Older versions are available under [this name](https://crates.io/crates/oxigraph_server).

[![Packaging status](https://repology.org/badge/vertical-allrepos/oxigraph.svg)](https://repology.org/project/oxigraph/versions)

## Installation

You need to have [a recent stable version of Rust and Cargo installed](https://www.rust-lang.org/tools/install) as well as Clang for the RocksDB Rust bindings.

To download, build, and install the latest released version run `cargo install oxigraph-cli`.
There is no need to clone the git repository.

To compile the command-line tool from source, clone this git repository including its submodules (`git clone --recursive https://github.com/oxigraph/oxigraph.git`), and execute `cargo build --release` in the `cli` directory to compile the full binary after having downloaded its dependencies.
It will create a fat binary in `target/release/oxigraph`.

Some build options (cargo features) are available:
- `rocksdb-pkg-config`: links against an already compiled rocksdb shared library found using [pkg-config](https://crates.io/crates/pkg-config).
- `native-tls`: Enables Oxigraph HTTP client for query federation using the host OS TLS stack (enabled by default).
- `rustls-native` Enables Oxigraph HTTP client for query federation using [Rustls](https://crates.io/crates/rustls) and the native certificates.
- `rustls-webpki` Enables Oxigraph HTTP client for query federation using [Rustls](https://crates.io/crates/rustls) and the [Common CA Database](https://www.ccadb.org/) certificates.


## Usage

Run `oxigraph serve --location my_data_storage_directory` to start the server where `my_data_storage_directory` is the directory where you want Oxigraph data to be stored. It listens by default on `localhost:7878`.

The server provides an HTML UI, based on [YASGUI](https://yasgui.triply.cc), with a form to execute SPARQL requests.

It provides the following REST actions:
* `/query` allows evaluating SPARQL queries against the server repository following the [SPARQL 1.1 Protocol](https://www.w3.org/TR/sparql11-protocol/#query-operation).
  For example:
  ```bash
  curl -X POST -H 'Content-Type:application/sparql-query' \
    --data 'SELECT * WHERE { ?s ?p ?o } LIMIT 10' http://localhost:7878/query
  ```
  This action supports content negotiation and could return [Turtle](https://www.w3.org/TR/turtle/), [N-Triples](https://www.w3.org/TR/n-triples/), [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/), [SPARQL Query Results XML Format](https://www.w3.org/TR/rdf-sparql-XMLres/) and [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/).
* `/update` allows to execute SPARQL updates against the server repository following the [SPARQL 1.1 Protocol](https://www.w3.org/TR/sparql11-protocol/#update-operation).
  For example:
  ```sh
  curl -X POST -H 'Content-Type: application/sparql-update' \
    --data 'DELETE WHERE { <http://example.com/s> ?p ?o }' http://localhost:7878/update
  ```
* `/store` allows to retrieve and change the server content using the [SPARQL 1.1 Graph Store HTTP Protocol](https://www.w3.org/TR/sparql11-http-rdf-update/).
  For example:
  ```sh
  curl -f -X POST -H 'Content-Type:application/n-triples' \
    -T MY_FILE.nt "http://localhost:7878/store?graph=http://example.com/g"
  ```
  will add the N-Triples file `MY_FILE.nt` to the server dataset inside of the `http://example.com/g` named graph.
  [Turtle](https://www.w3.org/TR/turtle/), [N-Triples](https://www.w3.org/TR/n-triples/) and [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/) are supported.
  It is also possible to `POST`, `PUT` and `GET` the complete RDF dataset on the server using RDF dataset formats ([TriG](https://www.w3.org/TR/trig/) and [N-Quads](https://www.w3.org/TR/n-quads/)) against the `/store` endpoint.
  For example:
  ```sh
  curl -f -X POST -H 'Content-Type:application/n-quads' \
    -T MY_FILE.nq http://localhost:7878/store
  ```
  will add the N-Quads file `MY_FILE.nq` to the server dataset.

Use `oxigraph --help` to see the possible options when starting the server.

It is also possible to load RDF data offline using bulk loading:
`oxigraph load --location my_data_storage_directory --file my_file.nq`

## Using a Docker image

### Display the help menu
```sh
docker run --rm ghcr.io/oxigraph/oxigraph --help
```

### Run the Webserver
Expose the server on port `7878` of the host machine, and save data on the local `./data` folder
```sh
docker run --rm -v $PWD/data:/data -p 7878:7878 ghcr.io/oxigraph/oxigraph serve --location /data --bind 0.0.0.0:7878
```

You can then access it from your machine on port `7878`:

```sh
# Open the GUI in a browser
firefox http://localhost:7878

# Post some data
curl http://localhost:7878/store?default -H 'Content-Type: text/turtle' -T ./data.ttl

# Make a query
curl -X POST -H 'Accept: application/sparql-results+json' -H 'Content-Type: application/sparql-query' --data 'SELECT * WHERE { ?s ?p ?o } LIMIT 10' http://localhost:7878/query

# Make an UPDATE
curl -X POST -H 'Content-Type: application/sparql-update' --data 'DELETE WHERE { <http://example.com/s> ?p ?o }' http://localhost:7878/update
```

### Run the Web server with basic authentication

It can be useful to make Oxigraph SPARQL endpoint available publicly, with a layer of authentication on `/update` to be able to add data.

You can do so by using a nginx basic authentication in an additional docker container with `docker-compose`. First create a `nginx.conf` file:

```nginx
daemon off;
events {
    worker_connections  1024;
}
http {
    server {
        server_name localhost;
        listen 7878;
        rewrite ^/(.*) /$1 break;
        proxy_ignore_client_abort on;
        proxy_set_header  X-Real-IP  $remote_addr;
        proxy_set_header  X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header  Host $http_host;
        proxy_set_header Access-Control-Allow-Origin "*";
        location ~ ^(/|/query)$ {
            proxy_pass http://oxigraph:7878;
            proxy_pass_request_headers on;
        }
        location ~ ^(/update|/store)$ {
            auth_basic "Oxigraph Administrator's Area";
            auth_basic_user_file /etc/nginx/.htpasswd; 
            proxy_pass http://oxigraph:7878;
            proxy_pass_request_headers on;
        }
    }
}
```

Then a `docker-compose.yml` in the same folder, you can change the default user and password in the `environment` section:

```yaml
version: "3"
services:
  oxigraph:
    image: ghcr.io/oxigraph/oxigraph:latest
    ## To build from local source code:
    # build:
    #   context: .
    #   dockerfile: cli/Dockerfile
    volumes:
      - ./data:/data

  nginx-auth:
    image: nginx:1.21.4
    environment:
      - OXIGRAPH_USER=oxigraph
      - OXIGRAPH_PASSWORD=oxigraphy
    volumes:
      - ./nginx.conf:/etc/nginx/nginx.conf
      ## For multiple users: uncomment this line to mount a pre-generated .htpasswd 
      # - ./.htpasswd:/etc/nginx/.htpasswd
    ports:
      - "7878:7878"
    entrypoint: "bash -c 'echo -n $OXIGRAPH_USER: >> /etc/nginx/.htpasswd && echo $OXIGRAPH_PASSWORD | openssl passwd -stdin -apr1 >> /etc/nginx/.htpasswd && /docker-entrypoint.sh nginx'"
```

Once the `docker-compose.yaml` and `nginx.conf` are ready, start the Oxigraph server and nginx proxy for authentication on http://localhost:7878:

```sh
docker-compose up
```

Then it is possible to update the graph using basic authentication mechanisms. For example with `curl`: change `$OXIGRAPH_USER` and `$OXIGRAPH_PASSWORD`, or set them as environment variables, then run this command to insert a simple triple:

```sh
curl -X POST -u $OXIGRAPH_USER:$OXIGRAPH_PASSWORD -H 'Content-Type: application/sparql-update' --data 'INSERT DATA { <http://example.com/s> <http://example.com/p> <http://example.com/o> }' http://localhost:7878/update
```

In case you want to have multiple users, you can comment the `entrypoint:` line in the `docker-compose.yml` file, uncomment the `.htpasswd` volume, then generate each user in the `.htpasswd` file with this command:

```sh
htpasswd -Bbn $OXIGRAPH_USER $OXIGRAPH_PASSWORD >> .htpasswd
```

### Build the image

You could easily build your own Docker image by cloning this repository with its submodules, and going to the root folder:

```sh
git clone --recursive https://github.com/oxigraph/oxigraph.git
cd oxigraph
```

Then run this command to build the image locally:

```sh
docker build -t ghcr.io/oxigraph/oxigraph -f cli/Dockerfile .
```

## Systemd

It is possible to run Oxigraph in the background using systemd.

For that, you can use the following `oxigraph.service` file (it might be inserted into `/etc/systemd/system/` or `$HOME/.config/systemd/user`):
```ini
[Unit]
Description=Oxigraph database server
After=network-online.target
Wants=network-online.target

[Service]
Type=notify
ExecStart=/PATH/TO/oxigraph serve --location /PATH/TO/OXIGRAPH/DATA

[Install]
WantedBy=multi-user.target
```

## Man pages and autocompletion

Autocompletion for various shells are generated on build in the `target/{debug,release}/build/oxigraph-cli-<hash>/out/complete` directory.
Similarly, man pages are generated in the `target/{debug,release}/build/oxigraph-cli-<hash>/out/man` directory.

## Migration guide

### From 0.2 to 0.3
* The cli API has been completely rewritten. To start the server run `oxigraph serve --location MY_STORAGE` instead of `oxigraph --file MY_STORAGE`.
* Fast data bulk loading is now supported using `oxigraph load --location MY_STORAGE --file MY_FILE`. The file format is guessed from the extension (`.nt`, `.ttl`, `.nq`, ...).
* [RDF-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html) is now implemented.
* All operations are now transactional using the "repeatable read" isolation level:
  the store only exposes changes that have been "committed" (i.e. no partial writes)
  and the exposed state does not change for the complete duration of a read operation (e.g. a SPARQL query) or a read/write operation (e.g. a SPARQL update).


## Help

Feel free to use [GitHub discussions](https://github.com/oxigraph/oxigraph/discussions) or [the Gitter chat](https://gitter.im/oxigraph/community) to ask questions or talk about Oxigraph.
[Bug reports](https://github.com/oxigraph/oxigraph/issues) are also very welcome.

If you need advanced support or are willing to pay to get some extra features, feel free to reach out to [Tpt](https://github.com/Tpt).


## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](../LICENSE-MIT) or
  http://opensource.org/licenses/MIT)

at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Oxigraph by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
