//! A [Turtle](https://www.w3.org/TR/turtle/) streaming parser implemented by [`TurtleParser`]
//! and a serializer implemented by [`TurtleSerializer`].

use crate::terse::TriGRecognizer;
#[cfg(feature = "async-tokio")]
use crate::toolkit::FromTokioAsyncReadIterator;
use crate::toolkit::{FromReadIterator, ParseError, Parser, SyntaxError};
#[cfg(feature = "async-tokio")]
use crate::trig::ToTokioAsyncWriteTriGWriter;
use crate::trig::{LowLevelTriGWriter, ToWriteTriGWriter, TriGSerializer};
use oxiri::{Iri, IriParseError};
use oxrdf::{GraphNameRef, Triple, TripleRef};
use std::collections::HashMap;
use std::io::{self, Read, Write};
#[cfg(feature = "async-tokio")]
use tokio::io::{AsyncRead, AsyncWrite};

/// A [Turtle](https://www.w3.org/TR/turtle/) streaming parser.
///
/// Support for [Turtle-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#turtle-star) is available behind the `rdf-star` feature and the [`TurtleParser::with_quoted_triples`] option.
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::TurtleParser;
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
/// for triple in TurtleParser::new().parse_read(file.as_ref()) {
///     let triple = triple?;
///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default)]
#[must_use]
pub struct TurtleParser {
    unchecked: bool,
    base: Option<Iri<String>>,
    prefixes: HashMap<String, Iri<String>>,
    #[cfg(feature = "rdf-star")]
    with_quoted_triples: bool,
}

impl TurtleParser {
    /// Builds a new [`TurtleParser`].
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

    /// Enables [Turtle-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#turtle-star).
    #[cfg(feature = "rdf-star")]
    #[inline]
    pub fn with_quoted_triples(mut self) -> Self {
        self.with_quoted_triples = true;
        self
    }

    /// Parses a Turtle file from a [`Read`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::{NamedNodeRef, vocab::rdf};
    /// use oxttl::TurtleParser;
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
    /// for triple in TurtleParser::new().parse_read(file.as_ref()) {
    ///     let triple = triple?;
    ///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn parse_read<R: Read>(self, read: R) -> FromReadTurtleReader<R> {
        FromReadTurtleReader {
            inner: self.parse().parser.parse_read(read),
        }
    }

    /// Parses a Turtle file from a [`AsyncRead`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::{NamedNodeRef, vocab::rdf};
    /// use oxttl::TurtleParser;
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
    /// let mut parser = TurtleParser::new().parse_tokio_async_read(file.as_ref());
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
    ) -> FromTokioAsyncReadTurtleReader<R> {
        FromTokioAsyncReadTurtleReader {
            inner: self.parse().parser.parse_tokio_async_read(read),
        }
    }

    /// Allows to parse a Turtle file by using a low-level API.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::{NamedNodeRef, vocab::rdf};
    /// use oxttl::TurtleParser;
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
    /// let mut parser = TurtleParser::new().parse();
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
    pub fn parse(self) -> LowLevelTurtleReader {
        LowLevelTurtleReader {
            parser: TriGRecognizer::new_parser(
                false,
                #[cfg(feature = "rdf-star")]
                self.with_quoted_triples,
                self.unchecked,
                self.base,
                self.prefixes,
            ),
        }
    }
}

/// Parses a Turtle file from a [`Read`] implementation. Can be built using [`TurtleParser::parse_read`].
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::TurtleParser;
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
/// for triple in TurtleParser::new().parse_read(file.as_ref()) {
///     let triple = triple?;
///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct FromReadTurtleReader<R: Read> {
    inner: FromReadIterator<R, TriGRecognizer>,
}

impl<R: Read> FromReadTurtleReader<R> {
    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns the mapping from prefix name to prefix value.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TurtleParser;
    ///
    /// let file = b"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name \"Foo\" .";
    ///
    /// let mut reader = TurtleParser::new().parse_read(file.as_ref());
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

impl<R: Read> Iterator for FromReadTurtleReader<R> {
    type Item = Result<Triple, ParseError>;

