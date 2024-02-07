#![allow(deprecated)]

//! Utilities to write RDF graphs and datasets.

use crate::io::{DatasetFormat, GraphFormat};
use crate::model::*;
use oxrdfio::{RdfSerializer, ToWriteQuadWriter};
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
///     subject: NamedNode::new("http://example.com/s")?.into(),
///     predicate: NamedNode::new("http://example.com/p")?,
///     object: NamedNode::new("http://example.com/o")?.into(),
/// })?;
/// writer.finish()?;
///
/// assert_eq!(
///     buffer.as_slice(),
///     "<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n".as_bytes()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[deprecated(note = "use RdfSerializer instead", since = "0.4.0")]
pub struct GraphSerializer {
    inner: RdfSerializer,
}

impl GraphSerializer {
    /// Builds a serializer for the given format
    #[inline]
    pub fn from_format(format: GraphFormat) -> Self {
        Self {
            inner: RdfSerializer::from_format(format.into()),
        }
    }

    /// Returns a [`TripleWriter`] allowing writing triples into the given [`Write`] implementation
    pub fn triple_writer<W: Write>(self, write: W) -> TripleWriter<W> {
        TripleWriter {
            writer: self.inner.serialize_to_write(write),
        }
    }
}

/// Allows writing triples.
/// Could be built using a [`GraphSerializer`].
///
/// <div class="warning">
///
/// Do not forget to run the [`finish`](TripleWriter::finish()) method to properly write the last bytes of the file.</div>
///
/// ```
/// use oxigraph::io::{GraphFormat, GraphSerializer};
/// use oxigraph::model::*;
///
/// let mut buffer = Vec::new();
/// let mut writer = GraphSerializer::from_format(GraphFormat::NTriples).triple_writer(&mut buffer);
/// writer.write(&Triple {
///     subject: NamedNode::new("http://example.com/s")?.into(),
///     predicate: NamedNode::new("http://example.com/p")?,
///     object: NamedNode::new("http://example.com/o")?.into(),
/// })?;
/// writer.finish()?;
///
/// assert_eq!(
///     buffer.as_slice(),
///     "<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n".as_bytes()
/// );
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct TripleWriter<W: Write> {
    writer: ToWriteQuadWriter<W>,
}

impl<W: Write> TripleWriter<W> {
    /// Writes a triple
    pub fn write<'a>(&mut self, triple: impl Into<TripleRef<'a>>) -> io::Result<()> {
        self.writer.write_triple(triple)
    }

    /// Writes the last bytes of the file
    pub fn finish(self) -> io::Result<()> {
        self.writer.finish()?.flush()
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
/// assert_eq!(buffer.as_slice(), "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n".as_bytes());
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[deprecated(note = "use RdfSerializer instead", since = "0.4.0")]
pub struct DatasetSerializer {
    inner: RdfSerializer,
}

impl DatasetSerializer {
    /// Builds a serializer for the given format
    #[inline]
    pub fn from_format(format: DatasetFormat) -> Self {
        Self {
            inner: RdfSerializer::from_format(format.into()),
        }
    }

    /// Returns a [`QuadWriter`] allowing writing triples into the given [`Write`] implementation
    pub fn quad_writer<W: Write>(self, write: W) -> QuadWriter<W> {
        QuadWriter {
            writer: self.inner.serialize_to_write(write),
        }
    }
}

/// Allows writing triples.
/// Could be built using a [`DatasetSerializer`].
///
/// <div class="warning">
///
/// Do not forget to run the [`finish`](QuadWriter::finish()) method to properly write the last bytes of the file.</div>
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
/// assert_eq!(buffer.as_slice(), "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n".as_bytes());
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct QuadWriter<W: Write> {
    writer: ToWriteQuadWriter<W>,
}

impl<W: Write> QuadWriter<W> {
    /// Writes a quad
    pub fn write<'a>(&mut self, quad: impl Into<QuadRef<'a>>) -> io::Result<()> {
        self.writer.write_quad(quad)
    }

    /// Writes the last bytes of the file
    pub fn finish(self) -> io::Result<()> {
        self.writer.finish()?.flush()
    }
}
