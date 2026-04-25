//! Manifest-driven evaluator for SHACL Core validation tests.
//!
//! Each entry declares a combined shapes-plus-data graph (`mf:action`)
//! and an expected-violations file (`mf:result`). The combined-graph
//! convention keeps fixtures small; SHACL allows shapes to live in the
//! data graph and treats them as a single graph at validation time.
//!
//! The expected file is a tiny TTL graph where each blank-node statement
//! of the form `[] ox:focusNode <focus> ; ox:violatesConstraint <component> .`
//! asserts that the report must contain at least one result with that
//! focus node and constraint component. Other report fields (severity,
//! result_path, message) are not asserted here so authors can keep
//! expected files terse.

use crate::evaluator::TestEvaluator;
use crate::files::load_graph;
use crate::manifest::Test;
use anyhow::{Context, Result, bail};
use oxigraph::io::RdfFormat;
use oxigraph::model::{NamedNodeRef, Term, TermRef};
use oxreason::{Severity, Validator, ValidatorConfig};

/// Custom test type for SHACL Core validation tests.
const SHACL_VALIDATION_TEST: &str =
    "https://github.com/oxigraph/oxigraph/tests#ShaclValidationTest";

/// `https://github.com/oxigraph/oxigraph/tests#focusNode` — points at the
/// focus node that the test author expects to violate at least one
/// constraint.
const OX_FOCUS_NODE: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("https://github.com/oxigraph/oxigraph/tests#focusNode");

/// `https://github.com/oxigraph/oxigraph/tests#violatesConstraint` — points
/// at the SHACL constraint component IRI the focus node must violate.
const OX_VIOLATES_CONSTRAINT: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("https://github.com/oxigraph/oxigraph/tests#violatesConstraint");

pub fn register_shacl_tests(evaluator: &mut TestEvaluator) {
    evaluator.register(SHACL_VALIDATION_TEST, evaluate_shacl_test);
}

fn evaluate_shacl_test(test: &Test) -> Result<()> {
    let action_url = test.action.as_deref().context(
        "SHACL test must declare mf:action pointing at a combined shapes-plus-data file",
    )?;
    let expected_url = test
        .result
        .as_deref()
        .context("SHACL test must declare mf:result pointing at an expected-violations file")?;

    let combined = load_graph(action_url, RdfFormat::Turtle, false)
        .with_context(|| format!("Failed to load combined shapes+data graph at {action_url}"))?;
    let expected = load_graph(expected_url, RdfFormat::Turtle, false)
        .with_context(|| format!("Failed to load expected-violations graph at {expected_url}"))?;

    let report = Validator::new(ValidatorConfig::shacl_core(), combined.clone())
        .validate(&combined)
        .with_context(|| format!("Validator::validate failed on {action_url}"))?;

    // Walk every blank-node assertion in the expected file and confirm a
    // matching result exists in the report.
    let mut missing = Vec::new();
    for focus_triple in expected.triples_for_predicate(OX_FOCUS_NODE) {
        let focus_term: Term = match focus_triple.object {
            TermRef::NamedNode(n) => n.into_owned().into(),
            TermRef::BlankNode(b) => b.into_owned().into(),
            _ => bail!("ox:focusNode must point at an IRI or blank node, got {focus_triple}"),
        };
        let component = expected
            .object_for_subject_predicate(focus_triple.subject, OX_VIOLATES_CONSTRAINT)
            .with_context(|| {
                format!(
                    "expected entry {} is missing ox:violatesConstraint",
                    focus_triple.subject
                )
            })?;
        let component_iri = match component {
            TermRef::NamedNode(n) => n.into_owned(),
            _ => bail!(
                "ox:violatesConstraint must point at an IRI, got {component} for focus {focus_term}"
            ),
        };
        let matched = report.results().iter().any(|r| {
            r.severity == Severity::Violation
                && r.focus_node == focus_term
                && r.source_constraint_component == component_iri
        });
        if !matched {
            missing.push(format!("focus={focus_term} component={component_iri}"));
        }
    }

    if !missing.is_empty() {
        bail!(
            "Validation report is missing {} expected violation(s):\n  {}",
            missing.len(),
            missing.join("\n  "),
        );
    }
    Ok(())
}
