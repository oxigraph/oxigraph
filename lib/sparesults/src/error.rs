use oxrdf::TermParseError;
use std::error::Error;
use std::ops::Range;
use std::sync::Arc;
use std::{fmt, io};

/// Error returned during SPARQL result formats format parsing.
#[derive(Debug)]
pub enum ParseError {
    /// I/O error during parsing (file not found...).
    Io(io::Error),
    /// An error in the file syntax.
    Syntax(SyntaxError),
}

impl fmt::Display for ParseError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::Syntax(e) => e.fmt(f),
        }
    }
}

impl Error for ParseError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Syntax(e) => Some(e),
        }
    }
}

impl From<io::Error> for ParseError {
    #[inline]
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<SyntaxError> for ParseError {
    #[inline]
    fn from(error: SyntaxError) -> Self {
        Self::Syntax(error)
    }
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
            quick_xml::Error::Io(error) => Self::Io(match Arc::try_unwrap(error) {
                Ok(error) => error,
                Err(error) => io::Error::new(error.kind(), error),
            }),
            _ => Self::Syntax(SyntaxError {
                inner: SyntaxErrorKind::Xml(error),
            }),
        }
    }
}

/// An error in the syntax of the parsed file.
#[derive(Debug)]
pub struct SyntaxError {
    pub(crate) inner: SyntaxErrorKind,
}

#[derive(Debug)]
pub(crate) enum SyntaxErrorKind {
    Json(json_event_parser::SyntaxError),
    Xml(quick_xml::Error),
    Term {
        error: TermParseError,
        term: String,
        location: Range<TextPosition>,
    },
    Msg {
        msg: String,
        location: Option<Range<TextPosition>>,
    },
}

impl SyntaxError {
    /// Builds an error from a printable error message.
    #[inline]
    pub(crate) fn msg(msg: impl Into<String>) -> Self {
        Self {
            inner: SyntaxErrorKind::Msg {
                msg: msg.into(),
                location: None,
            },
        }
    }

    /// Builds an error from a printable error message and a location
    #[inline]
    pub(crate) fn located_message(msg: impl Into<String>, location: Range<TextPosition>) -> Self {
        Self {
            inner: SyntaxErrorKind::Msg {
                msg: msg.into(),
                location: Some(location),
            },
        }
    }

    /// The location of the error inside of the file.
    #[inline]
    pub fn location(&self) -> Option<Range<TextPosition>> {
        match &self.inner {
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

impl fmt::Display for SyntaxError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            SyntaxErrorKind::Json(e) => e.fmt(f),
            SyntaxErrorKind::Xml(e) => e.fmt(f),
            SyntaxErrorKind::Term {
                error,
                term,
                location,
            } => write!(f, "{error} on '{term}' in line {}", location.start.line + 1),
            SyntaxErrorKind::Msg { msg, .. } => f.write_str(msg),
        }
    }
}

impl Error for SyntaxError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.inner {
            SyntaxErrorKind::Json(e) => Some(e),
            SyntaxErrorKind::Xml(e) => Some(e),
            SyntaxErrorKind::Term { error, .. } => Some(error),
            SyntaxErrorKind::Msg { .. } => None,
        }
    }
}

impl From<SyntaxError> for io::Error {
    #[inline]
    fn from(error: SyntaxError) -> Self {
        match error.inner {
            SyntaxErrorKind::Json(error) => Self::new(io::ErrorKind::InvalidData, error),
            SyntaxErrorKind::Xml(error) => match error {
                quick_xml::Error::Io(error) => match Arc::try_unwrap(error) {
                    Ok(error) => error,
                    Err(error) => Self::new(error.kind(), error),
                },
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

impl From<json_event_parser::SyntaxError> for SyntaxError {
    fn from(error: json_event_parser::SyntaxError) -> Self {
        Self {
            inner: SyntaxErrorKind::Json(error),
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
