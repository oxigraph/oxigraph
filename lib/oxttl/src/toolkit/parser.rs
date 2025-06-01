use crate::toolkit::error::{TurtleParseError, TurtleSyntaxError};
use crate::toolkit::lexer::{Lexer, TokenOrLineJump, TokenRecognizer};
use std::io::Read;
use std::ops::Deref;
#[cfg(feature = "async-tokio")]
use tokio::io::AsyncRead;

pub trait RuleRecognizer: Sized {
    type TokenRecognizer: TokenRecognizer;
    type Output;
    type Context;

    fn error_recovery_state(self) -> Self;

    fn recognize_next(
        self,
        token: TokenOrLineJump<<Self::TokenRecognizer as TokenRecognizer>::Token<'_>>,
        context: &mut Self::Context,
        results: &mut Vec<Self::Output>,
        errors: &mut Vec<RuleRecognizerError>,
    ) -> Self;

    fn recognize_end(
        self,
        context: &mut Self::Context,
        results: &mut Vec<Self::Output>,
        errors: &mut Vec<RuleRecognizerError>,
    );

    fn lexer_options(
        context: &Self::Context,
    ) -> &<Self::TokenRecognizer as TokenRecognizer>::Options;
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

#[expect(clippy::partial_pub_fields)]
pub struct Parser<B, RR: RuleRecognizer> {
    lexer: Lexer<B, RR::TokenRecognizer>,
    state: Option<RR>,
    pub context: RR::Context,
    results: Vec<RR::Output>,
    errors: Vec<RuleRecognizerError>,
}

impl<B, RR: RuleRecognizer> Parser<B, RR> {
    pub fn new(lexer: Lexer<B, RR::TokenRecognizer>, recognizer: RR, context: RR::Context) -> Self {
        Self {
            lexer,
            state: Some(recognizer),
            context,
            results: vec![],
            errors: vec![],
        }
    }
}

impl<B: Deref<Target = [u8]>, RR: RuleRecognizer> Parser<B, RR> {
    #[inline]
    pub fn is_end(&self) -> bool {
        self.state.is_none() && self.results.is_empty() && self.errors.is_empty()
    }

    pub fn parse_next(&mut self) -> Option<Result<RR::Output, TurtleSyntaxError>> {
        loop {
            if let Some(error) = self.errors.pop() {
                return Some(Err(TurtleSyntaxError::new(
                    self.lexer.last_token_location(),
                    error
                        .message
                        .replace("TOKEN", &self.lexer.last_token_source()),
                )));
            }
            if let Some(result) = self.results.pop() {
                return Some(Ok(result));
            }
            if let Some(result) = self.lexer.parse_next(RR::lexer_options(&self.context)) {
                match result {
                    Ok(token) => {
                        self.state = self.state.take().map(|state| {
                            state.recognize_next(
                                token,
                                &mut self.context,
                                &mut self.results,
                                &mut self.errors,
                            )
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
                self.state.take()?.recognize_end(
                    &mut self.context,
                    &mut self.results,
                    &mut self.errors,
                )
            } else {
                return None;
            }
        }
    }
}

impl<RR: RuleRecognizer> Parser<Vec<u8>, RR> {
    #[inline]
    pub fn end(&mut self) {
        self.lexer.end()
    }

    pub fn extend_from_slice(&mut self, other: &[u8]) {
        self.lexer.extend_from_slice(other)
    }

    pub fn for_reader<R: Read>(self, reader: R) -> ReaderIterator<R, RR> {
        ReaderIterator {
            reader,
            parser: self,
        }
    }

    #[cfg(feature = "async-tokio")]
    pub fn for_tokio_async_reader<R: AsyncRead + Unpin>(
        self,
        reader: R,
    ) -> TokioAsyncReaderIterator<R, RR> {
        TokioAsyncReaderIterator {
            reader,
            parser: self,
        }
    }
}

impl<'a, RR: RuleRecognizer> IntoIterator for Parser<&'a [u8], RR> {
    type Item = Result<RR::Output, TurtleSyntaxError>;
    type IntoIter = SliceIterator<'a, RR>;

    fn into_iter(self) -> Self::IntoIter {
        SliceIterator { parser: self }
    }
}

#[expect(clippy::partial_pub_fields)]
pub struct ReaderIterator<R: Read, RR: RuleRecognizer> {
    reader: R,
    pub parser: Parser<Vec<u8>, RR>,
}

impl<R: Read, RR: RuleRecognizer> Iterator for ReaderIterator<R, RR> {
    type Item = Result<RR::Output, TurtleParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        while !self.parser.is_end() {
            if let Some(result) = self.parser.parse_next() {
                return Some(result.map_err(TurtleParseError::Syntax));
            }
            if let Err(e) = self.parser.lexer.extend_from_reader(&mut self.reader) {
                return Some(Err(e.into()));
            }
        }
        None
    }
}

#[cfg(feature = "async-tokio")]
pub struct TokioAsyncReaderIterator<R: AsyncRead + Unpin, RR: RuleRecognizer> {
    pub reader: R,
    pub parser: Parser<Vec<u8>, RR>,
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin, RR: RuleRecognizer> TokioAsyncReaderIterator<R, RR> {
    pub async fn next(&mut self) -> Option<Result<RR::Output, TurtleParseError>> {
        while !self.parser.is_end() {
            if let Some(result) = self.parser.parse_next() {
                return Some(result.map_err(TurtleParseError::Syntax));
            }
            if let Err(e) = self
                .parser
                .lexer
                .extend_from_tokio_async_read(&mut self.reader)
                .await
            {
                return Some(Err(e.into()));
            }
        }
        None
    }
}

pub struct SliceIterator<'a, RR: RuleRecognizer> {
    pub parser: Parser<&'a [u8], RR>,
}

impl<RR: RuleRecognizer> Iterator for SliceIterator<'_, RR> {
    type Item = Result<RR::Output, TurtleSyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.parser.parse_next()
    }
}
