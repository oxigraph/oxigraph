use crate::context::{JsonLdLoadDocumentOptions, JsonLdRemoteDocument, JsonLdTermDefinition};
use crate::error::{JsonLdParseError, JsonLdSyntaxError};
use crate::expansion::{JsonLdEvent, JsonLdExpansionConverter, JsonLdValue};
use crate::profile::{JsonLdProcessingMode, JsonLdProfile, JsonLdProfileSet};
#[cfg(feature = "async-tokio")]
use json_event_parser::TokioAsyncReaderJsonParser;
use json_event_parser::{JsonEvent, ReaderJsonParser, SliceJsonParser};
use oxiri::{Iri, IriParseError};
use oxrdf::vocab::{rdf, xsd};
use oxrdf::{BlankNode, GraphName, Literal, NamedNode, NamedNodeRef, NamedOrBlankNode, Quad};
use std::error::Error;
use std::fmt::Write;
use std::io::Read;
use std::panic::{RefUnwindSafe, UnwindSafe};
use std::str;
#[cfg(feature = "async-tokio")]
use tokio::io::AsyncRead;

/// A [JSON-LD](https://www.w3.org/TR/json-ld/) parser.
///
/// The parser is a work in progress.
/// Only JSON-LD 1.0 is supported at the moment. JSON-LD 1.1 is not supported yet.
///
/// The parser supports two modes:
/// - regular JSON-LD parsing that needs to buffer the full file into memory.
/// - [Streaming JSON-LD](https://www.w3.org/TR/json-ld11-streaming/) that can avoid buffering in a few cases.
///   To enable it call the [`with_profile(JsonLdProfile::Streaming)`](JsonLdParser::with_profile) method.
///
/// Count the number of people:
/// ```
/// use oxjsonld::JsonLdParser;
/// use oxrdf::NamedNodeRef;
/// use oxrdf::vocab::rdf;
///
/// let file = r#"{
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
/// for quad in JsonLdParser::new().for_reader(file.as_bytes()) {
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
    processing_mode: JsonLdProcessingMode,
    lenient: bool,
    profile: JsonLdProfileSet,
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
    /// Note that if the file is actually not valid, the parser might emit broken RDF.
    #[inline]
    pub fn lenient(mut self) -> Self {
        self.lenient = true;
        self
    }

    /// Assume the given profile(s) during parsing.
    ///
    /// If you set the [Streaming JSON-LD](https://www.w3.org/TR/json-ld11-streaming/) profile ([`JsonLdProfile::Streaming`]),
    /// the parser will skip some buffering to make parsing faster and memory consumption lower.
    ///
    /// ```
    /// use oxjsonld::{JsonLdParser, JsonLdProfile};
    /// use oxrdf::NamedNodeRef;
    /// use oxrdf::vocab::rdf;
    ///
    /// let file = r#"{
    ///     "@context": {"schema": "http://schema.org/"},
    ///     "@graph": [
    ///         {
    ///             "@type": "schema:Person",
    ///             "@id": "http://example.com/foo",
    ///             "schema:name": "Foo"
    ///         }
    ///     ]
    /// }"#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// for quad in JsonLdParser::new()
    ///     .with_profile(JsonLdProfile::Streaming)
    ///     .for_slice(file)
    /// {
    ///     let quad = quad?;
    ///     if quad.predicate == rdf::TYPE && quad.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(1, count);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_profile(mut self, profile: impl Into<JsonLdProfileSet>) -> Self {
        self.profile = profile.into();
        self
    }

    /// Set the [processing mode](https://www.w3.org/TR/json-ld11/#dfn-processing-mode) of the parser.
    #[inline]
    #[doc(hidden)] // TODO: expose after implementing JSON-LD 1.1
    pub fn with_processing_mode(mut self, processing_mode: JsonLdProcessingMode) -> Self {
        self.processing_mode = processing_mode;
        self
    }

    /// Base IRI to use when expanding the document.
    ///
    /// It corresponds to the [`base` option from the algorithm specification](https://www.w3.org/TR/json-ld-api/#dom-jsonldoptions-base).
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
    /// use oxrdf::NamedNodeRef;
    /// use oxrdf::vocab::rdf;
    ///
    /// let file = r#"{
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
    /// for quad in JsonLdParser::new().for_reader(file.as_bytes()) {
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
    /// use oxrdf::NamedNodeRef;
    /// use oxrdf::vocab::rdf;
    ///
    /// let file = r#"{
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
    /// let mut parser = JsonLdParser::new().for_tokio_async_reader(file.as_bytes());
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
    /// use oxrdf::NamedNodeRef;
    /// use oxrdf::vocab::rdf;
    ///
    /// let file = r#"{
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
    pub fn for_slice(self, slice: &(impl AsRef<[u8]> + ?Sized)) -> SliceJsonLdParser<'_> {
        SliceJsonLdParser {
            results: Vec::new(),
            errors: Vec::new(),
            inner: self.into_inner(),
            json_parser: SliceJsonParser::new(slice.as_ref()),
        }
    }

    fn into_inner(self) -> InternalJsonLdParser {
        InternalJsonLdParser {
            expansion: JsonLdExpansionConverter::new(
                self.base,
                self.profile.contains(JsonLdProfile::Streaming),
                self.lenient,
                self.processing_mode,
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
/// use oxrdf::NamedNodeRef;
/// use oxrdf::vocab::rdf;
///
/// let file = r#"{
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
/// for quad in JsonLdParser::new().for_reader(file.as_bytes()) {
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
    /// Allows setting a callback to load remote documents and contexts
    ///
    /// The first argument is the document URL.
    ///
    /// It corresponds to the [`documentLoader` option from the algorithm specification](https://www.w3.org/TR/json-ld11-api/#dom-jsonldoptions-documentloader).
    ///
    /// See [`LoadDocumentCallback` API documentation](https://www.w3.org/TR/json-ld-api/#loaddocumentcallback) for more details
    ///
    /// ```
    /// use oxjsonld::{JsonLdParser, JsonLdRemoteDocument};
    /// use oxrdf::NamedNodeRef;
    /// use oxrdf::vocab::rdf;
    ///
    /// let file = r#"{
    ///     "@context": "file://context.jsonld",
    ///     "@type": "schema:Person",
    ///     "@id": "http://example.com/foo",
    ///     "schema:name": "Foo"
    /// }"#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// for quad in JsonLdParser::new()
    ///     .for_reader(file.as_bytes())
    ///     .with_load_document_callback(|url, _options| {
    ///         assert_eq!(url, "file://context.jsonld");
    ///         Ok(JsonLdRemoteDocument {
    ///             document: br#"{"@context":{"schema": "http://schema.org/"}}"#.to_vec(),
    ///             document_url: "file://context.jsonld".into(),
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
        callback: impl Fn(
            &str,
            &JsonLdLoadDocumentOptions,
        ) -> Result<JsonLdRemoteDocument, Box<dyn Error + Send + Sync>>
        + Send
        + Sync
        + UnwindSafe
        + RefUnwindSafe
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
    /// let file = r#"{
    ///     "@context": {"schema": "http://schema.org/", "@base": "http://example.com/"},
    ///     "@type": "schema:Person",
    ///     "@id": "foo",
    ///     "schema:name": "Foo"
    /// }"#;
    ///
    /// let mut parser = JsonLdParser::new().for_reader(file.as_bytes());
    /// assert_eq!(parser.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// parser.next().unwrap()?; // We read the first quad
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// //
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
    /// let file = r#"{
    ///     "@context": {"schema": "http://schema.org/", "@base": "http://example.com/"},
    ///     "@type": "schema:Person",
    ///     "@id": "foo",
    ///     "schema:name": "Foo"
    /// }"#;
    ///
    /// let mut parser = JsonLdParser::new().for_reader(file.as_bytes());
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
        let event = self.json_parser.parse_next().inspect_err(|_| {
            self.inner.json_error = true;
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
/// use oxrdf::NamedNodeRef;
/// use oxrdf::vocab::rdf;
///
/// let file = r#"{
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
/// let mut parser = JsonLdParser::new().for_tokio_async_reader(file.as_bytes());
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
    /// let file = r#"{
    ///     "@context": {"schema": "http://schema.org/", "@base": "http://example.com/"},
    ///     "@type": "schema:Person",
    ///     "@id": "foo",
    ///     "schema:name": "Foo"
    /// }"#;
    ///
    /// let mut parser = JsonLdParser::new().for_tokio_async_reader(file.as_bytes());
    /// assert_eq!(parser.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// parser.next().await.unwrap()?; // We read the first quad
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// //
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
    /// let file = r#"{
    ///     "@context": {"schema": "http://schema.org/", "@base": "http://example.com/"},
    ///     "@type": "schema:Person",
    ///     "@id": "foo",
    ///     "schema:name": "Foo"
    /// }"#;
    ///
    /// let mut parser = JsonLdParser::new().for_tokio_async_reader(file.as_bytes());
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
        let event = self.json_parser.parse_next().await.inspect_err(|_| {
            self.inner.json_error = true;
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
/// use oxrdf::NamedNodeRef;
/// use oxrdf::vocab::rdf;
///
/// let file = r#"{
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
    /// Allows setting a callback to load remote documents and contexts
    ///
    /// The first argument is the document URL.
    ///
    /// It corresponds to the [`documentLoader` option from the algorithm specification](https://www.w3.org/TR/json-ld11-api/#dom-jsonldoptions-documentloader).
    ///
    /// See [`LoadDocumentCallback` API documentation](https://www.w3.org/TR/json-ld-api/#loaddocumentcallback) for more details
    ///
    /// ```
    /// use oxjsonld::{JsonLdParser, JsonLdRemoteDocument};
    /// use oxrdf::NamedNodeRef;
    /// use oxrdf::vocab::rdf;
    ///
    /// let file = r#"{
    ///     "@context": "file://context.jsonld",
    ///     "@type": "schema:Person",
    ///     "@id": "http://example.com/foo",
    ///     "schema:name": "Foo"
    /// }"#;
    ///
    /// let schema_person = NamedNodeRef::new("http://schema.org/Person")?;
    /// let mut count = 0;
    /// for quad in JsonLdParser::new()
    ///     .for_slice(file)
    ///     .with_load_document_callback(|url, _options| {
    ///         assert_eq!(url, "file://context.jsonld");
    ///         Ok(JsonLdRemoteDocument {
    ///             document: br#"{"@context":{"schema": "http://schema.org/"}}"#.to_vec(),
    ///             document_url: "file://context.jsonld".into(),
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
        callback: impl Fn(
            &str,
            &JsonLdLoadDocumentOptions,
        ) -> Result<JsonLdRemoteDocument, Box<dyn Error + Send + Sync>>
        + Send
        + Sync
        + UnwindSafe
        + RefUnwindSafe
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
    /// let file = r#"{
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
    /// //
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
    /// let file = r#"{
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
        let event = self.json_parser.parse_next().inspect_err(|_| {
            self.inner.json_error = true;
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
                if let Some(Some(mapping)) = &term_definition.iri_mapping {
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
        types: Vec<NamedOrBlankNode>,
        /// Events before the @id event
        buffer: Vec<JsonLdEvent>,
        /// Nesting level of objects, useful during buffering
        nesting: usize,
    },
    Object(Option<NamedOrBlankNode>),
    Property {
        id: Option<NamedNode>,
        reverse: bool,
    },
    List(Option<NamedOrBlankNode>),
    Graph(Option<GraphName>),
}

struct JsonLdToRdfConverter {
    state: Vec<JsonLdToRdfState>,
    lenient: bool,
}

impl JsonLdToRdfConverter {
    fn convert_event(&mut self, event: JsonLdEvent, results: &mut Vec<Quad>) {
        #[expect(clippy::expect_used)]
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
                JsonLdEvent::StartProperty { name, reverse } => {
                    self.state.push(JsonLdToRdfState::Object(id));
                    self.state.push(JsonLdToRdfState::Property {
                        id: if self.has_defined_last_predicate() {
                            self.convert_named_node(name)
                        } else {
                            None // We do not want to emit if one of the parent property is not emitted
                        },
                        reverse,
                    });
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
                | JsonLdEvent::EndList
                | JsonLdEvent::StartSet
                | JsonLdEvent::EndSet => unreachable!(),
            },
            JsonLdToRdfState::Property { .. } => match event {
                JsonLdEvent::StartObject { types } => {
                    self.state.push(state);
                    self.state.push(JsonLdToRdfState::StartObject {
                        types: types
                            .into_iter()
                            .filter_map(|t| self.convert_named_or_blank_node(t))
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
                JsonLdEvent::StartSet | JsonLdEvent::EndSet => {
                    self.state.push(state);
                }
                JsonLdEvent::StartProperty { .. }
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
                            .filter_map(|t| self.convert_named_or_blank_node(t))
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
                JsonLdEvent::StartSet | JsonLdEvent::EndSet => {
                    // TODO: this is bad
                    self.state.push(JsonLdToRdfState::List(current_node));
                }
                JsonLdEvent::EndObject
                | JsonLdEvent::StartProperty { .. }
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
                            .filter_map(|t| self.convert_named_or_blank_node(t))
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
                | JsonLdEvent::StartProperty { .. }
                | JsonLdEvent::EndProperty
                | JsonLdEvent::Id(_)
                | JsonLdEvent::EndObject
                | JsonLdEvent::StartList
                | JsonLdEvent::EndList
                | JsonLdEvent::StartSet
                | JsonLdEvent::EndSet => unreachable!(),
            },
        }
    }

    fn emit_quads_for_new_object(
        &self,
        id: Option<&NamedOrBlankNode>,
        types: Vec<NamedOrBlankNode>,
        results: &mut Vec<Quad>,
    ) {
        let Some(id) = id else {
            return;
        };
        let Some(graph_name) = self.last_graph_name() else {
            return;
        };
        if let (Some(subject), Some((predicate, reverse))) =
            (self.last_subject(), self.last_predicate())
        {
            results.push(if reverse {
                Quad::new(id.clone(), predicate, subject.clone(), graph_name.clone())
            } else {
                Quad::new(subject.clone(), predicate, id.clone(), graph_name.clone())
            })
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
        let Some((predicate, reverse)) = self.last_predicate() else {
            return;
        };
        if reverse {
            return;
        }
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
                let value = canonicalize_json_number(
                    &value,
                    r#type.as_ref().is_some_and(|t| *t == xsd::DOUBLE),
                )
                .unwrap_or(RdfJsonNumber::Double(value));
                match value {
                    RdfJsonNumber::Integer(value) => Literal::new_typed_literal(
                        value,
                        r#type.unwrap_or_else(|| xsd::INTEGER.into()),
                    ),
                    RdfJsonNumber::Double(value) => Literal::new_typed_literal(
                        value,
                        r#type.unwrap_or_else(|| xsd::DOUBLE.into()),
                    ),
                }
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
                JsonLdToRdfState::Property { .. } => (),
                JsonLdToRdfState::List(id) => return id.as_ref(),
                JsonLdToRdfState::Graph(_) => {
                    return None;
                }
            }
        }
        None
    }

    fn last_predicate(&self) -> Option<(NamedNodeRef<'_>, bool)> {
        for state in self.state.iter().rev() {
            match state {
                JsonLdToRdfState::Property { id, reverse } => {
                    return Some((id.as_ref()?.as_ref(), *reverse));
                }
                JsonLdToRdfState::StartObject { .. } | JsonLdToRdfState::Object(_) => (),
                JsonLdToRdfState::List(_) => return Some((rdf::FIRST, false)),
                JsonLdToRdfState::Graph(_) => {
                    return None;
                }
            }
        }
        None
    }

    fn has_defined_last_predicate(&self) -> bool {
        for state in self.state.iter().rev() {
            if let JsonLdToRdfState::Property { id, .. } = state {
                return id.is_some();
            }
        }
        true
    }

    fn last_graph_name(&self) -> Option<&GraphName> {
        for state in self.state.iter().rev() {
            match state {
                JsonLdToRdfState::Graph(graph) => {
                    return graph.as_ref();
                }
                JsonLdToRdfState::StartObject { .. }
                | JsonLdToRdfState::Object(_)
                | JsonLdToRdfState::Property { .. }
                | JsonLdToRdfState::List(_) => (),
            }
        }
        None
    }
}

#[derive(Eq, PartialEq, Debug, Clone)]
enum RdfJsonNumber {
    Integer(String),
    Double(String),
}

/// Canonicalizes the JSON number to xsd:double canonical form.
fn canonicalize_json_number(value: &str, always_double: bool) -> Option<RdfJsonNumber> {
    // We parse
    let (value, is_negative) = if let Some(value) = value.strip_prefix('-') {
        (value, true)
    } else if let Some(value) = value.strip_prefix('+') {
        (value, false)
    } else {
        (value, false)
    };
    let (value, exp) = value.split_once(['e', 'E']).unwrap_or((value, "0"));
    let (mut integer_part, mut decimal_part) = value.split_once('.').unwrap_or((value, ""));
    let mut exp = exp.parse::<i64>().ok()?;

    // We normalize
    // We trim the zeros
    while let Some(c) = integer_part.strip_prefix('0') {
        integer_part = c;
    }
    while let Some(c) = decimal_part.strip_suffix('0') {
        decimal_part = c;
    }
    if decimal_part.is_empty() {
        while let Some(c) = integer_part.strip_suffix('0') {
            integer_part = c;
            exp = exp.checked_add(1)?;
        }
    }
    if integer_part.is_empty() {
        while let Some(c) = decimal_part.strip_prefix('0') {
            decimal_part = c;
            exp = exp.checked_sub(1)?;
        }
    }

    // We set the exponent in the 0.XXXEYYY form
    let exp_change = i64::try_from(integer_part.len()).ok()?;
    exp = exp.checked_add(exp_change)?;

    // We handle the zero case
    if integer_part.is_empty() && decimal_part.is_empty() {
        integer_part = "0";
        exp = 1;
    }

    // We serialize
    let mut buffer = String::with_capacity(value.len());
    if is_negative {
        buffer.push('-');
    }
    let digits_count = i64::try_from(integer_part.len() + decimal_part.len()).ok()?;
    Some(if !always_double && exp >= digits_count && exp < 21 {
        buffer.push_str(integer_part);
        buffer.push_str(decimal_part);
        buffer.extend((0..(exp - digits_count)).map(|_| '0'));
        RdfJsonNumber::Integer(buffer)
    } else {
        let mut all_digits = integer_part.chars().chain(decimal_part.chars());
        buffer.push(all_digits.next()?);
        buffer.push('.');
        if digits_count == 1 {
            buffer.push('0');
        } else {
            buffer.extend(all_digits);
        }
        write!(&mut buffer, "E{}", exp.checked_sub(1)?).ok()?;
        RdfJsonNumber::Double(buffer)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonicalize_json_number() {
        assert_eq!(
            canonicalize_json_number("12", false),
            Some(RdfJsonNumber::Integer("12".into()))
        );
        assert_eq!(
            canonicalize_json_number("-12", false),
            Some(RdfJsonNumber::Integer("-12".into()))
        );
        assert_eq!(
            canonicalize_json_number("1", true),
            Some(RdfJsonNumber::Double("1.0E0".into()))
        );
        assert_eq!(
            canonicalize_json_number("1", true),
            Some(RdfJsonNumber::Double("1.0E0".into()))
        );
        assert_eq!(
            canonicalize_json_number("+1", true),
            Some(RdfJsonNumber::Double("1.0E0".into()))
        );
        assert_eq!(
            canonicalize_json_number("-1", true),
            Some(RdfJsonNumber::Double("-1.0E0".into()))
        );
        assert_eq!(
            canonicalize_json_number("12", true),
            Some(RdfJsonNumber::Double("1.2E1".into()))
        );
        assert_eq!(
            canonicalize_json_number("-12", true),
            Some(RdfJsonNumber::Double("-1.2E1".into()))
        );
        assert_eq!(
            canonicalize_json_number("12.3456E3", false),
            Some(RdfJsonNumber::Double("1.23456E4".into()))
        );
        assert_eq!(
            canonicalize_json_number("12.3456e3", false),
            Some(RdfJsonNumber::Double("1.23456E4".into()))
        );
        assert_eq!(
            canonicalize_json_number("-12.3456E3", false),
            Some(RdfJsonNumber::Double("-1.23456E4".into()))
        );
        assert_eq!(
            canonicalize_json_number("12.34E-3", false),
            Some(RdfJsonNumber::Double("1.234E-2".into()))
        );
        assert_eq!(
            canonicalize_json_number("12.340E-3", false),
            Some(RdfJsonNumber::Double("1.234E-2".into()))
        );
        assert_eq!(
            canonicalize_json_number("0.01234E-1", false),
            Some(RdfJsonNumber::Double("1.234E-3".into()))
        );
        assert_eq!(
            canonicalize_json_number("1.0", false),
            Some(RdfJsonNumber::Integer("1".into()))
        );
        assert_eq!(
            canonicalize_json_number("1.0E0", false),
            Some(RdfJsonNumber::Integer("1".into()))
        );
        assert_eq!(
            canonicalize_json_number("0.01E2", false),
            Some(RdfJsonNumber::Integer("1".into()))
        );
        assert_eq!(
            canonicalize_json_number("1E2", false),
            Some(RdfJsonNumber::Integer("100".into()))
        );
        assert_eq!(
            canonicalize_json_number("1E21", false),
            Some(RdfJsonNumber::Double("1.0E21".into()))
        );
        assert_eq!(
            canonicalize_json_number("0", false),
            Some(RdfJsonNumber::Integer("0".into()))
        );
        assert_eq!(
            canonicalize_json_number("0", true),
            Some(RdfJsonNumber::Double("0.0E0".into()))
        );
        assert_eq!(
            canonicalize_json_number("-0", true),
            Some(RdfJsonNumber::Double("-0.0E0".into()))
        );
        assert_eq!(
            canonicalize_json_number("0E-10", true),
            Some(RdfJsonNumber::Double("0.0E0".into()))
        );
    }
}
