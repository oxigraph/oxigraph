use crate::error::{JsonLdErrorCode, JsonLdParseError, JsonLdSyntaxError};
#[cfg(feature = "async-tokio")]
use json_event_parser::TokioAsyncReaderJsonParser;
use json_event_parser::{JsonEvent, ReaderJsonParser, SliceJsonParser};
use oxilangtag::LanguageTag;
use oxiri::{Iri, IriParseError};
use oxrdf::vocab::{rdf, xsd};
use oxrdf::{
    BlankNode, GraphName, Literal, NamedNode, NamedNodeRef, NamedOrBlankNode, Quad, Subject,
};
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
            inner: self.into_inner(),
            json_parser: SliceJsonParser::new(slice),
        }
    }

    fn into_inner(self) -> InternalJsonLdParser {
        InternalJsonLdParser {
            state: vec![JsonLdState::Root],
            is_end: false,
            unchecked: self.unchecked,
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
    inner: InternalJsonLdParser,
    json_parser: ReaderJsonParser<R>,
}

impl<R: Read> Iterator for ReaderJsonLdParser<R> {
    type Item = Result<Quad, JsonLdParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(quad) = self.results.pop() {
                return Some(Ok(quad));
            } else if self.inner.is_end {
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
            unchecked: self.inner.unchecked,
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
        Ok(self
            .inner
            .parse_event(self.json_parser.parse_next()?, &mut self.results)?)
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
    inner: InternalJsonLdParser,
    json_parser: TokioAsyncReaderJsonParser<R>,
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> TokioAsyncReaderJsonLdParser<R> {
    /// Reads the next quad or returns `None` if the file is finished.
    pub async fn next(&mut self) -> Option<Result<Quad, JsonLdParseError>> {
        loop {
            if let Some(quad) = self.results.pop() {
                return Some(Ok(quad));
            } else if self.inner.is_end {
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
        Ok(self
            .inner
            .parse_event(self.json_parser.parse_next().await?, &mut self.results)?)
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
    inner: InternalJsonLdParser,
    json_parser: SliceJsonParser<'a>,
}

impl Iterator for SliceJsonLdParser<'_> {
    type Item = Result<Quad, JsonLdSyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(quad) = self.results.pop() {
                return Some(Ok(quad));
            } else if self.inner.is_end {
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
            unchecked: self.inner.unchecked,
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
        self.inner
            .parse_event(self.json_parser.parse_next()?, &mut self.results)
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

enum JsonLdState {
    Root,
    RootArray,
    StartObject {
        r#type: Vec<NamedNode>,
    },
    ObjectOrLiteralType,
    Object {
        id: Option<NamedOrBlankNode>,
        r#type: Vec<NamedNode>,
    },
    ObjectId,
    ObjectPredicate {
        predicate: NamedNode,
    },
    Literal {
        value: Vec<String>,
        r#type: Option<NamedNode>,
        language: Option<String>,
        fallback_type: Option<NamedNodeRef<'static>>,
    },
    LiteralValue,
    LiteralType,
    LiteralLanguage,
    Skip,
}

struct InternalJsonLdParser {
    state: Vec<JsonLdState>,
    is_end: bool,
    unchecked: bool,
}

impl InternalJsonLdParser {
    fn parse_event(
        &mut self,
        event: JsonEvent<'_>,
        results: &mut Vec<Quad>,
    ) -> Result<(), JsonLdSyntaxError> {
        if self.state.len() > 1000 {
            unimplemented!();
        }
        if event == JsonEvent::Eof {
            self.is_end = true;
            return Ok(());
        }
        let Some(state) = self.state.pop() else {
            assert_eq!(event, JsonEvent::Eof, "State can only be empty on EOF");
            return Ok(());
        };
        match state {
            JsonLdState::Root => match event {
                JsonEvent::StartObject => {
                    self.state
                        .push(JsonLdState::StartObject { r#type: Vec::new() });
                    Ok(())
                }
                JsonEvent::StartArray => {
                    self.state.push(JsonLdState::RootArray);
                    Ok(())
                }
                JsonEvent::String(_)
                | JsonEvent::Number(_)
                | JsonEvent::Boolean(_)
                | JsonEvent::Null => {
                    Ok(()) // Empty document
                }
                JsonEvent::EndArray
                | JsonEvent::EndObject
                | JsonEvent::ObjectKey(_)
                | JsonEvent::Eof => unreachable!(),
            },
            JsonLdState::RootArray => match event {
                JsonEvent::StartObject => {
                    self.state.push(JsonLdState::RootArray);
                    self.state
                        .push(JsonLdState::StartObject { r#type: Vec::new() });
                    Ok(())
                }
                JsonEvent::StartArray => {
                    self.state.push(JsonLdState::RootArray);
                    self.state.push(JsonLdState::RootArray);
                    Ok(())
                }
                JsonEvent::String(_)
                | JsonEvent::Number(_)
                | JsonEvent::Boolean(_)
                | JsonEvent::Null => {
                    self.state.push(JsonLdState::RootArray);
                    Ok(())
                }
                JsonEvent::EndArray => Ok(()),
                JsonEvent::EndObject | JsonEvent::ObjectKey(_) | JsonEvent::Eof => unreachable!(),
            },
            JsonLdState::StartObject { r#type } => match event {
                JsonEvent::ObjectKey(key) => {
                    self.state.push(
                        if matches!(key.as_ref(), "@value" | "@language" | "@direction") {
                            JsonLdState::Literal {
                                value: Vec::new(),
                                r#type: r#type.into_iter().next(),
                                language: None,
                                fallback_type: None,
                            }
                        } else if key == "@type" {
                            JsonLdState::ObjectOrLiteralType
                        } else {
                            JsonLdState::Object { id: None, r#type }
                        },
                    );
                    self.parse_event(JsonEvent::ObjectKey(key), results)
                }
                JsonEvent::EndObject => unimplemented!(),
                _ => unreachable!(),
            },
            JsonLdState::ObjectOrLiteralType => match event {
                JsonEvent::String(t) => match NamedNode::new(t.as_ref()) {
                    Ok(t) => {
                        self.state
                            .push(JsonLdState::StartObject { r#type: vec![t] });
                        Ok(())
                    }
                    Err(e) => {
                        self.state
                            .push(JsonLdState::StartObject { r#type: Vec::new() });
                        Err(JsonLdSyntaxError::msg_and_code(
                            format!("@type value '{t}' must be a a valid IRI: {e}"),
                            JsonLdErrorCode::InvalidTypeValue,
                        ))
                    }
                },
                JsonEvent::StartArray | JsonEvent::EndArray => unimplemented!(),
                JsonEvent::Number(_)
                | JsonEvent::Boolean(_)
                | JsonEvent::Null
                | JsonEvent::StartObject
                | JsonEvent::EndObject
                | JsonEvent::ObjectKey(_)
                | JsonEvent::Eof => Err(JsonLdSyntaxError::msg_and_code(
                    "A @type must be a string or an array of strings",
                    JsonLdErrorCode::InvalidTypeValue,
                )),
            },
            JsonLdState::Object { id, r#type } => match event {
                JsonEvent::ObjectKey(key) => {
                    self.state.push(JsonLdState::Object { id, r#type });
                    if key.as_ref() == "@id" {
                        self.state.push(JsonLdState::ObjectId);
                        Ok(())
                    } else {
                        match NamedNode::new(key.as_ref()) {
                            Ok(predicate) => {
                                self.state.push(JsonLdState::ObjectPredicate { predicate });
                                Ok(())
                            }
                            Err(e) => {
                                self.state.push(JsonLdState::Skip);
                                Err(JsonLdSyntaxError::msg(format!(
                                    "Invalid predicate IRI '{key}': {e}"
                                )))
                            }
                        }
                    }
                }
                JsonEvent::EndObject => {
                    // TODO: do it as soon as @id is emitted to get nicer output
                    if let Some(subject) = self.current_subject() {
                        for t in r#type {
                            results.push(Quad::new(
                                subject.clone(),
                                rdf::TYPE,
                                t,
                                self.current_graph_name(),
                            ));
                        }
                        let Some(predicate) = self.current_predicate() else {
                            unreachable!("Subject without predicate")
                        };
                        results.push(Quad {
                            subject,
                            predicate,
                            object: id.unwrap_or_else(|| BlankNode::default().into()).into(),
                            graph_name: self.current_graph_name(),
                        });
                    }
                    Ok(())
                }
                _ => unreachable!(),
            },
            JsonLdState::ObjectPredicate { predicate } => match event {
                JsonEvent::String(value) => {
                    let Some(subject) = self.current_subject() else {
                        unreachable!("Predicate without subject")
                    };
                    results.push(Quad {
                        subject,
                        predicate,
                        object: Literal::new_simple_literal(value).into(),
                        graph_name: self.current_graph_name(),
                    });
                    Ok(())
                }
                JsonEvent::Number(_) => unimplemented!(),
                JsonEvent::Boolean(_) => unimplemented!(),
                JsonEvent::Null => Ok(()),
                JsonEvent::StartArray => unimplemented!(),
                JsonEvent::EndArray => unimplemented!(),
                JsonEvent::StartObject => {
                    self.state.push(JsonLdState::ObjectPredicate { predicate });
                    self.state
                        .push(JsonLdState::StartObject { r#type: Vec::new() });
                    Ok(())
                }
                JsonEvent::EndObject | JsonEvent::ObjectKey(_) | JsonEvent::Eof => {
                    self.parse_event(event, results)
                }
            },
            JsonLdState::ObjectId => {
                let current_graph_name = self.current_graph_name();
                let Some(JsonLdState::Object { id, r#type }) = self.state.last_mut() else {
                    unreachable!();
                };
                if id.is_some() {
                    return Err(JsonLdSyntaxError::msg_and_code(
                        "An @id is already set for this object",
                        JsonLdErrorCode::CollidingKeywords,
                    ));
                }
                match event {
                    JsonEvent::String(i) => {
                        let i = parse_id(i)?;
                        // We push types early
                        for t in r#type.drain(..) {
                            results.push(Quad::new(
                                i.clone(),
                                rdf::TYPE,
                                t,
                                current_graph_name.clone(),
                            ));
                        }
                        *id = Some(i);
                        Ok(())
                    }
                    JsonEvent::Number(_) | JsonEvent::Boolean(_) => {
                        Err(JsonLdSyntaxError::msg_and_code(
                            "The value of @id must be a string",
                            JsonLdErrorCode::InvalidIdValue,
                        ))
                    }
                    JsonEvent::Null
                    | JsonEvent::StartArray
                    | JsonEvent::EndArray
                    | JsonEvent::StartObject
                    | JsonEvent::EndObject
                    | JsonEvent::ObjectKey(_)
                    | JsonEvent::Eof => unimplemented!(),
                }
            }
            JsonLdState::Literal {
                value,
                language,
                r#type,
                fallback_type,
            } => match event {
                JsonEvent::ObjectKey(key) => {
                    self.state.push(JsonLdState::Literal {
                        value,
                        language,
                        r#type,
                        fallback_type,
                    });
                    match key.as_ref() {
                        "@value" => {
                            self.state.push(JsonLdState::LiteralValue);
                            Ok(())
                        }
                        "@language" => {
                            self.state.push(JsonLdState::LiteralLanguage);
                            Ok(())
                        }
                        "@type" => {
                            self.state.push(JsonLdState::LiteralType);
                            Ok(())
                        }
                        _ => {
                            self.state.push(JsonLdState::Skip);
                            Err(JsonLdSyntaxError::msg(
                                "Only @language, @type and @value keys are allowed in literals",
                            ))
                        }
                    }
                }
                JsonEvent::EndObject => {
                    let object = if let Some(language) = language {
                        if fallback_type.is_some() {
                            return Err(JsonLdSyntaxError::msg_and_code(
                                "@language must be used only on string @value",
                                JsonLdErrorCode::InvalidLanguageTaggedValue,
                            ));
                        }
                        if !r#type.is_some_and(|t| t == rdf::LANG_STRING) {
                            return Err(JsonLdSyntaxError::msg(
                                "When @language is present, @type must not be set",
                            ));
                        }
                        Literal::new_language_tagged_literal_unchecked(value, language)
                    } else if let Some(r#type) = r#type {
                        Literal::new_typed_literal(value, r#type)
                    } else if let Some(fallback_type) = fallback_type {
                        Literal::new_typed_literal(value, fallback_type)
                    } else {
                        Literal::new_simple_literal(value)
                    }
                    .into();
                    let Some(subject) = self.current_subject() else {
                        return Ok(()); // TODO: is it ok to skip?
                    };
                    let Some(predicate) = self.current_predicate() else {
                        unreachable!("No predicate when parsing a value")
                    };
                    results.push(Quad {
                        subject,
                        predicate,
                        object,
                        graph_name: self.current_graph_name(),
                    });
                    Ok(())
                }
                _ => unreachable!(),
            },
            JsonLdState::LiteralValue => {
                let Some(JsonLdState::Literal {
                    value,
                    fallback_type,
                    language,
                    ..
                }) = self.state.last_mut()
                else {
                    unreachable!();
                };
                if !value.is_empty() {
                    return Err(JsonLdSyntaxError::msg_and_code(
                        "An @value is already set for this object",
                        JsonLdErrorCode::CollidingKeywords,
                    ));
                }
                match event {
                    JsonEvent::String(v) => {
                        *value = vec![v.into_owned()];
                        *fallback_type = Some(xsd::STRING);
                        Ok(())
                    }
                    JsonEvent::Number(v) => {
                        *value = vec![v.to_string()];
                        *fallback_type = Some(guess_number_datatype(&v));
                        if language.is_some() {
                            return Err(JsonLdSyntaxError::msg_and_code(
                                "@value must be a string if @language is set",
                                JsonLdErrorCode::InvalidLanguageTaggedValue,
                            ));
                        }
                        Ok(())
                    }
                    JsonEvent::Boolean(v) => {
                        *value = vec![v.to_string()];
                        *fallback_type = Some(xsd::BOOLEAN);
                        if language.is_some() {
                            return Err(JsonLdSyntaxError::msg_and_code(
                                "@value must be a string if @language is set",
                                JsonLdErrorCode::InvalidLanguageTaggedValue,
                            ));
                        }
                        Ok(())
                    }
                    JsonEvent::Null
                    | JsonEvent::StartArray
                    | JsonEvent::EndArray
                    | JsonEvent::StartObject
                    | JsonEvent::EndObject
                    | JsonEvent::ObjectKey(_)
                    | JsonEvent::Eof => unimplemented!(),
                }
            }
            JsonLdState::LiteralLanguage => {
                let Some(JsonLdState::Literal { language, .. }) = self.state.last_mut() else {
                    unreachable!();
                };
                if language.is_some() {
                    return Err(JsonLdSyntaxError::msg_and_code(
                        "A @language is already set for this object",
                        JsonLdErrorCode::CollidingKeywords,
                    ));
                }
                match event {
                    JsonEvent::String(v) => {
                        if let Err(e) = LanguageTag::parse(v.as_ref()) {
                            return Err(JsonLdSyntaxError::msg_and_code(
                                format!("Invalid language tag '{v}': {e}"),
                                JsonLdErrorCode::InvalidLanguageTaggedString,
                            ));
                        }
                        *language = Some(v.into_owned());
                        Ok(())
                    }
                    JsonEvent::Number(_)
                    | JsonEvent::Boolean(_)
                    | JsonEvent::Null
                    | JsonEvent::StartArray
                    | JsonEvent::EndArray
                    | JsonEvent::StartObject
                    | JsonEvent::EndObject
                    | JsonEvent::ObjectKey(_)
                    | JsonEvent::Eof => Err(JsonLdSyntaxError::msg_and_code(
                        "A literal language must be a string",
                        JsonLdErrorCode::InvalidLanguageTaggedString,
                    )),
                }
            }
            JsonLdState::LiteralType => {
                let Some(JsonLdState::Literal { r#type, .. }) = self.state.last_mut() else {
                    unreachable!();
                };
                if r#type.is_some() {
                    return Err(JsonLdSyntaxError::msg_and_code(
                        "A @language is already set for this object",
                        JsonLdErrorCode::CollidingKeywords,
                    ));
                }
                match event {
                    JsonEvent::String(t) => match NamedNode::new(t.as_ref()) {
                        Ok(t) => {
                            *r#type = Some(t);
                            Ok(())
                        }
                        Err(e) => Err(JsonLdSyntaxError::msg_and_code(
                            format!("@type value '{t}' must be a a valid IRI: {e}"),
                            JsonLdErrorCode::InvalidTypeValue,
                        )),
                    },
                    JsonEvent::Number(_)
                    | JsonEvent::Boolean(_)
                    | JsonEvent::Null
                    | JsonEvent::StartArray
                    | JsonEvent::EndArray
                    | JsonEvent::StartObject
                    | JsonEvent::EndObject
                    | JsonEvent::ObjectKey(_)
                    | JsonEvent::Eof => Err(JsonLdSyntaxError::msg_and_code(
                        "A literal type must be a string",
                        JsonLdErrorCode::InvalidTypeValue,
                    )),
                }
            }
            JsonLdState::Skip => match event {
                JsonEvent::String(_)
                | JsonEvent::Number(_)
                | JsonEvent::Boolean(_)
                | JsonEvent::Null
                | JsonEvent::EndArray
                | JsonEvent::EndObject
                | JsonEvent::Eof => Ok(()),
                JsonEvent::StartArray | JsonEvent::StartObject | JsonEvent::ObjectKey(_) => {
                    self.state.push(JsonLdState::Skip);
                    self.state.push(JsonLdState::Skip);
                    Ok(())
                }
            },
        }
    }

    fn current_subject(&mut self) -> Option<Subject> {
        for state in self.state.iter_mut().rev() {
            if let JsonLdState::Object { id, .. } = state {
                // TODO: is only valid in streaming
                return Some(
                    id.get_or_insert_with(|| BlankNode::default().into())
                        .clone()
                        .into(),
                );
            }
        }
        None
    }

    fn current_predicate(&self) -> Option<NamedNode> {
        for state in self.state.iter().rev() {
            if let JsonLdState::ObjectPredicate { predicate } = state {
                return Some(predicate.clone());
            }
        }
        None
    }

    fn current_graph_name(&self) -> GraphName {
        GraphName::DefaultGraph // TODO
    }
}

fn parse_id(value: Cow<'_, str>) -> Result<NamedOrBlankNode, JsonLdSyntaxError> {
    // TODO: lenient
    Ok(if let Some(bnode_id) = value.strip_prefix("_:") {
        BlankNode::new(bnode_id)
            .map_err(|e| JsonLdSyntaxError::msg(format!("Invalid blank node @id '{value}': {e}")))?
            .into()
    } else {
        NamedNode::new(&*value)
            .map_err(|e| JsonLdSyntaxError::msg(format!("Invalid IRI @id '{value}': {e}")))?
            .into()
    })
}

fn guess_number_datatype(number: &str) -> NamedNodeRef<'static> {
    if number.contains('e') || number.contains('E') {
        xsd::DOUBLE
    } else if number.contains('.') {
        xsd::DECIMAL // TODO: this is false!
    } else {
        xsd::INTEGER
    }
}
