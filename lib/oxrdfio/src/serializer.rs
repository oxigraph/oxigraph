//! Utilities to write RDF graphs and datasets.

use crate::format::RdfFormat;
use oxrdf::{GraphNameRef, QuadRef, TripleRef};
#[cfg(feature = "async-tokio")]
use oxrdfxml::ToTokioAsyncWriteRdfXmlWriter;
use oxrdfxml::{RdfXmlSerializer, ToWriteRdfXmlWriter};
#[cfg(feature = "async-tokio")]
use oxttl::nquads::ToTokioAsyncWriteNQuadsWriter;
use oxttl::nquads::{NQuadsSerializer, ToWriteNQuadsWriter};
#[cfg(feature = "async-tokio")]
use oxttl::ntriples::ToTokioAsyncWriteNTriplesWriter;
use oxttl::ntriples::{NTriplesSerializer, ToWriteNTriplesWriter};
#[cfg(feature = "async-tokio")]
use oxttl::trig::ToTokioAsyncWriteTriGWriter;
use oxttl::trig::{ToWriteTriGWriter, TriGSerializer};
#[cfg(feature = "async-tokio")]
use oxttl::turtle::ToTokioAsyncWriteTurtleWriter;
use oxttl::turtle::{ToWriteTurtleWriter, TurtleSerializer};
use std::io::{self, Write};
#[cfg(feature = "async-tokio")]
use tokio::io::AsyncWrite;

/// A serializer for RDF serialization formats.
///
/// It currently supports the following formats:
/// * [N3](https://w3c.github.io/N3/spec/) ([`RdfFormat::N3`])
/// * [N-Quads](https://www.w3.org/TR/n-quads/) ([`RdfFormat::NQuads`])
/// * [canonical](https://www.w3.org/TR/n-triples/#canonical-ntriples) [N-Triples](https://www.w3.org/TR/n-triples/) ([`RdfFormat::NTriples`])
/// * [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/) ([`RdfFormat::RdfXml`])
/// * [TriG](https://www.w3.org/TR/trig/) ([`RdfFormat::TriG`])
/// * [Turtle](https://www.w3.org/TR/turtle/) ([`RdfFormat::Turtle`])
///
/// ```
/// use oxrdfio::{RdfFormat, RdfSerializer};
/// use oxrdf::{Quad, NamedNode};
///
/// let mut buffer = Vec::new();
/// let mut writer = RdfSerializer::from_format(RdfFormat::NQuads).serialize_to_write(&mut buffer);
/// writer.write_quad(&Quad {
///    subject: NamedNode::new("http://example.com/s")?.into(),
///    predicate: NamedNode::new("http://example.com/p")?,
///    object: NamedNode::new("http://example.com/o")?.into(),
///    graph_name: NamedNode::new("http://example.com/g")?.into()
/// })?;
/// writer.finish()?;
///
/// assert_eq!(buffer.as_slice(), "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n".as_bytes());
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct RdfSerializer {
    format: RdfFormat,
}

impl RdfSerializer {
    /// Builds a serializer for the given format
    #[inline]
    pub fn from_format(format: RdfFormat) -> Self {
        Self { format }
    }

    /// The format the serializer serializes to.
    ///
    /// ```
    /// use oxrdfio::{RdfSerializer, RdfFormat};
    ///
    /// assert_eq!(RdfSerializer::from_format(RdfFormat::Turtle).format(), RdfFormat::Turtle);
    /// ```
    pub fn format(&self) -> RdfFormat {
        self.format
    }

