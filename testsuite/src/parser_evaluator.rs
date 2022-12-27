use crate::evaluator::TestEvaluator;
use crate::files::load_dataset;
use crate::manifest::Test;
use crate::report::dataset_diff;
use anyhow::{anyhow, bail, Result};

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
        .ok_or_else(|| anyhow!("No action found for test {test}"))?;
    load_dataset(action).map_err(|e| anyhow!("Parse error: {e}"))?;
    Ok(())
}

fn evaluate_negative_syntax_test(test: &Test) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {test}"))?;
    match load_dataset(action) {
        Ok(_) => bail!("File parsed with an error even if it should not"),
        Err(_) => Ok(()),
    }
}

fn evaluate_eval_test(test: &Test) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {test}"))?;
    let mut actual_graph =
        load_dataset(action).map_err(|e| anyhow!("Parse error on file {action}: {e}"))?;
    actual_graph.canonicalize();
    if let Some(result) = &test.result {
        let mut expected_graph =
            load_dataset(result).map_err(|e| anyhow!("Parse error on file {action}: {e}"))?;
        expected_graph.canonicalize();
        if expected_graph == actual_graph {
            Ok(())
        } else {
            bail!(
                "The two files are not isomorphic. Diff:\n{}",
                dataset_diff(&expected_graph, &actual_graph)
            )
        }
    } else {
        bail!("No tests result found")
    }
}
