#[cfg(feature = "async-tokio")]
use json_event_parser::TokioAsyncWriterJsonSerializer;
use json_event_parser::{JsonEvent, WriterJsonSerializer};
use oxiri::{Iri, IriParseError};
use oxrdf::vocab::xsd;
use oxrdf::{
    GraphName, GraphNameRef, NamedNode, NamedOrBlankNodeRef, QuadRef, Subject, SubjectRef, TermRef,
};
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::io::Write;
#[cfg(feature = "async-tokio")]
use tokio::io::AsyncWrite;

/// A [JSON-LD](https://www.w3.org/TR/rdf-syntax-grammar/) serializer.
///
/// ```
/// use oxrdf::{LiteralRef, NamedNodeRef, QuadRef};
/// use oxrdf::vocab::rdf;
/// use oxjsonld::JsonLdSerializer;
///
/// let mut serializer = JsonLdSerializer::new().with_prefix("schema", "http://schema.org/")?.for_writer(Vec::new());
/// serializer.serialize_quad(QuadRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     rdf::TYPE,
///     NamedNodeRef::new("http://schema.org/Person")?,
/// ))?;
/// serializer.serialize_quad(QuadRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://schema.org/name")?,
///     LiteralRef::new_language_tagged_literal_unchecked("Foo Bar", "en"),
/// ))?;
/// assert_eq!(
///     b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rdf:RDF xmlns:schema=\"http://schema.org/\" xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\t<schema:Person rdf:about=\"http://example.com#me\">\n\t\t<schema:name xml:lang=\"en\">Foo Bar</schema:name>\n\t</schema:Person>\n</rdf:RDF>",
///     serializer.finish()?.as_slice()
/// );
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default, Clone)]
#[must_use]
pub struct JsonLdSerializer {
    prefixes: BTreeMap<String, String>,
    base_iri: Option<Iri<String>>,
}

impl JsonLdSerializer {
    /// Builds a new [`JsonLdSerializer`].
    #[inline]
    pub fn new() -> Self {
        Self {
            prefixes: BTreeMap::new(),
            base_iri: None,
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

    /// ```
    /// use oxrdf::{NamedNodeRef, QuadRef};
    /// use oxjsonld::JsonLdSerializer;
    ///
    /// let mut serializer = JsonLdSerializer::new()
    ///     .with_base_iri("http://example.com")?
    ///     .with_prefix("ex", "http://example.com/ns#")?
    ///     .for_writer(Vec::new());
    /// serializer.serialize_quad(QuadRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
    ///     NamedNodeRef::new("http://example.com/ns#Person")?,
    /// ))?;
    /// serializer.serialize_quad(QuadRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     NamedNodeRef::new("http://example.com/ns#parent")?,
    ///     NamedNodeRef::new("http://example.com#other")?,
    /// ))?;
    /// assert_eq!(
    ///     b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rdf:RDF xml:base=\"http://example.com\" xmlns:ex=\"http://example.com/ns#\" xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\t<ex:Person rdf:about=\"#me\">\n\t\t<ex:parent rdf:resource=\"#other\"/>\n\t</ex:Person>\n</rdf:RDF>",
    ///     serializer.finish()?.as_slice()
    /// );
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_base_iri(mut self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        self.base_iri = Some(Iri::parse(base_iri.into())?);
        Ok(self)
    }

    /// Serializes a JSON-LD file to a [`Write`] implementation.
    ///
    /// This writer does unbuffered writes.
    ///
    /// ```
    /// use oxrdf::{LiteralRef, NamedNodeRef, QuadRef};
    /// use oxrdf::vocab::rdf;
    /// use oxjsonld::JsonLdSerializer;
    ///
    /// let mut serializer = JsonLdSerializer::new().with_prefix("schema", "http://schema.org/")?.for_writer(Vec::new());
    /// serializer.serialize_quad(QuadRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     rdf::TYPE,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    /// ))?;
    /// serializer.serialize_quad(QuadRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     NamedNodeRef::new("http://schema.org/name")?,
    ///     LiteralRef::new_language_tagged_literal_unchecked("Foo Bar", "en"),
    /// ))?;
    /// assert_eq!(
    ///     b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rdf:RDF xmlns:schema=\"http://schema.org/\" xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\t<schema:Person rdf:about=\"http://example.com#me\">\n\t\t<schema:name xml:lang=\"en\">Foo Bar</schema:name>\n\t</schema:Person>\n</rdf:RDF>",
    ///     serializer.finish()?.as_slice()
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[allow(clippy::unused_self)]
    pub fn for_writer<W: Write>(self, writer: W) -> WriterJsonLdSerializer<W> {
        WriterJsonLdSerializer {
            writer: WriterJsonSerializer::new(writer),
            inner: self.inner_writer(),
        }
    }

    /// Serializes a JSON-LD file to a [`AsyncWrite`] implementation.
    ///
    /// This writer does unbuffered writes.
    ///
    /// ```
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use oxrdf::{NamedNodeRef, QuadRef, LiteralRef};
    /// use oxrdf::vocab::rdf;
    /// use oxjsonld::JsonLdSerializer;
    ///
    /// let mut serializer = JsonLdSerializer::new().with_prefix("schema", "http://schema.org/")?.for_tokio_async_writer(Vec::new());
    /// serializer.serialize_quad(QuadRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     rdf::TYPE,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    /// )).await?;
    /// serializer.serialize_quad(QuadRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     NamedNodeRef::new("http://schema.org/name")?,
    ///     LiteralRef::new_language_tagged_literal_unchecked("Foo Bar", "en"),
    /// )).await?;
    /// assert_eq!(
    ///     b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rdf:RDF xmlns:schema=\"http://schema.org/\" xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\t<schema:Person rdf:about=\"http://example.com#me\">\n\t\t<schema:name xml:lang=\"en\">Foo Bar</schema:name>\n\t</schema:Person>\n</rdf:RDF>",
    ///     serializer.finish().await?.as_slice()
    /// );
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::unused_self)]
    #[cfg(feature = "async-tokio")]
    pub fn for_tokio_async_writer<W: AsyncWrite + Unpin>(
        self,
        writer: W,
    ) -> TokioAsyncWriterJsonLdSerializer<W> {
        TokioAsyncWriterJsonLdSerializer {
            writer: TokioAsyncWriterJsonSerializer::new(writer),
            inner: self.inner_writer(),
        }
    }

