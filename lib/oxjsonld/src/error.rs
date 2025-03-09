use json_event_parser::{JsonParseError, JsonSyntaxError};
use std::fmt::Formatter;
use std::{fmt, io};

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
    #[error("{msg}")]
    Msg {
        msg: String,
        code: Option<JsonLdErrorCode>,
    },
}

impl JsonLdSyntaxError {
    ///     A string representing the particular error type, as described in the various algorithms in this document.
    pub fn code(&self) -> Option<JsonLdErrorCode> {
        match &self.0 {
            SyntaxErrorKind::Json(_) => None,
            SyntaxErrorKind::Msg { code, .. } => *code,
        }
    }

    /// Builds an error from a printable error message.
    pub(crate) fn msg(msg: impl Into<String>) -> Self {
        Self(SyntaxErrorKind::Msg {
            msg: msg.into(),
            code: None,
        })
    }

    /// Builds an error from a printable error message and an error code.
    pub(crate) fn msg_and_code(msg: impl Into<String>, code: JsonLdErrorCode) -> Self {
        Self(SyntaxErrorKind::Msg {
            msg: msg.into(),
            code: Some(code),
        })
    }
}

impl From<JsonLdSyntaxError> for io::Error {
    #[inline]
    fn from(error: JsonLdSyntaxError) -> Self {
        match error.0 {
            SyntaxErrorKind::Json(error) => error.into(),
            SyntaxErrorKind::Msg { msg, .. } => Self::new(io::ErrorKind::InvalidData, msg),
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

#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum JsonLdErrorCode {
    /// Two properties which expand to the same keyword have been detected.
    /// This might occur if a keyword and an alias thereof are used at the same time.
    CollidingKeywords,
    /// An @id entry was encountered whose value was not a string.
    InvalidIdValue,
    /// An invalid base IRI has been detected, i.e., it is neither an IRI nor null.
    InvalidBaseIri,
    /// An entry in a context is invalid due to processing mode incompatibility.
    InvalidContextEntry,
    /// A local context contains a term that has an invalid or missing IRI mapping.
    InvalidIriMapping,
    /// A language-tagged string with an invalid language value was detected.
    InvalidLanguageTaggedString,
    /// A number, true, or false with an associated language tag was detected.
    InvalidLanguageTaggedValue,
    /// An invalid value for an @type entry has been detected, i.e., the value was neither a string nor an array of strings.
    InvalidTypeValue,
    /// A typed value with an invalid type was detected.
    InvalidTypedValue,
    /// A value object with disallowed entries has been detected.
    InvalidValueObject,
    /// An invalid value for the @value entry of a value object has been detected, i.e., it is neither a scalar nor null.
    InvalidValueObjectValue,
}

impl fmt::Display for JsonLdErrorCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CollidingKeywords => "colliding keywords",
            Self::InvalidIdValue => "invalid @id value",
            Self::InvalidBaseIri => "invalid base IRI",
            Self::InvalidContextEntry => "invalid context entry",
            Self::InvalidIriMapping => "invalid IRI mapping",
            Self::InvalidLanguageTaggedString => "invalid language-tagged string",
            Self::InvalidLanguageTaggedValue => "invalid language-tagged value",
            Self::InvalidTypeValue => "invalid type value",
            Self::InvalidTypedValue => "invalid typed value",
            Self::InvalidValueObject => "invalid value object",
            Self::InvalidValueObjectValue => "invalid value object value",
        })
    }
}
