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
/// It is composed of a message, optional context about what was expected/found,
/// and a byte range in the input.
#[derive(Debug, thiserror::Error)]
pub struct TurtleSyntaxError {
    location: Range<TextPosition>,
    message: String,
    expected: Option<String>,
    found: Option<String>,
    suggestion: Option<String>,
}

impl TurtleSyntaxError {
    pub(crate) fn new(location: Range<TextPosition>, message: impl Into<String>) -> Self {
        Self {
            location,
            message: message.into(),
            expected: None,
            found: None,
            suggestion: None,
        }
    }

    /// Create an error with expected/found context.
    pub(crate) fn with_context(
        location: Range<TextPosition>,
        message: impl Into<String>,
        expected: impl Into<String>,
        found: impl Into<String>,
    ) -> Self {
        Self {
            location,
            message: message.into(),
            expected: Some(expected.into()),
            found: Some(found.into()),
            suggestion: None,
        }
    }

    /// Add a suggestion to help fix the error.
    pub(crate) fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Create an error for an unexpected token.
    pub(crate) fn unexpected_token(
        location: Range<TextPosition>,
        expected: impl Into<String>,
        found: impl Into<String>,
    ) -> Self {
        let expected_str = expected.into();
        let found_str = found.into();
        Self {
            location,
            message: format!("Expected {}, but found {}", expected_str, found_str),
            expected: Some(expected_str),
            found: Some(found_str),
            suggestion: None,
        }
    }

    /// Create an error for an unexpected end of file.
    pub(crate) fn unexpected_eof(
        location: Range<TextPosition>,
        expected: impl Into<String>,
    ) -> Self {
        let expected_str = expected.into();
        Self {
            location,
            message: format!("Unexpected end of file while expecting {}", expected_str),
            expected: Some(expected_str),
            found: Some("end of file".to_string()),
            suggestion: Some("Check if the file is complete and all constructs are properly closed".to_string()),
        }
    }

    /// Create an error for invalid syntax.
    pub(crate) fn invalid_syntax(
        location: Range<TextPosition>,
        what: impl Into<String>,
        why: impl Into<String>,
    ) -> Self {
        let what_str = what.into();
        Self {
            location,
            message: format!("Invalid {}: {}", what_str, why.into()),
            expected: None,
            found: None,
            suggestion: None,
        }
    }

    /// Create an error for exceeding nesting depth limit.
    pub(crate) fn nesting_limit_exceeded(
        location: Range<TextPosition>,
        current_depth: usize,
        max_depth: usize,
    ) -> Self {
        Self {
            location,
            message: format!(
                "Parser nesting depth limit exceeded: current depth {} exceeds maximum allowed depth of {}",
                current_depth, max_depth
            ),
            expected: Some(format!("nesting depth ≤ {}", max_depth)),
            found: Some(format!("nesting depth {}", current_depth)),
            suggestion: Some(
                "Reduce the nesting depth of collections, blank nodes, or other nested structures. \
                If this is legitimate input, consider using a streaming approach or contact the administrator to increase the limit."
                    .to_string(),
            ),
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

    /// What was expected (if available).
    #[inline]
    pub fn expected(&self) -> Option<&str> {
        self.expected.as_deref()
    }

    /// What was found instead (if available).
    #[inline]
    pub fn found(&self) -> Option<&str> {
        self.found.as_deref()
    }

    /// A suggestion for fixing the error (if available).
    #[inline]
    pub fn suggestion(&self) -> Option<&str> {
        self.suggestion.as_deref()
    }
}

impl fmt::Display for TurtleSyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Format location
        let location = if self.location.start.offset + 1 >= self.location.end.offset {
            format!(
                "line {} column {}",
                self.location.start.line + 1,
                self.location.start.column + 1
            )
        } else if self.location.start.line == self.location.end.line {
            format!(
                "line {} columns {}-{}",
                self.location.start.line + 1,
                self.location.start.column + 1,
                self.location.end.column + 1
            )
        } else {
            format!(
                "line {} column {} to line {} column {}",
                self.location.start.line + 1,
                self.location.start.column + 1,
                self.location.end.line + 1,
                self.location.end.column + 1
            )
        };

        // Main error message
        writeln!(f, "RDF parsing error at {}", location)?;
        writeln!(f, "  {}", self.message)?;

        // Add expected/found context if available
        if let (Some(expected), Some(found)) = (&self.expected, &self.found) {
            writeln!(f, "  Expected: {}", expected)?;
            writeln!(f, "  Found:    {}", found)?;
        }

        // Add suggestion if available
        if let Some(suggestion) = &self.suggestion {
            writeln!(f, "  Suggestion: {}", suggestion)?;
        }

        Ok(())
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
