//! Utilities to write RDF graphs and datasets.

use crate::format::RdfFormat;
use oxrdf::{GraphNameRef, IriParseError, QuadRef, TripleRef};
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
/// let mut writer = RdfSerializer::from_format(RdfFormat::NQuads).serialize_to_write(Vec::new());
/// writer.write_quad(&Quad {
///    subject: NamedNode::new("http://example.com/s")?.into(),
///    predicate: NamedNode::new("http://example.com/p")?,
///    object: NamedNode::new("http://example.com/o")?.into(),
///    graph_name: NamedNode::new("http://example.com/g")?.into()
/// })?;
/// assert_eq!(writer.finish()?, b"<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n");
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
#[derive(Clone)]
pub struct RdfSerializer {
    inner: RdfSerializerKind,
}

#[derive(Clone)]
enum RdfSerializerKind {
    NQuads(NQuadsSerializer),
    NTriples(NTriplesSerializer),
    RdfXml(RdfXmlSerializer),
    TriG(TriGSerializer),
    Turtle(TurtleSerializer),
}

impl RdfSerializer {
    /// Builds a serializer for the given format
    #[inline]
    pub fn from_format(format: RdfFormat) -> Self {
        Self {
            inner: match format {
                RdfFormat::NQuads => RdfSerializerKind::NQuads(NQuadsSerializer::new()),
                RdfFormat::NTriples => RdfSerializerKind::NTriples(NTriplesSerializer::new()),
                RdfFormat::RdfXml => RdfSerializerKind::RdfXml(RdfXmlSerializer::new()),
                RdfFormat::TriG => RdfSerializerKind::TriG(TriGSerializer::new()),
                RdfFormat::Turtle | RdfFormat::N3 => {
                    RdfSerializerKind::Turtle(TurtleSerializer::new())
                }
            },
        }
    }

    /// The format the serializer serializes to.
    ///
    /// ```
    /// use oxrdfio::{RdfFormat, RdfSerializer};
    ///
    /// assert_eq!(
    ///     RdfSerializer::from_format(RdfFormat::Turtle).format(),
    ///     RdfFormat::Turtle
    /// );
    /// ```
    pub fn format(&self) -> RdfFormat {
        match &self.inner {
            RdfSerializerKind::NQuads(_) => RdfFormat::NQuads,
            RdfSerializerKind::NTriples(_) => RdfFormat::NTriples,
            RdfSerializerKind::RdfXml(_) => RdfFormat::RdfXml,
            RdfSerializerKind::TriG(_) => RdfFormat::TriG,
            RdfSerializerKind::Turtle(_) => RdfFormat::Turtle,
        }
    }