    /// Writes to a [`Write`] implementation.
    ///
    /// <div class="warning">
    ///
    /// Do not forget to run the [`finish`](ToWriteQuadWriter::finish()) method to properly write the last bytes of the file.</div>
    ///
    /// <div class="warning">
    ///
    /// This writer does unbuffered writes. You might want to use [`BufWriter`](io::BufWriter) to avoid that.</div>
    ///
    /// ```
    /// use oxrdfio::{RdfFormat, RdfSerializer};
    /// use oxrdf::{Quad, NamedNode};
    ///
    /// let mut buffer = Vec::new();
    /// let mut writer = RdfSerializer::from_format(RdfFormat::NQuads).serialize_to_write(&mut buffer);
    /// writer.write_quad(&Quad {
    ///    subject: NamedNode::new("http://example.com/s")?.into(),
    ///    predicate: NamedNode::new("http://example.com/p")?,
    ///    object: NamedNode::new("http://example.com/o")?.into(),
    ///    graph_name: NamedNode::new("http://example.com/g")?.into()
    /// })?;
    /// writer.finish()?;
    ///
    /// assert_eq!(buffer.as_slice(), "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n".as_bytes());
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn serialize_to_write<W: Write>(self, write: W) -> ToWriteQuadWriter<W> {
        ToWriteQuadWriter {
            formatter: match self.format {
                RdfFormat::NQuads => {
                    ToWriteQuadWriterKind::NQuads(NQuadsSerializer::new().serialize_to_write(write))
                }
                RdfFormat::NTriples => ToWriteQuadWriterKind::NTriples(
                    NTriplesSerializer::new().serialize_to_write(write),
                ),
                RdfFormat::RdfXml => {
                    ToWriteQuadWriterKind::RdfXml(RdfXmlSerializer::new().serialize_to_write(write))
                }
                RdfFormat::TriG => {
                    ToWriteQuadWriterKind::TriG(TriGSerializer::new().serialize_to_write(write))
                }
                RdfFormat::Turtle | RdfFormat::N3 => {
                    ToWriteQuadWriterKind::Turtle(TurtleSerializer::new().serialize_to_write(write))
                }
            },
        }
    }

