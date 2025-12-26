//! Validation result types
//!
//! This module provides types for representing ShEx validation results,
//! including detailed constraint violation information.

use oxrdf::{NamedNode, NamedNodeRef, Term};
use std::fmt;

/// Validation result
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationResult {
    valid: bool,
    errors: Vec<String>,
}

impl ValidationResult {
    /// Create a valid result
    pub fn valid() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
        }
    }

    /// Create an invalid result
    pub fn invalid(errors: Vec<String>) -> Self {
        Self {
            valid: false,
            errors,
        }
    }

    /// Check if the result is valid
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Get validation errors
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// Add an error message
    pub fn add_error(&mut self, error: String) {
        self.valid = false;
        self.errors.push(error);
    }
}

impl Default for ValidationResult {
    fn default() -> Self {
        Self::valid()
    }
}

/// Detailed validation report with structured constraint violations.
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationReport {
    /// Whether the validation conforms (no violations).
    conforms: bool,
    /// Individual constraint violations.
    violations: Vec<ConstraintViolation>,
}

impl ValidationReport {
    /// Creates a new empty validation report (conforming).
    pub fn new() -> Self {
        Self {
            conforms: true,
            violations: Vec::new(),
        }
    }

    /// Creates a report with a single violation.
    pub fn with_violation(violation: ConstraintViolation) -> Self {
        Self {
            conforms: false,
            violations: vec![violation],
        }
    }

    /// Creates a report with multiple violations.
    pub fn with_violations(violations: Vec<ConstraintViolation>) -> Self {
        let conforms = violations.is_empty();
        Self {
            conforms,
            violations,
        }
    }

    /// Returns true if the validation conforms (no violations).
    pub fn conforms(&self) -> bool {
        self.conforms
    }

    /// Returns the violations.
    pub fn violations(&self) -> &[ConstraintViolation] {
        &self.violations
    }

    /// Returns the number of violations.
    pub fn violation_count(&self) -> usize {
        self.violations.len()
    }

    /// Adds a violation to the report.
    pub fn add_violation(&mut self, violation: ConstraintViolation) {
        self.conforms = false;
        self.violations.push(violation);
    }

    /// Merges another report into this one.
    pub fn merge(&mut self, other: ValidationReport) {
        if !other.conforms {
            self.conforms = false;
        }
        self.violations.extend(other.violations);
    }

    /// Returns true if there are no violations.
    pub fn is_empty(&self) -> bool {
        self.violations.is_empty()
    }

    /// Converts to a simple ValidationResult.
    pub fn to_validation_result(&self) -> ValidationResult {
        if self.conforms {
            ValidationResult::valid()
        } else {
            let errors = self
                .violations
                .iter()
                .map(|v| v.message.clone())
                .collect();
            ValidationResult::invalid(errors)
        }
    }
}

impl Default for ValidationReport {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for ValidationReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.conforms {
            write!(f, "Conforms (no violations)")
        } else {
            write!(f, "{} violation(s)", self.violations.len())?;
            for (i, violation) in self.violations.iter().enumerate() {
                write!(f, "\n  {}. {}", i + 1, violation)?;
            }
            Ok(())
        }
    }
}

/// A specific constraint violation found during validation.
#[derive(Debug, Clone, PartialEq)]
pub struct ConstraintViolation {
    /// The focus node that failed validation.
    pub focus_node: Term,
    /// The shape that was being validated against.
    pub shape_id: ShapeId,
    /// The type of constraint that was violated.
    pub constraint_type: ConstraintType,
    /// The predicate that was being validated (for triple constraints).
    pub predicate: Option<NamedNode>,
    /// The value that caused the violation.
    pub value: Option<Term>,
    /// Human-readable message describing the violation.
    pub message: String,
}

impl ConstraintViolation {
    /// Creates a new constraint violation.
    pub fn new(
        focus_node: Term,
        shape_id: ShapeId,
        constraint_type: ConstraintType,
        message: impl Into<String>,
    ) -> Self {
        Self {
            focus_node,
            shape_id,
            constraint_type,
            predicate: None,
            value: None,
            message: message.into(),
        }
    }

