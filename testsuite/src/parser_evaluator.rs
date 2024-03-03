use crate::evaluator::TestEvaluator;
use crate::files::{guess_rdf_format, load_dataset, load_n3, read_file_to_string};
use crate::manifest::Test;
use crate::report::{dataset_diff, format_diff};
use anyhow::{bail, ensure, Context, Result};
use oxigraph::io::RdfFormat;
use oxigraph::model::graph::CanonicalizationAlgorithm;
use oxigraph::model::{BlankNode, Dataset, Quad};
use oxttl::n3::{N3Quad, N3Term};

pub fn register_parser_tests(evaluator: &mut TestEvaluator) {
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestNTriplesPositiveSyntax",
        |t| evaluate_positive_syntax_test(t, RdfFormat::NTriples),
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestNQuadsPositiveSyntax",
        |t| evaluate_positive_syntax_test(t, RdfFormat::NQuads),
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestTurtlePositiveSyntax",
        |t| evaluate_positive_syntax_test(t, RdfFormat::Turtle),
    );
    evaluator.register("http://www.w3.org/ns/rdftest#TestTrigPositiveSyntax", |t| {
        evaluate_positive_syntax_test(t, RdfFormat::TriG)
    });
    evaluator.register(
        "https://w3c.github.io/N3/tests/test.n3#TestN3PositiveSyntax",
        evaluate_positive_n3_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestNTriplesNegativeSyntax",
        |t| evaluate_negative_syntax_test(t, RdfFormat::NTriples),
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestNQuadsNegativeSyntax",
        |t| evaluate_negative_syntax_test(t, RdfFormat::NQuads),
    );
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestTurtleNegativeSyntax",
        |t| evaluate_negative_syntax_test(t, RdfFormat::Turtle),
    );
    evaluator.register("http://www.w3.org/ns/rdftest#TestTrigNegativeSyntax", |t| {
        evaluate_negative_syntax_test(t, RdfFormat::TriG)
    });
    evaluator.register("http://www.w3.org/ns/rdftest#TestXMLNegativeSyntax", |t| {
        evaluate_negative_syntax_test(t, RdfFormat::RdfXml)
    });
    evaluator.register(
        "https://w3c.github.io/N3/tests/test.n3#TestN3NegativeSyntax",
        evaluate_negative_n3_syntax_test,
    );
    evaluator.register("http://www.w3.org/ns/rdftest#TestTurtleEval", |t| {
        evaluate_eval_test(t, RdfFormat::Turtle, false)
    });
    evaluator.register("http://www.w3.org/ns/rdftest#TestTrigEval", |t| {
        evaluate_eval_test(t, RdfFormat::TriG, false)
    });
    evaluator.register("http://www.w3.org/ns/rdftest#TestXMLEval", |t| {
        evaluate_eval_test(t, RdfFormat::RdfXml, false)
    });
    evaluator.register("https://w3c.github.io/N3/tests/test.n3#TestN3Eval", |t| {
        evaluate_n3_eval_test(t, false)
    });
    evaluator.register("http://www.w3.org/ns/rdftest#TestTurtleNegativeEval", |t| {
        evaluate_negative_syntax_test(t, RdfFormat::Turtle)
    });
    evaluator.register("http://www.w3.org/ns/rdftest#TestTrigNegativeEval", |t| {
        evaluate_negative_syntax_test(t, RdfFormat::TriG)
    });
    evaluator.register(
        "http://www.w3.org/ns/rdftest#TestNTriplesPositiveC14N",
        |t| evaluate_positive_c14n_test(t, RdfFormat::NTriples),
    );
    evaluator.register(
        "https://w3c.github.io/rdf-canon/tests/vocab#RDFC10EvalTest",
        |t| evaluate_positive_syntax_test(t, RdfFormat::NQuads), //TODO: not a proper implementation!
    );
    evaluator.register(
        "https://w3c.github.io/rdf-canon/tests/vocab#RDFC10NegativeEvalTest",
        |_| Ok(()), // TODO: not a proper implementation
    );
    evaluator.register(
        "https://w3c.github.io/rdf-canon/tests/vocab#RDFC10MapTest",
        |_| Ok(()), // TODO: not a proper implementation
    );
    evaluator.register(
        "https://github.com/oxigraph/oxigraph/tests#TestNTripleRecovery",
        |t| evaluate_eval_test(t, RdfFormat::NTriples, true),
    );
    evaluator.register(
        "https://github.com/oxigraph/oxigraph/tests#TestTurtleRecovery",
        |t| evaluate_eval_test(t, RdfFormat::Turtle, true),
    );
    evaluator.register(
        "https://github.com/oxigraph/oxigraph/tests#TestN3Recovery",
        |t| evaluate_n3_eval_test(t, true),
    );
}

