//! A [N-Quads](https://www.w3.org/TR/n-quads/) streaming parser implemented by [`NQuadsParser`]
//! and a serializer implemented by [`NQuadsSerializer`].

use crate::chunker::get_ntriples_file_chunks;
use crate::line_formats::NQuadsRecognizer;
#[cfg(feature = "async-tokio")]
use crate::toolkit::TokioAsyncReaderIterator;
use crate::toolkit::{Parser, ReaderIterator, SliceIterator, TurtleParseError, TurtleSyntaxError};
use crate::MIN_PARALLEL_CHUNK_SIZE;
use oxrdf::{Quad, QuadRef};
use std::io::{self, Read, Write};
#[cfg(feature = "async-tokio")]
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

/// A [N-Quads](https://www.w3.org/TR/n-quads/) streaming parser.
///
/// Support for [N-Quads-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#n-quads-star) is available behind the `rdf-star` feature and the [`NQuadsParser::with_quoted_triples`] option.
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::NQuadsParser;
///
/// let file = br#"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/foo> <http://schema.org/name> "Foo" .
/// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/bar> <http://schema.org/name> "Bar" ."#;
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for quad in NQuadsParser::new().for_reader(file.as_ref()) {
///     let quad = quad?;
///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default, Clone)]
#[must_use]
pub struct NQuadsParser {
    unchecked: bool,
    #[cfg(feature = "rdf-star")]
    with_quoted_triples: bool,
}

impl NQuadsParser {
    /// Builds a new [`NQuadsParser`].
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Assumes the file is valid to make parsing faster.
    ///
    /// It will skip some validations.
    ///
    /// Note that if the file is actually not valid, broken RDF might be emitted by the parser.
    #[inline]
    pub fn unchecked(mut self) -> Self {
        self.unchecked = true;
        self
    }

    /// Enables [N-Quads-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#n-quads-star).
    #[cfg(feature = "rdf-star")]
    #[inline]
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
    /// let file = br#"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/foo> <http://schema.org/name> "Foo" .
    /// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/bar> <http://schema.org/name> "Bar" ."#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// for quad in NQuadsParser::new().for_reader(file.as_ref()) {
    ///     let quad = quad?;
    ///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_reader<R: Read>(self, reader: R) -> ReaderNQuadsParser<R> {
        ReaderNQuadsParser {
            inner: self.low_level().parser.for_reader(reader),
        }
    }

    /// Parses a N-Quads file from a [`AsyncRead`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::{NamedNodeRef, vocab::rdf};
    /// use oxttl::NQuadsParser;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), oxttl::TurtleParseError> {
    /// let file = br#"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/foo> <http://schema.org/name> "Foo" .
    /// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/bar> <http://schema.org/name> "Bar" ."#;
    ///
    /// let schema_person = NamedNodeRef::new_unchecked("http://schema.org/Person");
    /// let mut count = 0;
    /// let mut parser = NQuadsParser::new().for_tokio_async_reader(file.as_ref());
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
    pub fn for_tokio_async_reader<R: AsyncRead + Unpin>(
        self,
        reader: R,
    ) -> TokioAsyncReaderNQuadsParser<R> {
        TokioAsyncReaderNQuadsParser {
            inner: self.low_level().parser.for_tokio_async_reader(reader),
        }
    }

    /// Parses a N-Quads file from a byte slice.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::{NamedNodeRef, vocab::rdf};
    /// use oxttl::NQuadsParser;
    ///
    /// let file = br#"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/foo> <http://schema.org/name> "Foo" .
    /// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/bar> <http://schema.org/name> "Bar" ."#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// for quad in NQuadsParser::new().for_slice(file) {
    ///     let quad = quad?;
    ///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_slice(self, slice: &[u8]) -> SliceNQuadsParser<'_> {
        SliceNQuadsParser {
            inner: NQuadsRecognizer::new_parser(
                slice,
                true,
                true,
                #[cfg(feature = "rdf-star")]
                self.with_quoted_triples,
                self.unchecked,
            )
            .into_iter(),
        }
    }

    /// Creates a vector of iterators that may be used to parse an NQuads document slice in parallel.
    /// To dynamically specify target_parallelism, use e.g. [`std::thread::available_parallelism`].
    /// Intended to work on large documents.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    /// use oxttl::NQuadsParser;
    /// use rayon::iter::{IntoParallelIterator, ParallelIterator};
    ///
    /// let file = br#"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/foo> <http://schema.org/name> "Foo" .
    /// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/bar> <http://schema.org/name> "Bar" ."#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let readers = NQuadsParser::new().split_slice_for_parallel_parsing(file.as_ref(), 2);
    /// let count = readers
    ///     .into_par_iter()
    ///     .map(|reader| {
    ///         let mut count = 0;
    ///         for quad in reader {
    ///             let quad = quad.unwrap();
    ///             if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
    ///                 count += 1;
    ///             }
    ///         }
    ///         count
    ///     })
    ///     .sum();
    /// assert_eq!(2, count);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn split_slice_for_parallel_parsing<'a>(
        &self,
        slice: &'a [u8],
        target_parallelism: usize,
    ) -> Vec<SliceNQuadsParser<'a>> {
        let n_chunks = (slice.len() / MIN_PARALLEL_CHUNK_SIZE).clamp(1, target_parallelism);
        get_ntriples_file_chunks(slice, n_chunks)
            .into_iter()
            .map(|(start, end)| self.clone().for_slice(&slice[start..end]))
            .collect()
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
    /// let mut parser = NQuadsParser::new().low_level();
    /// let mut file_chunks = file.iter();
    /// while !parser.is_end() {
    ///     // We feed more data to the parser
    ///     if let Some(chunk) = file_chunks.next() {
    ///         parser.extend_from_slice(chunk);    
    ///     } else {
    ///         parser.end(); // It's finished
    ///     }
    ///     // We read as many quads from the parser as possible
    ///     while let Some(quad) = parser.parse_next() {
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
    pub fn low_level(self) -> LowLevelNQuadsParser {
        LowLevelNQuadsParser {
            parser: NQuadsRecognizer::new_parser(
                Vec::new(),
                false,
                true,
                #[cfg(feature = "rdf-star")]
                self.with_quoted_triples,
                self.unchecked,
            ),
        }
    }
}

