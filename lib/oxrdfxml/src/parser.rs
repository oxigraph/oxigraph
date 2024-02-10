use crate::error::{RdfXmlParseError, RdfXmlSyntaxError, SyntaxErrorKind};
use crate::utils::*;
use oxilangtag::LanguageTag;
use oxiri::{Iri, IriParseError};
use oxrdf::vocab::rdf;
use oxrdf::{BlankNode, Literal, NamedNode, Subject, Term, Triple};
use quick_xml::escape::unescape_with;
use quick_xml::events::attributes::Attribute;
use quick_xml::events::*;
use quick_xml::name::{LocalName, QName, ResolveResult};
use quick_xml::{Error, NsReader, Writer};
use std::collections::{HashMap, HashSet};
use std::io::{BufReader, Read};
use std::str;
#[cfg(feature = "async-tokio")]
use tokio::io::{AsyncRead, BufReader as AsyncBufReader};

/// A [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/) streaming parser.
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
/// use oxrdfxml::RdfXmlParser;
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
/// for triple in RdfXmlParser::new().parse_read(file.as_ref()) {
///     let triple = triple?;
///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default)]
#[must_use]
pub struct RdfXmlParser {
    unchecked: bool,
    base: Option<Iri<String>>,
}

impl RdfXmlParser {
    /// Builds a new [`RdfXmlParser`].
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

    /// Parses a RDF/XML file from a [`Read`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    /// use oxrdfxml::RdfXmlParser;
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
    /// for triple in RdfXmlParser::new().parse_read(file.as_ref()) {
    ///     let triple = triple?;
    ///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn parse_read<R: Read>(self, read: R) -> FromReadRdfXmlReader<R> {
        FromReadRdfXmlReader {
            results: Vec::new(),
            reader: self.parse(BufReader::new(read)),
            reader_buffer: Vec::default(),
        }
    }

    /// Parses a RDF/XML file from a [`AsyncRead`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::NamedNodeRef;
    /// use oxrdfxml::RdfXmlParser;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), oxrdfxml::RdfXmlParseError> {
    /// let file = br#"<?xml version="1.0"?>
    /// <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:schema="http://schema.org/">
    ///   <rdf:Description rdf:about="http://example.com/foo">
    ///     <rdf:type rdf:resource="http://schema.org/Person" />
    ///     <schema:name>Foo</schema:name>
    ///   </rdf:Description>
    ///   <schema:Person rdf:about="http://example.com/bar" schema:name="Bar" />
    /// </rdf:RDF>"#;
    ///
    /// let schema_person = NamedNodeRef::new_unchecked("http://schema.org/Person");
    /// let mut count = 0;
    /// let mut parser = RdfXmlParser::new().parse_tokio_async_read(file.as_ref());
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
    ) -> FromTokioAsyncReadRdfXmlReader<R> {
        FromTokioAsyncReadRdfXmlReader {
            results: Vec::new(),
            reader: self.parse(AsyncBufReader::new(read)),
            reader_buffer: Vec::default(),
        }
    }

    fn parse<T>(&self, reader: T) -> RdfXmlReader<T> {
        let mut reader = NsReader::from_reader(reader);
        reader.expand_empty_elements(true);
        RdfXmlReader {
            reader,
            state: vec![RdfXmlState::Doc {
                base_iri: self.base.clone(),
            }],
            custom_entities: HashMap::default(),
            in_literal_depth: 0,
            known_rdf_id: HashSet::default(),
            is_end: false,
            unchecked: self.unchecked,
        }
    }
}

/// Parses a RDF/XML file from a [`Read`] implementation. Can be built using [`RdfXmlParser::parse_read`].
///
/// Count the number of people:
/// ```
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
/// use oxrdfxml::RdfXmlParser;
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
/// for triple in RdfXmlParser::new().parse_read(file.as_ref()) {
///     let triple = triple?;
///     if triple.predicate == rdf::TYPE && triple.object == schema_person.into() {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct FromReadRdfXmlReader<R: Read> {
    results: Vec<Triple>,
    reader: RdfXmlReader<BufReader<R>>,
    reader_buffer: Vec<u8>,
}

impl<R: Read> Iterator for FromReadRdfXmlReader<R> {
    type Item = Result<Triple, RdfXmlParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(triple) = self.results.pop() {
                return Some(Ok(triple));
            } else if self.reader.is_end {
                return None;
            }
            if let Err(e) = self.parse_step() {
                return Some(Err(e));
            }
        }
    }
}

impl<R: Read> FromReadRdfXmlReader<R> {
    /// The current byte position in the input data.
    pub fn buffer_position(&self) -> usize {
        self.reader.reader.buffer_position()
    }

