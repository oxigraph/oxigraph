#!/bin/bash -eu
shopt -s globstar

function build_seed_corpus() {
  mkdir "/tmp/oxigraph_$1"
  for file in **/*."$2"
  do
    hash=($(sha256sum "$file"))
    cp "$file" "/tmp/oxigraph_$1/$hash"
  done
  zip "$1_seed_corpus.zip" /tmp/"oxigraph_$1"/*
  rm -r "/tmp/oxigraph_$1"
}


cd "$SRC"/oxigraph
cargo fuzz build -O --debug-assertions
# shellcheck disable=SC2043
#  SC2043 (warning): This loop will only ever run once.
for TARGET in sparql_eval # sparql_results_json sparql_results_tsv
do
  cp fuzz/target/x86_64-unknown-linux-gnu/release/$TARGET "$OUT"/
done
# build_seed_corpus sparql_results_json json
# build_seed_corpus sparql_results_tsv tsv
