use oxrdf::TermParseError;
use std::io;
use std::ops::Range;
use std::sync::Arc;
use thiserror::Error;

/// Error returned during SPARQL result formats format parsing.
#[derive(Debug, Error)]
pub enum ParseError {
    /// I/O error during parsing (file not found...).
    #[error(transparent)]
    Io(#[from] io::Error),
    /// An error in the file syntax.
    #[error(transparent)]
    Syntax(#[from] SyntaxError),
}

impl From<ParseError> for io::Error {
    #[inline]
    fn from(error: ParseError) -> Self {
        match error {
            ParseError::Io(error) => error,
            ParseError::Syntax(error) => error.into(),
        }
    }
}

impl From<json_event_parser::ParseError> for ParseError {
    fn from(error: json_event_parser::ParseError) -> Self {
        match error {
            json_event_parser::ParseError::Syntax(error) => SyntaxError::from(error).into(),
            json_event_parser::ParseError::Io(error) => error.into(),
        }
    }
}

impl From<quick_xml::Error> for ParseError {
    #[inline]
    fn from(error: quick_xml::Error) -> Self {
        match error {
            quick_xml::Error::Io(error) => {
                Self::Io(Arc::try_unwrap(error).unwrap_or_else(|e| io::Error::new(e.kind(), e)))
            }
            _ => Self::Syntax(SyntaxError::Xml(error)),
        }
    }
}

/// An error in the syntax of the parsed file.
#[derive(Debug, Error)]
pub enum SyntaxError {
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

impl SyntaxError {
    /// Builds an error from a printable error message.
    #[inline]
    pub(crate) fn msg(msg: impl Into<String>) -> Self {
        Self::Msg {
            msg: msg.into(),
            location: None,
        }
    }

    /// Builds an error from a printable error message and a location
    #[inline]
    pub(crate) fn located_message(msg: impl Into<String>, location: Range<TextPosition>) -> Self {
        Self::Msg {
            msg: msg.into(),
            location: Some(location),
        }
    }

    /// The location of the error inside of the file.
    #[inline]
    pub fn location(&self) -> Option<Range<TextPosition>> {
        match self {
            Self::Json(e) => {
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
            Self::Term { location, .. } => Some(location.clone()),
            Self::Msg { location, .. } => location.clone(),
            Self::Xml(_) => None,
        }
    }
}

impl From<SyntaxError> for io::Error {
    #[inline]
    fn from(error: SyntaxError) -> Self {
        match error {
            SyntaxError::Json(error) => Self::new(io::ErrorKind::InvalidData, error),
            SyntaxError::Xml(error) => match error {
                quick_xml::Error::Io(error) => {
                    Arc::try_unwrap(error).unwrap_or_else(|e| Self::new(e.kind(), e))
                }
                quick_xml::Error::UnexpectedEof(error) => {
                    Self::new(io::ErrorKind::UnexpectedEof, error)
                }
                _ => Self::new(io::ErrorKind::InvalidData, error),
            },
            SyntaxError::Term { .. } => Self::new(io::ErrorKind::InvalidData, error),
            SyntaxError::Msg { msg, .. } => Self::new(io::ErrorKind::InvalidData, msg),
        }
    }
}

/// A position in a text i.e. a `line` number starting from 0, a `column` number starting from 0 (in number of code points) and a global file `offset` starting from 0 (in number of bytes).
#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub struct TextPosition {
    pub line: u64,
    pub column: u64,
    pub offset: u64,
}
