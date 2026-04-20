//! SHACL validation scaffold.
//!
//! This module only exposes the type surface. Running a validation returns
//! [`crate::ValidateError::NotImplemented`]. Milestone M4 replaces these
//! stubs with a SHACL Core evaluator that compiles shapes to SPARQL and
//! runs them through `spareval` against the target graph.

use oxrdf::{Graph, NamedNode, Term};

use crate::error::ValidateError;

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

    /// Append a result. Used by the evaluator once it is implemented.
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
/// [`Validator::validate`] against a data graph.
///
/// All methods currently return [`ValidateError::NotImplemented`].
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
    /// Current behaviour: returns [`ValidateError::NotImplemented`].
    #[expect(clippy::unused_self, reason = "stub until M4 lands the SHACL Core evaluator")]
    pub fn validate(&self, _data: &Graph) -> Result<ValidationReport, ValidateError> {
        Err(ValidateError::NotImplemented(
            "Validator::validate is not implemented yet, see DESIGN.md",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_report_is_conforming() {
        let r = ValidationReport::new();
        assert!(r.is_conforming());
        assert!(r.is_empty());
    }

    #[test]
    fn validator_returns_not_implemented_in_scaffold() {
        let v = Validator::new(ValidatorConfig::shacl_core(), Graph::default());
        let err = v.validate(&Graph::default()).unwrap_err();
        assert!(matches!(err, ValidateError::NotImplemented(_)));
    }

    #[test]
    fn config_defaults_are_conservative() {
        let c = ValidatorConfig::shacl_core();
        assert!(!c.stops_on_first_violation());
        assert!(c.includes_warnings());
        assert!(c.includes_infos());
        assert!(!c.runs_inference());
    }
}
