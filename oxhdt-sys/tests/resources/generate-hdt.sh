#!/bin/bash

# Generate HDT files from the W3C rdf-tests data files.

# HDT Java (https://github.com/rdfhdt/hdt-java) creates the
# data-3.hdt from the data-3.ttl correctly. HDT C++ does not
# per https://github.com/rdfhdt/hdt-cpp/issues/219.

# This is a temporary implementation intended to be replaced with a
# Rust function that generates HDT from upstream RDF text encodings as
# needed during test execution.

# TODO This should probably be a GNU Makefile rule instead of a GNU
# Bash function.

# TODO Ignore manifest.ttl

# TODO Ignore result.ttl
function test_ttl_to_hdt() {
    # First parameter is the directory of source files to process.
    local ttl_dir="../../../testsuite/rdf-tests/sparql/sparql10/${1}"
    local hdt_dir="rdf-tests/sparql/sparql10/${1}"

    # For each RDF Turtle file in the directory
    shopt -s nullglob
    for i in "${ttl_dir}"/*.ttl; do
	# echo "Processing" $i
	hdt_file=$(basename --suffix ".ttl" "${i}")".hdt"
	rdf2hdt.sh "${i}" "$hdt_dir/$hdt_file"
    done
}

# test_ttl_to_hdt "basic"
# test_ttl_to_hdt "triple-match"
# test_ttl_to_hdt "open-world"
# test_ttl_to_hdt "algebra"
# test_ttl_to_hdt "bnode-coreference"
test_ttl_to_hdt "optional"
