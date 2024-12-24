use oxrdf::{NamedNode, Term, Variable};
use std::convert::Infallible;
use std::error::Error;

/// A SPARQL evaluation error
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum QueryEvaluationError {
    /// Error from the underlying RDF dataset
    #[error(transparent)]
    Dataset(Box<dyn Error + Send + Sync>),
    /// Error during `SERVICE` evaluation
    #[error("{0}")]
    Service(#[source] Box<dyn Error + Send + Sync>),
    /// If a variable present in the given initial substitution is not present in the `SELECT` part of the query
    #[error("The SPARQL query does not contains variable {0} in its SELECT projection")]
    NotExistingSubstitutedVariable(Variable),
    /// Error if the dataset returns the default graph even if a named graph is expected
    #[error("The SPARQL dataset returned the default graph even if a named graph is expected")]
    UnexpectedDefaultGraph,
    /// The variable storing the `SERVICE` name is unbound
    #[error("The variable encoding the service name is unbound")]
    UnboundService,
    /// Invalid service name
    #[error("{0} is not a valid service name")]
    InvalidServiceName(Term),
    /// The given `SERVICE` is not supported
    #[error("The service {0} is not supported")]
    UnsupportedService(NamedNode),
    #[cfg(feature = "rdf-star")]
    #[error("The storage provided a triple term that is not a valid RDF-star term")]
    InvalidStorageTripleTerm,
}

impl From<Infallible> for QueryEvaluationError {
    #[inline]
    fn from(error: Infallible) -> Self {
        match error {}
    }
}
