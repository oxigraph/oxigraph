use anyhow::Result;
use oxigraph_testsuite::evaluator::TestEvaluator;
use oxigraph_testsuite::manifest::TestManifest;
use oxigraph_testsuite::sparql_evaluator::register_sparql_tests;

fn run_testsuite(manifest_url: &str) -> Result<()> {
    let mut evaluator = TestEvaluator::default();
    register_sparql_tests(&mut evaluator);
    let manifest = TestManifest::new([manifest_url]);
    let results = evaluator.evaluate(manifest)?;

    let mut errors = Vec::default();
    for result in results {
        if let Err(error) = &result.outcome {
            errors.push(format!("{}: failed with error {}", result.test, error))
        }
    }

    assert!(
        errors.is_empty(),
        "{} failing tests:\n{}\n",
        errors.len(),
        errors.join("\n")
    );
    Ok(())
}

#[test]
fn oxigraph_sparql_testsuite() -> Result<()> {
    run_testsuite("https://github.com/oxigraph/oxigraph/tests/sparql/manifest.ttl")
}

#[test]
fn oxigraph_sparql_results_testsuite() -> Result<()> {
    run_testsuite("https://github.com/oxigraph/oxigraph/tests/sparql-results/manifest.ttl")
}

#[test]
fn oxigraph_optimizer_testsuite() -> Result<()> {
    run_testsuite("https://github.com/oxigraph/oxigraph/tests/sparql-optimization/manifest.ttl")
}
