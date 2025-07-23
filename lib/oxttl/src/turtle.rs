//! A [Turtle](https://www.w3.org/TR/turtle/) streaming parser implemented by [`TurtleParser`]
//! and a serializer implemented by [`TurtleSerializer`].

use crate::MIN_PARALLEL_CHUNK_SIZE;
use crate::chunker::get_turtle_file_chunks;
use crate::terse::TriGRecognizer;
#[cfg(feature = "async-tokio")]
use crate::toolkit::TokioAsyncReaderIterator;
use crate::toolkit::{Parser, ReaderIterator, SliceIterator, TurtleParseError, TurtleSyntaxError};
#[cfg(feature = "async-tokio")]
use crate::trig::TokioAsyncWriterTriGSerializer;
use crate::trig::{LowLevelTriGSerializer, TriGSerializer, WriterTriGSerializer};
use oxiri::{Iri, IriParseError};
use oxrdf::{GraphNameRef, Triple, TripleRef};
use std::collections::HashMap;
use std::collections::hash_map::Iter;
use std::io::{self, Read, Write};
#[cfg(feature = "async-tokio")]
use tokio::io::{AsyncRead, AsyncWrite};

/// A [Turtle](https://www.w3.org/TR/turtle/) streaming parser.
///
/// Count the number of people:
/// ```
/// use oxrdf::NamedNodeRef;
/// use oxrdf::vocab::rdf;
/// use oxttl::TurtleParser;
///
/// let file = r#"@base <http://example.com/> .
/// @prefix schema: <http://schema.org/> .
/// <foo> a schema:Person ;
///     schema:name "Foo" .
/// <bar> a schema:Person ;
///     schema:name "Bar" ."#;
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for triple in TurtleParser::new().for_reader(file.as_bytes()) {
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
pub struct TurtleParser {
    lenient: bool,
    base: Option<Iri<String>>,
    prefixes: HashMap<String, Iri<String>>,
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
    /// Note that if the file is actually not valid, the parser might emit broken RDF.
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

    /// Parses a Turtle file from a [`Read`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::NamedNodeRef;
    /// use oxrdf::vocab::rdf;
    /// use oxttl::TurtleParser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" .
    /// <bar> a schema:Person ;
    ///     schema:name "Bar" ."#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// for triple in TurtleParser::new().for_reader(file.as_bytes()) {
    ///     let triple = triple?;
    ///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_reader<R: Read>(self, reader: R) -> ReaderTurtleParser<R> {
        ReaderTurtleParser {
            inner: self.low_level().parser.for_reader(reader),
        }
    }

    /// Parses a Turtle file from a [`AsyncRead`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use oxrdf::NamedNodeRef;
    /// use oxrdf::vocab::rdf;
    /// use oxttl::TurtleParser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" .
    /// <bar> a schema:Person ;
    ///     schema:name "Bar" ."#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// let mut parser = TurtleParser::new().for_tokio_async_reader(file.as_bytes());
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
    ) -> TokioAsyncReaderTurtleParser<R> {
        TokioAsyncReaderTurtleParser {
            inner: self.low_level().parser.for_tokio_async_reader(reader),
        }
    }

    /// Parses Turtle file from a byte slice.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::NamedNodeRef;
    /// use oxrdf::vocab::rdf;
    /// use oxttl::TurtleParser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" .
    /// <bar> a schema:Person ;
    ///     schema:name "Bar" ."#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// for triple in TurtleParser::new().for_slice(file) {
    ///     let triple = triple?;
    ///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_slice(self, slice: &(impl AsRef<[u8]> + ?Sized)) -> SliceTurtleParser<'_> {
        SliceTurtleParser {
            inner: TriGRecognizer::new_parser(
                slice.as_ref(),
                true,
                false,
                self.lenient,
                self.base,
                self.prefixes,
            )
            .into_iter(),
        }
    }

    /// Creates a vector of parsers that may be used to parse a Turtle document slice in parallel.
    /// To dynamically specify target_parallelism, use e.g. [`std::thread::available_parallelism`].
    /// Intended to work on large documents.
    /// Can fail or return wrong results if there are prefixes or base iris that are not defined
    /// at the top of the document, or valid turtle syntax inside literal values.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::NamedNodeRef;
    /// use oxrdf::vocab::rdf;
    /// use oxttl::TurtleParser;
    /// use rayon::iter::{IntoParallelIterator, ParallelIterator};
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" .
    /// <bar> a schema:Person ;
    ///     schema:name "Bar" ."#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let readers = TurtleParser::new().split_slice_for_parallel_parsing(file, 2);
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
        mut self,
        slice: &(impl AsRef<[u8]> + ?Sized),
        target_parallelism: usize,
    ) -> Vec<SliceTurtleParser<'_>> {
        let slice = slice.as_ref();
        let n_chunks = (slice.len() / MIN_PARALLEL_CHUNK_SIZE).clamp(1, target_parallelism);

        if n_chunks > 1 {
            // Prefixes must be determined before chunks, since determining chunks relies on parser with prefixes determined.
            let mut from_slice_parser = self.clone().for_slice(slice);
            // We don't care about errors: they will be raised when parsing the first chunk anyway
            from_slice_parser.next();
            for (p, iri) in from_slice_parser.prefixes() {
                // Already know this is a valid IRI
                self = self.with_prefix(p, iri).unwrap();
            }
        }

        get_turtle_file_chunks(slice, n_chunks, &self)
            .into_iter()
            .map(|(start, end)| self.clone().for_slice(&slice[start..end]))
            .collect()
    }

    /// Allows to parse a Turtle file by using a low-level API.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::NamedNodeRef;
    /// use oxrdf::vocab::rdf;
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
    /// let mut parser = TurtleParser::new().low_level();
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
    pub fn low_level(self) -> LowLevelTurtleParser {
        LowLevelTurtleParser {
            parser: TriGRecognizer::new_parser(
                Vec::new(),
                false,
                false,
                self.lenient,
                self.base,
                self.prefixes,
            ),
        }
    }
}

