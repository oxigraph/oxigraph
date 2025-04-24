pub use json_event_parser::TextPosition;
use json_event_parser::{JsonParseError, JsonSyntaxError};
use std::fmt::Formatter;
use std::ops::Range;
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
    /// The [JSON-LD error code](https://www.w3.org/TR/json-ld-api/#dom-jsonlderrorcode) related to this error.
    pub fn code(&self) -> Option<JsonLdErrorCode> {
        match &self.0 {
            SyntaxErrorKind::Json(_) => None,
            SyntaxErrorKind::Msg { code, .. } => *code,
        }
    }

    /// The location of the error inside of the file.
    pub fn location(&self) -> Option<Range<TextPosition>> {
        match &self.0 {
            SyntaxErrorKind::Json(e) => Some(e.location()),
            SyntaxErrorKind::Msg { .. } => None,
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

/// A [JSON-LD error code](https://www.w3.org/TR/json-ld-api/#dom-jsonlderrorcode)
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum JsonLdErrorCode {
    /// Two properties which expand to the same keyword have been detected.
    /// This might occur if a keyword and an alias thereof are used at the same time.
    CollidingKeywords,
    /// Multiple conflicting indexes have been found for the same node.
    ConflictingIndexes,
    /// Maximum number of @context URLs exceeded.
    ContextOverflow,
    /// A cycle in IRI mappings has been detected.
    CyclicIriMapping,
    /// An @id entry was encountered whose value was not a string.
    InvalidIdValue,
    /// An invalid value for @import has been found.
    InvalidImportValue,
    /// An included block contains an invalid value.
    InvalidIncludedValue,
    /// An @index entry was encountered whose value was not a string.
    InvalidIndexValue,
    /// An invalid value for @nest has been found.
    InvalidNestValue,
    /// An invalid value for @prefix has been found.
    InvalidPrefixValue,
    /// An invalid value for @propagate has been found.
    InvalidPropagateValue,
    /// An invalid value for @protected has been found.
    InvalidProtectedValue,
    /// An invalid value for an @reverse entry has been detected, i.e., the value was not a map.
    InvalidReverseValue,
    /// The @version entry was used in a context with an out of range value.
    InvalidVersionValue,
    /// The value of @direction is not "ltr", "rtl", or null and thus invalid.
    InvalidBaseDirection,
    /// An invalid base IRI has been detected, i.e., it is neither an IRI nor null.
    InvalidBaseIri,
    /// An @container entry was encountered whose value was not one of the following strings:
    /// @list, @set, @language, @index, @id, @graph, or @type.
    InvalidContainerMapping,
    /// An entry in a context is invalid due to processing mode incompatibility.
    InvalidContextEntry,
    /// An attempt was made to nullify a context containing protected term definitions.
    InvalidContextNullification,
    /// The value of the default language is not a string or null and thus invalid.
    InvalidDefaultLanguage,
    /// A local context contains a term that has an invalid or missing IRI mapping.
    InvalidIriMapping,
    /// An invalid JSON literal was detected.
    InvalidJsonLiteral,
    /// An invalid keyword alias definition has been encountered.
    InvalidKeywordAlias,
    /// An invalid value in a language map has been detected.
    /// It MUST be a string or an array of strings.
    InvalidLanguageMapValue,
    /// An @language entry in a term definition was encountered
    /// whose value was neither a string nor null and thus invalid.
    InvalidLanguageMapping,
    /// A language-tagged string with an invalid language value was detected.
    InvalidLanguageTaggedString,
    /// A number, true, or false with an associated language tag was detected.
    InvalidLanguageTaggedValue,
    /// An invalid local context was detected.
    InvalidLocalContext,
    /// No valid context document has been found for a referenced remote context.
    InvalidRemoteContext,
    /// An invalid reverse property definition has been detected.
    InvalidReverseProperty,
    /// An invalid reverse property map has been detected.
    /// No keywords apart from @context are allowed in reverse property maps.
    InvalidReversePropertyMap,
    /// An invalid value for a reverse property has been detected.
    /// The value of an inverse property must be a node object.
    InvalidReversePropertyValue,
    /// The local context defined within a term definition is invalid.
    InvalidScopedContext,
    /// A set object or list object with disallowed entries has been detected.
    InvalidSetOrListObject,
    /// The key ordering is not compatible with the streaming profile.
    InvalidStreamingKeyOrder,
    /// An invalid term definition has been detected.
    InvalidTermDefinition,
    /// An @type entry in a term definition was encountered whose value could not be expanded to an IRI.
    InvalidTypeMapping,
    /// An invalid value for an @type entry has been detected,
    /// i.e., the value was neither a string nor an array of strings.
    InvalidTypeValue,
    /// A typed value with an invalid type was detected.
    InvalidTypedValue,
    /// A value object with disallowed entries has been detected.
    InvalidValueObject,
    /// An invalid value for the @value entry of a value object has been detected,
    /// i.e., it is neither a scalar nor null.
    InvalidValueObjectValue,
    /// An invalid vocabulary mapping has been detected, i.e., it is neither an IRI nor null.
    InvalidVocabMapping,
    /// When compacting an IRI would result in an IRI which could be confused with a compact IRI
    /// (because its IRI scheme matches a term definition and it has no IRI authority).
    IriConfusedWithPrefix,
    /// A keyword redefinition has been detected.
    KeywordRedefinition,
    /// The document could not be loaded or parsed as JSON.
    LoadingDocumentFailed,
    /// There was a problem encountered loading a remote context.
    LoadingRemoteContextFailed,
    /// An attempt was made to change the processing mode which is incompatible with the previous specified version.
    ProcessingModeConflict,
    /// An attempt was made to redefine a protected term.
    ProtectedTermRedefinition,
}

impl fmt::Display for JsonLdErrorCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::CollidingKeywords => "colliding keywords",
            Self::ConflictingIndexes => "conflicting indexes",
            Self::ContextOverflow => "context overflow",
            Self::CyclicIriMapping => "cyclic IRI mapping",
            Self::InvalidIdValue => "invalid @id value",
            Self::InvalidImportValue => "invalid @import value",
            Self::InvalidIncludedValue => "invalid @included value",
            Self::InvalidIndexValue => "invalid @index value",
            Self::InvalidNestValue => "invalid @nest value",
            Self::InvalidPrefixValue => "invalid @prefix value",
            Self::InvalidPropagateValue => "invalid @propagate value",
            Self::InvalidProtectedValue => "invalid @protected value",
            Self::InvalidReverseValue => "invalid @reverse value",
            Self::InvalidVersionValue => "invalid @version value",
            Self::InvalidBaseDirection => "invalid base direction",
            Self::InvalidBaseIri => "invalid base IRI",
            Self::InvalidContainerMapping => "invalid container mapping",
            Self::InvalidContextEntry => "invalid context entry",
            Self::InvalidContextNullification => "invalid context nullification",
            Self::InvalidDefaultLanguage => "invalid default language",
            Self::InvalidIriMapping => "invalid IRI mapping",
            Self::InvalidJsonLiteral => "invalid JSON literal",
            Self::InvalidKeywordAlias => "invalid keyword alias",
            Self::InvalidLanguageMapValue => "invalid language map value",
            Self::InvalidLanguageMapping => "invalid language mapping",
            Self::InvalidLanguageTaggedString => "invalid language-tagged string",
            Self::InvalidLanguageTaggedValue => "invalid language-tagged value",
            Self::InvalidLocalContext => "invalid local context",
            Self::InvalidRemoteContext => "invalid remote context",
            Self::InvalidReverseProperty => "invalid reverse property",
            Self::InvalidReversePropertyMap => "invalid reverse property map",
            Self::InvalidReversePropertyValue => "invalid reverse property value",
            Self::InvalidScopedContext => "invalid scoped context",
            Self::InvalidSetOrListObject => "invalid set or list object",
            Self::InvalidStreamingKeyOrder => "invalid streaming key order",
            Self::InvalidTermDefinition => "invalid term definition",
            Self::InvalidTypeMapping => "invalid type mapping",
            Self::InvalidTypeValue => "invalid type value",
            Self::InvalidTypedValue => "invalid typed value",
            Self::InvalidValueObject => "invalid value object",
            Self::InvalidValueObjectValue => "invalid value object value",
            Self::InvalidVocabMapping => "invalid vocab mapping",
            Self::IriConfusedWithPrefix => "IRI confused with prefix",
            Self::KeywordRedefinition => "keyword redefinition",
            Self::LoadingDocumentFailed => "loading document failed",
            Self::LoadingRemoteContextFailed => "loading remote context failed",
            Self::ProcessingModeConflict => "processing mode conflict",
            Self::ProtectedTermRedefinition => "protected term redefinition",
        })
    }
}
