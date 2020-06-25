use crate::files::load_store;
use crate::manifest::Test;
use crate::report::TestResult;
use chrono::Utc;
use oxigraph::{Error, Result};

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
        .ok_or_else(|| Error::msg(format!("No action found for test {}", test)))?;
    if test.kind == "http://www.w3.org/ns/rdftest#TestNTriplesPositiveSyntax"
        || test.kind == "http://www.w3.org/ns/rdftest#TestNQuadsPositiveSyntax"
        || test.kind == "http://www.w3.org/ns/rdftest#TestTurtlePositiveSyntax"
        || test.kind == "http://www.w3.org/ns/rdftest#TestTrigPositiveSyntax"
    {
        match load_store(action) {
            Ok(_) => Ok(()),
            Err(e) => Err(Error::msg(format!("Parse error: {}", e))),
        }
    } else if test.kind == "http://www.w3.org/ns/rdftest#TestNTriplesNegativeSyntax"
        || test.kind == "http://www.w3.org/ns/rdftest#TestNQuadsNegativeSyntax"
        || test.kind == "http://www.w3.org/ns/rdftest#TestTurtleNegativeSyntax"
        || test.kind == "http://www.w3.org/ns/rdftest#TestTurtleNegativeEval"
        || test.kind == "http://www.w3.org/ns/rdftest#TestTrigNegativeSyntax"
        || test.kind == "http://www.w3.org/ns/rdftest#TestTrigNegativeEval"
        || test.kind == "http://www.w3.org/ns/rdftest#TestXMLNegativeSyntax"
    {
        match load_store(action) {
            Ok(_) => Err(Error::msg(
                "File parsed with an error even if it should not",
            )),
            Err(_) => Ok(()),
        }
    } else if test.kind == "http://www.w3.org/ns/rdftest#TestTurtleEval"
        || test.kind == "http://www.w3.org/ns/rdftest#TestTrigEval"
        || test.kind == "http://www.w3.org/ns/rdftest#TestXMLEval"
    {
        match load_store(action) {
            Ok(actual_graph) => {
                if let Some(result) = &test.result {
                    match load_store(result) {
                        Ok(expected_graph) => {
                            if expected_graph.is_isomorphic(&actual_graph) {
                                Ok(())
                            } else {
                                Err(Error::msg(format!(
                                    "The two files are not isomorphic. Expected:\n{}\nActual:\n{}",
                                    expected_graph, actual_graph
                                )))
                            }
                        }
                        Err(e) => Err(Error::msg(format!("Parse error on file {}: {}", action, e))),
                    }
                } else {
                    Err(Error::msg("No tests result found".to_string()))
                }
            }
            Err(e) => Err(Error::msg(format!("Parse error on file {}: {}", action, e))),
        }
    } else {
        Err(Error::msg(format!("Unsupported test type: {}", test.kind)))
    }
}
