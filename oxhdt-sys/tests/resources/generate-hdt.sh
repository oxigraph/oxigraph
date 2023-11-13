#!/bin/bash

# Generate HDT files from the W3C rdf-tests data files.

# HDT Java (https://github.com/rdfhdt/hdt-java) creates the
# data-3.hdt from the data-3.ttl correctly. HDT C++ does not
# per https://github.com/rdfhdt/hdt-cpp/issues/219.

# HDT C++ (https://github.com/rdfhdt/hdt-cpp) creates the
# normalization-02.hdt from the normalization-02.ttl correctly. HDT
# Java does not per https://github.com/rdfhdt/hdt-java/issues/203.

# Therefore, it is not possible to use a single HDT implementation to
# create all of the test case data files and pass consistently.

# This Bash function is a temporary implementation intended to be
# replaced with a Rust function that generates HDT from upstream RDF
# text encodings as needed during test execution.

# Even from the perspective of scripting, this should probably be a
# GNU Makefile rule instead of a GNU Bash function.

# TODO Ignore manifest.ttl

# TODO Ignore result.ttl
function test_ttl_to_hdt() {
    # First parameter is the directory of source files to process.
    local ttl_dir="../../../testsuite/rdf-tests/sparql/sparql10/${1}"
    local hdt_dir="rdf-tests/sparql/sparql10/${1}"

    mkdir --parents "${hdt_dir}"

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
# test_ttl_to_hdt "optional"
# test_ttl_to_hdt "boolean-effective-value"
# test_ttl_to_hdt "bound"
# test_ttl_to_hdt "expr-builtin"
# test_ttl_to_hdt "expr-ops"
# test_ttl_to_hdt "expr-equals"
# test_ttl_to_hdt "regex"
# test_ttl_to_hdt "construct"
# test_ttl_to_hdt "ask"
# test_ttl_to_hdt "distinct"
# test_ttl_to_hdt "sort"
# test_ttl_to_hdt "solution-seq"
test_ttl_to_hdt "reduced"
