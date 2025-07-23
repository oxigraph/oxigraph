//! A [N-Triples](https://www.w3.org/TR/n-triples/) streaming parser implemented by [`NTriplesParser`]
//! and a serializer implemented by [`NTriplesSerializer`].

use crate::MIN_PARALLEL_CHUNK_SIZE;
use crate::chunker::get_ntriples_file_chunks;
use crate::line_formats::NQuadsRecognizer;
#[cfg(feature = "async-tokio")]
use crate::toolkit::TokioAsyncReaderIterator;
use crate::toolkit::{Parser, ReaderIterator, SliceIterator, TurtleParseError, TurtleSyntaxError};
use oxrdf::{Triple, TripleRef};
use std::io::{self, Read, Write};
#[cfg(feature = "async-tokio")]
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

/// A [N-Triples](https://www.w3.org/TR/n-triples/) streaming parser.
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::NTriplesParser;
///
/// let file = r#"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/foo> <http://schema.org/name> "Foo" .
/// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/bar> <http://schema.org/name> "Bar" ."#;
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for triple in NTriplesParser::new().for_reader(file.as_bytes()) {
///     let triple = triple?;
///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default, Clone)]
#[must_use]
pub struct NTriplesParser {
    lenient: bool,
}

impl NTriplesParser {
    /// Builds a new [`NTriplesParser`].
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Assumes the file is valid to make parsing faster.
    ///
    /// It will skip some validations.
    ///
    /// Note that if the file is actually not valid, the parser might emit broken RDF.    ///
    #[inline]
    pub fn lenient(mut self) -> Self {
        self.lenient = true;
        self
    }

    #[deprecated(note = "Use `lenient()` instead", since = "0.2.0")]
    #[inline]
    pub fn unchecked(self) -> Self {
        self.lenient()
    }

    /// Parses a N-Triples file from a [`Read`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::{NamedNodeRef, vocab::rdf};
    /// use oxttl::NTriplesParser;
    ///
    /// let file = r#"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/foo> <http://schema.org/name> "Foo" .
    /// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/bar> <http://schema.org/name> "Bar" ."#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// for triple in NTriplesParser::new().for_reader(file.as_bytes()) {
    ///     let triple = triple?;
    ///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_reader<R: Read>(self, reader: R) -> ReaderNTriplesParser<R> {
        ReaderNTriplesParser {
            inner: self.low_level().parser.for_reader(reader),
        }
    }

    /// Parses a N-Triples file from a [`AsyncRead`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use oxrdf::{NamedNodeRef, vocab::rdf};
    /// use oxttl::NTriplesParser;
    ///
    /// let file = r#"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/foo> <http://schema.org/name> "Foo" .
    /// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/bar> <http://schema.org/name> "Bar" ."#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// let mut parser = NTriplesParser::new().for_tokio_async_reader(file.as_bytes());
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
    ) -> TokioAsyncReaderNTriplesParser<R> {
        TokioAsyncReaderNTriplesParser {
            inner: self.low_level().parser.for_tokio_async_reader(reader),
        }
    }

    /// Parses a N-Triples file from a byte slice.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::{NamedNodeRef, vocab::rdf};
    /// use oxttl::NTriplesParser;
    ///
    /// let file = r#"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/foo> <http://schema.org/name> "Foo" .
    /// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/bar> <http://schema.org/name> "Bar" ."#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// for triple in NTriplesParser::new().for_slice(file) {
    ///     let triple = triple?;
    ///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_slice(self, slice: &(impl AsRef<[u8]> + ?Sized)) -> SliceNTriplesParser<'_> {
        SliceNTriplesParser {
            inner: NQuadsRecognizer::new_parser(slice.as_ref(), true, false, self.lenient)
                .into_iter(),
        }
    }

    /// Creates a vector of parsers that may be used to parse an NTriples document slice in parallel.
    /// To dynamically specify target_parallelism, use e.g. [`std::thread::available_parallelism`].
    /// Intended to work on large documents.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    /// use oxttl::{NTriplesParser};
    /// use rayon::iter::{IntoParallelIterator, ParallelIterator};
    ///
    /// let file = r#"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/foo> <http://schema.org/name> "Foo" .
    /// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
    /// <http://example.com/bar> <http://schema.org/name> "Bar" ."#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let readers = NTriplesParser::new().split_slice_for_parallel_parsing(file, 2);
    /// let count = readers
    ///     .into_par_iter()
    ///     .map(|reader| {
    ///         let mut count = 0;
    ///         for triple in reader {
    ///             let triple = triple.unwrap();
    ///             if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
    ///                 count += 1;
    ///             }
    ///         }
    ///         count
    ///     })
    ///     .sum();
    /// assert_eq!(2, count);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn split_slice_for_parallel_parsing(
        self,
        slice: &(impl AsRef<[u8]> + ?Sized),
        target_parallelism: usize,
    ) -> Vec<SliceNTriplesParser<'_>> {
        let slice = slice.as_ref();
        let n_chunks = (slice.len() / MIN_PARALLEL_CHUNK_SIZE).clamp(1, target_parallelism);
        get_ntriples_file_chunks(slice, n_chunks)
            .into_iter()
            .map(|(start, end)| self.clone().for_slice(&slice[start..end]))
            .collect()
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
    /// let mut parser = NTriplesParser::new().low_level();
    /// let mut file_chunks = file.iter();
    /// while !parser.is_end() {
    ///     // We feed more data to the parser
    ///     if let Some(chunk) = file_chunks.next() {
    ///         parser.extend_from_slice(chunk);    
    ///     } else {
    ///         parser.end(); // It's finished
    ///     }
    ///     // We read as many triples from the parser as possible
    ///     while let Some(triple) = parser.parse_next() {
    ///         let triple = triple?;
    ///         if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
    ///             count += 1;
    ///         }
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn low_level(self) -> LowLevelNTriplesParser {
        LowLevelNTriplesParser {
            parser: NQuadsRecognizer::new_parser(Vec::new(), false, false, self.lenient),
        }
    }
}

