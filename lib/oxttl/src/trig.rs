//! A [TriG](https://www.w3.org/TR/trig/) streaming parser implemented by [`TriGParser`]
//! and a serializer implemented by [`TriGSerializer`].

use crate::lexer::N3Lexer;
use crate::terse::TriGRecognizer;
#[cfg(feature = "async-tokio")]
use crate::toolkit::TokioAsyncReaderIterator;
use crate::toolkit::{Parser, ReaderIterator, SliceIterator, TurtleParseError, TurtleSyntaxError};
use oxiri::{Iri, IriParseError};
use oxrdf::vocab::{rdf, xsd};
use oxrdf::{
    GraphName, GraphNameRef, LiteralRef, NamedNode, NamedNodeRef, NamedOrBlankNode, Quad, QuadRef,
    TermRef,
};
use std::borrow::Cow;
use std::collections::hash_map::Iter;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::io::{self, Read, Write};
#[cfg(feature = "async-tokio")]
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

/// A [TriG](https://www.w3.org/TR/trig/) streaming parser.
///
/// Count the number of people:
/// ```
/// use oxrdf::NamedNodeRef;
/// use oxrdf::vocab::rdf;
/// use oxttl::TriGParser;
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
/// for quad in TriGParser::new().for_reader(file.as_bytes()) {
///     let quad = quad?;
///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default, Clone)]
#[must_use]
pub struct TriGParser {
    lenient: bool,
    base: Option<Iri<String>>,
    prefixes: HashMap<String, Iri<String>>,
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

    /// Parses a TriG file from a [`Read`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::NamedNodeRef;
    /// use oxrdf::vocab::rdf;
    /// use oxttl::TriGParser;
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
    /// for quad in TriGParser::new().for_reader(file.as_bytes()) {
    ///     let quad = quad?;
    ///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_reader<R: Read>(self, reader: R) -> ReaderTriGParser<R> {
        ReaderTriGParser {
            inner: self.low_level().parser.for_reader(reader),
        }
    }

    /// Parses a TriG file from a [`AsyncRead`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use oxrdf::NamedNodeRef;
    /// use oxrdf::vocab::rdf;
    /// use oxttl::TriGParser;
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
    /// let mut parser = TriGParser::new().for_tokio_async_reader(file.as_bytes());
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
    ) -> TokioAsyncReaderTriGParser<R> {
        TokioAsyncReaderTriGParser {
            inner: self.low_level().parser.for_tokio_async_reader(reader),
        }
    }

    /// Parses a TriG file from a byte slice.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::NamedNodeRef;
    /// use oxrdf::vocab::rdf;
    /// use oxttl::TriGParser;
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
    /// for quad in TriGParser::new().for_slice(file) {
    ///     let quad = quad?;
    ///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_slice(self, slice: &(impl AsRef<[u8]> + ?Sized)) -> SliceTriGParser<'_> {
        SliceTriGParser {
            inner: TriGRecognizer::new_parser(
                slice.as_ref(),
                true,
                true,
                self.lenient,
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
    /// use oxrdf::NamedNodeRef;
    /// use oxrdf::vocab::rdf;
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
    /// let mut parser = TriGParser::new().low_level();
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
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn low_level(self) -> LowLevelTriGParser {
        LowLevelTriGParser {
            parser: TriGRecognizer::new_parser(
                Vec::new(),
                false,
                true,
                self.lenient,
                self.base,
                self.prefixes,
            ),
        }
    }
}

/// Parses a TriG file from a [`Read`] implementation.
///
/// Can be built using [`TriGParser::for_reader`].
///
/// Count the number of people:
/// ```
/// use oxrdf::NamedNodeRef;
/// use oxrdf::vocab::rdf;
/// use oxttl::TriGParser;
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
/// for quad in TriGParser::new().for_reader(file.as_bytes()) {
///     let quad = quad?;
///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct ReaderTriGParser<R: Read> {
    inner: ReaderIterator<R, TriGRecognizer>,
}

