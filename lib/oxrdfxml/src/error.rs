use oxilangtag::LanguageTagParseError;
use oxiri::IriParseError;
use std::io;
use std::sync::Arc;

/// Error returned during RDF/XML parsing.
#[derive(Debug, thiserror::Error)]
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
            _ => Self::Syntax(SyntaxError::Xml(error)),
        }
    }
}

/// An error in the syntax of the parsed file.
#[derive(Debug, thiserror::Error)]
pub enum SyntaxError {
    #[error(transparent)]
    Xml(#[from] quick_xml::Error),
    #[error("error while parsing IRI '{iri}': {error}")]
    InvalidIri {
        iri: String,
        #[source]
        error: IriParseError,
    },
    #[error("error while parsing language tag '{tag}': {error}")]
    InvalidLanguageTag {
        tag: String,
        #[source]
        error: LanguageTagParseError,
    },
    #[error("{msg}")]
    Msg { msg: String },
}

impl SyntaxError {
    /// Builds an error from a printable error message.
    #[inline]
    pub(crate) fn msg(msg: impl Into<String>) -> Self {
        Self::Msg { msg: msg.into() }
    }
}

impl From<SyntaxError> for io::Error {
    #[inline]
    fn from(error: SyntaxError) -> Self {
        match error {
            SyntaxError::Xml(error) => match error {
                quick_xml::Error::Io(error) => {
                    Arc::try_unwrap(error).unwrap_or_else(|e| Self::new(e.kind(), e))
                }
                quick_xml::Error::UnexpectedEof(error) => {
                    Self::new(io::ErrorKind::UnexpectedEof, error)
                }
                _ => Self::new(io::ErrorKind::InvalidData, error),
            },
            SyntaxError::Msg { msg } => Self::new(io::ErrorKind::InvalidData, msg),
            _ => Self::new(io::ErrorKind::InvalidData, error),
        }
    }
}
