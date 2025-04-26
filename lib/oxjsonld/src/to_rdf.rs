use crate::context::{
    JsonLdProcessingMode, JsonLdTermDefinition, LoadDocumentOptions, RemoteDocument,
};
use crate::error::{JsonLdParseError, JsonLdSyntaxError};
use crate::expansion::{JsonLdEvent, JsonLdExpansionConverter, JsonLdValue};
#[cfg(feature = "async-tokio")]
use json_event_parser::TokioAsyncReaderJsonParser;
use json_event_parser::{JsonEvent, ReaderJsonParser, SliceJsonParser};
use oxiri::{Iri, IriParseError};
use oxrdf::vocab::{rdf, xsd};
use oxrdf::{BlankNode, GraphName, Literal, NamedNode, NamedNodeRef, NamedOrBlankNode, Quad};
use std::error::Error;
use std::io::Read;
use std::str;
#[cfg(feature = "async-tokio")]
use tokio::io::AsyncRead;

/// A [JSON-LD](https://www.w3.org/TR/json-ld/) parser.
///
/// The parser is a work in progress and only a few JSON-LD 1.0 features are supported at the moment.
///
/// The parser supports two modes:
/// - regular JSON-LD parsing that needs to buffer the full file into memory.
/// - [Streaming JSON-LD](https://www.w3.org/TR/json-ld11-streaming/) that can avoid buffering in a few cases. To enable it call the [`streaming`](JsonLdParser::streaming) method.
///
/// Count the number of people:
/// ```
/// use oxjsonld::JsonLdParser;
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
///
/// let file = br#"{
///     "@context": {"schema": "http://schema.org/"},
///     "@graph": [
///         {
///             "@type": "schema:Person",
///             "@id": "http://example.com/foo",
///             "schema:name": "Foo"
///         },
///         {
///             "@type": "schema:Person",
///             "schema:name": "Bar"
///         }
///     ]
/// }"#;
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
    lenient: bool,
    streaming: bool,
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
    pub fn lenient(mut self) -> Self {
        self.lenient = true;
        self
    }

    /// Assumes the file follows [Streaming JSON-LD](https://www.w3.org/TR/json-ld11-streaming/) specification.
    ///
    /// It will skip some buffering to make parsing faster and memory consumption lower.
    #[inline]
    pub fn streaming(mut self) -> Self {
        self.streaming = true;
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
    /// use oxjsonld::JsonLdParser;
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    ///
    /// let file = br#"{
    ///     "@context": {"schema": "http://schema.org/"},
    ///     "@graph": [
    ///         {
    ///             "@type": "schema:Person",
    ///             "@id": "http://example.com/foo",
    ///             "schema:name": "Foo"
    ///         },
    ///         {
    ///             "@type": "schema:Person",
    ///             "schema:name": "Bar"
    ///         }
    ///     ]
    /// }"#;
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
    /// use oxjsonld::JsonLdParser;
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    ///
    /// let file = br#"{
    ///     "@context": {"schema": "http://schema.org/"},
    ///     "@graph": [
    ///         {
    ///             "@type": "schema:Person",
    ///             "@id": "http://example.com/foo",
    ///             "schema:name": "Foo"
    ///         },
    ///         {
    ///             "@type": "schema:Person",
    ///             "schema:name": "Bar"
    ///         }
    ///     ]
    /// }"#;
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
    /// use oxjsonld::JsonLdParser;
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    ///
    /// let file = br#"{
    ///     "@context": {"schema": "http://schema.org/"},
    ///     "@graph": [
    ///         {
    ///             "@type": "schema:Person",
    ///             "@id": "http://example.com/foo",
    ///             "schema:name": "Foo"
    ///         },
    ///         {
    ///             "@type": "schema:Person",
    ///             "schema:name": "Bar"
    ///         }
    ///     ]
    /// }"#;
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
            expansion: JsonLdExpansionConverter::new(
                self.base,
                self.lenient,
                JsonLdProcessingMode::JsonLd1_0,
            ),
            expended_events: Vec::new(),
            to_rdf: JsonLdToRdfConverter {
                state: vec![JsonLdToRdfState::Graph(Some(GraphName::DefaultGraph))],
                lenient: self.lenient,
            },
            json_error: false,
        }
    }
}

