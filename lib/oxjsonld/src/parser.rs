use crate::error::{JsonLdErrorCode, JsonLdParseError, JsonLdSyntaxError};
#[cfg(feature = "async-tokio")]
use json_event_parser::TokioAsyncReaderJsonParser;
use json_event_parser::{JsonEvent, ReaderJsonParser, SliceJsonParser};
use oxiri::{Iri, IriParseError};
use oxrdf::vocab::{rdf, xsd};
use oxrdf::{BlankNode, GraphName, Literal, NamedNode, NamedOrBlankNode, Quad};
use std::borrow::Cow;
use std::io::Read;
use std::marker::PhantomData;
use std::str;
#[cfg(feature = "async-tokio")]
use tokio::io::AsyncRead;

/// A [JSON-LD](https://www.w3.org/TR/rdf-syntax-grammar/) streaming parser.
///
/// It reads the file in streaming.
/// It does not keep data in memory except a stack for handling nested XML tags, and a set of all
/// seen `rdf:ID`s to detect duplicate ids and fail according to the specification.
///
/// Its performances are not optimized yet and hopefully could be significantly enhanced by reducing the
/// number of allocations and copies done by the parser.
///
/// Count the number of people:
/// ```
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
/// use oxrdfxml::JsonLdParser;
///
/// let file = br#"<?xml version="1.0"?>
/// <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:schema="http://schema.org/">
///  <rdf:Description rdf:about="http://example.com/foo">
///    <rdf:type rdf:resource="http://schema.org/Person" />
///    <schema:name>Foo</schema:name>
///  </rdf:Description>
///  <schema:Person rdf:about="http://example.com/bar" schema:name="Bar" />
/// </rdf:RDF>"#;
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for quad in JsonLdParser::new().for_reader(file.as_ref()) {
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
pub struct JsonLdParser {
    unchecked: bool,
    base: Option<Iri<String>>,
}

impl JsonLdParser {
    /// Builds a new [`JsonLdParser`].
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

    #[inline]
    pub fn with_base_iri(mut self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        self.base = Some(Iri::parse(base_iri.into())?);
        Ok(self)
    }

    /// Parses a JSON-LD file from a [`Read`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    /// use oxrdfxml::JsonLdParser;
    ///
    /// let file = br#"<?xml version="1.0"?>
    /// <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:schema="http://schema.org/">
    ///  <rdf:Description rdf:about="http://example.com/foo">
    ///    <rdf:type rdf:resource="http://schema.org/Person" />
    ///    <schema:name>Foo</schema:name>
    ///  </rdf:Description>
    ///  <schema:Person rdf:about="http://example.com/bar" schema:name="Bar" />
    /// </rdf:RDF>"#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// for quad in JsonLdParser::new().for_reader(file.as_ref()) {
    ///     let quad = quad?;
    ///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_reader<R: Read>(self, reader: R) -> ReaderJsonLdParser<R> {
        ReaderJsonLdParser {
            results: Vec::new(),
            errors: Vec::new(),
            inner: self.into_inner(),
            json_parser: ReaderJsonParser::new(reader),
        }
    }

    /// Parses a JSON-LD file from a [`AsyncRead`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    /// use oxrdfxml::JsonLdParser;
    ///
    /// let file = br#"<?xml version="1.0"?>
    /// <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:schema="http://schema.org/">
    ///   <rdf:Description rdf:about="http://example.com/foo">
    ///     <rdf:type rdf:resource="http://schema.org/Person" />
    ///     <schema:name>Foo</schema:name>
    ///   </rdf:Description>
    ///   <schema:Person rdf:about="http://example.com/bar" schema:name="Bar" />
    /// </rdf:RDF>"#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// let mut parser = JsonLdParser::new().for_tokio_async_reader(file.as_ref());
    /// while let Some(quad) = parser.next().await {
    ///     let quad = quad?;
    ///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
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
    ) -> TokioAsyncReaderJsonLdParser<R> {
        TokioAsyncReaderJsonLdParser {
            results: Vec::new(),
            errors: Vec::new(),
            inner: self.into_inner(),
            json_parser: TokioAsyncReaderJsonParser::new(reader),
        }
    }

    /// Parses a JSON-LD file from a byte slice.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    /// use oxrdfxml::JsonLdParser;
    ///
    /// let file = br#"<?xml version="1.0"?>
    /// <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:schema="http://schema.org/">
    ///  <rdf:Description rdf:about="http://example.com/foo">
    ///    <rdf:type rdf:resource="http://schema.org/Person" />
    ///    <schema:name>Foo</schema:name>
    ///  </rdf:Description>
    ///  <schema:Person rdf:about="http://example.com/bar" schema:name="Bar" />
    /// </rdf:RDF>"#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// for quad in JsonLdParser::new().for_slice(file) {
    ///     let quad = quad?;
    ///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_slice(self, slice: &[u8]) -> SliceJsonLdParser<'_> {
        SliceJsonLdParser {
            results: Vec::new(),
            errors: Vec::new(),
            inner: self.into_inner(),
            json_parser: SliceJsonParser::new(slice),
        }
    }

    fn into_inner(self) -> InternalJsonLdParser {
        InternalJsonLdParser {
            expansion: JsonLdExpansionConverter {
                state: vec![JsonLdExpansionState::General],
                is_end: false,
            },
            expended_events: Vec::new(),
            to_rdf: JsonLdToRdfConverter {
                stack: vec![JsonLdToRdfState::Graph(Some(GraphName::DefaultGraph))],
                unchecked: self.unchecked,
            },
        }
    }
}

