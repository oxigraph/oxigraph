//! A [TriG](https://www.w3.org/TR/trig/) streaming parser implemented by [`TriGParser`]
//! and a serializer implemented by [`TriGSerializer`].

use crate::terse::TriGRecognizer;
#[cfg(feature = "async-tokio")]
use crate::toolkit::FromTokioAsyncReadIterator;
use crate::toolkit::{FromReadIterator, ParseError, Parser, SyntaxError};
use oxiri::{Iri, IriParseError};
use oxrdf::{vocab::xsd, GraphName, NamedNode, Quad, QuadRef, Subject, TermRef};
use std::collections::HashMap;
use std::fmt;
use std::io::{self, Read, Write};
#[cfg(feature = "async-tokio")]
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

/// A [TriG](https://www.w3.org/TR/trig/) streaming parser.
///
/// Support for [TriG-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#trig-star) is available behind the `rdf-star` feature and the [`TriGParser::with_quoted_triples`] option.
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::TriGParser;
///
/// let file = b"@base <http://example.com/> .
/// @prefix schema: <http://schema.org/> .
/// <foo> a schema:Person ;
///     schema:name \"Foo\" .
/// <bar> a schema:Person ;
///     schema:name \"Bar\" .";
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for quad in TriGParser::new().parse_read(file.as_ref()) {
///     let quad = quad?;
///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default)]
#[must_use]
pub struct TriGParser {
    unchecked: bool,
    base: Option<Iri<String>>,
    prefixes: HashMap<String, Iri<String>>,
    #[cfg(feature = "rdf-star")]
    with_quoted_triples: bool,
}

impl TriGParser {
    /// Builds a new [`TriGParser`].
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Assumes the file is valid to make parsing faster.
    ///
    /// It will skip some validations.
    ///
    /// Note that if the file is actually not valid, then broken RDF might be emitted by the parser.
    #[inline]
    pub fn unchecked(mut self) -> Self {
        self.unchecked = true;
        self
    }

    #[inline]
    pub fn with_base_iri(mut self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        self.base = Some(Iri::parse(base_iri.into())?);
        Ok(self)
    }

    #[inline]
    pub fn with_prefix(
        mut self,
        prefix_name: impl Into<String>,
        prefix_iri: impl Into<String>,
    ) -> Result<Self, IriParseError> {
        self.prefixes
            .insert(prefix_name.into(), Iri::parse(prefix_iri.into())?);
        Ok(self)
    }

    /// Enables [TriG-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#trig-star).
    #[cfg(feature = "rdf-star")]
    #[inline]
    pub fn with_quoted_triples(mut self) -> Self {
        self.with_quoted_triples = true;
        self
    }

    /// Parses a TriG file from a [`Read`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::{NamedNodeRef, vocab::rdf};
    /// use oxttl::TriGParser;
    ///
    /// let file = b"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name \"Foo\" .
    /// <bar> a schema:Person ;
    ///     schema:name \"Bar\" .";
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// for quad in TriGParser::new().parse_read(file.as_ref()) {
    ///     let quad = quad?;
    ///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn parse_read<R: Read>(self, read: R) -> FromReadTriGReader<R> {
        FromReadTriGReader {
            inner: self.parse().parser.parse_read(read),
        }
    }

    /// Parses a TriG file from a [`AsyncRead`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::{NamedNodeRef, vocab::rdf};
    /// use oxttl::TriGParser;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), oxttl::ParseError> {
    /// let file = b"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name \"Foo\" .
    /// <bar> a schema:Person ;
    ///     schema:name \"Bar\" .";
    ///
    /// let schema_person = NamedNodeRef::new_unchecked("http://schema.org/Person");
    /// let mut count = 0;
    /// let mut parser = TriGParser::new().parse_tokio_async_read(file.as_ref());
    /// while let Some(triple) = parser.next().await {
    ///     let triple = triple?;
    ///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async-tokio")]
    pub fn parse_tokio_async_read<R: AsyncRead + Unpin>(
        self,
        read: R,
    ) -> FromTokioAsyncReadTriGReader<R> {
        FromTokioAsyncReadTriGReader {
            inner: self.parse().parser.parse_tokio_async_read(read),
        }
    }

    /// Allows to parse a TriG file by using a low-level API.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::{NamedNodeRef, vocab::rdf};
    /// use oxttl::TriGParser;
    ///
    /// let file: [&[u8]; 5] = [b"@base <http://example.com/>",
    ///     b". @prefix schema: <http://schema.org/> .",
    ///     b"<foo> a schema:Person",
    ///     b" ; schema:name \"Foo\" . <bar>",
    ///     b" a schema:Person ; schema:name \"Bar\" ."
    /// ];
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// let mut parser = TriGParser::new().parse();
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
    pub fn parse(self) -> LowLevelTriGReader {
        LowLevelTriGReader {
            parser: TriGRecognizer::new_parser(
                true,
                #[cfg(feature = "rdf-star")]
                self.with_quoted_triples,
                self.unchecked,
                self.base,
                self.prefixes,
            ),
        }
    }
}

