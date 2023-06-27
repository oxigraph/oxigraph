use memchr::memchr2;
use std::error::Error;
use std::fmt;
use std::io::{self, Read};
use std::ops::{Range, RangeInclusive};
#[cfg(feature = "async-tokio")]
use tokio::io::{AsyncRead, AsyncReadExt};

pub trait TokenRecognizer {
    type Token<'a>
    where
        Self: 'a;
    type Options: Default;

    fn recognize_next_token<'a>(
        &mut self,
        data: &'a [u8],
        is_ending: bool,
        config: &Self::Options,
    ) -> Option<(usize, Result<Self::Token<'a>, TokenRecognizerError>)>;
}

pub struct TokenRecognizerError {
    pub position: Range<usize>,
    pub message: String,
}

impl<S: Into<String>> From<(Range<usize>, S)> for TokenRecognizerError {
    fn from((position, message): (Range<usize>, S)) -> Self {
        Self {
            position,
            message: message.into(),
        }
    }
}

#[allow(clippy::range_plus_one)]
impl<S: Into<String>> From<(RangeInclusive<usize>, S)> for TokenRecognizerError {
    fn from((position, message): (RangeInclusive<usize>, S)) -> Self {
        (*position.start()..*position.end() + 1, message).into()
    }
}

impl<S: Into<String>> From<(usize, S)> for TokenRecognizerError {
    fn from((position, message): (usize, S)) -> Self {
        (position..=position, message).into()
    }
}

pub struct TokenWithPosition<T> {
    pub token: T,
    pub position: Range<usize>,
}

pub struct Lexer<R: TokenRecognizer> {
    parser: R,
    data: Vec<u8>,
    start: usize,
    end: usize,
    is_ending: bool,
    position: usize,
    min_buffer_size: usize,
    max_buffer_size: usize,
    is_line_jump_whitespace: bool,
    line_comment_start: Option<&'static [u8]>,
}

impl<R: TokenRecognizer> Lexer<R> {
    pub fn new(
        parser: R,
        min_buffer_size: usize,
        max_buffer_size: usize,
        is_line_jump_whitespace: bool,
        line_comment_start: Option<&'static [u8]>,
    ) -> Self {
        Self {
            parser,
            data: Vec::new(),
            start: 0,
            end: 0,
            is_ending: false,
            position: 0,
            min_buffer_size,
            max_buffer_size,
            is_line_jump_whitespace,
            line_comment_start,
        }
    }

    pub fn extend_from_slice(&mut self, other: &[u8]) {
        self.shrink_if_useful();
        self.data.truncate(self.end);
        self.data.extend_from_slice(other);
        self.end = self.data.len();
    }

    #[inline]
    pub fn end(&mut self) {
        self.is_ending = true;
    }