    /// Writes to a Tokio [`AsyncWrite`] implementation.
    ///
    /// <div class="warning">
    ///
    /// Do not forget to run the [`finish`](ToTokioAsyncWriteQuadWriter::finish()) method to properly write the last bytes of the file.</div>
    ///
    /// <div class="warning">
    ///
    /// This writer does unbuffered writes. You might want to use [`BufWriter`](tokio::io::BufWriter) to avoid that.</div>
    ///
    /// ```
    /// use oxrdfio::{RdfFormat, RdfSerializer};
    /// use oxrdf::{Quad, NamedNode};
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> std::io::Result<()> {
    /// let mut buffer = Vec::new();
    /// let mut writer = RdfSerializer::from_format(RdfFormat::NQuads).serialize_to_tokio_async_write(&mut buffer);
    /// writer.write_quad(&Quad {
    ///     subject: NamedNode::new_unchecked("http://example.com/s").into(),
    ///     predicate: NamedNode::new_unchecked("http://example.com/p"),
    ///     object: NamedNode::new_unchecked("http://example.com/o").into(),
    ///     graph_name: NamedNode::new_unchecked("http://example.com/g").into()
    /// }).await?;
    /// writer.finish().await?;
    ///
    /// assert_eq!(buffer.as_slice(), "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n".as_bytes());
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async-tokio")]
    pub fn serialize_to_tokio_async_write<W: AsyncWrite + Unpin>(
        self,
        write: W,
    ) -> ToTokioAsyncWriteQuadWriter<W> {
        ToTokioAsyncWriteQuadWriter {
            formatter: match self.format {
                RdfFormat::NQuads => ToTokioAsyncWriteQuadWriterKind::NQuads(
                    NQuadsSerializer::new().serialize_to_tokio_async_write(write),
                ),
                RdfFormat::NTriples => ToTokioAsyncWriteQuadWriterKind::NTriples(
                    NTriplesSerializer::new().serialize_to_tokio_async_write(write),
                ),
                RdfFormat::RdfXml => ToTokioAsyncWriteQuadWriterKind::RdfXml(
                    RdfXmlSerializer::new().serialize_to_tokio_async_write(write),
                ),
                RdfFormat::TriG => ToTokioAsyncWriteQuadWriterKind::TriG(
                    TriGSerializer::new().serialize_to_tokio_async_write(write),
                ),
                RdfFormat::Turtle | RdfFormat::N3 => ToTokioAsyncWriteQuadWriterKind::Turtle(
                    TurtleSerializer::new().serialize_to_tokio_async_write(write),
                ),
            },
        }
    }
}

impl From<RdfFormat> for RdfSerializer {
    fn from(format: RdfFormat) -> Self {
        Self::from_format(format)
    }
}

/// Writes quads or triples to a [`Write`] implementation.
///
/// Can be built using [`RdfSerializer::serialize_to_write`].
///
/// <div class="warning">
///
/// Do not forget to run the [`finish`](ToWriteQuadWriter::finish()) method to properly write the last bytes of the file.</div>
///
/// <div class="warning">
///
/// This writer does unbuffered writes. You might want to use [`BufWriter`](io::BufWriter) to avoid that.</div>
///
/// ```
/// use oxrdfio::{RdfFormat, RdfSerializer};
/// use oxrdf::{Quad, NamedNode};
///
/// let mut buffer = Vec::new();
/// let mut writer = RdfSerializer::from_format(RdfFormat::NQuads).serialize_to_write(&mut buffer);
/// writer.write_quad(&Quad {
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
pub struct ToWriteQuadWriter<W: Write> {
    formatter: ToWriteQuadWriterKind<W>,
}

enum ToWriteQuadWriterKind<W: Write> {
    NQuads(ToWriteNQuadsWriter<W>),
    NTriples(ToWriteNTriplesWriter<W>),
    RdfXml(ToWriteRdfXmlWriter<W>),
    TriG(ToWriteTriGWriter<W>),
    Turtle(ToWriteTurtleWriter<W>),
}

impl<W: Write> ToWriteQuadWriter<W> {
    /// Writes a [`QuadRef`]
    pub fn write_quad<'a>(&mut self, quad: impl Into<QuadRef<'a>>) -> io::Result<()> {
        match &mut self.formatter {
            ToWriteQuadWriterKind::NQuads(writer) => writer.write_quad(quad),
            ToWriteQuadWriterKind::NTriples(writer) => writer.write_triple(to_triple(quad)?),
            ToWriteQuadWriterKind::RdfXml(writer) => writer.write_triple(to_triple(quad)?),
            ToWriteQuadWriterKind::TriG(writer) => writer.write_quad(quad),
            ToWriteQuadWriterKind::Turtle(writer) => writer.write_triple(to_triple(quad)?),
        }
    }

    /// Writes a [`TripleRef`]
    pub fn write_triple<'a>(&mut self, triple: impl Into<TripleRef<'a>>) -> io::Result<()> {
        self.write_quad(triple.into().in_graph(GraphNameRef::DefaultGraph))
    }

    /// Writes the last bytes of the file
    ///
    /// Note that this function does not flush the writer. You need to do that if you are using a [`BufWriter`](io::BufWriter).
    pub fn finish(self) -> io::Result<W> {
        Ok(match self.formatter {
            ToWriteQuadWriterKind::NQuads(writer) => writer.finish(),
            ToWriteQuadWriterKind::NTriples(writer) => writer.finish(),
            ToWriteQuadWriterKind::RdfXml(writer) => writer.finish()?,
            ToWriteQuadWriterKind::TriG(writer) => writer.finish()?,
            ToWriteQuadWriterKind::Turtle(writer) => writer.finish()?,
        })
    }
}