/// Parses a Turtle file from a [`Read`] implementation.
///
/// Can be built using [`TurtleParser::for_reader`].
///
/// Count the number of people:
/// ```
/// use oxrdf::NamedNodeRef;
/// use oxrdf::vocab::rdf;
/// use oxttl::TurtleParser;
///
/// let file = r#"@base <http://example.com/> .
/// @prefix schema: <http://schema.org/> .
/// <foo> a schema:Person ;
///     schema:name "Foo" .
/// <bar> a schema:Person ;
///     schema:name "Bar" ."#;
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for triple in TurtleParser::new().for_reader(file.as_bytes()) {
///     let triple = triple?;
///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct ReaderTurtleParser<R: Read> {
    inner: ReaderIterator<R, TriGRecognizer>,
}

impl<R: Read> ReaderTurtleParser<R> {
    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TurtleParser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = TurtleParser::new().for_reader(file.as_bytes());
    /// assert!(parser.prefixes().collect::<Vec<_>>().is_empty()); // No prefix at the beginning
    ///
    /// parser.next().unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// //
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
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
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = TurtleParser::new().for_reader(file.as_bytes());
    /// assert!(parser.base_iri().is_none()); // No base at the beginning because none has been given to the parser.
    ///
    /// parser.next().unwrap()?; // We read the first triple
    /// assert_eq!(parser.base_iri(), Some("http://example.com/")); // There is now a base IRI.
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
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

impl<R: Read> Iterator for ReaderTurtleParser<R> {
    type Item = Result<Triple, TurtleParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.inner.next()?.map(Into::into))
    }
}

