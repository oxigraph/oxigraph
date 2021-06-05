//! Utilities to write RDF graphs and datasets

use crate::error::invalid_input_error;
use crate::io::{DatasetFormat, GraphFormat};
use crate::model::*;
use rio_api::formatter::{QuadsFormatter, TriplesFormatter};
use rio_api::model as rio;
use rio_turtle::{NQuadsFormatter, NTriplesFormatter, TriGFormatter, TurtleFormatter};
use rio_xml::RdfXmlFormatter;
use std::io;
use std::io::Write;

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
#[allow(missing_copy_implementations)]
pub struct GraphSerializer {
    format: GraphFormat,
}

impl GraphSerializer {
    /// Builds a serializer for the given format
    pub fn from_format(format: GraphFormat) -> Self {
        Self { format }
    }

    /// Returns a `TripleWriter` allowing writing triples into the given [`Write`](std::io::Write) implementation
    pub fn triple_writer<W: Write>(&self, writer: W) -> io::Result<TripleWriter<W>> {
        Ok(TripleWriter {
            formatter: match self.format {
                GraphFormat::NTriples => TripleWriterKind::NTriples(NTriplesFormatter::new(writer)),
                GraphFormat::Turtle => TripleWriterKind::Turtle(TurtleFormatter::new(writer)),
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
    NTriples(NTriplesFormatter<W>),
    Turtle(TurtleFormatter<W>),
    RdfXml(RdfXmlFormatter<W>),
}

impl<W: Write> TripleWriter<W> {
    /// Writes a triple
    pub fn write<'a>(&mut self, triple: impl Into<TripleRef<'a>>) -> io::Result<()> {
        let triple = triple.into();
        let triple = rio::Triple {
            subject: match triple.subject {
                SubjectRef::NamedNode(node) => rio::NamedNode { iri: node.as_str() }.into(),
                SubjectRef::BlankNode(node) => rio::BlankNode { id: node.as_str() }.into(),
                SubjectRef::Triple(_) => {
                    return Err(invalid_input_error(
                        "Rio library does not support RDF-star yet",
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
                    return Err(invalid_input_error(
                        "Rio library does not support RDF-star yet",
                    ))
                }
            },
        };
        match &mut self.formatter {
            TripleWriterKind::NTriples(formatter) => formatter.format(&triple)?,
            TripleWriterKind::Turtle(formatter) => formatter.format(&triple)?,
            TripleWriterKind::RdfXml(formatter) => formatter.format(&triple)?,
        }
        Ok(())
    }

    /// Writes the last bytes of the file
    pub fn finish(self) -> io::Result<()> {
        match self.formatter {
            TripleWriterKind::NTriples(formatter) => formatter.finish(),
            TripleWriterKind::Turtle(formatter) => formatter.finish()?,
            TripleWriterKind::RdfXml(formatter) => formatter.finish()?,
        };
        Ok(())
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
#[allow(missing_copy_implementations)]
pub struct DatasetSerializer {
    format: DatasetFormat,
}

impl DatasetSerializer {
    /// Builds a serializer for the given format
    pub fn from_format(format: DatasetFormat) -> Self {
        Self { format }
    }

    /// Returns a `QuadWriter` allowing writing triples into the given [`Write`](std::io::Write) implementation
    pub fn quad_writer<W: Write>(&self, writer: W) -> io::Result<QuadWriter<W>> {
        Ok(QuadWriter {
            formatter: match self.format {
                DatasetFormat::NQuads => QuadWriterKind::NQuads(NQuadsFormatter::new(writer)),
                DatasetFormat::TriG => QuadWriterKind::TriG(TriGFormatter::new(writer)),
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
    NQuads(NQuadsFormatter<W>),
    TriG(TriGFormatter<W>),
}

impl<W: Write> QuadWriter<W> {
    /// Writes a quad
    pub fn write<'a>(&mut self, quad: impl Into<QuadRef<'a>>) -> io::Result<()> {
        let quad = quad.into();
        let quad = rio::Quad {
            subject: match quad.subject {
                SubjectRef::NamedNode(node) => rio::NamedNode { iri: node.as_str() }.into(),
                SubjectRef::BlankNode(node) => rio::BlankNode { id: node.as_str() }.into(),
                SubjectRef::Triple(_) => {
                    return Err(invalid_input_error(
                        "Rio library does not support RDF-star yet",
                    ))
                }
            },
            predicate: rio::NamedNode {
                iri: quad.predicate.as_str(),
            },
            object: match quad.object {
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
                    return Err(invalid_input_error(
                        "Rio library does not support RDF-star yet",
                    ))
                }
            },
            graph_name: match quad.graph_name {
                GraphNameRef::NamedNode(node) => Some(rio::NamedNode { iri: node.as_str() }.into()),
                GraphNameRef::BlankNode(node) => Some(rio::BlankNode { id: node.as_str() }.into()),
                GraphNameRef::DefaultGraph => None,
            },
        };
        match &mut self.formatter {
            QuadWriterKind::NQuads(formatter) => formatter.format(&quad)?,
            QuadWriterKind::TriG(formatter) => formatter.format(&quad)?,
        }
        Ok(())
    }

    /// Writes the last bytes of the file
    pub fn finish(self) -> io::Result<()> {
        match self.formatter {
            QuadWriterKind::NQuads(formatter) => formatter.finish(),
            QuadWriterKind::TriG(formatter) => formatter.finish()?,
        };
        Ok(())
    }
}
