#!/bin/bash -eu
shopt -s globstar

function build_seed_corpus() {
  mkdir "/tmp/oxigraph_$1"
  for file in **/*."$2"
  do
    hash=$(sha256sum "$file" | awk '{print $1;}')
    cp "$file" "/tmp/oxigraph_$1/$hash"
  done
  zip "$1_seed_corpus.zip" /tmp/"oxigraph_$1"/*
  rm -r "/tmp/oxigraph_$1"
}

cd "$SRC"/oxigraph
git submodule init
git submodule update
cargo fuzz build -O --debug-assertions --strip-dead-code
for TARGET in sparql_query_eval sparql_update_eval sparql_results_json sparql_results_tsv sparql_results_xml n3 nquads trig rdf_xml
do
  cp fuzz/target/x86_64-unknown-linux-gnu/release/$TARGET "$OUT"/
done
build_seed_corpus sparql_results_json srj
build_seed_corpus sparql_results_tsv tsv
build_seed_corpus sparql_results_xml srx
build_seed_corpus n3 n3
build_seed_corpus nquads nq
build_seed_corpus trig trig
build_seed_corpus rdf_xml rdf
