#!/bin/bash

# Generate HDT files from the W3C rdf-tests data files.

# This is a temporary implementation intended to be replaced with a
# Rust function that generates HDT from upstream RDF text encodings as
# needed during test execution.

basic_data=("data-1"
            "data-2"
            "data-3"
            "data-4"
            "data-5"
            "data-6"
            "data-7")

for data_file in "${basic_data[@]}"
do
    # Use the rdf2hdt from the HDT C++ implementation, presumed to be
    # in the PATH.
    rdf2hdt -f ttl \
            -p \
            -v \
            ../../../testsuite/rdf-tests/sparql/sparql10/basic/"$data_file".ttl \
            rdf-tests/sparql/sparql10/basic/"$data_file".hdt
done
