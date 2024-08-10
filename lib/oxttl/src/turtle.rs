//! A [Turtle](https://www.w3.org/TR/turtle/) streaming parser implemented by [`TurtleParser`]
//! and a serializer implemented by [`TurtleSerializer`].

use crate::chunker::get_turtle_file_chunks;
use crate::terse::TriGRecognizer;
#[cfg(feature = "async-tokio")]
use crate::toolkit::FromTokioAsyncReadIterator;
use crate::toolkit::{
    FromReadIterator, FromSliceIterator, Parser, TurtleParseError, TurtleSyntaxError,
};
#[cfg(feature = "async-tokio")]
use crate::trig::ToTokioAsyncWriteTriGWriter;
use crate::trig::{LowLevelTriGWriter, ToWriteTriGWriter, TriGSerializer};
use crate::MIN_PARALLEL_CHUNK_SIZE;
use oxiri::{Iri, IriParseError};
use oxrdf::{GraphNameRef, Triple, TripleRef};
use std::collections::hash_map::Iter;
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
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
/// use oxttl::TurtleParser;
///
/// let file = br#"@base <http://example.com/> .
/// @prefix schema: <http://schema.org/> .
/// <foo> a schema:Person ;
///     schema:name "Foo" .
/// <bar> a schema:Person ;
///     schema:name "Bar" ."#;
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
#[derive(Default, Clone)]
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
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    /// use oxttl::TurtleParser;
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" .
    /// <bar> a schema:Person ;
    ///     schema:name "Bar" ."#;
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
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    /// use oxttl::TurtleParser;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), oxttl::TurtleParseError> {
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" .
    /// <bar> a schema:Person ;
    ///     schema:name "Bar" ."#;
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

    /// Parses Turtle file from a byte slice.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    /// use oxttl::TurtleParser;
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" .
    /// <bar> a schema:Person ;
    ///     schema:name "Bar" ."#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// for triple in TurtleParser::new().parse_slice(file) {
    ///     let triple = triple?;
    ///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn parse_slice(self, slice: &[u8]) -> FromSliceTurtleReader<'_> {
        FromSliceTurtleReader {
            inner: TriGRecognizer::new_parser(
                slice,
                true,
                false,
                #[cfg(feature = "rdf-star")]
                self.with_quoted_triples,
                self.unchecked,
                self.base,
                self.prefixes,
            )
            .into_iter(),
        }
    }

    /// Creates a vector of iterators that may be used to parse a Turtle document slice in parallel.
    /// To dynamically specify target_parallelism, use e.g. [`std::thread::available_parallelism`].
    /// Intended to work on large documents.
    /// Can fail or return wrong results if there are prefixes or base iris that are not defined
    /// at the top of the document, or valid turtle syntax inside literal values.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    /// use oxttl::TurtleParser;
    /// use rayon::iter::{IntoParallelIterator, ParallelIterator};
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" .
    /// <bar> a schema:Person ;
    ///     schema:name "Bar" ."#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let readers = TurtleParser::new().split_slice_for_parallel_parsing(file.as_ref(), 2);
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
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn split_slice_for_parallel_parsing(
        mut self,
        slice: &[u8],
        target_parallelism: usize,
    ) -> Vec<FromSliceTurtleReader<'_>> {
        let n_chunks = (slice.len() / MIN_PARALLEL_CHUNK_SIZE).clamp(1, target_parallelism);

        if n_chunks > 1 {
            // Prefixes must be determined before chunks, since determining chunks relies on parser with prefixes determined.
            let mut from_slice_reader = self.clone().parse_slice(slice);
            // We don't care about errors: they will be raised when parsing the first chunk anyway
            from_slice_reader.next();
            for (p, iri) in from_slice_reader.prefixes() {
                // Already know this is a valid IRI
                self = self.with_prefix(p, iri).unwrap();
            }
        }

        get_turtle_file_chunks(slice, n_chunks, &self)
            .into_iter()
            .map(|(start, end)| self.clone().parse_slice(&slice[start..end]))
            .collect()
    }

    /// Allows to parse a Turtle file by using a low-level API.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    /// use oxttl::TurtleParser;
    ///
    /// let file: [&[u8]; 5] = [
    ///     b"@base <http://example.com/>",
    ///     b". @prefix schema: <http://schema.org/> .",
    ///     b"<foo> a schema:Person",
    ///     b" ; schema:name \"Foo\" . <bar>",
    ///     b" a schema:Person ; schema:name \"Bar\" .",
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
                Vec::new(),
                false,
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
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
/// use oxttl::TurtleParser;
///
/// let file = br#"@base <http://example.com/> .
/// @prefix schema: <http://schema.org/> .
/// <foo> a schema:Person ;
///     schema:name "Foo" .
/// <bar> a schema:Person ;
///     schema:name "Bar" ."#;
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
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TurtleParser;
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut reader = TurtleParser::new().parse_read(file.as_ref());
    /// assert!(reader.prefixes().collect::<Vec<_>>().is_empty()); // No prefix at the beginning
    ///
    /// reader.next().unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     reader.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn prefixes(&self) -> TurtlePrefixesIter<'_> {
        TurtlePrefixesIter {
            inner: self.inner.parser.context.prefixes(),
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// use oxttl::TurtleParser;
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut reader = TurtleParser::new().parse_read(file.as_ref());
    /// assert!(reader.base_iri().is_none()); // No base at the beginning because none has been given to the parser.
    ///
    /// reader.next().unwrap()?; // We read the first triple
    /// assert_eq!(reader.base_iri(), Some("http://example.com/")); // There is now a base IRI.
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn base_iri(&self) -> Option<&str> {
        self.inner
            .parser
            .context
            .lexer_options
            .base_iri
            .as_ref()
            .map(Iri::as_str)
    }
}

