use std::io;
use std::ops::Range;

/// Error returned during RDF format parsing.
#[derive(Debug, thiserror::Error)]
pub enum RdfParseError {
    /// I/O error during parsing (file not found...).
    #[error(transparent)]
    Io(#[from] io::Error),
    /// An error in the file syntax.
    #[error(transparent)]
    Syntax(#[from] RdfSyntaxError),
}

impl From<oxjsonld::JsonLdParseError> for RdfParseError {
    #[inline]
    fn from(error: oxjsonld::JsonLdParseError) -> Self {
        match error {
            oxjsonld::JsonLdParseError::Syntax(e) => Self::Syntax(e.into()),
            oxjsonld::JsonLdParseError::Io(e) => Self::Io(e),
        }
    }
}

impl From<oxttl::TurtleParseError> for RdfParseError {
    #[inline]
    fn from(error: oxttl::TurtleParseError) -> Self {
        match error {
            oxttl::TurtleParseError::Syntax(e) => Self::Syntax(e.into()),
            oxttl::TurtleParseError::Io(e) => Self::Io(e),
        }
    }
}

impl From<oxrdfxml::RdfXmlParseError> for RdfParseError {
    #[inline]
    fn from(error: oxrdfxml::RdfXmlParseError) -> Self {
        match error {
            oxrdfxml::RdfXmlParseError::Syntax(e) => Self::Syntax(e.into()),
            oxrdfxml::RdfXmlParseError::Io(e) => Self::Io(e),
        }
    }
}

impl From<RdfParseError> for io::Error {
    #[inline]
    fn from(error: RdfParseError) -> Self {
        match error {
            RdfParseError::Io(error) => error,
            RdfParseError::Syntax(error) => error.into(),
        }
    }
}

/// An error in the syntax of the parsed file.
#[derive(Debug, thiserror::Error)]
pub struct RdfSyntaxError(SyntaxErrorKind);

/// An error in the syntax of the parsed file.
#[derive(Debug, thiserror::Error)]
enum SyntaxErrorKind {
    #[error(transparent)]
    JsonLd(#[from] oxjsonld::JsonLdSyntaxError),
    #[error(transparent)]
    Turtle(#[from] oxttl::TurtleSyntaxError),
    #[error(transparent)]
    RdfXml(#[from] oxrdfxml::RdfXmlSyntaxError),
    #[error("{0}")]
    Msg(&'static str),
}

impl RdfSyntaxError {
    /// The location of the error inside of the file.
    #[inline]
    pub fn location(&self) -> Option<Range<TextPosition>> {
        match &self.0 {
            SyntaxErrorKind::JsonLd(e) => {
                let location = e.location()?;
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
            SyntaxErrorKind::Turtle(e) => {
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
            SyntaxErrorKind::RdfXml(e) => {
                e.location().map(|location| {
                    TextPosition {
                        line: location.start.line,
                        column: location.start.column,
                        offset: location.start.offset,
                    }..TextPosition {
                        line: location.end.line,
                        column: location.end.column,
                        offset: location.end.offset,
                    }
                })
            }
            SyntaxErrorKind::Msg(_) => None,
        }
    }

    /// Get additional context about what was expected, if available.
    #[inline]
    pub fn expected(&self) -> Option<&str> {
        match &self.0 {
            SyntaxErrorKind::Turtle(e) => e.expected(),
            _ => None,
        }
    }

    /// Get additional context about what was found, if available.
    #[inline]
    pub fn found(&self) -> Option<&str> {
        match &self.0 {
            SyntaxErrorKind::Turtle(e) => e.found(),
            _ => None,
        }
    }

    /// Get a suggestion for fixing the error, if available.
    #[inline]
    pub fn suggestion(&self) -> Option<&str> {
        match &self.0 {
            SyntaxErrorKind::Turtle(e) => e.suggestion(),
            _ => None,
        }
    }

    pub(crate) fn msg(msg: &'static str) -> Self {
        Self(SyntaxErrorKind::Msg(msg))
    }
}

impl std::fmt::Display for RdfSyntaxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Delegate to the underlying error's Display
        write!(f, "{}", self.0)
    }
}

impl From<oxjsonld::JsonLdSyntaxError> for RdfSyntaxError {
    #[inline]
    fn from(error: oxjsonld::JsonLdSyntaxError) -> Self {
        Self(SyntaxErrorKind::JsonLd(error))
    }
}

impl From<oxttl::TurtleSyntaxError> for RdfSyntaxError {
    #[inline]
    fn from(error: oxttl::TurtleSyntaxError) -> Self {
        Self(SyntaxErrorKind::Turtle(error))
    }
}

impl From<oxrdfxml::RdfXmlSyntaxError> for RdfSyntaxError {
    #[inline]
    fn from(error: oxrdfxml::RdfXmlSyntaxError) -> Self {
        Self(SyntaxErrorKind::RdfXml(error))
    }
}

impl From<RdfSyntaxError> for io::Error {
    #[inline]
    fn from(error: RdfSyntaxError) -> Self {
        match error.0 {
            SyntaxErrorKind::JsonLd(error) => error.into(),
            SyntaxErrorKind::Turtle(error) => error.into(),
            SyntaxErrorKind::RdfXml(error) => error.into(),
            SyntaxErrorKind::Msg(msg) => Self::new(io::ErrorKind::InvalidData, msg),
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