/// Parses a JSON-LD file from a [`Read`] implementation.
///
/// Can be built using [`JsonLdParser::for_reader`].
///
/// Count the number of people:
/// ```
/// use oxjsonld::JsonLdParser;
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
///
/// let file = br#"{
///     "@context": {"schema": "http://schema.org/"},
///     "@graph": [
///         {
///             "@type": "schema:Person",
///             "@id": "http://example.com/foo",
///             "schema:name": "Foo"
///         },
///         {
///             "@type": "schema:Person",
///             "schema:name": "Bar"
///         }
///     ]
/// }"#;
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
            } else if self.inner.is_end() {
                return None;
            }
            let step = self.parse_step();
            if let Err(e) = step {
                return Some(Err(e));
            }
            // We make sure to have data in the right order
            self.results.reverse();
            self.errors.reverse();
        }
    }
}

impl<R: Read> ReaderJsonLdParser<R> {
    /// Allows to set a callback to load remote document and contexts
    ///
    /// The first argument is the document URL.
    /// See [`LoadDocumentCallback`](https://www.w3.org/TR/json-ld-api/#loaddocumentcallback) API documentation.
    ///
    /// ```
    /// use oxjsonld::{JsonLdParser, RemoteDocument};
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    ///
    /// let file = br#"{
    ///     "@context": "file://context.jsonld",
    ///     "@type": "schema:Person",
    ///     "@id": "http://example.com/foo",
    ///     "schema:name": "Foo"
    /// }"#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// for quad in JsonLdParser::new()
    ///     .for_reader(file.as_ref())
    ///     .with_load_document_callback(|url, _options| {
    ///         assert_eq!(url, "file://context.jsonld");
    ///         Ok(RemoteDocument {
    ///             content_type: "application/ld+json".into(),
    ///             document: br#"{"@context":{"schema": "http://schema.org/"}}"#.to_vec(),
    ///             document_url: "file://context.jsonld".into(),
    ///             profile: None,
    ///         })
    ///     })
    /// {
    ///     let quad = quad?;
    ///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(1, count);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn with_load_document_callback(
        mut self,
        callback: impl Fn(&str, &LoadDocumentOptions) -> Result<RemoteDocument, Box<dyn Error + Send + Sync>>
            + Send
            + Sync
            + 'static,
    ) -> Self {
        self.inner.expansion = self.inner.expansion.with_load_document_callback(callback);
        self
    }

    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxjsonld::JsonLdParser;
    ///
    /// let file = br#"{
    ///     "@context": {"schema": "http://schema.org/", "@base": "http://example.com/"},
    ///     "@type": "schema:Person",
    ///     "@id": "foo",
    ///     "schema:name": "Foo"
    /// }"#;
    ///
    /// let mut parser = JsonLdParser::new().for_reader(file.as_ref());
    /// assert_eq!(parser.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// parser.next().unwrap()?; // We read the first quad
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn prefixes(&self) -> JsonLdPrefixesIter<'_> {
        self.inner.prefixes()
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// use oxjsonld::JsonLdParser;
    ///
    /// let file = br#"{
    ///     "@context": {"schema": "http://schema.org/", "@base": "http://example.com/"},
    ///     "@type": "schema:Person",
    ///     "@id": "foo",
    ///     "schema:name": "Foo"
    /// }"#;
    ///
    /// let mut parser = JsonLdParser::new().for_reader(file.as_ref());
    /// assert!(parser.base_iri().is_none()); // No base at the beginning because none has been given to the parser.
    ///
    /// parser.next().unwrap()?; // We read the first quad
    /// assert_eq!(parser.base_iri(), Some("http://example.com/")); // There is now a base IRI.
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn base_iri(&self) -> Option<&str> {
        self.inner.base_iri()
    }

    fn parse_step(&mut self) -> Result<(), JsonLdParseError> {
        let event = self.json_parser.parse_next().map_err(|e| {
            self.inner.json_error = true;
            e
        })?;
        self.inner
            .parse_event(event, &mut self.results, &mut self.errors);
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
/// use oxjsonld::JsonLdParser;
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
///
/// let file = br#"{
///     "@context": {"schema": "http://schema.org/"},
///     "@graph": [
///         {
///             "@type": "schema:Person",
///             "@id": "http://example.com/foo",
///             "schema:name": "Foo"
///         },
///         {
///             "@type": "schema:Person",
///             "schema:name": "Bar"
///         }
///     ]
/// }"#;
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
            } else if self.inner.is_end() {
                return None;
            }
            if let Err(e) = self.parse_step().await {
                return Some(Err(e));
            }
            // We make sure to have data in the right order
            self.results.reverse();
            self.errors.reverse();
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
    /// use oxjsonld::JsonLdParser;
    ///
    /// let file = br#"{
    ///     "@context": {"schema": "http://schema.org/", "@base": "http://example.com/"},
    ///     "@type": "schema:Person",
    ///     "@id": "foo",
    ///     "schema:name": "Foo"
    /// }"#;
    ///
    /// let mut parser = JsonLdParser::new().for_tokio_async_reader(file.as_ref());
    /// assert_eq!(parser.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// parser.next().await.unwrap()?; // We read the first quad
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// # Ok(())
    /// # }
    /// ```
    pub fn prefixes(&self) -> JsonLdPrefixesIter<'_> {
        self.inner.prefixes()
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use oxjsonld::JsonLdParser;
    ///
    /// let file = br#"{
    ///     "@context": {"schema": "http://schema.org/", "@base": "http://example.com/"},
    ///     "@type": "schema:Person",
    ///     "@id": "foo",
    ///     "schema:name": "Foo"
    /// }"#;
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
        self.inner.base_iri()
    }

    async fn parse_step(&mut self) -> Result<(), JsonLdParseError> {
        let event = self.json_parser.parse_next().await.map_err(|e| {
            self.inner.json_error = true;
            e
        })?;
        self.inner
            .parse_event(event, &mut self.results, &mut self.errors);
        Ok(())
    }
}

