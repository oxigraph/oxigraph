use std::io;
use std::ops::Range;

/// Error returned during RDF format parsing.
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// I/O error during parsing (file not found...).
    #[error(transparent)]
    Io(#[from] io::Error),
    /// An error in the file syntax.
    #[error(transparent)]
    Syntax(#[from] SyntaxError),
}

impl ParseError {
    pub(crate) fn msg(msg: &'static str) -> Self {
        Self::Syntax(SyntaxError::Msg { msg })
    }
}

impl From<oxttl::ParseError> for ParseError {
    #[inline]
    fn from(error: oxttl::ParseError) -> Self {
        match error {
            oxttl::ParseError::Syntax(e) => Self::Syntax(e.into()),
            oxttl::ParseError::Io(e) => Self::Io(e),
        }
    }
}

impl From<oxrdfxml::ParseError> for ParseError {
    #[inline]
    fn from(error: oxrdfxml::ParseError) -> Self {
        match error {
            oxrdfxml::ParseError::Syntax(e) => Self::Syntax(e.into()),
            oxrdfxml::ParseError::Io(e) => Self::Io(e),
        }
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

/// An error in the syntax of the parsed file.
#[derive(Debug, thiserror::Error)]
pub enum SyntaxError {
    #[error(transparent)]
    Turtle(#[from] oxttl::SyntaxError),
    #[error(transparent)]
    RdfXml(#[from] oxrdfxml::SyntaxError),
    #[error("{msg}")]
    Msg { msg: &'static str },
}

impl SyntaxError {
    /// The location of the error inside of the file.
    #[inline]
    pub fn location(&self) -> Option<Range<TextPosition>> {
        match &self {
            Self::Turtle(e) => {
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
            Self::RdfXml(_) | Self::Msg { .. } => None,
        }
    }
}

impl From<SyntaxError> for io::Error {
    #[inline]
    fn from(error: SyntaxError) -> Self {
        match error {
            SyntaxError::Turtle(error) => error.into(),
            SyntaxError::RdfXml(error) => error.into(),
            SyntaxError::Msg { msg } => Self::new(io::ErrorKind::InvalidData, msg),
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
