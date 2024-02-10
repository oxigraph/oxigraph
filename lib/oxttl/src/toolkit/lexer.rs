use crate::toolkit::error::{TextPosition, TurtleSyntaxError};
use memchr::{memchr2, memchr2_iter};
use std::borrow::Cow;
use std::cmp::min;
use std::io::{self, Read};
use std::ops::{Range, RangeInclusive};
use std::str;
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
    pub location: Range<usize>,
    pub message: String,
}

impl<S: Into<String>> From<(Range<usize>, S)> for TokenRecognizerError {
    fn from((location, message): (Range<usize>, S)) -> Self {
        Self {
            location,
            message: message.into(),
        }
    }
}

#[allow(clippy::range_plus_one)]
impl<S: Into<String>> From<(RangeInclusive<usize>, S)> for TokenRecognizerError {
    fn from((location, message): (RangeInclusive<usize>, S)) -> Self {
        (*location.start()..*location.end() + 1, message).into()
    }
}

impl<S: Into<String>> From<(usize, S)> for TokenRecognizerError {
    fn from((location, message): (usize, S)) -> Self {
        (location..=location, message).into()
    }
}

pub struct Lexer<R: TokenRecognizer> {
    parser: R,
    data: Vec<u8>,
    position: Position,
    previous_position: Position, // Lexer position before the last emitted token
    is_ending: bool,
    min_buffer_size: usize,
    max_buffer_size: usize,
    is_line_jump_whitespace: bool,
    line_comment_start: Option<&'static [u8]>,
}

