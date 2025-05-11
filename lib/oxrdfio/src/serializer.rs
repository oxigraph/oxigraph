//! Utilities to write RDF graphs and datasets.

use crate::format::RdfFormat;
#[cfg(feature = "async-tokio")]
use oxjsonld::TokioAsyncWriterJsonLdSerializer;
use oxjsonld::{JsonLdProfile, JsonLdSerializer, WriterJsonLdSerializer};
use oxrdf::{GraphNameRef, IriParseError, QuadRef, TripleRef};
#[cfg(feature = "async-tokio")]
use oxrdfxml::TokioAsyncWriterRdfXmlSerializer;
use oxrdfxml::{RdfXmlSerializer, WriterRdfXmlSerializer};
#[cfg(feature = "async-tokio")]
use oxttl::nquads::TokioAsyncWriterNQuadsSerializer;
use oxttl::nquads::{NQuadsSerializer, WriterNQuadsSerializer};
#[cfg(feature = "async-tokio")]
use oxttl::ntriples::TokioAsyncWriterNTriplesSerializer;
use oxttl::ntriples::{NTriplesSerializer, WriterNTriplesSerializer};
#[cfg(feature = "async-tokio")]
use oxttl::trig::TokioAsyncWriterTriGSerializer;
use oxttl::trig::{TriGSerializer, WriterTriGSerializer};
#[cfg(feature = "async-tokio")]
use oxttl::turtle::TokioAsyncWriterTurtleSerializer;
use oxttl::turtle::{TurtleSerializer, WriterTurtleSerializer};
use std::io::{self, Write};
#[cfg(feature = "async-tokio")]
use tokio::io::AsyncWrite;

/// A serializer for RDF serialization formats.
///
/// It currently supports the following formats:
/// * [JSON-LD](https://www.w3.org/TR/json-ld/) ([`RdfFormat::JsonLd`])
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
/// let mut serializer = RdfSerializer::from_format(RdfFormat::NQuads).for_writer(Vec::new());
/// serializer.serialize_quad(&Quad {
///    subject: NamedNode::new("http://example.com/s")?.into(),
///    predicate: NamedNode::new("http://example.com/p")?,
///    object: NamedNode::new("http://example.com/o")?.into(),
///    graph_name: NamedNode::new("http://example.com/g")?.into()
/// })?;
/// assert_eq!(serializer.finish()?, b"<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n");
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
#[derive(Clone)]
pub struct RdfSerializer {
    inner: RdfSerializerKind,
}

