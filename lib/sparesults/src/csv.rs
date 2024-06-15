//! Implementation of [SPARQL 1.1 Query Results CSV and TSV Formats](https://www.w3.org/TR/sparql11-results-csv-tsv/)

use crate::error::{
    QueryResultsParseError, QueryResultsSyntaxError, SyntaxErrorKind, TextPosition,
};
use memchr::memchr;
use oxrdf::vocab::xsd;
use oxrdf::*;
use std::io::{self, Read, Write};
use std::str::{self, FromStr};
#[cfg(feature = "async-tokio")]
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

const MAX_BUFFER_SIZE: usize = 4096 * 4096;

pub fn write_boolean_csv_result<W: Write>(mut write: W, value: bool) -> io::Result<W> {
    write.write_all(if value { b"true" } else { b"false" })?;
    Ok(write)
}

#[cfg(feature = "async-tokio")]
pub async fn tokio_async_write_boolean_csv_result<W: AsyncWrite + Unpin>(
    mut write: W,
    value: bool,
) -> io::Result<W> {
    write
        .write_all(if value { b"true" } else { b"false" })
        .await?;
    Ok(write)
}

pub struct ToWriteCsvSolutionsWriter<W: Write> {
    inner: InnerCsvSolutionsWriter,
    write: W,
    buffer: String,
}

impl<W: Write> ToWriteCsvSolutionsWriter<W> {
    pub fn start(mut write: W, variables: Vec<Variable>) -> io::Result<Self> {
        let mut buffer = String::new();
        let inner = InnerCsvSolutionsWriter::start(&mut buffer, variables);
        write.write_all(buffer.as_bytes())?;
        buffer.clear();
        Ok(Self {
            inner,
            write,
            buffer,
        })
    }

    pub fn write<'a>(
        &mut self,
        solution: impl IntoIterator<Item = (VariableRef<'a>, TermRef<'a>)>,
    ) -> io::Result<()> {
        self.inner.write(&mut self.buffer, solution);
        self.write.write_all(self.buffer.as_bytes())?;
        self.buffer.clear();
        Ok(())
    }

    pub fn finish(self) -> W {
        self.write
    }
}

#[cfg(feature = "async-tokio")]
pub struct ToTokioAsyncWriteCsvSolutionsWriter<W: AsyncWrite + Unpin> {
    inner: InnerCsvSolutionsWriter,
    write: W,
    buffer: String,
}

#[cfg(feature = "async-tokio")]
impl<W: AsyncWrite + Unpin> ToTokioAsyncWriteCsvSolutionsWriter<W> {
    pub async fn start(mut write: W, variables: Vec<Variable>) -> io::Result<Self> {
        let mut buffer = String::new();
        let inner = InnerCsvSolutionsWriter::start(&mut buffer, variables);
        write.write_all(buffer.as_bytes()).await?;
        buffer.clear();
        Ok(Self {
            inner,
            write,
            buffer,
        })
    }

    pub async fn write<'a>(
        &mut self,
        solution: impl IntoIterator<Item = (VariableRef<'a>, TermRef<'a>)>,
    ) -> io::Result<()> {
        self.inner.write(&mut self.buffer, solution);
        self.write.write_all(self.buffer.as_bytes()).await?;
        self.buffer.clear();
        Ok(())
    }

    pub fn finish(self) -> W {
        self.write
    }
}

struct InnerCsvSolutionsWriter {
    variables: Vec<Variable>,
}

impl InnerCsvSolutionsWriter {
    fn start(output: &mut String, variables: Vec<Variable>) -> Self {
        let mut start_vars = true;
        for variable in &variables {
            if start_vars {
                start_vars = false;
            } else {
                output.push(',');
            }
            output.push_str(variable.as_str());
        }
        output.push_str("\r\n");
        Self { variables }
    }

