//! SHACL validation.
//!
//! First slice of the SHACL Core evaluator landed in milestone M4. The
//! implementation walks the shapes graph directly using indexed graph
//! lookups instead of compiling each shape to SPARQL. That keeps the
//! crate dependency free while the constraint set is small. A SPARQL
//! backend takes over later when `sh:sparql` and full path expressions
//! are wired in.
//!
//! Supported surface today:
//!
//! 1. Targets: `sh:targetClass` and `sh:targetNode`. Implicit class
//!    targets and `sh:targetSubjectsOf` / `sh:targetObjectsOf` land in
//!    the next slice.
//! 2. Paths: IRI paths only. Blank node path expressions (inverse,
//!    sequence, alternative, zero or more) are skipped with no result
//!    emitted.
//! 3. Constraint components: `sh:minCount` (`sh:MinCountConstraintComponent`).
//!    Every other component is silently ignored so a shapes graph that
//!    mixes supported and unsupported constraints still yields useful
//!    output. Unknown components flip to explicit errors once each is
//!    implemented.

use crate::error::ValidateError;
use oxrdf::vocab::rdf;
use oxrdf::{Graph, NamedNode, NamedNodeRef, NamedOrBlankNode, Term, TermRef};
use rustc_hash::FxHashSet;

/// `http://www.w3.org/ns/shacl#property`
const SH_PROPERTY: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#property");
/// `http://www.w3.org/ns/shacl#path`
const SH_PATH: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#path");
/// `http://www.w3.org/ns/shacl#minCount`
const SH_MIN_COUNT: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#minCount");
/// `http://www.w3.org/ns/shacl#targetClass`
const SH_TARGET_CLASS: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#targetClass");
/// `http://www.w3.org/ns/shacl#targetNode`
const SH_TARGET_NODE: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#targetNode");
/// `http://www.w3.org/ns/shacl#MinCountConstraintComponent`
const SH_MIN_COUNT_CC: NamedNodeRef<'static> =
    NamedNodeRef::new_unchecked("http://www.w3.org/ns/shacl#MinCountConstraintComponent");

/// Severity level attached to a validation result. Mirrors the values in
/// the SHACL vocabulary (`sh:Info`, `sh:Warning`, `sh:Violation`).
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Severity {
    Info,
    Warning,
    #[default]
    Violation,
}

/// Configuration for a [`Validator`] instance.
#[expect(
    clippy::struct_excessive_bools,
    reason = "SHACL has multiple independent boolean toggles; collapsing them into a bitflag would hide the meaning"
)]
#[derive(Clone, Debug, Default)]
pub struct ValidatorConfig {
    abort_on_first_violation: bool,
    include_warnings: bool,
    include_infos: bool,
    inference_before_validation: bool,
}

impl ValidatorConfig {
    /// Reasonable defaults for SHACL Core validation.
    #[must_use]
    pub fn shacl_core() -> Self {
        Self {
            abort_on_first_violation: false,
            include_warnings: true,
            include_infos: true,
            inference_before_validation: false,
        }
    }

    /// Stop validating as soon as a single violation is found. Useful for
    /// CI gates where only a boolean answer is needed.
    #[must_use]
    pub fn abort_on_first_violation(mut self, enabled: bool) -> Self {
        self.abort_on_first_violation = enabled;
        self
    }

    /// Include `sh:Warning` results in the report.
    #[must_use]
    pub fn with_warnings(mut self, enabled: bool) -> Self {
        self.include_warnings = enabled;
        self
    }

    /// Include `sh:Info` results in the report.
    #[must_use]
    pub fn with_infos(mut self, enabled: bool) -> Self {
        self.include_infos = enabled;
        self
    }

    /// Run RDFS or OWL 2 RL reasoning on the data graph before validating.
    /// Recommended for shapes that reference inferred class membership.
    #[must_use]
    pub fn with_inference(mut self, enabled: bool) -> Self {
        self.inference_before_validation = enabled;
        self
    }

    /// Whether validation stops at the first violation.
    #[must_use]
    pub fn stops_on_first_violation(&self) -> bool {
        self.abort_on_first_violation
    }

    /// Whether warnings are reported.
    #[must_use]
    pub fn includes_warnings(&self) -> bool {
        self.include_warnings
    }

    /// Whether infos are reported.
    #[must_use]
    pub fn includes_infos(&self) -> bool {
        self.include_infos
    }

    /// Whether inference runs before validation.
    #[must_use]
    pub fn runs_inference(&self) -> bool {
        self.inference_before_validation
    }
}

/// A single SHACL validation result.
#[derive(Clone, Debug)]
pub struct ValidationResult {
    /// Focus node the result is about.
    pub focus_node: Term,
    /// Path or property the result is about, if any.
    pub result_path: Option<NamedNode>,
    /// Severity of the result.
    pub severity: Severity,
    /// Constraint component that produced the result (`sh:MinCountConstraintComponent`, ...).
    pub source_constraint_component: NamedNode,
    /// Human readable message.
    pub message: String,
}