/// Parses a N-Quads file from a [`Read`] implementation.
///
/// Can be built using [`NQuadsParser::for_reader`].
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::NQuadsParser;
///
/// let file = br#"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/foo> <http://schema.org/name> "Foo" .
/// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/bar> <http://schema.org/name> "Bar" ."#;
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for quad in NQuadsParser::new().for_reader(file.as_ref()) {
///     let quad = quad?;
///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct ReaderNQuadsParser<R: Read> {
    inner: ReaderIterator<R, NQuadsRecognizer>,
}

impl<R: Read> Iterator for ReaderNQuadsParser<R> {
    type Item = Result<Quad, TurtleParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// Parses a N-Quads file from a [`AsyncRead`] implementation.
///
/// Can be built using [`NQuadsParser::for_tokio_async_reader`].
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::NQuadsParser;
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), oxttl::TurtleParseError> {
/// let file = br#"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/foo> <http://schema.org/name> "Foo" .
/// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/bar> <http://schema.org/name> "Bar" ."#;
///
/// let schema_person = NamedNodeRef::new_unchecked("http://schema.org/Person");
/// let mut count = 0;
/// let mut parser = NQuadsParser::new().for_tokio_async_reader(file.as_ref());
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
pub struct TokioAsyncReaderNQuadsParser<R: AsyncRead + Unpin> {
    inner: TokioAsyncReaderIterator<R, NQuadsRecognizer>,
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> TokioAsyncReaderNQuadsParser<R> {
    /// Reads the next triple or returns `None` if the file is finished.
    pub async fn next(&mut self) -> Option<Result<Quad, TurtleParseError>> {
        Some(self.inner.next().await?.map(Into::into))
    }
}

/// Parses a N-Quads file from a byte slice.
///
/// Can be built using [`NQuadsParser::for_slice`].
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::NQuadsParser;
///
/// let file = br#"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/foo> <http://schema.org/name> "Foo" .
/// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/bar> <http://schema.org/name> "Bar" ."#;
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for quad in NQuadsParser::new().for_slice(file) {
///     let quad = quad?;
///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct SliceNQuadsParser<'a> {
    inner: SliceIterator<'a, NQuadsRecognizer>,
}

impl<'a> Iterator for SliceNQuadsParser<'a> {
    type Item = Result<Quad, TurtleSyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// Parses a N-Quads file by using a low-level API.
///
/// Can be built using [`NQuadsParser::low_level`].
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
/// let mut parser = NQuadsParser::new().low_level();
/// let mut file_chunks = file.iter();
/// while !parser.is_end() {
///     // We feed more data to the parser
///     if let Some(chunk) = file_chunks.next() {
///         parser.extend_from_slice(chunk);    
///     } else {
///         parser.end(); // It's finished
///     }
///     // We read as many quads from the parser as possible
///     while let Some(quad) = parser.parse_next() {
///         let quad = quad?;
///         if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
///             count += 1;
///         }
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct LowLevelNQuadsParser {
    parser: Parser<Vec<u8>, NQuadsRecognizer>,
}

impl LowLevelNQuadsParser {
    /// Adds some extra bytes to the parser. Should be called when [`parse_next`](Self::parse_next) returns [`None`] and there is still unread data.
    pub fn extend_from_slice(&mut self, other: &[u8]) {
        self.parser.extend_from_slice(other)
    }

    /// Tell the parser that the file is finished.
    ///
    /// This triggers the parsing of the final bytes and might lead [`parse_next`](Self::parse_next) to return some extra values.
    pub fn end(&mut self) {
        self.parser.end()
    }

    /// Returns if the parsing is finished i.e. [`end`](Self::end) has been called and [`parse_next`](Self::parse_next) is always going to return `None`.
    pub fn is_end(&self) -> bool {
        self.parser.is_end()
    }

    /// Attempt to parse a new quad from the already provided data.
    ///
    /// Returns [`None`] if the parsing is finished or more data is required.
    /// If it is the case more data should be fed using [`extend_from_slice`](Self::extend_from_slice).
    pub fn parse_next(&mut self) -> Option<Result<Quad, TurtleSyntaxError>> {
        self.parser.parse_next()
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
/// let mut serializer = NQuadsSerializer::new().for_writer(Vec::new());
/// serializer.serialize_quad(QuadRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
///     NamedNodeRef::new("http://example.com")?,
/// ))?;
/// assert_eq!(
///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> <http://example.com> .\n",
///     serializer.finish().as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default, Clone)]
#[must_use]
#[allow(clippy::empty_structs_with_brackets)]
pub struct NQuadsSerializer {}

impl NQuadsSerializer {
    /// Builds a new [`NQuadsSerializer`].
    #[inline]
    pub fn new() -> Self {
        Self {}
    }

    /// Writes a N-Quads file to a [`Write`] implementation.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, QuadRef};
    /// use oxttl::NQuadsSerializer;
    ///
    /// let mut serializer = NQuadsSerializer::new().for_writer(Vec::new());
    /// serializer.serialize_quad(QuadRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    ///     NamedNodeRef::new("http://example.com")?,
    /// ))?;
    /// assert_eq!(
    ///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> <http://example.com> .\n",
    ///     serializer.finish().as_slice()
    /// );
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_writer<W: Write>(self, writer: W) -> WriterNQuadsSerializer<W> {
        WriterNQuadsSerializer {
            writer,
            low_level_writer: self.low_level(),
        }
    }

    /// Writes a N-Quads file to a [`AsyncWrite`] implementation.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, QuadRef};
    /// use oxttl::NQuadsSerializer;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> std::io::Result<()> {
    ///     let mut serializer = NQuadsSerializer::new().for_tokio_async_writer(Vec::new());
    ///     serializer.serialize_quad(QuadRef::new(
    ///         NamedNodeRef::new_unchecked("http://example.com#me"),
    ///         NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
    ///         NamedNodeRef::new_unchecked("http://schema.org/Person"),
    ///         NamedNodeRef::new_unchecked("http://example.com"),
    ///     )).await?;
    ///     assert_eq!(
    ///         b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> <http://example.com> .\n",
    ///         serializer.finish().as_slice()
    ///     );
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async-tokio")]
    pub fn for_tokio_async_writer<W: AsyncWrite + Unpin>(
        self,
        writer: W,
    ) -> TokioAsyncWriterNQuadsSerializer<W> {
        TokioAsyncWriterNQuadsSerializer {
            writer,
            low_level_writer: self.low_level(),
            buffer: Vec::new(),
        }
    }

    /// Builds a low-level N-Quads writer.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, QuadRef};
    /// use oxttl::NQuadsSerializer;
    ///
    /// let mut buf = Vec::new();
    /// let mut serializer = NQuadsSerializer::new().low_level();
    /// serializer.serialize_quad(QuadRef::new(
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
    pub fn low_level(self) -> LowLevelNQuadsSerializer {
        LowLevelNQuadsSerializer {}
    }
}

/// Writes a N-Quads file to a [`Write`] implementation.
///
/// Can be built using [`NQuadsSerializer::for_writer`].
///
/// ```
/// use oxrdf::{NamedNodeRef, QuadRef};
/// use oxttl::NQuadsSerializer;
///
/// let mut serializer = NQuadsSerializer::new().for_writer(Vec::new());
/// serializer.serialize_quad(QuadRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
///     NamedNodeRef::new("http://example.com")?,
/// ))?;
/// assert_eq!(
///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> <http://example.com> .\n",
///     serializer.finish().as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct WriterNQuadsSerializer<W: Write> {
    writer: W,
    low_level_writer: LowLevelNQuadsSerializer,
}

impl<W: Write> WriterNQuadsSerializer<W> {
    /// Writes an extra quad.
    pub fn serialize_quad<'a>(&mut self, q: impl Into<QuadRef<'a>>) -> io::Result<()> {
        self.low_level_writer.serialize_quad(q, &mut self.writer)
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub fn finish(self) -> W {
        self.writer
    }
}

/// Writes a N-Quads file to a [`AsyncWrite`] implementation.
///
/// Can be built using [`NQuadsSerializer::for_tokio_async_writer`].
///
/// ```
/// use oxrdf::{NamedNodeRef, QuadRef};
/// use oxttl::NQuadsSerializer;
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> std::io::Result<()> {
///     let mut serializer = NQuadsSerializer::new().for_tokio_async_writer(Vec::new());
///     serializer.serialize_quad(QuadRef::new(
///         NamedNodeRef::new_unchecked("http://example.com#me"),
///         NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
///         NamedNodeRef::new_unchecked("http://schema.org/Person"),
///         NamedNodeRef::new_unchecked("http://example.com"),
///     )).await?;
///     assert_eq!(
///         b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> <http://example.com> .\n",
///         serializer.finish().as_slice()
///     );
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async-tokio")]
#[must_use]
pub struct TokioAsyncWriterNQuadsSerializer<W: AsyncWrite + Unpin> {
    writer: W,
    low_level_writer: LowLevelNQuadsSerializer,
    buffer: Vec<u8>,
}

#[cfg(feature = "async-tokio")]
impl<W: AsyncWrite + Unpin> TokioAsyncWriterNQuadsSerializer<W> {
    /// Writes an extra quad.
    pub async fn serialize_quad<'a>(&mut self, q: impl Into<QuadRef<'a>>) -> io::Result<()> {
        self.low_level_writer.serialize_quad(q, &mut self.buffer)?;
        self.writer.write_all(&self.buffer).await?;
        self.buffer.clear();
        Ok(())
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub fn finish(self) -> W {
        self.writer
    }
}

/// Writes a N-Quads file by using a low-level API.
///
/// Can be built using [`NQuadsSerializer::low_level`].
///
/// ```
/// use oxrdf::{NamedNodeRef, QuadRef};
/// use oxttl::NQuadsSerializer;
///
/// let mut buf = Vec::new();
/// let mut serializer = NQuadsSerializer::new().low_level();
/// serializer.serialize_quad(QuadRef::new(
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
#[allow(clippy::empty_structs_with_brackets)]
pub struct LowLevelNQuadsSerializer {}

impl LowLevelNQuadsSerializer {
    /// Writes an extra quad.
    #[allow(clippy::unused_self)]
    pub fn serialize_quad<'a>(
        &mut self,
        q: impl Into<QuadRef<'a>>,
        mut writer: impl Write,
    ) -> io::Result<()> {
        writeln!(writer, "{} .", q.into())
    }
}
