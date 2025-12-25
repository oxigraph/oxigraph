//! SHACL Validation Report implementation.
//!
//! This module implements the SHACL validation report structure as defined in
//! the W3C SHACL specification.

use oxrdf::{
    BlankNode, Graph, Literal, NamedNodeRef, Term, Triple,
    vocab::{rdf, shacl, xsd},
};

use crate::constraint::ConstraintComponent;
use crate::model::ShapeId;
use crate::path::PropertyPath;

/// Severity level for validation results.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Severity {
    /// Violation severity (most severe).
    #[default]
    Violation,
    /// Warning severity.
    Warning,
    /// Info severity (least severe).
    Info,
}

impl Severity {
    /// Returns the IRI for this severity level.
    pub fn iri(&self) -> NamedNodeRef<'_> {
        match self {
            Self::Violation => shacl::VIOLATION,
            Self::Warning => shacl::WARNING,
            Self::Info => shacl::INFO,
        }
    }

    /// Parses a severity from an IRI.
    pub fn from_iri(iri: NamedNodeRef<'_>) -> Option<Self> {
        if iri == shacl::VIOLATION {
            Some(Self::Violation)
        } else if iri == shacl::WARNING {
            Some(Self::Warning)
        } else if iri == shacl::INFO {
            Some(Self::Info)
        } else {
            None
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Violation => write!(f, "Violation"),
            Self::Warning => write!(f, "Warning"),
            Self::Info => write!(f, "Info"),
        }
    }
}

/// A single validation result.
#[derive(Debug, Clone)]
pub struct ValidationResult {
    /// The focus node that was validated.
    pub focus_node: Term,

    /// The property path (if this is from a property shape).
    pub result_path: Option<PropertyPath>,

    /// The value that caused the violation.
    pub value: Option<Term>,

    /// The source shape that generated this result.
    pub source_shape: ShapeId,

    /// The constraint component that was violated.
    pub source_constraint_component: ConstraintComponent,

    /// Human-readable message describing the violation.
    pub result_message: Option<String>,

    /// Severity level.
    pub result_severity: Severity,

    /// Nested validation results (for detailed errors).
    pub detail: Vec<ValidationResult>,
}

impl ValidationResult {
    /// Creates a new validation result.
    pub fn new(
        focus_node: Term,
        source_shape: ShapeId,
        source_constraint_component: ConstraintComponent,
    ) -> Self {
        Self {
            focus_node,
            result_path: None,
            value: None,
            source_shape,
            source_constraint_component,
            result_message: None,
            result_severity: Severity::Violation,
            detail: Vec::new(),
        }
    }

    /// Sets the result path.
    #[must_use]
    pub fn with_path(mut self, path: PropertyPath) -> Self {
        self.result_path = Some(path);
        self
    }

    /// Sets the value that caused the violation.
    #[must_use]
    pub fn with_value(mut self, value: Term) -> Self {
        self.value = Some(value);
        self
    }

    /// Sets the result message.
    #[must_use]
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.result_message = Some(message.into());
        self
    }

    /// Sets the severity.
    #[must_use]
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.result_severity = severity;
        self
    }

    /// Adds a nested detail result.
    #[must_use]
    pub fn with_detail(mut self, detail: ValidationResult) -> Self {
        self.detail.push(detail);
        self
    }
}

/// A SHACL validation report.
#[derive(Debug, Clone)]
pub struct ValidationReport {
    /// Whether the data graph conforms to the shapes graph.
    conforms: bool,

    /// Individual validation results.
    results: Vec<ValidationResult>,
}

impl ValidationReport {
    /// Creates a new validation report.
    pub fn new() -> Self {
        Self {
            conforms: true,
            results: Vec::new(),
        }
    }

    /// Returns true if the data graph conforms to the shapes graph.
    pub fn conforms(&self) -> bool {
        self.conforms
    }

    /// Returns the validation results.
    pub fn results(&self) -> &[ValidationResult] {
        &self.results
    }