    fn parse_step(&mut self) -> Result<(), RdfXmlParseError> {
        self.reader_buffer.clear();
        let event = self
            .reader
            .reader
            .read_event_into(&mut self.reader_buffer)?;
        self.reader.parse_event(event, &mut self.results)
    }
}

/// Parses a RDF/XML file from a [`AsyncRead`] implementation. Can be built using [`RdfXmlParser::parse_tokio_async_read`].
///
/// Count the number of people:
/// ```
/// use oxrdf::vocab::rdf;
/// use oxrdf::NamedNodeRef;
/// use oxrdfxml::RdfXmlParser;
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), oxrdfxml::RdfXmlParseError> {
/// let file = br#"<?xml version="1.0"?>
/// <rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#" xmlns:schema="http://schema.org/">
///   <rdf:Description rdf:about="http://example.com/foo">
///     <rdf:type rdf:resource="http://schema.org/Person" />
///     <schema:name>Foo</schema:name>
///   </rdf:Description>
///   <schema:Person rdf:about="http://example.com/bar" schema:name="Bar" />
/// </rdf:RDF>"#;
///
/// let schema_person = NamedNodeRef::new_unchecked("http://schema.org/Person");
/// let mut count = 0;
/// let mut parser = RdfXmlParser::new().parse_tokio_async_read(file.as_ref());
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
pub struct FromTokioAsyncReadRdfXmlReader<R: AsyncRead + Unpin> {
    results: Vec<Triple>,
    reader: RdfXmlReader<AsyncBufReader<R>>,
    reader_buffer: Vec<u8>,
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> FromTokioAsyncReadRdfXmlReader<R> {
    /// Reads the next triple or returns `None` if the file is finished.
    pub async fn next(&mut self) -> Option<Result<Triple, RdfXmlParseError>> {
        loop {
            if let Some(triple) = self.results.pop() {
                return Some(Ok(triple));
            } else if self.reader.is_end {
                return None;
            }
            if let Err(e) = self.parse_step().await {
                return Some(Err(e));
            }
        }
    }

    /// The current byte position in the input data.
    pub fn buffer_position(&self) -> usize {
        self.reader.reader.buffer_position()
    }

    async fn parse_step(&mut self) -> Result<(), RdfXmlParseError> {
        self.reader_buffer.clear();
        let event = self
            .reader
            .reader
            .read_event_into_async(&mut self.reader_buffer)
            .await?;
        self.reader.parse_event(event, &mut self.results)
    }
}

const RDF_ABOUT: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#about";
const RDF_ABOUT_EACH: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#aboutEach";
const RDF_ABOUT_EACH_PREFIX: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#aboutEachPrefix";
const RDF_BAG_ID: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#bagID";
const RDF_DATATYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#datatype";
const RDF_DESCRIPTION: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#Description";
const RDF_ID: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#ID";
const RDF_LI: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#li";
const RDF_NODE_ID: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#nodeID";
const RDF_PARSE_TYPE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#parseType";
const RDF_RDF: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#RDF";
const RDF_RESOURCE: &str = "http://www.w3.org/1999/02/22-rdf-syntax-ns#resource";

const RESERVED_RDF_ELEMENTS: [&str; 11] = [
    RDF_ABOUT,
    RDF_ABOUT_EACH,
    RDF_ABOUT_EACH_PREFIX,
    RDF_BAG_ID,
    RDF_DATATYPE,
    RDF_ID,
    RDF_LI,
    RDF_NODE_ID,
    RDF_PARSE_TYPE,
    RDF_RDF,
    RDF_RESOURCE,
];
const RESERVED_RDF_ATTRIBUTES: [&str; 5] = [
    RDF_ABOUT_EACH,
    RDF_ABOUT_EACH_PREFIX,
    RDF_LI,
    RDF_RDF,
    RDF_RESOURCE,
];

#[derive(Clone, Debug)]
enum NodeOrText {
    Node(Subject),
    Text(String),
}

enum RdfXmlState {
    Doc {
        base_iri: Option<Iri<String>>,
    },
    Rdf {
        base_iri: Option<Iri<String>>,
        language: Option<String>,
    },
    NodeElt {
        base_iri: Option<Iri<String>>,
        language: Option<String>,
        subject: Subject,
        li_counter: u64,
    },
    PropertyElt {
        // Resource, Literal or Empty property element
        iri: NamedNode,
        base_iri: Option<Iri<String>>,
        language: Option<String>,
        subject: Subject,
        object: Option<NodeOrText>,
        id_attr: Option<NamedNode>,
        datatype_attr: Option<NamedNode>,
    },
    ParseTypeCollectionPropertyElt {
        iri: NamedNode,
        base_iri: Option<Iri<String>>,
        language: Option<String>,
        subject: Subject,
        objects: Vec<Subject>,
        id_attr: Option<NamedNode>,
    },
    ParseTypeLiteralPropertyElt {
        iri: NamedNode,
        base_iri: Option<Iri<String>>,
        language: Option<String>,
        subject: Subject,
        writer: Writer<Vec<u8>>,
        id_attr: Option<NamedNode>,
        emit: bool, // false for parseTypeOtherPropertyElt support
    },
}

impl RdfXmlState {
    fn base_iri(&self) -> Option<&Iri<String>> {
        match self {
            Self::Doc { base_iri, .. }
            | Self::Rdf { base_iri, .. }
            | Self::NodeElt { base_iri, .. }
            | Self::PropertyElt { base_iri, .. }
            | Self::ParseTypeCollectionPropertyElt { base_iri, .. }
            | Self::ParseTypeLiteralPropertyElt { base_iri, .. } => base_iri.as_ref(),
        }
    }

    fn language(&self) -> Option<&String> {
        match self {
            Self::Doc { .. } => None,
            Self::Rdf { language, .. }
            | Self::NodeElt { language, .. }
            | Self::PropertyElt { language, .. }
            | Self::ParseTypeCollectionPropertyElt { language, .. }
            | Self::ParseTypeLiteralPropertyElt { language, .. } => language.as_ref(),
        }
    }
}

struct RdfXmlReader<R> {
    reader: NsReader<R>,
    state: Vec<RdfXmlState>,
    custom_entities: HashMap<String, String>,
    in_literal_depth: usize,
    known_rdf_id: HashSet<String>,
    is_end: bool,
    unchecked: bool,
}

impl<R> RdfXmlReader<R> {
    fn parse_event(
        &mut self,
        event: Event<'_>,
        results: &mut Vec<Triple>,
    ) -> Result<(), RdfXmlParseError> {
        match event {
            Event::Start(event) => self.parse_start_event(&event, results),
            Event::End(event) => self.parse_end_event(&event, results),
            Event::Empty(_) => Err(RdfXmlSyntaxError::msg(
                "The expand_empty_elements option must be enabled",
            )
            .into()),
            Event::Text(event) => self.parse_text_event(&event),
            Event::CData(event) => self.parse_text_event(&event.escape()?),
            Event::Comment(_) | Event::PI(_) => Ok(()),
            Event::Decl(decl) => {
                if let Some(encoding) = decl.encoding() {
                    if !is_utf8(&encoding?) {
                        return Err(RdfXmlSyntaxError::msg(
                            "Only UTF-8 is supported by the RDF/XML parser",
                        )
                        .into());
                    }
                }
                Ok(())
            }
            Event::DocType(dt) => self.parse_doctype(&dt),
            Event::Eof => {
                self.is_end = true;
                Ok(())
            }
        }
    }

    fn parse_doctype(&mut self, dt: &BytesText<'_>) -> Result<(), RdfXmlParseError> {
        // we extract entities
        for input in self
            .reader
            .decoder()
            .decode(dt.as_ref())?
            .split('<')
            .skip(1)
        {
            if let Some(input) = input.strip_prefix("!ENTITY") {
                let input = input.trim_start().strip_prefix('%').unwrap_or(input);
                let (entity_name, input) = input.trim_start().split_once(|c: char| c.is_ascii_whitespace()).ok_or_else(|| {
                    RdfXmlSyntaxError::msg(
                        "<!ENTITY declarations should contain both an entity name and an entity value",
                    )
                })?;
                let input = input.trim_start().strip_prefix('\"').ok_or_else(|| {
                    RdfXmlSyntaxError::msg("<!ENTITY values should be enclosed in double quotes")
                })?;
                let (entity_value, input) = input.split_once('"').ok_or_else(|| {
                    RdfXmlSyntaxError::msg(
                        "<!ENTITY declarations values should be enclosed in double quotes",
                    )
                })?;
                input.trim_start().strip_prefix('>').ok_or_else(|| {
                    RdfXmlSyntaxError::msg("<!ENTITY declarations values should end with >")
                })?;

                // Resolves custom entities within the current entity definition.
                let entity_value = unescape_with(entity_value, |e| self.resolve_entity(e))
                    .map_err(quick_xml::Error::from)?;
                self.custom_entities
                    .insert(entity_name.to_owned(), entity_value.to_string());
            }
        }
        Ok(())
    }

    fn parse_start_event(
        &mut self,
        event: &BytesStart<'_>,
        results: &mut Vec<Triple>,
    ) -> Result<(), RdfXmlParseError> {
        #[derive(PartialEq, Eq)]
        enum RdfXmlParseType {
            Default,
            Collection,
            Literal,
            Resource,
            Other,
        }

        #[derive(PartialEq, Eq)]
        enum RdfXmlNextProduction {
            Rdf,
            NodeElt,
            PropertyElt { subject: Subject },
        }

        // Literal case
        if let Some(RdfXmlState::ParseTypeLiteralPropertyElt { writer, .. }) = self.state.last_mut()
        {
            let mut clean_event = BytesStart::new(
                self.reader
                    .decoder()
                    .decode(event.name().as_ref())?
                    .to_string(),
            );
            for attr in event.attributes() {
                clean_event.push_attribute(attr.map_err(Error::InvalidAttr)?);
            }
            writer.write_event(Event::Start(clean_event))?;
            self.in_literal_depth += 1;
            return Ok(());
        }

        let tag_name = self.resolve_tag_name(event.name())?;

        // We read attributes
        let (mut language, mut base_iri) = if let Some(current_state) = self.state.last() {
            (
                current_state.language().cloned(),
                current_state.base_iri().cloned(),
            )
        } else {
            (None, None)
        };

        let mut id_attr = None;
        let mut node_id_attr = None;
        let mut about_attr = None;
        let mut property_attrs = Vec::default();
        let mut resource_attr = None;
        let mut datatype_attr = None;
        let mut parse_type = RdfXmlParseType::Default;
        let mut type_attr = None;

        for attribute in event.attributes() {
            let attribute = attribute.map_err(Error::InvalidAttr)?;
            if attribute.key.as_ref().starts_with(b"xml") {
                if attribute.key.as_ref() == b"xml:lang" {
                    let tag = self.convert_attribute(&attribute)?.to_ascii_lowercase();
                    language = Some(if self.unchecked {
                        tag
                    } else {
                        LanguageTag::parse(tag.to_ascii_lowercase())
                            .map_err(|error| {
                                RdfXmlSyntaxError(SyntaxErrorKind::InvalidLanguageTag {
                                    tag,
                                    error,
                                })
                            })?
                            .into_inner()
                    });
                } else if attribute.key.as_ref() == b"xml:base" {
                    let iri = self.convert_attribute(&attribute)?;
                    base_iri = Some(
                        if self.unchecked {
                            Iri::parse_unchecked(iri.clone())
                        } else {
                            Iri::parse(iri.clone())
                        }
                        .map_err(|error| {
                            RdfXmlSyntaxError(SyntaxErrorKind::InvalidIri { iri, error })
                        })?,
                    )
                } else {
                    // We ignore other xml attributes
                }
            } else {
                let attribute_url = self.resolve_attribute_name(attribute.key)?;
                if *attribute_url == *RDF_ID {
                    let mut id = self.convert_attribute(&attribute)?;
                    if !is_nc_name(&id) {
                        return Err(RdfXmlSyntaxError::msg(format!(
                            "{id} is not a valid rdf:ID value"
                        ))
                        .into());
                    }
                    id.insert(0, '#');
                    id_attr = Some(id);
                } else if *attribute_url == *RDF_BAG_ID {
                    let bag_id = self.convert_attribute(&attribute)?;
                    if !is_nc_name(&bag_id) {
                        return Err(RdfXmlSyntaxError::msg(format!(
                            "{bag_id} is not a valid rdf:bagID value"
                        ))
                        .into());
                    }
                } else if *attribute_url == *RDF_NODE_ID {
                    let id = self.convert_attribute(&attribute)?;
                    if !is_nc_name(&id) {
                        return Err(RdfXmlSyntaxError::msg(format!(
                            "{id} is not a valid rdf:nodeID value"
                        ))
                        .into());
                    }
                    node_id_attr = Some(BlankNode::new_unchecked(id));
                } else if *attribute_url == *RDF_ABOUT {
                    about_attr = Some(attribute);
                } else if *attribute_url == *RDF_RESOURCE {
                    resource_attr = Some(attribute);
                } else if *attribute_url == *RDF_DATATYPE {
                    datatype_attr = Some(attribute);
                } else if *attribute_url == *RDF_PARSE_TYPE {
                    parse_type = match attribute.value.as_ref() {
                        b"Collection" => RdfXmlParseType::Collection,
                        b"Literal" => RdfXmlParseType::Literal,
                        b"Resource" => RdfXmlParseType::Resource,
                        _ => RdfXmlParseType::Other,
                    };
                } else if attribute_url == rdf::TYPE.as_str() {
                    type_attr = Some(attribute);
                } else if RESERVED_RDF_ATTRIBUTES.contains(&&*attribute_url) {
                    return Err(RdfXmlSyntaxError::msg(format!(
                        "{attribute_url} is not a valid attribute"
                    ))
                    .into());
                } else {
                    property_attrs.push((
                        self.parse_iri(attribute_url)?,
                        self.convert_attribute(&attribute)?,
                    ));
                }
            }
        }

        // Parsing with the base URI
        let id_attr = match id_attr {
            Some(iri) => {
                let iri = self.resolve_iri(&base_iri, iri)?;
                if self.known_rdf_id.contains(iri.as_str()) {
                    return Err(RdfXmlSyntaxError::msg(format!(
                        "{iri} has already been used as rdf:ID value"
                    ))
                    .into());
                }
                self.known_rdf_id.insert(iri.as_str().into());
                Some(iri)
            }
            None => None,
        };
        let about_attr = match about_attr {
            Some(attr) => Some(self.convert_iri_attribute(&base_iri, &attr)?),
            None => None,
        };
        let resource_attr = match resource_attr {
            Some(attr) => Some(self.convert_iri_attribute(&base_iri, &attr)?),
            None => None,
        };
        let datatype_attr = match datatype_attr {
            Some(attr) => Some(self.convert_iri_attribute(&base_iri, &attr)?),
            None => None,
        };
        let type_attr = match type_attr {
            Some(attr) => Some(self.convert_iri_attribute(&base_iri, &attr)?),
            None => None,
        };

        let expected_production = match self.state.last() {
            Some(RdfXmlState::Doc { .. }) => RdfXmlNextProduction::Rdf,
            Some(
                RdfXmlState::Rdf { .. }
                | RdfXmlState::PropertyElt { .. }
                | RdfXmlState::ParseTypeCollectionPropertyElt { .. },
            ) => RdfXmlNextProduction::NodeElt,
            Some(RdfXmlState::NodeElt { subject, .. }) => RdfXmlNextProduction::PropertyElt {
                subject: subject.clone(),
            },
            Some(RdfXmlState::ParseTypeLiteralPropertyElt { .. }) => {
                return Err(
                    RdfXmlSyntaxError::msg("ParseTypeLiteralPropertyElt production children should never be considered as a RDF/XML content").into()
                );
            }
            None => {
                return Err(RdfXmlSyntaxError::msg(
                    "No state in the stack: the XML is not balanced",
                )
                .into());
            }
        };

        let new_state = match expected_production {
            RdfXmlNextProduction::Rdf => {
                if *tag_name == *RDF_RDF {
                    RdfXmlState::Rdf { base_iri, language }
                } else if RESERVED_RDF_ELEMENTS.contains(&&*tag_name) {
                    return Err(RdfXmlSyntaxError::msg(format!(
                        "Invalid node element tag name: {tag_name}"
                    ))
                    .into());
                } else {
                    Self::build_node_elt(
                        self.parse_iri(tag_name)?,
                        base_iri,
                        language,
                        id_attr,
                        node_id_attr,
                        about_attr,
                        type_attr,
                        property_attrs,
                        results,
                    )?
                }
            }
            RdfXmlNextProduction::NodeElt => {
                if RESERVED_RDF_ELEMENTS.contains(&&*tag_name) {
                    return Err(RdfXmlSyntaxError::msg(format!(
                        "Invalid property element tag name: {tag_name}"
                    ))
                    .into());
                }
                Self::build_node_elt(
                    self.parse_iri(tag_name)?,
                    base_iri,
                    language,
                    id_attr,
                    node_id_attr,
                    about_attr,
                    type_attr,
                    property_attrs,
                    results,
                )?
            }
            RdfXmlNextProduction::PropertyElt { subject } => {
                let iri = if *tag_name == *RDF_LI {
                    let Some(RdfXmlState::NodeElt { li_counter, .. }) = self.state.last_mut()
                    else {
                        return Err(RdfXmlSyntaxError::msg(format!(
                            "Invalid property element tag name: {tag_name}"
                        ))
                        .into());
                    };
                    *li_counter += 1;
                    NamedNode::new_unchecked(format!(
                        "http://www.w3.org/1999/02/22-rdf-syntax-ns#_{li_counter}"
                    ))
                } else if RESERVED_RDF_ELEMENTS.contains(&&*tag_name)
                    || *tag_name == *RDF_DESCRIPTION
                {
                    return Err(RdfXmlSyntaxError::msg(format!(
                        "Invalid property element tag name: {tag_name}"
                    ))
                    .into());
                } else {
                    self.parse_iri(tag_name)?
                };
                match parse_type {
                    RdfXmlParseType::Default => {
                        if resource_attr.is_some()
                            || node_id_attr.is_some()
                            || !property_attrs.is_empty()
                        {
                            let object = match (resource_attr, node_id_attr)
                    {
                        (Some(resource_attr), None) => Subject::from(resource_attr),
                        (None, Some(node_id_attr)) => node_id_attr.into(),
                        (None, None) => BlankNode::default().into(),
                        (Some(_), Some(_)) => return Err(RdfXmlSyntaxError::msg("Not both rdf:resource and rdf:nodeID could be set at the same time").into())
                    };
                            Self::emit_property_attrs(&object, property_attrs, &language, results);
                            if let Some(type_attr) = type_attr {
                                results.push(Triple::new(object.clone(), rdf::TYPE, type_attr));
                            }
                            RdfXmlState::PropertyElt {
                                iri,
                                base_iri,
                                language,
                                subject,
                                object: Some(NodeOrText::Node(object)),
                                id_attr,
                                datatype_attr,
                            }
                        } else {
                            RdfXmlState::PropertyElt {
                                iri,
                                base_iri,
                                language,
                                subject,
                                object: None,
                                id_attr,
                                datatype_attr,
                            }
                        }
                    }
                    RdfXmlParseType::Literal => RdfXmlState::ParseTypeLiteralPropertyElt {
                        iri,
                        base_iri,
                        language,
                        subject,
                        writer: Writer::new(Vec::default()),
                        id_attr,
                        emit: true,
                    },
                    RdfXmlParseType::Resource => Self::build_parse_type_resource_property_elt(
                        iri, base_iri, language, subject, id_attr, results,
                    ),
                    RdfXmlParseType::Collection => RdfXmlState::ParseTypeCollectionPropertyElt {
                        iri,
                        base_iri,
                        language,
                        subject,
                        objects: Vec::default(),
                        id_attr,
                    },
                    RdfXmlParseType::Other => RdfXmlState::ParseTypeLiteralPropertyElt {
                        iri,
                        base_iri,
                        language,
                        subject,
                        writer: Writer::new(Vec::default()),
                        id_attr,
                        emit: false,
                    },
                }
            }
        };
        self.state.push(new_state);
        Ok(())
    }

    fn parse_end_event(
        &mut self,
        event: &BytesEnd<'_>,
        results: &mut Vec<Triple>,
    ) -> Result<(), RdfXmlParseError> {
        // Literal case
        if self.in_literal_depth > 0 {
            if let Some(RdfXmlState::ParseTypeLiteralPropertyElt { writer, .. }) =
                self.state.last_mut()
            {
                writer.write_event(Event::End(BytesEnd::new(
                    self.reader.decoder().decode(event.name().as_ref())?,
                )))?;
                self.in_literal_depth -= 1;
                return Ok(());
            }
        }

        if let Some(current_state) = self.state.pop() {
            self.end_state(current_state, results)?;
        }
        Ok(())
    }

    fn parse_text_event(&mut self, event: &BytesText<'_>) -> Result<(), RdfXmlParseError> {
        let text = event.unescape_with(|e| self.resolve_entity(e))?.to_string();
        match self.state.last_mut() {
            Some(RdfXmlState::PropertyElt { object, .. }) => {
                if !event.iter().copied().all(is_whitespace) {
                    *object = Some(NodeOrText::Text(text));
                }
                Ok(())
            }
            Some(RdfXmlState::ParseTypeLiteralPropertyElt { writer, .. }) => {
                writer.write_event(Event::Text(BytesText::new(&text)))?;
                Ok(())
            }
            _ => {
                if event.iter().copied().all(is_whitespace) {
                    Ok(())
                } else {
                    Err(RdfXmlSyntaxError::msg(format!("Unexpected text event: '{text}'")).into())
                }
            }
        }
    }

    fn resolve_tag_name(&self, qname: QName<'_>) -> Result<String, RdfXmlParseError> {
        let (namespace, local_name) = self.reader.resolve_element(qname);
        self.resolve_ns_name(namespace, local_name)
    }

    fn resolve_attribute_name(&self, qname: QName<'_>) -> Result<String, RdfXmlParseError> {
        let (namespace, local_name) = self.reader.resolve_attribute(qname);
        self.resolve_ns_name(namespace, local_name)
    }

    fn resolve_ns_name(
        &self,
        namespace: ResolveResult<'_>,
        local_name: LocalName<'_>,
    ) -> Result<String, RdfXmlParseError> {
        match namespace {
            ResolveResult::Bound(ns) => {
                let mut value = Vec::with_capacity(ns.as_ref().len() + local_name.as_ref().len());
                value.extend_from_slice(ns.as_ref());
                value.extend_from_slice(local_name.as_ref());
                Ok(unescape_with(&self.reader.decoder().decode(&value)?, |e| {
                    self.resolve_entity(e)
                })
                .map_err(quick_xml::Error::from)?
                .to_string())
            }
            ResolveResult::Unbound => {
                Err(RdfXmlSyntaxError::msg("XML namespaces are required in RDF/XML").into())
            }
            ResolveResult::Unknown(v) => Err(RdfXmlSyntaxError::msg(format!(
                "Unknown prefix {}:",
                self.reader.decoder().decode(&v)?
            ))
            .into()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn build_node_elt(
        iri: NamedNode,
        base_iri: Option<Iri<String>>,
        language: Option<String>,
        id_attr: Option<NamedNode>,
        node_id_attr: Option<BlankNode>,
        about_attr: Option<NamedNode>,
        type_attr: Option<NamedNode>,
        property_attrs: Vec<(NamedNode, String)>,
        results: &mut Vec<Triple>,
    ) -> Result<RdfXmlState, RdfXmlSyntaxError> {
        let subject = match (id_attr, node_id_attr, about_attr) {
            (Some(id_attr), None, None) => Subject::from(id_attr),
            (None, Some(node_id_attr), None) => node_id_attr.into(),
            (None, None, Some(about_attr)) => about_attr.into(),
            (None, None, None) => BlankNode::default().into(),
            (Some(_), Some(_), _) => {
                return Err(RdfXmlSyntaxError::msg(
                    "Not both rdf:ID and rdf:nodeID could be set at the same time",
                ))
            }
            (_, Some(_), Some(_)) => {
                return Err(RdfXmlSyntaxError::msg(
                    "Not both rdf:nodeID and rdf:resource could be set at the same time",
                ))
            }
            (Some(_), _, Some(_)) => {
                return Err(RdfXmlSyntaxError::msg(
                    "Not both rdf:ID and rdf:resource could be set at the same time",
                ))
            }
        };

        Self::emit_property_attrs(&subject, property_attrs, &language, results);

        if let Some(type_attr) = type_attr {
            results.push(Triple::new(subject.clone(), rdf::TYPE, type_attr));
        }

        if iri != *RDF_DESCRIPTION {
            results.push(Triple::new(subject.clone(), rdf::TYPE, iri));
        }
        Ok(RdfXmlState::NodeElt {
            base_iri,
            language,
            subject,
            li_counter: 0,
        })
    }

    fn build_parse_type_resource_property_elt(
        iri: NamedNode,
        base_iri: Option<Iri<String>>,
        language: Option<String>,
        subject: Subject,
        id_attr: Option<NamedNode>,
        results: &mut Vec<Triple>,
    ) -> RdfXmlState {
        let object = BlankNode::default();
        let triple = Triple::new(subject, iri, object.clone());
        if let Some(id_attr) = id_attr {
            Self::reify(triple.clone(), id_attr, results);
        }
        results.push(triple);
        RdfXmlState::NodeElt {
            base_iri,
            language,
            subject: object.into(),
            li_counter: 0,
        }
    }

    fn end_state(
        &mut self,
        state: RdfXmlState,
        results: &mut Vec<Triple>,
    ) -> Result<(), RdfXmlSyntaxError> {
        match state {
            RdfXmlState::PropertyElt {
                iri,
                language,
                subject,
                id_attr,
                datatype_attr,
                object,
                ..
            } => {
                let object = match object {
                    Some(NodeOrText::Node(node)) => Term::from(node),
                    Some(NodeOrText::Text(text)) => {
                        Self::new_literal(text, language, datatype_attr).into()
                    }
                    None => Self::new_literal(String::new(), language, datatype_attr).into(),
                };
                let triple = Triple::new(subject, iri, object);
                if let Some(id_attr) = id_attr {
                    Self::reify(triple.clone(), id_attr, results);
                }
                results.push(triple);
            }
            RdfXmlState::ParseTypeCollectionPropertyElt {
                iri,
                subject,
                id_attr,
                objects,
                ..
            } => {
                let mut current_node = Subject::from(rdf::NIL);
                for object in objects.into_iter().rev() {
                    let subject = Subject::from(BlankNode::default());
                    results.push(Triple::new(subject.clone(), rdf::FIRST, object));
                    results.push(Triple::new(subject.clone(), rdf::REST, current_node));
                    current_node = subject;
                }
                let triple = Triple::new(subject, iri, current_node);
                if let Some(id_attr) = id_attr {
                    Self::reify(triple.clone(), id_attr, results);
                }
                results.push(triple);
            }
            RdfXmlState::ParseTypeLiteralPropertyElt {
                iri,
                subject,
                id_attr,
                writer,
                emit,
                ..
            } => {
                if emit {
                    let object = writer.into_inner();
                    if object.is_empty() {
                        return Err(RdfXmlSyntaxError::msg(format!(
                            "No value found for rdf:XMLLiteral value of property {iri}"
                        )));
                    }
                    let triple = Triple::new(
                        subject,
                        iri,
                        Literal::new_typed_literal(
                            str::from_utf8(&object).map_err(|_| {
                                RdfXmlSyntaxError::msg(
                                    "The XML literal is not in valid UTF-8".to_owned(),
                                )
                            })?,
                            rdf::XML_LITERAL,
                        ),
                    );
                    if let Some(id_attr) = id_attr {
                        Self::reify(triple.clone(), id_attr, results);
                    }
                    results.push(triple);
                }
            }
            RdfXmlState::NodeElt { subject, .. } => match self.state.last_mut() {
                Some(RdfXmlState::PropertyElt { object, .. }) => {
                    *object = Some(NodeOrText::Node(subject))
                }
                Some(RdfXmlState::ParseTypeCollectionPropertyElt { objects, .. }) => {
                    objects.push(subject)
                }
                _ => (),
            },
            _ => (),
        }
        Ok(())
    }

    fn new_literal(
        value: String,
        language: Option<String>,
        datatype: Option<NamedNode>,
    ) -> Literal {
        if let Some(datatype) = datatype {
            Literal::new_typed_literal(value, datatype)
        } else if let Some(language) = language {
            Literal::new_language_tagged_literal_unchecked(value, language)
        } else {
            Literal::new_simple_literal(value)
        }
    }

    fn reify(triple: Triple, statement_id: NamedNode, results: &mut Vec<Triple>) {
        results.push(Triple::new(statement_id.clone(), rdf::TYPE, rdf::STATEMENT));
        results.push(Triple::new(
            statement_id.clone(),
            rdf::SUBJECT,
            triple.subject,
        ));
        results.push(Triple::new(
            statement_id.clone(),
            rdf::PREDICATE,
            triple.predicate,
        ));
        results.push(Triple::new(statement_id, rdf::OBJECT, triple.object));
    }

    fn emit_property_attrs(
        subject: &Subject,
        literal_attributes: Vec<(NamedNode, String)>,
        language: &Option<String>,
        results: &mut Vec<Triple>,
    ) {
        for (literal_predicate, literal_value) in literal_attributes {
            results.push(Triple::new(
                subject.clone(),
                literal_predicate,
                if let Some(language) = language.clone() {
                    Literal::new_language_tagged_literal_unchecked(literal_value, language)
                } else {
                    Literal::new_simple_literal(literal_value)
                },
            ));
        }
    }

    fn convert_attribute(&self, attribute: &Attribute<'_>) -> Result<String, RdfXmlParseError> {
        Ok(attribute
            .decode_and_unescape_value_with(&self.reader, |e| self.resolve_entity(e))?
            .into_owned())
    }

    fn convert_iri_attribute(
        &self,
        base_iri: &Option<Iri<String>>,
        attribute: &Attribute<'_>,
    ) -> Result<NamedNode, RdfXmlParseError> {
        Ok(self.resolve_iri(base_iri, self.convert_attribute(attribute)?)?)
    }

    fn resolve_iri(
        &self,
        base_iri: &Option<Iri<String>>,
        relative_iri: String,
    ) -> Result<NamedNode, RdfXmlSyntaxError> {
        if let Some(base_iri) = base_iri {
            Ok(NamedNode::new_unchecked(
                if self.unchecked {
                    base_iri.resolve_unchecked(&relative_iri)
                } else {
                    base_iri.resolve(&relative_iri)
                }
                .map_err(|error| {
                    RdfXmlSyntaxError(SyntaxErrorKind::InvalidIri {
                        iri: relative_iri,
                        error,
                    })
                })?
                .into_inner(),
            ))
        } else {
            self.parse_iri(relative_iri)
        }
    }

    fn parse_iri(&self, relative_iri: String) -> Result<NamedNode, RdfXmlSyntaxError> {
        Ok(NamedNode::new_unchecked(if self.unchecked {
            relative_iri
        } else {
            Iri::parse(relative_iri.clone())
                .map_err(|error| {
                    RdfXmlSyntaxError(SyntaxErrorKind::InvalidIri {
                        iri: relative_iri,
                        error,
                    })
                })?
                .into_inner()
        }))
    }

    fn resolve_entity(&self, e: &str) -> Option<&str> {
        self.custom_entities.get(e).map(String::as_str)
    }
}

fn is_nc_name(name: &str) -> bool {
    // Name - (Char* ':' Char*)
    is_name(name) && name.chars().all(|c| c != ':')
}

fn is_name(name: &str) -> bool {
    // NameStartChar (NameChar)*
    let mut c = name.chars();
    if !c.next().map_or(false, is_name_start_char) {
        return false;
    }
    c.all(is_name_char)
}

fn is_whitespace(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | b'\r')
}

fn is_utf8(encoding: &[u8]) -> bool {
    matches!(
        encoding.to_ascii_lowercase().as_slice(),
        b"unicode-1-1-utf-8"
            | b"unicode11utf8"
            | b"unicode20utf8"
            | b"utf-8"
            | b"utf8"
            | b"x-unicode20utf8"
    )
}