    fn next(&mut self) -> Option<Result<Triple, ParseError>> {
        Some(self.inner.next()?.map(Into::into))
    }
}

/// Parses a Turtle file from a [`AsyncRead`] implementation. Can be built using [`TurtleParser::parse_tokio_async_read`].
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::TurtleParser;
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
/// let mut parser = TurtleParser::new().parse_tokio_async_read(file.as_ref());
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
pub struct FromTokioAsyncReadTurtleReader<R: AsyncRead + Unpin> {
    inner: FromTokioAsyncReadIterator<R, TriGRecognizer>,
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> FromTokioAsyncReadTurtleReader<R> {
    /// Reads the next triple or returns `None` if the file is finished.
    pub async fn next(&mut self) -> Option<Result<Triple, ParseError>> {
        Some(self.inner.next().await?.map(Into::into))
    }

    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns the mapping from prefix name to prefix value.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TurtleParser;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), oxttl::ParseError> {
    /// let file = b"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name \"Foo\" .";
    ///
    /// let mut reader = TurtleParser::new().parse_tokio_async_read(file.as_ref());
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

/// Parses a Turtle file by using a low-level API. Can be built using [`TurtleParser::parse`].
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::TurtleParser;
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
/// let mut parser = TurtleParser::new().parse();
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
pub struct LowLevelTurtleReader {
    parser: Parser<TriGRecognizer>,
}

impl LowLevelTurtleReader {
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
    pub fn read_next(&mut self) -> Option<Result<Triple, SyntaxError>> {
        Some(self.parser.read_next()?.map(Into::into))
    }

    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns the mapping from prefix name to prefix value.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TurtleParser;
    ///
    /// let file = b"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name \"Foo\" .";
    ///
    /// let mut reader = TurtleParser::new().parse();
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

/// A [Turtle](https://www.w3.org/TR/turtle/) serializer.
///
/// Support for [Turtle-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#turtle-star) is available behind the `rdf-star` feature.
///
/// ```
/// use oxrdf::{NamedNodeRef, TripleRef};
/// use oxttl::TurtleSerializer;
///
/// let mut writer = TurtleSerializer::new().serialize_to_write(Vec::new());
/// writer.write_triple(TripleRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
/// ))?;
/// assert_eq!(
///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
///     writer.finish()?.as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default)]
#[must_use]
pub struct TurtleSerializer {
    inner: TriGSerializer,
}

impl TurtleSerializer {
    /// Builds a new [`TurtleSerializer`].
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Writes a Turtle file to a [`Write`] implementation.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, TripleRef};
    /// use oxttl::TurtleSerializer;
    ///
    /// let mut writer = TurtleSerializer::new().serialize_to_write(Vec::new());
    /// writer.write_triple(TripleRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    /// ))?;
    /// assert_eq!(
    ///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
    ///     writer.finish()?.as_slice()
    /// );
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn serialize_to_write<W: Write>(self, write: W) -> ToWriteTurtleWriter<W> {
        ToWriteTurtleWriter {
            inner: self.inner.serialize_to_write(write),
        }
    }

    /// Writes a Turtle file to a [`AsyncWrite`] implementation.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, TripleRef};
    /// use oxttl::TurtleSerializer;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> std::io::Result<()> {
    /// let mut writer = TurtleSerializer::new().serialize_to_tokio_async_write(Vec::new());
    /// writer.write_triple(TripleRef::new(
    ///     NamedNodeRef::new_unchecked("http://example.com#me"),
    ///     NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
    ///     NamedNodeRef::new_unchecked("http://schema.org/Person"),
    /// )).await?;
    /// assert_eq!(
    /// b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
    ///     writer.finish().await?.as_slice()
    /// );
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async-tokio")]
    pub fn serialize_to_tokio_async_write<W: AsyncWrite + Unpin>(
        self,
        write: W,
    ) -> ToTokioAsyncWriteTurtleWriter<W> {
        ToTokioAsyncWriteTurtleWriter {
            inner: self.inner.serialize_to_tokio_async_write(write),
        }
    }

    /// Builds a low-level Turtle writer.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, TripleRef};
    /// use oxttl::TurtleSerializer;
    ///
    /// let mut buf = Vec::new();
    /// let mut writer = TurtleSerializer::new().serialize();
    /// writer.write_triple(TripleRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    /// ), &mut buf)?;
    /// writer.finish(&mut buf)?;
    /// assert_eq!(
    ///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
    ///     buf.as_slice()
    /// );
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn serialize(&self) -> LowLevelTurtleWriter {
        LowLevelTurtleWriter {
            inner: self.inner.serialize(),
        }
    }
}

/// Writes a Turtle file to a [`Write`] implementation. Can be built using [`TurtleSerializer::serialize_to_write`].
///
/// ```
/// use oxrdf::{NamedNodeRef, TripleRef};
/// use oxttl::TurtleSerializer;
///
/// let mut writer = TurtleSerializer::new().serialize_to_write(Vec::new());
/// writer.write_triple(TripleRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
/// ))?;
/// assert_eq!(
///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
///     writer.finish()?.as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct ToWriteTurtleWriter<W: Write> {
    inner: ToWriteTriGWriter<W>,
}

impl<W: Write> ToWriteTurtleWriter<W> {
    /// Writes an extra triple.
    pub fn write_triple<'a>(&mut self, t: impl Into<TripleRef<'a>>) -> io::Result<()> {
        self.inner
            .write_quad(t.into().in_graph(GraphNameRef::DefaultGraph))
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub fn finish(self) -> io::Result<W> {
        self.inner.finish()
    }
}

/// Writes a Turtle file to a [`AsyncWrite`] implementation. Can be built using [`TurtleSerializer::serialize_to_tokio_async_write`].
///
/// ```
/// use oxrdf::{NamedNodeRef, TripleRef};
/// use oxttl::TurtleSerializer;
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> std::io::Result<()> {
/// let mut writer = TurtleSerializer::new().serialize_to_tokio_async_write(Vec::new());
/// writer.write_triple(TripleRef::new(
///     NamedNodeRef::new_unchecked("http://example.com#me"),
///     NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
///     NamedNodeRef::new_unchecked("http://schema.org/Person")
/// )).await?;
/// assert_eq!(
///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
///     writer.finish().await?.as_slice()
/// );
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async-tokio")]
#[must_use]
pub struct ToTokioAsyncWriteTurtleWriter<W: AsyncWrite + Unpin> {
    inner: ToTokioAsyncWriteTriGWriter<W>,
}

#[cfg(feature = "async-tokio")]
impl<W: AsyncWrite + Unpin> ToTokioAsyncWriteTurtleWriter<W> {
    /// Writes an extra triple.
    pub async fn write_triple<'a>(&mut self, t: impl Into<TripleRef<'a>>) -> io::Result<()> {
        self.inner
            .write_quad(t.into().in_graph(GraphNameRef::DefaultGraph))
            .await
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub async fn finish(self) -> io::Result<W> {
        self.inner.finish().await
    }
}

/// Writes a Turtle file by using a low-level API. Can be built using [`TurtleSerializer::serialize`].
///
/// ```
/// use oxrdf::{NamedNodeRef, TripleRef};
/// use oxttl::TurtleSerializer;
///
/// let mut buf = Vec::new();
/// let mut writer = TurtleSerializer::new().serialize();
/// writer.write_triple(TripleRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
/// ), &mut buf)?;
/// writer.finish(&mut buf)?;
/// assert_eq!(
///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
///     buf.as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct LowLevelTurtleWriter {
    inner: LowLevelTriGWriter,
}

impl LowLevelTurtleWriter {
    /// Writes an extra triple.
    pub fn write_triple<'a>(
        &mut self,
        t: impl Into<TripleRef<'a>>,
        write: impl Write,
    ) -> io::Result<()> {
        self.inner
            .write_quad(t.into().in_graph(GraphNameRef::DefaultGraph), write)
    }

