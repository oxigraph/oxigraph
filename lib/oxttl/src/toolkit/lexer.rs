use crate::toolkit::error::{TextPosition, TurtleSyntaxError};
use memchr::{memchr2, memchr2_iter};
use std::cmp::{max, min};
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
        options: &Self::Options,
    ) -> Option<(usize, Result<Self::Token<'a>, TokenRecognizerError>)>;

    fn token_contains_line_jumps(token: &Self::Token<'_>) -> bool;
}

#[derive(Debug, PartialEq, Eq)]
pub enum TokenOrLineJump<T> {
    Token(T),
    LineJump,
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
    position: TextPosition,
    line_comment_start: Option<&'static [u8]>,
}

impl<R: TokenRecognizer> Lexer<R> {
    pub fn new(parser: R, line_comment_start: Option<&'static [u8]>) -> Self {
        Self {
            parser,
            position: TextPosition {
                offset: 0,
                line: 0,
                column: 0,
            },
            line_comment_start,
        }
    }

    pub fn parse_next<'a>(
        &mut self,
        data: &'a [u8],
        is_ending: bool,
        options: &R::Options,
    ) -> (
        usize,
        Option<Result<(TokenOrLineJump<R::Token<'a>>, Range<TextPosition>), TurtleSyntaxError>>,
    ) {
        let (read, has_line_jump) = self.skip_whitespaces_and_comments(data, is_ending);
        let Some(has_line_jump) = has_line_jump else {
            return (read, None);
        };
        if has_line_jump {
            return (
                read,
                Some(Ok((
                    TokenOrLineJump::LineJump,
                    self.position..self.position,
                ))),
            );
        }
        let previous_position = self.position;
        let data = &data[read..];
        let (consumed, result) = if let Some((consumed, result)) =
            self.parser.recognize_next_token(data, is_ending, options)
        {
            debug_assert!(
                consumed > 0,
                "The lexer must consume at least one byte each time"
            );
            (consumed, Some(result))
        } else if is_ending {
            (
                data.len(),
                // We fail if there are unrecognized bytes
                (!data.is_empty()).then(|| Err((0..data.len(), "Unexpected end of file").into())),
            )
        } else {
            (0, None)
        };
        debug_assert!(
            consumed <= data.len(),
            "The lexer tried to consumed {consumed} bytes but only {} bytes are readable",
            data.len()
        );
        let (new_line_jumps, line_offset) = match &result {
            Some(Ok(token)) if !R::token_contains_line_jumps(token) => {
                (0, u64::try_from(consumed).unwrap())
            }
            _ => Self::find_number_of_line_jumps_and_size_of_last_line(&data[..consumed]),
        };
        self.position.offset += u64::try_from(consumed).unwrap();
        self.position.line += new_line_jumps;
        if new_line_jumps > 0 {
            self.position.column = line_offset;
        } else {
            self.position.column += line_offset;
        }
        (
            read + consumed,
            result.map(|result| {
                result
                    .map(|token| {
                        (
                            TokenOrLineJump::Token(token),
                            previous_position..self.position,
                        )
                    })
                    .map_err(|e| {
                        TurtleSyntaxError::new(
                            Self::location_from_buffer_offset_range(
                                &previous_position,
                                e.location,
                                data,
                            ),
                            e.message,
                        )
                    })
            }),
        )
    }

    fn location_from_buffer_offset_range(
        previous_position: &TextPosition,
        offset_range: Range<usize>,
        data: &[u8],
    ) -> Range<TextPosition> {
        let (start_extra_line_jumps, mut start_last_line_size) =
            Self::find_number_of_line_jumps_and_size_of_last_line(&data[..offset_range.start]);
        if start_extra_line_jumps == 0 {
            start_last_line_size += previous_position.column;
        }
        let (end_extra_line_jumps, mut end_last_line_size) =
            Self::find_number_of_line_jumps_and_size_of_last_line(&data[..offset_range.end]);
        if end_extra_line_jumps == 0 {
            end_last_line_size += previous_position.column;
        }
        TextPosition {
            line: previous_position.line + start_extra_line_jumps,
            column: start_last_line_size,
            offset: previous_position.offset + u64::try_from(offset_range.start).unwrap(),
        }..TextPosition {
            line: previous_position.line + end_extra_line_jumps,
            column: end_last_line_size,
            offset: previous_position.offset + u64::try_from(offset_range.end).unwrap(),
        }
    }

    fn skip_whitespaces_and_comments(
        &mut self,
        data: &[u8],
        is_ending: bool,
    ) -> (usize, Option<bool>) {
        let (read, has_line_jump_or_missing) = self.skip_whitespaces(data, is_ending);
        let Some(has_line_jump_or_missing) = has_line_jump_or_missing else {
            return (read, None);
        };
        if has_line_jump_or_missing {
            return (read, Some(true));
        }

        let buf = &data[read..];
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
                        } else if !is_ending {
                            return (read, None); // We need to read more
                        }
                    }
                    let comment_size = end_position + 1;
                    self.position.offset += u64::try_from(comment_size).unwrap();
                    self.position.line += 1;
                    self.position.column = 0;
                    return (read + comment_size, Some(true));
                }
                if is_ending {
                    return (data.len(), Some(false));
                }
                return (read, None); // We need more data
            } else if !is_ending && buf.len() < line_comment_start.len() {
                return (read, None); // We need more data
            }
        }
        (read, Some(false))
    }

    fn skip_whitespaces(&mut self, data: &[u8], is_ending: bool) -> (usize, Option<bool>) {
        let mut i = 0;
        while let Some(c) = data.get(i) {
            match c {
                b' ' | b'\t' => {
                    self.position.offset += 1;
                    self.position.column += 1;
                }
                b'\r' => {
                    // We look for \n for Windows line end style
                    let mut increment: u8 = 1;
                    if let Some(c) = data.get(i + 1) {
                        if *c == b'\n' {
                            increment += 1;
                            i += 1;
                        }
                    } else if !is_ending {
                        return (i, None); // We need to read more
                    }
                    self.position.offset += u64::from(increment);
                    self.position.line += 1;
                    self.position.column = 0;
                    return (i + 1, Some(true));
                }
                b'\n' => {
                    self.position.offset += 1;
                    self.position.line += 1;
                    self.position.column = 0;
                    return (i + 1, Some(true));
                }
                _ => return (i, Some(false)),
            }
            i += 1;
            // TODO: SIMD
        }
        (i, is_ending.then_some(false)) // We return None if there is not enough data
    }

    fn find_number_of_line_jumps_and_size_of_last_line(bytes: &[u8]) -> (u64, u64) {
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
        (
            num_of_jumps,
            (bytes.len() - last_jump_pos).try_into().unwrap(),
        )
    }
}