/// Aggregated validation outcome for a run.
#[derive(Clone, Debug, Default)]
pub struct ValidationReport {
    results: Vec<ValidationResult>,
}

impl ValidationReport {
    /// Empty report (conforming).
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// True if there are no results with [`Severity::Violation`].
    #[must_use]
    pub fn is_conforming(&self) -> bool {
        !self
            .results
            .iter()
            .any(|r| r.severity == Severity::Violation)
    }

    /// All results in the report.
    #[must_use]
    pub fn results(&self) -> &[ValidationResult] {
        &self.results
    }

    /// Append a result. Used by the evaluator to record each violation.
    pub fn push(&mut self, result: ValidationResult) {
        self.results.push(result);
    }

    /// Number of results.
    #[must_use]
    pub fn len(&self) -> usize {
        self.results.len()
    }

    /// Whether the report has any result at all, regardless of severity.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }
}

/// SHACL validator.
///
/// Construct with a configuration and a shapes graph, then call
/// [`Validator::validate`] against a data graph. Only the constraint
/// components listed in the module documentation are evaluated today.
#[derive(Clone, Debug)]
pub struct Validator {
    config: ValidatorConfig,
    shapes: Graph,
}

impl Validator {
    /// Construct a validator from a configuration and a shapes graph.
    #[must_use]
    pub fn new(config: ValidatorConfig, shapes: Graph) -> Self {
        Self { config, shapes }
    }

    /// Configuration this validator was built with.
    #[must_use]
    pub fn config(&self) -> &ValidatorConfig {
        &self.config
    }

    /// Shapes graph this validator was built with.
    #[must_use]
    pub fn shapes(&self) -> &Graph {
        &self.shapes
    }

    /// Validate `data` against the configured shapes.
    ///
    /// Walks the shapes graph once per call. Every `(shape, sh:property,
    /// pshape)` edge produces one scan: the property shape's `sh:minCount`
    /// (if any) is compared against the number of `sh:path` values each
    /// focus node has in `data`. Focus nodes come from `sh:targetClass`
    /// and `sh:targetNode` on the parent shape.
    ///
    /// Returns a [`ValidationReport`] with one [`ValidationResult`] per
    /// violation. The report's `is_conforming` method returns `true` when
    /// no violations were found. `abort_on_first_violation` in the
    /// configuration short circuits the scan after the first violation.
    #[expect(
        clippy::unnecessary_wraps,
        reason = "validate returns Result so future slices can surface UnsupportedConstraint and SparqlFailure without a breaking signature change"
    )]
    pub fn validate(&self, data: &Graph) -> Result<ValidationReport, ValidateError> {
        let mut report = ValidationReport::new();

        for (shape, pshape) in property_shape_edges(&self.shapes) {
            let Some(min_count) = min_count_value(&self.shapes, &pshape) else {
                continue;
            };
            let Some(path) = path_iri(&self.shapes, &pshape) else {
                continue;
            };

            for focus in collect_focus_nodes(&self.shapes, &shape, data) {
                let count = count_values(data, &focus, path.as_ref());
                if count >= min_count {
                    continue;
                }
                report.push(ValidationResult {
                    focus_node: focus_term(&focus),
                    result_path: Some(path.clone()),
                    severity: Severity::Violation,
                    source_constraint_component: SH_MIN_COUNT_CC.into_owned(),
                    message: format!(
                        "sh:minCount {min_count} not satisfied on path {path}; found {count} value(s)"
                    ),
                });
                if self.config.stops_on_first_violation() {
                    return Ok(report);
                }
            }
        }

        Ok(report)
    }
}

/// Collect every `(shape, pshape)` pair where the shapes graph has the
/// triple `shape sh:property pshape`. Returned owned so the loop body
/// can query the shapes graph again without fighting the borrow checker.
fn property_shape_edges(shapes: &Graph) -> Vec<(NamedOrBlankNode, NamedOrBlankNode)> {
    shapes
        .triples_for_predicate(SH_PROPERTY)
        .filter_map(|t| {
            let shape = t.subject.into_owned();
            let pshape = match t.object {
                TermRef::NamedNode(n) => NamedOrBlankNode::NamedNode(n.into_owned()),
                TermRef::BlankNode(b) => NamedOrBlankNode::BlankNode(b.into_owned()),
                TermRef::Literal(_) => return None,
                #[cfg(feature = "rdf-12")]
                TermRef::Triple(_) => return None,
            };
            Some((shape, pshape))
        })
        .collect()
}