    pub fn extend_from_read(&mut self, read: &mut impl Read) -> io::Result<()> {
        self.shrink_if_useful();
        let min_end = self.end + self.min_buffer_size;
        if min_end > self.max_buffer_size {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                format!(
                    "The buffer maximal size is {} < {min_end}",
                    self.max_buffer_size
                ),
            ));
        }
        if self.data.len() < min_end {
            self.data.resize(min_end, 0);
        }
        if self.data.len() < self.data.capacity() {
            // We keep extending to have as much space as available without reallocation
            self.data.resize(self.data.capacity(), 0);
        }
        let read = read.read(&mut self.data[self.end..])?;
        self.end += read;
        self.is_ending = read == 0;
        Ok(())
    }

    #[cfg(feature = "async-tokio")]
    pub async fn extend_from_tokio_async_read(
        &mut self,
        read: &mut (impl AsyncRead + Unpin),
    ) -> io::Result<()> {
        self.shrink_if_useful();
        let min_end = self.end + self.min_buffer_size;
        if min_end > self.max_buffer_size {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                format!(
                    "The buffer maximal size is {} < {min_end}",
                    self.max_buffer_size
                ),
            ));
        }
        if self.data.len() < min_end {
            self.data.resize(min_end, 0);
        }
        if self.data.len() < self.data.capacity() {
            // We keep extending to have as much space as available without reallocation
            self.data.resize(self.data.capacity(), 0);
        }
        let read = read.read(&mut self.data[self.end..]).await?;
        self.end += read;
        self.is_ending = read == 0;
        Ok(())
    }

    pub fn read_next(
        &mut self,
        options: &R::Options,
    ) -> Option<Result<TokenWithPosition<R::Token<'_>>, LexerError>> {
        self.skip_whitespaces_and_comments()?;
        let (consumed, result) = if let Some(r) = self.parser.recognize_next_token(
            &self.data[self.start..self.end],
            self.is_ending,
            options,
        ) {
            r
        } else {
            return if self.is_ending {
                if self.start == self.end {
                    None // We have finished
                } else {
                    let error = LexerError {
                        position: self.position..self.position + (self.end - self.start),
                        message: "Unexpected end of file".into(),
                    };
                    self.end = self.start; // We consume everything
                    Some(Err(error))
                }
            } else {
                None
            };
        };
        debug_assert!(
            consumed > 0,
            "The lexer must consume at least one byte each time"
        );
        debug_assert!(
            self.start + consumed <= self.end,
            "The lexer tried to consumed {consumed} bytes but only {} bytes are readable",
            self.end - self.start
        );
        let old_position = self.position;
        self.start += consumed;
        self.position += consumed;
        Some(match result {
            Ok(token) => Ok(TokenWithPosition {
                token,
                position: old_position..self.position,
            }),
            Err(e) => Err(LexerError {
                position: e.position.start + self.position..e.position.end + self.position,
                message: e.message,
            }),
        })
    }

    pub fn is_end(&self) -> bool {
        self.is_ending && self.end == self.start
    }

    fn skip_whitespaces_and_comments(&mut self) -> Option<()> {
        loop {
            self.skip_whitespaces();

            let buf = &self.data[self.start..self.end];
            if let Some(line_comment_start) = self.line_comment_start {
                if buf.starts_with(line_comment_start) {
                    // Comment
                    if let Some(end) = memchr2(b'\r', b'\n', &buf[line_comment_start.len()..]) {
                        self.start += end + line_comment_start.len();
                        self.position += end + line_comment_start.len();
                        continue;
                    }
                    if self.is_ending {
                        self.end = self.start; // EOF
                        return Some(());
                    }
                    return None; // We need more data
                }
            }
            return Some(());
        }
    }

    fn skip_whitespaces(&mut self) {
        if self.is_line_jump_whitespace {
            for (i, c) in self.data[self.start..self.end].iter().enumerate() {
                if !matches!(c, b' ' | b'\t' | b'\r' | b'\n') {
                    self.start += i;
                    self.position += i;
                    return;
                }
                //TODO: SIMD
            }
        } else {
            for (i, c) in self.data[self.start..self.end].iter().enumerate() {
                if !matches!(c, b' ' | b'\t') {
                    self.start += i;
                    self.position += i;
                    return;
                }
                //TODO: SIMD
            }
        }
        // We only have whitespaces
        self.position += self.end - self.start;
        self.end = self.start;
    }

    fn shrink_if_useful(&mut self) {
        if self.start * 2 > self.data.len() {
            // We have read more than half of the buffer, let's move the data to the beginning
            self.data.copy_within(self.start..self.end, 0);
            self.end -= self.start;
            self.start = 0;
        }
    }
}

#[derive(Debug)]
pub struct LexerError {
    position: Range<usize>,
    message: String,
}

impl LexerError {
    pub fn position(&self) -> Range<usize> {
        self.position.clone()
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn into_message(self) -> String {
        self.message
    }
}

impl fmt::Display for LexerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.position.start + 1 == self.position.end {
            write!(
                f,
                "Lexer error at byte {}: {}",
                self.position.start, self.message
            )
        } else {
            write!(
                f,
                "Lexer error between bytes {} and {}: {}",
                self.position.start, self.position.end, self.message
            )
        }
    }
}

impl Error for LexerError {
    fn description(&self) -> &str {
        self.message()
    }
}