fn evaluate_positive_syntax_test(test: &Test, format: RdfFormat) -> Result<()> {
    let action = test.action.as_deref().context("No action found")?;
    load_dataset(action, format, false).context("Parse error")?;
    Ok(())
}

fn evaluate_positive_n3_syntax_test(test: &Test) -> Result<()> {
    let action = test.action.as_deref().context("No action found")?;
    load_n3(action, false).context("Parse error")?;
    Ok(())
}

fn evaluate_negative_syntax_test(test: &Test, format: RdfFormat) -> Result<()> {
    let action = test.action.as_deref().context("No action found")?;
    let Err(error) = load_dataset(action, format, false) else {
        bail!("File parsed without errors even if it should not");
    };
    if let Some(result) = &test.result {
        let expected = read_file_to_string(result)?;
        ensure!(
            expected == error.to_string(),
            "Not expected error message:\n{}",
            format_diff(&expected, &error.to_string(), "message")
        );
    }
    Ok(())
}

fn evaluate_negative_n3_syntax_test(test: &Test) -> Result<()> {
    let action = test.action.as_deref().context("No action found")?;
    ensure!(
        load_n3(action, false).is_err(),
        "File parsed without errors even if it should not"
    );
    Ok(())
}

fn evaluate_eval_test(test: &Test, format: RdfFormat, ignore_errors: bool) -> Result<()> {
    let action = test.action.as_deref().context("No action found")?;
    let mut actual_dataset = load_dataset(action, format, ignore_errors)
        .with_context(|| format!("Parse error on file {action}"))?;
    actual_dataset.canonicalize(CanonicalizationAlgorithm::Unstable);
    let results = test.result.as_ref().context("No tests result found")?;
    let mut expected_dataset = load_dataset(results, guess_rdf_format(results)?, false)
        .with_context(|| format!("Parse error on file {results}"))?;
    expected_dataset.canonicalize(CanonicalizationAlgorithm::Unstable);
    ensure!(
        expected_dataset == actual_dataset,
        "The two files are not isomorphic. Diff:\n{}",
        dataset_diff(&expected_dataset, &actual_dataset)
    );
    Ok(())
}

fn evaluate_n3_eval_test(test: &Test, ignore_errors: bool) -> Result<()> {
    let action = test.action.as_deref().context("No action found")?;
    let mut actual_dataset = n3_to_dataset(
        load_n3(action, ignore_errors).with_context(|| format!("Parse error on file {action}"))?,
    );
    actual_dataset.canonicalize(CanonicalizationAlgorithm::Unstable);
    let results = test.result.as_ref().context("No tests result found")?;
    let mut expected_dataset = n3_to_dataset(
        load_n3(results, false).with_context(|| format!("Parse error on file {results}"))?,
    );
    expected_dataset.canonicalize(CanonicalizationAlgorithm::Unstable);
    ensure!(
        expected_dataset == actual_dataset,
        "The two files are not isomorphic. Diff:\n{}",
        dataset_diff(&expected_dataset, &actual_dataset)
    );
    Ok(())
}

fn evaluate_positive_c14n_test(test: &Test, format: RdfFormat) -> Result<()> {
    let action = test.action.as_deref().context("No action found")?;
    let actual = load_dataset(action, format, false)
        .with_context(|| format!("Parse error on file {action}"))?
        .to_string();
    let results = test.result.as_ref().context("No tests result found")?;
    let expected =
        read_file_to_string(results).with_context(|| format!("Read error on file {results}"))?;
    ensure!(
        expected == actual,
        "The two files are not equal. Diff:\n{}",
        format_diff(&expected, &actual, "c14n")
    );
    Ok(())
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
