use oxrdf::TermParseError;
use std::io;
use std::ops::Range;
use std::sync::Arc;

/// Error returned during SPARQL result formats format parsing.
#[derive(Debug, thiserror::Error)]
pub enum QueryResultsParseError {
    /// I/O error during parsing (file not found...).
    #[error(transparent)]
    Io(#[from] io::Error),
    /// An error in the file syntax.
    #[error(transparent)]
    Syntax(#[from] QueryResultsSyntaxError),
}

impl From<QueryResultsParseError> for io::Error {
    #[inline]
    fn from(error: QueryResultsParseError) -> Self {
        match error {
            QueryResultsParseError::Io(error) => error,
            QueryResultsParseError::Syntax(error) => error.into(),
        }
    }
}

impl From<json_event_parser::ParseError> for QueryResultsParseError {
    fn from(error: json_event_parser::ParseError) -> Self {
        match error {
            json_event_parser::ParseError::Syntax(error) => {
                QueryResultsSyntaxError::from(error).into()
            }
            json_event_parser::ParseError::Io(error) => error.into(),
        }
    }
}

impl From<quick_xml::Error> for QueryResultsParseError {
    #[inline]
    fn from(error: quick_xml::Error) -> Self {
        match error {
            quick_xml::Error::Io(error) => {
                Self::Io(Arc::try_unwrap(error).unwrap_or_else(|e| io::Error::new(e.kind(), e)))
            }
            _ => Self::Syntax(QueryResultsSyntaxError(SyntaxErrorKind::Xml(error))),
        }
    }
}

impl From<quick_xml::escape::EscapeError> for QueryResultsParseError {
    #[inline]
    fn from(error: quick_xml::escape::EscapeError) -> Self {
        quick_xml::Error::from(error).into()
    }
}
/// An error in the syntax of the parsed file.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct QueryResultsSyntaxError(#[from] pub(crate) SyntaxErrorKind);

#[derive(Debug, thiserror::Error)]
pub(crate) enum SyntaxErrorKind {
    #[error(transparent)]
    Json(#[from] json_event_parser::SyntaxError),
    #[error(transparent)]
    Xml(#[from] quick_xml::Error),
    #[error("Error {error} on '{term}' in line {}", location.start.line + 1)]
    Term {
        #[source]
        error: TermParseError,
        term: String,
        location: Range<TextPosition>,
    },
    #[error("{msg}")]
    Msg {
        msg: String,
        location: Option<Range<TextPosition>>,
    },
}

impl QueryResultsSyntaxError {
    /// Builds an error from a printable error message.
    #[inline]
    pub(crate) fn msg(msg: impl Into<String>) -> Self {
        Self(SyntaxErrorKind::Msg {
            msg: msg.into(),
            location: None,
        })
    }

    /// Builds an error from a printable error message and a location
    #[inline]
    pub(crate) fn located_message(msg: impl Into<String>, location: Range<TextPosition>) -> Self {
        Self(SyntaxErrorKind::Msg {
            msg: msg.into(),
            location: Some(location),
        })
    }

    /// The location of the error inside of the file.
    #[inline]
    pub fn location(&self) -> Option<Range<TextPosition>> {
        match &self.0 {
            SyntaxErrorKind::Json(e) => {
                let location = e.location();
                Some(
                    TextPosition {
                        line: location.start.line,
                        column: location.start.column,
                        offset: location.start.offset,
                    }..TextPosition {
                        line: location.end.line,
                        column: location.end.column,
                        offset: location.end.offset,
                    },
                )
            }
            SyntaxErrorKind::Term { location, .. } => Some(location.clone()),
            SyntaxErrorKind::Msg { location, .. } => location.clone(),
            SyntaxErrorKind::Xml(_) => None,
        }
    }
}

impl From<QueryResultsSyntaxError> for io::Error {
    #[inline]
    fn from(error: QueryResultsSyntaxError) -> Self {
        match error.0 {
            SyntaxErrorKind::Json(error) => Self::new(io::ErrorKind::InvalidData, error),
            SyntaxErrorKind::Xml(error) => match error {
                quick_xml::Error::Io(error) => {
                    Arc::try_unwrap(error).unwrap_or_else(|e| Self::new(e.kind(), e))
                }
                quick_xml::Error::UnexpectedEof(error) => {
                    Self::new(io::ErrorKind::UnexpectedEof, error)
                }
                _ => Self::new(io::ErrorKind::InvalidData, error),
            },
            SyntaxErrorKind::Term { .. } => Self::new(io::ErrorKind::InvalidData, error),
            SyntaxErrorKind::Msg { msg, .. } => Self::new(io::ErrorKind::InvalidData, msg),
        }
    }
}

impl From<json_event_parser::SyntaxError> for QueryResultsSyntaxError {
    fn from(error: json_event_parser::SyntaxError) -> Self {
        Self(SyntaxErrorKind::Json(error))
    }
}

/// A position in a text i.e. a `line` number starting from 0, a `column` number starting from 0 (in number of code points) and a global file `offset` starting from 0 (in number of bytes).
#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub struct TextPosition {
    pub line: u64,
    pub column: u64,
    pub offset: u64,
}