/// Parses a TriG file from a [`Read`] implementation. Can be built using [`TriGParser::parse_read`].
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::TriGParser;
///
/// let file = b"@base <http://example.com/> .
/// @prefix schema: <http://schema.org/> .
/// <foo> a schema:Person ;
///     schema:name \"Foo\" .
/// <bar> a schema:Person ;
///     schema:name \"Bar\" .";
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for quad in TriGParser::new().parse_read(file.as_ref()) {
///     let quad = quad?;
///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct FromReadTriGReader<R: Read> {
    inner: FromReadIterator<R, TriGRecognizer>,
}

impl<R: Read> FromReadTriGReader<R> {
    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns the mapping from prefix name to prefix value.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TriGParser;
    ///
    /// let file = b"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name \"Foo\" .";
    ///
    /// let mut reader = TriGParser::new().parse_read(file.as_ref());
    /// assert!(reader.prefixes().is_empty()); // No prefix at the beginning
    ///
    /// reader.next().unwrap()?; // We read the first triple
    /// assert_eq!(reader.prefixes()["schema"], "http://schema.org/"); // There are now prefixes
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn prefixes(&self) -> &HashMap<String, Iri<String>> {
        &self.inner.parser.context.prefixes
    }
}

impl<R: Read> Iterator for FromReadTriGReader<R> {
    type Item = Result<Quad, ParseError>;

    fn next(&mut self) -> Option<Result<Quad, ParseError>> {
        self.inner.next()
    }
}

