//! Error types for SHACL validation.

use oxrdf::{NamedNode, Term};

/// Main error type for SHACL operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ShaclError {
    /// Error parsing shapes graph.
    #[error(transparent)]
    Parse(#[from] ShaclParseError),

    /// Error during validation.
    #[error(transparent)]
    Validation(#[from] ShaclValidationError),
}

/// Error type for parsing shapes from RDF graphs.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ShaclParseError {
    /// Invalid shape definition.
    #[error("Invalid shape definition for {shape}: {message}")]
    InvalidShape { shape: Term, message: String },

    /// Missing required property.
    #[error("Missing required property {property} for shape {shape}")]
    MissingProperty { shape: Term, property: NamedNode },

    /// Invalid property value.
    #[error("Invalid value for property {property} in shape {shape}: expected {expected}, got {actual}")]
    InvalidPropertyValue {
        shape: Term,
        property: NamedNode,
        expected: String,
        actual: Term,
    },

    /// Invalid property path.
    #[error("Invalid property path in shape {shape}: {message}")]
    InvalidPropertyPath { shape: Term, message: String },

    /// Invalid RDF list.
    #[error("Invalid RDF list in shape {shape}: {message}")]
    InvalidRdfList { shape: Term, message: String },

    /// Cyclic shape reference detected.
    #[error("Cyclic shape reference detected: {message}")]
    CyclicReference { message: String },

    /// Invalid regex pattern.
    #[error("Invalid regex pattern '{pattern}': {message}")]
    InvalidRegex { pattern: String, message: String },
}

/// Error type for validation operations.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ShaclValidationError {
    /// Maximum recursion depth exceeded.
    #[error("Maximum recursion depth ({depth}) exceeded during validation")]
    MaxRecursionDepth { depth: usize },

    /// Invalid focus node.
    #[error("Invalid focus node: {message}")]
    InvalidFocusNode { message: String },

    /// Internal error.
    #[error("Internal validation error: {message}")]
    Internal { message: String },

    /// SPARQL evaluation error.
    #[cfg(feature = "sparql")]
    #[error("SPARQL evaluation error: {message}")]
    SparqlError { message: String },
}

impl ShaclParseError {
    /// Creates an invalid shape error.
    pub fn invalid_shape(shape: impl Into<Term>, message: impl Into<String>) -> Self {
        Self::InvalidShape {
            shape: shape.into(),
            message: message.into(),
        }
    }

    /// Creates a missing property error.
    pub fn missing_property(shape: impl Into<Term>, property: impl Into<NamedNode>) -> Self {
        Self::MissingProperty {
            shape: shape.into(),
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
            shape: shape.into(),
            property: property.into(),
            expected: expected.into(),
            actual: actual.into(),
        }
    }

    /// Creates an invalid property path error.
    pub fn invalid_property_path(shape: impl Into<Term>, message: impl Into<String>) -> Self {
        Self::InvalidPropertyPath {
            shape: shape.into(),
            message: message.into(),
        }
    }

    /// Creates an invalid RDF list error.
    pub fn invalid_rdf_list(shape: impl Into<Term>, message: impl Into<String>) -> Self {
        Self::InvalidRdfList {
            shape: shape.into(),
            message: message.into(),
        }
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
}

impl ShaclValidationError {
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

    /// Creates an internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }
}
