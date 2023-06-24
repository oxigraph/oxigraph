//! A [N-Quads](https://www.w3.org/TR/n-quads/) streaming parser implemented by [`NQuadsParser`].

use crate::line_formats::NQuadsRecognizer;
use crate::toolkit::{FromReadIterator, ParseError, ParseOrIoError, Parser};
use oxrdf::{Quad, QuadRef};
use std::io::{self, Read, Write};

/// A [N-Quads](https://www.w3.org/TR/n-quads/) streaming parser.
///
/// Support for [N-Quads-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#n-quads-star) is available behind the `rdf-star` feature and the [`NQuadsParser::with_quoted_triples`] option.
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::NQuadsParser;
///
/// let file = b"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/foo> <http://schema.org/name> \"Foo\" .
/// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/bar> <http://schema.org/name> \"Bar\" .";
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for quad in NQuadsParser::new().parse_from_read(file.as_ref()) {
///     let quad = quad?;
///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default)]
pub struct NQuadsParser {
    #[cfg(feature = "rdf-star")]
    with_quoted_triples: bool,
}

impl NQuadsParser {
    /// Builds a new [`NQuadsParser`].
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enables [N-Quads-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#n-quads-star).
    #[cfg(feature = "rdf-star")]
    #[inline]
    #[must_use]
    pub fn with_quoted_triples(mut self) -> Self {
        self.with_quoted_triples = true;
        self
    }

    /// Parses a N-Quads file from a [`Read`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::{NamedNodeRef, vocab::rdf};
    /// use oxttl::NQuadsParser;
    ///
    /// let file = b"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/foo> <http://schema.org/name> \"Foo\" .
    /// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/bar> <http://schema.org/name> \"Bar\" .";
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// for quad in NQuadsParser::new().parse_from_read(file.as_ref()) {
    ///     let quad = quad?;
    ///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn parse_from_read<R: Read>(&self, read: R) -> FromReadNQuadsReader<R> {
        FromReadNQuadsReader {
            inner: self.parse().parser.parse_from_read(read),
        }
    }

    /// Allows to parse a N-Quads file by using a low-level API.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::{NamedNodeRef, vocab::rdf};
    /// use oxttl::NQuadsParser;
    ///
    /// let file: [&[u8]; 4] = [
    ///     b"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
    ///     b"<http://example.com/foo> <http://schema.org/name> \"Foo\" .\n",
    ///     b"<http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
    ///     b"<http://example.com/bar> <http://schema.org/name> \"Bar\" .\n"
    /// ];
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// let mut parser = NQuadsParser::new().parse();
    /// let mut file_chunks = file.iter();
    /// while !parser.is_end() {
    ///     // We feed more data to the parser
    ///     if let Some(chunk) = file_chunks.next() {
    ///         parser.extend_from_slice(chunk);    
    ///     } else {
    ///         parser.end(); // It's finished
    ///     }
    ///     // We read as many quads from the parser as possible
    ///     while let Some(quad) = parser.read_next() {
    ///         let quad = quad?;
    ///         if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
    ///             count += 1;
    ///         }
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[allow(clippy::unused_self)]
    pub fn parse(&self) -> LowLevelNQuadsReader {
        LowLevelNQuadsReader {
            parser: NQuadsRecognizer::new_parser(
                true,
                #[cfg(feature = "rdf-star")]
                self.with_quoted_triples,
            ),
        }
    }
}

/// Parses a N-Quads file from a [`Read`] implementation. Can be built using [`NQuadsParser::parse_from_read`].
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::NQuadsParser;
///
/// let file = b"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/foo> <http://schema.org/name> \"Foo\" .
/// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/bar> <http://schema.org/name> \"Bar\" .";
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for quad in NQuadsParser::new().parse_from_read(file.as_ref()) {
///     let quad = quad?;
///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct FromReadNQuadsReader<R: Read> {
    inner: FromReadIterator<R, NQuadsRecognizer>,
}

impl<R: Read> Iterator for FromReadNQuadsReader<R> {
    type Item = Result<Quad, ParseOrIoError>;

    fn next(&mut self) -> Option<Result<Quad, ParseOrIoError>> {
        self.inner.next()
    }
}

/// Parses a N-Quads file by using a low-level API. Can be built using [`NQuadsParser::parse`].
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::NQuadsParser;
///
/// let file: [&[u8]; 4] = [
///     b"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
///     b"<http://example.com/foo> <http://schema.org/name> \"Foo\" .\n",
///     b"<http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
///     b"<http://example.com/bar> <http://schema.org/name> \"Bar\" .\n"
/// ];
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// let mut parser = NQuadsParser::new().parse();
/// let mut file_chunks = file.iter();
/// while !parser.is_end() {
///     // We feed more data to the parser
///     if let Some(chunk) = file_chunks.next() {
///         parser.extend_from_slice(chunk);    
///     } else {
///         parser.end(); // It's finished
///     }
///     // We read as many quads from the parser as possible
///     while let Some(quad) = parser.read_next() {
///         let quad = quad?;
///         if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
///             count += 1;
///         }
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct LowLevelNQuadsReader {
    parser: Parser<NQuadsRecognizer>,
}

impl LowLevelNQuadsReader {
    /// Adds some extra bytes to the parser. Should be called when [`read_next`](Self::read_next) returns [`None`] and there is still unread data.
    pub fn extend_from_slice(&mut self, other: &[u8]) {
        self.parser.extend_from_slice(other)
    }

    /// Tell the parser that the file is finished.
    ///
    /// This triggers the parsing of the final bytes and might lead [`read_next`](Self::read_next) to return some extra values.
    pub fn end(&mut self) {
        self.parser.end()
    }

    /// Returns if the parsing is finished i.e. [`end`](Self::end) has been called and [`read_next`](Self::read_next) is always going to return `None`.
    pub fn is_end(&self) -> bool {
        self.parser.is_end()
    }

    /// Attempt to parse a new quad from the already provided data.
    ///
    /// Returns [`None`] if the parsing is finished or more data is required.
    /// If it is the case more data should be fed using [`extend_from_slice`](Self::extend_from_slice).
    pub fn read_next(&mut self) -> Option<Result<Quad, ParseError>> {
        self.parser.read_next()
    }
}

/// A [N-Quads](https://www.w3.org/TR/n-quads/) serializer.
///
/// Support for [N-Quads-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#n-quads-star) is available behind the `rdf-star` feature.
///
/// ```
/// use oxrdf::{NamedNodeRef, QuadRef};
/// use oxttl::NQuadsSerializer;
///
/// let mut writer = NQuadsSerializer::new().serialize_to_write(Vec::new());
/// writer.write_quad(QuadRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
///     NamedNodeRef::new("http://example.com")?,
/// ))?;
/// assert_eq!(
///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> <http://example.com> .\n",
///     writer.finish().as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default)]
pub struct NQuadsSerializer;

impl NQuadsSerializer {
    /// Builds a new [`NQuadsSerializer`].
    #[inline]
    pub fn new() -> Self {
        Self
    }

    /// Writes a N-Quads file to a [`Write`] implementation.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, QuadRef};
    /// use oxttl::NQuadsSerializer;
    ///
    /// let mut writer = NQuadsSerializer::new().serialize_to_write(Vec::new());
    /// writer.write_quad(QuadRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    ///     NamedNodeRef::new("http://example.com")?,
    /// ))?;
    /// assert_eq!(
    ///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> <http://example.com> .\n",
    ///     writer.finish().as_slice()
    /// );
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn serialize_to_write<W: Write>(&self, write: W) -> ToWriteNQuadsWriter<W> {
        ToWriteNQuadsWriter {
            write,
            writer: self.serialize(),
        }
    }

    /// Builds a low-level N-Quads writer.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, QuadRef};
    /// use oxttl::NQuadsSerializer;
    ///
    /// let mut buf = Vec::new();
    /// let mut writer = NQuadsSerializer::new().serialize();
    /// writer.write_quad(QuadRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    ///     NamedNodeRef::new("http://example.com")?,
    /// ), &mut buf)?;
    /// assert_eq!(
    ///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> <http://example.com> .\n",
    ///     buf.as_slice()
    /// );
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[allow(clippy::unused_self)]
    pub fn serialize(&self) -> LowLevelNQuadsWriter {
        LowLevelNQuadsWriter
    }
}

/// Writes a N-Quads file to a [`Write`] implementation. Can be built using [`NQuadsSerializer::serialize_to_write`].
///
/// ```
/// use oxrdf::{NamedNodeRef, QuadRef};
/// use oxttl::NQuadsSerializer;
///
/// let mut writer = NQuadsSerializer::new().serialize_to_write(Vec::new());
/// writer.write_quad(QuadRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
///     NamedNodeRef::new("http://example.com")?,
/// ))?;
/// assert_eq!(
///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> <http://example.com> .\n",
///     writer.finish().as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct ToWriteNQuadsWriter<W: Write> {
    write: W,
    writer: LowLevelNQuadsWriter,
}

impl<W: Write> ToWriteNQuadsWriter<W> {
    /// Writes an extra quad.
    pub fn write_quad<'a>(&mut self, q: impl Into<QuadRef<'a>>) -> io::Result<()> {
        self.writer.write_quad(q, &mut self.write)
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub fn finish(self) -> W {
        self.write
    }
}

/// Writes a N-Quads file by using a low-level API. Can be built using [`NQuadsSerializer::serialize`].
///
/// ```
/// use oxrdf::{NamedNodeRef, QuadRef};
/// use oxttl::NQuadsSerializer;
///
/// let mut buf = Vec::new();
/// let mut writer = NQuadsSerializer::new().serialize();
/// writer.write_quad(QuadRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
///     NamedNodeRef::new("http://example.com")?,
/// ), &mut buf)?;
/// assert_eq!(
///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> <http://example.com> .\n",
///     buf.as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct LowLevelNQuadsWriter;

impl LowLevelNQuadsWriter {
    /// Writes an extra quad.
    #[allow(clippy::unused_self)]
    pub fn write_quad<'a>(
        &mut self,
        q: impl Into<QuadRef<'a>>,
        mut write: impl Write,
    ) -> io::Result<()> {
        writeln!(write, "{} .", q.into())
    }
}
