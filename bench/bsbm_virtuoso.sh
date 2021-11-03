#!/usr/bin/env bash

DATASET_SIZE=10000
PARALLELISM=5
cd bsbm-tools
./generate -fc -pc ${DATASET_SIZE} -s nt -fn "explore-${DATASET_SIZE}"
cp ../virtuoso-opensource/database/virtuoso.ini.sample virtuoso.ini
mkdir ../database
../virtuoso-opensource/bin/virtuoso-t -f &
sleep 10
../virtuoso-opensource/bin/isql 1111 dba dba <<EOF
SPARQL CREATE GRAPH <urn:graph:test>;
ld_dir('$(realpath .)', 'explore-${DATASET_SIZE}.nt', 'urn:graph:test');
rdf_loader_run();
EOF
./testdriver -mt ${PARALLELISM} -ucf usecases/explore/sparql.txt -o "../bsbm.explore.virtuoso.${DATASET_SIZE}.7.2.5.xml" 'http://localhost:8890/sparql?graph-uri=urn:graph:test'
#./testdriver -mt ${PARALLELISM} -ucf usecases/businessIntelligence/sparql.txt -o "../bsbm.businessIntelligence.virtuoso.${DATASET_SIZE}.7.2.5.xml" 'http://localhost:8890/sparql?graph-uri=urn:graph:test'
kill $!
rm -r ../database
rm "explore-${DATASET_SIZE}.nt"
rm "explore-update-${DATASET_SIZE}.nt"
rm -r td_data