/// Parses a JSON-LD file from a byte slice.
///
/// Can be built using [`JsonLdParser::for_slice`].
///
/// Count the number of people:
/// ```
/// use oxjsonld::JsonLdParser;
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
///
/// let file = br#"{
///     "@context": {"schema": "http://schema.org/"},
///     "@graph": [
///         {
///             "@type": "schema:Person",
///             "@id": "http://example.com/foo",
///             "schema:name": "Foo"
///         },
///         {
///             "@type": "schema:Person",
///             "schema:name": "Bar"
///         }
///     ]
/// }"#;
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
            } else if self.inner.is_end() {
                return None;
            }
            if let Err(e) = self.parse_step() {
                // I/O errors cannot happen
                return Some(Err(e));
            }
            // We make sure to have data in the right order
            self.results.reverse();
            self.errors.reverse();
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
    /// use oxjsonld::JsonLdParser;
    ///
    /// let file = br#"{
    ///     "@context": {"schema": "http://schema.org/", "@base": "http://example.com/"},
    ///     "@type": "schema:Person",
    ///     "@id": "foo",
    ///     "schema:name": "Foo"
    /// }"#;
    ///
    /// let mut parser = JsonLdParser::new().for_slice(file);
    /// assert_eq!(parser.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// parser.next().unwrap()?; // We read the first quad
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn prefixes(&self) -> JsonLdPrefixesIter<'_> {
        self.inner.prefixes()
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// use oxjsonld::JsonLdParser;
    ///
    /// let file = br#"{
    ///     "@context": {"schema": "http://schema.org/", "@base": "http://example.com/"},
    ///     "@type": "schema:Person",
    ///     "@id": "foo",
    ///     "schema:name": "Foo"
    /// }"#;
    ///
    /// let mut parser = JsonLdParser::new().for_slice(file);
    /// assert!(parser.base_iri().is_none()); // No base at the beginning because none has been given to the parser.
    ///
    /// parser.next().unwrap()?; // We read the first quad
    /// assert_eq!(parser.base_iri(), Some("http://example.com/")); // There is now a base IRI.
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn base_iri(&self) -> Option<&str> {
        self.inner.base_iri()
    }

    fn parse_step(&mut self) -> Result<(), JsonLdSyntaxError> {
        let event = self.json_parser.parse_next().map_err(|e| {
            self.inner.json_error = true;
            e
        })?;
        self.inner
            .parse_event(event, &mut self.results, &mut self.errors);
        Ok(())
    }
}