/// Parses a JSON-LD file from a [`Read`] implementation.
///
/// Can be built using [`JsonLdParser::for_reader`].
///
/// Count the number of people:
/// ```
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
/// use oxrdfxml::JsonLdParser;
///
/// let file = br#"<?xml version="1.0"?>
/// <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:schema="http://schema.org/">
///  <rdf:Description rdf:about="http://example.com/foo">
///    <rdf:type rdf:resource="http://schema.org/Person" />
///    <schema:name>Foo</schema:name>
///  </rdf:Description>
///  <schema:Person rdf:about="http://example.com/bar" schema:name="Bar" />
/// </rdf:RDF>"#;
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for quad in JsonLdParser::new().for_reader(file.as_ref()) {
///     let quad = quad?;
///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct ReaderJsonLdParser<R: Read> {
    results: Vec<Quad>,
    errors: Vec<JsonLdSyntaxError>,
    inner: InternalJsonLdParser,
    json_parser: ReaderJsonParser<R>,
}

impl<R: Read> Iterator for ReaderJsonLdParser<R> {
    type Item = Result<Quad, JsonLdParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(error) = self.errors.pop() {
                return Some(Err(error.into()));
            } else if let Some(quad) = self.results.pop() {
                return Some(Ok(quad));
            } else if self.inner.expansion.is_end {
                return None;
            }
            if let Err(e) = self.parse_step() {
                return Some(Err(e));
            }
        }
    }
}