/// Parses a N-Triples file from a [`Read`] implementation.
///
/// Can be built using [`NTriplesParser::for_reader`].
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::NTriplesParser;
///
/// let file = r#"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/foo> <http://schema.org/name> "Foo" .
/// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/bar> <http://schema.org/name> "Bar" ."#;
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for triple in NTriplesParser::new().for_reader(file.as_bytes()) {
///     let triple = triple?;
///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct ReaderNTriplesParser<R: Read> {
    inner: ReaderIterator<R, NQuadsRecognizer>,
}

impl<R: Read> Iterator for ReaderNTriplesParser<R> {
    type Item = Result<Triple, TurtleParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.inner.next()?.map(Into::into))
    }
}

/// Parses a N-Triples file from a [`AsyncRead`] implementation.
///
/// Can be built using [`NTriplesParser::for_tokio_async_reader`].
///
/// Count the number of people:
/// ```
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::NTriplesParser;
///
/// let file = r#"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/foo> <http://schema.org/name> "Foo" .
/// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/bar> <http://schema.org/name> "Bar" ."#;
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// let mut parser = NTriplesParser::new().for_tokio_async_reader(file.as_bytes());
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
pub struct TokioAsyncReaderNTriplesParser<R: AsyncRead + Unpin> {
    inner: TokioAsyncReaderIterator<R, NQuadsRecognizer>,
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> TokioAsyncReaderNTriplesParser<R> {
    /// Reads the next triple or returns `None` if the file is finished.
    pub async fn next(&mut self) -> Option<Result<Triple, TurtleParseError>> {
        Some(self.inner.next().await?.map(Into::into))
    }
}

/// Parses an N-Triples file from a byte slice.
///
/// Can be built using [`NTriplesParser::for_slice`].
///
/// Count the number of people:
/// ```
/// use oxrdf::{NamedNodeRef, vocab::rdf};
/// use oxttl::NTriplesParser;
///
/// let file = r#"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/foo> <http://schema.org/name> "Foo" .
/// <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
/// <http://example.com/bar> <http://schema.org/name> "Bar" ."#;
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for triple in NTriplesParser::new().for_slice(file) {
///     let triple = triple?;
///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct SliceNTriplesParser<'a> {
    inner: SliceIterator<'a, NQuadsRecognizer>,
}

impl Iterator for SliceNTriplesParser<'_> {
    type Item = Result<Triple, TurtleSyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.inner.next()?.map(Into::into))
    }
}