    fn write<'a>(
        &self,
        output: &mut String,
        solution: impl IntoIterator<Item = (VariableRef<'a>, TermRef<'a>)>,
    ) {
        let mut values = vec![None; self.variables.len()];
        for (variable, value) in solution {
            if let Some(position) = self.variables.iter().position(|v| *v == variable) {
                values[position] = Some(value);
            }
        }
        let mut start_binding = true;
        for value in values {
            if start_binding {
                start_binding = false;
            } else {
                output.push(',');
            }
            if let Some(value) = value {
                write_csv_term(output, value);
            }
        }
        output.push_str("\r\n");
    }
}

fn write_csv_term<'a>(output: &mut String, term: impl Into<TermRef<'a>>) {
    match term.into() {
        TermRef::NamedNode(uri) => output.push_str(uri.as_str()),
        TermRef::BlankNode(bnode) => {
            output.push_str("_:");
            output.push_str(bnode.as_str())
        }
        TermRef::Literal(literal) => write_escaped_csv_string(output, literal.value()),
        #[cfg(feature = "rdf-star")]
        TermRef::Triple(triple) => {
            write_csv_term(output, &triple.subject);
            output.push(' ');
            write_csv_term(output, &triple.predicate);
            output.push(' ');
            write_csv_term(output, &triple.object)
        }
    }
}

fn write_escaped_csv_string(output: &mut String, s: &str) {
    if s.bytes().any(|c| matches!(c, b'"' | b',' | b'\n' | b'\r')) {
        output.push('"');
        for c in s.chars() {
            if c == '"' {
                output.push('"');
                output.push('"');
            } else {
                output.push(c)
            };
        }
        output.push('"');
    } else {
        output.push_str(s)
    }
}

pub struct ToWriteTsvSolutionsWriter<W: Write> {
    inner: InnerTsvSolutionsWriter,
    write: W,
    buffer: String,
}

impl<W: Write> ToWriteTsvSolutionsWriter<W> {
    pub fn start(mut write: W, variables: Vec<Variable>) -> io::Result<Self> {
        let mut buffer = String::new();
        let inner = InnerTsvSolutionsWriter::start(&mut buffer, variables);
        write.write_all(buffer.as_bytes())?;
        buffer.clear();
        Ok(Self {
            inner,
            write,
            buffer,
        })
    }

    pub fn write<'a>(
        &mut self,
        solution: impl IntoIterator<Item = (VariableRef<'a>, TermRef<'a>)>,
    ) -> io::Result<()> {
        self.inner.write(&mut self.buffer, solution);
        self.write.write_all(self.buffer.as_bytes())?;
        self.buffer.clear();
        Ok(())
    }

    pub fn finish(self) -> W {
        self.write
    }
}

#[cfg(feature = "async-tokio")]
pub struct ToTokioAsyncWriteTsvSolutionsWriter<W: AsyncWrite + Unpin> {
    inner: InnerTsvSolutionsWriter,
    write: W,
    buffer: String,
}

#[cfg(feature = "async-tokio")]
impl<W: AsyncWrite + Unpin> ToTokioAsyncWriteTsvSolutionsWriter<W> {
    pub async fn start(mut write: W, variables: Vec<Variable>) -> io::Result<Self> {
        let mut buffer = String::new();
        let inner = InnerTsvSolutionsWriter::start(&mut buffer, variables);
        write.write_all(buffer.as_bytes()).await?;
        buffer.clear();
        Ok(Self {
            inner,
            write,
            buffer,
        })
    }

    pub async fn write<'a>(
        &mut self,
        solution: impl IntoIterator<Item = (VariableRef<'a>, TermRef<'a>)>,
    ) -> io::Result<()> {
        self.inner.write(&mut self.buffer, solution);
        self.write.write_all(self.buffer.as_bytes()).await?;
        self.buffer.clear();
        Ok(())
    }

    pub fn finish(self) -> W {
        self.write
    }
}

struct InnerTsvSolutionsWriter {
    variables: Vec<Variable>,
}

impl InnerTsvSolutionsWriter {
    fn start(output: &mut String, variables: Vec<Variable>) -> Self {
        let mut start_vars = true;
        for variable in &variables {
            if start_vars {
                start_vars = false;
            } else {
                output.push('\t');
            }
            output.push('?');
            output.push_str(variable.as_str());
        }
        output.push('\n');
        Self { variables }
    }

