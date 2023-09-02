use std::error::Error;
use std::ops::Range;
use std::{fmt, io};

/// A position in a text i.e. a `line` number starting from 0, a `column` number starting from 0 (in number of code points) and a global file `offset` starting from 0 (in number of bytes).
#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub struct TextPosition {
    pub line: u64,
    pub column: u64,
    pub offset: u64,
}

/// An error in the syntax of the parsed file.
///
/// It is composed of a message and a byte range in the input.
#[derive(Debug)]
pub struct SyntaxError {
    pub(super) location: Range<TextPosition>,
    pub(super) message: String,
}

impl SyntaxError {
    /// The location of the error inside of the file.
    #[inline]
    pub fn location(&self) -> Range<TextPosition> {
        self.location.clone()
    }

    /// The error message.
    #[inline]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for SyntaxError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.location.start.offset + 1 >= self.location.end.offset {
            write!(
                f,
                "Parser error at line {} column {}: {}",
                self.location.start.line + 1,
                self.location.start.column + 1,
                self.message
            )
        } else if self.location.start.line == self.location.end.line {
            write!(
                f,
                "Parser error between at line {} between columns {} and column {}: {}",
                self.location.start.line + 1,
                self.location.start.column + 1,
                self.location.end.column + 1,
                self.message
            )
        } else {
            write!(
                f,
                "Parser error between line {} column {} and line {} column {}: {}",
                self.location.start.line + 1,
                self.location.start.column + 1,
                self.location.end.line + 1,
                self.location.end.column + 1,
                self.message
            )
        }
    }
}

impl Error for SyntaxError {}

impl From<SyntaxError> for io::Error {
    #[inline]
    fn from(error: SyntaxError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, error)
    }
}

/// A parsing error.
///
/// It is the union of [`SyntaxError`] and [`std::io::Error`].
#[derive(Debug)]
pub enum ParseError {
    /// I/O error during parsing (file not found...).
    Io(io::Error),
    /// An error in the file syntax.
    Syntax(SyntaxError),
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
        Some(match self {
            Self::Io(e) => e,
            Self::Syntax(e) => e,
        })
    }
}

impl From<SyntaxError> for ParseError {
    #[inline]
    fn from(error: SyntaxError) -> Self {
        Self::Syntax(error)
    }
}

impl From<io::Error> for ParseError {
    #[inline]
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ParseError> for io::Error {
    #[inline]
    fn from(error: ParseError) -> Self {
        match error {
            ParseError::Syntax(e) => e.into(),
            ParseError::Io(e) => e,
        }
    }
}
