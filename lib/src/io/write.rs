//! Utilities to write RDF graphs and datasets

use crate::io::{DatasetFormat, GraphFormat};
use crate::model::*;
use rio_api::formatter::{QuadsFormatter, TriplesFormatter};
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
    pub fn triple_writer<W: Write>(&self, writer: W) -> Result<TripleWriter<W>, io::Error> {
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
    pub fn write<'a>(&mut self, triple: impl Into<TripleRef<'a>>) -> Result<(), io::Error> {
        let triple = triple.into();
        match &mut self.formatter {
            TripleWriterKind::NTriples(formatter) => formatter.format(&triple.into())?,
            TripleWriterKind::Turtle(formatter) => formatter.format(&triple.into())?,
            TripleWriterKind::RdfXml(formatter) => formatter.format(&triple.into())?,
        }
        Ok(())
    }

    /// Writes the last bytes of the file
    pub fn finish(self) -> Result<(), io::Error> {
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
    pub fn quad_writer<W: Write>(&self, writer: W) -> Result<QuadWriter<W>, io::Error> {
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
    pub fn write<'a>(&mut self, quad: impl Into<QuadRef<'a>>) -> Result<(), io::Error> {
        let quad = quad.into();
        match &mut self.formatter {
            QuadWriterKind::NQuads(formatter) => formatter.format(&quad.into())?,
            QuadWriterKind::TriG(formatter) => formatter.format(&quad.into())?,
        }
        Ok(())
    }

    /// Writes the last bytes of the file
    pub fn finish(self) -> Result<(), io::Error> {
        match self.formatter {
            QuadWriterKind::NQuads(formatter) => formatter.finish(),
            QuadWriterKind::TriG(formatter) => formatter.finish()?,
        };
        Ok(())
    }
}
