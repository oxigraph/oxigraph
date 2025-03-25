use std::ops::Range;
use std::{fmt, io};

/// A position in a text i.e. a `line` number starting from 0, a `column` number starting from 0 (in number of code points) and a global file `offset` starting from 0 (in number of bytes).
#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub struct TextPosition {
    pub line: u64,
    pub column: u64,
    pub offset: u64,
}

/// An error in the syntax of the parsed file.
///
/// It is composed of a message and a byte range in the input.
#[derive(Debug, thiserror::Error)]
pub struct TurtleSyntaxError {
    location: Range<TextPosition>,
    message: String,
}

impl TurtleSyntaxError {
    pub(crate) fn new(location: Range<TextPosition>, message: impl Into<String>) -> Self {
        Self {
            location,
            message: message.into(),
        }
    }

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

impl fmt::Display for TurtleSyntaxError {
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
                "Parser error at line {} between columns {} and {}: {}",
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

impl From<TurtleSyntaxError> for io::Error {
    #[inline]
    fn from(error: TurtleSyntaxError) -> Self {
        Self::new(io::ErrorKind::InvalidData, error)
    }
}

/// A parsing error.
///
/// It is the union of [`TurtleSyntaxError`] and [`io::Error`].
#[derive(Debug, thiserror::Error)]
pub enum TurtleParseError {
    /// I/O error during parsing (file not found...).
    #[error(transparent)]
    Io(#[from] io::Error),
    /// An error in the file syntax.
    #[error(transparent)]
    Syntax(#[from] TurtleSyntaxError),
}

impl From<TurtleParseError> for io::Error {
    #[inline]
    fn from(error: TurtleParseError) -> Self {
        match error {
            TurtleParseError::Syntax(e) => e.into(),
            TurtleParseError::Io(e) => e,
        }
    }
}