    fn inner_writer(self) -> InnerJsonLdWriter {
        InnerJsonLdWriter {
            started: false,
            current_graph_name: None,
            current_subject: None,
            current_predicate: None,
            emitted_predicates: BTreeSet::new(),
            prefixes: self.prefixes,
            base_iri: self.base_iri,
        }
    }
}

/// Serializes a JSON-LD file to a [`Write`] implementation.
///
/// Can be built using [`JsonLdSerializer::for_writer`].
///
/// ```
/// use oxrdf::{LiteralRef, NamedNodeRef, QuadRef};
/// use oxrdf::vocab::rdf;
/// use oxjsonld::JsonLdSerializer;
///
/// let mut serializer = JsonLdSerializer::new().with_prefix("schema", "http://schema.org/")?.for_writer(Vec::new());
/// serializer.serialize_quad(QuadRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     rdf::TYPE,
///     NamedNodeRef::new("http://schema.org/Person")?,
/// ))?;
/// serializer.serialize_quad(QuadRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://schema.org/name")?,
///     LiteralRef::new_language_tagged_literal_unchecked("Foo Bar", "en"),
/// ))?;
/// assert_eq!(
///     b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rdf:RDF xmlns:schema=\"http://schema.org/\" xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\t<schema:Person rdf:about=\"http://example.com#me\">\n\t\t<schema:name xml:lang=\"en\">Foo Bar</schema:name>\n\t</schema:Person>\n</rdf:RDF>",
///     serializer.finish()?.as_slice()
/// );
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct WriterJsonLdSerializer<W: Write> {
    writer: WriterJsonSerializer<W>,
    inner: InnerJsonLdWriter,
}

impl<W: Write> WriterJsonLdSerializer<W> {
    /// Serializes an extra triple.
    pub fn serialize_quad<'a>(&mut self, t: impl Into<QuadRef<'a>>) -> io::Result<()> {
        let mut buffer = Vec::new();
        self.inner.serialize_quad(t, &mut buffer)?;
        self.flush_buffer(&mut buffer)
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub fn finish(mut self) -> io::Result<W> {
        let mut buffer = Vec::new();
        self.inner.finish(&mut buffer);
        self.flush_buffer(&mut buffer)?;
        self.writer.finish()
    }

    fn flush_buffer(&mut self, buffer: &mut Vec<JsonEvent<'_>>) -> io::Result<()> {
        for event in buffer.drain(0..) {
            self.writer.serialize_event(event)?;
        }
        Ok(())
    }
}