/// Writes quads or triples to a [`Write`] implementation.
///
/// Can be built using [`RdfSerializer::serialize_to_write`].
///
/// <div class="warning">
///
/// Do not forget to run the [`finish`](ToWriteQuadWriter::finish()) method to properly write the last bytes of the file.</div>
///
/// <div class="warning">
///
/// This writer does unbuffered writes. You might want to use [`BufWriter`](io::BufWriter) to avoid that.</div>
///
/// ```
/// use oxrdfio::{RdfFormat, RdfSerializer};
/// use oxrdf::{Quad, NamedNode};
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> std::io::Result<()> {
/// let mut buffer = Vec::new();
/// let mut writer = RdfSerializer::from_format(RdfFormat::NQuads).serialize_to_tokio_async_write(&mut buffer);
/// writer.write_quad(&Quad {
///     subject: NamedNode::new_unchecked("http://example.com/s").into(),
///     predicate: NamedNode::new_unchecked("http://example.com/p"),
///     object: NamedNode::new_unchecked("http://example.com/o").into(),
///     graph_name: NamedNode::new_unchecked("http://example.com/g").into()
/// }).await?;
/// writer.finish().await?;
///
/// assert_eq!(buffer.as_slice(), "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n".as_bytes());
/// # Ok(())
/// # }
/// ```
#[must_use]
#[cfg(feature = "async-tokio")]
pub struct ToTokioAsyncWriteQuadWriter<W: AsyncWrite + Unpin> {
    formatter: ToTokioAsyncWriteQuadWriterKind<W>,
}

#[cfg(feature = "async-tokio")]
enum ToTokioAsyncWriteQuadWriterKind<W: AsyncWrite + Unpin> {
    NQuads(ToTokioAsyncWriteNQuadsWriter<W>),
    NTriples(ToTokioAsyncWriteNTriplesWriter<W>),
    RdfXml(ToTokioAsyncWriteRdfXmlWriter<W>),
    TriG(ToTokioAsyncWriteTriGWriter<W>),
    Turtle(ToTokioAsyncWriteTurtleWriter<W>),
}

#[cfg(feature = "async-tokio")]
impl<W: AsyncWrite + Unpin> ToTokioAsyncWriteQuadWriter<W> {
    /// Writes a [`QuadRef`]
    pub async fn write_quad<'a>(&mut self, quad: impl Into<QuadRef<'a>>) -> io::Result<()> {
        match &mut self.formatter {
            ToTokioAsyncWriteQuadWriterKind::NQuads(writer) => writer.write_quad(quad).await,
            ToTokioAsyncWriteQuadWriterKind::NTriples(writer) => {
                writer.write_triple(to_triple(quad)?).await
            }
            ToTokioAsyncWriteQuadWriterKind::RdfXml(writer) => {
                writer.write_triple(to_triple(quad)?).await
            }
            ToTokioAsyncWriteQuadWriterKind::TriG(writer) => writer.write_quad(quad).await,
            ToTokioAsyncWriteQuadWriterKind::Turtle(writer) => {
                writer.write_triple(to_triple(quad)?).await
            }
        }
    }

    /// Writes a [`TripleRef`]
    pub async fn write_triple<'a>(&mut self, triple: impl Into<TripleRef<'a>>) -> io::Result<()> {
        self.write_quad(triple.into().in_graph(GraphNameRef::DefaultGraph))
            .await
    }

    /// Writes the last bytes of the file
    ///
    /// Note that this function does not flush the writer. You need to do that if you are using a [`BufWriter`](io::BufWriter).
    pub async fn finish(self) -> io::Result<W> {
        Ok(match self.formatter {
            ToTokioAsyncWriteQuadWriterKind::NQuads(writer) => writer.finish(),
            ToTokioAsyncWriteQuadWriterKind::NTriples(writer) => writer.finish(),
            ToTokioAsyncWriteQuadWriterKind::RdfXml(writer) => writer.finish().await?,
            ToTokioAsyncWriteQuadWriterKind::TriG(writer) => writer.finish().await?,
            ToTokioAsyncWriteQuadWriterKind::Turtle(writer) => writer.finish().await?,
        })
    }
}

fn to_triple<'a>(quad: impl Into<QuadRef<'a>>) -> io::Result<TripleRef<'a>> {
    let quad = quad.into();
    if quad.graph_name.is_default_graph() {
        Ok(quad.into())
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Only quads in the default graph can be serialized to a RDF graph format",
        ))
    }
}