impl<R: Read> ReaderTriGParser<R> {
    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TriGParser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = TriGParser::new().for_reader(file.as_bytes());
    /// assert_eq!(parser.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// parser.next().unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// //
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
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
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = TriGParser::new().for_reader(file.as_bytes());
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

impl<R: Read> Iterator for ReaderTriGParser<R> {
    type Item = Result<Quad, TurtleParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// Parses a TriG file from a [`AsyncRead`] implementation.
///
/// Can be built using [`TriGParser::for_tokio_async_reader`].
///
/// Count the number of people:
/// ```
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use oxrdf::NamedNodeRef;
/// use oxrdf::vocab::rdf;
/// use oxttl::TriGParser;
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
/// let mut parser = TriGParser::new().for_tokio_async_reader(file.as_bytes());
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
pub struct TokioAsyncReaderTriGParser<R: AsyncRead + Unpin> {
    inner: TokioAsyncReaderIterator<R, TriGRecognizer>,
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> TokioAsyncReaderTriGParser<R> {
    /// Reads the next triple or returns `None` if the file is finished.
    pub async fn next(&mut self) -> Option<Result<Quad, TurtleParseError>> {
        self.inner.next().await
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
    /// use oxttl::TriGParser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = TriGParser::new().for_tokio_async_reader(file.as_bytes());
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
    pub fn prefixes(&self) -> TriGPrefixesIter<'_> {
        TriGPrefixesIter {
            inner: self.inner.parser.context.prefixes(),
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use oxttl::TriGParser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = TriGParser::new().for_tokio_async_reader(file.as_bytes());
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

/// Parses a TriG file from a byte slice.
///
/// Can be built using [`TriGParser::for_slice`].
///
/// Count the number of people:
/// ```
/// use oxrdf::NamedNodeRef;
/// use oxrdf::vocab::rdf;
/// use oxttl::TriGParser;
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
/// for quad in TriGParser::new().for_slice(file) {
///     let quad = quad?;
///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct SliceTriGParser<'a> {
    inner: SliceIterator<'a, TriGRecognizer>,
}

impl SliceTriGParser<'_> {
    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TriGParser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = TriGParser::new().for_slice(file);
    /// assert_eq!(parser.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// parser.next().unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// //
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
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
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = TriGParser::new().for_slice(file);
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

impl Iterator for SliceTriGParser<'_> {
    type Item = Result<Quad, TurtleSyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// Parses a TriG file by using a low-level API.
///
/// Can be built using [`TriGParser::low_level`].
///
/// Count the number of people:
/// ```
/// use oxrdf::NamedNodeRef;
/// use oxrdf::vocab::rdf;
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
/// let mut parser = TriGParser::new().low_level();
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
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct LowLevelTriGParser {
    parser: Parser<Vec<u8>, TriGRecognizer>,
}

impl LowLevelTriGParser {
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

    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::TriGParser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = TriGParser::new().low_level();
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
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = TriGParser::new().low_level();
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
/// See [`LowLevelTriGParser::prefixes`].
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
/// ```
/// use oxrdf::{NamedNodeRef, QuadRef};
/// use oxrdf::vocab::rdf;
/// use oxttl::TriGSerializer;
///
/// let mut serializer = TriGSerializer::new()
///     .with_prefix("schema", "http://schema.org/")?
///     .for_writer(Vec::new());
/// serializer.serialize_quad(QuadRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     rdf::TYPE,
///     NamedNodeRef::new("http://schema.org/Person")?,
///     NamedNodeRef::new("http://example.com")?,
/// ))?;
/// assert_eq!(
///     b"@prefix schema: <http://schema.org/> .\n<http://example.com> {\n\t<http://example.com#me> a schema:Person .\n}\n",
///     serializer.finish()?.as_slice()
/// );
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default, Clone)]
#[must_use]
pub struct TriGSerializer {
    base_iri: Option<Iri<String>>,
    prefixes: BTreeMap<String, String>,
}

impl TriGSerializer {
    /// Builds a new [`TriGSerializer`].
    #[inline]
    pub fn new() -> Self {
        Self {
            base_iri: None,
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
            prefix_name.into(),
            Iri::parse(prefix_iri.into())?.into_inner(),
        );
        Ok(self)
    }

    /// Adds a base IRI to the serialization.
    ///
    /// ```
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::{NamedNodeRef, QuadRef};
    /// use oxttl::TriGSerializer;
    ///
    /// let mut serializer = TriGSerializer::new()
    ///     .with_base_iri("http://example.com")?
    ///     .with_prefix("ex", "http://example.com/ns#")?
    ///     .for_writer(Vec::new());
    /// serializer.serialize_quad(QuadRef::new(
    ///     NamedNodeRef::new("http://example.com/me")?,
    ///     rdf::TYPE,
    ///     NamedNodeRef::new("http://example.com/ns#Person")?,
    ///     NamedNodeRef::new("http://example.com")?,
    /// ))?;
    /// assert_eq!(
    ///     b"@base <http://example.com> .\n@prefix ex: </ns#> .\n<> {\n\t</me> a ex:Person .\n}\n",
    ///     serializer.finish()?.as_slice()
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_base_iri(mut self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        self.base_iri = Some(Iri::parse(base_iri.into())?);
        Ok(self)
    }

    /// Writes a TriG file to a [`Write`] implementation.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, QuadRef};
    /// use oxrdf::vocab::rdf;
    /// use oxttl::TriGSerializer;
    ///
    /// let mut serializer = TriGSerializer::new()
    ///     .with_prefix("schema", "http://schema.org/")?
    ///     .for_writer(Vec::new());
    /// serializer.serialize_quad(QuadRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     rdf::TYPE,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    ///     NamedNodeRef::new("http://example.com")?,
    /// ))?;
    /// assert_eq!(
    ///     b"@prefix schema: <http://schema.org/> .\n<http://example.com> {\n\t<http://example.com#me> a schema:Person .\n}\n",
    ///     serializer.finish()?.as_slice()
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_writer<W: Write>(self, writer: W) -> WriterTriGSerializer<W> {
        WriterTriGSerializer {
            writer,
            low_level_writer: self.low_level(),
        }
    }

    /// Writes a TriG file to a [`AsyncWrite`] implementation.
    ///
    /// ```
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use oxrdf::{NamedNodeRef, QuadRef};
    /// use oxrdf::vocab::rdf;
    /// use oxttl::TriGSerializer;
    ///
    /// let mut serializer = TriGSerializer::new()
    ///     .with_prefix("schema", "http://schema.org/")?
    ///     .for_tokio_async_writer(Vec::new());
    /// serializer
    ///     .serialize_quad(QuadRef::new(
    ///         NamedNodeRef::new("http://example.com#me")?,
    ///         rdf::TYPE,
    ///         NamedNodeRef::new("http://schema.org/Person")?,
    ///         NamedNodeRef::new("http://example.com")?,
    ///     ))
    ///     .await?;
    /// assert_eq!(
    ///     b"@prefix schema: <http://schema.org/> .\n<http://example.com> {\n\t<http://example.com#me> a schema:Person .\n}\n",
    ///     serializer.finish().await?.as_slice()
    /// );
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async-tokio")]
    pub fn for_tokio_async_writer<W: AsyncWrite + Unpin>(
        self,
        writer: W,
    ) -> TokioAsyncWriterTriGSerializer<W> {
        TokioAsyncWriterTriGSerializer {
            writer,
            low_level_writer: self.low_level(),
            buffer: Vec::new(),
        }
    }

    /// Builds a low-level TriG writer.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, QuadRef};
    /// use oxrdf::vocab::rdf;
    /// use oxttl::TriGSerializer;
    ///
    /// let mut buf = Vec::new();
    /// let mut serializer = TriGSerializer::new()
    ///     .with_prefix("schema", "http://schema.org/")?
    ///     .low_level();
    /// serializer.serialize_quad(
    ///     QuadRef::new(
    ///         NamedNodeRef::new("http://example.com#me")?,
    ///         rdf::TYPE,
    ///         NamedNodeRef::new("http://schema.org/Person")?,
    ///         NamedNodeRef::new("http://example.com")?,
    ///     ),
    ///     &mut buf,
    /// )?;
    /// serializer.finish(&mut buf)?;
    /// assert_eq!(
    ///     b"@prefix schema: <http://schema.org/> .\n<http://example.com> {\n\t<http://example.com#me> a schema:Person .\n}\n",
    ///     buf.as_slice()
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn low_level(self) -> LowLevelTriGSerializer {
        // We sort prefixes by decreasing length
        let mut prefixes = self.prefixes.into_iter().collect::<Vec<_>>();
        prefixes.sort_unstable_by(|(_, l), (_, r)| r.len().cmp(&l.len()));
        LowLevelTriGSerializer {
            prefixes,
            base_iri: self.base_iri,
            prelude_written: false,
            current_graph_name: GraphName::DefaultGraph,
            current_subject_predicate: None,
        }
    }
}

/// Writes a TriG file to a [`Write`] implementation.
///
/// Can be built using [`TriGSerializer::for_writer`].
///
/// ```
/// use oxrdf::{NamedNodeRef, QuadRef};
/// use oxrdf::vocab::rdf;
/// use oxttl::TriGSerializer;
///
/// let mut serializer = TriGSerializer::new()
///     .with_prefix("schema", "http://schema.org/")?
///     .for_writer(Vec::new());
/// serializer.serialize_quad(QuadRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     rdf::TYPE,
///     NamedNodeRef::new("http://schema.org/Person")?,
///     NamedNodeRef::new("http://example.com")?,
/// ))?;
/// assert_eq!(
///     b"@prefix schema: <http://schema.org/> .\n<http://example.com> {\n\t<http://example.com#me> a schema:Person .\n}\n",
///     serializer.finish()?.as_slice()
/// );
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct WriterTriGSerializer<W: Write> {
    writer: W,
    low_level_writer: LowLevelTriGSerializer,
}

impl<W: Write> WriterTriGSerializer<W> {
    /// Writes an extra quad.
    pub fn serialize_quad<'a>(&mut self, q: impl Into<QuadRef<'a>>) -> io::Result<()> {
        self.low_level_writer.serialize_quad(q, &mut self.writer)
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub fn finish(mut self) -> io::Result<W> {
        self.low_level_writer.finish(&mut self.writer)?;
        Ok(self.writer)
    }
}

/// Writes a TriG file to a [`AsyncWrite`] implementation.
///
/// Can be built using [`TriGSerializer::for_tokio_async_writer`].
///
/// ```
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use oxrdf::{NamedNodeRef, QuadRef};
/// use oxrdf::vocab::rdf;
/// use oxttl::TriGSerializer;
///
/// let mut serializer = TriGSerializer::new()
///     .with_prefix("schema", "http://schema.org/")?
///     .for_tokio_async_writer(Vec::new());
/// serializer
///     .serialize_quad(QuadRef::new(
///         NamedNodeRef::new("http://example.com#me")?,
///         rdf::TYPE,
///         NamedNodeRef::new("http://schema.org/Person")?,
///         NamedNodeRef::new("http://example.com")?,
///     ))
///     .await?;
/// assert_eq!(
///     b"@prefix schema: <http://schema.org/> .\n<http://example.com> {\n\t<http://example.com#me> a schema:Person .\n}\n",
///     serializer.finish().await?.as_slice()
/// );
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async-tokio")]
#[must_use]
pub struct TokioAsyncWriterTriGSerializer<W: AsyncWrite + Unpin> {
    writer: W,
    low_level_writer: LowLevelTriGSerializer,
    buffer: Vec<u8>,
}

#[cfg(feature = "async-tokio")]
impl<W: AsyncWrite + Unpin> TokioAsyncWriterTriGSerializer<W> {
    /// Writes an extra quad.
    pub async fn serialize_quad<'a>(&mut self, q: impl Into<QuadRef<'a>>) -> io::Result<()> {
        self.low_level_writer.serialize_quad(q, &mut self.buffer)?;
        self.writer.write_all(&self.buffer).await?;
        self.buffer.clear();
        Ok(())
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub async fn finish(mut self) -> io::Result<W> {
        self.low_level_writer.finish(&mut self.buffer)?;
        self.writer.write_all(&self.buffer).await?;
        self.buffer.clear();
        Ok(self.writer)
    }
}

/// Writes a TriG file by using a low-level API.
///
/// Can be built using [`TriGSerializer::low_level`].
///
/// ```
/// use oxrdf::{NamedNodeRef, QuadRef};
/// use oxrdf::vocab::rdf;
/// use oxttl::TriGSerializer;
///
/// let mut buf = Vec::new();
/// let mut serializer = TriGSerializer::new()
///     .with_prefix("schema", "http://schema.org/")?
///     .low_level();
/// serializer.serialize_quad(
///     QuadRef::new(
///         NamedNodeRef::new("http://example.com#me")?,
///         rdf::TYPE,
///         NamedNodeRef::new("http://schema.org/Person")?,
///         NamedNodeRef::new("http://example.com")?,
///     ),
///     &mut buf,
/// )?;
/// serializer.finish(&mut buf)?;
/// assert_eq!(
///     b"@prefix schema: <http://schema.org/> .\n<http://example.com> {\n\t<http://example.com#me> a schema:Person .\n}\n",
///     buf.as_slice()
/// );
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct LowLevelTriGSerializer {
    prefixes: Vec<(String, String)>,
    base_iri: Option<Iri<String>>,
    prelude_written: bool,
    current_graph_name: GraphName,
    current_subject_predicate: Option<(NamedOrBlankNode, NamedNode)>,
}

impl LowLevelTriGSerializer {
    /// Writes an extra quad.
    pub fn serialize_quad<'a>(
        &mut self,
        q: impl Into<QuadRef<'a>>,
        mut writer: impl Write,
    ) -> io::Result<()> {
        if !self.prelude_written {
            self.prelude_written = true;
            if let Some(base_iri) = &self.base_iri {
                writeln!(writer, "@base <{base_iri}> .")?;
            }
            for (prefix_name, prefix_iri) in &self.prefixes {
                writeln!(
                    writer,
                    "@prefix {prefix_name}: <{}> .",
                    relative_iri(prefix_iri, &self.base_iri)
                )?;
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
                        write!(writer, " , {}", self.term(q.object))
                    } else {
                        self.current_subject_predicate =
                            Some((current_subject, q.predicate.into_owned()));
                        writeln!(writer, " ;")?;
                        if !self.current_graph_name.is_default_graph() {
                            write!(writer, "\t")?;
                        }
                        write!(
                            writer,
                            "\t{} {}",
                            self.predicate(q.predicate),
                            self.term(q.object)
                        )
                    }
                } else {
                    self.current_subject_predicate =
                        Some((q.subject.into_owned(), q.predicate.into_owned()));
                    writeln!(writer, " .")?;
                    if !self.current_graph_name.is_default_graph() {
                        write!(writer, "\t")?;
                    }
                    write!(
                        writer,
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
                    write!(writer, "\t")?;
                }
                write!(
                    writer,
                    "{} {} {}",
                    self.term(q.subject),
                    self.predicate(q.predicate),
                    self.term(q.object)
                )
            }
        } else {
            if self.current_subject_predicate.is_some() {
                writeln!(writer, " .")?;
            }
            if !self.current_graph_name.is_default_graph() {
                writeln!(writer, "}}")?;
            }
            self.current_graph_name = q.graph_name.into_owned();
            self.current_subject_predicate =
                Some((q.subject.into_owned(), q.predicate.into_owned()));
            match self.current_graph_name.as_ref() {
                GraphNameRef::NamedNode(g) => {
                    writeln!(writer, "{} {{", self.term(g))?;
                    write!(writer, "\t")?;
                }
                GraphNameRef::BlankNode(g) => {
                    writeln!(writer, "{} {{", self.term(g))?;
                    write!(writer, "\t")?;
                }
                GraphNameRef::DefaultGraph => (),
            }

            write!(
                writer,
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
            base_iri: &self.base_iri,
        }
    }

    fn term<'a>(&'a self, term: impl Into<TermRef<'a>>) -> TurtleTerm<'a> {
        TurtleTerm {
            term: term.into(),
            prefixes: &self.prefixes,
            base_iri: &self.base_iri,
        }
    }

    /// Finishes to write the file.
    pub fn finish(&mut self, mut writer: impl Write) -> io::Result<()> {
        if self.current_subject_predicate.is_some() {
            writeln!(writer, " .")?;
        }
        if !self.current_graph_name.is_default_graph() {
            writeln!(writer, "}}")?;
        }
        Ok(())
    }
}

struct TurtlePredicate<'a> {
    named_node: NamedNodeRef<'a>,
    prefixes: &'a Vec<(String, String)>,
    base_iri: &'a Option<Iri<String>>,
}

impl fmt::Display for TurtlePredicate<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.named_node == rdf::TYPE {
            f.write_str("a")
        } else {
            TurtleTerm {
                term: self.named_node.into(),
                prefixes: self.prefixes,
                base_iri: self.base_iri,
            }
            .fmt(f)
        }
    }
}

