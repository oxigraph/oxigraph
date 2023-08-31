use crate::io::ParseError as RdfParseError;
use crate::model::NamedNode;
use crate::sparql::results::ParseError as ResultsParseError;
use crate::sparql::ParseError;
use crate::storage::StorageError;
use std::convert::Infallible;
use std::error::Error;
use std::fmt;
use std::io;

/// A SPARQL evaluation error.
#[derive(Debug)]
#[non_exhaustive]
pub enum EvaluationError {
    /// An error in SPARQL parsing.
    Parsing(ParseError),
    /// An error from the storage.
    Storage(StorageError),
    /// An error while parsing an external RDF file.
    GraphParsing(RdfParseError),
    /// An error while parsing an external result file (likely from a federated query).
    ResultsParsing(ResultsParseError),
    /// An error returned during results serialization.
    ResultsSerialization(io::Error),
    /// Error during `SERVICE` evaluation
    Service(Box<dyn Error + Send + Sync + 'static>),
    /// Error when `CREATE` tries to create an already existing graph
    GraphAlreadyExists(NamedNode),
    /// Error when `DROP` or `CLEAR` tries to remove a not existing graph
    GraphDoesNotExist(NamedNode),
    /// The variable storing the `SERVICE` name is unbound
    UnboundService,
    /// The given `SERVICE` is not supported
    UnsupportedService(NamedNode),
    /// The given content media type returned from an HTTP response is not supported (`SERVICE` and `LOAD`)
    UnsupportedContentType(String),
    /// The `SERVICE` call has not returns solutions
    ServiceDoesNotReturnSolutions,
    /// The results are not a RDF graph
    NotAGraph,
}

impl fmt::Display for EvaluationError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parsing(error) => error.fmt(f),
            Self::Storage(error) => error.fmt(f),
            Self::GraphParsing(error) => error.fmt(f),
            Self::ResultsParsing(error) => error.fmt(f),
            Self::ResultsSerialization(error) => error.fmt(f),
            Self::Service(error) => error.fmt(f),
            Self::GraphAlreadyExists(graph) => write!(f, "The graph {graph} already exists"),
            Self::GraphDoesNotExist(graph) => write!(f, "The graph {graph} does not exist"),
            Self::UnboundService => write!(f, "The variable encoding the service name is unbound"),
            Self::UnsupportedService(service) => {
                write!(f, "The service {service} is not supported")
            }
            Self::UnsupportedContentType(content_type) => {
                write!(f, "The content media type {content_type} is not supported")
            }
            Self::ServiceDoesNotReturnSolutions => write!(
                f,
                "The service is not returning solutions but a boolean or a graph"
            ),
            Self::NotAGraph => write!(f, "The query results are not a RDF graph"),
        }
    }
}

impl Error for EvaluationError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Parsing(e) => Some(e),
            Self::Storage(e) => Some(e),
            Self::GraphParsing(e) => Some(e),
            Self::ResultsParsing(e) => Some(e),
            Self::ResultsSerialization(e) => Some(e),
            Self::Service(e) => {
                let e = Box::as_ref(e);
                Some(e)
            }
            Self::GraphAlreadyExists(_)
            | Self::GraphDoesNotExist(_)
            | Self::UnboundService
            | Self::UnsupportedService(_)
            | Self::UnsupportedContentType(_)
            | Self::ServiceDoesNotReturnSolutions
            | Self::NotAGraph => None,
        }
    }
}

impl From<Infallible> for EvaluationError {
    #[inline]
    fn from(error: Infallible) -> Self {
        match error {}
    }
}

impl From<ParseError> for EvaluationError {
    #[inline]
    fn from(error: ParseError) -> Self {
        Self::Parsing(error)
    }
}

impl From<StorageError> for EvaluationError {
    #[inline]
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

impl From<RdfParseError> for EvaluationError {
    #[inline]
    fn from(error: RdfParseError) -> Self {
        Self::GraphParsing(error)
    }
}

impl From<ResultsParseError> for EvaluationError {
    #[inline]
    fn from(error: ResultsParseError) -> Self {
        Self::ResultsParsing(error)
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
            EvaluationError::Service(error) => match error.downcast() {
                Ok(error) => *error,
                Err(error) => Self::new(io::ErrorKind::Other, error),
            },
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
