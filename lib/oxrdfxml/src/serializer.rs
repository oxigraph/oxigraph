use crate::utils::*;
use oxrdf::{Subject, SubjectRef, TermRef, TripleRef};
use quick_xml::events::*;
use quick_xml::Writer;
use std::borrow::Cow;
use std::io;
use std::io::Write;
use std::sync::Arc;
#[cfg(feature = "async-tokio")]
use tokio::io::AsyncWrite;

/// A [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/) serializer.
///
/// ```
/// use oxrdf::{NamedNodeRef, TripleRef};
/// use oxrdfxml::RdfXmlSerializer;
///
/// let mut writer = RdfXmlSerializer::new().serialize_to_write(Vec::new());
/// writer.write_triple(TripleRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
/// ))?;
/// assert_eq!(
///     b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\t<rdf:Description rdf:about=\"http://example.com#me\">\n\t\t<rdf:type rdf:resource=\"http://schema.org/Person\"/>\n\t</rdf:Description>\n</rdf:RDF>",
///     writer.finish()?.as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default)]
#[must_use]
pub struct RdfXmlSerializer;

impl RdfXmlSerializer {
    /// Builds a new [`RdfXmlSerializer`].
    #[inline]
    pub fn new() -> Self {
        Self
    }

    /// Writes a RDF/XML file to a [`Write`] implementation.
    ///
    /// This writer does unbuffered writes.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, TripleRef};
    /// use oxrdfxml::RdfXmlSerializer;
    ///
    /// let mut writer = RdfXmlSerializer::new().serialize_to_write(Vec::new());
    /// writer.write_triple(TripleRef::new(
    ///     NamedNodeRef::new("http://example.com#me")?,
    ///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
    ///     NamedNodeRef::new("http://schema.org/Person")?,
    /// ))?;
    /// assert_eq!(
    ///     b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\t<rdf:Description rdf:about=\"http://example.com#me\">\n\t\t<rdf:type rdf:resource=\"http://schema.org/Person\"/>\n\t</rdf:Description>\n</rdf:RDF>",
    ///     writer.finish()?.as_slice()
    /// );
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[allow(clippy::unused_self)]
    pub fn serialize_to_write<W: Write>(self, write: W) -> ToWriteRdfXmlWriter<W> {
        ToWriteRdfXmlWriter {
            writer: Writer::new_with_indent(write, b'\t', 1),
            inner: InnerRdfXmlWriter {
                current_subject: None,
            },
        }
    }

    /// Writes a RDF/XML file to a [`AsyncWrite`] implementation.
    ///
    /// This writer does unbuffered writes.
    ///
    /// ```
    /// use oxrdf::{NamedNodeRef, TripleRef};
    /// use oxrdfxml::RdfXmlSerializer;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> std::io::Result<()> {
    /// let mut writer = RdfXmlSerializer::new().serialize_to_tokio_async_write(Vec::new());
    /// writer.write_triple(TripleRef::new(
    ///     NamedNodeRef::new_unchecked("http://example.com#me"),
    ///     NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
    ///     NamedNodeRef::new_unchecked("http://schema.org/Person"),
    /// )).await?;
    /// assert_eq!(
    ///     b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\t<rdf:Description rdf:about=\"http://example.com#me\">\n\t\t<rdf:type rdf:resource=\"http://schema.org/Person\"/>\n\t</rdf:Description>\n</rdf:RDF>",
    ///     writer.finish().await?.as_slice()
    /// );
    /// # Ok(())
    /// # }
    /// ```
    #[allow(clippy::unused_self)]
    #[cfg(feature = "async-tokio")]
    pub fn serialize_to_tokio_async_write<W: AsyncWrite + Unpin>(
        self,
        write: W,
    ) -> ToTokioAsyncWriteRdfXmlWriter<W> {
        ToTokioAsyncWriteRdfXmlWriter {
            writer: Writer::new_with_indent(write, b'\t', 1),
            inner: InnerRdfXmlWriter {
                current_subject: None,
            },
        }
    }
}