struct TurtleTerm<'a> {
    term: TermRef<'a>,
    prefixes: &'a Vec<(String, String)>,
    base_iri: &'a Option<Iri<String>>,
}

impl fmt::Display for TurtleTerm<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.term {
            TermRef::NamedNode(v) => {
                for (prefix_name, prefix_iri) in self.prefixes {
                    if let Some(local_name) = v.as_str().strip_prefix(prefix_iri) {
                        if local_name.is_empty() {
                            return write!(f, "{prefix_name}:");
                        } else if let Some(escaped_local_name) = escape_local_name(local_name) {
                            return write!(f, "{prefix_name}:{escaped_local_name}");
                        }
                    }
                }
                write!(f, "<{}>", relative_iri(v.as_str(), self.base_iri))
            }
            TermRef::BlankNode(v) => write!(f, "{v}"),
            TermRef::Literal(v) => {
                let value = v.value();
                let is_plain = {
                    #[cfg(feature = "rdf-12")]
                    {
                        matches!(
                            v.datatype(),
                            xsd::STRING | rdf::LANG_STRING | rdf::DIR_LANG_STRING
                        )
                    }
                    #[cfg(not(feature = "rdf-12"))]
                    {
                        matches!(v.datatype(), xsd::STRING | rdf::LANG_STRING)
                    }
                };
                if is_plain {
                    write!(f, "{v}")
                } else {
                    let inline = match v.datatype() {
                        xsd::BOOLEAN => is_turtle_boolean(value),
                        xsd::INTEGER => is_turtle_integer(value),
                        xsd::DECIMAL => is_turtle_decimal(value),
                        xsd::DOUBLE => is_turtle_double(value),
                        _ => false,
                    };
                    if inline {
                        f.write_str(value)
                    } else {
                        write!(
                            f,
                            "{}^^{}",
                            LiteralRef::new_simple_literal(v.value()),
                            TurtleTerm {
                                term: v.datatype().into(),
                                prefixes: self.prefixes,
                                base_iri: self.base_iri,
                            }
                        )
                    }
                }
            }
            #[cfg(feature = "rdf-12")]
            TermRef::Triple(t) => {
                write!(
                    f,
                    "<<( {} {} {} )>>",
                    TurtleTerm {
                        term: t.subject.as_ref().into(),
                        prefixes: self.prefixes,
                        base_iri: self.base_iri,
                    },
                    TurtleTerm {
                        term: t.predicate.as_ref().into(),
                        prefixes: self.prefixes,
                        base_iri: self.base_iri,
                    },
                    TurtleTerm {
                        term: t.object.as_ref(),
                        prefixes: self.prefixes,
                        base_iri: self.base_iri,
                    }
                )
            }
        }
    }
}

