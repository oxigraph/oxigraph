use anyhow::Result;
use oxigraph_testsuite::evaluator::TestEvaluator;
use oxigraph_testsuite::manifest::TestManifest;
use oxigraph_testsuite::sparql_evaluator::register_sparql_tests;

fn run_testsuite(manifest_url: &str, ignored_tests: Vec<&str>) -> Result<()> {
    let mut evaluator = TestEvaluator::default();
    register_sparql_tests(&mut evaluator);
    let manifest = TestManifest::new(vec![manifest_url]);
    let results = evaluator.evaluate(manifest)?;

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
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/optional-filter/manifest#dawg-optional-filter-005-not-simplified",
        // This test relies on naive iteration on the input file
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/reduced/manifest#reduced-2"
    ])
}

#[test]
fn sparql11_query_w3c_evaluation_testsuite() -> Result<()> {
    run_testsuite(
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/manifest-sparql11-query.ttl",
        vec![
            //BNODE() scope is currently wrong
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/functions/manifest#bnode01",
            //Property path with unbound graph name are not supported yet
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/property-path/manifest#pp35",
            //SERVICE name from a BGP
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/service/manifest#service5",
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
        vec![
            // We allow multiple INSERT DATA with the same blank nodes
            "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/syntax-update-1/manifest#test_54",
        ],
    )
}

#[test]
fn sparql11_json_w3c_evaluation_testsuite() -> Result<()> {
    run_testsuite(
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/json-res/manifest.ttl",
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
fn sparql_star_syntax_testsuite() -> Result<()> {
    run_testsuite(
        "https://w3c.github.io/rdf-star/tests/sparql/syntax/manifest.ttl",
        vec![],
    )
}

#[test]
fn sparql_star_eval_testsuite() -> Result<()> {
    run_testsuite(
        "https://w3c.github.io/rdf-star/tests/sparql/eval/manifest.ttl",
        vec![],
    )
}
