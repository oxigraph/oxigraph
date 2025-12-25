use oxilangtag::LanguageTagParseError;
use oxiri::IriParseError;
use quick_xml::encoding::EncodingError;
use quick_xml::events::attributes::AttrError;
use std::io;
use std::ops::Range;
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
            _ => Self::Syntax(RdfXmlSyntaxError {
                inner: SyntaxErrorKind::Xml(error),
                position: None,
            }),
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

/// A position in a text i.e. a `line` number starting from 0, a `column` number starting from 0 (in number of code points) and a global file `offset` starting from 0 (in number of bytes).
#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub struct TextPosition {
    pub line: u64,
    pub column: u64,
    pub offset: u64,
}

/// An error in the syntax of the parsed file.
#[derive(Debug)]
pub struct RdfXmlSyntaxError {
    inner: SyntaxErrorKind,
    position: Option<Range<TextPosition>>,
}

impl std::fmt::Display for RdfXmlSyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(position) = &self.position {
            write!(
                f,
                "{} at line {}, column {}",
                self.inner, position.start.line, position.start.column
            )
        } else {
            write!(f, "{}", self.inner)
        }
    }
}

impl std::error::Error for RdfXmlSyntaxError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.inner.source()
    }
}

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
        Self {
            inner: SyntaxErrorKind::Msg(msg.into()),
            position: None,
        }
    }

    pub(crate) fn invalid_iri(iri: String, error: IriParseError) -> Self {
        Self {
            inner: SyntaxErrorKind::InvalidIri { iri, error },
            position: None,
        }
    }

    pub(crate) fn invalid_language_tag(tag: String, error: LanguageTagParseError) -> Self {
        Self {
            inner: SyntaxErrorKind::InvalidLanguageTag { tag, error },
            position: None,
        }
    }

    /// Sets the position of the error.
    pub(crate) fn with_position(mut self, position: Range<TextPosition>) -> Self {
        self.position = Some(position);
        self
    }

    /// Returns the location of the error inside of the file.
    pub fn location(&self) -> Option<Range<TextPosition>> {
        self.position.clone()
    }
}

impl From<RdfXmlSyntaxError> for io::Error {
    #[inline]
    fn from(error: RdfXmlSyntaxError) -> Self {
        match error.inner {
            SyntaxErrorKind::Xml(error) => match error {
                quick_xml::Error::Io(error) => {
                    Arc::try_unwrap(error).unwrap_or_else(|e| Self::new(e.kind(), e))
                }
                _ => Self::new(io::ErrorKind::InvalidData, error),
            },
            SyntaxErrorKind::Msg(msg) => Self::new(io::ErrorKind::InvalidData, msg),
            _ => Self::new(io::ErrorKind::InvalidData, format!("{}", error.inner)),
        }
    }
}
