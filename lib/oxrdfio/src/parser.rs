//! Utilities to read RDF graphs and datasets.

pub use crate::error::RdfParseError;
use crate::format::RdfFormat;
use crate::RdfSyntaxError;
use oxrdf::{BlankNode, GraphName, IriParseError, Quad, Subject, Term, Triple};
#[cfg(feature = "async-tokio")]
use oxrdfxml::TokioAsyncReaderRdfXmlParser;
use oxrdfxml::{RdfXmlParser, RdfXmlPrefixesIter, ReaderRdfXmlParser, SliceRdfXmlParser};
#[cfg(feature = "async-tokio")]
use oxttl::n3::TokioAsyncReaderN3Parser;
use oxttl::n3::{N3Parser, N3PrefixesIter, N3Quad, N3Term, ReaderN3Parser, SliceN3Parser};
#[cfg(feature = "async-tokio")]
use oxttl::nquads::TokioAsyncReaderNQuadsParser;
use oxttl::nquads::{NQuadsParser, ReaderNQuadsParser, SliceNQuadsParser};
#[cfg(feature = "async-tokio")]
use oxttl::ntriples::TokioAsyncReaderNTriplesParser;
use oxttl::ntriples::{NTriplesParser, ReaderNTriplesParser, SliceNTriplesParser};
#[cfg(feature = "async-tokio")]
use oxttl::trig::TokioAsyncReaderTriGParser;
use oxttl::trig::{ReaderTriGParser, SliceTriGParser, TriGParser, TriGPrefixesIter};
#[cfg(feature = "async-tokio")]
use oxttl::turtle::TokioAsyncReaderTurtleParser;
use oxttl::turtle::{ReaderTurtleParser, SliceTurtleParser, TurtleParser, TurtlePrefixesIter};
use std::collections::HashMap;
use std::io::Read;
#[cfg(feature = "async-tokio")]
use tokio::io::AsyncRead;

/// Parsers for RDF serialization formats.
///
/// It currently supports the following formats:
/// * [N3](https://w3c.github.io/N3/spec/) ([`RdfFormat::N3`])
/// * [N-Quads](https://www.w3.org/TR/n-quads/) ([`RdfFormat::NQuads`])
/// * [N-Triples](https://www.w3.org/TR/n-triples/) ([`RdfFormat::NTriples`])
/// * [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/) ([`RdfFormat::RdfXml`])
/// * [TriG](https://www.w3.org/TR/trig/) ([`RdfFormat::TriG`])
/// * [Turtle](https://www.w3.org/TR/turtle/) ([`RdfFormat::Turtle`])
///
/// Note the useful options:
/// - [`with_base_iri`](Self::with_base_iri) to resolve the relative IRIs.
/// - [`rename_blank_nodes`](Self::rename_blank_nodes) to rename the blank nodes to auto-generated numbers to avoid conflicts when merging RDF graphs together.
/// - [`without_named_graphs`](Self::without_named_graphs) to parse a single graph.
/// - [`unchecked`](Self::unchecked) to skip some validations if the file is already known to be valid.
///
/// ```
/// use oxrdfio::{RdfFormat, RdfParser};
///
/// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> .";
///
/// let quads = RdfParser::from_format(RdfFormat::NTriples)
///     .for_reader(file.as_bytes())
///     .collect::<Result<Vec<_>, _>>()?;
///
/// assert_eq!(quads.len(), 1);
/// assert_eq!(quads[0].subject.to_string(), "<http://example.com/s>");
/// # std::io::Result::Ok(())
/// ```
#[must_use]
#[derive(Clone)]
pub struct RdfParser {
    inner: RdfParserKind,
    default_graph: GraphName,
    without_named_graphs: bool,
    rename_blank_nodes: bool,
}

#[derive(Clone)]
enum RdfParserKind {
    N3(N3Parser),
    NQuads(NQuadsParser),
    NTriples(NTriplesParser),
    RdfXml(RdfXmlParser),
    TriG(TriGParser),
    Turtle(TurtleParser),
}

