use anyhow::Result;
use oxigraph_testsuite::manifest::TestManifest;
use oxigraph_testsuite::sparql_evaluator::evaluate_sparql_tests;

fn run_testsuite(manifest_urls: Vec<&str>, ignored_tests: Vec<&str>) -> Result<()> {
    let manifest = TestManifest::new(manifest_urls);
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
fn sparql10_w3c_query_evaluation_testsuite() -> Result<()> {
    let manifest_urls = vec![
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/manifest-syntax.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/algebra/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/ask/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/basic/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/bnode-coreference/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/boolean-effective-value/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/bound/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/cast/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/construct/manifest.ttl",
        //TODO FROM and FROM NAMED "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/construct/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/distinct/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-equals/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-ops/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/graph/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/i18n/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/optional/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/optional-filter/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/reduced/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/regex/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/solution-seq/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/sort/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/triple-match/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/type-promotion/manifest.ttl",
    ];

    let test_blacklist = vec![
        //Bad SPARQL query that should be rejected by the parser
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql4/manifest#syn-bad-38",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql4/manifest#syn-bad-34",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql3/manifest#syn-bad-26",

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
    ];

    run_testsuite(manifest_urls, test_blacklist)
}

#[test]
fn sparql11_query_w3c_evaluation_testsuite() -> Result<()> {
    let manifest_urls =
        vec!["http://www.w3.org/2009/sparql/docs/tests/data-sparql11/manifest-sparql11-query.ttl"];

    let test_blacklist = vec![
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
        // FROM support
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/construct/manifest#constructwhere04",
        //BNODE() scope is currently wrong
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/functions/manifest#bnode01",
        //Property path with unbound graph name are not supported yet
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/property-path/manifest#pp35",
        //SERVICE name from a BGP
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/service/manifest#service5"
    ];

    run_testsuite(manifest_urls, test_blacklist)
}
