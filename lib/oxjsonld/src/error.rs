use json_event_parser::{JsonParseError, JsonSyntaxError};
use oxilangtag::LanguageTagParseError;
use oxiri::IriParseError;
use std::io;

/// Error returned during JSON-LD parsing.
#[derive(Debug, thiserror::Error)]
pub enum JsonLdParseError {
    /// I/O error during parsing (file not found...).
    #[error(transparent)]
    Io(#[from] io::Error),
    /// An error in the file syntax.
    #[error(transparent)]
    Syntax(#[from] JsonLdSyntaxError),
}

impl From<JsonLdParseError> for io::Error {
    #[inline]
    fn from(error: JsonLdParseError) -> Self {
        match error {
            JsonLdParseError::Io(error) => error,
            JsonLdParseError::Syntax(error) => error.into(),
        }
    }
}

#[doc(hidden)]
impl From<JsonParseError> for JsonLdParseError {
    #[inline]
    fn from(error: JsonParseError) -> Self {
        match error {
            JsonParseError::Io(error) => Self::Io(error),
            JsonParseError::Syntax(error) => Self::Syntax(error.into()),
        }
    }
}

/// An error in the syntax of the parsed file.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct JsonLdSyntaxError(#[from] SyntaxErrorKind);

#[derive(Debug, thiserror::Error)]
enum SyntaxErrorKind {
    #[error(transparent)]
    Json(#[from] JsonSyntaxError),
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

impl JsonLdSyntaxError {
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

impl From<JsonLdSyntaxError> for io::Error {
    #[inline]
    fn from(error: JsonLdSyntaxError) -> Self {
        match error.0 {
            SyntaxErrorKind::Json(error) => Self::new(io::ErrorKind::InvalidData, error),
            SyntaxErrorKind::Msg(msg) => Self::new(io::ErrorKind::InvalidData, msg),
            _ => Self::new(io::ErrorKind::InvalidData, error),
        }
    }
}

#[doc(hidden)]
impl From<JsonSyntaxError> for JsonLdSyntaxError {
    #[inline]
    fn from(error: JsonSyntaxError) -> Self {
        Self(SyntaxErrorKind::Json(error))
    }
}
