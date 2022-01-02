use oxrdf::TermParseError;
use std::error::Error;
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

impl From<quick_xml::Error> for ParseError {
    #[inline]
    fn from(error: quick_xml::Error) -> Self {
        match error {
            quick_xml::Error::Io(error) => Self::Io(error),
            error => Self::Syntax(SyntaxError {
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
    Xml(quick_xml::Error),
    Term(TermParseError),
    Msg { msg: String },
}

impl SyntaxError {
    /// Builds an error from a printable error message.
    #[inline]
    pub(crate) fn msg(msg: impl Into<String>) -> Self {
        Self {
            inner: SyntaxErrorKind::Msg { msg: msg.into() },
        }
    }
}

impl fmt::Display for SyntaxError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            SyntaxErrorKind::Xml(e) => e.fmt(f),
            SyntaxErrorKind::Term(e) => e.fmt(f),
            SyntaxErrorKind::Msg { msg } => f.write_str(msg),
        }
    }
}

impl Error for SyntaxError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.inner {
            SyntaxErrorKind::Xml(e) => Some(e),
            SyntaxErrorKind::Term(e) => Some(e),
            SyntaxErrorKind::Msg { .. } => None,
        }
    }
}

impl From<SyntaxError> for io::Error {
    #[inline]
    fn from(error: SyntaxError) -> Self {
        match error.inner {
            SyntaxErrorKind::Xml(error) => match error {
                quick_xml::Error::Io(error) => error,
                quick_xml::Error::UnexpectedEof(error) => {
                    Self::new(io::ErrorKind::UnexpectedEof, error)
                }
                error => Self::new(io::ErrorKind::InvalidData, error),
            },
            SyntaxErrorKind::Term(error) => Self::new(io::ErrorKind::InvalidData, error),
            SyntaxErrorKind::Msg { msg } => Self::new(io::ErrorKind::InvalidData, msg),
        }
    }
}