    /// Sets the predicate.
    #[must_use]
    pub fn with_predicate(mut self, predicate: NamedNode) -> Self {
        self.predicate = Some(predicate);
        self
    }

    /// Sets the value.
    #[must_use]
    pub fn with_value(mut self, value: Term) -> Self {
        self.value = Some(value);
        self
    }
}

impl fmt::Display for ConstraintViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.constraint_type, self.message)?;
        if let Some(pred) = &self.predicate {
            write!(f, " [predicate: {}]", pred)?;
        }
        if let Some(val) = &self.value {
            write!(f, " [value: {}]", val)?;
        }
        Ok(())
    }
}

/// Identifier for a shape (named or blank node).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ShapeId {
    /// A named shape identified by an IRI.
    Named(NamedNode),
    /// A blank node shape (anonymous).
    Blank(String),
}

impl ShapeId {
    /// Returns the IRI if this is a named shape.
    pub fn as_named_node(&self) -> Option<&NamedNode> {
        match self {
            ShapeId::Named(n) => Some(n),
            ShapeId::Blank(_) => None,
        }
    }

    /// Returns true if this is a named shape.
    pub fn is_named(&self) -> bool {
        matches!(self, ShapeId::Named(_))
    }

    /// Returns true if this is a blank node shape.
    pub fn is_blank(&self) -> bool {
        matches!(self, ShapeId::Blank(_))
    }

    /// Converts to a Term for RDF output.
    pub fn to_term(&self) -> Term {
        match self {
            ShapeId::Named(n) => Term::NamedNode(n.clone()),
            ShapeId::Blank(id) => {
                // In practice, we'd use a proper BlankNode here
                Term::NamedNode(NamedNode::new(format!("_:{}", id)).unwrap_or_else(|_| {
                    NamedNode::new("http://example.org/blank").unwrap()
                }))
            }
        }
    }
}

impl fmt::Display for ShapeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShapeId::Named(n) => write!(f, "<{}>", n.as_str()),
            ShapeId::Blank(id) => write!(f, "_:{}", id),
        }
    }
}

impl From<NamedNode> for ShapeId {
    fn from(node: NamedNode) -> Self {
        ShapeId::Named(node)
    }
}

impl From<NamedNodeRef<'_>> for ShapeId {
    fn from(node: NamedNodeRef<'_>) -> Self {
        ShapeId::Named(node.into_owned())
    }
}

/// The type of constraint that was violated.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConstraintType {
    /// Node kind constraint (IRI, Literal, BlankNode, etc.)
    NodeKind,
    /// Datatype constraint for literals.
    Datatype,
    /// Minimum cardinality constraint.
    MinCardinality,
    /// Maximum cardinality constraint.
    MaxCardinality,
    /// Exact cardinality constraint.
    ExactCardinality,
    /// Shape reference constraint.
    ShapeRef,
    /// String pattern constraint.
    Pattern,
    /// Minimum length constraint.
    MinLength,
    /// Maximum length constraint.
    MaxLength,
    /// Minimum value constraint.
    MinValue,
    /// Maximum value constraint.
    MaxValue,
    /// Value constraint (fixed value).
    Value,
    /// Values constraint (enumeration).
    Values,
    /// ShapeAnd constraint (all shapes must match).
    ShapeAnd,
    /// ShapeOr constraint (at least one shape must match).
    ShapeOr,
    /// ShapeNot constraint (shape must not match).
    ShapeNot,
    /// Closed shape constraint (no extra properties).
    Closed,
    /// Language constraint.
    Language,
    /// General validation error.
    ValidationError,
}

