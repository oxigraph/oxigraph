#!/usr/bin/env bash

DATASET_SIZE=100000
PARALLELISM=16
VERSION="9.3.3"
JAVA_HOME=/usr/lib/jvm/java-11-openjdk
cd bsbm-tools
./generate -fc -pc ${DATASET_SIZE} -s nt -fn "explore-${DATASET_SIZE}" -ud -ufn "explore-update-${DATASET_SIZE}"
../graphdb-free-9.3.3/bin/graphdb -s -Dgraphdb.logger.root.level=WARN &
sleep 10
curl -f -X POST http://localhost:7200/rest/repositories -H 'Content-Type:application/json' -d '
{"id":"bsbm","params":{"ruleset":{"label":"Ruleset","name":"ruleset","value":"empty"},"title":{"label":"Repository title","name":"title","value":"GraphDB Free repository"},"checkForInconsistencies":{"label":"Check for inconsistencies","name":"checkForInconsistencies","value":"false"},"disableSameAs":{"label":"Disable owl:sameAs","name":"disableSameAs","value":"true"},"baseURL":{"label":"Base URL","name":"baseURL","value":"http://example.org/owlim#"},"repositoryType":{"label":"Repository type","name":"repositoryType","value":"file-repository"},"id":{"label":"Repository ID","name":"id","value":"repo-bsbm"},"storageFolder":{"label":"Storage folder","name":"storageFolder","value":"storage"}},"title":"BSBM","type":"free"}
'
curl -f -X PUT -H 'Content-Type:application/n-triples' -T "explore-${DATASET_SIZE}.nt" http://localhost:7200/repositories/bsbm/statements
./testdriver -mt ${PARALLELISM} -ucf usecases/explore/sparql.txt -o "../bsbm.explore.graphdb.${VERSION}.${DATASET_SIZE}.${PARALLELISM}.xml" http://localhost:7200/repositories/bsbm
./testdriver -mt ${PARALLELISM} -ucf usecases/exploreAndUpdate/sparql.txt -o "../bsbm.exploreAndUpdate.graphdb.${VERSION}.${DATASET_SIZE}.${PARALLELISM}.xml" http://localhost:7200/repositories/bsbm -u http://localhost:7200/repositories/bsbm/statements -udataset "explore-update-${DATASET_SIZE}.nt"
#./testdriver -mt ${PARALLELISM} -ucf usecases/businessIntelligence/sparql.txt -o "../bsbm.businessIntelligence.graphdb.${VERSION}.${DATASET_SIZE}.${PARALLELISM}.xml" http://localhost:7200/repositories/bsbm
kill $!
sleep 5
rm -r ../graphdb-free-9.3.3/data
rm "explore-${DATASET_SIZE}.nt"
rm "explore-update-${DATASET_SIZE}.nt"
rm -r td_data
