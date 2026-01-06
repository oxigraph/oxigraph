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
            // Our scoping of variables introduced by GRAPH is wrong
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/graph/manifest#graph-variable-scope",
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/graph/manifest#graph-optional",
            #[cfg(feature = "datafusion")]
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#open-eq-01",
            #[cfg(feature = "datafusion")]
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-equals/manifest#eq-graph-1",
            #[cfg(feature = "datafusion")]
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-equals/manifest#eq-graph-2",
            #[cfg(feature = "datafusion")]
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/distinct/manifest#distinct-1",
            #[cfg(feature = "datafusion")]
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/distinct/manifest#distinct-9",
            #[cfg(feature = "datafusion")]
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm-simple",
            #[cfg(feature = "datafusion")]
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/dataset/manifest#dawg-dataset-01", /* TODO: easy to fix */
            #[cfg(feature = "datafusion")]
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/dataset/manifest#dawg-dataset-05", /* TODO: easy to fix */
            #[cfg(feature = "datafusion")]
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm-eq",
            #[cfg(feature = "datafusion")]
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm-not-eq",
            #[cfg(feature = "datafusion")]
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/construct/manifest#construct-3", /* blank node scoping */
            #[cfg(feature = "datafusion")]
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/construct/manifest#construct-4", /* blank node scoping */
        ],
    )
}

#[test]
fn sparql11_query_w3c_evaluation_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/sparql/sparql11/manifest-sparql11-query.ttl",
        &[
            // Our scoping of variables introduced by GRAPH is wrong
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/negation/manifest#graph-minus",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/aggregates/manifest#agg-empty-group-count-graph",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/bindings/manifest#graph",
            // Our property path handling is wrong
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/property-path/manifest#zero_or_more_set_start",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/property-path/manifest#zero_or_more_set_end",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/property-path/manifest#zero_or_one_set_start",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/property-path/manifest#zero_or_one_set_end",
            #[cfg(feature = "datafusion")] // TODO: bad decorelation in DataFusion
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/exists/manifest#exists04",
            #[cfg(feature = "datafusion")]
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/exists/manifest#exists05",
            #[cfg(feature = "datafusion")]
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/negation/manifest#temporal-proximity-by-exclusion-nex-1",
            #[cfg(feature = "datafusion")]
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/construct/manifest#constructlist", /* blank node scoping */
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

#[test]
fn sparql12_w3c_testsuite() -> Result<()> {
    check_testsuite(
        "https://w3c.github.io/rdf-tests/sparql/sparql12/manifest.ttl",
        &[
            // TODO: https://github.com/w3c/sparql-query/issues/282
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-negative/manifest#tripleterm-subject-03",
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax-triple-terms-negative/manifest#tripleterm-subject-06",
            // We do not prevent nested aggregate functions
            "https://w3c.github.io/rdf-tests/sparql/sparql12/syntax/manifest#nested-aggregate-functions",
            #[cfg(feature = "datafusion")]
            "https://w3c.github.io/rdf-tests/sparql/sparql12/grouping/manifest#group01",
            #[cfg(feature = "datafusion")]
            "https://w3c.github.io/rdf-tests/sparql/sparql12/eval-triple-terms/manifest#op-3",
        ],
    )
}
