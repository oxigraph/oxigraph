//! Manifest-driven evaluator for OWL 2 RL reasoning tests.
//!
//! Each test entry declares an input graph (`qt:data`) and an expected
//! closure (`mf:result`). The evaluator loads the input, runs
//! [`oxreason::Reasoner::expand`] under the OWL 2 RL profile, and asserts
//! that every triple in the expected closure is also in the inferred
//! closure. Subset rather than equality so authors don't have to enumerate
//! every reflexive `scm-cls` triple the reasoner emits.

use crate::evaluator::TestEvaluator;
use crate::files::load_graph;
use crate::manifest::Test;
use anyhow::{Context, Result, bail};
use oxigraph::io::RdfFormat;
use oxigraph::model::Graph;
use oxreason::{Reasoner, ReasonerConfig};

/// Custom test type for OWL 2 RL closure-equivalence tests.
const REASONING_EVALUATION_TEST: &str =
    "https://github.com/oxigraph/oxigraph/tests#ReasoningEvaluationTest";

pub fn register_reasoning_tests(evaluator: &mut TestEvaluator) {
    evaluator.register(REASONING_EVALUATION_TEST, evaluate_reasoning_test);
}

fn evaluate_reasoning_test(test: &Test) -> Result<()> {
    let input_url = test
        .data
        .as_deref()
        .context("Reasoning test action must include qt:data pointing at the input graph")?;
    let expected_url = test
        .result
        .as_deref()
        .context("Reasoning test must include mf:result pointing at the expected closure")?;

    let mut input = load_graph(input_url, RdfFormat::Turtle, false)
        .with_context(|| format!("Failed to load input graph at {input_url}"))?;
    let expected = load_graph(expected_url, RdfFormat::Turtle, false)
        .with_context(|| format!("Failed to load expected closure at {expected_url}"))?;

    Reasoner::new(ReasonerConfig::owl2_rl())
        .expand(&mut input)
        .with_context(|| format!("Reasoner::expand failed on {input_url}"))?;

    let mut missing = Vec::new();
    for triple in expected.iter() {
        if !input.contains(triple) {
            missing.push(triple.to_string());
        }
    }

    if !missing.is_empty() {
        bail!(
            "Inferred closure is missing {} triple(s) from the expected set:\n  {}",
            missing.len(),
            missing.join("\n  "),
        );
    }
    Ok(())
}