    fn write<'a>(
        &self,
        output: &mut String,
        solution: impl IntoIterator<Item = (VariableRef<'a>, TermRef<'a>)>,
    ) {
        let mut values = vec![None; self.variables.len()];
        for (variable, value) in solution {
            if let Some(position) = self.variables.iter().position(|v| *v == variable) {
                values[position] = Some(value);
            }
        }
        let mut start_binding = true;
        for value in values {
            if start_binding {
                start_binding = false;
            } else {
                output.push('\t');
            }
            if let Some(value) = value {
                write_tsv_term(output, value);
            }
        }
        output.push('\n');
    }
}

fn write_tsv_term<'a>(output: &mut String, term: impl Into<TermRef<'a>>) {
    match term.into() {
        TermRef::NamedNode(node) => {
            output.push('<');
            output.push_str(node.as_str());
            output.push('>');
        }
        TermRef::BlankNode(node) => {
            output.push_str("_:");
            output.push_str(node.as_str());
        }
        TermRef::Literal(literal) => {
            let value = literal.value();
            if let Some(language) = literal.language() {
                write_tsv_quoted_str(output, value);
                output.push('@');
                output.push_str(language);
            } else {
                match literal.datatype() {
                    xsd::BOOLEAN if is_turtle_boolean(value) => output.push_str(value),
                    xsd::INTEGER if is_turtle_integer(value) => output.push_str(value),
                    xsd::DECIMAL if is_turtle_decimal(value) => output.push_str(value),
                    xsd::DOUBLE if is_turtle_double(value) => output.push_str(value),
                    xsd::STRING => write_tsv_quoted_str(output, value),
                    datatype => {
                        write_tsv_quoted_str(output, value);
                        output.push_str("^^");
                        write_tsv_term(output, datatype);
                    }
                }
            }
        }
        #[cfg(feature = "rdf-star")]
        TermRef::Triple(triple) => {
            output.push_str("<< ");
            write_tsv_term(output, &triple.subject);
            output.push(' ');
            write_tsv_term(output, &triple.predicate);
            output.push(' ');
            write_tsv_term(output, &triple.object);
            output.push_str(" >>");
        }
    }
}

