//! Error types for OWL operations.

use std::fmt;
use std::error::Error;

/// Errors that can occur during OWL operations.
#[derive(Debug)]
pub enum OwlError {
    /// Error parsing OWL from RDF.
    Parse(OwlParseError),

    /// Error during reasoning.
    Reasoning(ReasoningError),

    /// Ontology is inconsistent.
    Inconsistent(InconsistencyError),

    /// IRI parsing error.
    InvalidIri(oxiri::IriParseError),

    /// General error with message.
    Other(String),
}

impl fmt::Display for OwlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "OWL parse error: {}", e),
            Self::Reasoning(e) => write!(f, "Reasoning error: {}", e),
            Self::Inconsistent(e) => write!(f, "Inconsistency: {}", e),
            Self::InvalidIri(e) => write!(f, "Invalid IRI: {}", e),
            Self::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl Error for OwlError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Parse(e) => Some(e),
            Self::Reasoning(e) => Some(e),
            Self::Inconsistent(e) => Some(e),
            Self::InvalidIri(e) => Some(e),
            Self::Other(_) => None,
        }
    }
}

impl From<OwlParseError> for OwlError {
    fn from(e: OwlParseError) -> Self {
        Self::Parse(e)
    }
}

impl From<ReasoningError> for OwlError {
    fn from(e: ReasoningError) -> Self {
        Self::Reasoning(e)
    }
}

impl From<InconsistencyError> for OwlError {
    fn from(e: InconsistencyError) -> Self {
        Self::Inconsistent(e)
    }
}

impl From<oxiri::IriParseError> for OwlError {
    fn from(e: oxiri::IriParseError) -> Self {
        Self::InvalidIri(e)
    }
}

/// Errors that can occur during OWL parsing from RDF.
#[derive(Debug)]
pub struct OwlParseError {
    kind: ParseErrorKind,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseErrorKind {
    /// Missing required property.
    MissingProperty,
    /// Invalid property value type.
    InvalidValue,
    /// Unknown OWL construct.
    UnknownConstruct,
    /// Malformed RDF list.
    MalformedList,
    /// Circular reference detected.
    CircularReference,
    /// Invalid cardinality value.
    InvalidCardinality,
    /// General syntax error.
    Syntax,
}

impl OwlParseError {
    /// Creates a new parse error.
    pub fn new(kind: ParseErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Creates a missing property error.
    pub fn missing_property(property: &str) -> Self {
        Self::new(ParseErrorKind::MissingProperty, format!("Missing required property: {}", property))
    }

    /// Creates an invalid value error.
    pub fn invalid_value(message: impl Into<String>) -> Self {
        Self::new(ParseErrorKind::InvalidValue, message)
    }

    /// Creates a malformed list error.
    pub fn malformed_list(message: impl Into<String>) -> Self {
        Self::new(ParseErrorKind::MalformedList, message)
    }

    /// Creates a circular reference error.
    pub fn circular_reference(message: impl Into<String>) -> Self {
        Self::new(ParseErrorKind::CircularReference, message)
    }

    /// Returns the error kind.
    pub fn kind(&self) -> ParseErrorKind {
        self.kind
    }
}

impl fmt::Display for OwlParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl Error for OwlParseError {}

/// Errors that can occur during reasoning.
#[derive(Debug)]
pub struct ReasoningError {
    kind: ReasoningErrorKind,
    message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningErrorKind {
    /// Reasoning exceeded maximum iterations.
    MaxIterationsExceeded,
    /// Reasoning exceeded memory limit.
    MemoryLimitExceeded,
    /// Unsupported OWL construct for the profile.
    UnsupportedConstruct,
    /// Timeout during reasoning.
    Timeout,
    /// Internal error.
    Internal,
}

impl ReasoningError {
    /// Creates a new reasoning error.
    pub fn new(kind: ReasoningErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }

    /// Creates a max iterations exceeded error.
    pub fn max_iterations_exceeded(iterations: usize) -> Self {
        Self::new(
            ReasoningErrorKind::MaxIterationsExceeded,
            format!("Exceeded maximum iterations ({})", iterations),
        )
    }

    /// Creates an unsupported construct error.
    pub fn unsupported_construct(construct: &str, profile: &str) -> Self {
        Self::new(
            ReasoningErrorKind::UnsupportedConstruct,
            format!("{} is not supported in {} profile", construct, profile),
        )
    }

    /// Returns the error kind.
    pub fn kind(&self) -> ReasoningErrorKind {
        self.kind
    }
}

impl fmt::Display for ReasoningError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl Error for ReasoningError {}

/// Error indicating an inconsistent ontology.
#[derive(Debug, Clone)]
pub struct InconsistencyError {
    explanation: String,
    axioms: Vec<String>,
}

impl InconsistencyError {
    /// Creates a new inconsistency error.
    pub fn new(explanation: impl Into<String>) -> Self {
        Self {
            explanation: explanation.into(),
            axioms: Vec::new(),
        }
    }

    /// Creates an inconsistency error with contributing axioms.
    pub fn with_axioms(explanation: impl Into<String>, axioms: Vec<String>) -> Self {
        Self {
            explanation: explanation.into(),
            axioms,
        }
    }

    /// Returns the explanation.
    pub fn explanation(&self) -> &str {
        &self.explanation
    }

    /// Returns the axioms that contribute to the inconsistency.
    pub fn axioms(&self) -> &[String] {
        &self.axioms
    }
}

impl fmt::Display for InconsistencyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.explanation)?;
        if !self.axioms.is_empty() {
            write!(f, " (involved axioms: {})", self.axioms.join(", "))?;
        }
        Ok(())
    }
}

impl Error for InconsistencyError {}

/// Result type for OWL operations.
pub type OwlResult<T> = Result<T, OwlError>;
