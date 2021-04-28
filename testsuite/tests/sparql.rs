use anyhow::Result;
use oxigraph_testsuite::manifest::TestManifest;
use oxigraph_testsuite::sparql_evaluator::evaluate_sparql_tests;

fn run_testsuite(manifest_url: &str, ignored_tests: Vec<&str>) -> Result<()> {
    let manifest = TestManifest::new(vec![manifest_url]);
    let results = evaluate_sparql_tests(manifest)?;

    let mut errors = Vec::default();
    for result in results {
        if let Err(error) = &result.outcome {
            if !ignored_tests.contains(&result.test.as_str()) {
                errors.push(format!("{}: failed with error {}", result.test, error))
            }
        }
    }

    assert!(errors.is_empty(), "\n{}\n", errors.join("\n"));
    Ok(())
}

#[test]
fn sparql10_w3c_query_syntax_testsuite() -> Result<()> {
    run_testsuite(
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/manifest-syntax.ttl",
        vec![
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql4/manifest#syn-bad-38", // bnode scope
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql4/manifest#syn-bad-34", // bnode scope
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql3/manifest#syn-bad-26", // tokenizer
        ],
    )
}

#[test]
fn sparql10_w3c_query_evaluation_testsuite() -> Result<()> {
    run_testsuite("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/manifest-evaluation.ttl", vec![
        //Multiple writing of the same xsd:integer. Our system does strong normalization.
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/distinct/manifest#distinct-1",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/distinct/manifest#distinct-9",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-str-1",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-str-2",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-equals/manifest#eq-graph-1",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-equals/manifest#eq-graph-2",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#open-eq-01",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#open-eq-04",
        //Multiple writing of the same xsd:double. Our system does strong normalization.
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm-simple",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm-eq",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm-not-eq",
        //Simple literal vs xsd:string. We apply RDF 1.1
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/distinct/manifest#distinct-2",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#open-eq-08",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#open-eq-10",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#open-eq-11",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#open-eq-12",
        //DATATYPE("foo"@en) returns rdf:langString in RDF 1.1
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-datatype-2",
        // We use XSD 1.1 equality on dates
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#date-2",
        // We choose to simplify first the nested group patterns in OPTIONAL
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/optional-filter/manifest#dawg-optional-filter-005-not-simplified"
    ])
}

#[test]
fn sparql11_query_w3c_evaluation_testsuite() -> Result<()> {
    run_testsuite(
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/manifest-sparql11-query.ttl",
        vec![
            //Bad SPARQL query that should be rejected by the parser
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/aggregates/manifest#agg08",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/aggregates/manifest#agg09",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/aggregates/manifest#agg10",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/aggregates/manifest#agg11",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/aggregates/manifest#agg12",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/grouping/manifest#group07",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/grouping/manifest#group06",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/grouping/manifest#group07",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/syntax-query/manifest#test_43",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/syntax-query/manifest#test_44",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/syntax-query/manifest#test_45",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/syntax-query/manifest#test_60",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/syntax-query/manifest#test_61a",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/syntax-query/manifest#test_62a",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/syntax-query/manifest#test_65",
            // SPARQL 1.1 JSON query results deserialization is not implemented yet
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/aggregates/manifest#agg-empty-group-count-1",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/aggregates/manifest#agg-empty-group-count-2",
            //BNODE() scope is currently wrong
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/functions/manifest#bnode01",
            //Property path with unbound graph name are not supported yet
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/property-path/manifest#pp35",
            //SERVICE name from a BGP
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/service/manifest#service5"
        ],
    )
}

#[test]
fn sparql11_federation_w3c_evaluation_testsuite() -> Result<()> {
    run_testsuite(
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/manifest-sparql11-fed.ttl",
        vec![
            // Problem during service evaluation order
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/service/manifest#service5",
        ],
    )
}

#[test]
fn sparql11_update_w3c_evaluation_testsuite() -> Result<()> {
    run_testsuite(
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/manifest-sparql11-update.ttl",
        vec![],
    )
}

#[test]
fn sparql11_tsv_w3c_evaluation_testsuite() -> Result<()> {
    run_testsuite(
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/csv-tsv-res/manifest.ttl",
        vec![
            // We do not run CSVResultFormatTest tests yet
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/csv-tsv-res/manifest#csv01",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/csv-tsv-res/manifest#csv02",
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/csv-tsv-res/manifest#csv03",
        ],
    )
}

#[test]
fn sparql_star_query_syntax_testsuite() -> Result<()> {
    run_testsuite(
        "https://w3c.github.io/rdf-star/tests/sparql/syntax/manifest.ttl",
        vec![
            // SPARQL* is not implemented yet
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-1",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-2",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-3",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-4",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-5",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-6",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-7",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-ann-01",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-ann-02",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-ann-03",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-ann-04",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-ann-05",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-ann-06",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-ann-07",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-ann-08",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-ann-09",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-bnode-1",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-bnode-2",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-bnode-3",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-compound-1",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-expr-1",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-expr-2",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-expr-6",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-inside-1",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-inside-2",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-nested-1",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-nested-2",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-update-1",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-update-2",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-update-3",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-update-4",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-update-5",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-update-6",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-update-7",
            "https://w3c.github.io/rdf-star/tests/sparql/syntax#sparql-star-update-8",
        ],
    )
}
