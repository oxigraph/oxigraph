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
cargo fuzz build -O --debug-assertions
for TARGET in sparql_eval sparql_results_json sparql_results_tsv sparql_results_xml
do
  cp fuzz/target/x86_64-unknown-linux-gnu/release/$TARGET "$OUT"/
done
build_seed_corpus sparql_results_json srj
build_seed_corpus sparql_results_tsv tsv
build_seed_corpus sparql_results_xml srx