impl<R: Read> Iterator for FromReadTurtleReader<R> {
    type Item = Result<Triple, TurtleParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.inner.next()?.map(Into::into))
    }
}

/// Parses a Turtle file from a [`AsyncRead`] implementation. Can be built using [`TurtleParser::parse_tokio_async_read`].
///
/// Count the number of people:
/// ```
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
/// use oxttl::TurtleParser;
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), oxttl::TurtleParseError> {
/// let file = br#"@base <http://example.com/> .
/// @prefix schema: <http://schema.org/> .
/// <foo> a schema:Person ;
///     schema:name "Foo" .
/// <bar> a schema:Person ;
///     schema:name "Bar" ."#;
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
    pub async fn next(&mut self) -> Option<Result<Triple, TurtleParseError>> {
        Some(self.inner.next().await?.map(Into::into))
    }

    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TurtleParser;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), oxttl::TurtleParseError> {
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut reader = TurtleParser::new().parse_tokio_async_read(file.as_ref());
    /// assert_eq!(reader.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// reader.next().await.unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     reader.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// # Ok(())
    /// # }
    /// ```
    pub fn prefixes(&self) -> TurtlePrefixesIter<'_> {
        TurtlePrefixesIter {
            inner: self.inner.parser.context.prefixes(),
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// use oxttl::TurtleParser;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), oxttl::TurtleParseError> {
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut reader = TurtleParser::new().parse_tokio_async_read(file.as_ref());
    /// assert!(reader.base_iri().is_none()); // No base IRI at the beginning
    ///
    /// reader.next().await.unwrap()?; // We read the first triple
    /// assert_eq!(reader.base_iri(), Some("http://example.com/")); // There is now a base IRI
    /// # Ok(())
    /// # }
    /// ```
    pub fn base_iri(&self) -> Option<&str> {
        self.inner
            .parser
            .context
            .lexer_options
            .base_iri
            .as_ref()
            .map(Iri::as_str)
    }
}

/// Parses a Turtle file from a byte slice. Can be built using [`TurtleParser::parse_slice`].
///
/// Count the number of people:
/// ```
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
/// use oxttl::TurtleParser;
///
/// let file = br#"@base <http://example.com/> .
/// @prefix schema: <http://schema.org/> .
/// <foo> a schema:Person ;
///     schema:name "Foo" .
/// <bar> a schema:Person ;
///     schema:name "Bar" ."#;
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for triple in TurtleParser::new().parse_slice(file) {
///     let triple = triple?;
///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct FromSliceTurtleReader<'a> {
    inner: FromSliceIterator<'a, TriGRecognizer>,
}

impl<'a> FromSliceTurtleReader<'a> {
    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TurtleParser;
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut reader = TurtleParser::new().parse_slice(file);
    /// assert!(reader.prefixes().collect::<Vec<_>>().is_empty()); // No prefix at the beginning
    ///
    /// reader.next().unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     reader.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn prefixes(&self) -> TurtlePrefixesIter<'_> {
        TurtlePrefixesIter {
            inner: self.inner.parser.context.prefixes(),
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// use oxttl::TurtleParser;
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut reader = TurtleParser::new().parse_slice(file);
    /// assert!(reader.base_iri().is_none()); // No base at the beginning because none has been given to the parser.
    ///
    /// reader.next().unwrap()?; // We read the first triple
    /// assert_eq!(reader.base_iri(), Some("http://example.com/")); // There is now a base IRI.
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn base_iri(&self) -> Option<&str> {
        self.inner
            .parser
            .context
            .lexer_options
            .base_iri
            .as_ref()
            .map(Iri::as_str)
    }
}

impl<'a> Iterator for FromSliceTurtleReader<'a> {
    type Item = Result<Triple, TurtleSyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.inner.next()?.map(Into::into))
    }
}

