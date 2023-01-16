#!/usr/bin/env bash

DATASET_SIZE=100000
PARALLELISM=16
VERSION="4.2.2"
TOMCAT_VERSION="9.0.71"
wget -nc -O "rdf4j-${VERSION}.zip" "https://www.eclipse.org/downloads/download.php?file=/rdf4j/eclipse-rdf4j-${VERSION}-sdk.zip&mirror_id=1"
wget -nc -O "tomcat-${TOMCAT_VERSION}.zip" "https://dlcdn.apache.org/tomcat/tomcat-9/v${TOMCAT_VERSION}/bin/apache-tomcat-${TOMCAT_VERSION}.zip"
cd bsbm-tools || exit
./generate -fc -pc ${DATASET_SIZE} -s nt -fn "explore-${DATASET_SIZE}" -ud -ufn "explore-update-${DATASET_SIZE}"
wget -nc -O "rdf4j-${VERSION}.zip" "https://www.eclipse.org/downloads/download.php?file=/rdf4j/eclipse-rdf4j-${VERSION}-sdk.zip&mirror_id=1"
unzip ../"rdf4j-${VERSION}.zip"
unzip ../"tomcat-${TOMCAT_VERSION}.zip"
CATALINA_HOME="$(pwd)/apache-tomcat-${TOMCAT_VERSION}"
export CATALINA_HOME
export JAVA_OPTS="-Dorg.eclipse.rdf4j.appdata.basedir=${CATALINA_HOME}/rdf4j"
cp "eclipse-rdf4j-${VERSION}"/war/rdf4j-server.war "${CATALINA_HOME}"/webapps/
chmod +x "${CATALINA_HOME}"/bin/*.sh
"${CATALINA_HOME}"/bin/startup.sh
sleep 30
curl -f -X PUT http://localhost:8080/rdf4j-server/repositories/bsbm -H 'Content-Type:text/turtle' -d '
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#>.
@prefix rep: <http://www.openrdf.org/config/repository#>.
@prefix sr: <http://www.openrdf.org/config/repository/sail#>.
@prefix sail: <http://www.openrdf.org/config/sail#>.

[] a rep:Repository ;
  rep:repositoryID "bsbm" ;
  rdfs:label "BSBM" ;
  rep:repositoryImpl [
    rep:repositoryType "openrdf:SailRepository" ;
    sr:sailImpl [
      sail:sailType "rdf4j:LmdbStore"
    ]
  ] .
'
sleep 10
curl -f -X PUT -H 'Content-Type:application/n-triples' -T "explore-${DATASET_SIZE}.nt" http://localhost:8080/rdf4j-server/repositories/bsbm/statements
./testdriver -mt ${PARALLELISM} -ucf usecases/explore/sparql.txt -o "../bsbm.explore.rdf4j-lmdb.${VERSION}.${DATASET_SIZE}.${PARALLELISM}.xml" http://localhost:8080/rdf4j-server/repositories/bsbm
./testdriver -mt ${PARALLELISM} -ucf usecases/exploreAndUpdate/sparql.txt -o "../bsbm.exploreAndUpdate.rdf4j-lmdb.${VERSION}.${DATASET_SIZE}.${PARALLELISM}.xml" http://localhost:8080/rdf4j-server/repositories/bsbm -u http://localhost:8080/rdf4j-server/repositories/bsbm/statements -udataset "explore-update-${DATASET_SIZE}.nt"
#./testdriver -mt ${PARALLELISM} -ucf usecases/businessIntelligence/sparql.txt -o "../bsbm.businessIntelligence.rdf4j-lmdb.${VERSION}.${DATASET_SIZE}.${PARALLELISM}.xml" http://localhost:8080/rdf4j-server/repositories/bsbm
"${CATALINA_HOME}"/bin/shutdown.sh
rm "explore-${DATASET_SIZE}.nt"
rm "explore-update-${DATASET_SIZE}.nt"
rm -r td_data
rm -r "eclipse-rdf4j-${VERSION}"
rm -r "apache-tomcat-${TOMCAT_VERSION}"