/// Read `pshape sh:minCount "N"^^xsd:integer`. Any non numeric literal
/// is ignored because SHACL considers a non integer sh:minCount a
/// malformed shape rather than a violation of the data.
fn min_count_value(shapes: &Graph, pshape: &NamedOrBlankNode) -> Option<u64> {
    match shapes.object_for_subject_predicate(pshape, SH_MIN_COUNT)? {
        TermRef::Literal(l) => l.value().parse::<u64>().ok(),
        _ => None,
    }
}

/// Read `pshape sh:path <IRI>`. Returns `None` for blank node paths so
/// complex path expressions are silently skipped until they land.
fn path_iri(shapes: &Graph, pshape: &NamedOrBlankNode) -> Option<NamedNode> {
    match shapes.object_for_subject_predicate(pshape, SH_PATH)? {
        TermRef::NamedNode(n) => Some(n.into_owned()),
        _ => None,
    }
}

/// Enumerate focus nodes for `shape` from the data graph. Only
/// `sh:targetClass` and `sh:targetNode` are resolved here.
fn collect_focus_nodes(
    shapes: &Graph,
    shape: &NamedOrBlankNode,
    data: &Graph,
) -> FxHashSet<NamedOrBlankNode> {
    let mut focus: FxHashSet<NamedOrBlankNode> = FxHashSet::default();

    for t in shapes.triples_for_subject(shape) {
        if t.predicate == SH_TARGET_CLASS {
            if let TermRef::NamedNode(class) = t.object {
                for subj in data.subjects_for_predicate_object(rdf::TYPE, class) {
                    focus.insert(subj.into_owned());
                }
            }
        } else if t.predicate == SH_TARGET_NODE {
            if let Some(n) = target_node_as_named_or_blank(t.object) {
                focus.insert(n);
            }
        }
    }
    focus
}

fn target_node_as_named_or_blank(term: TermRef<'_>) -> Option<NamedOrBlankNode> {
    match term {
        TermRef::NamedNode(n) => Some(NamedOrBlankNode::NamedNode(n.into_owned())),
        TermRef::BlankNode(b) => Some(NamedOrBlankNode::BlankNode(b.into_owned())),
        TermRef::Literal(_) => None,
        #[cfg(feature = "rdf-12")]
        TermRef::Triple(_) => None,
    }
}

fn count_values(data: &Graph, focus: &NamedOrBlankNode, path: NamedNodeRef<'_>) -> u64 {
    let c = data.objects_for_subject_predicate(focus, path).count();
    u64::try_from(c).unwrap_or(u64::MAX)
}