#[derive(Clone, Copy)]
struct Position {
    line_start_buffer_offset: usize,
    buffer_offset: usize,
    global_offset: u64,
    global_line: u64,
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
            position: Position {
                line_start_buffer_offset: 0,
                buffer_offset: 0,
                global_offset: 0,
                global_line: 0,
            },
            previous_position: Position {
                line_start_buffer_offset: 0,
                buffer_offset: 0,
                global_offset: 0,
                global_line: 0,
            },
            is_ending: false,
            min_buffer_size,
            max_buffer_size,
            is_line_jump_whitespace,
            line_comment_start,
        }
    }

    pub fn extend_from_slice(&mut self, other: &[u8]) {
        self.shrink_data();
        self.data.extend_from_slice(other);
    }

    #[inline]
    pub fn end(&mut self) {
        self.is_ending = true;
    }

    pub fn extend_from_read(&mut self, read: &mut impl Read) -> io::Result<()> {
        self.shrink_data();
        if self.data.len() == self.max_buffer_size {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                format!(
                    "Reached the buffer maximal size of {}",
                    self.max_buffer_size
                ),
            ));
        }
        let min_end = min(self.data.len() + self.min_buffer_size, self.max_buffer_size);
        let new_start = self.data.len();
        self.data.resize(min_end, 0);
        if self.data.len() < self.data.capacity() {
            // We keep extending to have as much space as available without reallocation
            self.data.resize(self.data.capacity(), 0);
        }
        let read = read.read(&mut self.data[new_start..])?;
        self.data.truncate(new_start + read);
        self.is_ending = read == 0;
        Ok(())
    }

    #[cfg(feature = "async-tokio")]
    pub async fn extend_from_tokio_async_read(
        &mut self,
        read: &mut (impl AsyncRead + Unpin),
    ) -> io::Result<()> {
        self.shrink_data();
        if self.data.len() == self.max_buffer_size {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                format!(
                    "Reached the buffer maximal size of {}",
                    self.max_buffer_size
                ),
            ));
        }
        let min_end = min(self.data.len() + self.min_buffer_size, self.max_buffer_size);
        let new_start = self.data.len();
        self.data.resize(min_end, 0);
        if self.data.len() < self.data.capacity() {
            // We keep extending to have as much space as available without reallocation
            self.data.resize(self.data.capacity(), 0);
        }
        let read = read.read(&mut self.data[new_start..]).await?;
        self.data.truncate(new_start + read);
        self.is_ending = read == 0;
        Ok(())
    }

    #[allow(clippy::unwrap_in_result)]
    pub fn read_next(
        &mut self,
        options: &R::Options,
    ) -> Option<Result<R::Token<'_>, TurtleSyntaxError>> {
        self.skip_whitespaces_and_comments()?;
        self.previous_position = self.position;
        let Some((consumed, result)) = self.parser.recognize_next_token(
            &self.data[self.position.buffer_offset..],
            self.is_ending,
            options,
        ) else {
            return if self.is_ending {
                if self.position.buffer_offset == self.data.len() {
                    None // We have finished
                } else {
                    let (new_line_jumps, new_line_start) =
                        Self::find_number_of_line_jumps_and_start_of_last_line(
                            &self.data[self.position.buffer_offset..],
                        );
                    if new_line_jumps > 0 {
                        self.position.line_start_buffer_offset =
                            self.position.buffer_offset + new_line_start;
                    }
                    self.position.global_offset +=
                        u64::try_from(self.data.len() - self.position.buffer_offset).unwrap();
                    self.position.buffer_offset = self.data.len();
                    self.position.global_line += new_line_jumps;
                    let new_position = TextPosition {
                        line: self.position.global_line,
                        column: Self::column_from_bytes(
                            &self.data[self.position.line_start_buffer_offset..],
                        ),
                        offset: self.position.global_offset,
                    };
                    let error = TurtleSyntaxError {
                        location: new_position..new_position,
                        message: "Unexpected end of file".into(),
                    };
                    self.position.buffer_offset = self.data.len(); // We consume everything
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
            self.position.buffer_offset + consumed <= self.data.len(),
            "The lexer tried to consumed {consumed} bytes but only {} bytes are readable",
            self.data.len() - self.position.buffer_offset
        );
        let (new_line_jumps, new_line_start) =
            Self::find_number_of_line_jumps_and_start_of_last_line(
                &self.data[self.position.buffer_offset..self.position.buffer_offset + consumed],
            );
        if new_line_jumps > 0 {
            self.position.line_start_buffer_offset = self.position.buffer_offset + new_line_start;
        }
        self.position.buffer_offset += consumed;
        self.position.global_offset += u64::try_from(consumed).unwrap();
        self.position.global_line += new_line_jumps;
        Some(result.map_err(|e| TurtleSyntaxError {
            location: self.location_from_buffer_offset_range(e.location),
            message: e.message,
        }))
    }

    pub fn location_from_buffer_offset_range(
        &self,
        offset_range: Range<usize>,
    ) -> Range<TextPosition> {
        let start_offset = self.previous_position.buffer_offset + offset_range.start;
        let (start_extra_line_jumps, start_line_start) =
            Self::find_number_of_line_jumps_and_start_of_last_line(
                &self.data[self.previous_position.buffer_offset..start_offset],
            );
        let start_line_start = if start_extra_line_jumps > 0 {
            start_line_start + self.previous_position.buffer_offset
        } else {
            self.previous_position.line_start_buffer_offset
        };
        let end_offset = self.previous_position.buffer_offset + offset_range.end;
        let (end_extra_line_jumps, end_line_start) =
            Self::find_number_of_line_jumps_and_start_of_last_line(
                &self.data[self.previous_position.buffer_offset..end_offset],
            );
        let end_line_start = if end_extra_line_jumps > 0 {
            end_line_start + self.previous_position.buffer_offset
        } else {
            self.previous_position.line_start_buffer_offset
        };
        TextPosition {
            line: self.previous_position.global_line + start_extra_line_jumps,
            column: Self::column_from_bytes(&self.data[start_line_start..start_offset]),
            offset: self.previous_position.global_offset
                + u64::try_from(offset_range.start).unwrap(),
        }..TextPosition {
            line: self.previous_position.global_line + end_extra_line_jumps,
            column: Self::column_from_bytes(&self.data[end_line_start..end_offset]),
            offset: self.previous_position.global_offset + u64::try_from(offset_range.end).unwrap(),
        }
    }

    pub fn last_token_location(&self) -> Range<TextPosition> {
        TextPosition {
            line: self.previous_position.global_line,
            column: Self::column_from_bytes(
                &self.data[self.previous_position.line_start_buffer_offset
                    ..self.previous_position.buffer_offset],
            ),
            offset: self.previous_position.global_offset,
        }..TextPosition {
            line: self.position.global_line,
            column: Self::column_from_bytes(
                &self.data[self.position.line_start_buffer_offset..self.position.buffer_offset],
            ),
            offset: self.position.global_offset,
        }
    }

    pub fn last_token_source(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(
            &self.data[self.previous_position.buffer_offset..self.position.buffer_offset],
        )
    }

    pub fn is_end(&self) -> bool {
        self.is_ending && self.data.len() == self.position.buffer_offset
    }

    #[allow(clippy::unwrap_in_result)]
    fn skip_whitespaces_and_comments(&mut self) -> Option<()> {
        loop {
            self.skip_whitespaces()?;

            let buf = &self.data[self.position.buffer_offset..];
            if let Some(line_comment_start) = self.line_comment_start {
                if buf.starts_with(line_comment_start) {
                    // Comment
                    if let Some(end) = memchr2(b'\r', b'\n', &buf[line_comment_start.len()..]) {
                        let mut end_position = line_comment_start.len() + end;
                        if buf.get(end_position).copied() == Some(b'\r') {
                            // We look for \n for Windows line end style
                            if let Some(c) = buf.get(end_position + 1) {
                                if *c == b'\n' {
                                    end_position += 1;
                                }
                            } else if !self.is_ending {
                                return None; // We need to read more
                            }
                        }
                        let comment_size = end_position + 1;
                        self.position.buffer_offset += comment_size;
                        self.position.line_start_buffer_offset = self.position.buffer_offset;
                        self.position.global_offset += u64::try_from(comment_size).unwrap();
                        self.position.global_line += 1;
                        continue;
                    }
                    if self.is_ending {
                        self.position.buffer_offset = self.data.len(); // EOF
                        return Some(());
                    }
                    return None; // We need more data
                }
            }
            return Some(());
        }
    }

    fn skip_whitespaces(&mut self) -> Option<()> {
        if self.is_line_jump_whitespace {
            let mut i = self.position.buffer_offset;
            while let Some(c) = self.data.get(i) {
                match c {
                    b' ' | b'\t' => {
                        self.position.buffer_offset += 1;
                        self.position.global_offset += 1;
                    }
                    b'\r' => {
                        // We look for \n for Windows line end style
                        let mut increment: u8 = 1;
                        if let Some(c) = self.data.get(i + 1) {
                            if *c == b'\n' {
                                increment += 1;
                                i += 1;
                            }
                        } else if !self.is_ending {
                            return None; // We need to read more
                        }
                        self.position.buffer_offset += usize::from(increment);
                        self.position.line_start_buffer_offset = self.position.buffer_offset;
                        self.position.global_offset += u64::from(increment);
                        self.position.global_line += 1;
                    }
                    b'\n' => {
                        self.position.buffer_offset += 1;
                        self.position.line_start_buffer_offset = self.position.buffer_offset;
                        self.position.global_offset += 1;
                        self.position.global_line += 1;
                    }
                    _ => return Some(()),
                }
                i += 1;
                // TODO: SIMD
            }
        } else {
            for c in &self.data[self.position.buffer_offset..] {
                if matches!(c, b' ' | b'\t') {
                    self.position.buffer_offset += 1;
                    self.position.global_offset += 1;
                } else {
                    return Some(());
                }
                // TODO: SIMD
            }
        }
        Some(())
    }

    fn shrink_data(&mut self) {
        if self.position.line_start_buffer_offset > 0 {
            self.data
                .copy_within(self.position.line_start_buffer_offset.., 0);
            self.data
                .truncate(self.data.len() - self.position.line_start_buffer_offset);
            self.position.buffer_offset -= self.position.line_start_buffer_offset;
            self.position.line_start_buffer_offset = 0;
            self.previous_position = self.position;
        }
    }

    fn find_number_of_line_jumps_and_start_of_last_line(bytes: &[u8]) -> (u64, usize) {
        let mut num_of_jumps = 0;
        let mut last_jump_pos = 0;
        let mut previous_cr = 0;
        for pos in memchr2_iter(b'\r', b'\n', bytes) {
            if bytes[pos] == b'\r' {
                previous_cr = pos;
                num_of_jumps += 1;
                last_jump_pos = pos + 1;
            } else {
                if previous_cr < pos - 1 {
                    // We count \r\n as a single line jump
                    num_of_jumps += 1;
                }
                last_jump_pos = pos + 1;
            }
        }
        (num_of_jumps, last_jump_pos)
    }

    fn column_from_bytes(bytes: &[u8]) -> u64 {
        match str::from_utf8(bytes) {
            Ok(s) => u64::try_from(s.chars().count()).unwrap(),
            Err(e) => {
                if e.valid_up_to() == 0 {
                    0
                } else {
                    Self::column_from_bytes(&bytes[..e.valid_up_to()])
                }
            }
        }
    }
}