    /// If the format supports it, sets a prefix.
    ///
    /// ```
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::{NamedNodeRef, TripleRef};
    /// use oxrdfio::{RdfFormat, RdfSerializer};
    ///
    /// let mut writer = RdfSerializer::from_format(RdfFormat::Turtle)
    ///     .with_prefix("schema", "http://schema.org/")?
    ///     .serialize_to_write(Vec::new());
    /// writer.write_triple(TripleRef {
    ///     subject: NamedNodeRef::new("http://example.com/s")?.into(),
    ///     predicate: rdf::TYPE.into(),
    ///     object: NamedNodeRef::new("http://schema.org/Person")?.into(),
    /// })?;
    /// assert_eq!(
    ///     writer.finish()?,
    ///     b"@prefix schema: <http://schema.org/> .\n<http://example.com/s> a schema:Person .\n"
    /// );
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_prefix(
        mut self,
        prefix_name: impl Into<String>,
        prefix_iri: impl Into<String>,
    ) -> Result<Self, IriParseError> {
        self.inner = match self.inner {
            RdfSerializerKind::NQuads(s) => RdfSerializerKind::NQuads(s),
            RdfSerializerKind::NTriples(s) => RdfSerializerKind::NTriples(s),
            RdfSerializerKind::RdfXml(s) => {
                RdfSerializerKind::RdfXml(s.with_prefix(prefix_name, prefix_iri)?)
            }
            RdfSerializerKind::TriG(s) => {
                RdfSerializerKind::TriG(s.with_prefix(prefix_name, prefix_iri)?)
            }
            RdfSerializerKind::Turtle(s) => {
                RdfSerializerKind::Turtle(s.with_prefix(prefix_name, prefix_iri)?)
            }
        };
        Ok(self)
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
    /// let mut writer = RdfSerializer::from_format(RdfFormat::NQuads).serialize_to_write(Vec::new());
    /// writer.write_quad(&Quad {
    ///    subject: NamedNode::new("http://example.com/s")?.into(),
    ///    predicate: NamedNode::new("http://example.com/p")?,
    ///    object: NamedNode::new("http://example.com/o")?.into(),
    ///    graph_name: NamedNode::new("http://example.com/g")?.into()
    /// })?;
    /// assert_eq!(writer.finish()?, b"<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n");
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn serialize_to_write<W: Write>(self, write: W) -> ToWriteQuadWriter<W> {
        ToWriteQuadWriter {
            formatter: match self.inner {
                RdfSerializerKind::NQuads(s) => {
                    ToWriteQuadWriterKind::NQuads(s.serialize_to_write(write))
                }
                RdfSerializerKind::NTriples(s) => {
                    ToWriteQuadWriterKind::NTriples(s.serialize_to_write(write))
                }
                RdfSerializerKind::RdfXml(s) => {
                    ToWriteQuadWriterKind::RdfXml(s.serialize_to_write(write))
                }
                RdfSerializerKind::TriG(s) => {
                    ToWriteQuadWriterKind::TriG(s.serialize_to_write(write))
                }
                RdfSerializerKind::Turtle(s) => {
                    ToWriteQuadWriterKind::Turtle(s.serialize_to_write(write))
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
    /// let mut writer = RdfSerializer::from_format(RdfFormat::NQuads).serialize_to_tokio_async_write(Vec::new());
    /// writer.write_quad(&Quad {
    ///     subject: NamedNode::new_unchecked("http://example.com/s").into(),
    ///     predicate: NamedNode::new_unchecked("http://example.com/p"),
    ///     object: NamedNode::new_unchecked("http://example.com/o").into(),
    ///     graph_name: NamedNode::new_unchecked("http://example.com/g").into()
    /// }).await?;
    /// assert_eq!(writer.finish().await?, "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n");
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async-tokio")]
    pub fn serialize_to_tokio_async_write<W: AsyncWrite + Unpin>(
        self,
        write: W,
    ) -> ToTokioAsyncWriteQuadWriter<W> {
        ToTokioAsyncWriteQuadWriter {
            formatter: match self.inner {
                RdfSerializerKind::NQuads(s) => {
                    ToTokioAsyncWriteQuadWriterKind::NQuads(s.serialize_to_tokio_async_write(write))
                }
                RdfSerializerKind::NTriples(s) => ToTokioAsyncWriteQuadWriterKind::NTriples(
                    s.serialize_to_tokio_async_write(write),
                ),
                RdfSerializerKind::RdfXml(s) => {
                    ToTokioAsyncWriteQuadWriterKind::RdfXml(s.serialize_to_tokio_async_write(write))
                }
                RdfSerializerKind::TriG(s) => {
                    ToTokioAsyncWriteQuadWriterKind::TriG(s.serialize_to_tokio_async_write(write))
                }
                RdfSerializerKind::Turtle(s) => {
                    ToTokioAsyncWriteQuadWriterKind::Turtle(s.serialize_to_tokio_async_write(write))
                }
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
/// let mut writer = RdfSerializer::from_format(RdfFormat::NQuads).serialize_to_write(Vec::new());
/// writer.write_quad(&Quad {
///    subject: NamedNode::new("http://example.com/s")?.into(),
///    predicate: NamedNode::new("http://example.com/p")?,
///    object: NamedNode::new("http://example.com/o")?.into(),
///    graph_name: NamedNode::new("http://example.com/g")?.into(),
/// })?;
/// assert_eq!(writer.finish()?, b"<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n");
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
/// let mut writer = RdfSerializer::from_format(RdfFormat::NQuads).serialize_to_tokio_async_write(Vec::new());
/// writer.write_quad(&Quad {
///     subject: NamedNode::new_unchecked("http://example.com/s").into(),
///     predicate: NamedNode::new_unchecked("http://example.com/p"),
///     object: NamedNode::new_unchecked("http://example.com/o").into(),
///     graph_name: NamedNode::new_unchecked("http://example.com/g").into()
/// }).await?;
/// assert_eq!(writer.finish().await?, "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n");
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