/// Iterator on the file prefixes.
///
/// See [`ReaderJsonLdParser::prefixes`].
pub struct JsonLdPrefixesIter<'a> {
    term_definitions: std::collections::hash_map::Iter<'a, String, JsonLdTermDefinition>,
    lenient: bool,
}

impl<'a> Iterator for JsonLdPrefixesIter<'a> {
    type Item = (&'a str, &'a str);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (prefix, term_definition) = self.term_definitions.next()?;
            if term_definition.prefix_flag {
                if let Some(mapping) = &term_definition.iri_mapping {
                    if self.lenient || Iri::parse(mapping.as_str()).is_ok() {
                        return Some((prefix, mapping));
                    }
                }
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.term_definitions.size_hint().1)
    }
}

struct InternalJsonLdParser {
    expansion: JsonLdExpansionConverter,
    expended_events: Vec<JsonLdEvent>,
    to_rdf: JsonLdToRdfConverter,
    json_error: bool,
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
            self.to_rdf.convert_event(event, results);
        }
    }

    fn is_end(&self) -> bool {
        self.json_error || self.expansion.is_end()
    }

    fn base_iri(&self) -> Option<&str> {
        Some(self.expansion.context().base_iri.as_ref()?.as_str())
    }

    fn prefixes(&self) -> JsonLdPrefixesIter<'_> {
        JsonLdPrefixesIter {
            term_definitions: self.expansion.context().term_definitions.iter(),
            lenient: self.to_rdf.lenient,
        }
    }
}

enum JsonLdToRdfState {
    StartObject {
        types: Vec<NamedNode>,
        /// Events before the @id event
        buffer: Vec<JsonLdEvent>,
        /// Nesting level of objects, useful during buffering
        nesting: usize,
    },
    Object(Option<NamedOrBlankNode>),
    Property(Option<NamedNode>),
    List(Option<NamedOrBlankNode>),
    Graph(Option<GraphName>),
}

struct JsonLdToRdfConverter {
    state: Vec<JsonLdToRdfState>,
    lenient: bool,
}

