//! Error types for the oxreason crate.
//!
//! Every fallible public method returns one of these two enums. Both are
//! `#[non_exhaustive]` so new variants can be added without a breaking
//! change once the rule engine and SHACL validator are implemented.

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
