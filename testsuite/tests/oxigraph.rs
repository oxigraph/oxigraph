use oxigraph::Result;
use oxigraph_testsuite::manifest::TestManifest;
use oxigraph_testsuite::sparql_evaluator::evaluate_sparql_tests;

fn run_testsuite(manifest_urls: Vec<&str>) -> Result<()> {
    let manifest = TestManifest::new(manifest_urls);
    let results = evaluate_sparql_tests(manifest)?;

    let mut errors = Vec::default();
    for result in results {
        if let Err(error) = &result.outcome {
            errors.push(format!("{}: failed with error {}", result.test, error))
        }
    }

    assert!(errors.is_empty(), "\n{}\n", errors.join("\n"));
    Ok(())
}

#[test]
fn oxigraph_sparql_testsuite() -> Result<()> {
    run_testsuite(vec![
        "https://github.com/oxigraph/oxigraph/tests/sparql/manifest.ttl",
    ])
}