impl JsonLdToRdfConverter {
    fn convert_event(&mut self, event: JsonLdEvent, results: &mut Vec<Quad>) {
        let state = self.state.pop().expect("Empty stack");
        match state {
            JsonLdToRdfState::StartObject {
                types,
                mut buffer,
                nesting,
            } => {
                match event {
                    JsonLdEvent::Id(id) => {
                        if nesting > 0 {
                            buffer.push(JsonLdEvent::Id(id));
                            self.state.push(JsonLdToRdfState::StartObject {
                                types,
                                buffer,
                                nesting,
                            });
                        } else {
                            let id = self.convert_named_or_blank_node(id);
                            self.emit_quads_for_new_object(id.as_ref(), types, results);
                            self.state.push(JsonLdToRdfState::Object(id));
                            for event in buffer {
                                self.convert_event(event, results);
                            }
                        }
                    }
                    JsonLdEvent::EndObject => {
                        if nesting > 0 {
                            buffer.push(JsonLdEvent::EndObject);
                            self.state.push(JsonLdToRdfState::StartObject {
                                types,
                                buffer,
                                nesting: nesting - 1,
                            });
                        } else {
                            let id = Some(BlankNode::default().into());
                            self.emit_quads_for_new_object(id.as_ref(), types, results);
                            if !buffer.is_empty() {
                                self.state.push(JsonLdToRdfState::Object(id));
                                for event in buffer {
                                    self.convert_event(event, results);
                                }
                                // We properly end after playing the buffer
                                self.convert_event(JsonLdEvent::EndObject, results);
                            }
                        }
                    }
                    JsonLdEvent::StartObject { .. } => {
                        buffer.push(event);
                        self.state.push(JsonLdToRdfState::StartObject {
                            types,
                            buffer,
                            nesting: nesting + 1,
                        });
                    }
                    _ => {
                        buffer.push(event);
                        self.state.push(JsonLdToRdfState::StartObject {
                            types,
                            buffer,
                            nesting,
                        });
                    }
                }
            }
            JsonLdToRdfState::Object(id) => match event {
                JsonLdEvent::Id(_) => {
                    unreachable!("Should have buffered before @id")
                }
                JsonLdEvent::EndObject => (),
                JsonLdEvent::StartProperty(name) => {
                    self.state.push(JsonLdToRdfState::Object(id));
                    self.state
                        .push(JsonLdToRdfState::Property(self.convert_named_node(name)));
                }
                JsonLdEvent::StartGraph => {
                    let graph_name = id.clone().map(Into::into);
                    self.state.push(JsonLdToRdfState::Object(id));
                    self.state.push(JsonLdToRdfState::Graph(graph_name));
                }
                JsonLdEvent::StartObject { .. }
                | JsonLdEvent::Value { .. }
                | JsonLdEvent::EndProperty
                | JsonLdEvent::EndGraph
                | JsonLdEvent::StartList
                | JsonLdEvent::EndList => unreachable!(),
            },
            JsonLdToRdfState::Property(_) => match event {
                JsonLdEvent::StartObject { types } => {
                    self.state.push(state);
                    self.state.push(JsonLdToRdfState::StartObject {
                        types: types
                            .into_iter()
                            .filter_map(|t| self.convert_named_node(t))
                            .collect(),
                        buffer: Vec::new(),
                        nesting: 0,
                    });
                }
                JsonLdEvent::Value {
                    value,
                    r#type,
                    language,
                } => {
                    self.state.push(state);
                    self.emit_quad_for_new_literal(
                        self.convert_literal(value, language, r#type),
                        results,
                    )
                }
                JsonLdEvent::EndProperty => (),
                JsonLdEvent::StartList => {
                    self.state.push(state);
                    self.state.push(JsonLdToRdfState::List(None));
                }
                JsonLdEvent::StartProperty(_)
                | JsonLdEvent::Id(_)
                | JsonLdEvent::EndObject
                | JsonLdEvent::StartGraph
                | JsonLdEvent::EndGraph
                | JsonLdEvent::EndList => unreachable!(),
            },
            JsonLdToRdfState::List(current_node) => match event {
                JsonLdEvent::StartObject { types } => {
                    self.add_new_list_node_state(current_node, results);
                    self.state.push(JsonLdToRdfState::StartObject {
                        types: types
                            .into_iter()
                            .filter_map(|t| self.convert_named_node(t))
                            .collect(),
                        buffer: Vec::new(),
                        nesting: 0,
                    })
                }
                JsonLdEvent::Value {
                    value,
                    r#type,
                    language,
                } => {
                    self.add_new_list_node_state(current_node, results);
                    self.emit_quad_for_new_literal(
                        self.convert_literal(value, language, r#type),
                        results,
                    )
                }
                JsonLdEvent::StartList => {
                    self.add_new_list_node_state(current_node, results);
                    self.state.push(JsonLdToRdfState::List(None));
                }
                JsonLdEvent::EndList => {
                    if let Some(previous_node) = current_node {
                        if let Some(graph_name) = self.last_graph_name() {
                            results.push(Quad::new(
                                previous_node,
                                rdf::REST,
                                rdf::NIL.into_owned(),
                                graph_name.clone(),
                            ));
                        }
                    } else {
                        self.emit_quads_for_new_object(
                            Some(&rdf::NIL.into_owned().into()),
                            Vec::new(),
                            results,
                        )
                    }
                }
                JsonLdEvent::EndObject
                | JsonLdEvent::StartProperty(_)
                | JsonLdEvent::EndProperty
                | JsonLdEvent::Id(_)
                | JsonLdEvent::StartGraph
                | JsonLdEvent::EndGraph => unreachable!(),
            },
            JsonLdToRdfState::Graph(_) => match event {
                JsonLdEvent::StartObject { types } => {
                    self.state.push(state);
                    self.state.push(JsonLdToRdfState::StartObject {
                        types: types
                            .into_iter()
                            .filter_map(|t| self.convert_named_node(t))
                            .collect(),
                        buffer: Vec::new(),
                        nesting: 0,
                    });
                }
                JsonLdEvent::Value { .. } => {
                    self.state.push(state);
                }
                JsonLdEvent::EndGraph => (),
                JsonLdEvent::StartGraph
                | JsonLdEvent::StartProperty(_)
                | JsonLdEvent::EndProperty
                | JsonLdEvent::Id(_)
                | JsonLdEvent::EndObject
                | JsonLdEvent::StartList
                | JsonLdEvent::EndList => unreachable!(),
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
                predicate,
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
            predicate,
            literal,
            graph_name.clone(),
        ))
    }

    fn add_new_list_node_state(
        &mut self,
        current_node: Option<NamedOrBlankNode>,
        results: &mut Vec<Quad>,
    ) {
        let new_node = BlankNode::default();
        if let Some(previous_node) = current_node {
            if let Some(graph_name) = self.last_graph_name() {
                results.push(Quad::new(
                    previous_node,
                    rdf::REST,
                    new_node.clone(),
                    graph_name.clone(),
                ));
            }
        } else {
            self.emit_quads_for_new_object(Some(&new_node.clone().into()), Vec::new(), results)
        }
        self.state
            .push(JsonLdToRdfState::List(Some(new_node.into())));
    }

    fn convert_named_or_blank_node(&self, value: String) -> Option<NamedOrBlankNode> {
        Some(if let Some(bnode_id) = value.strip_prefix("_:") {
            if self.lenient {
                Some(BlankNode::new_unchecked(bnode_id))
            } else {
                BlankNode::new(bnode_id).ok()
            }?
            .into()
        } else {
            self.convert_named_node(value)?.into()
        })
    }

    fn convert_named_node(&self, value: String) -> Option<NamedNode> {
        if self.lenient {
            Some(NamedNode::new_unchecked(value))
        } else {
            NamedNode::new(&value).ok()
        }
    }

    fn convert_literal(
        &self,
        value: JsonLdValue,
        language: Option<String>,
        r#type: Option<String>,
    ) -> Option<Literal> {
        let r#type = if let Some(t) = r#type {
            Some(self.convert_named_node(t)?)
        } else {
            None
        };
        Some(match value {
            JsonLdValue::String(value) => {
                if let Some(language) = language {
                    if r#type.is_some_and(|t| t != rdf::LANG_STRING) {
                        return None; // Expansion already returns an error
                    }
                    if self.lenient {
                        Literal::new_language_tagged_literal_unchecked(value, language)
                    } else {
                        Literal::new_language_tagged_literal(value, &language).ok()?
                    }
                } else if let Some(datatype) = r#type {
                    Literal::new_typed_literal(value, datatype)
                } else {
                    Literal::new_simple_literal(value)
                }
            }
            JsonLdValue::Number(value) => {
                if language.is_some() {
                    return None; // Expansion already returns an error
                }
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
            JsonLdValue::Boolean(value) => {
                if language.is_some() {
                    return None; // Expansion already returns an error
                }
                Literal::new_typed_literal(
                    if value { "true" } else { "false" },
                    r#type.unwrap_or_else(|| xsd::BOOLEAN.into()),
                )
            }
        })
    }

