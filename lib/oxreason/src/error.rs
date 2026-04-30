//! Error types for the oxreason crate.
//!
//! Every fallible public method returns one of these two enums. Both are
//! `#[non_exhaustive]` so new variants can be added without a breaking
//! change once the rule engine and SHACL validator are implemented.

use std::fmt;
use thiserror::Error;

/// Errors raised by the OWL 2 RL reasoner.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ReasonError {
    /// The requested reasoning profile or rule is not yet implemented.
    ///
    /// Returned by every [`crate::Reasoner`] method in the current scaffold.
    #[error("reasoning capability not implemented yet: {0}")]
    NotImplemented(&'static str),

    /// A triple referenced an IRI that could not be parsed.
    #[error("invalid IRI encountered during reasoning: {0}")]
    InvalidIri(String),

    /// Reasoning was cancelled, for example because a caller provided
    /// timeout or cancellation token fired.
    #[error("reasoning was cancelled before it could complete")]
    Cancelled,

    /// A rule produced a triple that the target graph rejected.
    #[error("failed to write inferred triple: {0}")]
    Write(String),

    /// Reasoning detected an inconsistency in the input graph, for example
    /// an individual typed as two classes declared owl:disjointWith. The
    /// reasoner stops materialising further triples when a clash is found
    /// because every subsequent inference would be vacuous under classical
    /// OWL semantics.
    #[error(
        "inconsistent graph: individual {individual} is typed as disjoint classes {class_a} and {class_b}"
    )]
    Inconsistent {
        /// The individual that carries both conflicting types.
        individual: String,
        /// One of the disjoint classes.
        class_a: String,
        /// The other disjoint class.
        class_b: String,
    },

    /// Reasoning detected an inconsistency that is not a cax-dw clash. Covers
    /// cls-nothing2 (an individual typed as `owl:Nothing`), prp-irp (a
    /// reflexive edge on an `owl:IrreflexiveProperty`), prp-asyp (both
    /// directions of an `owl:AsymmetricProperty` between the same two
    /// individuals), and prp-pdw (an individual pair linked by two
    /// `owl:propertyDisjointWith` properties). The `message` carries a
    /// rule-specific human description of the clash, prefixed with the rule
    /// identifier.
    #[error("inconsistent graph: {message}")]
    InconsistentAxiom {
        /// Rule-specific human description of the clash.
        message: String,
    },
}

/// Errors raised by [`crate::Reasoner::expand_streaming`].
///
/// A streaming reasoning run can fail for two reasons: an engine-level
/// issue (inconsistency, unsupported profile, etc.) surfaces as
/// [`ReasonStreamError::Reason`] wrapping a regular [`ReasonError`]; an
/// error from the caller-provided sink (for example an I/O failure while
/// writing an inferred triple back into storage) surfaces as
/// [`ReasonStreamError::Sink`] preserving the original sink error type.
#[derive(Debug)]
pub enum ReasonStreamError<E> {
    /// The reasoning engine could not complete the run.
    Reason(ReasonError),
    /// The caller-provided sink rejected a freshly materialised triple.
    Sink(E),
}

impl<E: fmt::Display> fmt::Display for ReasonStreamError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Reason(e) => write!(f, "{e}"),
            Self::Sink(e) => write!(f, "reasoning sink error: {e}"),
        }
    }
}

impl<E: std::error::Error + 'static> std::error::Error for ReasonStreamError<E> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Reason(e) => Some(e),
            Self::Sink(e) => Some(e),
        }
    }
}

impl<E> From<ReasonError> for ReasonStreamError<E> {
    fn from(err: ReasonError) -> Self {
        Self::Reason(err)
    }
}

/// Errors raised by the SHACL validator.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ValidateError {
    /// The requested SHACL feature is not yet implemented.
    ///
    /// Returned by every [`crate::Validator`] method in the current
    /// scaffold.
    #[error("SHACL capability not implemented yet: {0}")]
    NotImplemented(&'static str),

    /// A shapes graph referenced a constraint component that oxreason does
    /// not support.
    #[error("unsupported SHACL constraint component: {0}")]
    UnsupportedConstraint(String),

    /// Evaluating a shape required executing a SPARQL query that failed.
    #[error("SPARQL evaluation failed during validation: {0}")]
    SparqlFailure(String),
}
