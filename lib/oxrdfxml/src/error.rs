use oxilangtag::LanguageTagParseError;
use oxiri::IriParseError;
use std::io;
use std::sync::Arc;

/// Error returned during RDF/XML parsing.
#[derive(Debug, thiserror::Error)]
pub enum RdfXmlParseError {
    /// I/O error during parsing (file not found...).
    #[error(transparent)]
    Io(#[from] io::Error),
    /// An error in the file syntax.
    #[error(transparent)]
    Syntax(#[from] RdfXmlSyntaxError),
}

impl From<RdfXmlParseError> for io::Error {
    #[inline]
    fn from(error: RdfXmlParseError) -> Self {
        match error {
            RdfXmlParseError::Io(error) => error,
            RdfXmlParseError::Syntax(error) => error.into(),
        }
    }
}

#[doc(hidden)]
impl From<quick_xml::Error> for RdfXmlParseError {
    #[inline]
    fn from(error: quick_xml::Error) -> Self {
        match error {
            quick_xml::Error::Io(error) => {
                Self::Io(Arc::try_unwrap(error).unwrap_or_else(|e| io::Error::new(e.kind(), e)))
            }
            _ => Self::Syntax(RdfXmlSyntaxError(SyntaxErrorKind::Xml(error))),
        }
    }
}

/// An error in the syntax of the parsed file.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct RdfXmlSyntaxError(#[from] pub(crate) SyntaxErrorKind);

#[derive(Debug, thiserror::Error)]
pub enum SyntaxErrorKind {
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
    #[error("{0}")]
    Msg(String),
}

impl RdfXmlSyntaxError {
    /// Builds an error from a printable error message.
    #[inline]
    pub(crate) fn msg(msg: impl Into<String>) -> Self {
        Self(SyntaxErrorKind::Msg(msg.into()))
    }
}

impl From<RdfXmlSyntaxError> for io::Error {
    #[inline]
    fn from(error: RdfXmlSyntaxError) -> Self {
        match error.0 {
            SyntaxErrorKind::Xml(error) => match error {
                quick_xml::Error::Io(error) => {
                    Arc::try_unwrap(error).unwrap_or_else(|e| Self::new(e.kind(), e))
                }
                _ => Self::new(io::ErrorKind::InvalidData, error),
            },
            SyntaxErrorKind::Msg(msg) => Self::new(io::ErrorKind::InvalidData, msg),
            _ => Self::new(io::ErrorKind::InvalidData, error),
        }
    }
}
