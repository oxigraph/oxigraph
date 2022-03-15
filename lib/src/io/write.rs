//! Utilities to write RDF graphs and datasets.

use crate::io::{DatasetFormat, GraphFormat};
use crate::model::*;
use rio_api::formatter::TriplesFormatter;
use rio_api::model as rio;
use rio_xml::RdfXmlFormatter;
use std::io::{self, Write};

/// A serializer for RDF graph serialization formats.
///
/// It currently supports the following formats:
/// * [N-Triples](https://www.w3.org/TR/n-triples/) ([`GraphFormat::NTriples`](super::GraphFormat::NTriples))
/// * [Turtle](https://www.w3.org/TR/turtle/) ([`GraphFormat::Turtle`](super::GraphFormat::Turtle))
/// * [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/) ([`GraphFormat::RdfXml`](super::GraphFormat::RdfXml))
///
/// ```
/// use oxigraph::io::{GraphFormat, GraphSerializer};
/// use oxigraph::model::*;
///
/// let mut buffer = Vec::new();
/// let mut writer = GraphSerializer::from_format(GraphFormat::NTriples).triple_writer(&mut buffer)?;
/// writer.write(&Triple {
///    subject: NamedNode::new("http://example.com/s")?.into(),
///    predicate: NamedNode::new("http://example.com/p")?,
///    object: NamedNode::new("http://example.com/o")?.into()
/// })?;
/// writer.finish()?;
///
///assert_eq!(buffer.as_slice(), "<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n".as_bytes());
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct GraphSerializer {
    format: GraphFormat,
}

impl GraphSerializer {
    /// Builds a serializer for the given format
    #[inline]
    pub fn from_format(format: GraphFormat) -> Self {
        Self { format }
    }

    /// Returns a [`TripleWriter`] allowing writing triples into the given [`Write`](std::io::Write) implementation
    pub fn triple_writer<W: Write>(&self, writer: W) -> io::Result<TripleWriter<W>> {
        Ok(TripleWriter {
            formatter: match self.format {
                GraphFormat::NTriples | GraphFormat::Turtle => TripleWriterKind::NTriples(writer),
                GraphFormat::RdfXml => TripleWriterKind::RdfXml(RdfXmlFormatter::new(writer)?),
            },
        })
    }
}

/// Allows writing triples.
/// Could be built using a [`GraphSerializer`].
///
/// Warning: Do not forget to run the [`finish`](TripleWriter::finish()) method to properly write the last bytes of the file.
///
/// ```
/// use oxigraph::io::{GraphFormat, GraphSerializer};
/// use oxigraph::model::*;
///
/// let mut buffer = Vec::new();
/// let mut writer = GraphSerializer::from_format(GraphFormat::NTriples).triple_writer(&mut buffer)?;
/// writer.write(&Triple {
///    subject: NamedNode::new("http://example.com/s")?.into(),
///    predicate: NamedNode::new("http://example.com/p")?,
///    object: NamedNode::new("http://example.com/o")?.into()
/// })?;
/// writer.finish()?;
///
///assert_eq!(buffer.as_slice(), "<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n".as_bytes());
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct TripleWriter<W: Write> {
    formatter: TripleWriterKind<W>,
}

enum TripleWriterKind<W: Write> {
    NTriples(W),
    RdfXml(RdfXmlFormatter<W>),
}

impl<W: Write> TripleWriter<W> {
    /// Writes a triple
    pub fn write<'a>(&mut self, triple: impl Into<TripleRef<'a>>) -> io::Result<()> {
        let triple = triple.into();
        match &mut self.formatter {
            TripleWriterKind::NTriples(writer) => {
                writeln!(writer, "{} .", triple)?;
            }
            TripleWriterKind::RdfXml(formatter) => formatter.format(&rio::Triple {
                subject: match triple.subject {
                    SubjectRef::NamedNode(node) => rio::NamedNode { iri: node.as_str() }.into(),
                    SubjectRef::BlankNode(node) => rio::BlankNode { id: node.as_str() }.into(),
                    SubjectRef::Triple(_) => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "RDF/XML does not support RDF-star yet",
                        ))
                    }
                },
                predicate: rio::NamedNode {
                    iri: triple.predicate.as_str(),
                },
                object: match triple.object {
                    TermRef::NamedNode(node) => rio::NamedNode { iri: node.as_str() }.into(),
                    TermRef::BlankNode(node) => rio::BlankNode { id: node.as_str() }.into(),
                    TermRef::Literal(literal) => if literal.is_plain() {
                        if let Some(language) = literal.language() {
                            rio::Literal::LanguageTaggedString {
                                value: literal.value(),
                                language,
                            }
                        } else {
                            rio::Literal::Simple {
                                value: literal.value(),
                            }
                        }
                    } else {
                        rio::Literal::Typed {
                            value: literal.value(),
                            datatype: rio::NamedNode {
                                iri: literal.datatype().as_str(),
                            },
                        }
                    }
                    .into(),
                    TermRef::Triple(_) => {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidInput,
                            "RDF/XML does not support RDF-star yet",
                        ))
                    }
                },
            })?,
        }
        Ok(())
    }

    /// Writes the last bytes of the file
    pub fn finish(self) -> io::Result<()> {
        match self.formatter {
            TripleWriterKind::NTriples(mut writer) => writer.flush(),
            TripleWriterKind::RdfXml(formatter) => formatter.finish()?.flush(), //TODO: remove flush when the next version of Rio is going to be released
        }
    }
}