impl RdfParser {
    /// Builds a parser for the given format.
    #[inline]
    pub fn from_format(format: RdfFormat) -> Self {
        Self {
            inner: match format {
                RdfFormat::N3 => RdfParserKind::N3(N3Parser::new()),
                RdfFormat::NQuads => RdfParserKind::NQuads({
                    #[cfg(feature = "rdf-star")]
                    {
                        NQuadsParser::new().with_quoted_triples()
                    }
                    #[cfg(not(feature = "rdf-star"))]
                    {
                        NQuadsParser::new()
                    }
                }),
                RdfFormat::NTriples => RdfParserKind::NTriples({
                    #[cfg(feature = "rdf-star")]
                    {
                        NTriplesParser::new().with_quoted_triples()
                    }
                    #[cfg(not(feature = "rdf-star"))]
                    {
                        NTriplesParser::new()
                    }
                }),
                RdfFormat::RdfXml => RdfParserKind::RdfXml(RdfXmlParser::new()),
                RdfFormat::TriG => RdfParserKind::TriG({
                    #[cfg(feature = "rdf-star")]
                    {
                        TriGParser::new().with_quoted_triples()
                    }
                    #[cfg(not(feature = "rdf-star"))]
                    {
                        TriGParser::new()
                    }
                }),
                RdfFormat::Turtle => RdfParserKind::Turtle({
                    #[cfg(feature = "rdf-star")]
                    {
                        TurtleParser::new().with_quoted_triples()
                    }
                    #[cfg(not(feature = "rdf-star"))]
                    {
                        TurtleParser::new()
                    }
                }),
            },
            default_graph: GraphName::DefaultGraph,
            without_named_graphs: false,
            rename_blank_nodes: false,
        }
    }

    /// The format the parser uses.
    ///
    /// ```
    /// use oxrdfio::{RdfFormat, RdfParser};
    ///
    /// assert_eq!(
    ///     RdfParser::from_format(RdfFormat::Turtle).format(),
    ///     RdfFormat::Turtle
    /// );
    /// ```
    pub fn format(&self) -> RdfFormat {
        match &self.inner {
            RdfParserKind::N3(_) => RdfFormat::N3,
            RdfParserKind::NQuads(_) => RdfFormat::NQuads,
            RdfParserKind::NTriples(_) => RdfFormat::NTriples,
            RdfParserKind::RdfXml(_) => RdfFormat::RdfXml,
            RdfParserKind::TriG(_) => RdfFormat::TriG,
            RdfParserKind::Turtle(_) => RdfFormat::Turtle,
        }
    }

    /// Provides an IRI that could be used to resolve the file relative IRIs.
    ///
    /// ```
    /// use oxrdfio::{RdfFormat, RdfParser};
    ///
    /// let file = "</s> </p> </o> .";
    ///
    /// let quads = RdfParser::from_format(RdfFormat::Turtle)
    ///     .with_base_iri("http://example.com")?
    ///     .for_reader(file.as_bytes())
    ///     .collect::<Result<Vec<_>, _>>()?;
    ///
    /// assert_eq!(quads.len(), 1);
    /// assert_eq!(quads[0].subject.to_string(), "<http://example.com/s>");
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_base_iri(mut self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        self.inner = match self.inner {
            RdfParserKind::N3(p) => RdfParserKind::N3(p),
            RdfParserKind::NTriples(p) => RdfParserKind::NTriples(p),
            RdfParserKind::NQuads(p) => RdfParserKind::NQuads(p),
            RdfParserKind::RdfXml(p) => RdfParserKind::RdfXml(p.with_base_iri(base_iri)?),
            RdfParserKind::TriG(p) => RdfParserKind::TriG(p.with_base_iri(base_iri)?),
            RdfParserKind::Turtle(p) => RdfParserKind::Turtle(p.with_base_iri(base_iri)?),
        };
        Ok(self)
    }

    /// Provides the name graph name that should replace the default graph in the returned quads.
    ///
    /// ```
    /// use oxrdf::NamedNode;
    /// use oxrdfio::{RdfFormat, RdfParser};
    ///
    /// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> .";
    ///
    /// let quads = RdfParser::from_format(RdfFormat::Turtle)
    ///     .with_default_graph(NamedNode::new("http://example.com/g")?)
    ///     .for_reader(file.as_bytes())
    ///     .collect::<Result<Vec<_>, _>>()?;
    ///
    /// assert_eq!(quads.len(), 1);
    /// assert_eq!(quads[0].graph_name.to_string(), "<http://example.com/g>");
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_default_graph(mut self, default_graph: impl Into<GraphName>) -> Self {
        self.default_graph = default_graph.into();
        self
    }

    /// Sets that the parser must fail if parsing a named graph.
    ///
    /// This function restricts the parser to only parse a single [RDF graph](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-graph) and not an [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
    ///
    /// ```
    /// use oxrdfio::{RdfFormat, RdfParser};
    ///
    /// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .";
    ///
    /// let parser = RdfParser::from_format(RdfFormat::NQuads).without_named_graphs();
    /// assert!(parser.for_reader(file.as_bytes()).next().unwrap().is_err());
    /// ```
    #[inline]
    pub fn without_named_graphs(mut self) -> Self {
        self.without_named_graphs = true;
        self
    }

