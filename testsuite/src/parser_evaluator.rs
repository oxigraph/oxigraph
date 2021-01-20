use crate::files::load_dataset;
use crate::manifest::Test;
use crate::report::{dataset_diff, TestResult};
use anyhow::{anyhow, Result};
use chrono::Utc;

pub fn evaluate_parser_tests(
    manifest: impl Iterator<Item = Result<Test>>,
) -> Result<Vec<TestResult>> {
    manifest
        .map(|test| {
            let test = test?;
            let outcome = evaluate_parser_test(&test);
            Ok(TestResult {
                test: test.id,
                outcome,
                date: Utc::now(),
            })
        })
        .collect()
}

fn evaluate_parser_test(test: &Test) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {}", test))?;
    if test.kind == "http://www.w3.org/ns/rdftest#TestNTriplesPositiveSyntax"
        || test.kind == "http://www.w3.org/ns/rdftest#TestNQuadsPositiveSyntax"
        || test.kind == "http://www.w3.org/ns/rdftest#TestTurtlePositiveSyntax"
        || test.kind == "http://www.w3.org/ns/rdftest#TestTrigPositiveSyntax"
    {
        match load_dataset(action) {
            Ok(_) => Ok(()),
            Err(e) => Err(anyhow!(format!("Parse error: {}", e))),
        }
    } else if test.kind == "http://www.w3.org/ns/rdftest#TestNTriplesNegativeSyntax"
        || test.kind == "http://www.w3.org/ns/rdftest#TestNQuadsNegativeSyntax"
        || test.kind == "http://www.w3.org/ns/rdftest#TestTurtleNegativeSyntax"
        || test.kind == "http://www.w3.org/ns/rdftest#TestTurtleNegativeEval"
        || test.kind == "http://www.w3.org/ns/rdftest#TestTrigNegativeSyntax"
        || test.kind == "http://www.w3.org/ns/rdftest#TestTrigNegativeEval"
        || test.kind == "http://www.w3.org/ns/rdftest#TestXMLNegativeSyntax"
    {
        match load_dataset(action) {
            Ok(_) => Err(anyhow!("File parsed with an error even if it should not",)),
            Err(_) => Ok(()),
        }
    } else if test.kind == "http://www.w3.org/ns/rdftest#TestTurtleEval"
        || test.kind == "http://www.w3.org/ns/rdftest#TestTrigEval"
        || test.kind == "http://www.w3.org/ns/rdftest#TestXMLEval"
    {
        match load_dataset(action) {
            Ok(mut actual_graph) => {
                actual_graph.canonicalize();
                if let Some(result) = &test.result {
                    match load_dataset(result) {
                        Ok(mut expected_graph) => {
                            expected_graph.canonicalize();
                            if expected_graph == actual_graph {
                                Ok(())
                            } else {
                                Err(anyhow!(
                                    "The two files are not isomorphic. Diff:\n{}",
                                    dataset_diff(&expected_graph, &actual_graph)
                                ))
                            }
                        }
                        Err(e) => Err(anyhow!("Parse error on file {}: {}", action, e)),
                    }
                } else {
                    Err(anyhow!("No tests result found"))
                }
            }
            Err(e) => Err(anyhow!("Parse error on file {}: {}", action, e)),
        }
    } else {
        Err(anyhow!("Unsupported test type: {}", test.kind))
    }
}
