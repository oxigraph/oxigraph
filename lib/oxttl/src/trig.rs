//! A [TriG](https://www.w3.org/TR/trig/) streaming parser implemented by [`TriGParser`]
//! and a serializer implemented by [`TriGSerializer`].

use crate::lexer::N3Lexer;
use crate::terse::TriGRecognizer;
#[cfg(feature = "async-tokio")]
use crate::toolkit::FromTokioAsyncReadIterator;
use crate::toolkit::{
    FromReadIterator, FromSliceIterator, Parser, TurtleParseError, TurtleSyntaxError,
};
use oxiri::{Iri, IriParseError};
use oxrdf::vocab::{rdf, xsd};
use oxrdf::{
    GraphName, GraphNameRef, LiteralRef, NamedNode, NamedNodeRef, Quad, QuadRef, Subject, TermRef,
};
use std::collections::hash_map::Iter;
use std::collections::{BTreeMap, HashMap};
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
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
/// use oxttl::TriGParser;
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
/// for quad in TriGParser::new().parse_read(file.as_ref()) {
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
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    /// use oxttl::TriGParser;
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
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    /// use oxttl::TriGParser;
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

    /// Parses a TriG file from a byte slice.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    /// use oxttl::TriGParser;
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
    /// for quad in TriGParser::new().parse_slice(file) {
    ///     let quad = quad?;
    ///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn parse_slice(self, slice: &[u8]) -> FromSliceTriGReader<'_> {
        FromSliceTriGReader {
            inner: TriGRecognizer::new_parser(
                slice,
                true,
                true,
                #[cfg(feature = "rdf-star")]
                self.with_quoted_triples,
                self.unchecked,
                self.base,
                self.prefixes,
            )
            .into_iter(),
        }
    }

    /// Allows to parse a TriG file by using a low-level API.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    /// use oxttl::TriGParser;
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
                Vec::new(),
                false,
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
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
/// use oxttl::TriGParser;
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
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TriGParser;
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut reader = TriGParser::new().parse_read(file.as_ref());
    /// assert_eq!(reader.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// reader.next().unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     reader.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn prefixes(&self) -> TriGPrefixesIter<'_> {
        TriGPrefixesIter {
            inner: self.inner.parser.context.prefixes(),
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// use oxttl::TriGParser;
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut reader = TriGParser::new().parse_read(file.as_ref());
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

impl<R: Read> Iterator for FromReadTriGReader<R> {
    type Item = Result<Quad, TurtleParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// Parses a TriG file from a [`AsyncRead`] implementation. Can be built using [`TriGParser::parse_tokio_async_read`].
///
/// Count the number of people:
/// ```
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
/// use oxttl::TriGParser;
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
    pub async fn next(&mut self) -> Option<Result<Quad, TurtleParseError>> {
        Some(self.inner.next().await?.map(Into::into))
    }

    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TriGParser;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), oxttl::TurtleParseError> {
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut reader = TriGParser::new().parse_tokio_async_read(file.as_ref());
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
    pub fn prefixes(&self) -> TriGPrefixesIter<'_> {
        TriGPrefixesIter {
            inner: self.inner.parser.context.prefixes(),
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// use oxttl::TriGParser;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), oxttl::TurtleParseError> {
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut reader = TriGParser::new().parse_tokio_async_read(file.as_ref());
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

/// Parses a TriG file from a byte slice. Can be built using [`TriGParser::parse_slice`].
///
/// Count the number of people:
/// ```
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
/// use oxttl::TriGParser;
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
/// for quad in TriGParser::new().parse_slice(file) {
///     let quad = quad?;
///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct FromSliceTriGReader<'a> {
    inner: FromSliceIterator<'a, TriGRecognizer>,
}

impl<'a> FromSliceTriGReader<'a> {
    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TriGParser;
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut reader = TriGParser::new().parse_slice(file);
    /// assert_eq!(reader.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// reader.next().unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     reader.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn prefixes(&self) -> TriGPrefixesIter<'_> {
        TriGPrefixesIter {
            inner: self.inner.parser.context.prefixes(),
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// use oxttl::TriGParser;
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut reader = TriGParser::new().parse_slice(file);
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

impl<'a> Iterator for FromSliceTriGReader<'a> {
    type Item = Result<Quad, TurtleSyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// Parses a TriG file by using a low-level API. Can be built using [`TriGParser::parse`].
///
/// Count the number of people:
/// ```
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
/// use oxttl::TriGParser;
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
    parser: Parser<Vec<u8>, TriGRecognizer>,
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
    pub fn read_next(&mut self) -> Option<Result<Quad, TurtleSyntaxError>> {
        self.parser.read_next()
    }

    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TriGParser;
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut reader = TriGParser::new().parse();
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
    pub fn prefixes(&self) -> TriGPrefixesIter<'_> {
        TriGPrefixesIter {
            inner: self.parser.context.prefixes(),
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// use oxttl::TriGParser;
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut reader = TriGParser::new().parse();
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
/// See [`LowLevelTriGReader::prefixes`].
pub struct TriGPrefixesIter<'a> {
    inner: Iter<'a, String, Iri<String>>,
}

impl<'a> Iterator for TriGPrefixesIter<'a> {
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

/// A [TriG](https://www.w3.org/TR/trig/) serializer.
///
/// Support for [TriG-star](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#trig-star) is available behind the `rdf-star` feature.
///
/// ```
/// use oxrdf::{NamedNodeRef, QuadRef};
/// use oxttl::TriGSerializer;
///
/// let mut writer = TriGSerializer::new()
///     .with_prefix("schema", "http://schema.org/")?
///     .serialize_to_write(Vec::new());
/// writer.write_quad(QuadRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
///     NamedNodeRef::new("http://example.com")?,
/// ))?;
/// assert_eq!(
///     b"@prefix schema: <http://schema.org/> .\n<http://example.com> {\n\t<http://example.com#me> a schema:Person .\n}\n",
///     writer.finish()?.as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default, Clone)]
#[must_use]
pub struct TriGSerializer {
    prefixes: BTreeMap<String, String>,
}

impl TriGSerializer {
    /// Builds a new [`TriGSerializer`].
    #[inline]
    pub fn new() -> Self {
        Self {
            prefixes: BTreeMap::new(),
        }
    }

    #[inline]
    pub fn with_prefix(
        mut self,
        prefix_name: impl Into<String>,
        prefix_iri: impl Into<String>,
    ) -> Result<Self, IriParseError> {
        self.prefixes.insert(
            Iri::parse(prefix_iri.into())?.into_inner(),
            prefix_name.into(),
        );
        Ok(self)
    }

    /// Writes a TriG file to a [`Write`] implementation.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, QuadRef};
    /// use oxttl::TriGSerializer;
    ///
    /// let mut writer = TriGSerializer::new()
    ///     .with_prefix("schema", "http://schema.org/")?
    ///     .serialize_to_write(Vec::new());
    /// writer.write_quad(QuadRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    ///     NamedNodeRef::new("http://example.com")?,
    /// ))?;
    /// assert_eq!(
    ///     b"@prefix schema: <http://schema.org/> .\n<http://example.com> {\n\t<http://example.com#me> a schema:Person .\n}\n",
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
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut writer = TriGSerializer::new()
    ///     .with_prefix("schema", "http://schema.org/")?
    ///     .serialize_to_tokio_async_write(Vec::new());
    /// writer
    ///     .write_quad(QuadRef::new(
    ///         NamedNodeRef::new_unchecked("http://example.com#me"),
    ///         NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
    ///         NamedNodeRef::new_unchecked("http://schema.org/Person"),
    ///         NamedNodeRef::new_unchecked("http://example.com"),
    ///     ))
    ///     .await?;
    /// assert_eq!(
    ///     b"@prefix schema: <http://schema.org/> .\n<http://example.com> {\n\t<http://example.com#me> a schema:Person .\n}\n",
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
    /// let mut writer = TriGSerializer::new()
    ///     .with_prefix("schema", "http://schema.org/")?
    ///     .serialize();
    /// writer.write_quad(
    ///     QuadRef::new(
    ///         NamedNodeRef::new("http://example.com#me")?,
    ///         NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
    ///         NamedNodeRef::new("http://schema.org/Person")?,
    ///         NamedNodeRef::new("http://example.com")?,
    ///     ),
    ///     &mut buf,
    /// )?;
    /// writer.finish(&mut buf)?;
    /// assert_eq!(
    ///     b"@prefix schema: <http://schema.org/> .\n<http://example.com> {\n\t<http://example.com#me> a schema:Person .\n}\n",
    ///     buf.as_slice()
    /// );
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn serialize(self) -> LowLevelTriGWriter {
        LowLevelTriGWriter {
            prefixes: self.prefixes,
            prelude_written: false,
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
/// let mut writer = TriGSerializer::new()
///     .with_prefix("schema", "http://schema.org/")?
///     .serialize_to_write(Vec::new());
/// writer.write_quad(QuadRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
///     NamedNodeRef::new("http://example.com")?,
/// ))?;
/// assert_eq!(
///     b"@prefix schema: <http://schema.org/> .\n<http://example.com> {\n\t<http://example.com#me> a schema:Person .\n}\n",
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
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let mut writer = TriGSerializer::new()
///     .with_prefix("schema", "http://schema.org/")?
///     .serialize_to_tokio_async_write(Vec::new());
/// writer
///     .write_quad(QuadRef::new(
///         NamedNodeRef::new_unchecked("http://example.com#me"),
///         NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
///         NamedNodeRef::new_unchecked("http://schema.org/Person"),
///         NamedNodeRef::new_unchecked("http://example.com"),
///     ))
///     .await?;
/// assert_eq!(
///     b"@prefix schema: <http://schema.org/> .\n<http://example.com> {\n\t<http://example.com#me> a schema:Person .\n}\n",
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
/// let mut writer = TriGSerializer::new()
///     .with_prefix("schema", "http://schema.org/")?
///     .serialize();
/// writer.write_quad(
///     QuadRef::new(
///         NamedNodeRef::new("http://example.com#me")?,
///         NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///         NamedNodeRef::new("http://schema.org/Person")?,
///         NamedNodeRef::new("http://example.com")?,
///     ),
///     &mut buf,
/// )?;
/// writer.finish(&mut buf)?;
/// assert_eq!(
///     b"@prefix schema: <http://schema.org/> .\n<http://example.com> {\n\t<http://example.com#me> a schema:Person .\n}\n",
///     buf.as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct LowLevelTriGWriter {
    prefixes: BTreeMap<String, String>,
    prelude_written: bool,
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
        if !self.prelude_written {
            self.prelude_written = true;
            for (prefix_iri, prefix_name) in &self.prefixes {
                writeln!(write, "@prefix {prefix_name}: <{prefix_iri}> .")?;
            }
        }
        let q = q.into();
        if q.graph_name == self.current_graph_name.as_ref() {
            if let Some((current_subject, current_predicate)) =
                self.current_subject_predicate.take()
            {
                if q.subject == current_subject.as_ref() {
                    if q.predicate == current_predicate {
                        self.current_subject_predicate = Some((current_subject, current_predicate));
                        write!(write, " , {}", self.term(q.object))
                    } else {
                        self.current_subject_predicate =
                            Some((current_subject, q.predicate.into_owned()));
                        writeln!(write, " ;")?;
                        if !self.current_graph_name.is_default_graph() {
                            write!(write, "\t")?;
                        }
                        write!(
                            write,
                            "\t{} {}",
                            self.predicate(q.predicate),
                            self.term(q.object)
                        )
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
                        self.term(q.subject),
                        self.predicate(q.predicate),
                        self.term(q.object)
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
                    self.term(q.subject),
                    self.predicate(q.predicate),
                    self.term(q.object)
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
            match self.current_graph_name.as_ref() {
                GraphNameRef::NamedNode(g) => {
                    writeln!(write, "{} {{", self.term(g))?;
                    write!(write, "\t")?;
                }
                GraphNameRef::BlankNode(g) => {
                    writeln!(write, "{} {{", self.term(g))?;
                    write!(write, "\t")?;
                }
                GraphNameRef::DefaultGraph => (),
            }

            write!(
                write,
                "{} {} {}",
                self.term(q.subject),
                self.predicate(q.predicate),
                self.term(q.object)
            )
        }
    }

    fn predicate<'a>(&'a self, named_node: impl Into<NamedNodeRef<'a>>) -> TurtlePredicate<'a> {
        TurtlePredicate {
            named_node: named_node.into(),
            prefixes: &self.prefixes,
        }
    }

    fn term<'a>(&'a self, term: impl Into<TermRef<'a>>) -> TurtleTerm<'a> {
        TurtleTerm {
            term: term.into(),
            prefixes: &self.prefixes,
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

struct TurtlePredicate<'a> {
    named_node: NamedNodeRef<'a>,
    prefixes: &'a BTreeMap<String, String>,
}

impl<'a> fmt::Display for TurtlePredicate<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.named_node == rdf::TYPE {
            f.write_str("a")
        } else {
            TurtleTerm {
                term: self.named_node.into(),
                prefixes: self.prefixes,
            }
            .fmt(f)
        }
    }
}

struct TurtleTerm<'a> {
    term: TermRef<'a>,
    prefixes: &'a BTreeMap<String, String>,
}

impl<'a> fmt::Display for TurtleTerm<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.term {
            TermRef::NamedNode(v) => {
                for (prefix_iri, prefix_name) in self.prefixes {
                    if let Some(local_name) = v.as_str().strip_prefix(prefix_iri) {
                        if let Some(escaped_local_name) = escape_local_name(local_name) {
                            return write!(f, "{prefix_name}:{escaped_local_name}");
                        }
                    }
                }
                write!(f, "{v}")
            }
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
                    f.write_str(value)
                } else if v.is_plain() {
                    write!(f, "{v}")
                } else {
                    write!(
                        f,
                        "{}^^{}",
                        LiteralRef::new_simple_literal(v.value()),
                        TurtleTerm {
                            term: v.datatype().into(),
                            prefixes: self.prefixes
                        }
                    )
                }
            }
            #[cfg(feature = "rdf-star")]
            TermRef::Triple(t) => {
                write!(
                    f,
                    "<< {} {} {} >>",
                    TurtleTerm {
                        term: t.subject.as_ref().into(),
                        prefixes: self.prefixes
                    },
                    TurtleTerm {
                        term: t.predicate.as_ref().into(),
                        prefixes: self.prefixes
                    },
                    TurtleTerm {
                        term: t.object.as_ref(),
                        prefixes: self.prefixes
                    }
                )
            }
        }
    }
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

fn escape_local_name(value: &str) -> Option<String> {
    // TODO: PLX
    // [168s] 	PN_LOCAL 	::= 	(PN_CHARS_U | ':' | [0-9] | PLX) ((PN_CHARS | '.' | ':' | PLX)* (PN_CHARS | ':' | PLX))?
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars();
    let first = chars.next()?;
    if N3Lexer::is_possible_pn_chars_u(first) || first == ':' || first.is_ascii_digit() {
        output.push(first);
    } else if can_be_escaped_in_local_name(first) {
        output.push('\\');
        output.push(first);
    } else {
        return None;
    }

    while let Some(c) = chars.next() {
        if N3Lexer::is_possible_pn_chars(c) || c == ':' || (c == '.' && !chars.as_str().is_empty())
        {
            output.push(c);
        } else if can_be_escaped_in_local_name(c) {
            output.push('\\');
            output.push(c);
        } else {
            return None;
        }
    }

    Some(output)
}

fn can_be_escaped_in_local_name(c: char) -> bool {
    matches!(
        c,
        '_' | '~'
            | '.'
            | '-'
            | '!'
            | '$'
            | '&'
            | '\''
            | '('
            | ')'
            | '*'
            | '+'
            | ','
            | ';'
            | '='
            | '/'
            | '?'
            | '#'
            | '@'
            | '%'
    )
}

#[cfg(test)]
#[allow(clippy::panic_in_result_fn)]
mod tests {
    use super::*;
    use oxrdf::BlankNodeRef;

    #[test]
    fn test_write() -> io::Result<()> {
        let mut writer = TriGSerializer::new()
            .with_prefix("ex", "http://example.com/")
            .unwrap()
            .serialize_to_write(Vec::new());
        writer.write_quad(QuadRef::new(
            NamedNodeRef::new_unchecked("http://example.com/s"),
            NamedNodeRef::new_unchecked("http://example.com/p"),
            NamedNodeRef::new_unchecked("http://example.com/o."),
            NamedNodeRef::new_unchecked("http://example.com/g"),
        ))?;
        writer.write_quad(QuadRef::new(
            NamedNodeRef::new_unchecked("http://example.com/s"),
            NamedNodeRef::new_unchecked("http://example.com/p"),
            NamedNodeRef::new_unchecked("http://example.com/o{o}"),
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
            NamedNodeRef::new_unchecked("http://example.org/p2"),
            LiteralRef::new_typed_literal("false", xsd::BOOLEAN),
            NamedNodeRef::new_unchecked("http://example.com/g2"),
        ))?;
        assert_eq!(
            String::from_utf8(writer.finish()?).unwrap(),
            "@prefix ex: <http://example.com/> .\nex:g {\n\tex:s ex:p ex:o\\. , <http://example.com/o{o}> , \"foo\" ;\n\t\tex:p2 \"foo\"@en .\n\t_:b ex:p2 _:b2 .\n}\n_:b ex:p2 true .\nex:g2 {\n\t_:b <http://example.org/p2> false .\n}\n"
        );
        Ok(())
    }
}