    fn last_subject(&self) -> Option<&NamedOrBlankNode> {
        for state in self.state.iter().rev() {
            match state {
                JsonLdToRdfState::Object(id) => {
                    return id.as_ref();
                }
                JsonLdToRdfState::StartObject { .. } => {
                    unreachable!()
                }
                JsonLdToRdfState::Property(_) => (),
                JsonLdToRdfState::List(id) => return id.as_ref(),
                JsonLdToRdfState::Graph(_) => {
                    return None;
                }
            }
        }
        None
    }

    fn last_predicate(&self) -> Option<NamedNodeRef<'_>> {
        for state in self.state.iter().rev() {
            match state {
                JsonLdToRdfState::Property(predicate) => {
                    return predicate.as_ref().map(NamedNode::as_ref);
                }
                JsonLdToRdfState::StartObject { .. } | JsonLdToRdfState::Object(_) => (),
                JsonLdToRdfState::List(_) => return Some(rdf::FIRST),
                JsonLdToRdfState::Graph(_) => {
                    return None;
                }
            }
        }
        None
    }

    fn last_graph_name(&self) -> Option<&GraphName> {
        for state in self.state.iter().rev() {
            match state {
                JsonLdToRdfState::Graph(graph) => {
                    return graph.as_ref();
                }
                JsonLdToRdfState::StartObject { .. }
                | JsonLdToRdfState::Object(_)
                | JsonLdToRdfState::Property(_)
                | JsonLdToRdfState::List(_) => (),
            }
        }
        None
    }
}