fn write_tsv_quoted_str(output: &mut String, string: &str) {
    output.push('"');
    for c in string.chars() {
        match c {
            '\t' => output.push_str("\\t"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            _ => output.push(c),
        };
    }
    output.push('"');
}

fn is_turtle_boolean(value: &str) -> bool {
    matches!(value, "true" | "false")
}

fn is_turtle_integer(value: &str) -> bool {
    // [19]  INTEGER  ::=  [+-]? [0-9]+
    let mut value = value.as_bytes();
    if let Some(v) = value.strip_prefix(b"+") {
        value = v;
    } else if let Some(v) = value.strip_prefix(b"-") {
        value = v;
    }
    !value.is_empty() && value.iter().all(u8::is_ascii_digit)
}

fn is_turtle_decimal(value: &str) -> bool {
    // [20]  DECIMAL  ::=  [+-]? [0-9]* '.' [0-9]+
    let mut value = value.as_bytes();
    if let Some(v) = value.strip_prefix(b"+") {
        value = v;
    } else if let Some(v) = value.strip_prefix(b"-") {
        value = v;
    }
    while value.first().map_or(false, u8::is_ascii_digit) {
        value = &value[1..];
    }
    let Some(value) = value.strip_prefix(b".") else {
        return false;
    };
    !value.is_empty() && value.iter().all(u8::is_ascii_digit)
}

fn is_turtle_double(value: &str) -> bool {
    // [21]    DOUBLE    ::=  [+-]? ([0-9]+ '.' [0-9]* EXPONENT | '.' [0-9]+ EXPONENT | [0-9]+ EXPONENT)
    // [154s]  EXPONENT  ::=  [eE] [+-]? [0-9]+
    let mut value = value.as_bytes();
    if let Some(v) = value.strip_prefix(b"+") {
        value = v;
    } else if let Some(v) = value.strip_prefix(b"-") {
        value = v;
    }
    let mut with_before = false;
    while value.first().map_or(false, u8::is_ascii_digit) {
        value = &value[1..];
        with_before = true;
    }
    let mut with_after = false;
    if let Some(v) = value.strip_prefix(b".") {
        value = v;
        while value.first().map_or(false, u8::is_ascii_digit) {
            value = &value[1..];
            with_after = true;
        }
    }
    if let Some(v) = value.strip_prefix(b"e") {
        value = v;
    } else if let Some(v) = value.strip_prefix(b"E") {
        value = v;
    } else {
        return false;
    }
    if let Some(v) = value.strip_prefix(b"+") {
        value = v;
    } else if let Some(v) = value.strip_prefix(b"-") {
        value = v;
    }
    (with_before || with_after) && !value.is_empty() && value.iter().all(u8::is_ascii_digit)
}

pub enum FromReadTsvQueryResultsReader<R: Read> {
    Solutions {
        variables: Vec<Variable>,
        solutions: FromReadTsvSolutionsReader<R>,
    },
    Boolean(bool),
}

impl<R: Read> FromReadTsvQueryResultsReader<R> {
    pub fn read(mut read: R) -> Result<Self, QueryResultsParseError> {
        let mut reader = LineReader::new();
        let mut buffer = Vec::new();
        let line = reader.next_line_from_read(&mut buffer, &mut read)?;
        Ok(match inner_read_first_line(reader, line)? {
            TsvInnerQueryResults::Solutions {
                variables,
                solutions,
            } => Self::Solutions {
                variables,
                solutions: FromReadTsvSolutionsReader {
                    read,
                    inner: solutions,
                    buffer,
                },
            },
            TsvInnerQueryResults::Boolean(value) => Self::Boolean(value),
        })
    }
}

pub struct FromReadTsvSolutionsReader<R: Read> {
    read: R,
    inner: TsvInnerSolutionsReader,
    buffer: Vec<u8>,
}

impl<R: Read> FromReadTsvSolutionsReader<R> {
    pub fn read_next(&mut self) -> Result<Option<Vec<Option<Term>>>, QueryResultsParseError> {
        let line = self
            .inner
            .reader
            .next_line_from_read(&mut self.buffer, &mut self.read)?;
        Ok(self.inner.read_next(line)?)
    }
}

#[cfg(feature = "async-tokio")]
pub enum FromTokioAsyncReadTsvQueryResultsReader<R: AsyncRead + Unpin> {
    Solutions {
        variables: Vec<Variable>,
        solutions: FromTokioAsyncReadTsvSolutionsReader<R>,
    },
    Boolean(bool),
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> FromTokioAsyncReadTsvQueryResultsReader<R> {
    pub async fn read(mut read: R) -> Result<Self, QueryResultsParseError> {
        let mut reader = LineReader::new();
        let mut buffer = Vec::new();
        let line = reader
            .next_line_from_tokio_async_read(&mut buffer, &mut read)
            .await?;
        Ok(match inner_read_first_line(reader, line)? {
            TsvInnerQueryResults::Solutions {
                variables,
                solutions,
            } => Self::Solutions {
                variables,
                solutions: FromTokioAsyncReadTsvSolutionsReader {
                    read,
                    inner: solutions,
                    buffer,
                },
            },
            TsvInnerQueryResults::Boolean(value) => Self::Boolean(value),
        })
    }
}

#[cfg(feature = "async-tokio")]
pub struct FromTokioAsyncReadTsvSolutionsReader<R: AsyncRead + Unpin> {
    read: R,
    inner: TsvInnerSolutionsReader,
    buffer: Vec<u8>,
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> FromTokioAsyncReadTsvSolutionsReader<R> {
    pub async fn read_next(&mut self) -> Result<Option<Vec<Option<Term>>>, QueryResultsParseError> {
        let line = self
            .inner
            .reader
            .next_line_from_tokio_async_read(&mut self.buffer, &mut self.read)
            .await?;
        Ok(self.inner.read_next(line)?)
    }
}

pub enum FromSliceTsvQueryResultsReader<'a> {
    Solutions {
        variables: Vec<Variable>,
        solutions: FromSliceTsvSolutionsReader<'a>,
    },
    Boolean(bool),
}

impl<'a> FromSliceTsvQueryResultsReader<'a> {
    pub fn read(slice: &'a [u8]) -> Result<Self, QueryResultsSyntaxError> {
        let mut reader = LineReader::new();
        let line = reader.next_line_from_slice(slice)?;
        Ok(match inner_read_first_line(reader, line)? {
            TsvInnerQueryResults::Solutions {
                variables,
                solutions,
            } => Self::Solutions {
                variables,
                solutions: FromSliceTsvSolutionsReader {
                    slice,
                    inner: solutions,
                },
            },
            TsvInnerQueryResults::Boolean(value) => Self::Boolean(value),
        })
    }
}

pub struct FromSliceTsvSolutionsReader<'a> {
    slice: &'a [u8],
    inner: TsvInnerSolutionsReader,
}

impl<'a> FromSliceTsvSolutionsReader<'a> {
    pub fn read_next(&mut self) -> Result<Option<Vec<Option<Term>>>, QueryResultsSyntaxError> {
        let line = self.inner.reader.next_line_from_slice(self.slice)?;
        self.inner.read_next(line)
    }
}

enum TsvInnerQueryResults {
    Solutions {
        variables: Vec<Variable>,
        solutions: TsvInnerSolutionsReader,
    },
    Boolean(bool),
}

fn inner_read_first_line(
    reader: LineReader,
    line: &str,
) -> Result<TsvInnerQueryResults, QueryResultsSyntaxError> {
    let line = line.trim_matches(|c| matches!(c, ' ' | '\r' | '\n'));
    if line.eq_ignore_ascii_case("true") {
        return Ok(TsvInnerQueryResults::Boolean(true));
    }
    if line.eq_ignore_ascii_case("false") {
        return Ok(TsvInnerQueryResults::Boolean(false));
    }
    let mut variables = Vec::new();
    if !line.is_empty() {
        for v in line.split('\t') {
            let v = v.trim();
            if v.is_empty() {
                return Err(QueryResultsSyntaxError::msg("Empty column on the first row. The first row should be a list of variables like ?foo or $bar"));
            }
            let variable = Variable::from_str(v).map_err(|e| {
                QueryResultsSyntaxError::msg(format!("Invalid variable declaration '{v}': {e}"))
            })?;
            if variables.contains(&variable) {
                return Err(QueryResultsSyntaxError::msg(format!(
                    "The variable {variable} is declared twice"
                )));
            }
            variables.push(variable);
        }
    }
    let column_len = variables.len();
    Ok(TsvInnerQueryResults::Solutions {
        variables,
        solutions: TsvInnerSolutionsReader { reader, column_len },
    })
}

struct TsvInnerSolutionsReader {
    reader: LineReader,
    column_len: usize,
}

impl TsvInnerSolutionsReader {
    #[allow(clippy::unwrap_in_result)]
    pub fn read_next(
        &self,
        line: &str,
    ) -> Result<Option<Vec<Option<Term>>>, QueryResultsSyntaxError> {
        if line.is_empty() {
            return Ok(None); // EOF
        }
        let elements = line
            .split('\t')
            .enumerate()
            .map(|(i, v)| {
                let v = v.trim();
                if v.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(Term::from_str(v).map_err(|e| {
                        let start_position_char = line
                            .split('\t')
                            .take(i)
                            .map(|c| c.chars().count() + 1)
                            .sum::<usize>();
                        let start_position_bytes =
                            line.split('\t').take(i).map(|c| c.len() + 1).sum::<usize>();
                        QueryResultsSyntaxError(SyntaxErrorKind::Term {
                            error: e,
                            term: v.into(),
                            location: TextPosition {
                                line: self.reader.line_count - 1,
                                column: start_position_char.try_into().unwrap(),
                                offset: self.reader.last_line_start
                                    + u64::try_from(start_position_bytes).unwrap(),
                            }..TextPosition {
                                line: self.reader.line_count - 1,
                                column: (start_position_char + v.chars().count())
                                    .try_into()
                                    .unwrap(),
                                offset: self.reader.last_line_start
                                    + u64::try_from(start_position_bytes + v.len()).unwrap(),
                            },
                        })
                    })?))
                }
            })
            .collect::<Result<Vec<_>, QueryResultsSyntaxError>>()?;
        if elements.len() == self.column_len {
            Ok(Some(elements))
        } else if self.column_len == 0 && elements == [None] {
            Ok(Some(Vec::new())) // Zero columns case
        } else {
            Err(QueryResultsSyntaxError::located_message(
                format!(
                    "This TSV files has {} columns but we found a row on line {} with {} columns: {}",
                    self.column_len,
                    self.reader.line_count - 1,
                    elements.len(),
                    line
                ),
                TextPosition {
                    line: self.reader.line_count - 1,
                    column: 0,
                    offset: self.reader.last_line_start,
                }..TextPosition {
                    line: self.reader.line_count - 1,
                    column: line.chars().count().try_into().unwrap(),
                    offset: self.reader.last_line_end,
                },
            ))
        }
    }
}

