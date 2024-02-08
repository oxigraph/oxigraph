use oxilangtag::LanguageTagParseError;
use oxiri::IriParseError;
use std::error::Error;
use std::sync::Arc;
use std::{fmt, io};
use thiserror::Error;

/// Error returned during RDF/XML parsing.
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

impl From<quick_xml::Error> for ParseError {
    #[inline]
    fn from(error: quick_xml::Error) -> Self {
        match error {
            quick_xml::Error::Io(error) => {
                Self::Io(Arc::try_unwrap(error).unwrap_or_else(|e| io::Error::new(e.kind(), e)))
            }
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
pub enum SyntaxErrorKind {
    Xml(quick_xml::Error),
    InvalidIri {
        iri: String,
        error: IriParseError,
    },
    InvalidLanguageTag {
        tag: String,
        error: LanguageTagParseError,
    },
    Msg {
        msg: String,
    },
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
            SyntaxErrorKind::Xml(error) => error.fmt(f),
            SyntaxErrorKind::InvalidIri { iri, error } => {
                write!(f, "error while parsing IRI '{iri}': {error}")
            }
            SyntaxErrorKind::InvalidLanguageTag { tag, error } => {
                write!(f, "error while parsing language tag '{tag}': {error}")
            }
            SyntaxErrorKind::Msg { msg } => f.write_str(msg),
        }
    }
}

impl Error for SyntaxError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.inner {
            SyntaxErrorKind::Xml(error) => Some(error),
            SyntaxErrorKind::InvalidIri { error, .. } => Some(error),
            SyntaxErrorKind::InvalidLanguageTag { error, .. } => Some(error),
            SyntaxErrorKind::Msg { .. } => None,
        }
    }
}

impl From<SyntaxError> for io::Error {
    #[inline]
    fn from(error: SyntaxError) -> Self {
        match error.inner {
            SyntaxErrorKind::Xml(error) => match error {
                quick_xml::Error::Io(error) => {
                    Arc::try_unwrap(error).unwrap_or_else(|e| Self::new(e.kind(), e))
                }
                quick_xml::Error::UnexpectedEof(error) => {
                    Self::new(io::ErrorKind::UnexpectedEof, error)
                }
                _ => Self::new(io::ErrorKind::InvalidData, error),
            },
            SyntaxErrorKind::Msg { msg } => Self::new(io::ErrorKind::InvalidData, msg),
            _ => Self::new(io::ErrorKind::InvalidData, error),
        }
    }
}
