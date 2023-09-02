use std::error::Error;
use std::ops::Range;
use std::{fmt, io};

/// Error returned during RDF format parsing.
#[derive(Debug)]
pub enum ParseError {
    /// I/O error during parsing (file not found...).
    Io(io::Error),
    /// An error in the file syntax.
    Syntax(SyntaxError),
}

impl ParseError {
    pub(crate) fn msg(msg: &'static str) -> Self {
        Self::Syntax(SyntaxError {
            inner: SyntaxErrorKind::Msg { msg },
        })
    }
}

impl fmt::Display for ParseError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::Syntax(e) => e.fmt(f),
        }
    }
}

impl Error for ParseError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Syntax(e) => Some(e),
        }
    }
}

impl From<oxttl::SyntaxError> for SyntaxError {
    #[inline]
    fn from(error: oxttl::SyntaxError) -> Self {
        SyntaxError {
            inner: SyntaxErrorKind::Turtle(error),
        }
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

impl From<oxrdfxml::SyntaxError> for SyntaxError {
    #[inline]
    fn from(error: oxrdfxml::SyntaxError) -> Self {
        SyntaxError {
            inner: SyntaxErrorKind::RdfXml(error),
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

impl From<io::Error> for ParseError {
    #[inline]
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<SyntaxError> for ParseError {
    #[inline]
    fn from(error: SyntaxError) -> Self {
        Self::Syntax(error)
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
#[derive(Debug)]
pub struct SyntaxError {
    inner: SyntaxErrorKind,
}

#[derive(Debug)]
enum SyntaxErrorKind {
    Turtle(oxttl::SyntaxError),
    RdfXml(oxrdfxml::SyntaxError),
    Msg { msg: &'static str },
}

impl SyntaxError {
    /// The location of the error inside of the file.
    #[inline]
    pub fn location(&self) -> Option<Range<TextPosition>> {
        match &self.inner {
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
            SyntaxErrorKind::RdfXml(_) | SyntaxErrorKind::Msg { .. } => None,
        }
    }
}

impl fmt::Display for SyntaxError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            SyntaxErrorKind::Turtle(e) => e.fmt(f),
            SyntaxErrorKind::RdfXml(e) => e.fmt(f),
            SyntaxErrorKind::Msg { msg } => write!(f, "{msg}"),
        }
    }
}

impl Error for SyntaxError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.inner {
            SyntaxErrorKind::Turtle(e) => Some(e),
            SyntaxErrorKind::RdfXml(e) => Some(e),
            SyntaxErrorKind::Msg { .. } => None,
        }
    }
}

impl From<SyntaxError> for io::Error {
    #[inline]
    fn from(error: SyntaxError) -> Self {
        match error.inner {
            SyntaxErrorKind::Turtle(error) => error.into(),
            SyntaxErrorKind::RdfXml(error) => error.into(),
            SyntaxErrorKind::Msg { msg } => io::Error::new(io::ErrorKind::InvalidData, msg),
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