/// Parses a Turtle file from a [`AsyncRead`] implementation.
///
/// Can be built using [`TurtleParser::for_tokio_async_reader`].
///
/// Count the number of people:
/// ```
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use oxrdf::NamedNodeRef;
/// use oxrdf::vocab::rdf;
/// use oxttl::TurtleParser;
///
/// let file = r#"@base <http://example.com/> .
/// @prefix schema: <http://schema.org/> .
/// <foo> a schema:Person ;
///     schema:name "Foo" .
/// <bar> a schema:Person ;
///     schema:name "Bar" ."#;
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// let mut parser = TurtleParser::new().for_tokio_async_reader(file.as_bytes());
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
pub struct TokioAsyncReaderTurtleParser<R: AsyncRead + Unpin> {
    inner: TokioAsyncReaderIterator<R, TriGRecognizer>,
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> TokioAsyncReaderTurtleParser<R> {
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
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use oxttl::TurtleParser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = TurtleParser::new().for_tokio_async_reader(file.as_bytes());
    /// assert_eq!(parser.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// parser.next().await.unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// //
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
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use oxttl::TurtleParser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = TurtleParser::new().for_tokio_async_reader(file.as_bytes());
    /// assert!(parser.base_iri().is_none()); // No base IRI at the beginning
    ///
    /// parser.next().await.unwrap()?; // We read the first triple
    /// assert_eq!(parser.base_iri(), Some("http://example.com/")); // There is now a base IRI
    /// //
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

/// Parses a Turtle file from a byte slice.
///
/// Can be built using [`TurtleParser::for_slice`].
///
/// Count the number of people:
/// ```
/// use oxrdf::NamedNodeRef;
/// use oxrdf::vocab::rdf;
/// use oxttl::TurtleParser;
///
/// let file = r#"@base <http://example.com/> .
/// @prefix schema: <http://schema.org/> .
/// <foo> a schema:Person ;
///     schema:name "Foo" .
/// <bar> a schema:Person ;
///     schema:name "Bar" ."#;
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for triple in TurtleParser::new().for_slice(file) {
///     let triple = triple?;
///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct SliceTurtleParser<'a> {
    inner: SliceIterator<'a, TriGRecognizer>,
}

impl SliceTurtleParser<'_> {
    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TurtleParser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = TurtleParser::new().for_slice(file);
    /// assert!(parser.prefixes().collect::<Vec<_>>().is_empty()); // No prefix at the beginning
    ///
    /// parser.next().unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// //
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
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
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = TurtleParser::new().for_slice(file);
    /// assert!(parser.base_iri().is_none()); // No base at the beginning because none has been given to the parser.
    ///
    /// parser.next().unwrap()?; // We read the first triple
    /// assert_eq!(parser.base_iri(), Some("http://example.com/")); // There is now a base IRI.
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
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

impl Iterator for SliceTurtleParser<'_> {
    type Item = Result<Triple, TurtleSyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.inner.next()?.map(Into::into))
    }
}

/// Parses a Turtle file by using a low-level API.
///
/// Can be built using [`TurtleParser::low_level`].
///
/// Count the number of people:
/// ```
/// use oxrdf::NamedNodeRef;
/// use oxrdf::vocab::rdf;
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
/// let mut parser = TurtleParser::new().low_level();
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
pub struct LowLevelTurtleParser {
    parser: Parser<Vec<u8>, TriGRecognizer>,
}

impl LowLevelTurtleParser {
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

    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TurtleParser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = TurtleParser::new().low_level();
    /// parser.extend_from_slice(file.as_bytes());
    /// assert_eq!(parser.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// parser.parse_next().unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// //
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
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
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = TurtleParser::new().low_level();
    /// parser.extend_from_slice(file.as_bytes());
    /// assert!(parser.base_iri().is_none()); // No base IRI at the beginning
    ///
    /// parser.parse_next().unwrap()?; // We read the first triple
    /// assert_eq!(parser.base_iri(), Some("http://example.com/")); // There is now a base IRI
    /// //
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
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
/// See [`LowLevelTurtleParser::prefixes`].
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
/// ```
/// use oxrdf::vocab::rdf;
/// use oxrdf::{NamedNodeRef, TripleRef};
/// use oxttl::TurtleSerializer;
///
/// let mut serializer = TurtleSerializer::new()
///     .with_prefix("schema", "http://schema.org/")?
///     .for_writer(Vec::new());
/// serializer.serialize_triple(TripleRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     rdf::TYPE,
///     NamedNodeRef::new("http://schema.org/Person")?,
/// ))?;
/// assert_eq!(
///     b"@prefix schema: <http://schema.org/> .\n<http://example.com#me> a schema:Person .\n",
///     serializer.finish()?.as_slice()
/// );
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
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

    /// Adds a base IRI to the serialization.
    ///
    /// ```
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::{NamedNodeRef, TripleRef};
    /// use oxttl::TurtleSerializer;
    ///
    /// let mut serializer = TurtleSerializer::new()
    ///     .with_base_iri("http://example.com")?
    ///     .with_prefix("ex", "http://example.com/ns#")?
    ///     .for_writer(Vec::new());
    /// serializer.serialize_triple(TripleRef::new(
    ///     NamedNodeRef::new("http://example.com/me")?,
    ///     rdf::TYPE,
    ///     NamedNodeRef::new("http://example.com/ns#Person")?,
    /// ))?;
    /// assert_eq!(
    ///     b"@base <http://example.com> .\n@prefix ex: </ns#> .\n</me> a ex:Person .\n",
    ///     serializer.finish()?.as_slice()
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_base_iri(mut self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        self.inner = self.inner.with_base_iri(base_iri)?;
        Ok(self)
    }