    /// Finishes to write the file.
    pub fn finish(&mut self, write: impl Write) -> io::Result<()> {
        self.inner.finish(write)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic_in_result_fn)]

    use super::*;
    use oxrdf::{BlankNodeRef, LiteralRef, NamedNodeRef};

    #[test]
    fn test_write() -> io::Result<()> {
        let mut writer = TurtleSerializer::new().serialize_to_write(Vec::new());
        writer.write_triple(TripleRef::new(
            NamedNodeRef::new_unchecked("http://example.com/s"),
            NamedNodeRef::new_unchecked("http://example.com/p"),
            NamedNodeRef::new_unchecked("http://example.com/o"),
        ))?;
        writer.write_triple(TripleRef::new(
            NamedNodeRef::new_unchecked("http://example.com/s"),
            NamedNodeRef::new_unchecked("http://example.com/p"),
            LiteralRef::new_simple_literal("foo"),
        ))?;
        writer.write_triple(TripleRef::new(
            NamedNodeRef::new_unchecked("http://example.com/s"),
            NamedNodeRef::new_unchecked("http://example.com/p2"),
            LiteralRef::new_language_tagged_literal_unchecked("foo", "en"),
        ))?;
        writer.write_triple(TripleRef::new(
            BlankNodeRef::new_unchecked("b"),
            NamedNodeRef::new_unchecked("http://example.com/p2"),
            BlankNodeRef::new_unchecked("b2"),
        ))?;
        assert_eq!(String::from_utf8(writer.finish()?).unwrap(), "<http://example.com/s> <http://example.com/p> <http://example.com/o> , \"foo\" ;\n\t<http://example.com/p2> \"foo\"@en .\n_:b <http://example.com/p2> _:b2 .\n");
        Ok(())
    }
}
