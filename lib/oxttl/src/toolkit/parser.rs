use crate::toolkit::error::{TurtleParseError, TurtleSyntaxError};
use crate::toolkit::lexer::{GrowableBuffer, Lexer, TokenOrLineJump, TokenRecognizer};
use crate::{MIN_BUFFER_SIZE, TextPosition};
use std::io::Read;
use std::ops::Range;
#[cfg(feature = "async-tokio")]
use tokio::io::AsyncRead;

pub trait RuleRecognizer: Sized {
    type TokenRecognizer: TokenRecognizer;
    type Output;
    type Context;

    fn set_error_recovery_state(&mut self);

    fn recognize_next(
        &mut self,
        token: TokenOrLineJump<<Self::TokenRecognizer as TokenRecognizer>::Token<'_>>,
        context: &mut Self::Context,
        results: &mut Vec<Self::Output>,
        errors: &mut Vec<RuleRecognizerError>,
    );

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
pub struct Parser<RR: RuleRecognizer> {
    lexer: Lexer<RR::TokenRecognizer>,
    state: Option<RR>,
    pub context: RR::Context,
    results: Vec<RR::Output>,
    errors: Vec<TurtleSyntaxError>,
    last_location: TextPosition,
}

impl<RR: RuleRecognizer> Parser<RR> {
    pub fn new(lexer: Lexer<RR::TokenRecognizer>, recognizer: RR, context: RR::Context) -> Self {
        Self {
            lexer,
            state: Some(recognizer),
            context,
            results: vec![],
            errors: vec![],
            last_location: TextPosition {
                line: 0,
                column: 0,
                offset: 0,
            },
        }
    }

    pub fn for_reader<R: Read>(self, reader: R, max_buffer_size: usize) -> ReaderIterator<R, RR> {
        ReaderIterator {
            reader,
            buffer: GrowableBuffer::new(MIN_BUFFER_SIZE, max_buffer_size),
            parser: self,
        }
    }

    #[cfg(feature = "async-tokio")]
    pub fn for_tokio_async_reader<R: AsyncRead + Unpin>(
        self,
        reader: R,
        max_buffer_size: usize,
    ) -> TokioAsyncReaderIterator<R, RR> {
        TokioAsyncReaderIterator {
            reader,
            buffer: GrowableBuffer::new(MIN_BUFFER_SIZE, max_buffer_size),
            parser: self,
        }
    }

    pub fn for_slice(self, slice: &[u8]) -> SliceIterator<'_, RR> {
        SliceIterator {
            slice,
            parser: self,
        }
    }

    pub fn low_level(self) -> LowLevelIterator<RR> {
        LowLevelIterator {
            buffer: Vec::new(),
            buffer_offset: 0,
            is_end: false,
            parser: self,
        }
    }

    #[inline]
    pub fn is_end(&self) -> bool {
        self.state.is_none() && self.results.is_empty() && self.errors.is_empty()
    }

    fn parse_next(
        &mut self,
        data: &[u8],
        is_ending: bool,
    ) -> (usize, Option<Result<RR::Output, TurtleSyntaxError>>) {
        let mut consumed = 0;
        loop {
            if let Some(error) = self.errors.pop() {
                return (consumed, Some(Err(error)));
            }
            if let Some(result) = self.results.pop() {
                return (consumed, Some(Ok(result)));
            }
            let (consumed_lexer, result) = self.lexer.parse_next(
                &data[consumed..],
                is_ending,
                RR::lexer_options(&self.context),
            );
            let token_content = &data[consumed..][..consumed_lexer];
            consumed += consumed_lexer;
            let Some(result) = result else {
                if is_ending {
                    let Some(state) = self.state.take() else {
                        return (consumed, None);
                    };
                    let mut errors = Vec::new();
                    state.recognize_end(&mut self.context, &mut self.results, &mut errors);
                    for error in errors {
                        Self::add_syntax_error(
                            &error,
                            self.last_location..self.last_location,
                            token_content,
                            &mut self.errors,
                        );
                    }
                    continue;
                }
                return (consumed, None);
            };
            match result {
                Ok((token, token_location)) => {
                    self.last_location = token_location.end;
                    if let Some(state) = &mut self.state {
                        let mut errors = Vec::new();
                        state.recognize_next(
                            token,
                            &mut self.context,
                            &mut self.results,
                            &mut errors,
                        );
                        for error in errors {
                            Self::add_syntax_error(
                                &error,
                                token_location.clone(),
                                token_content,
                                &mut self.errors,
                            );
                        }
                    }
                }
                Err(e) => {
                    if let Some(state) = &mut self.state {
                        state.set_error_recovery_state();
                    }
                    self.errors.push(e);
                }
            }
        }
    }