    /// Writes a Turtle file to a [`Write`] implementation.
    ///
    /// ```
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::{NamedNodeRef, TripleRef};
    /// use oxttl::TurtleSerializer;
    ///
    /// let mut serializer = TurtleSerializer::new()
    ///     .with_prefix("schema", "http://schema.org/")?
    ///     .for_writer(Vec::new());
    /// serializer.serialize_triple(TripleRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     rdf::TYPE,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    /// ))?;
    /// assert_eq!(
    ///     b"@prefix schema: <http://schema.org/> .\n<http://example.com#me> a schema:Person .\n",
    ///     serializer.finish()?.as_slice()
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_writer<W: Write>(self, writer: W) -> WriterTurtleSerializer<W> {
        WriterTurtleSerializer {
            inner: self.inner.for_writer(writer),
        }
    }

    /// Writes a Turtle file to a [`AsyncWrite`] implementation.
    ///
    /// ```
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::{NamedNodeRef, TripleRef};
    /// use oxttl::TurtleSerializer;
    ///
    /// let mut serializer = TurtleSerializer::new()
    ///     .with_prefix("schema", "http://schema.org/")?
    ///     .for_tokio_async_writer(Vec::new());
    /// serializer
    ///     .serialize_triple(TripleRef::new(
    ///         NamedNodeRef::new("http://example.com#me")?,
    ///         rdf::TYPE,
    ///         NamedNodeRef::new("http://schema.org/Person")?,
    ///     ))
    ///     .await?;
    /// assert_eq!(
    ///     b"@prefix schema: <http://schema.org/> .\n<http://example.com#me> a schema:Person .\n",
    ///     serializer.finish().await?.as_slice()
    /// );
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async-tokio")]
    pub fn for_tokio_async_writer<W: AsyncWrite + Unpin>(
        self,
        writer: W,
    ) -> TokioAsyncWriterTurtleSerializer<W> {
        TokioAsyncWriterTurtleSerializer {
            inner: self.inner.for_tokio_async_writer(writer),
        }
    }

    /// Builds a low-level Turtle writer.
    ///
    /// ```
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::{NamedNodeRef, TripleRef};
    /// use oxttl::TurtleSerializer;
    ///
    /// let mut buf = Vec::new();
    /// let mut serializer = TurtleSerializer::new()
    ///     .with_prefix("schema", "http://schema.org/")?
    ///     .low_level();
    /// serializer.serialize_triple(
    ///     TripleRef::new(
    ///         NamedNodeRef::new("http://example.com#me")?,
    ///         rdf::TYPE,
    ///         NamedNodeRef::new("http://schema.org/Person")?,
    ///     ),
    ///     &mut buf,
    /// )?;
    /// serializer.finish(&mut buf)?;
    /// assert_eq!(
    ///     b"@prefix schema: <http://schema.org/> .\n<http://example.com#me> a schema:Person .\n",
    ///     buf.as_slice()
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn low_level(self) -> LowLevelTurtleSerializer {
        LowLevelTurtleSerializer {
            inner: self.inner.low_level(),
        }
    }
}

/// Writes a Turtle file to a [`Write`] implementation.
///
/// Can be built using [`TurtleSerializer::for_writer`].
///
/// ```
/// use oxrdf::vocab::rdf;
/// use oxrdf::{NamedNodeRef, TripleRef};
/// use oxttl::TurtleSerializer;
///
/// let mut serializer = TurtleSerializer::new()
///     .with_prefix("schema", "http://schema.org/")?
///     .for_writer(Vec::new());
/// serializer.serialize_triple(TripleRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     rdf::TYPE,
///     NamedNodeRef::new("http://schema.org/Person")?,
/// ))?;
/// assert_eq!(
///     b"@prefix schema: <http://schema.org/> .\n<http://example.com#me> a schema:Person .\n",
///     serializer.finish()?.as_slice()
/// );
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct WriterTurtleSerializer<W: Write> {
    inner: WriterTriGSerializer<W>,
}

impl<W: Write> WriterTurtleSerializer<W> {
    /// Writes an extra triple.
    pub fn serialize_triple<'a>(&mut self, t: impl Into<TripleRef<'a>>) -> io::Result<()> {
        self.inner
            .serialize_quad(t.into().in_graph(GraphNameRef::DefaultGraph))
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub fn finish(self) -> io::Result<W> {
        self.inner.finish()
    }
}

