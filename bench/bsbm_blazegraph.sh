#!/usr/bin/env bash

DATASET_SIZE=10000
PARALLELISM=5
cd bsbm-tools
./generate -fc -pc ${DATASET_SIZE} -s nt -fn "explore-${DATASET_SIZE}" -ud -ufn "explore-update-${DATASET_SIZE}"
wget https://github.com/blazegraph/database/releases/download/BLAZEGRAPH_RELEASE_2_1_5/blazegraph.jar
/usr/lib/jvm/java-8-openjdk/bin/java -server -jar blazegraph.jar &
sleep 10
curl -f -X POST -H 'Content-Type:text/plain' --data-binary "@explore-${DATASET_SIZE}.nt" http://localhost:9999/blazegraph/sparql
./testdriver -mt ${PARALLELISM} -ucf usecases/explore/sparql.txt -o "../bsbm.explore.blazegraph.${DATASET_SIZE}.${PARALLELISM}.2.1.5.xml" http://localhost:9999/blazegraph/sparql
./testdriver -mt ${PARALLELISM} -ucf usecases/exploreAndUpdate/sparql.txt -o "../bsbm.exploreAndUpdate.blazegraph.${DATASET_SIZE}.${PARALLELISM}.2.1.5.xml" http://localhost:9999/blazegraph/sparql -u http://localhost:9999/blazegraph/sparql -udataset "explore-update-${DATASET_SIZE}.nt"
#./testdriver -mt ${PARALLELISM} -ucf usecases/businessIntelligence/sparql.txt -o "../bsbm.businessIntelligence.blazegraph.${DATASET_SIZE}.${PARALLELISM}.2.1.5.xml" http://localhost:9999/blazegraph/sparql
kill $!
rm blazegraph.jar
rm blazegraph.jnl
rm "explore-${DATASET_SIZE}.nt"
rm -r td_data
