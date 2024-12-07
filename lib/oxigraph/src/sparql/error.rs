use crate::io::RdfParseError;
use crate::model::NamedNode;
use crate::sparql::results::QueryResultsParseError as ResultsParseError;
use crate::sparql::SparqlSyntaxError;
use crate::store::{CorruptionError, StorageError};
use spareval::QueryEvaluationError;
use std::convert::Infallible;
use std::error::Error;
use std::io;

/// A SPARQL evaluation error.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EvaluationError {
    /// An error in SPARQL parsing.
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
    ResultsParsing(#[from] ResultsParseError),
    /// An error returned during results serialization.
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
    /// The given `SERVICE` is not supported
    #[error("The service {0} is not supported")]
    UnsupportedService(NamedNode),
    /// The given content media type returned from an HTTP response is not supported (`SERVICE` and `LOAD`)
    #[error("The content media type {0} is not supported")]
    UnsupportedContentType(String),
    /// The `SERVICE` call has not returns solutions
    #[error("The service is not returning solutions but a boolean or a graph")]
    ServiceDoesNotReturnSolutions,
    /// The results are not a RDF graph
    #[error("The query results are not a RDF graph")]
    NotAGraph,
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
                CorruptionError::new("Unexpected default graph in SPARQL results").into(),
            ),
            e => Self::Storage(
                CorruptionError::new(format!("Unsupported SPARQL evaluation error: {e}")).into(),
            ),
        }
    }
}

impl From<EvaluationError> for io::Error {
    #[inline]
    fn from(error: EvaluationError) -> Self {
        match error {
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
            | EvaluationError::UnsupportedService(_)
            | EvaluationError::UnsupportedContentType(_)
            | EvaluationError::ServiceDoesNotReturnSolutions
            | EvaluationError::NotAGraph => Self::new(io::ErrorKind::InvalidInput, error),
        }
    }
}