fn focus_term(focus: &NamedOrBlankNode) -> Term {
    match focus {
        NamedOrBlankNode::NamedNode(n) => Term::NamedNode(n.clone()),
        NamedOrBlankNode::BlankNode(b) => Term::BlankNode(b.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::vocab::xsd;
    use oxrdf::{BlankNode, Literal, Triple};

    fn ex(local: &str) -> NamedNode {
        NamedNode::new_unchecked(format!("https://example.org/ontology#{local}"))
    }

    #[test]
    fn empty_report_is_conforming() {
        let r = ValidationReport::new();
        assert!(r.is_conforming());
        assert!(r.is_empty());
    }

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "the test asserts the Ok path and panics on regression"
    )]
    fn validator_empty_shapes_returns_conforming_report() {
        let v = Validator::new(ValidatorConfig::shacl_core(), Graph::default());
        let report = v
            .validate(&Graph::default())
            .expect("empty shapes must validate cleanly");
        assert!(report.is_conforming());
        assert!(report.is_empty());
    }

    #[test]
    fn config_defaults_are_conservative() {
        let c = ValidatorConfig::shacl_core();
        assert!(!c.stops_on_first_violation());
        assert!(c.includes_warnings());
        assert!(c.includes_infos());
        assert!(!c.runs_inference());
    }

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "the test asserts the Ok path and panics on regression"
    )]
    fn min_count_on_target_class_reports_missing_value() {
        let mut data = Graph::default();
        data.insert(&Triple::new(ex("Acme"), rdf::TYPE, ex("Company")));
        data.insert(&Triple::new(ex("Bravo"), rdf::TYPE, ex("Company")));
        data.insert(&Triple::new(
            ex("Bravo"),
            ex("entityName"),
            Literal::new_simple_literal("Bravo Corp"),
        ));

        let mut shapes = Graph::default();
        let company_shape = ex("CompanyShape");
        let pshape = BlankNode::default();
        shapes.insert(&Triple::new(
            company_shape.clone(),
            SH_TARGET_CLASS.into_owned(),
            ex("Company"),
        ));
        shapes.insert(&Triple::new(
            company_shape,
            SH_PROPERTY.into_owned(),
            pshape.clone(),
        ));
        shapes.insert(&Triple::new(
            pshape.clone(),
            SH_PATH.into_owned(),
            ex("entityName"),
        ));
        shapes.insert(&Triple::new(
            pshape,
            SH_MIN_COUNT.into_owned(),
            Literal::new_typed_literal("1", xsd::INTEGER.into_owned()),
        ));

        let report = Validator::new(ValidatorConfig::shacl_core(), shapes)
            .validate(&data)
            .expect("validation must succeed");
        assert!(!report.is_conforming());
        assert_eq!(report.len(), 1);
        let result = &report.results()[0];
        assert_eq!(result.focus_node, Term::NamedNode(ex("Acme")));
        assert_eq!(result.result_path.as_ref(), Some(&ex("entityName")));
        assert_eq!(result.severity, Severity::Violation);
        assert_eq!(
            result.source_constraint_component,
            SH_MIN_COUNT_CC.into_owned()
        );
    }

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "the test asserts the Ok path and panics on regression"
    )]
    fn min_count_target_node_with_values_is_conforming() {
        let mut data = Graph::default();
        data.insert(&Triple::new(
            ex("Charlie"),
            ex("entityName"),
            Literal::new_simple_literal("Charlie Corp"),
        ));

        let mut shapes = Graph::default();
        let nshape = ex("CharlieShape");
        let pshape = BlankNode::default();
        shapes.insert(&Triple::new(
            nshape.clone(),
            SH_TARGET_NODE.into_owned(),
            ex("Charlie"),
        ));
        shapes.insert(&Triple::new(
            nshape,
            SH_PROPERTY.into_owned(),
            pshape.clone(),
        ));
        shapes.insert(&Triple::new(
            pshape.clone(),
            SH_PATH.into_owned(),
            ex("entityName"),
        ));
        shapes.insert(&Triple::new(
            pshape,
            SH_MIN_COUNT.into_owned(),
            Literal::new_typed_literal("1", xsd::INTEGER.into_owned()),
        ));

        let report = Validator::new(ValidatorConfig::shacl_core(), shapes)
            .validate(&data)
            .expect("validation must succeed");
        assert!(report.is_conforming());
        assert!(report.is_empty());
    }

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "the test asserts the Ok path and panics on regression"
    )]
    fn min_count_abort_on_first_violation_returns_single_result() {
        let mut data = Graph::default();
        data.insert(&Triple::new(ex("Acme"), rdf::TYPE, ex("Company")));
        data.insert(&Triple::new(ex("Delta"), rdf::TYPE, ex("Company")));

        let mut shapes = Graph::default();
        let company_shape = ex("CompanyShape");
        let pshape = BlankNode::default();
        shapes.insert(&Triple::new(
            company_shape.clone(),
            SH_TARGET_CLASS.into_owned(),
            ex("Company"),
        ));
        shapes.insert(&Triple::new(
            company_shape,
            SH_PROPERTY.into_owned(),
            pshape.clone(),
        ));
        shapes.insert(&Triple::new(
            pshape.clone(),
            SH_PATH.into_owned(),
            ex("entityName"),
        ));
        shapes.insert(&Triple::new(
            pshape,
            SH_MIN_COUNT.into_owned(),
            Literal::new_typed_literal("1", xsd::INTEGER.into_owned()),
        ));

        let config = ValidatorConfig::shacl_core().abort_on_first_violation(true);
        let report = Validator::new(config, shapes)
            .validate(&data)
            .expect("validation must succeed");
        assert!(!report.is_conforming());
        assert_eq!(report.len(), 1);
    }

    #[test]
    #[expect(
        clippy::expect_used,
        reason = "the test asserts the Ok path and panics on regression"
    )]
    fn unsupported_constraints_are_silently_ignored() {
        // Only sh:minCount is implemented. A shape that carries only an
        // (unsupported) sh:maxCount must not produce a result and must not
        // raise an error under the current slice.
        let mut data = Graph::default();
        data.insert(&Triple::new(ex("Acme"), rdf::TYPE, ex("Company")));

        let mut shapes = Graph::default();
        let company_shape = ex("CompanyShape");
        let pshape = BlankNode::default();
        shapes.insert(&Triple::new(
            company_shape.clone(),
            SH_TARGET_CLASS.into_owned(),
            ex("Company"),
        ));
        shapes.insert(&Triple::new(
            company_shape,
            SH_PROPERTY.into_owned(),
            pshape.clone(),
        ));
        shapes.insert(&Triple::new(
            pshape.clone(),
            SH_PATH.into_owned(),
            ex("entityName"),
        ));
        shapes.insert(&Triple::new(
            pshape,
            NamedNode::new_unchecked("http://www.w3.org/ns/shacl#maxCount"),
            Literal::new_typed_literal("1", xsd::INTEGER.into_owned()),
        ));

        let report = Validator::new(ValidatorConfig::shacl_core(), shapes)
            .validate(&data)
            .expect("validation must succeed");
        assert!(report.is_conforming());
    }
}