impl<R: Read> ReaderJsonLdParser<R> {
    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxrdfxml::JsonLdParser;
    ///
    /// let file = br#"<?xml version="1.0"?>
    /// <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:schema="http://schema.org/">
    ///  <rdf:Description rdf:about="http://example.com/foo">
    ///    <rdf:type rdf:resource="http://schema.org/Person" />
    ///    <schema:name>Foo</schema:name>
    ///  </rdf:Description>
    ///  <schema:Person rdf:about="http://example.com/bar" schema:name="Bar" />
    /// </rdf:RDF>"#;
    ///
    /// let mut parser = JsonLdParser::new().for_reader(file.as_ref());
    /// assert_eq!(parser.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// parser.next().unwrap()?; // We read the first quad
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [
    ///         ("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"),
    ///         ("schema", "http://schema.org/")
    ///     ]
    /// ); // There are now prefixes
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn prefixes(&self) -> JsonLdPrefixesIter<'_> {
        JsonLdPrefixesIter {
            lifetime: PhantomData,
            unchecked: self.inner.to_rdf.unchecked,
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// use oxrdfxml::JsonLdParser;
    ///
    /// let file = br#"<?xml version="1.0"?>
    /// <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xml:base="http://example.com/">
    ///  <rdf:Description rdf:about="foo">
    ///    <rdf:type rdf:resource="http://schema.org/Person" />
    ///  </rdf:Description>
    /// </rdf:RDF>"#;
    ///
    /// let mut parser = JsonLdParser::new().for_reader(file.as_ref());
    /// assert!(parser.base_iri().is_none()); // No base at the beginning because none has been given to the parser.
    ///
    /// parser.next().unwrap()?; // We read the first quad
    /// assert_eq!(parser.base_iri(), Some("http://example.com/")); // There is now a base IRI.
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn base_iri(&self) -> Option<&str> {
        todo!()
    }

    fn parse_step(&mut self) -> Result<(), JsonLdParseError> {
        self.inner.parse_event(
            self.json_parser.parse_next()?,
            &mut self.results,
            &mut self.errors,
        );
        Ok(())
    }
}

/// Parses a JSON-LD file from a [`AsyncRead`] implementation.
///
/// Can be built using [`JsonLdParser::for_tokio_async_reader`].
///
/// Count the number of people:
/// ```
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
/// use oxrdfxml::JsonLdParser;
///
/// let file = br#"<?xml version="1.0"?>
/// <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:schema="http://schema.org/">
///   <rdf:Description rdf:about="http://example.com/foo">
///     <rdf:type rdf:resource="http://schema.org/Person" />
///     <schema:name>Foo</schema:name>
///   </rdf:Description>
///   <schema:Person rdf:about="http://example.com/bar" schema:name="Bar" />
/// </rdf:RDF>"#;
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// let mut parser = JsonLdParser::new().for_tokio_async_reader(file.as_ref());
/// while let Some(quad) = parser.next().await {
///     let quad = quad?;
///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async-tokio")]
#[must_use]
pub struct TokioAsyncReaderJsonLdParser<R: AsyncRead + Unpin> {
    results: Vec<Quad>,
    errors: Vec<JsonLdSyntaxError>,
    inner: InternalJsonLdParser,
    json_parser: TokioAsyncReaderJsonParser<R>,
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> TokioAsyncReaderJsonLdParser<R> {
    /// Reads the next quad or returns `None` if the file is finished.
    pub async fn next(&mut self) -> Option<Result<Quad, JsonLdParseError>> {
        loop {
            if let Some(error) = self.errors.pop() {
                return Some(Err(error.into()));
            } else if let Some(quad) = self.results.pop() {
                return Some(Ok(quad));
            } else if self.inner.expansion.is_end {
                return None;
            }
            if let Err(e) = self.parse_step().await {
                return Some(Err(e));
            }
        }
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
    /// use oxrdfxml::JsonLdParser;
    ///
    /// let file = br#"<?xml version="1.0"?>
    /// <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:schema="http://schema.org/">
    ///  <rdf:Description rdf:about="http://example.com/foo">
    ///    <rdf:type rdf:resource="http://schema.org/Person" />
    ///    <schema:name>Foo</schema:name>
    ///  </rdf:Description>
    ///  <schema:Person rdf:about="http://example.com/bar" schema:name="Bar" />
    /// </rdf:RDF>"#;
    ///
    /// let mut parser = JsonLdParser::new().for_tokio_async_reader(file.as_ref());
    /// assert_eq!(parser.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// parser.next().await.unwrap()?; // We read the first quad
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [
    ///         ("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"),
    ///         ("schema", "http://schema.org/")
    ///     ]
    /// ); // There are now prefixes
    /// # Ok(())
    /// # }
    /// ```
    pub fn prefixes(&self) -> JsonLdPrefixesIter<'_> {
        JsonLdPrefixesIter {
            lifetime: PhantomData,
            unchecked: self.inner.unchecked,
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use oxrdfxml::JsonLdParser;
    ///
    /// let file = br#"<?xml version="1.0"?>
    /// <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xml:base="http://example.com/">
    ///  <rdf:Description rdf:about="foo">
    ///    <rdf:type rdf:resource="http://schema.org/Person" />
    ///  </rdf:Description>
    /// </rdf:RDF>"#;
    ///
    /// let mut parser = JsonLdParser::new().for_tokio_async_reader(file.as_ref());
    /// assert!(parser.base_iri().is_none()); // No base at the beginning because none has been given to the parser.
    ///
    /// parser.next().await.unwrap()?; // We read the first quad
    /// assert_eq!(parser.base_iri(), Some("http://example.com/")); // There is now a base IRI.
    /// # Ok(())
    /// # }
    /// ```
    pub fn base_iri(&self) -> Option<&str> {
        todo!()
    }

    async fn parse_step(&mut self) -> Result<(), JsonLdParseError> {
        self.inner.parse_event(
            self.json_parser.parse_next().await?,
            &mut self.results,
            &mut self.errors,
        );
        Ok(())
    }
}

/// Parses a JSON-LD file from a byte slice.
///
/// Can be built using [`JsonLdParser::for_slice`].
///
/// Count the number of people:
/// ```
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
/// use oxrdfxml::JsonLdParser;
///
/// let file = br#"<?xml version="1.0"?>
/// <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:schema="http://schema.org/">
///  <rdf:Description rdf:about="http://example.com/foo">
///    <rdf:type rdf:resource="http://schema.org/Person" />
///    <schema:name>Foo</schema:name>
///  </rdf:Description>
///  <schema:Person rdf:about="http://example.com/bar" schema:name="Bar" />
/// </rdf:RDF>"#;
///
/// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
/// let mut count = 0;
/// for quad in JsonLdParser::new().for_slice(file) {
///     let quad = quad?;
///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct SliceJsonLdParser<'a> {
    results: Vec<Quad>,
    errors: Vec<JsonLdSyntaxError>,
    inner: InternalJsonLdParser,
    json_parser: SliceJsonParser<'a>,
}

impl Iterator for SliceJsonLdParser<'_> {
    type Item = Result<Quad, JsonLdSyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(error) = self.errors.pop() {
                return Some(Err(error));
            } else if let Some(quad) = self.results.pop() {
                return Some(Ok(quad));
            } else if self.inner.expansion.is_end {
                return None;
            }
            if let Err(e) = self.parse_step() {
                // I/O errors can't happen
                return Some(Err(e));
            }
        }
    }
}

impl SliceJsonLdParser<'_> {
    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxrdfxml::JsonLdParser;
    ///
    /// let file = br#"<?xml version="1.0"?>
    /// <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:schema="http://schema.org/">
    ///  <rdf:Description rdf:about="http://example.com/foo">
    ///    <rdf:type rdf:resource="http://schema.org/Person" />
    ///    <schema:name>Foo</schema:name>
    ///  </rdf:Description>
    ///  <schema:Person rdf:about="http://example.com/bar" schema:name="Bar" />
    /// </rdf:RDF>"#;
    ///
    /// let mut parser = JsonLdParser::new().for_slice(file);
    /// assert_eq!(parser.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// parser.next().unwrap()?; // We read the first quad
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [
    ///         ("rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"),
    ///         ("schema", "http://schema.org/")
    ///     ]
    /// ); // There are now prefixes
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn prefixes(&self) -> JsonLdPrefixesIter<'_> {
        JsonLdPrefixesIter {
            lifetime: PhantomData,
            unchecked: self.inner.to_rdf.unchecked,
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// use oxrdfxml::JsonLdParser;
    ///
    /// let file = br#"<?xml version="1.0"?>
    /// <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xml:base="http://example.com/">
    ///  <rdf:Description rdf:about="foo">
    ///    <rdf:type rdf:resource="http://schema.org/Person" />
    ///  </rdf:Description>
    /// </rdf:RDF>"#;
    ///
    /// let mut parser = JsonLdParser::new().for_slice(file);
    /// assert!(parser.base_iri().is_none()); // No base at the beginning because none has been given to the parser.
    ///
    /// parser.next().unwrap()?; // We read the first quad
    /// assert_eq!(parser.base_iri(), Some("http://example.com/")); // There is now a base IRI.
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn base_iri(&self) -> Option<&str> {
        todo!()
    }

    fn parse_step(&mut self) -> Result<(), JsonLdSyntaxError> {
        self.inner.parse_event(
            self.json_parser.parse_next()?,
            &mut self.results,
            &mut self.errors,
        );
        Ok(())
    }
}

/// Iterator on the file prefixes.
///
/// See [`ReaderJsonLdParser::prefixes`].
pub struct JsonLdPrefixesIter<'a> {
    lifetime: PhantomData<&'a ()>,
    unchecked: bool,
}

impl<'a> Iterator for JsonLdPrefixesIter<'a> {
    type Item = (&'a str, &'a str);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        todo!()
    }
}

struct InternalJsonLdParser {
    expansion: JsonLdExpansionConverter,
    expended_events: Vec<JsonLdEvent>,
    to_rdf: JsonLdToRdfConverter,
}

impl InternalJsonLdParser {
    fn parse_event(
        &mut self,
        event: JsonEvent<'_>,
        results: &mut Vec<Quad>,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) {
        self.expansion
            .convert_event(event, &mut self.expended_events, errors);
        for event in self.expended_events.drain(..) {
            self.to_rdf.convert_event(event, results, errors);
        }
    }
}

enum JsonLdToRdfState {
    StartObject {
        types: Vec<NamedNode>,
    },
    Object(Option<NamedOrBlankNode>),
    Value {
        types: Vec<NamedNode>,
        value: Option<JsonLdValue>,
        language: Option<String>,
    },
    Property(Option<NamedNode>),
    Graph(Option<GraphName>),
}

struct JsonLdToRdfConverter {
    stack: Vec<JsonLdToRdfState>,
    unchecked: bool,
}

impl JsonLdToRdfConverter {
    fn convert_event(
        &mut self,
        event: JsonLdEvent,
        results: &mut Vec<Quad>,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) {
        let state = self.stack.pop().expect("Empty stack");
        match state {
            JsonLdToRdfState::StartObject { types } => match event {
                JsonLdEvent::Id(id) => {
                    let id = self.convert_named_or_blank_node(id, errors);
                    self.emit_quads_for_new_object(id.as_ref(), types, results);
                    self.stack.push(JsonLdToRdfState::Object(id))
                }
                JsonLdEvent::EndObject => {
                    let id = Some(BlankNode::default().into());
                    self.emit_quads_for_new_object(id.as_ref(), types, results);
                }
                JsonLdEvent::StartProperty(name) => {
                    let id = Some(BlankNode::default().into());
                    self.emit_quads_for_new_object(id.as_ref(), types, results);
                    self.stack.push(JsonLdToRdfState::Object(id));
                    self.stack.push(JsonLdToRdfState::Property(
                        self.convert_named_node(name, errors),
                    ));
                }
                JsonLdEvent::Value(value) => self.stack.push(JsonLdToRdfState::Value {
                    types,
                    value: Some(value),
                    language: None,
                }),
                JsonLdEvent::Language(language) => self.stack.push(JsonLdToRdfState::Value {
                    types,
                    value: None,
                    language: Some(language),
                }),
                JsonLdEvent::EndProperty | JsonLdEvent::StartObject { .. } => unreachable!(),
            },
            JsonLdToRdfState::Object(_) => match event {
                JsonLdEvent::Id(id) => {
                    errors.push(JsonLdSyntaxError::msg(
                        "Oxigraph JSON-LD parser does not support yet @id defined after properties",
                    ));
                }
                JsonLdEvent::EndObject => (),
                JsonLdEvent::StartProperty(name) => {
                    self.stack.push(JsonLdToRdfState::Property(
                        self.convert_named_node(name, errors),
                    ));
                }
                JsonLdEvent::StartObject { .. }
                | JsonLdEvent::Value(_)
                | JsonLdEvent::Language(_)
                | JsonLdEvent::EndProperty => unreachable!(),
            },
            JsonLdToRdfState::Value {
                types,
                value,
                language,
            } => match event {
                JsonLdEvent::Value(new_value) => self.stack.push(JsonLdToRdfState::Value {
                    types,
                    value: Some(new_value),
                    language,
                }),
                JsonLdEvent::Language(new_language) => self.stack.push(JsonLdToRdfState::Value {
                    types,
                    value,
                    language: Some(new_language),
                }),
                JsonLdEvent::EndObject => self.emit_quad_for_new_literal(
                    self.convert_literal(value, language, types, errors),
                    results,
                ),
                JsonLdEvent::StartProperty(_)
                | JsonLdEvent::StartObject { .. }
                | JsonLdEvent::EndProperty
                | JsonLdEvent::Id(_) => unreachable!(),
            },
            JsonLdToRdfState::Property(_) => match event {
                JsonLdEvent::StartObject { types } => {
                    self.stack.push(state);
                    self.stack.push(JsonLdToRdfState::StartObject {
                        types: types
                            .into_iter()
                            .filter_map(|t| self.convert_named_node(t, errors))
                            .collect(),
                    });
                }
                JsonLdEvent::EndProperty => (),
                JsonLdEvent::StartProperty(_)
                | JsonLdEvent::Id(_)
                | JsonLdEvent::EndObject
                | JsonLdEvent::Value(_)
                | JsonLdEvent::Language(_) => unreachable!(),
            },
            JsonLdToRdfState::Graph(_) => match event {
                JsonLdEvent::StartObject { types } => {
                    self.stack.push(state);
                    self.stack.push(JsonLdToRdfState::StartObject {
                        types: types
                            .into_iter()
                            .filter_map(|t| self.convert_named_node(t, errors))
                            .collect(),
                    });
                }
                JsonLdEvent::StartProperty(_)
                | JsonLdEvent::EndProperty
                | JsonLdEvent::Id(_)
                | JsonLdEvent::EndObject
                | JsonLdEvent::Value(_)
                | JsonLdEvent::Language(_) => unreachable!(),
            },
        }
    }

