//! Error types for ShEx validation.

use oxrdf::{NamedNode, Term};

/// Main error type for ShEx operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ShexError {
    /// Error parsing shapes schema.
    #[error(transparent)]
    Parse(#[from] ShexParseError),

    /// Error during validation.
    #[error(transparent)]
    Validation(#[from] ShexValidationError),
}

/// Error type for parsing shapes from RDF graphs or ShExC.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ShexParseError {
    /// Invalid shape expression.
    #[error("Invalid shape expression for {shape}: {message}")]
    InvalidShape { shape: Box<Term>, message: String },

    /// Missing required property.
    #[error("Missing required property {property} for shape {shape}")]
    MissingProperty {
        shape: Box<Term>,
        property: NamedNode,
    },

    /// Invalid property value.
    #[error(
        "Invalid value for property {property} in shape {shape}: expected {expected}, got {actual}"
    )]
    InvalidPropertyValue {
        shape: Box<Term>,
        property: NamedNode,
        expected: String,
        actual: Box<Term>,
    },

    /// Invalid triple constraint.
    #[error("Invalid triple constraint in shape {shape}: {message}")]
    InvalidTripleConstraint { shape: Box<Term>, message: String },

    /// Invalid node constraint.
    #[error("Invalid node constraint in shape {shape}: {message}")]
    InvalidNodeConstraint { shape: Box<Term>, message: String },

    /// Invalid cardinality.
    #[error("Invalid cardinality in shape {shape}: min={min}, max={max:?}")]
    InvalidCardinality {
        shape: Box<Term>,
        min: u32,
        max: Option<u32>,
    },

    /// Invalid RDF list.
    #[error("Invalid RDF list in shape {shape}: {message}")]
    InvalidRdfList { shape: Box<Term>, message: String },

    /// Circular RDF list detected.
    #[error("Circular RDF list detected at node {node}")]
    CircularList { node: Box<Term> },

    /// RDF list too long.
    #[error("RDF list exceeds maximum length of {max_length}")]
    ListTooLong { max_length: usize },

    /// Cyclic shape reference detected.
    #[error("Cyclic shape reference detected: {message}")]
    CyclicReference { message: String },

    /// Invalid regex pattern.
    #[error("Invalid regex pattern '{pattern}': {message}")]
    InvalidRegex { pattern: String, message: String },

    /// Invalid shape label.
    #[error("Invalid shape label: {message}")]
    InvalidShapeLabel { message: String },

    /// Undefined shape reference.
    #[error("Undefined shape reference: {label}")]
    UndefinedShapeRef { label: String },

    /// Invalid value set constraint.
    #[error("Invalid value set constraint in shape {shape}: {message}")]
    InvalidValueSet { shape: Box<Term>, message: String },
}

/// Error type for validation operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ShexValidationError {
    /// Maximum recursion depth exceeded.
    #[error("Maximum recursion depth ({depth}) exceeded during validation")]
    MaxRecursionDepth { depth: usize },

    /// Invalid focus node.
    #[error("Invalid focus node: {message}")]
    InvalidFocusNode { message: String },

    /// Shape not found.
    #[error("Shape not found: {label}")]
    ShapeNotFound { label: String },

    /// Cardinality violation.
    #[error("Cardinality violation: expected {expected}, got {actual}")]
    CardinalityViolation { expected: String, actual: usize },

    /// Node constraint violation.
    #[error("Node constraint violation: {message}")]
    NodeConstraintViolation { message: String },

    /// Triple constraint violation.
    #[error("Triple constraint violation on predicate {predicate}: {message}")]
    TripleConstraintViolation {
        predicate: NamedNode,
        message: String,
    },

    /// Internal error.
    #[error("Internal validation error: {message}")]
    Internal { message: String },
}

impl ShexParseError {
    /// Creates an invalid shape error.
    pub fn invalid_shape(shape: impl Into<Term>, message: impl Into<String>) -> Self {
        Self::InvalidShape {
            shape: Box::new(shape.into()),
            message: message.into(),
        }
    }

    /// Creates a missing property error.
    pub fn missing_property(shape: impl Into<Term>, property: impl Into<NamedNode>) -> Self {
        Self::MissingProperty {
            shape: Box::new(shape.into()),
            property: property.into(),
        }
    }