/// Parses a N-Triples file by using a low-level API.
///
/// Can be built using [`NTriplesParser::low_level`].
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
/// let mut parser = NTriplesParser::new().low_level();
/// let mut file_chunks = file.iter();
/// while !parser.is_end() {
///     // We feed more data to the parser
///     if let Some(chunk) = file_chunks.next() {
///         parser.extend_from_slice(chunk);    
///     } else {
///         parser.end(); // It's finished
///     }
///     // We read as many triples from the parser as possible
///     while let Some(triple) = parser.parse_next() {
///         let triple = triple?;
///         if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
///             count += 1;
///         }
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct LowLevelNTriplesParser {
    parser: Parser<Vec<u8>, NQuadsRecognizer>,
}

impl LowLevelNTriplesParser {
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

    /// Attempt to parse a new triple from the already provided data.
    ///
    /// Returns [`None`] if the parsing is finished or more data is required.
    /// If it is the case more data should be fed using [`extend_from_slice`](Self::extend_from_slice).
    pub fn parse_next(&mut self) -> Option<Result<Triple, TurtleSyntaxError>> {
        Some(self.parser.parse_next()?.map(Into::into))
    }
}

/// A [canonical](https://www.w3.org/TR/n-triples/#canonical-ntriples) [N-Triples](https://www.w3.org/TR/n-triples/) serializer.
///
/// ```
/// use oxrdf::{NamedNodeRef, TripleRef};
/// use oxrdf::vocab::rdf;
/// use oxttl::NTriplesSerializer;
///
/// let mut serializer = NTriplesSerializer::new().for_writer(Vec::new());
/// serializer.serialize_triple(TripleRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     rdf::TYPE,
///     NamedNodeRef::new("http://schema.org/Person")?,
/// ))?;
/// assert_eq!(
///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
///     serializer.finish().as_slice()
/// );
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default, Clone)]
#[must_use]
#[expect(clippy::empty_structs_with_brackets)]
pub struct NTriplesSerializer {}

impl NTriplesSerializer {
    /// Builds a new [`NTriplesSerializer`].
    #[inline]
    pub fn new() -> Self {
        Self {}
    }

    /// Writes a N-Triples file to a [`Write`] implementation.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, TripleRef};
    /// use oxrdf::vocab::rdf;
    /// use oxttl::NTriplesSerializer;
    ///
    /// let mut serializer = NTriplesSerializer::new().for_writer(Vec::new());
    /// serializer.serialize_triple(TripleRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     rdf::TYPE,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    /// ))?;
    /// assert_eq!(
    ///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
    ///     serializer.finish().as_slice()
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_writer<W: Write>(self, writer: W) -> WriterNTriplesSerializer<W> {
        WriterNTriplesSerializer {
            writer,
            low_level_writer: self.low_level(),
        }
    }

    /// Writes a N-Triples file to a [`AsyncWrite`] implementation.
    ///
    /// ```
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use oxrdf::{NamedNodeRef, TripleRef};
    /// use oxrdf::vocab::rdf;
    /// use oxttl::NTriplesSerializer;
    ///
    /// let mut serializer = NTriplesSerializer::new().for_tokio_async_writer(Vec::new());
    /// serializer.serialize_triple(TripleRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     rdf::TYPE,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    /// )).await?;
    /// assert_eq!(
    ///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
    ///     serializer.finish().as_slice()
    /// );
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async-tokio")]
    pub fn for_tokio_async_writer<W: AsyncWrite + Unpin>(
        self,
        writer: W,
    ) -> TokioAsyncWriterNTriplesSerializer<W> {
        TokioAsyncWriterNTriplesSerializer {
            writer,
            low_level_writer: self.low_level(),
            buffer: Vec::new(),
        }
    }

    /// Builds a low-level N-Triples writer.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, TripleRef};
    /// use oxrdf::vocab::rdf;
    /// use oxttl::NTriplesSerializer;
    ///
    /// let mut buf = Vec::new();
    /// let mut serializer = NTriplesSerializer::new().low_level();
    /// serializer.serialize_triple(TripleRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     rdf::TYPE,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    /// ), &mut buf)?;
    /// assert_eq!(
    ///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
    ///     buf.as_slice()
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[expect(clippy::unused_self)]
    pub fn low_level(self) -> LowLevelNTriplesSerializer {
        LowLevelNTriplesSerializer {}
    }
}

