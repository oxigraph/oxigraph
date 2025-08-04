use crate::io::RdfParseError;
use crate::model::NamedNode;
use crate::sparql::SparqlSyntaxError;
use crate::sparql::results::QueryResultsParseError;
use crate::store::{CorruptionError, StorageError};
use oxrdf::{Term, Variable};
use spareval::QueryEvaluationError;
use std::convert::Infallible;
use std::error::Error;
use std::io;

/// A SPARQL evaluation error.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EvaluationError {
    /// An error in SPARQL parsing.
    #[deprecated(
        note = "Only used by the deprecated oxigraph::Query struct",
        since = "0.5.0"
    )]
    #[error(transparent)]
    Parsing(#[from] SparqlSyntaxError),
    /// An error from the storage.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// An error while parsing an external RDF file.
    #[error(transparent)]
    GraphParsing(#[from] RdfParseError),
    /// An error while parsing an external result file (likely from a federated query).
    #[error(transparent)]
    ResultsParsing(#[from] QueryResultsParseError),
    /// An error returned during result serialization.
    #[error(transparent)]
    ResultsSerialization(io::Error),
    /// Error during `SERVICE` evaluation
    #[error("{0}")]
    Service(#[source] Box<dyn Error + Send + Sync + 'static>),
    /// Error when `CREATE` tries to create an already existing graph
    #[error("The graph {0} already exists")]
    GraphAlreadyExists(NamedNode),
    /// Error when `DROP` or `CLEAR` tries to remove a not existing graph
    #[error("The graph {0} does not exist")]
    GraphDoesNotExist(NamedNode),
    /// The variable storing the `SERVICE` name is unbound
    #[error("The variable encoding the service name is unbound")]
    UnboundService,
    /// Invalid service name
    #[error("{0} is not a valid service name")]
    InvalidServiceName(Term),
    /// The given `SERVICE` is not supported
    #[error("The service {0} is not supported")]
    UnsupportedService(NamedNode),
    /// The given content media type returned from an HTTP response is not supported (`SERVICE` and `LOAD`)
    #[error("The content media type {0} is not supported")]
    UnsupportedContentType(String),
    /// The `SERVICE` call has not returned solutions
    #[error("The service is not returning solutions but a boolean or a graph")]
    ServiceDoesNotReturnSolutions,
    /// The results are not an RDF graph
    #[error("The query results are not a RDF graph")]
    NotAGraph,
    /// If a variable present in the given initial substitution is not present in the `SELECT` part of the query
    #[error("The SPARQL query does not contains variable {0} in its SELECT projection")]
    NotExistingSubstitutedVariable(Variable),
    #[doc(hidden)]
    #[error(transparent)]
    Unexpected(Box<dyn Error + Send + Sync>),
}

impl From<Infallible> for EvaluationError {
    #[inline]
    fn from(error: Infallible) -> Self {
        match error {}
    }
}

impl From<QueryEvaluationError> for EvaluationError {
    fn from(error: QueryEvaluationError) -> Self {
        match error {
            QueryEvaluationError::Dataset(error) => match error.downcast() {
                Ok(error) => Self::Storage(*error),
                Err(error) => Self::Unexpected(error),
            },
            QueryEvaluationError::Service(error) => Self::Service(error),
            QueryEvaluationError::UnexpectedDefaultGraph => Self::Storage(
                CorruptionError::new("Unexpected default graph returned from the storage").into(),
            ),
            QueryEvaluationError::UnboundService => Self::UnboundService,
            QueryEvaluationError::UnsupportedService(name) => Self::UnsupportedService(name),
            QueryEvaluationError::NotExistingSubstitutedVariable(v) => {
                Self::NotExistingSubstitutedVariable(v)
            }
            QueryEvaluationError::InvalidServiceName(name) => Self::InvalidServiceName(name),
            #[cfg(feature = "rdf-12")]
            QueryEvaluationError::InvalidStorageTripleTerm => Self::Storage(
                CorruptionError::new(
                    "The storage returned a triple term that is not a valid RDF 1.2 term",
                )
                .into(),
            ),
            e => Self::Unexpected(Box::new(e)),
        }
    }
}

impl From<EvaluationError> for io::Error {
    #[inline]
    fn from(error: EvaluationError) -> Self {
        match error {
            #[expect(deprecated)]
            EvaluationError::Parsing(error) => Self::new(io::ErrorKind::InvalidData, error),
            EvaluationError::GraphParsing(error) => error.into(),
            EvaluationError::ResultsParsing(error) => error.into(),
            EvaluationError::ResultsSerialization(error) => error,
            EvaluationError::Storage(error) => error.into(),
            EvaluationError::Service(error) | EvaluationError::Unexpected(error) => {
                match error.downcast() {
                    Ok(error) => *error,
                    Err(error) => Self::other(error),
                }
            }
            EvaluationError::GraphAlreadyExists(_)
            | EvaluationError::GraphDoesNotExist(_)
            | EvaluationError::UnboundService
            | EvaluationError::InvalidServiceName(_)
            | EvaluationError::UnsupportedService(_)
            | EvaluationError::UnsupportedContentType(_)
            | EvaluationError::ServiceDoesNotReturnSolutions
            | EvaluationError::NotAGraph
            | EvaluationError::NotExistingSubstitutedVariable(_) => {
                Self::new(io::ErrorKind::InvalidInput, error)
            }
        }
    }
}