/// Parses a Turtle file by using a low-level API. Can be built using [`TurtleParser::parse`].
///
/// Count the number of people:
/// ```
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
/// use oxttl::TurtleParser;
///
/// let file: [&[u8]; 5] = [
///     b"@base <http://example.com/>",
///     b". @prefix schema: <http://schema.org/> .",
///     b"<foo> a schema:Person",
///     b" ; schema:name \"Foo\" . <bar>",
///     b" a schema:Person ; schema:name \"Bar\" .",
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
    parser: Parser<Vec<u8>, TriGRecognizer>,
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
    pub fn read_next(&mut self) -> Option<Result<Triple, TurtleSyntaxError>> {
        Some(self.parser.read_next()?.map(Into::into))
    }

    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TurtleParser;
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut reader = TurtleParser::new().parse();
    /// reader.extend_from_slice(file);
    /// assert_eq!(reader.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// reader.read_next().unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     reader.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn prefixes(&self) -> TurtlePrefixesIter<'_> {
        TurtlePrefixesIter {
            inner: self.parser.context.prefixes(),
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// use oxttl::TurtleParser;
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut reader = TurtleParser::new().parse();
    /// reader.extend_from_slice(file);
    /// assert!(reader.base_iri().is_none()); // No base IRI at the beginning
    ///
    /// reader.read_next().unwrap()?; // We read the first triple
    /// assert_eq!(reader.base_iri(), Some("http://example.com/")); // There is now a base IRI
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn base_iri(&self) -> Option<&str> {
        self.parser
            .context
            .lexer_options
            .base_iri
            .as_ref()
            .map(Iri::as_str)
    }
}

/// Iterator on the file prefixes.
///
/// See [`LowLevelTurtleReader::prefixes`].
pub struct TurtlePrefixesIter<'a> {
    inner: Iter<'a, String, Iri<String>>,
}

impl<'a> Iterator for TurtlePrefixesIter<'a> {
    type Item = (&'a str, &'a str);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (key, value) = self.inner.next()?;
        Some((key.as_str(), value.as_str()))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
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
/// let mut writer = TurtleSerializer::new()
///     .with_prefix("schema", "http://schema.org/")?
///     .serialize_to_write(Vec::new());
/// writer.write_triple(TripleRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
/// ))?;
/// assert_eq!(
///     b"@prefix schema: <http://schema.org/> .\n<http://example.com#me> a schema:Person .\n",
///     writer.finish()?.as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default, Clone)]
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

