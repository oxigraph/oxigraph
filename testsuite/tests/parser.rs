use anyhow::Result;
use oxigraph_testsuite::evaluator::TestEvaluator;
use oxigraph_testsuite::manifest::TestManifest;
use oxigraph_testsuite::parser_evaluator::register_parser_tests;

fn run_testsuite(manifest_url: &str) -> Result<()> {
    let mut evaluator = TestEvaluator::default();
    register_parser_tests(&mut evaluator);
    let manifest = TestManifest::new(vec![manifest_url]);
    let results = evaluator.evaluate(manifest)?;

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

#[cfg(not(target_os = "windows"))] // Tests don't like git auto "\r\n" on Windows
#[test]
fn turtle_w3c_testsuite() -> Result<()> {
    run_testsuite("http://w3c.github.io/rdf-tests/turtle/manifest.ttl")
}

#[cfg(not(target_os = "windows"))] // Tests don't like git auto "\r\n" on Windows
#[test]
fn trig_w3c_testsuite() -> Result<()> {
    run_testsuite("http://w3c.github.io/rdf-tests/trig/manifest.ttl")
}

#[test]
fn rdf_xml_w3c_testsuite() -> Result<()> {
    run_testsuite("http://www.w3.org/2013/RDFXMLTests/manifest.ttl")
}

#[test]
fn ntriples_star_w3c_testsuite() -> Result<()> {
    run_testsuite("https://w3c.github.io/rdf-star/tests/nt/syntax/manifest.ttl")
}

#[test]
fn turtle_star_syntax_w3c_testsuite() -> Result<()> {
    run_testsuite("https://w3c.github.io/rdf-star/tests/turtle/syntax/manifest.ttl")
}

#[test]
fn turtle_star_eval_w3c_testsuite() -> Result<()> {
    run_testsuite("https://w3c.github.io/rdf-star/tests/turtle/eval/manifest.ttl")
}

#[test]
fn trig_star_syntax_w3c_testsuite() -> Result<()> {
    run_testsuite("https://w3c.github.io/rdf-star/tests/trig/syntax/manifest.ttl")
}

#[test]
fn trig_star_eval_w3c_testsuite() -> Result<()> {
    run_testsuite("https://w3c.github.io/rdf-star/tests/trig/eval/manifest.ttl")
}
