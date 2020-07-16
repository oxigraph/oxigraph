#!/usr/bin/env bash
if [[ -z ./Dockerfile ]] ; then
  echo 'this script should be run the docker directory'
  exit 1
fi

cat Dockerfile | docker build -t oxigraph/oxigraph -
cat Dockerfile | sed s/oxigraph_server/oxigraph_wikibase/g | docker build -t oxigraph/oxigraph-wikibase -

docker push oxigraph/oxigraph:latest
docker push oxigraph/oxigraph-wikibase:latest
