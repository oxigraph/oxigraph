use crate::io::RdfParseError;
use crate::model::NamedNode;
use crate::store::{CorruptionError, StorageError};
use oxrdf::{Term, Variable};
use spareval::QueryEvaluationError;
use spargebra::SparqlSyntaxError;
use std::convert::Infallible;
use std::error::Error;
use std::io;

/// An error from SPARQL UPDATE evaluation
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum UpdateEvaluationError {
    /// An error from the storage.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// An error while parsing an external RDF file.
    #[error(transparent)]
    GraphParsing(#[from] RdfParseError),
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
    /// The given content media type returned from an HTTP response is not supported (`LOAD`)
    #[error("The content media type {0} is not supported")]
    UnsupportedContentType(String),
    /// If a variable present in the given initial substitution is not present in the `SELECT` part of the query
    #[error("The SPARQL query does not contains variable {0} in its SELECT projection")]
    NotExistingSubstitutedVariable(Variable),
    #[doc(hidden)]
    #[error(transparent)]
    Unexpected(Box<dyn Error + Send + Sync>),
}

impl From<Infallible> for UpdateEvaluationError {
    #[inline]
    fn from(error: Infallible) -> Self {
        match error {}
    }
}

impl From<QueryEvaluationError> for UpdateEvaluationError {
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

impl From<UpdateEvaluationError> for io::Error {
    #[inline]
    fn from(error: UpdateEvaluationError) -> Self {
        match error {
            UpdateEvaluationError::Storage(error) => error.into(),
            UpdateEvaluationError::GraphParsing(error) => error.into(),
            UpdateEvaluationError::Service(error) | UpdateEvaluationError::Unexpected(error) => {
                match error.downcast() {
                    Ok(error) => *error,
                    Err(error) => Self::other(error),
                }
            }
            UpdateEvaluationError::GraphAlreadyExists(_)
            | UpdateEvaluationError::GraphDoesNotExist(_)
            | UpdateEvaluationError::UnboundService
            | UpdateEvaluationError::InvalidServiceName(_)
            | UpdateEvaluationError::UnsupportedService(_)
            | UpdateEvaluationError::UnsupportedContentType(_)
            | UpdateEvaluationError::NotExistingSubstitutedVariable(_) => {
                Self::new(io::ErrorKind::InvalidInput, error)
            }
        }
    }
}

// TODO: remove when removing the Store::update method
#[doc(hidden)]
impl From<SparqlSyntaxError> for UpdateEvaluationError {
    #[inline]
    fn from(error: SparqlSyntaxError) -> Self {
        Self::Unexpected(Box::new(error))
    }
}
