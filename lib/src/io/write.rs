//! Utilities to write RDF graphs and datasets.

use crate::io::{DatasetFormat, GraphFormat};
use crate::model::*;
use oxrdfxml::{RdfXmlSerializer, ToWriteRdfXmlWriter};
use oxttl::nquads::{NQuadsSerializer, ToWriteNQuadsWriter};
use oxttl::ntriples::{NTriplesSerializer, ToWriteNTriplesWriter};
use oxttl::trig::{ToWriteTriGWriter, TriGSerializer};
use oxttl::turtle::{ToWriteTurtleWriter, TurtleSerializer};
use std::io::{self, Write};

/// A serializer for RDF graph serialization formats.
///
/// It currently supports the following formats:
/// * [N-Triples](https://www.w3.org/TR/n-triples/) ([`GraphFormat::NTriples`])
/// * [Turtle](https://www.w3.org/TR/turtle/) ([`GraphFormat::Turtle`])
/// * [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/) ([`GraphFormat::RdfXml`])
///
/// ```
/// use oxigraph::io::{GraphFormat, GraphSerializer};
/// use oxigraph::model::*;
///
/// let mut buffer = Vec::new();
/// let mut writer = GraphSerializer::from_format(GraphFormat::NTriples).triple_writer(&mut buffer);
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

    /// Returns a [`TripleWriter`] allowing writing triples into the given [`Write`] implementation
    pub fn triple_writer<W: Write>(&self, writer: W) -> TripleWriter<W> {
        TripleWriter {
            formatter: match self.format {
                GraphFormat::NTriples => {
                    TripleWriterKind::NTriples(NTriplesSerializer::new().serialize_to_write(writer))
                }
                GraphFormat::Turtle => {
                    TripleWriterKind::Turtle(TurtleSerializer::new().serialize_to_write(writer))
                }
                GraphFormat::RdfXml => {
                    TripleWriterKind::RdfXml(RdfXmlSerializer::new().serialize_to_write(writer))
                }
            },
        }
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
/// let mut writer = GraphSerializer::from_format(GraphFormat::NTriples).triple_writer(&mut buffer);
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
    NTriples(ToWriteNTriplesWriter<W>),
    Turtle(ToWriteTurtleWriter<W>),
    RdfXml(ToWriteRdfXmlWriter<W>),
}

impl<W: Write> TripleWriter<W> {
    /// Writes a triple
    pub fn write<'a>(&mut self, triple: impl Into<TripleRef<'a>>) -> io::Result<()> {
        match &mut self.formatter {
            TripleWriterKind::NTriples(writer) => writer.write_triple(triple),
            TripleWriterKind::Turtle(writer) => writer.write_triple(triple),
            TripleWriterKind::RdfXml(writer) => writer.write_triple(triple),
        }
    }

    /// Writes the last bytes of the file
    pub fn finish(self) -> io::Result<()> {
        match self.formatter {
            TripleWriterKind::NTriples(writer) => writer.finish().flush(),
            TripleWriterKind::Turtle(writer) => writer.finish()?.flush(),
            TripleWriterKind::RdfXml(formatter) => formatter.finish()?.flush(),
        }
    }
}

/// A serializer for RDF graph serialization formats.
///
/// It currently supports the following formats:
/// * [N-Quads](https://www.w3.org/TR/n-quads/) ([`DatasetFormat::NQuads`])
/// * [TriG](https://www.w3.org/TR/trig/) ([`DatasetFormat::TriG`])
///
/// ```
/// use oxigraph::io::{DatasetFormat, DatasetSerializer};
/// use oxigraph::model::*;
///
/// let mut buffer = Vec::new();
/// let mut writer = DatasetSerializer::from_format(DatasetFormat::NQuads).quad_writer(&mut buffer);
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

    /// Returns a [`QuadWriter`] allowing writing triples into the given [`Write`] implementation
    pub fn quad_writer<W: Write>(&self, writer: W) -> QuadWriter<W> {
        QuadWriter {
            formatter: match self.format {
                DatasetFormat::NQuads => {
                    QuadWriterKind::NQuads(NQuadsSerializer::new().serialize_to_write(writer))
                }
                DatasetFormat::TriG => {
                    QuadWriterKind::TriG(TriGSerializer::new().serialize_to_write(writer))
                }
            },
        }
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
/// let mut writer = DatasetSerializer::from_format(DatasetFormat::NQuads).quad_writer(&mut buffer);
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
    NQuads(ToWriteNQuadsWriter<W>),
    TriG(ToWriteTriGWriter<W>),
}

impl<W: Write> QuadWriter<W> {
    /// Writes a quad
    pub fn write<'a>(&mut self, quad: impl Into<QuadRef<'a>>) -> io::Result<()> {
        match &mut self.formatter {
            QuadWriterKind::NQuads(writer) => writer.write_quad(quad),
            QuadWriterKind::TriG(writer) => writer.write_quad(quad),
        }
    }

    /// Writes the last bytes of the file
    pub fn finish(self) -> io::Result<()> {
        match self.formatter {
            QuadWriterKind::NQuads(writer) => writer.finish().flush(),
            QuadWriterKind::TriG(writer) => writer.finish()?.flush(),
        }
    }
}
