use crate::evaluator::TestEvaluator;
use crate::files::load_dataset;
use crate::manifest::Test;
use crate::report::dataset_diff;
use anyhow::{anyhow, Result};

pub fn register_parser_tests(evaluator: &mut TestEvaluator) {
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestNTriplesPositiveSyntax",
        evaluate_positive_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestNQuadsPositiveSyntax",
        evaluate_positive_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestTurtlePositiveSyntax",
        evaluate_positive_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestTrigPositiveSyntax",
        evaluate_positive_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestNTriplesNegativeSyntax",
        evaluate_negative_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestNQuadsNegativeSyntax",
        evaluate_negative_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestTurtleNegativeSyntax",
        evaluate_negative_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestTurtleNegativeEval",
        evaluate_negative_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestTrigNegativeSyntax",
        evaluate_negative_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestTrigNegativeEval",
        evaluate_negative_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestXMLNegativeSyntax",
        evaluate_negative_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestTurtleEval",
        evaluate_eval_test,
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestTrigEval",
        evaluate_eval_test,
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestXMLEval",
        evaluate_eval_test,
    );
}

fn evaluate_positive_syntax_test(test: &Test) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {}", test))?;
    match load_dataset(action) {
        Ok(_) => Ok(()),
        Err(e) => Err(anyhow!(format!("Parse error: {}", e))),
    }
}

fn evaluate_negative_syntax_test(test: &Test) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {}", test))?;
    match load_dataset(action) {
        Ok(_) => Err(anyhow!("File parsed with an error even if it should not",)),
        Err(_) => Ok(()),
    }
}

fn evaluate_eval_test(test: &Test) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {}", test))?;
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
}