    /// Renames the blank nodes ids from the ones set in the serialization to random ids.
    ///
    /// This allows to avoid id conflicts when merging graphs together.
    ///
    /// ```
    /// use oxrdfio::{RdfFormat, RdfParser};
    ///
    /// let file = "_:a <http://example.com/p> <http://example.com/o> .";
    ///
    /// let result1 = RdfParser::from_format(RdfFormat::NQuads)
    ///     .rename_blank_nodes()
    ///     .for_reader(file.as_bytes())
    ///     .collect::<Result<Vec<_>, _>>()?;
    /// let result2 = RdfParser::from_format(RdfFormat::NQuads)
    ///     .rename_blank_nodes()
    ///     .for_reader(file.as_bytes())
    ///     .collect::<Result<Vec<_>, _>>()?;
    /// assert_ne!(result1, result2);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn rename_blank_nodes(mut self) -> Self {
        self.rename_blank_nodes = true;
        self
    }

    /// Assumes the file is valid to make parsing faster.
    ///
    /// It will skip some validations.
    ///
    /// Note that if the file is actually not valid, broken RDF might be emitted by the parser.
    #[inline]
    pub fn unchecked(mut self) -> Self {
        self.inner = match self.inner {
            RdfParserKind::N3(p) => RdfParserKind::N3(p.unchecked()),
            RdfParserKind::NTriples(p) => RdfParserKind::NTriples(p.unchecked()),
            RdfParserKind::NQuads(p) => RdfParserKind::NQuads(p.unchecked()),
            RdfParserKind::RdfXml(p) => RdfParserKind::RdfXml(p.unchecked()),
            RdfParserKind::TriG(p) => RdfParserKind::TriG(p.unchecked()),
            RdfParserKind::Turtle(p) => RdfParserKind::Turtle(p.unchecked()),
        };
        self
    }

    /// Parses from a [`Read`] implementation and returns an iterator of quads.
    ///
    /// Reads are buffered.
    ///
    /// ```
    /// use oxrdfio::{RdfFormat, RdfParser};
    ///
    /// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> .";
    ///
    /// let quads = RdfParser::from_format(RdfFormat::NTriples)
    ///     .for_reader(file.as_bytes())
    ///     .collect::<Result<Vec<_>, _>>()?;
    ///
    /// assert_eq!(quads.len(), 1);
    /// assert_eq!(quads[0].subject.to_string(), "<http://example.com/s>");
    /// # std::io::Result::Ok(())
    /// ```
    pub fn for_reader<R: Read>(self, reader: R) -> ReaderQuadParser<R> {
        ReaderQuadParser {
            inner: match self.inner {
                RdfParserKind::N3(p) => ReaderQuadParserKind::N3(p.for_reader(reader)),
                RdfParserKind::NQuads(p) => ReaderQuadParserKind::NQuads(p.for_reader(reader)),
                RdfParserKind::NTriples(p) => ReaderQuadParserKind::NTriples(p.for_reader(reader)),
                RdfParserKind::RdfXml(p) => ReaderQuadParserKind::RdfXml(p.for_reader(reader)),
                RdfParserKind::TriG(p) => ReaderQuadParserKind::TriG(p.for_reader(reader)),
                RdfParserKind::Turtle(p) => ReaderQuadParserKind::Turtle(p.for_reader(reader)),
            },
            mapper: QuadMapper {
                default_graph: self.default_graph.clone(),
                without_named_graphs: self.without_named_graphs,
                blank_node_map: self.rename_blank_nodes.then(HashMap::new),
            },
        }
    }

    /// Parses from a Tokio [`AsyncRead`] implementation and returns an async iterator of quads.
    ///
    /// Reads are buffered.
    ///
    /// ```
    /// use oxrdfio::{RdfFormat, RdfParser};
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), oxrdfio::RdfParseError> {
    /// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> .";
    ///
    /// let mut parser =
    ///     RdfParser::from_format(RdfFormat::NTriples).for_tokio_async_reader(file.as_bytes());
    /// if let Some(quad) = parser.next().await {
    ///     assert_eq!(quad?.subject.to_string(), "<http://example.com/s>");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async-tokio")]
    pub fn for_tokio_async_reader<R: AsyncRead + Unpin>(
        self,
        reader: R,
    ) -> TokioAsyncReaderQuadParser<R> {
        TokioAsyncReaderQuadParser {
            inner: match self.inner {
                RdfParserKind::N3(p) => {
                    TokioAsyncReaderQuadParserKind::N3(p.for_tokio_async_reader(reader))
                }
                RdfParserKind::NQuads(p) => {
                    TokioAsyncReaderQuadParserKind::NQuads(p.for_tokio_async_reader(reader))
                }
                RdfParserKind::NTriples(p) => {
                    TokioAsyncReaderQuadParserKind::NTriples(p.for_tokio_async_reader(reader))
                }
                RdfParserKind::RdfXml(p) => {
                    TokioAsyncReaderQuadParserKind::RdfXml(p.for_tokio_async_reader(reader))
                }
                RdfParserKind::TriG(p) => {
                    TokioAsyncReaderQuadParserKind::TriG(p.for_tokio_async_reader(reader))
                }
                RdfParserKind::Turtle(p) => {
                    TokioAsyncReaderQuadParserKind::Turtle(p.for_tokio_async_reader(reader))
                }
            },
            mapper: QuadMapper {
                default_graph: self.default_graph.clone(),
                without_named_graphs: self.without_named_graphs,
                blank_node_map: self.rename_blank_nodes.then(HashMap::new),
            },
        }
    }

    /// Parses from a byte slice and returns an iterator of quads.
    ///
    /// ```
    /// use oxrdfio::{RdfFormat, RdfParser};
    ///
    /// let file = b"<http://example.com/s> <http://example.com/p> <http://example.com/o> .";
    ///
    /// let quads = RdfParser::from_format(RdfFormat::NTriples)
    ///     .for_slice(file)
    ///     .collect::<Result<Vec<_>, _>>()?;
    ///
    /// assert_eq!(quads.len(), 1);
    /// assert_eq!(quads[0].subject.to_string(), "<http://example.com/s>");
    /// # std::io::Result::Ok(())
    /// ```
    pub fn for_slice(self, slice: &[u8]) -> SliceQuadParser<'_> {
        SliceQuadParser {
            inner: match self.inner {
                RdfParserKind::N3(p) => SliceQuadParserKind::N3(p.for_slice(slice)),
                RdfParserKind::NQuads(p) => SliceQuadParserKind::NQuads(p.for_slice(slice)),
                RdfParserKind::NTriples(p) => SliceQuadParserKind::NTriples(p.for_slice(slice)),
                RdfParserKind::RdfXml(p) => SliceQuadParserKind::RdfXml(p.for_slice(slice)),
                RdfParserKind::TriG(p) => SliceQuadParserKind::TriG(p.for_slice(slice)),
                RdfParserKind::Turtle(p) => SliceQuadParserKind::Turtle(p.for_slice(slice)),
            },
            mapper: QuadMapper {
                default_graph: self.default_graph.clone(),
                without_named_graphs: self.without_named_graphs,
                blank_node_map: self.rename_blank_nodes.then(HashMap::new),
            },
        }
    }
}