struct LineReader {
    buffer_start: usize,
    buffer_end: usize,
    line_count: u64,
    last_line_start: u64,
    last_line_end: u64,
}

impl LineReader {
    fn new() -> Self {
        Self {
            buffer_start: 0,
            buffer_end: 0,
            line_count: 0,
            last_line_start: 0,
            last_line_end: 0,
        }
    }

    #[allow(clippy::unwrap_in_result)]
    fn next_line_from_read<'a>(
        &mut self,
        buffer: &'a mut Vec<u8>,
        read: &mut impl Read,
    ) -> Result<&'a str, QueryResultsParseError> {
        let line_end = loop {
            if let Some(eol) = memchr(b'\n', &buffer[self.buffer_start..self.buffer_end]) {
                break self.buffer_start + eol + 1;
            }
            if self.buffer_start > 0 {
                buffer.copy_within(self.buffer_start..self.buffer_end, 0);
                self.buffer_end -= self.buffer_start;
                self.buffer_start = 0;
            }
            if self.buffer_end + 1024 > buffer.len() {
                if self.buffer_end + 1024 > MAX_BUFFER_SIZE {
                    return Err(io::Error::new(
                        io::ErrorKind::OutOfMemory,
                        format!("Reached the buffer maximal size of {MAX_BUFFER_SIZE}"),
                    )
                    .into());
                }
                buffer.resize(self.buffer_end + 1024, b'\0');
            }
            let read = read.read(&mut buffer[self.buffer_end..])?;
            if read == 0 {
                break self.buffer_end;
            }
            self.buffer_end += read;
        };
        let result = str::from_utf8(&buffer[self.buffer_start..line_end]).map_err(|e| {
            QueryResultsSyntaxError::msg(format!("Invalid UTF-8 in the TSV file: {e}")).into()
        });
        self.line_count += 1;
        self.last_line_start = self.last_line_end;
        self.last_line_end += u64::try_from(line_end - self.buffer_start).unwrap();
        self.buffer_start = line_end;
        result
    }

    #[cfg(feature = "async-tokio")]
    #[allow(clippy::unwrap_in_result)]
    async fn next_line_from_tokio_async_read<'a>(
        &mut self,
        buffer: &'a mut Vec<u8>,
        read: &mut (impl AsyncRead + Unpin),
    ) -> Result<&'a str, QueryResultsParseError> {
        let line_end = loop {
            if let Some(eol) = memchr(b'\n', &buffer[self.buffer_start..self.buffer_end]) {
                break self.buffer_start + eol + 1;
            }
            if self.buffer_start > 0 {
                buffer.copy_within(self.buffer_start..self.buffer_end, 0);
                self.buffer_end -= self.buffer_start;
                self.buffer_start = 0;
            }
            if self.buffer_end + 1024 > buffer.len() {
                if self.buffer_end + 1024 > MAX_BUFFER_SIZE {
                    return Err(io::Error::new(
                        io::ErrorKind::OutOfMemory,
                        format!("Reached the buffer maximal size of {MAX_BUFFER_SIZE}"),
                    )
                    .into());
                }
                buffer.resize(self.buffer_end + 1024, b'\0');
            }
            let read = read.read(&mut buffer[self.buffer_end..]).await?;
            if read == 0 {
                break self.buffer_end;
            }
            self.buffer_end += read;
        };
        let result = str::from_utf8(&buffer[self.buffer_start..line_end]).map_err(|e| {
            QueryResultsSyntaxError::msg(format!("Invalid UTF-8 in the TSV file: {e}")).into()
        });
        self.line_count += 1;
        self.last_line_start = self.last_line_end;
        self.last_line_end += u64::try_from(line_end - self.buffer_start).unwrap();
        self.buffer_start = line_end;
        result
    }

    #[allow(clippy::unwrap_in_result)]
    fn next_line_from_slice<'a>(
        &mut self,
        slice: &'a [u8],
    ) -> Result<&'a str, QueryResultsSyntaxError> {
        let line_end = memchr(b'\n', &slice[self.buffer_start..])
            .map_or_else(|| slice.len(), |eol| self.buffer_start + eol + 1);
        let result = str::from_utf8(&slice[self.buffer_start..line_end]).map_err(|e| {
            QueryResultsSyntaxError::msg(format!("Invalid UTF-8 in the TSV file: {e}"))
        });
        self.line_count += 1;
        self.last_line_start = self.last_line_end;
        self.last_line_end += u64::try_from(line_end - self.buffer_start).unwrap();
        self.buffer_start = line_end;
        result
    }
}

