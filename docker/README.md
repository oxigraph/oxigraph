# docker-oxigraph

[Oxigraph](https://github.com/oxigraph/oxigraph) in a [Docker container](https://www.docker.com/resources/what-container).

[![DockerHub Badge Oxigraph](https://dockeri.co/image/oxigraph/oxigraph)](https://hub.docker.com/r/oxigraph/oxigraph/)
[![DockerHub Badge Oxigraph-Wikibase](https://dockeri.co/image/oxigraph/oxigraph-wikibase)](https://hub.docker.com/r/oxigraph/oxigraph-wikibase/)


## Summary

<!-- START doctoc generated TOC please keep comment here to allow auto update -->
<!-- DON'T EDIT THIS SECTION, INSTEAD RE-RUN doctoc TO UPDATE -->


- [Display help menu](#display-help-menu)
- [Run the web server](#run-the-web-server)
- [Run the web server for Wikibase](#run-the-web-server-for-wikibase)
- [Build local image](#build-local-image)
  - [server image](#server-image)
  - [Wikibase server image](#wikibase-server-image)

<!-- END doctoc generated TOC please keep comment here to allow auto update -->

## Display the help menu
```sh
docker run --rm oxigraph/oxigraph --help
```

## Run the web server
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

## Run the web server for Wikibase
```sh
docker run --init --rm -v $PWD/wikibase_data:/wikibase_data -p 7878:7878 oxigraph/oxigraph-wikibase -b 0.0.0.0:7878 -f /wikibase_data --mediawiki-api http://some.wikibase.instance/w/api.php --mediawiki-base-url http://some.wikibase.instance/wiki/
```

:warning: the Wikibase instance needs to be accessible from within the container. The clean way to do that could be to have both your wikibase and oxigraph in the same [`docker-compose.yml`](https://docs.docker.com/compose/).

## Build local image

### server image
```sh
# Build with no build context, just the Dockerfile
cat Dockerfile | docker build -t oxigraph -
```
### Wikibase server image
```sh
# Same, simply replacing the entrypoint
cat Dockerfile | sed s/oxigraph_server/oxigraph_wikibase/ | docker build -t oxigraph-wikibase -
```
