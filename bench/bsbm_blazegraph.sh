#!/usr/bin/env bash

DATASET_SIZE=100000
cd bsbm-tools
./generate -fc -pc ${DATASET_SIZE} -s nt -fn "explore-${DATASET_SIZE}"
wget https://github.com/blazegraph/database/releases/download/BLAZEGRAPH_RELEASE_2_1_5/blazegraph.jar
/usr/lib/jvm/java-8-openjdk-amd64/bin/java -server -jar blazegraph.jar &
sleep 10
curl -f -X POST -H 'Content-Type:text/plain' --data-binary "@explore-${DATASET_SIZE}.nt" http://localhost:9999/blazegraph/sparql
./testdriver -ucf usecases/explore/sparql.txt -o "../bsbm.explore.blazegraph.${DATASET_SIZE}.2.1.5.xml" http://localhost:9999/blazegraph/sparql
./testdriver -ucf usecases/businessIntelligence/sparql.txt -o "../bsbm.businessIntelligence.blazegraph.${DATASET_SIZE}.2.1.5.xml" http://localhost:9999/blazegraph/sparql
kill $!
rm blazegraph.jar
rm blazegraph.jnl
rm "explore-${DATASET_SIZE}.nt"
rm -r td_data