impl fmt::Display for ConstraintType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConstraintType::NodeKind => write!(f, "NodeKind"),
            ConstraintType::Datatype => write!(f, "Datatype"),
            ConstraintType::MinCardinality => write!(f, "MinCardinality"),
            ConstraintType::MaxCardinality => write!(f, "MaxCardinality"),
            ConstraintType::ExactCardinality => write!(f, "ExactCardinality"),
            ConstraintType::ShapeRef => write!(f, "ShapeRef"),
            ConstraintType::Pattern => write!(f, "Pattern"),
            ConstraintType::MinLength => write!(f, "MinLength"),
            ConstraintType::MaxLength => write!(f, "MaxLength"),
            ConstraintType::MinValue => write!(f, "MinValue"),
            ConstraintType::MaxValue => write!(f, "MaxValue"),
            ConstraintType::Value => write!(f, "Value"),
            ConstraintType::Values => write!(f, "Values"),
            ConstraintType::ShapeAnd => write!(f, "ShapeAnd"),
            ConstraintType::ShapeOr => write!(f, "ShapeOr"),
            ConstraintType::ShapeNot => write!(f, "ShapeNot"),
            ConstraintType::Closed => write!(f, "Closed"),
            ConstraintType::Language => write!(f, "Language"),
            ConstraintType::ValidationError => write!(f, "ValidationError"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_result_valid() {
        let result = ValidationResult::valid();
        assert!(result.is_valid());
        assert!(result.errors().is_empty());
    }

    #[test]
    fn test_validation_result_invalid() {
        let result = ValidationResult::invalid(vec!["error 1".to_string(), "error 2".to_string()]);
        assert!(!result.is_valid());
        assert_eq!(result.errors().len(), 2);
    }

    #[test]
    fn test_empty_report_conforms() {
        let report = ValidationReport::new();
        assert!(report.conforms());
        assert!(report.is_empty());
        assert_eq!(report.violation_count(), 0);
    }

    #[test]
    fn test_report_with_violation() {
        let violation = ConstraintViolation::new(
            Term::NamedNode(NamedNode::new("http://example.org/x").unwrap()),
            ShapeId::Named(NamedNode::new("http://example.org/Shape").unwrap()),
            ConstraintType::MinCardinality,
            "Minimum cardinality not met",
        );
        let report = ValidationReport::with_violation(violation);
        assert!(!report.conforms());
        assert_eq!(report.violation_count(), 1);
    }

    #[test]
    fn test_add_violation() {
        let mut report = ValidationReport::new();
        assert!(report.conforms());

        let violation = ConstraintViolation::new(
            Term::NamedNode(NamedNode::new("http://example.org/x").unwrap()),
            ShapeId::Named(NamedNode::new("http://example.org/Shape").unwrap()),
            ConstraintType::Datatype,
            "Invalid datatype",
        );
        report.add_violation(violation);

        assert!(!report.conforms());
        assert_eq!(report.violation_count(), 1);
    }

    #[test]
    fn test_merge_reports() {
        let mut report1 = ValidationReport::new();
        let mut report2 = ValidationReport::new();

        let violation = ConstraintViolation::new(
            Term::NamedNode(NamedNode::new("http://example.org/x").unwrap()),
            ShapeId::Named(NamedNode::new("http://example.org/Shape").unwrap()),
            ConstraintType::NodeKind,
            "Invalid node kind",
        );
        report2.add_violation(violation);

        report1.merge(report2);
        assert!(!report1.conforms());
        assert_eq!(report1.violation_count(), 1);
    }

    #[test]
    fn test_report_to_validation_result() {
        let mut report = ValidationReport::new();
        let result = report.to_validation_result();
        assert!(result.is_valid());

        report.add_violation(ConstraintViolation::new(
            Term::NamedNode(NamedNode::new("http://example.org/x").unwrap()),
            ShapeId::Named(NamedNode::new("http://example.org/Shape").unwrap()),
            ConstraintType::Datatype,
            "Test error",
        ));

        let result = report.to_validation_result();
        assert!(!result.is_valid());
        assert_eq!(result.errors().len(), 1);
    }

    #[test]
    fn test_shape_id_display() {
        let named = ShapeId::Named(NamedNode::new("http://example.org/Shape").unwrap());
        assert_eq!(format!("{}", named), "<http://example.org/Shape>");

        let blank = ShapeId::Blank("b1".to_string());
        assert_eq!(format!("{}", blank), "_:b1");
    }
}