impl From<RdfFormat> for RdfParser {
    fn from(format: RdfFormat) -> Self {
        Self::from_format(format)
    }
}

/// Parses a RDF file from a [`Read`] implementation.
///
/// Can be built using [`RdfParser::for_reader`].
///
/// Reads are buffered.
///
/// ```
/// use oxrdfio::{RdfFormat, RdfParser};
///
/// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> .";
///
/// let quads = RdfParser::from_format(RdfFormat::NTriples)
///     .for_reader(file.as_bytes())
///     .collect::<Result<Vec<_>, _>>()?;
///
/// assert_eq!(quads.len(), 1);
/// assert_eq!(quads[0].subject.to_string(), "<http://example.com/s>");
/// # std::io::Result::Ok(())
/// ```
#[must_use]
pub struct ReaderQuadParser<R: Read> {
    inner: ReaderQuadParserKind<R>,
    mapper: QuadMapper,
}

enum ReaderQuadParserKind<R: Read> {
    N3(ReaderN3Parser<R>),
    NQuads(ReaderNQuadsParser<R>),
    NTriples(ReaderNTriplesParser<R>),
    RdfXml(ReaderRdfXmlParser<R>),
    TriG(ReaderTriGParser<R>),
    Turtle(ReaderTurtleParser<R>),
}

impl<R: Read> Iterator for ReaderQuadParser<R> {
    type Item = Result<Quad, RdfParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(match &mut self.inner {
            ReaderQuadParserKind::N3(parser) => match parser.next()? {
                Ok(quad) => self.mapper.map_n3_quad(quad).map_err(Into::into),
                Err(e) => Err(e.into()),
            },
            ReaderQuadParserKind::NQuads(parser) => match parser.next()? {
                Ok(quad) => self.mapper.map_quad(quad).map_err(Into::into),
                Err(e) => Err(e.into()),
            },
            ReaderQuadParserKind::NTriples(parser) => match parser.next()? {
                Ok(triple) => Ok(self.mapper.map_triple_to_quad(triple)),
                Err(e) => Err(e.into()),
            },
            ReaderQuadParserKind::RdfXml(parser) => match parser.next()? {
                Ok(triple) => Ok(self.mapper.map_triple_to_quad(triple)),
                Err(e) => Err(e.into()),
            },
            ReaderQuadParserKind::TriG(parser) => match parser.next()? {
                Ok(quad) => self.mapper.map_quad(quad).map_err(Into::into),
                Err(e) => Err(e.into()),
            },
            ReaderQuadParserKind::Turtle(parser) => match parser.next()? {
                Ok(triple) => Ok(self.mapper.map_triple_to_quad(triple)),
                Err(e) => Err(e.into()),
            },
        })
    }
}

