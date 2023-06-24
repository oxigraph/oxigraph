use crate::utils::*;
use oxrdf::{Subject, SubjectRef, TermRef, TripleRef};
use quick_xml::events::*;
use quick_xml::Writer;
use std::io;
use std::io::Write;
use std::sync::Arc;

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
///     b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\t<rdf:Description rdf:about=\"http://example.com#me\">\n\t\t<type xmlns=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\" rdf:resource=\"http://schema.org/Person\"/>\n\t</rdf:Description>\n</rdf:RDF>",
///     writer.finish()?.as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default)]
pub struct RdfXmlSerializer;

impl RdfXmlSerializer {
    /// Builds a new [`RdfXmlSerializer`].
    #[inline]
    pub fn new() -> Self {
        Self
    }

    /// Writes a RdfXml file to a [`Write`] implementation.
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
    ///     b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\t<rdf:Description rdf:about=\"http://example.com#me\">\n\t\t<type xmlns=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\" rdf:resource=\"http://schema.org/Person\"/>\n\t</rdf:Description>\n</rdf:RDF>",
    ///     writer.finish()?.as_slice()
    /// );
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[allow(clippy::unused_self)]
    pub fn serialize_to_write<W: Write>(&self, write: W) -> ToWriteRdfXmlWriter<W> {
        ToWriteRdfXmlWriter {
            writer: Writer::new_with_indent(write, b'\t', 1),
            current_subject: None,
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
///     b"<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rdf:RDF xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\t<rdf:Description rdf:about=\"http://example.com#me\">\n\t\t<type xmlns=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\" rdf:resource=\"http://schema.org/Person\"/>\n\t</rdf:Description>\n</rdf:RDF>",
///     writer.finish()?.as_slice()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct ToWriteRdfXmlWriter<W: Write> {
    writer: Writer<W>,
    current_subject: Option<Subject>,
}

impl<W: Write> ToWriteRdfXmlWriter<W> {
    /// Writes an extra triple.
    #[allow(clippy::match_wildcard_for_single_variants, unreachable_patterns)]
    pub fn write_triple<'a>(&mut self, t: impl Into<TripleRef<'a>>) -> io::Result<()> {
        if self.current_subject.is_none() {
            self.write_start()?;
        }

        let triple = t.into();
        // We open a new rdf:Description if useful
        if self.current_subject.as_ref().map(Subject::as_ref) != Some(triple.subject) {
            if self.current_subject.is_some() {
                self.writer
                    .write_event(Event::End(BytesEnd::new("rdf:Description")))
                    .map_err(map_err)?;
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
            self.writer
                .write_event(Event::Start(description_open))
                .map_err(map_err)?;
        }

        let (prop_prefix, prop_value) = split_iri(triple.predicate.as_str());
        let (prop_qname, prop_xmlns) = if prop_value.is_empty() {
            ("prop:", ("xmlns:prop", prop_prefix))
        } else {
            (prop_value, ("xmlns", prop_prefix))
        };
        let property_element = self.writer.create_element(prop_qname);
        let property_element = property_element.with_attribute(prop_xmlns);

        match triple.object {
            TermRef::NamedNode(node) => property_element
                .with_attribute(("rdf:resource", node.as_str()))
                .write_empty(),
            TermRef::BlankNode(node) => property_element
                .with_attribute(("rdf:nodeID", node.as_str()))
                .write_empty(),
            TermRef::Literal(literal) => {
                let property_element = if let Some(language) = literal.language() {
                    property_element.with_attribute(("xml:lang", language))
                } else if !literal.is_plain() {
                    property_element.with_attribute(("rdf:datatype", literal.datatype().as_str()))
                } else {
                    property_element
                };
                property_element.write_text_content(BytesText::new(literal.value()))
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "RDF/XML only supports named, blank or literal object",
                ))
            }
        }
        .map_err(map_err)?;
        self.current_subject = Some(triple.subject.into_owned());
        Ok(())
    }

    pub fn write_start(&mut self) -> io::Result<()> {
        // We open the file
        self.writer
            .write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))
            .map_err(map_err)?;
        let mut rdf_open = BytesStart::new("rdf:RDF");
        rdf_open.push_attribute(("xmlns:rdf", "http://www.w3.org/1999/02/22-rdf-syntax-ns#"));
        self.writer
            .write_event(Event::Start(rdf_open))
            .map_err(map_err)
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub fn finish(mut self) -> io::Result<W> {
        if self.current_subject.is_some() {
            self.writer
                .write_event(Event::End(BytesEnd::new("rdf:Description")))
                .map_err(map_err)?;
        } else {
            self.write_start()?;
        }
        self.writer
            .write_event(Event::End(BytesEnd::new("rdf:RDF")))
            .map_err(map_err)?;
        Ok(self.writer.into_inner())
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
