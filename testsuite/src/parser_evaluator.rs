use crate::evaluator::TestEvaluator;
use crate::files::{guess_dataset_format, guess_graph_format, load_dataset, load_graph, load_n3};
use crate::manifest::Test;
use crate::report::{dataset_diff, graph_diff};
use anyhow::{anyhow, bail, Result};
use oxigraph::io::{DatasetFormat, GraphFormat};
use oxigraph::model::{BlankNode, Dataset, Quad};
use oxttl::n3::{N3Quad, N3Term};

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
        "https://w3c.github.io/N3/tests/test.n3#TestN3PositiveSyntax",
        evaluate_positive_n3_syntax_test,
    );
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
    evaluator.register(
        "https://w3c.github.io/N3/tests/test.n3#TestN3NegativeSyntax",
        evaluate_negative_n3_syntax_test,
    );
    evaluator.register("http://www.w3.org/ns/rdftest#TestTurtleEval", |t| {
        evaluate_graph_eval_test(t, GraphFormat::Turtle, false)
    });
    evaluator.register("http://www.w3.org/ns/rdftest#TestTrigEval", |t| {
        evaluate_dataset_eval_test(t, DatasetFormat::TriG, false)
    });
    evaluator.register("http://www.w3.org/ns/rdftest#TestXMLEval", |t| {
        evaluate_graph_eval_test(t, GraphFormat::RdfXml, false)
    });
    evaluator.register("https://w3c.github.io/N3/tests/test.n3#TestN3Eval", |t| {
        evaluate_n3_eval_test(t, false)
    });
    evaluator.register("http://www.w3.org/ns/rdftest#TestTurtleNegativeEval", |t| {
        evaluate_negative_graph_syntax_test(t, GraphFormat::Turtle)
    });
    evaluator.register("http://www.w3.org/ns/rdftest#TestTrigNegativeEval", |t| {
        evaluate_negative_dataset_syntax_test(t, DatasetFormat::TriG)
    });
    evaluator.register(
        "https://w3c.github.io/rdf-canon/tests/vocab#RDFC10EvalTest",
        |t| evaluate_positive_dataset_syntax_test(t, DatasetFormat::NQuads), //TODO: not a proper implementation!
    );
    evaluator.register(
        "https://w3c.github.io/rdf-canon/tests/vocab#RDFC10NegativeEvalTest",
        |_| Ok(()), //TODO: not a proper implementation
    );
    evaluator.register(
        "https://w3c.github.io/rdf-canon/tests/vocab#RDFC10MapTest",
        |_| Ok(()), //TODO: not a proper implementation
    );
    evaluator.register(
        "https://github.com/oxigraph/oxigraph/tests#TestNTripleRecovery",
        |t| evaluate_graph_eval_test(t, GraphFormat::NTriples, true),
    );
    evaluator.register(
        "https://github.com/oxigraph/oxigraph/tests#TestTurtleRecovery",
        |t| evaluate_graph_eval_test(t, GraphFormat::Turtle, true),
    );
    evaluator.register(
        "https://github.com/oxigraph/oxigraph/tests#TestN3Recovery",
        |t| evaluate_n3_eval_test(t, true),
    );
}

fn evaluate_positive_graph_syntax_test(test: &Test, format: GraphFormat) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {test}"))?;
    load_graph(action, format, false).map_err(|e| anyhow!("Parse error: {e}"))?;
    Ok(())
}

fn evaluate_positive_dataset_syntax_test(test: &Test, format: DatasetFormat) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {test}"))?;
    load_dataset(action, format, false).map_err(|e| anyhow!("Parse error: {e}"))?;
    Ok(())
}

fn evaluate_positive_n3_syntax_test(test: &Test) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {test}"))?;
    load_n3(action, false).map_err(|e| anyhow!("Parse error: {e}"))?;
    Ok(())
}

fn evaluate_negative_graph_syntax_test(test: &Test, format: GraphFormat) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {test}"))?;
    match load_graph(action, format, false) {
        Ok(_) => bail!("File parsed without errors even if it should not"),
        Err(_) => Ok(()),
    }
}

fn evaluate_negative_dataset_syntax_test(test: &Test, format: DatasetFormat) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {test}"))?;
    match load_dataset(action, format, false) {
        Ok(_) => bail!("File parsed without errors even if it should not"),
        Err(_) => Ok(()),
    }
}

fn evaluate_negative_n3_syntax_test(test: &Test) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {test}"))?;
    match load_n3(action, false) {
        Ok(_) => bail!("File parsed without errors even if it should not"),
        Err(_) => Ok(()),
    }
}

fn evaluate_graph_eval_test(test: &Test, format: GraphFormat, ignore_errors: bool) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {test}"))?;
    let mut actual_graph = load_graph(action, format, ignore_errors)
        .map_err(|e| anyhow!("Parse error on file {action}: {e}"))?;
    actual_graph.canonicalize();
    let results = test
        .result
        .as_ref()
        .ok_or_else(|| anyhow!("No tests result found"))?;
    let mut expected_graph = load_graph(results, guess_graph_format(results)?, false)
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

fn evaluate_dataset_eval_test(
    test: &Test,
    format: DatasetFormat,
    ignore_errors: bool,
) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {test}"))?;
    let mut actual_dataset = load_dataset(action, format, ignore_errors)
        .map_err(|e| anyhow!("Parse error on file {action}: {e}"))?;
    actual_dataset.canonicalize();
    let results = test
        .result
        .as_ref()
        .ok_or_else(|| anyhow!("No tests result found"))?;
    let mut expected_dataset = load_dataset(results, guess_dataset_format(results)?, false)
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

fn evaluate_n3_eval_test(test: &Test, ignore_errors: bool) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {test}"))?;
    let mut actual_dataset = n3_to_dataset(
        load_n3(action, ignore_errors).map_err(|e| anyhow!("Parse error on file {action}: {e}"))?,
    );
    actual_dataset.canonicalize();
    let results = test
        .result
        .as_ref()
        .ok_or_else(|| anyhow!("No tests result found"))?;
    let mut expected_dataset = n3_to_dataset(
        load_n3(results, false).map_err(|e| anyhow!("Parse error on file {results}: {e}"))?,
    );
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

fn n3_to_dataset(quads: Vec<N3Quad>) -> Dataset {
    quads
        .into_iter()
        .filter_map(|q| {
            Some(Quad {
                subject: match q.subject {
                    N3Term::NamedNode(n) => n.into(),
                    N3Term::BlankNode(n) => n.into(),
                    N3Term::Triple(n) => n.into(),
                    N3Term::Literal(_) => return None,
                    N3Term::Variable(v) => BlankNode::new_unchecked(v.into_string()).into(),
                },
                predicate: match q.predicate {
                    N3Term::NamedNode(n) => n,
                    _ => return None,
                },
                object: match q.object {
                    N3Term::NamedNode(n) => n.into(),
                    N3Term::BlankNode(n) => n.into(),
                    N3Term::Triple(n) => n.into(),
                    N3Term::Literal(n) => n.into(),
                    N3Term::Variable(v) => BlankNode::new_unchecked(v.into_string()).into(),
                },
                graph_name: q.graph_name,
            })
        })
        .collect()
}
