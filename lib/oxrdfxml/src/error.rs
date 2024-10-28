use oxilangtag::LanguageTagParseError;
use oxiri::IriParseError;
use quick_xml::encoding::EncodingError;
use quick_xml::events::attributes::AttrError;
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

#[doc(hidden)]
impl From<EncodingError> for RdfXmlParseError {
    fn from(error: EncodingError) -> Self {
        quick_xml::Error::from(error).into()
    }
}

#[doc(hidden)]
impl From<AttrError> for RdfXmlParseError {
    fn from(error: AttrError) -> Self {
        quick_xml::Error::from(error).into()
    }
}

/// An error in the syntax of the parsed file.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct RdfXmlSyntaxError(#[from] SyntaxErrorKind);

#[derive(Debug, thiserror::Error)]
enum SyntaxErrorKind {
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
    pub(crate) fn msg(msg: impl Into<String>) -> Self {
        Self(SyntaxErrorKind::Msg(msg.into()))
    }

    pub(crate) fn invalid_iri(iri: String, error: IriParseError) -> Self {
        Self(SyntaxErrorKind::InvalidIri { iri, error })
    }

    pub(crate) fn invalid_language_tag(tag: String, error: LanguageTagParseError) -> Self {
        Self(SyntaxErrorKind::InvalidLanguageTag { tag, error })
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
