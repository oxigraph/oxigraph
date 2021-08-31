#!/usr/bin/env bash

DATASET_SIZE=10000
PARALLELISM=5
cd bsbm-tools
./generate -fc -pc ${DATASET_SIZE} -s nt -fn "explore-${DATASET_SIZE}" -ud -ufn "explore-update-${DATASET_SIZE}"
wget https://downloads.apache.org/jena/binaries/apache-jena-fuseki-4.1.0.zip
unzip apache-jena-fuseki-4.1.0.zip
rm apache-jena-fuseki-4.1.0.zip
echo "rootLogger.level = ERROR" > log4j2.properties
./apache-jena-fuseki-4.1.0/fuseki-server --tdb2 --loc=td_data --update /bsbm &
sleep 10
curl -f -X POST -H 'Content-Type:text/plain' --data-binary "@explore-${DATASET_SIZE}.nt" http://localhost:3030/bsbm
sleep 60
./testdriver -mt ${PARALLELISM} -ucf usecases/explore/sparql.txt -o "../bsbm.explore.jena.${DATASET_SIZE}.${PARALLELISM}.4.1.0.xml" http://localhost:3030/bsbm/query
./testdriver -mt ${PARALLELISM} -ucf usecases/exploreAndUpdate/sparql.txt -o "../bsbm.exploreAndUpdate.jena.${DATASET_SIZE}.${PARALLELISM}.4.1.0.xml" http://localhost:3030/bsbm/query -u http://localhost:3030/bsbm/update -udataset "explore-update-${DATASET_SIZE}.nt"
./testdriver -mt ${PARALLELISM} -ucf usecases/businessIntelligence/sparql.txt -o "../bsbm.businessIntelligence.jena.${DATASET_SIZE}.${PARALLELISM}.4.1.0.xml" http://localhost:3030/bsbm/query
kill $!
rm "explore-${DATASET_SIZE}.nt"
rm -r td_data
rm -r run
rm -r apache-jena-fuseki-4.1.0
rm log4j2.properties
