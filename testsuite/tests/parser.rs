use oxigraph::Result;
use oxigraph_testsuite::manifest::TestManifest;
use oxigraph_testsuite::parser_evaluator::evaluate_parser_tests;

fn run_testsuite(manifest_url: &str) -> Result<()> {
    let manifest = TestManifest::new(vec![manifest_url]);
    let results = evaluate_parser_tests(manifest)?;

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
fn ntriples_w3c_testsuite() -> Result<()> {
    run_testsuite("http://w3c.github.io/rdf-tests/ntriples/manifest.ttl")
}

#[test]
fn nquads_w3c_testsuite() -> Result<()> {
    run_testsuite("http://w3c.github.io/rdf-tests/nquads/manifest.ttl")
}

#[test]
fn turtle_w3c_testsuite() -> Result<()> {
    run_testsuite("http://w3c.github.io/rdf-tests/turtle/manifest.ttl")
}

#[test]
fn trig_w3c_testsuite() -> Result<()> {
    run_testsuite("http://w3c.github.io/rdf-tests/trig/manifest.ttl")
}

#[test]
fn rdf_xml_w3c_testsuite() -> Result<()> {
    run_testsuite("http://www.w3.org/2013/RDFXMLTests/manifest.ttl")
}