fn relative_iri<'a>(iri: &'a str, base_iri: &Option<Iri<String>>) -> Cow<'a, str> {
    if let Some(base_iri) = base_iri {
        if let Ok(relative) = base_iri.relativize(&Iri::parse_unchecked(iri)) {
            return relative.into_inner().into();
        }
    }
    iri.into()
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
    while value.first().is_some_and(u8::is_ascii_digit) {
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
    while value.first().is_some_and(u8::is_ascii_digit) {
        value = &value[1..];
        with_before = true;
    }
    let mut with_after = false;
    if let Some(v) = value.strip_prefix(b".") {
        value = v;
        while value.first().is_some_and(u8::is_ascii_digit) {
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
#[expect(clippy::panic_in_result_fn)]
mod tests {
    use super::*;
    use oxrdf::BlankNodeRef;

    #[test]
    fn test_write() -> io::Result<()> {
        let mut serializer = TriGSerializer::new()
            .with_prefix("ex", "http://example.com/")
            .unwrap()
            .with_prefix("exl", "http://example.com/p/")
            .unwrap()
            .for_writer(Vec::new());
        serializer.serialize_quad(QuadRef::new(
            NamedNodeRef::new_unchecked("http://example.com/s"),
            NamedNodeRef::new_unchecked("http://example.com/p"),
            NamedNodeRef::new_unchecked("http://example.com/p/o."),
            NamedNodeRef::new_unchecked("http://example.com/g"),
        ))?;
        serializer.serialize_quad(QuadRef::new(
            NamedNodeRef::new_unchecked("http://example.com/s"),
            NamedNodeRef::new_unchecked("http://example.com/p"),
            NamedNodeRef::new_unchecked("http://example.com/o{o}"),
            NamedNodeRef::new_unchecked("http://example.com/g"),
        ))?;
        serializer.serialize_quad(QuadRef::new(
            NamedNodeRef::new_unchecked("http://example.com/s"),
            NamedNodeRef::new_unchecked("http://example.com/p"),
            NamedNodeRef::new_unchecked("http://example.com/"),
            NamedNodeRef::new_unchecked("http://example.com/g"),
        ))?;
        serializer.serialize_quad(QuadRef::new(
            NamedNodeRef::new_unchecked("http://example.com/s"),
            NamedNodeRef::new_unchecked("http://example.com/p"),
            LiteralRef::new_simple_literal("foo"),
            NamedNodeRef::new_unchecked("http://example.com/g"),
        ))?;
        serializer.serialize_quad(QuadRef::new(
            NamedNodeRef::new_unchecked("http://example.com/s"),
            NamedNodeRef::new_unchecked("http://example.com/p2"),
            LiteralRef::new_language_tagged_literal_unchecked("foo", "en"),
            NamedNodeRef::new_unchecked("http://example.com/g"),
        ))?;
        serializer.serialize_quad(QuadRef::new(
            BlankNodeRef::new_unchecked("b"),
            NamedNodeRef::new_unchecked("http://example.com/p2"),
            BlankNodeRef::new_unchecked("b2"),
            NamedNodeRef::new_unchecked("http://example.com/g"),
        ))?;
        serializer.serialize_quad(QuadRef::new(
            BlankNodeRef::new_unchecked("b"),
            NamedNodeRef::new_unchecked("http://example.com/p2"),
            LiteralRef::new_typed_literal("true", xsd::BOOLEAN),
            GraphNameRef::DefaultGraph,
        ))?;
        serializer.serialize_quad(QuadRef::new(
            BlankNodeRef::new_unchecked("b"),
            NamedNodeRef::new_unchecked("http://example.org/p2"),
            LiteralRef::new_typed_literal("false", xsd::BOOLEAN),
            NamedNodeRef::new_unchecked("http://example.com/g2"),
        ))?;
        assert_eq!(
            String::from_utf8(serializer.finish()?).unwrap(),
            "@prefix exl: <http://example.com/p/> .\n@prefix ex: <http://example.com/> .\nex:g {\n\tex:s ex:p exl:o\\. , <http://example.com/o{o}> , ex: , \"foo\" ;\n\t\tex:p2 \"foo\"@en .\n\t_:b ex:p2 _:b2 .\n}\n_:b ex:p2 true .\nex:g2 {\n\t_:b <http://example.org/p2> false .\n}\n"
        );
        Ok(())
    }
}