/// Parses a TriG file from a [`AsyncRead`] implementation. Can be built using [`TriGParser::parse_tokio_async_read`].
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::TriGParser;
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), oxttl::ParseError> {
/// let file = b"@base <http://example.com/> .
/// @prefix schema: <http://schema.org/> .
/// <foo> a schema:Person ;
///     schema:name \"Foo\" .
/// <bar> a schema:Person ;
///     schema:name \"Bar\" .";
///
/// let schema_person = NamedNodeRef::new_unchecked("http://schema.org/Person");
/// let mut count = 0;
/// let mut parser = TriGParser::new().parse_tokio_async_read(file.as_ref());
/// while let Some(triple) = parser.next().await {
///     let triple = triple?;
///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async-tokio")]
#[must_use]
pub struct FromTokioAsyncReadTriGReader<R: AsyncRead + Unpin> {
    inner: FromTokioAsyncReadIterator<R, TriGRecognizer>,
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> FromTokioAsyncReadTriGReader<R> {
    /// Reads the next triple or returns `None` if the file is finished.
    pub async fn next(&mut self) -> Option<Result<Quad, ParseError>> {
        Some(self.inner.next().await?.map(Into::into))
    }

    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns the mapping from prefix name to prefix value.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TriGParser;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), oxttl::ParseError> {
    /// let file = b"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name \"Foo\" .";
    ///
    /// let mut reader = TriGParser::new().parse_tokio_async_read(file.as_ref());
    /// assert!(reader.prefixes().is_empty()); // No prefix at the beginning
    ///
    /// reader.next().await.unwrap()?; // We read the first triple
    /// assert_eq!(reader.prefixes()["schema"], "http://schema.org/"); // There are now prefixes
    /// # Ok(())
    /// # }
    /// ```
    pub fn prefixes(&self) -> &HashMap<String, Iri<String>> {
        &self.inner.parser.context.prefixes
    }
}

/// Parses a TriG file by using a low-level API. Can be built using [`TriGParser::parse`].
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::TriGParser;
///
/// let file: [&[u8]; 5] = [b"@base <http://example.com/>",
///     b". @prefix schema: <http://schema.org/> .",
///     b"<foo> a schema:Person",
///     b" ; schema:name \"Foo\" . <bar>",
///     b" a schema:Person ; schema:name \"Bar\" ."
/// ];
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// let mut parser = TriGParser::new().parse();
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
pub struct LowLevelTriGReader {
    parser: Parser<TriGRecognizer>,
}

impl LowLevelTriGReader {
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
    pub fn read_next(&mut self) -> Option<Result<Quad, SyntaxError>> {
        self.parser.read_next()
    }

    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns the mapping from prefix name to prefix value.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TriGParser;
    ///
    /// let file = b"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name \"Foo\" .";
    ///
    /// let mut reader = TriGParser::new().parse();
    /// reader.extend_from_slice(file);
    /// assert!(reader.prefixes().is_empty()); // No prefix at the beginning
    ///
    /// reader.read_next().unwrap()?; // We read the first triple
    /// assert_eq!(reader.prefixes()["schema"], "http://schema.org/"); // There are now prefixes
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn prefixes(&self) -> &HashMap<String, Iri<String>> {
        &self.parser.context.prefixes
    }
}

/// A [TriG](https://www.w3.org/TR/trig/) serializer.
///
/// Support for [TriG-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#trig-star) is available behind the `rdf-star` feature.
///
/// ```
/// use oxrdf::{NamedNodeRef, QuadRef};
/// use oxttl::TriGSerializer;
///
/// let mut writer = TriGSerializer::new().serialize_to_write(Vec::new());
/// writer.write_quad(QuadRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
///     NamedNodeRef::new("http://example.com")?,
/// ))?;
/// assert_eq!(
///     b"<http://example.com> {\n\t<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n}\n",
///     writer.finish()?.as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default)]
#[must_use]
pub struct TriGSerializer;

impl TriGSerializer {
    /// Builds a new [`TriGSerializer`].
    #[inline]
    pub fn new() -> Self {
        Self
    }

    /// Writes a TriG file to a [`Write`] implementation.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, QuadRef};
    /// use oxttl::TriGSerializer;
    ///
    /// let mut writer = TriGSerializer::new().serialize_to_write(Vec::new());
    /// writer.write_quad(QuadRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    ///     NamedNodeRef::new("http://example.com")?,
    /// ))?;
    /// assert_eq!(
    ///     b"<http://example.com> {\n\t<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n}\n",
    ///     writer.finish()?.as_slice()
    /// );
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn serialize_to_write<W: Write>(self, write: W) -> ToWriteTriGWriter<W> {
        ToWriteTriGWriter {
            write,
            writer: self.serialize(),
        }
    }

    /// Writes a TriG file to a [`AsyncWrite`] implementation.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, QuadRef};
    /// use oxttl::TriGSerializer;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> std::io::Result<()> {
    /// let mut writer = TriGSerializer::new().serialize_to_tokio_async_write(Vec::new());
    /// writer.write_quad(QuadRef::new(
    ///     NamedNodeRef::new_unchecked("http://example.com#me"),
    ///     NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
    ///     NamedNodeRef::new_unchecked("http://schema.org/Person"),
    ///     NamedNodeRef::new_unchecked("http://example.com"),
    /// )).await?;
    /// assert_eq!(
    /// b"<http://example.com> {\n\t<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n}\n",
    ///     writer.finish().await?.as_slice()
    /// );
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async-tokio")]
    pub fn serialize_to_tokio_async_write<W: AsyncWrite + Unpin>(
        self,
        write: W,
    ) -> ToTokioAsyncWriteTriGWriter<W> {
        ToTokioAsyncWriteTriGWriter {
            write,
            writer: self.serialize(),
            buffer: Vec::new(),
        }
    }

    /// Builds a low-level TriG writer.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, QuadRef};
    /// use oxttl::TriGSerializer;
    ///
    /// let mut buf = Vec::new();
    /// let mut writer = TriGSerializer::new().serialize();
    /// writer.write_quad(QuadRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    ///     NamedNodeRef::new("http://example.com")?,
    /// ), &mut buf)?;
    /// writer.finish(&mut buf)?;
    /// assert_eq!(
    ///     b"<http://example.com> {\n\t<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n}\n",
    ///     buf.as_slice()
    /// );
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[allow(clippy::unused_self)]
    pub fn serialize(&self) -> LowLevelTriGWriter {
        LowLevelTriGWriter {
            current_graph_name: GraphName::DefaultGraph,
            current_subject_predicate: None,
        }
    }
}

/// Writes a TriG file to a [`Write`] implementation. Can be built using [`TriGSerializer::serialize_to_write`].
///
/// ```
/// use oxrdf::{NamedNodeRef, QuadRef};
/// use oxttl::TriGSerializer;
///
/// let mut writer = TriGSerializer::new().serialize_to_write(Vec::new());
/// writer.write_quad(QuadRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
///     NamedNodeRef::new("http://example.com")?,
/// ))?;
/// assert_eq!(
///     b"<http://example.com> {\n\t<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n}\n",
///     writer.finish()?.as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct ToWriteTriGWriter<W: Write> {
    write: W,
    writer: LowLevelTriGWriter,
}

impl<W: Write> ToWriteTriGWriter<W> {
    /// Writes an extra quad.
    pub fn write_quad<'a>(&mut self, q: impl Into<QuadRef<'a>>) -> io::Result<()> {
        self.writer.write_quad(q, &mut self.write)
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub fn finish(mut self) -> io::Result<W> {
        self.writer.finish(&mut self.write)?;
        Ok(self.write)
    }
}

/// Writes a TriG file to a [`AsyncWrite`] implementation. Can be built using [`TriGSerializer::serialize_to_tokio_async_write`].
///
/// ```
/// use oxrdf::{NamedNodeRef, QuadRef};
/// use oxttl::TriGSerializer;
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> std::io::Result<()> {
/// let mut writer = TriGSerializer::new().serialize_to_tokio_async_write(Vec::new());
/// writer.write_quad(QuadRef::new(
///     NamedNodeRef::new_unchecked("http://example.com#me"),
///     NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
///     NamedNodeRef::new_unchecked("http://schema.org/Person"),
///     NamedNodeRef::new_unchecked("http://example.com"),
/// )).await?;
/// assert_eq!(
/// b"<http://example.com> {\n\t<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n}\n",
///     writer.finish().await?.as_slice()
/// );
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async-tokio")]
#[must_use]
pub struct ToTokioAsyncWriteTriGWriter<W: AsyncWrite + Unpin> {
    write: W,
    writer: LowLevelTriGWriter,
    buffer: Vec<u8>,
}

#[cfg(feature = "async-tokio")]
impl<W: AsyncWrite + Unpin> ToTokioAsyncWriteTriGWriter<W> {
    /// Writes an extra quad.
    pub async fn write_quad<'a>(&mut self, q: impl Into<QuadRef<'a>>) -> io::Result<()> {
        self.writer.write_quad(q, &mut self.buffer)?;
        self.write.write_all(&self.buffer).await?;
        self.buffer.clear();
        Ok(())
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub async fn finish(mut self) -> io::Result<W> {
        self.writer.finish(&mut self.buffer)?;
        self.write.write_all(&self.buffer).await?;
        self.buffer.clear();
        Ok(self.write)
    }
}

/// Writes a TriG file by using a low-level API. Can be built using [`TriGSerializer::serialize`].
///
/// ```
/// use oxrdf::{NamedNodeRef, QuadRef};
/// use oxttl::TriGSerializer;
///
/// let mut buf = Vec::new();
/// let mut writer = TriGSerializer::new().serialize();
/// writer.write_quad(QuadRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
///     NamedNodeRef::new("http://example.com")?,
/// ), &mut buf)?;
/// writer.finish(&mut buf)?;
/// assert_eq!(
///     b"<http://example.com> {\n\t<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n}\n",
///     buf.as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct LowLevelTriGWriter {
    current_graph_name: GraphName,
    current_subject_predicate: Option<(Subject, NamedNode)>,
}

impl LowLevelTriGWriter {
    /// Writes an extra quad.
    pub fn write_quad<'a>(
        &mut self,
        q: impl Into<QuadRef<'a>>,
        mut write: impl Write,
    ) -> io::Result<()> {
        let q = q.into();
        if q.graph_name == self.current_graph_name.as_ref() {
            if let Some((current_subject, current_predicate)) =
                self.current_subject_predicate.take()
            {
                if q.subject == current_subject.as_ref() {
                    if q.predicate == current_predicate {
                        self.current_subject_predicate = Some((current_subject, current_predicate));
                        write!(write, " , {}", TurtleTerm(q.object))
                    } else {
                        self.current_subject_predicate =
                            Some((current_subject, q.predicate.into_owned()));
                        writeln!(write, " ;")?;
                        if !self.current_graph_name.is_default_graph() {
                            write!(write, "\t")?;
                        }
                        write!(write, "\t{} {}", q.predicate, TurtleTerm(q.object))
                    }
                } else {
                    self.current_subject_predicate =
                        Some((q.subject.into_owned(), q.predicate.into_owned()));
                    writeln!(write, " .")?;
                    if !self.current_graph_name.is_default_graph() {
                        write!(write, "\t")?;
                    }
                    write!(
                        write,
                        "{} {} {}",
                        TurtleTerm(q.subject.into()),
                        q.predicate,
                        TurtleTerm(q.object)
                    )
                }
            } else {
                self.current_subject_predicate =
                    Some((q.subject.into_owned(), q.predicate.into_owned()));
                if !self.current_graph_name.is_default_graph() {
                    write!(write, "\t")?;
                }
                write!(
                    write,
                    "{} {} {}",
                    TurtleTerm(q.subject.into()),
                    q.predicate,
                    TurtleTerm(q.object)
                )
            }
        } else {
            if self.current_subject_predicate.is_some() {
                writeln!(write, " .")?;
            }
            if !self.current_graph_name.is_default_graph() {
                writeln!(write, "}}")?;
            }
            self.current_graph_name = q.graph_name.into_owned();
            self.current_subject_predicate =
                Some((q.subject.into_owned(), q.predicate.into_owned()));
            if !self.current_graph_name.is_default_graph() {
                writeln!(write, "{} {{", q.graph_name)?;
                write!(write, "\t")?;
            }
            write!(
                write,
                "{} {} {}",
                TurtleTerm(q.subject.into()),
                q.predicate,
                TurtleTerm(q.object)
            )
        }
    }

    /// Finishes to write the file.
    pub fn finish(&mut self, mut write: impl Write) -> io::Result<()> {
        if self.current_subject_predicate.is_some() {
            writeln!(write, " .")?;
        }
        if !self.current_graph_name.is_default_graph() {
            writeln!(write, "}}")?;
        }
        Ok(())
    }
}