/// Serializes a JSON-LD file to a [`AsyncWrite`] implementation.
///
/// Can be built using [`JsonLdSerializer::for_tokio_async_writer`].
///
/// ```
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use oxrdf::{NamedNodeRef, QuadRef, LiteralRef};
/// use oxrdf::vocab::rdf;
/// use oxjsonld::JsonLdSerializer;
///
/// let mut serializer = JsonLdSerializer::new().with_prefix("schema", "http://schema.org/")?.for_tokio_async_writer(Vec::new());
/// serializer.serialize_quad(QuadRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     rdf::TYPE,
///     NamedNodeRef::new("http://schema.org/Person")?,
/// )).await?;
/// serializer.serialize_quad(QuadRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://schema.org/name")?,
///     LiteralRef::new_language_tagged_literal_unchecked("Foo Bar", "en"),
/// )).await?;
/// assert_eq!(
///     b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rdf:RDF xmlns:schema=\"http://schema.org/\" xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\t<schema:Person rdf:about=\"http://example.com#me\">\n\t\t<schema:name xml:lang=\"en\">Foo Bar</schema:name>\n\t</schema:Person>\n</rdf:RDF>",
///     serializer.finish().await?.as_slice()
/// );
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async-tokio")]
#[must_use]
pub struct TokioAsyncWriterJsonLdSerializer<W: AsyncWrite + Unpin> {
    writer: TokioAsyncWriterJsonSerializer<W>,
    inner: InnerJsonLdWriter,
}

#[cfg(feature = "async-tokio")]
impl<W: AsyncWrite + Unpin> TokioAsyncWriterJsonLdSerializer<W> {
    /// Serializes an extra triple.
    pub async fn serialize_quad<'a>(&mut self, t: impl Into<QuadRef<'a>>) -> io::Result<()> {
        let mut buffer = Vec::new();
        self.inner.serialize_quad(t, &mut buffer)?;
        self.flush_buffer(&mut buffer).await
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub async fn finish(mut self) -> io::Result<W> {
        let mut buffer = Vec::new();
        self.inner.finish(&mut buffer);
        self.flush_buffer(&mut buffer).await?;
        self.writer.finish()
    }

    async fn flush_buffer(&mut self, buffer: &mut Vec<JsonEvent<'_>>) -> io::Result<()> {
        for event in buffer.drain(0..) {
            self.writer.serialize_event(event).await?;
        }
        Ok(())
    }
}

pub struct InnerJsonLdWriter {
    started: bool,
    current_graph_name: Option<GraphName>,
    current_subject: Option<Subject>,
    current_predicate: Option<NamedNode>,
    emitted_predicates: BTreeSet<String>,
    prefixes: BTreeMap<String, String>,
    base_iri: Option<Iri<String>>,
}

