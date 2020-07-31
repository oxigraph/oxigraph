use crate::error::Infallible;
use crate::sparql::ParseError;
use crate::store::numeric_encoder::DecoderError;
use std::error;
use std::fmt;
use std::io;

/// SPARQL evaluation error.
///
/// The `wrap` method allows us to make this type wrap any implementation of `std::error::Error`.
/// This type also avoids heap allocations for the most common cases of evaluation errors.
#[derive(Debug)]
#[non_exhaustive]
pub enum EvaluationError {
    /// An error in SPARQL query parsing
    Parsing(ParseError),
    /// An error returned during store IOs or during results write
    Io(io::Error),
    /// An error returned during the query evaluation itself
    Query(QueryError),
}

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
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parsing(error) => error.fmt(f),
            Self::Io(error) => error.fmt(f),
            Self::Query(error) => error.fmt(f),
        }
    }
}

impl fmt::Display for QueryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            QueryErrorKind::Msg { msg } => write!(f, "{}", msg),
            QueryErrorKind::Other(error) => error.fmt(f),
        }
    }
}

impl error::Error for EvaluationError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self {
            Self::Parsing(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::Query(e) => Some(e),
        }
    }
}

impl error::Error for QueryError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self.inner {
            QueryErrorKind::Msg { .. } => None,
            QueryErrorKind::Other(e) => Some(e.as_ref()),
        }
    }
}

impl EvaluationError {
    /// Wraps another error.
    pub(crate) fn wrap(error: impl error::Error + Send + Sync + 'static) -> Self {
        Self::Query(QueryError {
            inner: QueryErrorKind::Other(Box::new(error)),
        })
    }

    /// Builds an error from a printable error message.
    pub(crate) fn msg(msg: impl Into<String>) -> Self {
        Self::Query(QueryError {
            inner: QueryErrorKind::Msg { msg: msg.into() },
        })
    }
}

impl From<Infallible> for EvaluationError {
    fn from(error: Infallible) -> Self {
        match error {}
    }
}

impl From<std::convert::Infallible> for EvaluationError {
    fn from(error: std::convert::Infallible) -> Self {
        match error {}
    }
}

impl From<ParseError> for EvaluationError {
    fn from(error: ParseError) -> Self {
        Self::Parsing(error)
    }
}

impl From<io::Error> for EvaluationError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl<E: Into<io::Error>> From<DecoderError<E>> for EvaluationError {
    fn from(error: DecoderError<E>) -> Self {
        io::Error::from(error).into()
    }
}