    fn emit_quads_for_new_object(
        &self,
        id: Option<&NamedOrBlankNode>,
        types: Vec<NamedNode>,
        results: &mut Vec<Quad>,
    ) {
        let Some(id) = id else {
            return;
        };
        let Some(graph_name) = self.last_graph_name() else {
            return;
        };
        if let (Some(subject), Some(predicate)) = (self.last_subject(), self.last_predicate()) {
            results.push(Quad::new(
                subject.clone(),
                predicate.clone(),
                id.clone(),
                graph_name.clone(),
            ))
        }
        for t in types {
            results.push(Quad::new(id.clone(), rdf::TYPE, t, graph_name.clone()))
        }
    }

    fn emit_quad_for_new_literal(&self, literal: Option<Literal>, results: &mut Vec<Quad>) {
        let Some(literal) = literal else {
            return;
        };
        let Some(graph_name) = self.last_graph_name() else {
            return;
        };
        let Some(subject) = self.last_subject() else {
            return;
        };
        let Some(predicate) = self.last_predicate() else {
            return;
        };
        results.push(Quad::new(
            subject.clone(),
            predicate.clone(),
            literal,
            graph_name.clone(),
        ))
    }

    fn convert_named_or_blank_node(
        &self,
        value: String,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) -> Option<NamedOrBlankNode> {
        if let Some(bnode_id) = value.strip_prefix("_:") {
            Some(
                if self.unchecked {
                    Some(BlankNode::new_unchecked(bnode_id))
                } else {
                    match BlankNode::new(bnode_id) {
                        Ok(id) => Some(id),
                        Err(e) => {
                            errors.push(JsonLdSyntaxError::msg(format!(
                                "Invalid blank node @id '{value}': {e}"
                            )));
                            None
                        }
                    }
                }?
                .into(),
            )
        } else {
            Some(self.convert_named_node(value, errors)?.into())
        }
    }