struct TurtleTerm<'a>(TermRef<'a>);

impl<'a> fmt::Display for TurtleTerm<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            TermRef::NamedNode(v) => write!(f, "{v}"),
            TermRef::BlankNode(v) => write!(f, "{v}"),
            TermRef::Literal(v) => {
                let value = v.value();
                let inline = match v.datatype() {
                    xsd::BOOLEAN => is_turtle_boolean(value),
                    xsd::INTEGER => is_turtle_integer(value),
                    xsd::DECIMAL => is_turtle_decimal(value),
                    xsd::DOUBLE => is_turtle_double(value),
                    _ => false,
                };
                if inline {
                    write!(f, "{value}")
                } else {
                    write!(f, "{v}")
                }
            }
            #[cfg(feature = "rdf-star")]
            TermRef::Triple(t) => {
                write!(
                    f,
                    "<< {} {} {} >>",
                    TurtleTerm(t.subject.as_ref().into()),
                    t.predicate,
                    TurtleTerm(t.object.as_ref())
                )
            }
        }
    }
}

fn is_turtle_boolean(value: &str) -> bool {
    matches!(value, "true" | "false")
}

fn is_turtle_integer(value: &str) -> bool {
    // [19] 	INTEGER 	::= 	[+-]? [0-9]+
    let mut value = value.as_bytes();
    if let Some(v) = value.strip_prefix(b"+") {
        value = v;
    } else if let Some(v) = value.strip_prefix(b"-") {
        value = v;
    }
    !value.is_empty() && value.iter().all(u8::is_ascii_digit)
}