impl<R: Read> ReaderQuadParser<R> {
    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// An empty iterator is return if the format does not support prefixes.
    ///
    /// ```
    /// use oxrdfio::{RdfFormat, RdfParser};
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = RdfParser::from_format(RdfFormat::Turtle).for_reader(file.as_slice());
    /// assert!(parser.prefixes().collect::<Vec<_>>().is_empty()); // No prefix at the beginning
    ///
    /// parser.next().unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn prefixes(&self) -> PrefixesIter<'_> {
        PrefixesIter {
            inner: match &self.inner {
                ReaderQuadParserKind::N3(p) => PrefixesIterKind::N3(p.prefixes()),
                ReaderQuadParserKind::TriG(p) => PrefixesIterKind::TriG(p.prefixes()),
                ReaderQuadParserKind::Turtle(p) => PrefixesIterKind::Turtle(p.prefixes()),
                ReaderQuadParserKind::RdfXml(p) => PrefixesIterKind::RdfXml(p.prefixes()),
                ReaderQuadParserKind::NQuads(_) | ReaderQuadParserKind::NTriples(_) => {
                    PrefixesIterKind::None
                }
            },
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// `None` is returned if no base IRI is set or the format does not support base IRIs.
    ///
    /// ```
    /// use oxrdfio::{RdfFormat, RdfParser};
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = RdfParser::from_format(RdfFormat::Turtle).for_reader(file.as_slice());
    /// assert!(parser.base_iri().is_none()); // No base at the beginning because none has been given to the parser.
    ///
    /// parser.next().unwrap()?; // We read the first triple
    /// assert_eq!(parser.base_iri(), Some("http://example.com/")); // There is now a base IRI.
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn base_iri(&self) -> Option<&str> {
        match &self.inner {
            ReaderQuadParserKind::N3(p) => p.base_iri(),
            ReaderQuadParserKind::TriG(p) => p.base_iri(),
            ReaderQuadParserKind::Turtle(p) => p.base_iri(),
            ReaderQuadParserKind::RdfXml(p) => p.base_iri(),
            ReaderQuadParserKind::NQuads(_) | ReaderQuadParserKind::NTriples(_) => None,
        }
    }
}

/// Parses a RDF file from a Tokio [`AsyncRead`] implementation.
///
/// Can be built using [`RdfParser::for_tokio_async_reader`].
///
/// Reads are buffered.
///
/// ```
/// use oxrdfio::{RdfFormat, RdfParser};
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), oxrdfio::RdfParseError> {
/// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> .";
///
/// let mut parser =
///     RdfParser::from_format(RdfFormat::NTriples).for_tokio_async_reader(file.as_bytes());
/// if let Some(quad) = parser.next().await {
///     assert_eq!(quad?.subject.to_string(), "<http://example.com/s>");
/// }
/// # Ok(())
/// # }
/// ```
#[must_use]
#[cfg(feature = "async-tokio")]
pub struct TokioAsyncReaderQuadParser<R: AsyncRead + Unpin> {
    inner: TokioAsyncReaderQuadParserKind<R>,
    mapper: QuadMapper,
}

#[cfg(feature = "async-tokio")]
enum TokioAsyncReaderQuadParserKind<R: AsyncRead + Unpin> {
    N3(TokioAsyncReaderN3Parser<R>),
    NQuads(TokioAsyncReaderNQuadsParser<R>),
    NTriples(TokioAsyncReaderNTriplesParser<R>),
    RdfXml(TokioAsyncReaderRdfXmlParser<R>),
    TriG(TokioAsyncReaderTriGParser<R>),
    Turtle(TokioAsyncReaderTurtleParser<R>),
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> TokioAsyncReaderQuadParser<R> {
    pub async fn next(&mut self) -> Option<Result<Quad, RdfParseError>> {
        Some(match &mut self.inner {
            TokioAsyncReaderQuadParserKind::N3(parser) => match parser.next().await? {
                Ok(quad) => self.mapper.map_n3_quad(quad).map_err(Into::into),
                Err(e) => Err(e.into()),
            },
            TokioAsyncReaderQuadParserKind::NQuads(parser) => match parser.next().await? {
                Ok(quad) => self.mapper.map_quad(quad).map_err(Into::into),
                Err(e) => Err(e.into()),
            },
            TokioAsyncReaderQuadParserKind::NTriples(parser) => match parser.next().await? {
                Ok(triple) => Ok(self.mapper.map_triple_to_quad(triple)),
                Err(e) => Err(e.into()),
            },
            TokioAsyncReaderQuadParserKind::RdfXml(parser) => match parser.next().await? {
                Ok(triple) => Ok(self.mapper.map_triple_to_quad(triple)),
                Err(e) => Err(e.into()),
            },
            TokioAsyncReaderQuadParserKind::TriG(parser) => match parser.next().await? {
                Ok(quad) => self.mapper.map_quad(quad).map_err(Into::into),
                Err(e) => Err(e.into()),
            },
            TokioAsyncReaderQuadParserKind::Turtle(parser) => match parser.next().await? {
                Ok(triple) => Ok(self.mapper.map_triple_to_quad(triple)),
                Err(e) => Err(e.into()),
            },
        })
    }

    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// An empty iterator is return if the format does not support prefixes.
    ///
    /// ```
    /// use oxrdfio::{RdfFormat, RdfParser};
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), oxrdfio::RdfParseError> {
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser =
    ///     RdfParser::from_format(RdfFormat::Turtle).for_tokio_async_reader(file.as_slice());
    /// assert_eq!(parser.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// parser.next().await.unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// # Ok(())
    /// # }
    /// ```
    pub fn prefixes(&self) -> PrefixesIter<'_> {
        PrefixesIter {
            inner: match &self.inner {
                TokioAsyncReaderQuadParserKind::N3(p) => PrefixesIterKind::N3(p.prefixes()),
                TokioAsyncReaderQuadParserKind::TriG(p) => PrefixesIterKind::TriG(p.prefixes()),
                TokioAsyncReaderQuadParserKind::Turtle(p) => PrefixesIterKind::Turtle(p.prefixes()),
                TokioAsyncReaderQuadParserKind::RdfXml(p) => PrefixesIterKind::RdfXml(p.prefixes()),
                TokioAsyncReaderQuadParserKind::NQuads(_)
                | TokioAsyncReaderQuadParserKind::NTriples(_) => PrefixesIterKind::None,
            },
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// `None` is returned if no base IRI is set or the format does not support base IRIs.
    ///
    /// ```
    /// use oxrdfio::{RdfFormat, RdfParser};
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), oxrdfio::RdfParseError> {
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser =
    ///     RdfParser::from_format(RdfFormat::Turtle).for_tokio_async_reader(file.as_slice());
    /// assert!(parser.base_iri().is_none()); // No base IRI at the beginning
    ///
    /// parser.next().await.unwrap()?; // We read the first triple
    /// assert_eq!(parser.base_iri(), Some("http://example.com/")); // There is now a base IRI
    /// # Ok(())
    /// # }
    /// ```
    pub fn base_iri(&self) -> Option<&str> {
        match &self.inner {
            TokioAsyncReaderQuadParserKind::N3(p) => p.base_iri(),
            TokioAsyncReaderQuadParserKind::TriG(p) => p.base_iri(),
            TokioAsyncReaderQuadParserKind::Turtle(p) => p.base_iri(),
            TokioAsyncReaderQuadParserKind::RdfXml(p) => p.base_iri(),
            TokioAsyncReaderQuadParserKind::NQuads(_)
            | TokioAsyncReaderQuadParserKind::NTriples(_) => None,
        }
    }
}

/// Parses a RDF file from a byte slice.
///
/// Can be built using [`RdfParser::for_slice`].
///
/// ```
/// use oxrdfio::{RdfFormat, RdfParser};
///
/// let file = b"<http://example.com/s> <http://example.com/p> <http://example.com/o> .";
///
/// let quads = RdfParser::from_format(RdfFormat::NTriples)
///     .for_slice(file)
///     .collect::<Result<Vec<_>, _>>()?;
///
/// assert_eq!(quads.len(), 1);
/// assert_eq!(quads[0].subject.to_string(), "<http://example.com/s>");
/// # std::io::Result::Ok(())
/// ```
#[must_use]
pub struct SliceQuadParser<'a> {
    inner: SliceQuadParserKind<'a>,
    mapper: QuadMapper,
}

enum SliceQuadParserKind<'a> {
    N3(SliceN3Parser<'a>),
    NQuads(SliceNQuadsParser<'a>),
    NTriples(SliceNTriplesParser<'a>),
    RdfXml(SliceRdfXmlParser<'a>),
    TriG(SliceTriGParser<'a>),
    Turtle(SliceTurtleParser<'a>),
}

impl<'a> Iterator for SliceQuadParser<'a> {
    type Item = Result<Quad, RdfSyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(match &mut self.inner {
            SliceQuadParserKind::N3(parser) => match parser.next()? {
                Ok(quad) => self.mapper.map_n3_quad(quad),
                Err(e) => Err(e.into()),
            },
            SliceQuadParserKind::NQuads(parser) => match parser.next()? {
                Ok(quad) => self.mapper.map_quad(quad),
                Err(e) => Err(e.into()),
            },
            SliceQuadParserKind::NTriples(parser) => match parser.next()? {
                Ok(triple) => Ok(self.mapper.map_triple_to_quad(triple)),
                Err(e) => Err(e.into()),
            },
            SliceQuadParserKind::RdfXml(parser) => match parser.next()? {
                Ok(triple) => Ok(self.mapper.map_triple_to_quad(triple)),
                Err(e) => Err(e.into()),
            },
            SliceQuadParserKind::TriG(parser) => match parser.next()? {
                Ok(quad) => self.mapper.map_quad(quad),
                Err(e) => Err(e.into()),
            },
            SliceQuadParserKind::Turtle(parser) => match parser.next()? {
                Ok(triple) => Ok(self.mapper.map_triple_to_quad(triple)),
                Err(e) => Err(e.into()),
            },
        })
    }
}

impl<'a> SliceQuadParser<'a> {
    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// An empty iterator is return if the format does not support prefixes.
    ///
    /// ```
    /// use oxrdfio::{RdfFormat, RdfParser};
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = RdfParser::from_format(RdfFormat::Turtle).for_slice(file);
    /// assert!(parser.prefixes().collect::<Vec<_>>().is_empty()); // No prefix at the beginning
    ///
    /// parser.next().unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn prefixes(&self) -> PrefixesIter<'_> {
        PrefixesIter {
            inner: match &self.inner {
                SliceQuadParserKind::N3(p) => PrefixesIterKind::N3(p.prefixes()),
                SliceQuadParserKind::TriG(p) => PrefixesIterKind::TriG(p.prefixes()),
                SliceQuadParserKind::Turtle(p) => PrefixesIterKind::Turtle(p.prefixes()),
                SliceQuadParserKind::RdfXml(p) => PrefixesIterKind::RdfXml(p.prefixes()),
                SliceQuadParserKind::NQuads(_) | SliceQuadParserKind::NTriples(_) => {
                    PrefixesIterKind::None
                }
            },
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// `None` is returned if no base IRI is set or the format does not support base IRIs.
    ///
    /// ```
    /// use oxrdfio::{RdfFormat, RdfParser};
    ///
    /// let file = br#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = RdfParser::from_format(RdfFormat::Turtle).for_slice(file);
    /// assert!(parser.base_iri().is_none()); // No base at the beginning because none has been given to the parser.
    ///
    /// parser.next().unwrap()?; // We read the first triple
    /// assert_eq!(parser.base_iri(), Some("http://example.com/")); // There is now a base IRI.
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn base_iri(&self) -> Option<&str> {
        match &self.inner {
            SliceQuadParserKind::N3(p) => p.base_iri(),
            SliceQuadParserKind::TriG(p) => p.base_iri(),
            SliceQuadParserKind::Turtle(p) => p.base_iri(),
            SliceQuadParserKind::RdfXml(p) => p.base_iri(),
            SliceQuadParserKind::NQuads(_) | SliceQuadParserKind::NTriples(_) => None,
        }
    }
}

/// Iterator on the file prefixes.
///
/// See [`ReaderQuadParser::prefixes`].
pub struct PrefixesIter<'a> {
    inner: PrefixesIterKind<'a>,
}

enum PrefixesIterKind<'a> {
    Turtle(TurtlePrefixesIter<'a>),
    TriG(TriGPrefixesIter<'a>),
    N3(N3PrefixesIter<'a>),
    RdfXml(RdfXmlPrefixesIter<'a>),
    None,
}

impl<'a> Iterator for PrefixesIter<'a> {
    type Item = (&'a str, &'a str);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.inner {
            PrefixesIterKind::Turtle(iter) => iter.next(),
            PrefixesIterKind::TriG(iter) => iter.next(),
            PrefixesIterKind::N3(iter) => iter.next(),
            PrefixesIterKind::RdfXml(iter) => iter.next(),
            PrefixesIterKind::None => None,
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match &self.inner {
            PrefixesIterKind::Turtle(iter) => iter.size_hint(),
            PrefixesIterKind::TriG(iter) => iter.size_hint(),
            PrefixesIterKind::N3(iter) => iter.size_hint(),
            PrefixesIterKind::RdfXml(iter) => iter.size_hint(),
            PrefixesIterKind::None => (0, Some(0)),
        }
    }
}

struct QuadMapper {
    default_graph: GraphName,
    without_named_graphs: bool,
    blank_node_map: Option<HashMap<BlankNode, BlankNode>>,
}

impl QuadMapper {
    fn map_blank_node(&mut self, node: BlankNode) -> BlankNode {
        if let Some(blank_node_map) = &mut self.blank_node_map {
            blank_node_map
                .entry(node)
                .or_insert_with(BlankNode::default)
                .clone()
        } else {
            node
        }
    }

    fn map_subject(&mut self, node: Subject) -> Subject {
        match node {
            Subject::NamedNode(node) => node.into(),
            Subject::BlankNode(node) => self.map_blank_node(node).into(),
            #[cfg(feature = "rdf-star")]
            Subject::Triple(triple) => self.map_triple(*triple).into(),
        }
    }

    fn map_term(&mut self, node: Term) -> Term {
        match node {
            Term::NamedNode(node) => node.into(),
            Term::BlankNode(node) => self.map_blank_node(node).into(),
            Term::Literal(literal) => literal.into(),
            #[cfg(feature = "rdf-star")]
            Term::Triple(triple) => self.map_triple(*triple).into(),
        }
    }

    fn map_triple(&mut self, triple: Triple) -> Triple {
        Triple {
            subject: self.map_subject(triple.subject),
            predicate: triple.predicate,
            object: self.map_term(triple.object),
        }
    }

    fn map_graph_name(&mut self, graph_name: GraphName) -> Result<GraphName, RdfSyntaxError> {
        match graph_name {
            GraphName::NamedNode(node) => {
                if self.without_named_graphs {
                    Err(RdfSyntaxError::msg("Named graphs are not allowed"))
                } else {
                    Ok(node.into())
                }
            }
            GraphName::BlankNode(node) => {
                if self.without_named_graphs {
                    Err(RdfSyntaxError::msg("Named graphs are not allowed"))
                } else {
                    Ok(self.map_blank_node(node).into())
                }
            }
            GraphName::DefaultGraph => Ok(self.default_graph.clone()),
        }
    }

    fn map_quad(&mut self, quad: Quad) -> Result<Quad, RdfSyntaxError> {
        Ok(Quad {
            subject: self.map_subject(quad.subject),
            predicate: quad.predicate,
            object: self.map_term(quad.object),
            graph_name: self.map_graph_name(quad.graph_name)?,
        })
    }

    fn map_triple_to_quad(&mut self, triple: Triple) -> Quad {
        self.map_triple(triple).in_graph(self.default_graph.clone())
    }

    fn map_n3_quad(&mut self, quad: N3Quad) -> Result<Quad, RdfSyntaxError> {
        Ok(Quad {
            subject: match quad.subject {
                N3Term::NamedNode(s) => Ok(s.into()),
                N3Term::BlankNode(s) => Ok(self.map_blank_node(s).into()),
                N3Term::Literal(_) => Err(RdfSyntaxError::msg(
                    "literals are not allowed in regular RDF subjects",
                )),
                #[cfg(feature = "rdf-star")]
                N3Term::Triple(s) => Ok(self.map_triple(*s).into()),
                N3Term::Variable(_) => Err(RdfSyntaxError::msg(
                    "variables are not allowed in regular RDF subjects",
                )),
            }?,
            predicate: match quad.predicate {
                N3Term::NamedNode(p) => Ok(p),
                N3Term::BlankNode(_) => Err(RdfSyntaxError::msg(
                    "blank nodes are not allowed in regular RDF predicates",
                )),
                N3Term::Literal(_) => Err(RdfSyntaxError::msg(
                    "literals are not allowed in regular RDF predicates",
                )),
                #[cfg(feature = "rdf-star")]
                N3Term::Triple(_) => Err(RdfSyntaxError::msg(
                    "quoted triples are not allowed in regular RDF predicates",
                )),
                N3Term::Variable(_) => Err(RdfSyntaxError::msg(
                    "variables are not allowed in regular RDF predicates",
                )),
            }?,
            object: match quad.object {
                N3Term::NamedNode(o) => Ok(o.into()),
                N3Term::BlankNode(o) => Ok(self.map_blank_node(o).into()),
                N3Term::Literal(o) => Ok(o.into()),
                #[cfg(feature = "rdf-star")]
                N3Term::Triple(o) => Ok(self.map_triple(*o).into()),
                N3Term::Variable(_) => Err(RdfSyntaxError::msg(
                    "variables are not allowed in regular RDF objects",
                )),
            }?,
            graph_name: self.map_graph_name(quad.graph_name)?,
        })
    }
}