    fn convert_named_node(
        &self,
        value: String,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) -> Option<NamedNode> {
        if self.unchecked {
            Some(NamedNode::new_unchecked(value))
        } else {
            match NamedNode::new(&value) {
                Ok(iri) => Some(iri),
                Err(e) => {
                    errors.push(JsonLdSyntaxError::msg(format!(
                        "Invalid IRI @id '{value}': {e}"
                    )));
                    None
                }
            }
        }
    }

    fn convert_literal(
        &self,
        value: Option<JsonLdValue>,
        language: Option<String>,
        types: Vec<NamedNode>,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) -> Option<Literal> {
        let Some(value) = value else {
            errors.push(JsonLdSyntaxError::msg_and_code(
                "A literal must contain a @value key",
                JsonLdErrorCode::InvalidValueObject,
            ));
            return None;
        };
        if types.len() > 1 {
            errors.push(JsonLdSyntaxError::msg_and_code(
                "A literal must have at most one @type",
                JsonLdErrorCode::InvalidValueObject,
            ));
            return None;
        }
        let r#type = types.into_iter().next();
        Some(match value {
            JsonLdValue::String(value) => {
                if let Some(language) = language {
                    if self.unchecked {
                        Literal::new_language_tagged_literal_unchecked(value, language)
                    } else {
                        match Literal::new_language_tagged_literal(value, &language) {
                            Ok(l) => l,
                            Err(e) => {
                                errors.push(JsonLdSyntaxError::msg_and_code(
                                    format!("Invalid language tag '{language}': {e}"),
                                    JsonLdErrorCode::InvalidLanguageTaggedString,
                                ));
                                return None;
                            }
                        }
                    }
                } else if let Some(datatype) = r#type {
                    Literal::new_typed_literal(value, datatype)
                } else {
                    Literal::new_simple_literal(value)
                }
            }
            JsonLdValue::Number(value) => {
                let datatype = r#type.unwrap_or_else(|| {
                    {
                        if value.contains('e')
                            || value.contains('E')
                            || value.contains('.')
                            || value.strip_prefix('-').unwrap_or(value.as_ref()).len() >= 21
                        {
                            xsd::DOUBLE
                        } else {
                            xsd::INTEGER
                        }
                    }
                    .into()
                });
                Literal::new_typed_literal(value, datatype)
            }
            JsonLdValue::Boolean(value) => Literal::new_typed_literal(
                if value { "true" } else { "false" },
                r#type.unwrap_or_else(|| xsd::BOOLEAN.into()),
            ),
        })
    }

    fn last_subject(&self) -> Option<&NamedOrBlankNode> {
        for state in self.stack.iter().rev() {
            match state {
                JsonLdToRdfState::Object(id) => {
                    return id.as_ref();
                }
                JsonLdToRdfState::StartObject { .. } | JsonLdToRdfState::Value { .. } => {
                    unreachable!()
                }
                JsonLdToRdfState::Property(_) | JsonLdToRdfState::Graph(_) => (),
            }
        }
        None
    }

    fn last_predicate(&self) -> Option<&NamedNode> {
        for state in self.stack.iter().rev() {
            match state {
                JsonLdToRdfState::Property(predicate) => {
                    return predicate.as_ref();
                }
                JsonLdToRdfState::StartObject { .. }
                | JsonLdToRdfState::Object(_)
                | JsonLdToRdfState::Graph(_)
                | JsonLdToRdfState::Value { .. } => (),
            }
        }
        None
    }

    fn last_graph_name(&self) -> Option<&GraphName> {
        for state in self.stack.iter().rev() {
            match state {
                JsonLdToRdfState::Graph(graph) => {
                    return graph.as_ref();
                }
                JsonLdToRdfState::StartObject { .. }
                | JsonLdToRdfState::Object(_)
                | JsonLdToRdfState::Property(_)
                | JsonLdToRdfState::Value { .. } => (),
            }
        }
        None
    }
}