#[derive(Clone)]
enum RdfSerializerKind {
    JsonLd(JsonLdSerializer),
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
                RdfFormat::JsonLd { .. } => RdfSerializerKind::JsonLd(JsonLdSerializer::new()),
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
            RdfSerializerKind::JsonLd(_) => RdfFormat::JsonLd {
                profile: JsonLdProfile::Streaming.into(), // TODO: also expanded?
            },
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
    /// let mut serializer = RdfSerializer::from_format(RdfFormat::Turtle)
    ///     .with_prefix("schema", "http://schema.org/")?
    ///     .for_writer(Vec::new());
    /// serializer.serialize_triple(TripleRef {
    ///     subject: NamedNodeRef::new("http://example.com/s")?.into(),
    ///     predicate: rdf::TYPE.into(),
    ///     object: NamedNodeRef::new("http://schema.org/Person")?.into(),
    /// })?;
    /// assert_eq!(
    ///     serializer.finish()?,
    ///     b"@prefix schema: <http://schema.org/> .\n<http://example.com/s> a schema:Person .\n"
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_prefix(
        mut self,
        prefix_name: impl Into<String>,
        prefix_iri: impl Into<String>,
    ) -> Result<Self, IriParseError> {
        self.inner = match self.inner {
            RdfSerializerKind::JsonLd(s) => RdfSerializerKind::JsonLd(s),
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

    /// If the format supports it, sets a base IRI.
    ///
    /// ```
    /// use oxrdf::vocab::rdf;
    /// use oxrdf::{NamedNodeRef, TripleRef};
    /// use oxrdfio::{RdfFormat, RdfSerializer};
    ///
    /// let mut serializer = RdfSerializer::from_format(RdfFormat::Turtle)
    ///     .with_base_iri("http://example.com")?
    ///     .with_prefix("ex", "http://example.com/ns#")?
    ///     .for_writer(Vec::new());
    /// serializer.serialize_triple(TripleRef::new(
    ///     NamedNodeRef::new("http://example.com/me")?,
    ///     rdf::TYPE,
    ///     NamedNodeRef::new("http://example.com/ns#Person")?,
    /// ))?;
    /// assert_eq!(
    ///     serializer.finish()?,
    ///     b"@base <http://example.com> .\n@prefix ex: </ns#> .\n</me> a ex:Person .\n",
    /// );
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_base_iri(mut self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        self.inner = match self.inner {
            RdfSerializerKind::JsonLd(s) => RdfSerializerKind::JsonLd(s),
            RdfSerializerKind::NQuads(s) => RdfSerializerKind::NQuads(s),
            RdfSerializerKind::NTriples(s) => RdfSerializerKind::NTriples(s),
            RdfSerializerKind::RdfXml(s) => RdfSerializerKind::RdfXml(s.with_base_iri(base_iri)?),
            RdfSerializerKind::TriG(s) => RdfSerializerKind::TriG(s.with_base_iri(base_iri)?),
            RdfSerializerKind::Turtle(s) => RdfSerializerKind::Turtle(s.with_base_iri(base_iri)?),
        };
        Ok(self)
    }

    /// Serializes to a [`Write`] implementation.
    ///
    /// <div class="warning">
    ///
    /// Do not forget to run the [`finish`](WriterQuadSerializer::finish()) method to properly write the last bytes of the file.</div>
    ///
    /// <div class="warning">
    ///
    /// This writer does unbuffered writes. You might want to use [`BufWriter`](io::BufWriter) to avoid that.</div>
    ///
    /// ```
    /// use oxrdfio::{RdfFormat, RdfSerializer};
    /// use oxrdf::{Quad, NamedNode};
    ///
    /// let mut serializer = RdfSerializer::from_format(RdfFormat::NQuads).for_writer(Vec::new());
    /// serializer.serialize_quad(&Quad {
    ///    subject: NamedNode::new("http://example.com/s")?.into(),
    ///    predicate: NamedNode::new("http://example.com/p")?,
    ///    object: NamedNode::new("http://example.com/o")?.into(),
    ///    graph_name: NamedNode::new("http://example.com/g")?.into()
    /// })?;
    /// assert_eq!(serializer.finish()?, b"<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n");
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_writer<W: Write>(self, writer: W) -> WriterQuadSerializer<W> {
        WriterQuadSerializer {
            inner: match self.inner {
                RdfSerializerKind::JsonLd(s) => {
                    WriterQuadSerializerKind::JsonLd(s.for_writer(writer))
                }
                RdfSerializerKind::NQuads(s) => {
                    WriterQuadSerializerKind::NQuads(s.for_writer(writer))
                }
                RdfSerializerKind::NTriples(s) => {
                    WriterQuadSerializerKind::NTriples(s.for_writer(writer))
                }
                RdfSerializerKind::RdfXml(s) => {
                    WriterQuadSerializerKind::RdfXml(s.for_writer(writer))
                }
                RdfSerializerKind::TriG(s) => WriterQuadSerializerKind::TriG(s.for_writer(writer)),
                RdfSerializerKind::Turtle(s) => {
                    WriterQuadSerializerKind::Turtle(s.for_writer(writer))
                }
            },
        }
    }

    /// Serializes to a Tokio [`AsyncWrite`] implementation.
    ///
    /// <div class="warning">
    ///
    /// Do not forget to run the [`finish`](TokioAsyncWriterQuadSerializer::finish()) method to properly write the last bytes of the file.</div>
    ///
    /// <div class="warning">
    ///
    /// This writer does unbuffered writes. You might want to use [`BufWriter`](tokio::io::BufWriter) to avoid that.</div>
    ///
    /// ```
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use oxrdfio::{RdfFormat, RdfSerializer};
    /// use oxrdf::{Quad, NamedNode};
    ///
    /// let mut serializer = RdfSerializer::from_format(RdfFormat::NQuads).for_tokio_async_writer(Vec::new());
    /// serializer.serialize_quad(&Quad {
    ///     subject: NamedNode::new("http://example.com/s")?.into(),
    ///     predicate: NamedNode::new("http://example.com/p")?,
    ///     object: NamedNode::new("http://example.com/o")?.into(),
    ///     graph_name: NamedNode::new("http://example.com/g")?.into()
    /// }).await?;
    /// assert_eq!(serializer.finish().await?, b"<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n");
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async-tokio")]
    pub fn for_tokio_async_writer<W: AsyncWrite + Unpin>(
        self,
        writer: W,
    ) -> TokioAsyncWriterQuadSerializer<W> {
        TokioAsyncWriterQuadSerializer {
            inner: match self.inner {
                RdfSerializerKind::JsonLd(s) => {
                    TokioAsyncWriterQuadSerializerKind::JsonLd(s.for_tokio_async_writer(writer))
                }
                RdfSerializerKind::NQuads(s) => {
                    TokioAsyncWriterQuadSerializerKind::NQuads(s.for_tokio_async_writer(writer))
                }
                RdfSerializerKind::NTriples(s) => {
                    TokioAsyncWriterQuadSerializerKind::NTriples(s.for_tokio_async_writer(writer))
                }
                RdfSerializerKind::RdfXml(s) => {
                    TokioAsyncWriterQuadSerializerKind::RdfXml(s.for_tokio_async_writer(writer))
                }
                RdfSerializerKind::TriG(s) => {
                    TokioAsyncWriterQuadSerializerKind::TriG(s.for_tokio_async_writer(writer))
                }
                RdfSerializerKind::Turtle(s) => {
                    TokioAsyncWriterQuadSerializerKind::Turtle(s.for_tokio_async_writer(writer))
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

/// Serializes quads or triples to a [`Write`] implementation.
///
/// Can be built using [`RdfSerializer::for_writer`].
///
/// <div class="warning">
///
/// Do not forget to run the [`finish`](WriterQuadSerializer::finish()) method to properly write the last bytes of the file.</div>
///
/// <div class="warning">
///
/// This writer does unbuffered writes. You might want to use [`BufWriter`](io::BufWriter) to avoid that.</div>
///
/// ```
/// use oxrdfio::{RdfFormat, RdfSerializer};
/// use oxrdf::{Quad, NamedNode};
///
/// let mut serializer = RdfSerializer::from_format(RdfFormat::NQuads).for_writer(Vec::new());
/// serializer.serialize_quad(&Quad {
///    subject: NamedNode::new("http://example.com/s")?.into(),
///    predicate: NamedNode::new("http://example.com/p")?,
///    object: NamedNode::new("http://example.com/o")?.into(),
///    graph_name: NamedNode::new("http://example.com/g")?.into(),
/// })?;
/// assert_eq!(serializer.finish()?, b"<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n");
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct WriterQuadSerializer<W: Write> {
    inner: WriterQuadSerializerKind<W>,
}

enum WriterQuadSerializerKind<W: Write> {
    JsonLd(WriterJsonLdSerializer<W>),
    NQuads(WriterNQuadsSerializer<W>),
    NTriples(WriterNTriplesSerializer<W>),
    RdfXml(WriterRdfXmlSerializer<W>),
    TriG(WriterTriGSerializer<W>),
    Turtle(WriterTurtleSerializer<W>),
}

impl<W: Write> WriterQuadSerializer<W> {
    /// Serializes a [`QuadRef`]
    pub fn serialize_quad<'a>(&mut self, quad: impl Into<QuadRef<'a>>) -> io::Result<()> {
        match &mut self.inner {
            WriterQuadSerializerKind::JsonLd(serializer) => serializer.serialize_quad(quad),
            WriterQuadSerializerKind::NQuads(serializer) => serializer.serialize_quad(quad),
            WriterQuadSerializerKind::NTriples(serializer) => {
                serializer.serialize_triple(to_triple(quad)?)
            }
            WriterQuadSerializerKind::RdfXml(serializer) => {
                serializer.serialize_triple(to_triple(quad)?)
            }
            WriterQuadSerializerKind::TriG(serializer) => serializer.serialize_quad(quad),
            WriterQuadSerializerKind::Turtle(serializer) => {
                serializer.serialize_triple(to_triple(quad)?)
            }
        }
    }

    /// Serializes a [`TripleRef`]
    pub fn serialize_triple<'a>(&mut self, triple: impl Into<TripleRef<'a>>) -> io::Result<()> {
        self.serialize_quad(triple.into().in_graph(GraphNameRef::DefaultGraph))
    }

    /// Writes the last bytes of the file
    ///
    /// Note that this function does not flush the writer. You need to do that if you are using a [`BufWriter`](io::BufWriter).
    pub fn finish(self) -> io::Result<W> {
        Ok(match self.inner {
            WriterQuadSerializerKind::JsonLd(serializer) => serializer.finish()?,
            WriterQuadSerializerKind::NQuads(serializer) => serializer.finish(),
            WriterQuadSerializerKind::NTriples(serializer) => serializer.finish(),
            WriterQuadSerializerKind::RdfXml(serializer) => serializer.finish()?,
            WriterQuadSerializerKind::TriG(serializer) => serializer.finish()?,
            WriterQuadSerializerKind::Turtle(serializer) => serializer.finish()?,
        })
    }
}

/// Serializes quads or triples to a [`AsyncWrite`] implementation.
///
/// Can be built using [`RdfSerializer::for_tokio_async_writer`].
///
/// <div class="warning">
///
/// Do not forget to run the [`finish`](WriterQuadSerializer::finish()) method to properly write the last bytes of the file.</div>
///
/// <div class="warning">
///
/// This writer does unbuffered writes. You might want to use [`BufWriter`](io::BufWriter) to avoid that.</div>
///
/// ```
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use oxrdfio::{RdfFormat, RdfSerializer};
/// use oxrdf::{Quad, NamedNode};
///
/// let mut serializer = RdfSerializer::from_format(RdfFormat::NQuads).for_tokio_async_writer(Vec::new());
/// serializer.serialize_quad(&Quad {
///     subject: NamedNode::new("http://example.com/s")?.into(),
///     predicate: NamedNode::new("http://example.com/p")?,
///     object: NamedNode::new("http://example.com/o")?.into(),
///     graph_name: NamedNode::new("http://example.com/g")?.into()
/// }).await?;
/// assert_eq!(serializer.finish().await?, b"<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n");
/// # Ok(())
/// # }
/// ```
#[must_use]
#[cfg(feature = "async-tokio")]
pub struct TokioAsyncWriterQuadSerializer<W: AsyncWrite + Unpin> {
    inner: TokioAsyncWriterQuadSerializerKind<W>,
}

#[cfg(feature = "async-tokio")]
enum TokioAsyncWriterQuadSerializerKind<W: AsyncWrite + Unpin> {
    JsonLd(TokioAsyncWriterJsonLdSerializer<W>),
    NQuads(TokioAsyncWriterNQuadsSerializer<W>),
    NTriples(TokioAsyncWriterNTriplesSerializer<W>),
    RdfXml(TokioAsyncWriterRdfXmlSerializer<W>),
    TriG(TokioAsyncWriterTriGSerializer<W>),
    Turtle(TokioAsyncWriterTurtleSerializer<W>),
}

#[cfg(feature = "async-tokio")]
impl<W: AsyncWrite + Unpin> TokioAsyncWriterQuadSerializer<W> {
    /// Serializes a [`QuadRef`]
    pub async fn serialize_quad<'a>(&mut self, quad: impl Into<QuadRef<'a>>) -> io::Result<()> {
        match &mut self.inner {
            TokioAsyncWriterQuadSerializerKind::JsonLd(serializer) => {
                serializer.serialize_quad(quad).await
            }
            TokioAsyncWriterQuadSerializerKind::NQuads(serializer) => {
                serializer.serialize_quad(quad).await
            }
            TokioAsyncWriterQuadSerializerKind::NTriples(serializer) => {
                serializer.serialize_triple(to_triple(quad)?).await
            }
            TokioAsyncWriterQuadSerializerKind::RdfXml(serializer) => {
                serializer.serialize_triple(to_triple(quad)?).await
            }
            TokioAsyncWriterQuadSerializerKind::TriG(serializer) => {
                serializer.serialize_quad(quad).await
            }
            TokioAsyncWriterQuadSerializerKind::Turtle(serializer) => {
                serializer.serialize_triple(to_triple(quad)?).await
            }
        }
    }

    /// Serializes a [`TripleRef`]
    pub async fn serialize_triple<'a>(
        &mut self,
        triple: impl Into<TripleRef<'a>>,
    ) -> io::Result<()> {
        self.serialize_quad(triple.into().in_graph(GraphNameRef::DefaultGraph))
            .await
    }

    /// Writes the last bytes of the file
    ///
    /// Note that this function does not flush the writer. You need to do that if you are using a [`BufWriter`](io::BufWriter).
    pub async fn finish(self) -> io::Result<W> {
        Ok(match self.inner {
            TokioAsyncWriterQuadSerializerKind::JsonLd(serializer) => serializer.finish().await?,
            TokioAsyncWriterQuadSerializerKind::NQuads(serializer) => serializer.finish(),
            TokioAsyncWriterQuadSerializerKind::NTriples(serializer) => serializer.finish(),
            TokioAsyncWriterQuadSerializerKind::RdfXml(serializer) => serializer.finish().await?,
            TokioAsyncWriterQuadSerializerKind::TriG(serializer) => serializer.finish().await?,
            TokioAsyncWriterQuadSerializerKind::Turtle(serializer) => serializer.finish().await?,
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
