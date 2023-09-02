use crate::toolkit::error::{ParseError, SyntaxError};
use crate::toolkit::lexer::{Lexer, TokenRecognizer};
use std::io::Read;
#[cfg(feature = "async-tokio")]
use tokio::io::AsyncRead;

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
    default_lexer_options: <RR::TokenRecognizer as TokenRecognizer>::Options,
}

impl<RR: RuleRecognizer> Parser<RR> {
    pub fn new(lexer: Lexer<RR::TokenRecognizer>, recognizer: RR) -> Self {
        Self {
            lexer,
            state: Some(recognizer),
            results: vec![],
            errors: vec![],
            default_lexer_options: <RR::TokenRecognizer as TokenRecognizer>::Options::default(),
        }
    }

    pub fn extend_from_slice(&mut self, other: &[u8]) {
        self.lexer.extend_from_slice(other)
    }

    #[inline]
    pub fn end(&mut self) {
        self.lexer.end()
    }

    #[inline]
    pub fn is_end(&self) -> bool {
        self.state.is_none() && self.results.is_empty() && self.errors.is_empty()
    }

    pub fn read_next(&mut self) -> Option<Result<RR::Output, SyntaxError>> {
        loop {
            if let Some(error) = self.errors.pop() {
                return Some(Err(SyntaxError {
                    location: self.lexer.last_token_location(),
                    message: error
                        .message
                        .replace("TOKEN", &self.lexer.last_token_source()),
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
                    Ok(token) => {
                        self.state = self.state.take().map(|state| {
                            state.recognize_next(token, &mut self.results, &mut self.errors)
                        });
                        continue;
                    }
                    Err(e) => {
                        self.state = self.state.take().map(RR::error_recovery_state);
                        return Some(Err(e));
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

    pub fn parse_read<R: Read>(self, read: R) -> FromReadIterator<R, RR> {
        FromReadIterator { read, parser: self }
    }

    #[cfg(feature = "async-tokio")]
    pub fn parse_tokio_async_read<R: AsyncRead + Unpin>(
        self,
        read: R,
    ) -> FromTokioAsyncReadIterator<R, RR> {
        FromTokioAsyncReadIterator { read, parser: self }
    }
}

pub struct FromReadIterator<R: Read, RR: RuleRecognizer> {
    read: R,
    parser: Parser<RR>,
}

impl<R: Read, RR: RuleRecognizer> Iterator for FromReadIterator<R, RR> {
    type Item = Result<RR::Output, ParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        while !self.parser.is_end() {
            if let Some(result) = self.parser.read_next() {
                return Some(result.map_err(ParseError::Syntax));
            }
            if let Err(e) = self.parser.lexer.extend_from_read(&mut self.read) {
                return Some(Err(e.into()));
            }
        }
        None
    }
}

#[cfg(feature = "async-tokio")]
pub struct FromTokioAsyncReadIterator<R: AsyncRead + Unpin, RR: RuleRecognizer> {
    read: R,
    parser: Parser<RR>,
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin, RR: RuleRecognizer> FromTokioAsyncReadIterator<R, RR> {
    pub async fn next(&mut self) -> Option<Result<RR::Output, ParseError>> {
        while !self.parser.is_end() {
            if let Some(result) = self.parser.read_next() {
                return Some(result.map_err(ParseError::Syntax));
            }
            if let Err(e) = self
                .parser
                .lexer
                .extend_from_tokio_async_read(&mut self.read)
                .await
            {
                return Some(Err(e.into()));
            }
        }
        None
    }
}
