use crate::io::read::ParseError;
use crate::storage::StorageError;
use std::convert::Infallible;
use std::error;
use std::fmt;
use std::io;

/// A SPARQL evaluation error.
#[derive(Debug)]
#[non_exhaustive]
pub enum EvaluationError {
    /// An error in SPARQL parsing.
    Parsing(spargebra::ParseError),
    /// An error from the storage.
    Storage(StorageError),
    /// An error while parsing an external RDF file.
    GraphParsing(ParseError),
    /// An error while parsing an external result file (likely from a federated query).
    ResultsParsing(sparesults::ParseError),
    /// An error returned during store IOs or during results write.
    Io(io::Error),
    /// An error returned during the query evaluation itself (not supported custom function...).
    Query(QueryError),
}

/// An error returned during the query evaluation itself (not supported custom function...).
#[derive(Debug)]
pub struct QueryError {
    inner: QueryErrorKind,
}

#[derive(Debug)]
enum QueryErrorKind {
    Msg { msg: String },
    Other(Box<dyn error::Error + Send + Sync + 'static>),
}

impl fmt::Display for EvaluationError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parsing(error) => error.fmt(f),
            Self::Storage(error) => error.fmt(f),
            Self::GraphParsing(error) => error.fmt(f),
            Self::ResultsParsing(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
            Self::Query(error) => error.fmt(f),
        }
    }
}

impl fmt::Display for QueryError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            QueryErrorKind::Msg { msg } => write!(f, "{}", msg),
            QueryErrorKind::Other(error) => error.fmt(f),
        }
    }
}

impl error::Error for EvaluationError {
    #[inline]
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Parsing(e) => Some(e),
            Self::Storage(e) => Some(e),
            Self::GraphParsing(e) => Some(e),
            Self::ResultsParsing(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::Query(e) => Some(e),
        }
    }
}

impl error::Error for QueryError {
    #[inline]
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self.inner {
            QueryErrorKind::Msg { .. } => None,
            QueryErrorKind::Other(e) => Some(e.as_ref()),
        }
    }
}

impl EvaluationError {
    /// Wraps another error.
    #[inline]
    pub(crate) fn wrap(error: impl error::Error + Send + Sync + 'static) -> Self {
        Self::Query(QueryError {
            inner: QueryErrorKind::Other(Box::new(error)),
        })
    }

    /// Builds an error from a printable error message.
    #[inline]
    pub(crate) fn msg(msg: impl Into<String>) -> Self {
        Self::Query(QueryError {
            inner: QueryErrorKind::Msg { msg: msg.into() },
        })
    }
}

impl From<Infallible> for EvaluationError {
    #[inline]
    fn from(error: Infallible) -> Self {
        match error {}
    }
}

impl From<spargebra::ParseError> for EvaluationError {
    #[inline]
    fn from(error: spargebra::ParseError) -> Self {
        Self::Parsing(error)
    }
}

impl From<StorageError> for EvaluationError {
    #[inline]
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

impl From<io::Error> for EvaluationError {
    #[inline]
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ParseError> for EvaluationError {
    #[inline]
    fn from(error: ParseError) -> Self {
        Self::GraphParsing(error)
    }
}

impl From<sparesults::ParseError> for EvaluationError {
    #[inline]
    fn from(error: sparesults::ParseError) -> Self {
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
            EvaluationError::Io(error) => error,
            EvaluationError::Storage(error) => error.into(),
            EvaluationError::Query(error) => Self::new(io::ErrorKind::Other, error),
        }
    }
}