fn is_turtle_decimal(value: &str) -> bool {
    // [20] 	DECIMAL 	::= 	[+-]? [0-9]* '.' [0-9]+
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
    // [21] 	DOUBLE 	::= 	[+-]? ([0-9]+ '.' [0-9]* EXPONENT | '.' [0-9]+ EXPONENT | [0-9]+ EXPONENT)
    // [154s] 	EXPONENT 	::= 	[eE] [+-]? [0-9]+
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

#[cfg(test)]
mod tests {
    #![allow(clippy::panic_in_result_fn)]

    use super::*;
    use oxrdf::vocab::xsd;
    use oxrdf::{BlankNodeRef, GraphNameRef, LiteralRef, NamedNodeRef};

    #[test]
    fn test_write() -> io::Result<()> {
        let mut writer = TriGSerializer::new().serialize_to_write(Vec::new());
        writer.write_quad(QuadRef::new(
            NamedNodeRef::new_unchecked("http://example.com/s"),
            NamedNodeRef::new_unchecked("http://example.com/p"),
            NamedNodeRef::new_unchecked("http://example.com/o"),
            NamedNodeRef::new_unchecked("http://example.com/g"),
        ))?;
        writer.write_quad(QuadRef::new(
            NamedNodeRef::new_unchecked("http://example.com/s"),
            NamedNodeRef::new_unchecked("http://example.com/p"),
            LiteralRef::new_simple_literal("foo"),
            NamedNodeRef::new_unchecked("http://example.com/g"),
        ))?;
        writer.write_quad(QuadRef::new(
            NamedNodeRef::new_unchecked("http://example.com/s"),
            NamedNodeRef::new_unchecked("http://example.com/p2"),
            LiteralRef::new_language_tagged_literal_unchecked("foo", "en"),
            NamedNodeRef::new_unchecked("http://example.com/g"),
        ))?;
        writer.write_quad(QuadRef::new(
            BlankNodeRef::new_unchecked("b"),
            NamedNodeRef::new_unchecked("http://example.com/p2"),
            BlankNodeRef::new_unchecked("b2"),
            NamedNodeRef::new_unchecked("http://example.com/g"),
        ))?;
        writer.write_quad(QuadRef::new(
            BlankNodeRef::new_unchecked("b"),
            NamedNodeRef::new_unchecked("http://example.com/p2"),
            LiteralRef::new_typed_literal("true", xsd::BOOLEAN),
            GraphNameRef::DefaultGraph,
        ))?;
        writer.write_quad(QuadRef::new(
            BlankNodeRef::new_unchecked("b"),
            NamedNodeRef::new_unchecked("http://example.com/p2"),
            LiteralRef::new_typed_literal("false", xsd::BOOLEAN),
            NamedNodeRef::new_unchecked("http://example.com/g2"),
        ))?;
        assert_eq!(String::from_utf8(writer.finish()?).unwrap(), "<http://example.com/g> {\n\t<http://example.com/s> <http://example.com/p> <http://example.com/o> , \"foo\" ;\n\t\t<http://example.com/p2> \"foo\"@en .\n\t_:b <http://example.com/p2> _:b2 .\n}\n_:b <http://example.com/p2> true .\n<http://example.com/g2> {\n\t_:b <http://example.com/p2> false .\n}\n");
        Ok(())
    }
}