#[cfg(test)]
#[allow(clippy::panic_in_result_fn)]
mod tests {
    use super::*;
    use std::error::Error;

    fn build_example() -> (Vec<Variable>, Vec<Vec<Option<Term>>>) {
        (
            vec![
                Variable::new_unchecked("x"),
                Variable::new_unchecked("literal"),
            ],
            vec![
                vec![
                    Some(NamedNode::new_unchecked("http://example/x").into()),
                    Some(Literal::new_simple_literal("String").into()),
                ],
                vec![
                    Some(NamedNode::new_unchecked("http://example/x").into()),
                    Some(Literal::new_simple_literal("String-with-dquote\"").into()),
                ],
                vec![
                    Some(BlankNode::new_unchecked("b0").into()),
                    Some(Literal::new_simple_literal("Blank node").into()),
                ],
                vec![
                    None,
                    Some(Literal::new_simple_literal("Missing 'x'").into()),
                ],
                vec![None, None],
                vec![
                    Some(NamedNode::new_unchecked("http://example/x").into()),
                    None,
                ],
                vec![
                    Some(BlankNode::new_unchecked("b1").into()),
                    Some(
                        Literal::new_language_tagged_literal_unchecked("String-with-lang", "en")
                            .into(),
                    ),
                ],
                vec![
                    Some(BlankNode::new_unchecked("b1").into()),
                    Some(Literal::new_typed_literal("123", xsd::INTEGER).into()),
                ],
                vec![
                    None,
                    Some(Literal::new_simple_literal("escape,\t\r\n").into()),
                ],
            ],
        )
    }