/// A serializer for RDF graph serialization formats.
///
/// It currently supports the following formats:
/// * [N-Quads](https://www.w3.org/TR/n-quads/) ([`DatasetFormat::NQuads`](super::DatasetFormat::NQuads))
/// * [TriG](https://www.w3.org/TR/trig/) ([`DatasetFormat::TriG`](super::DatasetFormat::TriG))
///
/// ```
/// use oxigraph::io::{DatasetFormat, DatasetSerializer};
/// use oxigraph::model::*;
///
/// let mut buffer = Vec::new();
/// let mut writer = DatasetSerializer::from_format(DatasetFormat::NQuads).quad_writer(&mut buffer)?;
/// writer.write(&Quad {
///    subject: NamedNode::new("http://example.com/s")?.into(),
///    predicate: NamedNode::new("http://example.com/p")?,
///    object: NamedNode::new("http://example.com/o")?.into(),
///    graph_name: NamedNode::new("http://example.com/g")?.into(),
/// })?;
/// writer.finish()?;
///
///assert_eq!(buffer.as_slice(), "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n".as_bytes());
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct DatasetSerializer {
    format: DatasetFormat,
}

impl DatasetSerializer {
    /// Builds a serializer for the given format
    #[inline]
    pub fn from_format(format: DatasetFormat) -> Self {
        Self { format }
    }

    /// Returns a [`QuadWriter`] allowing writing triples into the given [`Write`](std::io::Write) implementation
    #[allow(clippy::unnecessary_wraps)]
    pub fn quad_writer<W: Write>(&self, writer: W) -> io::Result<QuadWriter<W>> {
        Ok(QuadWriter {
            formatter: match self.format {
                DatasetFormat::NQuads => QuadWriterKind::NQuads(writer),
                DatasetFormat::TriG => QuadWriterKind::TriG(writer),
            },
        })
    }
}

/// Allows writing triples.
/// Could be built using a [`DatasetSerializer`].
///
/// Warning: Do not forget to run the [`finish`](QuadWriter::finish()) method to properly write the last bytes of the file.
///
/// ```
/// use oxigraph::io::{DatasetFormat, DatasetSerializer};
/// use oxigraph::model::*;
///
/// let mut buffer = Vec::new();
/// let mut writer = DatasetSerializer::from_format(DatasetFormat::NQuads).quad_writer(&mut buffer)?;
/// writer.write(&Quad {
///    subject: NamedNode::new("http://example.com/s")?.into(),
///    predicate: NamedNode::new("http://example.com/p")?,
///    object: NamedNode::new("http://example.com/o")?.into(),
///    graph_name: NamedNode::new("http://example.com/g")?.into(),
/// })?;
/// writer.finish()?;
///
///assert_eq!(buffer.as_slice(), "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n".as_bytes());
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct QuadWriter<W: Write> {
    formatter: QuadWriterKind<W>,
}

enum QuadWriterKind<W: Write> {
    NQuads(W),
    TriG(W),
}

impl<W: Write> QuadWriter<W> {
    /// Writes a quad
    pub fn write<'a>(&mut self, quad: impl Into<QuadRef<'a>>) -> io::Result<()> {
        let quad = quad.into();
        match &mut self.formatter {
            QuadWriterKind::NQuads(writer) => {
                writeln!(writer, "{} .", quad)?;
            }
            QuadWriterKind::TriG(writer) => {
                if quad.graph_name == GraphNameRef::DefaultGraph {
                    writeln!(
                        writer,
                        "GRAPH {} {{ {} }}",
                        quad.graph_name,
                        TripleRef::from(quad)
                    )?;
                } else {
                    writeln!(writer, "{} .", quad)?;
                }
            }
        }
        Ok(())
    }

    /// Writes the last bytes of the file
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    pub fn finish(self) -> io::Result<()> {
        match self.formatter {
            QuadWriterKind::NQuads(mut writer) | QuadWriterKind::TriG(mut writer) => writer.flush(),
        }
    }
}