enum JsonLdEvent {
    StartObject { types: Vec<String> },
    EndObject,
    StartProperty(String),
    EndProperty,
    Id(String),
    Value(JsonLdValue),
    Language(String),
}

enum JsonLdValue {
    String(String),
    Number(String),
    Boolean(bool),
}

enum JsonLdIdOrKeyword<'a> {
    Id(Cow<'a, str>),
    Keyword(&'static str),
}

enum JsonLdExpansionState {
    General,
    GeneralArray,
    StartOfObject,
    ObjectType,
    ObjectTypeArray { types: Vec<String> },
    Skip,
    SkipArray,
}

/// Applies the [Expansion Algorithm](https://www.w3.org/TR/json-ld-api/#expansion-algorithms)
struct JsonLdExpansionConverter {
    state: Vec<JsonLdExpansionState>,
    is_end: bool,
}

impl JsonLdExpansionConverter {
    fn convert_event<'a>(
        &mut self,
        event: JsonEvent<'a>,
        results: &mut Vec<JsonLdEvent>,
        errors: &mut Vec<JsonLdSyntaxError>,
    ) {
        if self.state.len() > 4096 {
            unimplemented!();
        }
        if event == JsonEvent::Eof {
            self.is_end = true;
            return;
        }

        // Large hack to fetch the last state but keep it if we are in an array
        let state = self.state.pop().expect("Empty stack");
        match state {
            JsonLdExpansionState::General | JsonLdExpansionState::GeneralArray => {
                match event {
                    JsonEvent::Null => {
                        // 1)
                        if matches!(state, JsonLdExpansionState::GeneralArray) {
                            self.state.push(JsonLdExpansionState::GeneralArray);
                        }
                    }
                    JsonEvent::String(value) => {
                        // 4)
                        if matches!(state, JsonLdExpansionState::GeneralArray) {
                            self.state.push(JsonLdExpansionState::GeneralArray);
                        }
                        self.expand_value(JsonLdValue::String(value.into()), results);
                    }
                    JsonEvent::Number(value) => {
                        // 4)
                        if matches!(state, JsonLdExpansionState::GeneralArray) {
                            self.state.push(JsonLdExpansionState::GeneralArray);
                        }
                        self.expand_value(JsonLdValue::Number(value.into()), results);
                    }
                    JsonEvent::Boolean(value) => {
                        // 4)
                        if matches!(state, JsonLdExpansionState::GeneralArray) {
                            self.state.push(JsonLdExpansionState::GeneralArray);
                        }
                        self.expand_value(JsonLdValue::Boolean(value), results);
                    }
                    JsonEvent::StartArray => {
                        // 5)
                        if matches!(state, JsonLdExpansionState::GeneralArray) {
                            self.state.push(JsonLdExpansionState::GeneralArray);
                        }
                        self.state.push(JsonLdExpansionState::GeneralArray);
                    }
                    JsonEvent::EndArray => (),
                    JsonEvent::StartObject => {
                        if matches!(state, JsonLdExpansionState::GeneralArray) {
                            self.state.push(JsonLdExpansionState::GeneralArray);
                        }
                        self.state.push(JsonLdExpansionState::StartOfObject);
                    }
                    JsonEvent::EndObject => {
                        results.push(JsonLdEvent::EndObject);
                    }
                    JsonEvent::ObjectKey(key) => {
                        self.state.push(state);
                        if let Some(id_or_keyword) = self.expand_iri(key) {
                            match id_or_keyword {
                                JsonLdIdOrKeyword::Id(id) => {
                                    self.state.push(JsonLdExpansionState::General);
                                    results.push(JsonLdEvent::StartProperty(id.into()));
                                }
                                JsonLdIdOrKeyword::Keyword(keyword) => {
                                    // TODO: we do not support any keyword
                                    self.state.push(JsonLdExpansionState::Skip);
                                    errors.push(JsonLdSyntaxError::msg(format!(
                                        "Unsupported keyword: {keyword}"
                                    )));
                                }
                            }
                        } else {
                            self.state.push(JsonLdExpansionState::Skip);
                        }
                    }
                    JsonEvent::Eof => unreachable!(),
                }
            }
            JsonLdExpansionState::StartOfObject => match event {
                JsonEvent::ObjectKey(key) => {
                    if let Some(id_or_keyword) = self.expand_iri(key) {
                        match id_or_keyword {
                            JsonLdIdOrKeyword::Id(id) => {
                                results.push(JsonLdEvent::StartObject { types: Vec::new() });
                                results.push(JsonLdEvent::StartProperty(id.into()));
                                self.state.push(JsonLdExpansionState::General);
                                self.state.push(JsonLdExpansionState::General);
                            }
                            JsonLdIdOrKeyword::Keyword(keyword) => match keyword {
                                "@type" => {
                                    self.state.push(JsonLdExpansionState::ObjectType);
                                }
                                _ => {
                                    errors.push(JsonLdSyntaxError::msg(format!(
                                        "Unsupported keyword: {keyword}"
                                    )));
                                    self.state.push(JsonLdExpansionState::StartOfObject);
                                    self.state.push(JsonLdExpansionState::Skip);
                                }
                            },
                        }
                    } else {
                        self.state.push(JsonLdExpansionState::StartOfObject);
                        self.state.push(JsonLdExpansionState::Skip);
                    }
                }
                JsonEvent::EndObject => unimplemented!(),
                _ => unreachable!("Inside of an object"),
            },
            JsonLdExpansionState::ObjectType | JsonLdExpansionState::ObjectTypeArray { .. } => {
                let (mut types, is_array) =
                    if let JsonLdExpansionState::ObjectTypeArray { types } = state {
                        (types, true)
                    } else {
                        (Vec::new(), false)
                    };
                match event {
                    JsonEvent::Null | JsonEvent::Number(_) | JsonEvent::Boolean(_) => {
                        // 13.4.4.1)
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@type value must be a string",
                            JsonLdErrorCode::InvalidTypeValue,
                        ));
                        if is_array {
                            self.state
                                .push(JsonLdExpansionState::ObjectTypeArray { types });
                        } else {
                            results.push(JsonLdEvent::StartObject { types });
                            self.state.push(JsonLdExpansionState::General);
                        }
                    }
                    JsonEvent::String(value) => {
                        // 13.4.4.4)
                        if let Some(iri) = self.expand_iri(value) {
                            match iri {
                                JsonLdIdOrKeyword::Id(id) => {
                                    types.push(id.into());
                                }
                                JsonLdIdOrKeyword::Keyword(keyword) => {
                                    errors.push(JsonLdSyntaxError::msg(
                                        format!("{keyword} is not a supported by Oxigraph as a @type keyword"),
                                    ));
                                }
                            }
                        }
                        if is_array {
                            self.state
                                .push(JsonLdExpansionState::ObjectTypeArray { types });
                        } else {
                            results.push(JsonLdEvent::StartObject { types });
                            self.state.push(JsonLdExpansionState::General);
                        }
                        // TODO: fail here?
                    }
                    JsonEvent::StartArray => {
                        self.state
                            .push(JsonLdExpansionState::ObjectTypeArray { types });
                        if is_array {
                            errors.push(JsonLdSyntaxError::msg_and_code(
                                "@type cannot contain a nested array",
                                JsonLdErrorCode::InvalidTypeValue,
                            ));
                            self.state.push(JsonLdExpansionState::SkipArray);
                        }
                    }
                    JsonEvent::EndArray => {
                        results.push(JsonLdEvent::StartObject { types });
                        self.state.push(JsonLdExpansionState::General);
                    }
                    JsonEvent::StartObject => {
                        // 13.4.4.1)
                        errors.push(JsonLdSyntaxError::msg_and_code(
                            "@type value must be a string",
                            JsonLdErrorCode::InvalidTypeValue,
                        ));
                        if is_array {
                            self.state
                                .push(JsonLdExpansionState::ObjectTypeArray { types });
                        } else {
                            results.push(JsonLdEvent::StartObject { types });
                            self.state.push(JsonLdExpansionState::General);
                        }
                        self.state.push(JsonLdExpansionState::Skip);
                    }
                    JsonEvent::ObjectKey(_) | JsonEvent::EndObject | JsonEvent::Eof => {
                        unreachable!()
                    }
                }
            }
            JsonLdExpansionState::Skip | JsonLdExpansionState::SkipArray => match event {
                JsonEvent::String(_)
                | JsonEvent::Number(_)
                | JsonEvent::Boolean(_)
                | JsonEvent::Null => {
                    if matches!(state, JsonLdExpansionState::SkipArray) {
                        self.state.push(JsonLdExpansionState::SkipArray);
                    }
                }
                JsonEvent::EndArray | JsonEvent::EndObject => (),
                JsonEvent::StartArray => {
                    if matches!(state, JsonLdExpansionState::SkipArray) {
                        self.state.push(JsonLdExpansionState::SkipArray);
                    }
                    self.state.push(JsonLdExpansionState::SkipArray);
                }
                JsonEvent::StartObject => {
                    if matches!(state, JsonLdExpansionState::SkipArray) {
                        self.state.push(JsonLdExpansionState::SkipArray);
                    }
                    self.state.push(JsonLdExpansionState::Skip);
                }
                JsonEvent::ObjectKey(_) => {
                    self.state.push(JsonLdExpansionState::Skip);
                    self.state.push(JsonLdExpansionState::Skip);
                }
                JsonEvent::Eof => unreachable!(),
            },
        }
    }

    /// [IRI Expansion](https://www.w3.org/TR/json-ld-api/#iri-expansion)
    fn expand_iri<'a>(&self, iri: Cow<'a, str>) -> Option<JsonLdIdOrKeyword<'a>> {
        if let Some(suffix) = iri.strip_prefix('@') {
            // 1)
            match suffix {
                "direction" => return Some(JsonLdIdOrKeyword::Keyword("value")),
                "graph" => return Some(JsonLdIdOrKeyword::Keyword("value")),
                "id" => return Some(JsonLdIdOrKeyword::Keyword("id")),
                "language" => return Some(JsonLdIdOrKeyword::Keyword("value")),
                "type" => return Some(JsonLdIdOrKeyword::Keyword("id")),
                "value" => return Some(JsonLdIdOrKeyword::Keyword("value")),
                _ if suffix.bytes().all(|b| b.is_ascii_alphabetic()) => {
                    // 2)
                    return None;
                }
                _ => (),
            }
        }
        Some(JsonLdIdOrKeyword::Id(iri))
    }

    /// [Value Expansion](https://www.w3.org/TR/json-ld-api/#value-expansion)
    fn expand_value(&mut self, value: JsonLdValue, results: &mut Vec<JsonLdEvent>) {
        results.push(JsonLdEvent::StartObject { types: Vec::new() });
        results.push(JsonLdEvent::Value(value));
        results.push(JsonLdEvent::EndObject);
    }
}
