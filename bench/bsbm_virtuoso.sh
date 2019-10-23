#!/usr/bin/env bash

DATASET_SIZE=100000
cd bsbm-tools
./generate -fc -pc ${DATASET_SIZE} -s nt -fn "explore-${DATASET_SIZE}"
cp ../virtuoso-opensource/database/virtuoso.ini.sample virtuoso.ini
mkdir ../database
../virtuoso-opensource/bin/virtuoso-t -f &
sleep 30
curl -f --digest --user dba:dba -H 'Content-Type:application/n-triples' --data-binary "@explore-${DATASET_SIZE}.nt" 'http://localhost:8890/sparql-graph-crud-auth?graph-uri=urn:graph:test'
curl -f -H 'Content-Type:application/sparql-query' --data "SELECT (COUNT(*) AS ?c) WHERE { ?s ?p ?o }"  'http://localhost:8890/sparql?graph-uri=urn:graph:test'
./testdriver -ucf usecases/explore/sparql.txt -o "../bsbm.explore.virtuoso.${DATASET_SIZE}.7.2.5.xml" 'http://localhost:8890/sparql?graph-uri=urn:graph:test'
./testdriver -ucf usecases/businessIntelligence/sparql.txt -o "../bsbm.businessIntelligence.virtuoso.${DATASET_SIZE}.7.2.5.xml" 'http://localhost:8890/sparql?graph-uri=urn:graph:test'
kill $!
rm -r ../database
rm "explore-${DATASET_SIZE}.nt"
rm -r td_data
