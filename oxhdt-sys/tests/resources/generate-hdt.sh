#!/bin/bash

# Generate HDT files from the W3C rdf-tests data files.

# HDT Java (https://github.com/rdfhdt/hdt-java) creates the
# data-3.hdt from the data-3.ttl correctly. HDT C++ does not
# per https://github.com/rdfhdt/hdt-cpp/issues/219.

# This is a temporary implementation intended to be replaced with a
# Rust function that generates HDT from upstream RDF text encodings as
# needed during test execution.

function basic_data_hdt () {
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

        # TODO Convert call to HDT C++ rdf2hdt to HDT Java rdf2hdt.sh per
        # hdt-cpp Issue #219.
        rdf2hdt -f ttl \
                -p \
                -v \
                ../../../testsuite/rdf-tests/sparql/sparql10/basic/"$data_file".ttl \
                rdf-tests/sparql/sparql10/basic/"$data_file".hdt
    done
}

function triple_match_hdt () {
    triple_match_data=("data-01"
                       "data-02"
                       "dawg-data-01")

    for data_file in "${triple_match_data[@]}"
    do
        # Use the rdf2hdt.sh from the HDT Java implementation, presumed to be
        # in the PATH.

        rdf2hdt.sh ../../../testsuite/rdf-tests/sparql/sparql10/triple-match/"$data_file".ttl \
                   rdf-tests/sparql/sparql10/triple-match/"$data_file".hdt
    done
}

function open_world_hdt () {
    data=("data-1"
          "data-2"
          "data-3"
	  "data-4")

    for data_file in "${data[@]}"
    do
        # Use the rdf2hdt.sh from the HDT Java implementation, presumed to be
        # in the PATH.

        rdf2hdt.sh ../../../testsuite/rdf-tests/sparql/sparql10/open-world/"$data_file".ttl \
                   rdf-tests/sparql/sparql10/open-world/"$data_file".hdt
    done
}

# basic_data_hdt
# triple_match_hdt
open_world_hdt
