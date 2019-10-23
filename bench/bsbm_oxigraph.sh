#!/usr/bin/env bash

DATASET_SIZE=100000
MEMORY_SIZE=1000000
cd bsbm-tools
./generate -fc -pc ${DATASET_SIZE} -s nt -fn "explore-${DATASET_SIZE}"
cargo build --release --manifest-path="../../server/Cargo.toml" 
(
  ulimit -d ${MEMORY_SIZE}
  ./../../target/release/oxigraph_server --file oxigraph_data
) &
sleep 5
curl -f -X POST -H 'Content-Type:application/n-triples' --data-binary "@explore-${DATASET_SIZE}.nt" http://localhost:7878/
./testdriver -ucf usecases/explore/sparql.txt -o "../bsbm.explore.oxigraph.${DATASET_SIZE}.${MEMORY_SIZE}.$(date +'%Y-%m-%d').xml" http://localhost:7878/query
./testdriver -ucf usecases/businessIntelligence/sparql.txt -o "../bsbm.businessIntelligence.oxigraph.${DATASET_SIZE}.${MEMORY_SIZE}.$(date +'%Y-%m-%d').xml" http://localhost:7878/query
kill $!
rm -r oxigraph_data
rm "explore-${DATASET_SIZE}.nt"
rm -r td_data