/// Writes a RDF/XML file to a [`Write`] implementation. Can be built using [`RdfXmlSerializer::serialize_to_write`].
///
/// ```
/// use oxrdf::{NamedNodeRef, TripleRef};
/// use oxrdfxml::RdfXmlSerializer;
///
/// let mut writer = RdfXmlSerializer::new().serialize_to_write(Vec::new());
/// writer.write_triple(TripleRef::new(
///     NamedNodeRef::new("http://example.com#me")?,
///     NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?,
///     NamedNodeRef::new("http://schema.org/Person")?,
/// ))?;
/// assert_eq!(
///     b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\t<rdf:Description rdf:about=\"http://example.com#me\">\n\t\t<rdf:type rdf:resource=\"http://schema.org/Person\"/>\n\t</rdf:Description>\n</rdf:RDF>",
///     writer.finish()?.as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct ToWriteRdfXmlWriter<W: Write> {
    writer: Writer<W>,
    inner: InnerRdfXmlWriter,
}

impl<W: Write> ToWriteRdfXmlWriter<W> {
    /// Writes an extra triple.
    #[allow(clippy::match_wildcard_for_single_variants, unreachable_patterns)]
    pub fn write_triple<'a>(&mut self, t: impl Into<TripleRef<'a>>) -> io::Result<()> {
        let mut buffer = Vec::new();
        self.inner.write_triple(t, &mut buffer)?;
        self.flush_buffer(&mut buffer)
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub fn finish(mut self) -> io::Result<W> {
        let mut buffer = Vec::new();
        self.inner.finish(&mut buffer);
        self.flush_buffer(&mut buffer)?;
        Ok(self.writer.into_inner())
    }

    fn flush_buffer(&mut self, buffer: &mut Vec<Event<'_>>) -> io::Result<()> {
        for event in buffer.drain(0..) {
            self.writer.write_event(event).map_err(map_err)?;
        }
        Ok(())
    }
}