    /// Creates an invalid property value error.
    pub fn invalid_property_value(
        shape: impl Into<Term>,
        property: impl Into<NamedNode>,
        expected: impl Into<String>,
        actual: impl Into<Term>,
    ) -> Self {
        Self::InvalidPropertyValue {
            shape: Box::new(shape.into()),
            property: property.into(),
            expected: expected.into(),
            actual: Box::new(actual.into()),
        }
    }

    /// Creates an invalid triple constraint error.
    pub fn invalid_triple_constraint(shape: impl Into<Term>, message: impl Into<String>) -> Self {
        Self::InvalidTripleConstraint {
            shape: Box::new(shape.into()),
            message: message.into(),
        }
    }

    /// Creates an invalid node constraint error.
    pub fn invalid_node_constraint(shape: impl Into<Term>, message: impl Into<String>) -> Self {
        Self::InvalidNodeConstraint {
            shape: Box::new(shape.into()),
            message: message.into(),
        }
    }

    /// Creates an invalid cardinality error.
    pub fn invalid_cardinality(shape: impl Into<Term>, min: u32, max: Option<u32>) -> Self {
        Self::InvalidCardinality {
            shape: Box::new(shape.into()),
            min,
            max,
        }
    }

    /// Creates an invalid RDF list error.
    pub fn invalid_rdf_list(shape: impl Into<Term>, message: impl Into<String>) -> Self {
        Self::InvalidRdfList {
            shape: Box::new(shape.into()),
            message: message.into(),
        }
    }

    /// Creates a circular list error.
    pub fn circular_list(node: impl Into<Term>) -> Self {
        Self::CircularList {
            node: Box::new(node.into()),
        }
    }

    /// Creates a list too long error.
    pub fn list_too_long(max_length: usize) -> Self {
        Self::ListTooLong { max_length }
    }

    /// Creates a cyclic reference error.
    pub fn cyclic_reference(message: impl Into<String>) -> Self {
        Self::CyclicReference {
            message: message.into(),
        }
    }

    /// Creates an invalid regex error.
    pub fn invalid_regex(pattern: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidRegex {
            pattern: pattern.into(),
            message: message.into(),
        }
    }

    /// Creates an invalid shape label error.
    pub fn invalid_shape_label(message: impl Into<String>) -> Self {
        Self::InvalidShapeLabel {
            message: message.into(),
        }
    }

    /// Creates an undefined shape reference error.
    pub fn undefined_shape_ref(label: impl Into<String>) -> Self {
        Self::UndefinedShapeRef {
            label: label.into(),
        }
    }

    /// Creates an invalid value set error.
    pub fn invalid_value_set(shape: impl Into<Term>, message: impl Into<String>) -> Self {
        Self::InvalidValueSet {
            shape: Box::new(shape.into()),
            message: message.into(),
        }
    }
}

impl ShexValidationError {
    /// Creates a max recursion depth error.
    pub fn max_recursion_depth(depth: usize) -> Self {
        Self::MaxRecursionDepth { depth }
    }

    /// Creates an invalid focus node error.
    pub fn invalid_focus_node(message: impl Into<String>) -> Self {
        Self::InvalidFocusNode {
            message: message.into(),
        }
    }

    /// Creates a shape not found error.
    pub fn shape_not_found(label: impl Into<String>) -> Self {
        Self::ShapeNotFound {
            label: label.into(),
        }
    }

    /// Creates a cardinality violation error.
    pub fn cardinality_violation(expected: impl Into<String>, actual: usize) -> Self {
        Self::CardinalityViolation {
            expected: expected.into(),
            actual,
        }
    }

    /// Creates a node constraint violation error.
    pub fn node_constraint_violation(message: impl Into<String>) -> Self {
        Self::NodeConstraintViolation {
            message: message.into(),
        }
    }

    /// Creates a triple constraint violation error.
    pub fn triple_constraint_violation(
        predicate: impl Into<NamedNode>,
        message: impl Into<String>,
    ) -> Self {
        Self::TripleConstraintViolation {
            predicate: predicate.into(),
            message: message.into(),
        }
    }

    /// Creates an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }
}