    fn add_syntax_error(
        error: &RuleRecognizerError,
        token_location: Range<TextPosition>,
        token_content: &[u8],
        errors: &mut Vec<TurtleSyntaxError>,
    ) {
        errors.push(TurtleSyntaxError::new(
            token_location,
            error.message.replace(
                "TOKEN",
                &String::from_utf8_lossy(token_content.trim_ascii()),
            ),
        ));
    }

    fn finish(&mut self) {
        self.state = None;
        self.results.clear();
        self.errors.clear();
    }
}

#[expect(clippy::partial_pub_fields)]
pub struct LowLevelIterator<RR: RuleRecognizer> {
    buffer: Vec<u8>,
    buffer_offset: usize,
    is_end: bool,
    pub parser: Parser<RR>,
}

impl<RR: RuleRecognizer> LowLevelIterator<RR> {
    pub fn extend_from_slice(&mut self, other: &[u8]) {
        if self.buffer_offset > 0 {
            self.buffer.copy_within(&self.buffer_offset.., 0);
            self.buffer.truncate(self.buffer.len() - self.buffer_offset);
            self.buffer_offset = 0;
        }
        self.buffer.extend_from_slice(other);
    }

    #[inline]
    pub fn end(&mut self) {
        self.is_end = true;
    }
}

impl<RR: RuleRecognizer> Iterator for LowLevelIterator<RR> {
    type Item = Result<RR::Output, TurtleSyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        let (consumed, result) = self
            .parser
            .parse_next(&self.buffer[self.buffer_offset..], self.is_end);
        self.buffer_offset += consumed;
        result
    }
}

#[expect(clippy::partial_pub_fields)]
pub struct ReaderIterator<R: Read, RR: RuleRecognizer> {
    reader: R,
    buffer: GrowableBuffer,
    pub parser: Parser<RR>,
}

impl<R: Read, RR: RuleRecognizer> Iterator for ReaderIterator<R, RR> {
    type Item = Result<RR::Output, TurtleParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        while !self.parser.is_end() {
            let (consumed, result) = self
                .parser
                .parse_next(self.buffer.as_ref(), self.buffer.is_ending());
            self.buffer.consume(consumed);
            if let Some(result) = result {
                return Some(result.map_err(TurtleParseError::Syntax));
            }
            if let Err(e) = self.buffer.extend_from_reader(&mut self.reader) {
                self.parser.finish();
                return Some(Err(e.into()));
            }
        }
        None
    }
}

#[cfg(feature = "async-tokio")]
#[expect(clippy::partial_pub_fields)]
pub struct TokioAsyncReaderIterator<R: AsyncRead + Unpin, RR: RuleRecognizer> {
    reader: R,
    buffer: GrowableBuffer,
    pub parser: Parser<RR>,
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin, RR: RuleRecognizer> TokioAsyncReaderIterator<R, RR> {
    pub async fn next(&mut self) -> Option<Result<RR::Output, TurtleParseError>> {
        while !self.parser.is_end() {
            let (consumed, result) = self
                .parser
                .parse_next(self.buffer.as_ref(), self.buffer.is_ending());
            self.buffer.consume(consumed);
            if let Some(result) = result {
                return Some(result.map_err(TurtleParseError::Syntax));
            }
            if let Err(e) = self
                .buffer
                .extend_from_tokio_async_reader(&mut self.reader)
                .await
            {
                self.parser.finish();
                return Some(Err(e.into()));
            }
        }
        None
    }
}

#[expect(clippy::partial_pub_fields)]
pub struct SliceIterator<'a, RR: RuleRecognizer> {
    pub parser: Parser<RR>,
    slice: &'a [u8],
}

impl<RR: RuleRecognizer> Iterator for SliceIterator<'_, RR> {
    type Item = Result<RR::Output, TurtleSyntaxError>;

    fn next(&mut self) -> Option<Result<RR::Output, TurtleSyntaxError>> {
        let (consumed, result) = self.parser.parse_next(self.slice, true);
        self.slice = &self.slice[consumed..];
        result
    }
}