    #[test]
    fn test_csv_serialization() {
        let (variables, solutions) = build_example();
        let mut buffer = String::new();
        let writer = InnerCsvSolutionsWriter::start(&mut buffer, variables.clone());
        for solution in solutions {
            writer.write(
                &mut buffer,
                variables
                    .iter()
                    .zip(&solution)
                    .filter_map(|(v, s)| s.as_ref().map(|s| (v.as_ref(), s.as_ref()))),
            );
        }
        assert_eq!(buffer, "x,literal\r\nhttp://example/x,String\r\nhttp://example/x,\"String-with-dquote\"\"\"\r\n_:b0,Blank node\r\n,Missing 'x'\r\n,\r\nhttp://example/x,\r\n_:b1,String-with-lang\r\n_:b1,123\r\n,\"escape,\t\r\n\"\r\n");
    }

    #[test]
    fn test_tsv_roundtrip() -> Result<(), Box<dyn Error>> {
        let (variables, solutions) = build_example();

        // Write
        let mut buffer = String::new();
        let writer = InnerTsvSolutionsWriter::start(&mut buffer, variables.clone());
        for solution in &solutions {
            writer.write(
                &mut buffer,
                variables
                    .iter()
                    .zip(solution)
                    .filter_map(|(v, s)| s.as_ref().map(|s| (v.as_ref(), s.as_ref()))),
            );
        }
        assert_eq!(buffer, "?x\t?literal\n<http://example/x>\t\"String\"\n<http://example/x>\t\"String-with-dquote\\\"\"\n_:b0\t\"Blank node\"\n\t\"Missing 'x'\"\n\t\n<http://example/x>\t\n_:b1\t\"String-with-lang\"@en\n_:b1\t123\n\t\"escape,\\t\\r\\n\"\n");

        // Read
        if let FromSliceTsvQueryResultsReader::Solutions {
            solutions: mut solutions_iter,
            variables: actual_variables,
        } = FromSliceTsvQueryResultsReader::read(buffer.as_bytes())?
        {
            assert_eq!(actual_variables.as_slice(), variables.as_slice());
            let mut rows = Vec::new();
            while let Some(row) = solutions_iter.read_next()? {
                rows.push(row);
            }
            assert_eq!(rows, solutions);
        } else {
            unreachable!()
        }

        Ok(())
    }