/// Writes a N-Triples file to a [`Write`] implementation.
///
/// Can be built using [`NTriplesSerializer::for_writer`].
///
/// ```
/// use oxrdf::{NamedNodeRef, TripleRef};
/// use oxrdf::vocab::rdf;
/// use oxttl::NTriplesSerializer;
///
/// let mut serializer = NTriplesSerializer::new().for_writer(Vec::new());
/// serializer.serialize_triple(TripleRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     rdf::TYPE,
///     NamedNodeRef::new("http://schema.org/Person")?,
/// ))?;
/// assert_eq!(
///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
///     serializer.finish().as_slice()
/// );
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct WriterNTriplesSerializer<W: Write> {
    writer: W,
    low_level_writer: LowLevelNTriplesSerializer,
}

impl<W: Write> WriterNTriplesSerializer<W> {
    /// Writes an extra triple.
    pub fn serialize_triple<'a>(&mut self, t: impl Into<TripleRef<'a>>) -> io::Result<()> {
        self.low_level_writer.serialize_triple(t, &mut self.writer)
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub fn finish(self) -> W {
        self.writer
    }
}

/// Writes a N-Triples file to a [`AsyncWrite`] implementation.
///
/// Can be built using [`NTriplesSerializer::for_tokio_async_writer`].
///
/// ```
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use oxrdf::{NamedNodeRef, TripleRef};
/// use oxrdf::vocab::rdf;
/// use oxttl::NTriplesSerializer;
///
/// let mut serializer = NTriplesSerializer::new().for_tokio_async_writer(Vec::new());
/// serializer.serialize_triple(TripleRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     rdf::TYPE,
///     NamedNodeRef::new("http://schema.org/Person")?
/// )).await?;
/// assert_eq!(
///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
///     serializer.finish().as_slice()
/// );
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async-tokio")]
#[must_use]
pub struct TokioAsyncWriterNTriplesSerializer<W: AsyncWrite + Unpin> {
    writer: W,
    low_level_writer: LowLevelNTriplesSerializer,
    buffer: Vec<u8>,
}

#[cfg(feature = "async-tokio")]
impl<W: AsyncWrite + Unpin> TokioAsyncWriterNTriplesSerializer<W> {
    /// Writes an extra triple.
    pub async fn serialize_triple<'a>(&mut self, t: impl Into<TripleRef<'a>>) -> io::Result<()> {
        self.low_level_writer
            .serialize_triple(t, &mut self.buffer)?;
        self.writer.write_all(&self.buffer).await?;
        self.buffer.clear();
        Ok(())
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub fn finish(self) -> W {
        self.writer
    }
}

/// Writes a N-Triples file by using a low-level API.
///
/// Can be built using [`NTriplesSerializer::low_level`].
///
/// ```
/// use oxrdf::{NamedNodeRef, TripleRef};
/// use oxrdf::vocab::rdf;
/// use oxttl::NTriplesSerializer;
///
/// let mut buf = Vec::new();
/// let mut serializer = NTriplesSerializer::new().low_level();
/// serializer.serialize_triple(TripleRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     rdf::TYPE,
///     NamedNodeRef::new("http://schema.org/Person")?,
/// ), &mut buf)?;
/// assert_eq!(
///     b"<http://example.com#me> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .\n",
///     buf.as_slice()
/// );
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[expect(clippy::empty_structs_with_brackets)]
pub struct LowLevelNTriplesSerializer {}

impl LowLevelNTriplesSerializer {
    /// Writes an extra triple.
    #[expect(clippy::unused_self)]
    pub fn serialize_triple<'a>(
        &mut self,
        t: impl Into<TripleRef<'a>>,
        mut writer: impl Write,
    ) -> io::Result<()> {
        writeln!(writer, "{} .", t.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::{Literal, NamedNode};

    #[test]
    fn lenient_parsing() {
        let triples = NTriplesParser::new()
            .lenient()
            .for_reader(r#"<foo> <bar> "baz"@toolonglangtag ."#.as_bytes())
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        assert_eq!(
            triples,
            [Triple::new(
                NamedNode::new_unchecked("foo"),
                NamedNode::new_unchecked("bar"),
                Literal::new_language_tagged_literal_unchecked("baz", "toolonglangtag"),
            )]
        )
    }
}
