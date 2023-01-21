use crate::evaluator::TestEvaluator;
use crate::files::{guess_dataset_format, guess_graph_format, load_dataset, load_graph};
use crate::manifest::Test;
use crate::report::{dataset_diff, graph_diff};
use anyhow::{anyhow, bail, Result};
use oxigraph::io::{DatasetFormat, GraphFormat};

pub fn register_parser_tests(evaluator: &mut TestEvaluator) {
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestNTriplesPositiveSyntax",
        |t| evaluate_positive_graph_syntax_test(t, GraphFormat::NTriples),
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestNQuadsPositiveSyntax",
        |t| evaluate_positive_dataset_syntax_test(t, DatasetFormat::NQuads),
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestTurtlePositiveSyntax",
        |t| evaluate_positive_graph_syntax_test(t, GraphFormat::Turtle),
    );
    evaluator.register("http://www.w3.org/ns/rdftest#TestTrigPositiveSyntax", |t| {
        evaluate_positive_dataset_syntax_test(t, DatasetFormat::TriG)
    });
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestNTriplesNegativeSyntax",
        |t| evaluate_negative_graph_syntax_test(t, GraphFormat::NTriples),
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestNQuadsNegativeSyntax",
        |t| evaluate_negative_dataset_syntax_test(t, DatasetFormat::NQuads),
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestTurtleNegativeSyntax",
        |t| evaluate_negative_graph_syntax_test(t, GraphFormat::Turtle),
    );
    evaluator.register("http://www.w3.org/ns/rdftest#TestTrigNegativeSyntax", |t| {
        evaluate_negative_dataset_syntax_test(t, DatasetFormat::TriG)
    });
    evaluator.register("http://www.w3.org/ns/rdftest#TestXMLNegativeSyntax", |t| {
        evaluate_negative_graph_syntax_test(t, GraphFormat::RdfXml)
    });
    evaluator.register("http://www.w3.org/ns/rdftest#TestTurtleEval", |t| {
        evaluate_graph_eval_test(t, GraphFormat::Turtle)
    });
    evaluator.register("http://www.w3.org/ns/rdftest#TestTrigEval", |t| {
        evaluate_dataset_eval_test(t, DatasetFormat::TriG)
    });
    evaluator.register("http://www.w3.org/ns/rdftest#TestXMLEval", |t| {
        evaluate_graph_eval_test(t, GraphFormat::RdfXml)
    });
    evaluator.register("http://www.w3.org/ns/rdftest#TestTurtleNegativeEval", |t| {
        evaluate_negative_graph_syntax_test(t, GraphFormat::Turtle)
    });
    evaluator.register("http://www.w3.org/ns/rdftest#TestTrigNegativeEval", |t| {
        evaluate_negative_dataset_syntax_test(t, DatasetFormat::TriG)
    });
}

fn evaluate_positive_graph_syntax_test(test: &Test, format: GraphFormat) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {test}"))?;
    load_graph(action, format).map_err(|e| anyhow!("Parse error: {e}"))?;
    Ok(())
}

fn evaluate_positive_dataset_syntax_test(test: &Test, format: DatasetFormat) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {test}"))?;
    load_dataset(action, format).map_err(|e| anyhow!("Parse error: {e}"))?;
    Ok(())
}

fn evaluate_negative_graph_syntax_test(test: &Test, format: GraphFormat) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {test}"))?;
    match load_graph(action, format) {
        Ok(_) => bail!("File parsed without errors even if it should not"),
        Err(_) => Ok(()),
    }
}

fn evaluate_negative_dataset_syntax_test(test: &Test, format: DatasetFormat) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {test}"))?;
    match load_dataset(action, format) {
        Ok(_) => bail!("File parsed without errors even if it should not"),
        Err(_) => Ok(()),
    }
}

fn evaluate_graph_eval_test(test: &Test, format: GraphFormat) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {test}"))?;
    let mut actual_graph =
        load_graph(action, format).map_err(|e| anyhow!("Parse error on file {action}: {e}"))?;
    actual_graph.canonicalize();
    let results = test
        .result
        .as_ref()
        .ok_or_else(|| anyhow!("No tests result found"))?;
    let mut expected_graph = load_graph(results, guess_graph_format(results)?)
        .map_err(|e| anyhow!("Parse error on file {results}: {e}"))?;
    expected_graph.canonicalize();
    if expected_graph == actual_graph {
        Ok(())
    } else {
        bail!(
            "The two files are not isomorphic. Diff:\n{}",
            graph_diff(&expected_graph, &actual_graph)
        )
    }
}

fn evaluate_dataset_eval_test(test: &Test, format: DatasetFormat) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {test}"))?;
    let mut actual_dataset =
        load_dataset(action, format).map_err(|e| anyhow!("Parse error on file {action}: {e}"))?;
    actual_dataset.canonicalize();
    let results = test
        .result
        .as_ref()
        .ok_or_else(|| anyhow!("No tests result found"))?;
    let mut expected_dataset = load_dataset(results, guess_dataset_format(results)?)
        .map_err(|e| anyhow!("Parse error on file {results}: {e}"))?;
    expected_dataset.canonicalize();
    if expected_dataset == actual_dataset {
        Ok(())
    } else {
        bail!(
            "The two files are not isomorphic. Diff:\n{}",
            dataset_diff(&expected_dataset, &actual_dataset)
        )
    }
}
