use crate::toolkit::lexer::TokenWithPosition;
use crate::toolkit::{Lexer, LexerError, TokenRecognizer};
use std::error::Error;
use std::io::Read;
use std::ops::Range;
use std::{fmt, io};

pub trait RuleRecognizer: Sized {
    type TokenRecognizer: TokenRecognizer;
    type Output;

    fn error_recovery_state(self) -> Self;

    fn recognize_next(
        self,
        token: <Self::TokenRecognizer as TokenRecognizer>::Token<'_>,
        results: &mut Vec<Self::Output>,
        errors: &mut Vec<RuleRecognizerError>,
    ) -> Self;

    fn recognize_end(self, results: &mut Vec<Self::Output>, errors: &mut Vec<RuleRecognizerError>);

    fn lexer_options(&self) -> &<Self::TokenRecognizer as TokenRecognizer>::Options;
}

pub struct RuleRecognizerError {
    pub message: String,
}

impl<S: Into<String>> From<S> for RuleRecognizerError {
    fn from(message: S) -> Self {
        Self {
            message: message.into(),
        }
    }
}

pub struct Parser<RR: RuleRecognizer> {
    lexer: Lexer<RR::TokenRecognizer>,
    state: Option<RR>,
    results: Vec<RR::Output>,
    errors: Vec<RuleRecognizerError>,
    position: Range<usize>,
    default_lexer_options: <RR::TokenRecognizer as TokenRecognizer>::Options,
}

impl<RR: RuleRecognizer> Parser<RR> {
    pub fn new(lexer: Lexer<RR::TokenRecognizer>, recognizer: RR) -> Self {
        Self {
            lexer,
            state: Some(recognizer),
            results: vec![],
            errors: vec![],
            position: 0..0,
            default_lexer_options: <RR::TokenRecognizer as TokenRecognizer>::Options::default(),
        }
    }

    pub fn extend_from_slice(&mut self, other: &[u8]) {
        self.lexer.extend_from_slice(other)
    }

    pub fn end(&mut self) {
        self.lexer.end()
    }

    pub fn is_end(&self) -> bool {
        self.state.is_none() && self.results.is_empty() && self.errors.is_empty()
    }

    pub fn read_next(&mut self) -> Option<Result<RR::Output, ParseError>> {
        loop {
            if let Some(error) = self.errors.pop() {
                return Some(Err(ParseError {
                    position: self.position.clone(),
                    message: error.message,
                }));
            }
            if let Some(result) = self.results.pop() {
                return Some(Ok(result));
            }
            if let Some(result) = self.lexer.read_next(
                self.state
                    .as_ref()
                    .map_or(&self.default_lexer_options, |p| p.lexer_options()),
            ) {
                match result {
                    Ok(TokenWithPosition { token, position }) => {
                        self.position = position;
                        self.state = self.state.take().map(|state| {
                            state.recognize_next(token, &mut self.results, &mut self.errors)
                        });
                        continue;
                    }
                    Err(e) => {
                        self.state = self.state.take().map(RR::error_recovery_state);
                        return Some(Err(e.into()));
                    }
                }
            }
            if self.lexer.is_end() {
                let Some(state) = self.state.take() else {
                    return None;
                };
                state.recognize_end(&mut self.results, &mut self.errors)
            } else {
                return None;
            }
        }
    }

    pub fn parse_from_read<R: Read>(self, read: R) -> FromReadIterator<R, RR> {
        FromReadIterator { read, parser: self }
    }
}

/// An error from parsing.
///
/// It is composed of a message and a byte range in the input.
#[derive(Debug)]
pub struct ParseError {
    position: Range<usize>,
    message: String,
}

impl ParseError {
    /// The invalid byte range in the input.
    pub fn position(&self) -> Range<usize> {
        self.position.clone()
    }

    /// The error message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Converts this error to an error message.
    pub fn into_message(self) -> String {
        self.message
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.position.start + 1 == self.position.end {
            write!(
                f,
                "Parser error at byte {}: {}",
                self.position.start, self.message
            )
        } else {
            write!(
                f,
                "Parser error between bytes {} and {}: {}",
                self.position.start, self.position.end, self.message
            )
        }
    }
}

impl Error for ParseError {}

impl From<ParseError> for io::Error {
    fn from(error: ParseError) -> Self {
        io::Error::new(io::ErrorKind::InvalidData, error)
    }
}

impl From<LexerError> for ParseError {
    fn from(e: LexerError) -> Self {
        Self {
            position: e.position(),
            message: e.into_message(),
        }
    }
}

/// The union of [`ParseError`] and [`std::io::Error`].
#[derive(Debug)]
pub enum ParseOrIoError {
    Parse(ParseError),
    Io(io::Error),
}

impl fmt::Display for ParseOrIoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => e.fmt(f),
            Self::Io(e) => e.fmt(f),
        }
    }
}

impl Error for ParseOrIoError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Parse(e) => e,
            Self::Io(e) => e,
        })
    }
}

impl From<ParseError> for ParseOrIoError {
    fn from(error: ParseError) -> Self {
        Self::Parse(error)
    }
}

impl From<io::Error> for ParseOrIoError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<ParseOrIoError> for io::Error {
    fn from(error: ParseOrIoError) -> Self {
        match error {
            ParseOrIoError::Parse(e) => e.into(),
            ParseOrIoError::Io(e) => e,
        }
    }
}

pub struct FromReadIterator<R: Read, RR: RuleRecognizer> {
    read: R,
    parser: Parser<RR>,
}

impl<R: Read, RR: RuleRecognizer> Iterator for FromReadIterator<R, RR> {
    type Item = Result<RR::Output, ParseOrIoError>;

    fn next(&mut self) -> Option<Self::Item> {
        while !self.parser.is_end() {
            if let Some(result) = self.parser.read_next() {
                return Some(result.map_err(ParseOrIoError::Parse));
            }
            if let Err(e) = self.parser.lexer.extend_from_read(&mut self.read) {
                return Some(Err(e.into()));
            }
        }
        None
    }
}