/// Writes a RDF/XML file to a [`AsyncWrite`] implementation. Can be built using [`RdfXmlSerializer::serialize_to_tokio_async_write`].
///
/// ```
/// use oxrdf::{NamedNodeRef, TripleRef};
/// use oxrdfxml::RdfXmlSerializer;
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> std::io::Result<()> {
/// let mut writer = RdfXmlSerializer::new().serialize_to_tokio_async_write(Vec::new());
/// writer.write_triple(TripleRef::new(
///     NamedNodeRef::new_unchecked("http://example.com#me"),
///     NamedNodeRef::new_unchecked("http://www.w3.org/1999/02/22-rdf-syntax-ns#type"),
///     NamedNodeRef::new_unchecked("http://schema.org/Person"),
/// )).await?;
/// assert_eq!(
///     b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\t<rdf:Description rdf:about=\"http://example.com#me\">\n\t\t<type xmlns=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\" rdf:resource=\"http://schema.org/Person\"/>\n\t</rdf:Description>\n</rdf:RDF>",
///     writer.finish().await?.as_slice()
/// );
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async-tokio")]
#[must_use]
pub struct ToTokioAsyncWriteRdfXmlWriter<W: AsyncWrite + Unpin> {
    writer: Writer<W>,
    inner: InnerRdfXmlWriter,
}

#[cfg(feature = "async-tokio")]
impl<W: AsyncWrite + Unpin> ToTokioAsyncWriteRdfXmlWriter<W> {
    /// Writes an extra triple.
    #[allow(clippy::match_wildcard_for_single_variants, unreachable_patterns)]
    pub async fn write_triple<'a>(&mut self, t: impl Into<TripleRef<'a>>) -> io::Result<()> {
        let mut buffer = Vec::new();
        self.inner.write_triple(t, &mut buffer)?;
        self.flush_buffer(&mut buffer).await
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub async fn finish(mut self) -> io::Result<W> {
        let mut buffer = Vec::new();
        self.inner.finish(&mut buffer);
        self.flush_buffer(&mut buffer).await?;
        Ok(self.writer.into_inner())
    }

    async fn flush_buffer(&mut self, buffer: &mut Vec<Event<'_>>) -> io::Result<()> {
        for event in buffer.drain(0..) {
            self.writer
                .write_event_async(event)
                .await
                .map_err(map_err)?;
        }
        Ok(())
    }
}

pub struct InnerRdfXmlWriter {
    current_subject: Option<Subject>,
}

impl InnerRdfXmlWriter {
    #[allow(clippy::match_wildcard_for_single_variants, unreachable_patterns)]
    fn write_triple<'a>(
        &mut self,
        t: impl Into<TripleRef<'a>>,
        output: &mut Vec<Event<'a>>,
    ) -> io::Result<()> {
        if self.current_subject.is_none() {
            Self::write_start(output);
        }

        let triple = t.into();
        // We open a new rdf:Description if useful
        if self.current_subject.as_ref().map(Subject::as_ref) != Some(triple.subject) {
            if self.current_subject.is_some() {
                output.push(Event::End(BytesEnd::new("rdf:Description")));
            }

            let mut description_open = BytesStart::new("rdf:Description");
            match triple.subject {
                SubjectRef::NamedNode(node) => {
                    description_open.push_attribute(("rdf:about", node.as_str()))
                }
                SubjectRef::BlankNode(node) => {
                    description_open.push_attribute(("rdf:nodeID", node.as_str()))
                }
                _ => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "RDF/XML only supports named or blank subject",
                    ))
                }
            }
            output.push(Event::Start(description_open));
        }
        self.current_subject = Some(triple.subject.into_owned());

        let (prop_prefix, prop_value) = split_iri(triple.predicate.as_str());
        let (prop_qname, prop_xmlns) =
            if prop_prefix == "http://www.w3.org/1999/02/22-rdf-syntax-ns#" {
                (Cow::Owned(format!("rdf:{prop_value}")), None)
            } else if prop_prefix == "http://www.w3.org/2000/xmlns/" {
                (Cow::Owned(format!("xmlns:{prop_value}")), None)
            } else if prop_value.is_empty() {
                (Cow::Borrowed("p:"), Some(("xmlns:p", prop_prefix)))
            } else {
                (Cow::Borrowed(prop_value), Some(("xmlns", prop_prefix)))
            };
        let mut property_open = BytesStart::new(prop_qname.clone());
        if let Some(prop_xmlns) = prop_xmlns {
            property_open.push_attribute(prop_xmlns);
        }
        let content = match triple.object {
            TermRef::NamedNode(node) => {
                property_open.push_attribute(("rdf:resource", node.as_str()));
                None
            }
            TermRef::BlankNode(node) => {
                property_open.push_attribute(("rdf:nodeID", node.as_str()));
                None
            }
            TermRef::Literal(literal) => {
                if let Some(language) = literal.language() {
                    property_open.push_attribute(("xml:lang", language));
                } else if !literal.is_plain() {
                    property_open.push_attribute(("rdf:datatype", literal.datatype().as_str()));
                }
                Some(literal.value())
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "RDF/XML only supports named, blank or literal object",
                ))
            }
        };
        if let Some(content) = content {
            output.push(Event::Start(property_open));
            output.push(Event::Text(BytesText::new(content)));
            output.push(Event::End(BytesEnd::new(prop_qname)));
        } else {
            output.push(Event::Empty(property_open));
        }
        Ok(())
    }

    fn write_start(output: &mut Vec<Event<'_>>) {
        output.push(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)));
        let mut rdf_open = BytesStart::new("rdf:RDF");
        rdf_open.push_attribute(("xmlns:rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"));
        output.push(Event::Start(rdf_open))
    }

    fn finish(&self, output: &mut Vec<Event<'static>>) {
        if self.current_subject.is_some() {
            output.push(Event::End(BytesEnd::new("rdf:Description")));
        } else {
            Self::write_start(output);
        }
        output.push(Event::End(BytesEnd::new("rdf:RDF")));
    }
}

fn map_err(error: quick_xml::Error) -> io::Error {
    if let quick_xml::Error::Io(error) = error {
        match Arc::try_unwrap(error) {
            Ok(error) => error,
            Err(error) => io::Error::new(error.kind(), error),
        }
    } else {
        io::Error::new(io::ErrorKind::Other, error)
    }
}

fn split_iri(iri: &str) -> (&str, &str) {
    if let Some(position_base) = iri.rfind(|c| !is_name_char(c) || c == ':') {
        if let Some(position_add) = iri[position_base..].find(|c| is_name_start_char(c) && c != ':')
        {
            (
                &iri[..position_base + position_add],
                &iri[position_base + position_add..],
            )
        } else {
            (iri, "")
        }
    } else {
        (iri, "")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_iri() {
        assert_eq!(
            split_iri("http://schema.org/Person"),
            ("http://schema.org/", "Person")
        );
        assert_eq!(split_iri("http://schema.org/"), ("http://schema.org/", ""));
        assert_eq!(
            split_iri("http://schema.org#foo"),
            ("http://schema.org#", "foo")
        );
        assert_eq!(split_iri("urn:isbn:foo"), ("urn:isbn:", "foo"));
    }
}