/// Writes a Turtle file to a [`AsyncWrite`] implementation.
///
/// Can be built using [`TurtleSerializer::for_tokio_async_writer`].
///
/// ```
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use oxrdf::vocab::rdf;
/// use oxrdf::{NamedNodeRef, TripleRef};
/// use oxttl::TurtleSerializer;
///
/// let mut serializer = TurtleSerializer::new()
///     .with_prefix("schema", "http://schema.org/")?
///     .for_tokio_async_writer(Vec::new());
/// serializer
///     .serialize_triple(TripleRef::new(
///         NamedNodeRef::new("http://example.com#me")?,
///         rdf::TYPE,
///         NamedNodeRef::new("http://schema.org/Person")?,
///     ))
///     .await?;
/// assert_eq!(
///     b"@prefix schema: <http://schema.org/> .\n<http://example.com#me> a schema:Person .\n",
///     serializer.finish().await?.as_slice()
/// );
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async-tokio")]
#[must_use]
pub struct TokioAsyncWriterTurtleSerializer<W: AsyncWrite + Unpin> {
    inner: TokioAsyncWriterTriGSerializer<W>,
}

#[cfg(feature = "async-tokio")]
impl<W: AsyncWrite + Unpin> TokioAsyncWriterTurtleSerializer<W> {
    /// Writes an extra triple.
    pub async fn serialize_triple<'a>(&mut self, t: impl Into<TripleRef<'a>>) -> io::Result<()> {
        self.inner
            .serialize_quad(t.into().in_graph(GraphNameRef::DefaultGraph))
            .await
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub async fn finish(self) -> io::Result<W> {
        self.inner.finish().await
    }
}

/// Writes a Turtle file by using a low-level API.
///
/// Can be built using [`TurtleSerializer::low_level`].
///
/// ```
/// use oxrdf::vocab::rdf;
/// use oxrdf::{NamedNodeRef, TripleRef};
/// use oxttl::TurtleSerializer;
///
/// let mut buf = Vec::new();
/// let mut serializer = TurtleSerializer::new()
///     .with_prefix("schema", "http://schema.org/")?
///     .low_level();
/// serializer.serialize_triple(
///     TripleRef::new(
///         NamedNodeRef::new("http://example.com#me")?,
///         rdf::TYPE,
///         NamedNodeRef::new("http://schema.org/Person")?,
///     ),
///     &mut buf,
/// )?;
/// serializer.finish(&mut buf)?;
/// assert_eq!(
///     b"@prefix schema: <http://schema.org/> .\n<http://example.com#me> a schema:Person .\n",
///     buf.as_slice()
/// );
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct LowLevelTurtleSerializer {
    inner: LowLevelTriGSerializer,
}

impl LowLevelTurtleSerializer {
    /// Writes an extra triple.
    pub fn serialize_triple<'a>(
        &mut self,
        t: impl Into<TripleRef<'a>>,
        writer: impl Write,
    ) -> io::Result<()> {
        self.inner
            .serialize_quad(t.into().in_graph(GraphNameRef::DefaultGraph), writer)
    }

    /// Finishes to write the file.
    pub fn finish(&mut self, writer: impl Write) -> io::Result<()> {
        self.inner.finish(writer)
    }
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn)]
mod tests {
    use super::*;
    use oxrdf::{BlankNodeRef, LiteralRef, NamedNodeRef};

    #[test]
    fn test_write() -> io::Result<()> {
        let mut serializer = TurtleSerializer::new().for_writer(Vec::new());
        serializer.serialize_triple(TripleRef::new(
            NamedNodeRef::new_unchecked("http://example.com/s"),
            NamedNodeRef::new_unchecked("http://example.com/p"),
            NamedNodeRef::new_unchecked("http://example.com/o"),
        ))?;
        serializer.serialize_triple(TripleRef::new(
            NamedNodeRef::new_unchecked("http://example.com/s"),
            NamedNodeRef::new_unchecked("http://example.com/p"),
            LiteralRef::new_simple_literal("foo"),
        ))?;
        serializer.serialize_triple(TripleRef::new(
            NamedNodeRef::new_unchecked("http://example.com/s"),
            NamedNodeRef::new_unchecked("http://example.com/p2"),
            LiteralRef::new_language_tagged_literal_unchecked("foo", "en"),
        ))?;
        serializer.serialize_triple(TripleRef::new(
            BlankNodeRef::new_unchecked("b"),
            NamedNodeRef::new_unchecked("http://example.com/p2"),
            BlankNodeRef::new_unchecked("b2"),
        ))?;
        assert_eq!(
            String::from_utf8(serializer.finish()?).unwrap(),
            "<http://example.com/s> <http://example.com/p> <http://example.com/o> , \"foo\" ;\n\t<http://example.com/p2> \"foo\"@en .\n_:b <http://example.com/p2> _:b2 .\n"
        );
        Ok(())
    }
}
