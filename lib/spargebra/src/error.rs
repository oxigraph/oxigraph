use chumsky::error::Rich;
use chumsky::span::{SimpleSpan, Span};
use std::fmt;
use std::ops::Range;

/// Error returned during SPARQL parsing.
#[derive(Debug, thiserror::Error)]
pub struct SparqlSyntaxError {
    message: String,
    location: Range<TextPosition>,
}

impl SparqlSyntaxError {
    pub(crate) fn from_chumsky<T: fmt::Display>(errors: Vec<Rich<'_, T>>, text: &str) -> Self {
        errors.into_iter().next().map_or_else(
            || SparqlSyntaxError {
                message: "Unknown parser error".into(),
                location: TextPosition::from_text_span(text, SimpleSpan::new((), 0..text.len())),
            },
            |e| SparqlSyntaxError {
                message: e.reason().to_string(),
                location: TextPosition::from_text_span(text, *e.span()),
            },
        )
    }

    pub(crate) fn from_algebra_builder(error: AlgebraBuilderError, text: &str) -> Self {
        Self {
            message: error.message,
            location: TextPosition::from_text_span(text, error.location),
        }
    }

    /// The location of the error inside the file as a byte offsets.
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

impl fmt::Display for SparqlSyntaxError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.location.start.offset + 1 >= self.location.end.offset {
            write!(
                f,
                "Syntax error at line {} column {}: {}",
                self.location.start.line + 1,
                self.location.start.column + 1,
                self.message
            )
        } else if self.location.start.line == self.location.end.line {
            write!(
                f,
                "Syntax error at line {} between columns {} and {}: {}",
                self.location.start.line + 1,
                self.location.start.column + 1,
                self.location.end.column + 1,
                self.message
            )
        } else {
            write!(
                f,
                "Syntax error between line {} column {} and line {} column {}: {}",
                self.location.start.line + 1,
                self.location.start.column + 1,
                self.location.end.line + 1,
                self.location.end.column + 1,
                self.message
            )
        }
    }
}

/// A position in a text i.e. a `line` number starting from 0, a `column` number starting from 0 (in number of code points) and a global file `offset` starting from 0 (in number of bytes).
#[derive(Eq, PartialEq, Debug, Copy, Clone)]
pub struct TextPosition {
    pub line: u64,
    pub column: u64,
    pub offset: u64,
}

impl TextPosition {
    fn from_text_span(text: &str, span: SimpleSpan) -> Range<TextPosition> {
        Self::from_text_position(text, span.start)..Self::from_text_position(text, span.end)
    }

    fn from_text_position(text: &str, position: usize) -> TextPosition {
        let mut line_count = 0;
        let mut previous_line = "";
        for line in text[..position].split('\n') {
            line_count += 1;
            previous_line = line;
        }
        TextPosition {
            line: line_count - 1,
            column: previous_line.len().try_into().unwrap(),
            offset: position.try_into().unwrap(),
        }
    }
}

pub(crate) struct AlgebraBuilderError {
    message: String,
    location: SimpleSpan,
}

impl AlgebraBuilderError {
    pub(crate) fn new(location: SimpleSpan, message: impl Into<String>) -> Self {
        Self {
            location,
            message: message.into(),
        }
    }
}
