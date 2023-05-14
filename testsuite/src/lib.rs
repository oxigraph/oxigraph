//! Implementation of [W3C RDF tests](https://w3c.github.io/rdf-tests/) to tests Oxigraph conformance.

pub mod evaluator;
pub mod files;
pub mod manifest;
pub mod parser_evaluator;
pub mod report;
pub mod sparql_evaluator;
mod vocab;

use crate::evaluator::TestEvaluator;
use crate::manifest::TestManifest;
use crate::parser_evaluator::register_parser_tests;
use crate::sparql_evaluator::register_sparql_tests;
use anyhow::Result;

pub fn check_testsuite(manifest_url: &str, ignored_tests: &[&str]) -> Result<()> {
    let mut evaluator = TestEvaluator::default();
    register_parser_tests(&mut evaluator);
    register_sparql_tests(&mut evaluator);

    let manifest = TestManifest::new([manifest_url]);
    let results = evaluator.evaluate(manifest)?;

    let mut errors = Vec::default();
    for result in results {
        if let Err(error) = &result.outcome {
            if !ignored_tests.contains(&result.test.as_str()) {
                errors.push(format!("{}: failed with error {}", result.test, error))
            }
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