impl InnerJsonLdWriter {
    fn serialize_quad<'a>(
        &mut self,
        quad: impl Into<QuadRef<'a>>,
        output: &mut Vec<JsonEvent<'a>>,
    ) -> io::Result<()> {
        if !self.started {
            self.serialize_start(output);
            self.started = true;
        }

        let quad = quad.into();
        if self
            .current_graph_name
            .as_ref()
            .is_some_and(|graph_name| graph_name.as_ref() != quad.graph_name)
        {
            output.push(JsonEvent::EndArray);
            output.push(JsonEvent::EndObject);
            if self
                .current_graph_name
                .as_ref()
                .is_some_and(|g| !g.is_default_graph())
            {
                output.push(JsonEvent::EndArray);
                output.push(JsonEvent::EndObject);
            }
            self.current_graph_name = None;
            self.current_subject = None;
            self.current_predicate = None;
            self.emitted_predicates.clear();
        } else if self
            .current_subject
            .as_ref()
            .is_some_and(|subject| subject.as_ref() != quad.subject)
            || self
                .current_predicate
                .as_ref()
                .is_some_and(|predicate| predicate.as_ref() != quad.predicate)
                && self.emitted_predicates.contains(quad.predicate.as_str())
        {
            output.push(JsonEvent::EndArray);
            output.push(JsonEvent::EndObject);
            self.current_subject = None;
            self.emitted_predicates.clear();
            self.current_predicate = None;
        } else if self
            .current_predicate
            .as_ref()
            .is_some_and(|predicate| predicate.as_ref() != quad.predicate)
        {
            output.push(JsonEvent::EndArray);
            if let Some(current_predicate) = self.current_predicate.take() {
                self.emitted_predicates
                    .insert(current_predicate.into_string());
            }
        }

        if self.current_graph_name.is_none() {
            if !quad.graph_name.is_default_graph() {
                // We open a new graph name
                output.push(JsonEvent::StartObject);
                output.push(JsonEvent::ObjectKey("@id".into()));
                output.push(JsonEvent::String(self.id_value(match quad.graph_name {
                    GraphNameRef::NamedNode(iri) => iri.into(),
                    GraphNameRef::BlankNode(bnode) => bnode.into(),
                    GraphNameRef::DefaultGraph => unreachable!(),
                })));
                output.push(JsonEvent::ObjectKey("@graph".into()));
                output.push(JsonEvent::StartArray);
            }
            self.current_graph_name = Some(quad.graph_name.into_owned());
        }

        // We open a new subject block if useful (ie. new subject or already used predicate)
        if self.current_subject.is_none() {
            output.push(JsonEvent::StartObject);
            output.push(JsonEvent::ObjectKey("@id".into()));
            #[allow(clippy::match_wildcard_for_single_variants, unreachable_patterns)]
            output.push(JsonEvent::String(self.id_value(match quad.subject {
                SubjectRef::NamedNode(iri) => iri.into(),
                SubjectRef::BlankNode(bnode) => bnode.into(),
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "JSON-LD does not support RDF-star yet",
                    ))
                }
            })));
            self.current_subject = Some(quad.subject.into_owned());
        }

        // We open a predicate key
        if self.current_predicate.is_none() {
            output.push(JsonEvent::ObjectKey(
                // TODO: use @type
                quad.predicate.as_str().into(), // TODO: prefixes including @vocab
            ));
            output.push(JsonEvent::StartArray);
            self.current_predicate = Some(quad.predicate.into_owned());
        }

        self.serialize_term(quad.object, output)
    }

    fn serialize_start(&self, output: &mut Vec<JsonEvent<'_>>) {
        if self.base_iri.is_some() || !self.prefixes.is_empty() {
            output.push(JsonEvent::StartObject);
            output.push(JsonEvent::ObjectKey("@context".into()));
            output.push(JsonEvent::StartObject);
            if let Some(base_iri) = &self.base_iri {
                output.push(JsonEvent::ObjectKey("@base".into()));
                output.push(JsonEvent::String(base_iri.to_string().into()));
            }
            for (prefix_name, prefix_iri) in &self.prefixes {
                output.push(JsonEvent::ObjectKey(if prefix_name.is_empty() {
                    "@vocab".into()
                } else {
                    prefix_name.clone().into()
                }));
                output.push(JsonEvent::String(prefix_iri.clone().into()));
            }
            output.push(JsonEvent::EndObject);
            output.push(JsonEvent::ObjectKey("@graph".into()));
        }
        output.push(JsonEvent::StartArray);
    }

    fn serialize_term<'a>(
        &self,
        term: TermRef<'a>,
        output: &mut Vec<JsonEvent<'a>>,
    ) -> io::Result<()> {
        output.push(JsonEvent::StartObject);
        #[allow(clippy::match_wildcard_for_single_variants, unreachable_patterns)]
        match term {
            TermRef::NamedNode(iri) => {
                output.push(JsonEvent::ObjectKey("@id".into()));
                output.push(JsonEvent::String(self.id_value(iri.into())));
            }
            TermRef::BlankNode(bnode) => {
                output.push(JsonEvent::ObjectKey("@id".into()));
                output.push(JsonEvent::String(self.id_value(bnode.into())));
            }
            TermRef::Literal(literal) => {
                output.push(JsonEvent::ObjectKey("@value".into()));
                output.push(JsonEvent::String(literal.value().into()));
                if let Some(language) = literal.language() {
                    output.push(JsonEvent::ObjectKey("@language".into()));
                    output.push(JsonEvent::String(language.into()));
                } else if literal.datatype() != xsd::STRING {
                    output.push(JsonEvent::ObjectKey("@type".into()));
                    output.push(JsonEvent::String(self.id_value(literal.datatype().into())));
                }
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "JSON-LD does not support RDF-star yet",
                ))
            }
        }
        output.push(JsonEvent::EndObject);
        Ok(())
    }

    fn id_value<'a>(&self, id: NamedOrBlankNodeRef<'a>) -> Cow<'a, str> {
        match id {
            NamedOrBlankNodeRef::NamedNode(iri) => relative_iri(iri.as_str(), &self.base_iri),
            NamedOrBlankNodeRef::BlankNode(bnode) => bnode.to_string().into(),
        }
    }

    fn finish(&mut self, output: &mut Vec<JsonEvent<'static>>) {
        if self.current_predicate.is_some() {
            output.push(JsonEvent::EndArray)
        }
        if self.current_subject.is_some() {
            output.push(JsonEvent::EndObject)
        }
        if self
            .current_graph_name
            .as_ref()
            .is_some_and(|g| !g.is_default_graph())
        {
            output.push(JsonEvent::EndArray);
            output.push(JsonEvent::EndObject)
        }
        output.push(JsonEvent::EndArray);
        if self.base_iri.is_some() || !self.prefixes.is_empty() {
            output.push(JsonEvent::EndObject);
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