pub struct GrowableBuffer {
    buffer: Vec<u8>,
    start_offset: usize,
    end_offset: usize,
    is_ending: bool,
    min_buffer_size: usize,
    max_buffer_size: usize,
}

impl GrowableBuffer {
    pub fn new(min_buffer_size: usize, max_buffer_size: usize) -> Self {
        Self {
            buffer: Vec::new(),
            start_offset: 0,
            end_offset: 0,
            is_ending: false,
            min_buffer_size,
            max_buffer_size,
        }
    }

    pub fn extend_from_reader(&mut self, read: &mut impl Read) -> io::Result<()> {
        self.prepare_buffer_for_read()?;

        // Read data from the reader into the buffer from the
        // lower bound until the end
        let bytes_read = read.read(&mut self.buffer[self.end_offset..])?;
        // Shrink the data to the length of the data read
        // minus any padding 0s present from the previous resize
        self.end_offset += bytes_read;
        self.is_ending = bytes_read == 0;
        Ok(())
    }

    #[cfg(feature = "async-tokio")]
    pub async fn extend_from_tokio_async_reader(
        &mut self,
        read: &mut (impl AsyncRead + Unpin),
    ) -> io::Result<()> {
        self.prepare_buffer_for_read()?;

        // Read data from the reader into the buffer from the
        // lower bound until the end
        let bytes_read = read.read(&mut self.buffer[self.end_offset..]).await?;
        // Shrink the data to the length of the data read
        // minus any padding 0s present from the previous resize
        self.end_offset += bytes_read;
        self.is_ending = bytes_read == 0;
        Ok(())
    }

    fn prepare_buffer_for_read(&mut self) -> io::Result<()> {
        self.shift_buffer();

        if self.buffer.len() >= self.max_buffer_size {
            return Err(io::Error::new(
                io::ErrorKind::OutOfMemory,
                format!(
                    "Reached the buffer maximal size of {}. The buffer size can be increased at the cost of higher memory use if large data is required",
                    self.max_buffer_size
                ),
            ));
        }

        let upper_bound = self.resized_buffer_len();
        // Fill the buffer until the upper bound with 0s
        if self.buffer.len() < upper_bound {
            self.buffer.resize(upper_bound, 0);

            // We keep extending to have as much space as available without reallocation
            if self.buffer.len() < self.buffer.capacity() {
                self.buffer.resize(self.buffer.capacity(), 0);
            }
        }

        Ok(())
    }

    // Return the new size for a buffer which exponentially grows in size
    fn resized_buffer_len(&self) -> usize {
        // Each one of these expressions will at least double the
        // size of the buffer, but in such a way that will not
        // exceed the maximum buffer size or allocate under the minimum buffer size.
        min(
            self.max_buffer_size,
            // We take the max here to ensure that
            // the buffer always has at least the size of the
            // data plus the minimum buffer size
            max(
                self.end_offset + self.min_buffer_size,
                self.end_offset.saturating_mul(2),
            ),
        )
    }

    fn shift_buffer(&mut self) {
        if self.start_offset == 0 {
            return; // Nothing to do
        }
        self.buffer
            .copy_within(self.start_offset..self.end_offset, 0);
        self.end_offset -= self.start_offset;
        self.start_offset = 0;
    }

    #[inline]
    pub fn is_ending(&self) -> bool {
        self.is_ending
    }

    #[inline]
    pub fn consume(&mut self, count: usize) {
        self.start_offset += count;
        debug_assert!(
            self.start_offset <= self.end_offset,
            "Too large buffer consumption"
        );
    }
}

impl AsRef<[u8]> for GrowableBuffer {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.buffer[self.start_offset..self.end_offset]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_grows_exponentially() {
        // Ensure that the lexer buffer grows exponentially by doubling each time

        let data = vec![0_u8; 1024];
        let mut reader = &*data;

        let mut buffer = GrowableBuffer::new(16, 1024);

        let mut previous_len = buffer.buffer.len();

        for _ in 0..7 {
            buffer.extend_from_reader(&mut reader).unwrap();

            let double_previous = (previous_len * 2).min(buffer.max_buffer_size);

            assert!(
                buffer.buffer.len() >= double_previous,
                "buffer should at least  double from {previous_len} to {double_previous}"
            );

            previous_len = buffer.buffer.len();

            if previous_len == buffer.max_buffer_size {
                break;
            }
        }
    }
}