    #[test]
    fn test_bad_tsv() {
        let mut bad_tsvs = vec![
            "?",
            "?p",
            "?p?o",
            "?p\n<",
            "?p\n_",
            "?p\n_:",
            "?p\n\"",
            "?p\n<<",
            "?p\n1\t2\n",
            "?p\n\n",
        ];
        let a_lot_of_strings = format!("?p\n{}\n", "<".repeat(100_000));
        bad_tsvs.push(&a_lot_of_strings);
        for bad_tsv in bad_tsvs {
            if let Ok(FromReadTsvQueryResultsReader::Solutions { mut solutions, .. }) =
                FromReadTsvQueryResultsReader::read(bad_tsv.as_bytes())
            {
                while let Ok(Some(_)) = solutions.read_next() {}
            }
        }
    }

    #[test]
    fn test_no_columns_csv_serialization() {
        let mut buffer = String::new();
        let writer = InnerCsvSolutionsWriter::start(&mut buffer, Vec::new());
        writer.write(&mut buffer, []);
        assert_eq!(buffer, "\r\n\r\n");
    }

    #[test]
    fn test_no_columns_tsv_serialization() {
        let mut buffer = String::new();
        let writer = InnerTsvSolutionsWriter::start(&mut buffer, Vec::new());
        writer.write(&mut buffer, []);
        assert_eq!(buffer, "\n\n");
    }

    #[test]
    fn test_no_columns_tsv_parsing() -> io::Result<()> {
        if let FromReadTsvQueryResultsReader::Solutions {
            mut solutions,
            variables,
        } = FromReadTsvQueryResultsReader::read(b"\n\n".as_slice())?
        {
            assert_eq!(variables, Vec::<Variable>::new());
            assert_eq!(solutions.read_next()?, Some(Vec::new()));
            assert_eq!(solutions.read_next()?, None);
        } else {
            unreachable!()
        }
        Ok(())
    }

    #[test]
    fn test_no_results_csv_serialization() {
        let mut buffer = String::new();
        InnerCsvSolutionsWriter::start(&mut buffer, vec![Variable::new_unchecked("a")]);
        assert_eq!(buffer, "a\r\n");
    }

    #[test]
    fn test_no_results_tsv_serialization() {
        let mut buffer = String::new();
        InnerTsvSolutionsWriter::start(&mut buffer, vec![Variable::new_unchecked("a")]);
        assert_eq!(buffer, "?a\n");
    }

    #[test]
    fn test_no_results_tsv_parsing() -> io::Result<()> {
        if let FromReadTsvQueryResultsReader::Solutions {
            mut solutions,
            variables,
        } = FromReadTsvQueryResultsReader::read(b"?a\n".as_slice())?
        {
            assert_eq!(variables, vec![Variable::new_unchecked("a")]);
            assert_eq!(solutions.read_next()?, None);
        } else {
            unreachable!()
        }
        Ok(())
    }
}