    #[inline]
    pub fn with_prefix(
        mut self,
        prefix_name: impl Into<String>,
        prefix_iri: impl Into<String>,
    ) -> Result<Self, IriParseError> {
        self.inner = self.inner.with_prefix(prefix_name, prefix_iri)?;
        Ok(self)
    }

    /// Writes a Turtle file to a [`Write`] implementation.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, TripleRef};
    /// use oxttl::TurtleSerializer;
    ///
    /// let mut writer = TurtleSerializer::new()
    ///     .with_prefix("schema", "http://schema.org/")?
    ///     .serialize_to_write(Vec::new());
    /// writer.write_triple(TripleRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    /// ))?;
    /// assert_eq!(
    ///     b"@prefix schema: <http://schema.org/> .\n<http://example.com#me> a schema:Person .\n",
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
    /// # async fn main() -> Result<(),Box<dyn std::error::Error>> {
    /// let mut writer = TurtleSerializer::new()
    ///     .with_prefix("schema", "http://schema.org/")?
    ///     .serialize_to_tokio_async_write(Vec::new());
    /// writer
    ///     .write_triple(TripleRef::new(
    ///         NamedNodeRef::new_unchecked("http://example.com#me"),
    ///         NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
    ///         NamedNodeRef::new_unchecked("http://schema.org/Person"),
    ///     ))
    ///     .await?;
    /// assert_eq!(
    ///     b"@prefix schema: <http://schema.org/> .\n<http://example.com#me> a schema:Person .\n",
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
    /// let mut writer = TurtleSerializer::new()
    ///     .with_prefix("schema", "http://schema.org/")?
    ///     .serialize();
    /// writer.write_triple(
    ///     TripleRef::new(
    ///         NamedNodeRef::new("http://example.com#me")?,
    ///         NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
    ///         NamedNodeRef::new("http://schema.org/Person")?,
    ///     ),
    ///     &mut buf,
    /// )?;
    /// writer.finish(&mut buf)?;
    /// assert_eq!(
    ///     b"@prefix schema: <http://schema.org/> .\n<http://example.com#me> a schema:Person .\n",
    ///     buf.as_slice()
    /// );
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn serialize(self) -> LowLevelTurtleWriter {
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
/// let mut writer = TurtleSerializer::new()
///     .with_prefix("schema", "http://schema.org/")?
///     .serialize_to_write(Vec::new());
/// writer.write_triple(TripleRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
/// ))?;
/// assert_eq!(
///     b"@prefix schema: <http://schema.org/> .\n<http://example.com#me> a schema:Person .\n",
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
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut writer = TurtleSerializer::new()
///     .with_prefix("schema", "http://schema.org/")?
///     .serialize_to_tokio_async_write(Vec::new());
/// writer
///     .write_triple(TripleRef::new(
///         NamedNodeRef::new_unchecked("http://example.com#me"),
///         NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
///         NamedNodeRef::new_unchecked("http://schema.org/Person"),
///     ))
///     .await?;
/// assert_eq!(
///     b"@prefix schema: <http://schema.org/> .\n<http://example.com#me> a schema:Person .\n",
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
/// let mut writer = TurtleSerializer::new()
///     .with_prefix("schema", "http://schema.org/")?
///     .serialize();
/// writer.write_triple(
///     TripleRef::new(
///         NamedNodeRef::new("http://example.com#me")?,
///         NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///         NamedNodeRef::new("http://schema.org/Person")?,
///     ),
///     &mut buf,
/// )?;
/// writer.finish(&mut buf)?;
/// assert_eq!(
///     b"@prefix schema: <http://schema.org/> .\n<http://example.com#me> a schema:Person .\n",
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
#[allow(clippy::panic_in_result_fn)]
mod tests {
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
