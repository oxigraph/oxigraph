#![cfg(test)]

use anyhow::Result;
use oxigraph_testsuite::check_testsuite;

#[test]
fn sparql10_w3c_query_syntax_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/sparql/sparql10/manifest-syntax.ttl",
        &[],
    )
}

#[test]
fn sparql10_w3c_query_evaluation_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/sparql/sparql10/manifest-evaluation.ttl",
        &[
            // Multiple writing of the same xsd:integer. Our system does some normalization.
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-str-1",
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-str-2",
            // We use XSD 1.1 equality on dates
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#date-2",
            // This test relies on naive iteration on the input file
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/reduced/manifest#reduced-1",
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/reduced/manifest#reduced-2",
        ],
    )
}

#[test]
fn sparql11_query_w3c_evaluation_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/sparql/sparql11/manifest-sparql11-query.ttl",
        &[],
    )
}

#[test]
fn sparql11_federation_w3c_evaluation_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/sparql/sparql11/manifest-sparql11-fed.ttl",
        &[
            // Problem during service evaluation order
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/service/manifest#service5",
        ],
    )
}

#[test]
fn sparql11_update_w3c_evaluation_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/sparql/sparql11/manifest-sparql11-update.ttl",
        &[],
    )
}

#[test]
fn sparql11_json_w3c_evaluation_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/sparql/sparql11/json-res/manifest.ttl",
        &[],
    )
}

#[test]
fn sparql11_tsv_w3c_evaluation_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/sparql/sparql11/csv-tsv-res/manifest.ttl",
        &[
            // We do not run CSVResultFormatTest tests yet
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/csv-tsv-res/manifest#csv01",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/csv-tsv-res/manifest#csv02",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/csv-tsv-res/manifest#csv03",
        ],
    )
}

#[cfg(feature = "rdf-12")]
#[test]
fn sparql12_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/sparql/sparql12/manifest.ttl",
        &[],
    )
}
