#!/bin/bash -eu
cd $SRC/oxigraph
cargo fuzz build -O --debug-assertions
cp fuzz/target/x86_64-unknown-linux-gnu/release/sparql_eval $OUT/