    /// Returns the number of violations.
    pub fn violation_count(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.result_severity == Severity::Violation)
            .count()
    }

    /// Returns the number of warnings.
    pub fn warning_count(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.result_severity == Severity::Warning)
            .count()
    }

    /// Returns the number of info results.
    pub fn info_count(&self) -> usize {
        self.results
            .iter()
            .filter(|r| r.result_severity == Severity::Info)
            .count()
    }

    /// Adds a validation result.
    pub fn add_result(&mut self, result: ValidationResult) {
        // Only violations affect conformance
        if result.result_severity == Severity::Violation {
            self.conforms = false;
        }
        self.results.push(result);
    }

    /// Merges another report into this one.
    pub fn merge(&mut self, other: ValidationReport) {
        if !other.conforms {
            self.conforms = false;
        }
        self.results.extend(other.results);
    }

    /// Returns true if there are no results.
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// Converts the report to an RDF graph.
    pub fn to_graph(&self) -> Graph {
        let mut graph = Graph::new();
        let report_node = BlankNode::default();

        // Add report type
        graph.insert(&Triple::new(
            report_node.clone(),
            rdf::TYPE,
            shacl::VALIDATION_REPORT,
        ));

        // Add conforms property
        graph.insert(&Triple::new(
            report_node.clone(),
            shacl::CONFORMS,
            Literal::new_typed_literal(if self.conforms { "true" } else { "false" }, xsd::BOOLEAN),
        ));

        // Add results
        for result in &self.results {
            let result_node = BlankNode::default();

            // Link result to report
            graph.insert(&Triple::new(
                report_node.clone(),
                shacl::RESULT,
                result_node.clone(),
            ));

            // Add result type
            graph.insert(&Triple::new(
                result_node.clone(),
                rdf::TYPE,
                shacl::VALIDATION_RESULT,
            ));

            // Add focus node
            graph.insert(&Triple::new(
                result_node.clone(),
                shacl::FOCUS_NODE,
                result.focus_node.clone(),
            ));

            // Add result path (if present)
            if let Some(path) = &result.result_path {
                if let Some(pred) = path.as_predicate() {
                    graph.insert(&Triple::new(
                        result_node.clone(),
                        shacl::RESULT_PATH,
                        pred.clone(),
                    ));
                }
            }

            // Add value (if present)
            if let Some(value) = &result.value {
                graph.insert(&Triple::new(
                    result_node.clone(),
                    shacl::VALUE,
                    value.clone(),
                ));
            }

            // Add source shape
            graph.insert(&Triple::new(
                result_node.clone(),
                shacl::SOURCE_SHAPE,
                result.source_shape.to_term(),
            ));

            // Add source constraint component
            graph.insert(&Triple::new(
                result_node.clone(),
                shacl::SOURCE_CONSTRAINT_COMPONENT,
                result.source_constraint_component.iri(),
            ));

            // Add result message (if present)
            if let Some(message) = &result.result_message {
                graph.insert(&Triple::new(
                    result_node.clone(),
                    shacl::RESULT_MESSAGE,
                    Literal::new_simple_literal(message),
                ));
            }

            // Add severity
            graph.insert(&Triple::new(
                result_node.clone(),
                shacl::RESULT_SEVERITY,
                result.result_severity.iri(),
            ));

            // Add nested details
            for detail in &result.detail {
                add_detail_to_graph(&mut graph, &result_node, detail);
            }
        }

        graph
    }
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self::new()
    }
}

fn add_detail_to_graph(graph: &mut Graph, parent: &BlankNode, detail: &ValidationResult) {
    let detail_node = BlankNode::default();

    // Link detail to parent
    graph.insert(&Triple::new(
        parent.clone(),
        shacl::DETAIL,
        detail_node.clone(),
    ));

    // Add detail type
    graph.insert(&Triple::new(
        detail_node.clone(),
        rdf::TYPE,
        shacl::VALIDATION_RESULT,
    ));

    // Add focus node
    graph.insert(&Triple::new(
        detail_node.clone(),
        shacl::FOCUS_NODE,
        detail.focus_node.clone(),
    ));

    // Add source constraint component
    graph.insert(&Triple::new(
        detail_node.clone(),
        shacl::SOURCE_CONSTRAINT_COMPONENT,
        detail.source_constraint_component.iri(),
    ));

    // Add result message (if present)
    if let Some(message) = &detail.result_message {
        graph.insert(&Triple::new(
            detail_node.clone(),
            shacl::RESULT_MESSAGE,
            Literal::new_simple_literal(message),
        ));
    }

    // Recursively add nested details
    for nested in &detail.detail {
        add_detail_to_graph(graph, &detail_node, nested);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::NamedNode;

    #[test]
    fn test_empty_report_conforms() {
        let report = ValidationReport::new();
        assert!(report.conforms());
        assert!(report.is_empty());
    }

    #[test]
    fn test_violation_fails_conformance() {
        let mut report = ValidationReport::new();
        let result = ValidationResult::new(
            Term::NamedNode(NamedNode::new("http://example.org/x").unwrap()),
            ShapeId::Named(NamedNode::new("http://example.org/Shape").unwrap()),
            ConstraintComponent::MinCount,
        );
        report.add_result(result);

        assert!(!report.conforms());
        assert_eq!(report.violation_count(), 1);
    }

    #[test]
    fn test_warning_does_not_fail_conformance() {
        let mut report = ValidationReport::new();
        let result = ValidationResult::new(
            Term::NamedNode(NamedNode::new("http://example.org/x").unwrap()),
            ShapeId::Named(NamedNode::new("http://example.org/Shape").unwrap()),
            ConstraintComponent::MinCount,
        )
        .with_severity(Severity::Warning);
        report.add_result(result);

        assert!(report.conforms());
        assert_eq!(report.warning_count(), 1);
    }

    #[test]
    fn test_report_to_graph() {
        let mut report = ValidationReport::new();
        let result = ValidationResult::new(
            Term::NamedNode(NamedNode::new("http://example.org/x").unwrap()),
            ShapeId::Named(NamedNode::new("http://example.org/Shape").unwrap()),
            ConstraintComponent::MinCount,
        )
        .with_message("Minimum count violation");
        report.add_result(result);

        let graph = report.to_graph();
        assert!(!graph.is_empty());

        // Check that conforms is false
        let conforms_values: Vec<_> = graph.triples_for_predicate(shacl::CONFORMS).collect();
        assert_eq!(conforms_values.len(), 1);
    }
}
