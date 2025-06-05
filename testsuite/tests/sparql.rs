#![cfg(test)]

use anyhow::Result;
use oxigraph_testsuite::check_testsuite;

#[test]
fn sparql10_w3c_query_syntax_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/sparql/sparql10/manifest-syntax.ttl",
        &[
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql3/manifest#syn-bad-26", /* tokenizer */
        ],
    )
}

#[cfg(not(windows))] // Tests don't like git auto "\r\n" on Windows
#[test]
fn sparql10_w3c_query_evaluation_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/sparql/sparql10/manifest-evaluation.ttl",
        &[
            // Multiple writing of the same xsd:integer. Our system does some normalization.
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-str-1",
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-str-2",
            // Simple literal vs xsd:string. We apply RDF 1.1
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/distinct/manifest#distinct-2",
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#open-eq-08",
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#open-eq-10",
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#open-eq-11",
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#open-eq-12",
            // DATATYPE("foo"@en) returns rdf:langString in RDF 1.1
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-datatype-2",
            // We use XSD 1.1 equality on dates
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#date-2",
            // We choose to simplify first the nested group patterns in OPTIONAL
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/optional-filter/manifest#dawg-optional-filter-005-not-simplified",
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
        &[
            // BNODE() scope is currently wrong
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/functions/manifest#bnode01",
            // SERVICE name from a BGP
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/service/manifest#service5",
        ],
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
        &[
            // We allow multiple INSERT DATA with the same blank nodes
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/syntax-update-1/manifest#test_54",
        ],
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

#[test]
fn sparql12_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/sparql/sparql12/manifest.ttl",
        &[
            // TODO RDF 1.2
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-negative/manifest#bind-anonreified",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-negative/manifest#list-anonreifier-01",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-negative/manifest#syntax-update-anonreifier-01",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-negative/manifest#syntax-update-anonreifier-02",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-anonreifier-multiple-01",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-anonreifier-multiple-02",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-anonreifier-multiple-03",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-anonreifier-multiple-04",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-01",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-02",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-03",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-04",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-05",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-06",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-07",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-08",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-09",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-multiple-01",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-multiple-02",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-multiple-03",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-multiple-04",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-multiple-05",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-multiple-06",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-multiple-07",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-multiple-08",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-multiple-09",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#annotation-reifier-multiple-10",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-anonreifier-08",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-anonreifier-09",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-anonreifier-10",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-anonreifier-11",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-anonreifier-12",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-anonreifier-13",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-reifier-01",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-reifier-02",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-reifier-03",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-reifier-04",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-reifier-06",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-reifier-07",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-reifier-08",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-reifier-09",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-reifier-10",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-reifier-11",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-reifier-12",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-reifier-13",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-tripleterm-01",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-tripleterm-02",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-tripleterm-03",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-tripleterm-04",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-tripleterm-05",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-tripleterm-06",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#basic-tripleterm-07",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#bnode-reifier-01",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#bnode-reifier-02",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#bnode-reifier-03",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#bnode-tripleterm-01",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#bnode-tripleterm-02",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#bnode-tripleterm-03",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#compound-all",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#compound-reifier",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#compound-tripleterm",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#expr-tripleterm-01",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#expr-tripleterm-06",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#inside-reifier-01",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#inside-reifier-02",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#inside-tripleterm-01",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#inside-tripleterm-02",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#nested-reifier-01",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#nested-reifier-02",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#nested-tripleterm-01",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#update-anonreifier-01",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#update-anonreifier-02",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#update-anonreifier-03",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#update-anonreifier-04",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#update-anonreifier-05",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#update-anonreifier-06",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#update-reifier-01",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#update-reifier-02",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#update-reifier-03",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#update-reifier-04",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#update-reifier-05",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#update-reifier-06",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#update-reifier-07",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#update-reifier-08",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#update-tripleterm-01",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#update-tripleterm-03",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#update-tripleterm-04",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-positive/manifest#update-tripleterm-05",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#basic-2",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#basic-3",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#basic-4",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#basic-5",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#pattern-1",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#pattern-2",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#pattern-3",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#pattern-3-nomatch",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#pattern-4",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#pattern-5",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#pattern-6",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#pattern-7",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#pattern-8",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#pattern-8-nomatch",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#pattern-9",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#construct-1",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#construct-2",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#construct-3",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#construct-4",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#construct-5",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#graphs-1",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#graphs-2",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#expr-1",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#expr-2",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#op-1",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#op-2",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#op-3",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#op-4",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#update-1",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#update-2",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#update-3",
        ],
    )
}
