//! A [N-Triples](https://www.w3.org/TR/n-triples/) streaming parser implemented by [`NTriplesParser`]
//! and a serializer implemented by [`NTriplesSerializer`].

use crate::line_formats::NQuadsRecognizer;
use crate::toolkit::{FromReadIterator, ParseError, ParseOrIoError, Parser};
use oxrdf::{Triple, TripleRef};
use std::io::{self, Read, Write};

/// A [N-Triples](https://www.w3.org/TR/n-triples/) streaming parser.
///
/// Support for [N-Triples-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#n-triples-star) is available behind the `rdf-star` feature and the [`NTriplesParser::with_quoted_triples`] option.
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::NTriplesParser;
///
/// let file = b"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/foo> <http://schema.org/name> \"Foo\" .
/// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/bar> <http://schema.org/name> \"Bar\" .";
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for triple in NTriplesParser::new().parse_from_read(file.as_ref()) {
///     let triple = triple?;
///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default)]
pub struct NTriplesParser {
    #[cfg(feature = "rdf-star")]
    with_quoted_triples: bool,
}

impl NTriplesParser {
    /// Builds a new [`NTriplesParser`].
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enables [N-Triples-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#n-triples-star).
    #[cfg(feature = "rdf-star")]
    #[inline]
    #[must_use]
    pub fn with_quoted_triples(mut self) -> Self {
        self.with_quoted_triples = true;
        self
    }

    /// Parses a N-Triples file from a [`Read`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::{NamedNodeRef, vocab::rdf};
    /// use oxttl::NTriplesParser;
    ///
    /// let file = b"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/foo> <http://schema.org/name> \"Foo\" .
    /// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/bar> <http://schema.org/name> \"Bar\" .";
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// for triple in NTriplesParser::new().parse_from_read(file.as_ref()) {
    ///     let triple = triple?;
    ///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn parse_from_read<R: Read>(&self, read: R) -> FromReadNTriplesReader<R> {
        FromReadNTriplesReader {
            inner: self.parse().parser.parse_from_read(read),
        }
    }

    /// Allows to parse a N-Triples file by using a low-level API.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::{NamedNodeRef, vocab::rdf};
    /// use oxttl::NTriplesParser;
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
    /// let mut parser = NTriplesParser::new().parse();
    /// let mut file_chunks = file.iter();
    /// while !parser.is_end() {
    ///     // We feed more data to the parser
    ///     if let Some(chunk) = file_chunks.next() {
    ///         parser.extend_from_slice(chunk);    
    ///     } else {
    ///         parser.end(); // It's finished
    ///     }
    ///     // We read as many triples from the parser as possible
    ///     while let Some(triple) = parser.read_next() {
    ///         let triple = triple?;
    ///         if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
    ///             count += 1;
    ///         }
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[allow(clippy::unused_self)]
    pub fn parse(&self) -> LowLevelNTriplesReader {
        LowLevelNTriplesReader {
            parser: NQuadsRecognizer::new_parser(
                false,
                #[cfg(feature = "rdf-star")]
                self.with_quoted_triples,
            ),
        }
    }
}

/// Parses a N-Triples file from a [`Read`] implementation. Can be built using [`NTriplesParser::parse_from_read`].
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::NTriplesParser;
///
/// let file = b"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/foo> <http://schema.org/name> \"Foo\" .
/// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/bar> <http://schema.org/name> \"Bar\" .";
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for triple in NTriplesParser::new().parse_from_read(file.as_ref()) {
///     let triple = triple?;
///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct FromReadNTriplesReader<R: Read> {
    inner: FromReadIterator<R, NQuadsRecognizer>,
}

impl<R: Read> Iterator for FromReadNTriplesReader<R> {
    type Item = Result<Triple, ParseOrIoError>;

    fn next(&mut self) -> Option<Result<Triple, ParseOrIoError>> {
        Some(self.inner.next()?.map(Into::into))
    }
}

/// Parses a N-Triples file by using a low-level API. Can be built using [`NTriplesParser::parse`].
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::NTriplesParser;
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
/// let mut parser = NTriplesParser::new().parse();
/// let mut file_chunks = file.iter();
/// while !parser.is_end() {
///     // We feed more data to the parser
///     if let Some(chunk) = file_chunks.next() {
///         parser.extend_from_slice(chunk);    
///     } else {
///         parser.end(); // It's finished
///     }
///     // We read as many triples from the parser as possible
///     while let Some(triple) = parser.read_next() {
///         let triple = triple?;
///         if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
///             count += 1;
///         }
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct LowLevelNTriplesReader {
    parser: Parser<NQuadsRecognizer>,
}

impl LowLevelNTriplesReader {
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

    /// Attempt to parse a new triple from the already provided data.
    ///
    /// Returns [`None`] if the parsing is finished or more data is required.
    /// If it is the case more data should be fed using [`extend_from_slice`](Self::extend_from_slice).
    pub fn read_next(&mut self) -> Option<Result<Triple, ParseError>> {
        Some(self.parser.read_next()?.map(Into::into))
    }
}

/// A [N-Triples](https://www.w3.org/TR/n-triples/) serializer.
///
/// Support for [N-Triples-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#n-triples-star) is available behind the `rdf-star` feature.
///
/// ```
/// use oxrdf::{NamedNodeRef, TripleRef};
/// use oxttl::NTriplesSerializer;
///
/// let mut writer = NTriplesSerializer::new().serialize_to_write(Vec::new());
/// writer.write_triple(TripleRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
/// ))?;
/// assert_eq!(
///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
///     writer.finish().as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default)]
pub struct NTriplesSerializer;

impl NTriplesSerializer {
    /// Builds a new [`NTriplesSerializer`].
    #[inline]
    pub fn new() -> Self {
        Self
    }

    /// Writes a N-Triples file to a [`Write`] implementation.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, TripleRef};
    /// use oxttl::NTriplesSerializer;
    ///
    /// let mut writer = NTriplesSerializer::new().serialize_to_write(Vec::new());
    /// writer.write_triple(TripleRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    /// ))?;
    /// assert_eq!(
    ///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
    ///     writer.finish().as_slice()
    /// );
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn serialize_to_write<W: Write>(&self, write: W) -> ToWriteNTriplesWriter<W> {
        ToWriteNTriplesWriter {
            write,
            writer: self.serialize(),
        }
    }

    /// Builds a low-level N-Triples writer.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, TripleRef};
    /// use oxttl::NTriplesSerializer;
    ///
    /// let mut buf = Vec::new();
    /// let mut writer = NTriplesSerializer::new().serialize();
    /// writer.write_triple(TripleRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    /// ), &mut buf)?;
    /// assert_eq!(
    ///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
    ///     buf.as_slice()
    /// );
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[allow(clippy::unused_self)]
    pub fn serialize(&self) -> LowLevelNTriplesWriter {
        LowLevelNTriplesWriter
    }
}

/// Writes a N-Triples file to a [`Write`] implementation. Can be built using [`NTriplesSerializer::serialize_to_write`].
///
/// ```
/// use oxrdf::{NamedNodeRef, TripleRef};
/// use oxttl::NTriplesSerializer;
///
/// let mut writer = NTriplesSerializer::new().serialize_to_write(Vec::new());
/// writer.write_triple(TripleRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
/// ))?;
/// assert_eq!(
///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
///     writer.finish().as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct ToWriteNTriplesWriter<W: Write> {
    write: W,
    writer: LowLevelNTriplesWriter,
}

impl<W: Write> ToWriteNTriplesWriter<W> {
    /// Writes an extra triple.
    pub fn write_triple<'a>(&mut self, t: impl Into<TripleRef<'a>>) -> io::Result<()> {
        self.writer.write_triple(t, &mut self.write)
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub fn finish(self) -> W {
        self.write
    }
}

/// Writes a N-Triples file by using a low-level API. Can be built using [`NTriplesSerializer::serialize`].
///
/// ```
/// use oxrdf::{NamedNodeRef, TripleRef};
/// use oxttl::NTriplesSerializer;
///
/// let mut buf = Vec::new();
/// let mut writer = NTriplesSerializer::new().serialize();
/// writer.write_triple(TripleRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
/// ), &mut buf)?;
/// assert_eq!(
///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
///     buf.as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct LowLevelNTriplesWriter;

impl LowLevelNTriplesWriter {
    /// Writes an extra triple.
    #[allow(clippy::unused_self)]
    pub fn write_triple<'a>(
        &mut self,
        t: impl Into<TripleRef<'a>>,
        mut write: impl Write,
    ) -> io::Result<()> {
        writeln!(write, "{} .", t.into())
    }
}
