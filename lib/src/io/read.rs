//! Utilities to read RDF graphs and datasets.

pub use crate::io::error::{ParseError, SyntaxError};
use crate::io::{DatasetFormat, GraphFormat};
use crate::model::*;
use oxiri::IriParseError;
use oxrdfxml::{FromReadRdfXmlReader, RdfXmlParser};
use oxttl::nquads::{FromReadNQuadsReader, NQuadsParser};
use oxttl::ntriples::{FromReadNTriplesReader, NTriplesParser};
use oxttl::trig::{FromReadTriGReader, TriGParser};
use oxttl::turtle::{FromReadTurtleReader, TurtleParser};
use std::collections::HashMap;
use std::io::Read;

/// Parsers for RDF graph serialization formats.
///
/// It currently supports the following formats:
/// * [N-Triples](https://www.w3.org/TR/n-triples/) ([`GraphFormat::NTriples`])
/// * [Turtle](https://www.w3.org/TR/turtle/) ([`GraphFormat::Turtle`])
/// * [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/) ([`GraphFormat::RdfXml`])
///
/// ```
/// use oxigraph::io::{GraphFormat, GraphParser};
///
/// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> .";
///
/// let parser = GraphParser::from_format(GraphFormat::NTriples);
/// let triples = parser.read_triples(file.as_bytes()).collect::<Result<Vec<_>,_>>()?;
///
///assert_eq!(triples.len(), 1);
///assert_eq!(triples[0].subject.to_string(), "<http://example.com/s>");
/// # std::io::Result::Ok(())
/// ```
pub struct GraphParser {
    inner: GraphParserKind,
}

enum GraphParserKind {
    NTriples(NTriplesParser),
    Turtle(TurtleParser),
    RdfXml(RdfXmlParser),
}

impl GraphParser {
    /// Builds a parser for the given format.
    #[inline]
    pub fn from_format(format: GraphFormat) -> Self {
        Self {
            inner: match format {
                GraphFormat::NTriples => {
                    GraphParserKind::NTriples(NTriplesParser::new().with_quoted_triples())
                }
                GraphFormat::Turtle => {
                    GraphParserKind::Turtle(TurtleParser::new().with_quoted_triples())
                }
                GraphFormat::RdfXml => GraphParserKind::RdfXml(RdfXmlParser::new()),
            },
        }
    }

    /// Provides an IRI that could be used to resolve the file relative IRIs.
    ///
    /// ```
    /// use oxigraph::io::{GraphFormat, GraphParser};
    ///
    /// let file = "</s> </p> </o> .";
    ///
    /// let parser = GraphParser::from_format(GraphFormat::Turtle).with_base_iri("http://example.com")?;
    /// let triples = parser.read_triples(file.as_bytes()).collect::<Result<Vec<_>,_>>()?;
    ///
    ///assert_eq!(triples.len(), 1);
    ///assert_eq!(triples[0].subject.to_string(), "<http://example.com/s>");
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_base_iri(self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        Ok(Self {
            inner: match self.inner {
                GraphParserKind::NTriples(p) => GraphParserKind::NTriples(p),
                GraphParserKind::Turtle(p) => GraphParserKind::Turtle(p.with_base_iri(base_iri)?),
                GraphParserKind::RdfXml(p) => GraphParserKind::RdfXml(p.with_base_iri(base_iri)?),
            },
        })
    }

    /// Executes the parsing itself on a [`Read`] implementation and returns an iterator of triples.
    pub fn read_triples<R: Read>(&self, reader: R) -> TripleReader<R> {
        TripleReader {
            mapper: BlankNodeMapper::default(),
            parser: match &self.inner {
                GraphParserKind::NTriples(p) => TripleReaderKind::NTriples(p.parse_read(reader)),
                GraphParserKind::Turtle(p) => TripleReaderKind::Turtle(p.parse_read(reader)),
                GraphParserKind::RdfXml(p) => TripleReaderKind::RdfXml(p.parse_read(reader)),
            },
        }
    }
}

/// An iterator yielding read triples.
/// Could be built using a [`GraphParser`].
///
/// ```
/// use oxigraph::io::{GraphFormat, GraphParser};
///
/// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> .";
///
/// let parser = GraphParser::from_format(GraphFormat::NTriples);
/// let triples = parser.read_triples(file.as_bytes()).collect::<Result<Vec<_>,_>>()?;
///
///assert_eq!(triples.len(), 1);
///assert_eq!(triples[0].subject.to_string(), "<http://example.com/s>");
/// # std::io::Result::Ok(())
/// ```
#[must_use]
pub struct TripleReader<R: Read> {
    mapper: BlankNodeMapper,
    parser: TripleReaderKind<R>,
}

#[allow(clippy::large_enum_variant)]
enum TripleReaderKind<R: Read> {
    NTriples(FromReadNTriplesReader<R>),
    Turtle(FromReadTurtleReader<R>),
    RdfXml(FromReadRdfXmlReader<R>),
}

impl<R: Read> Iterator for TripleReader<R> {
    type Item = Result<Triple, ParseError>;

    fn next(&mut self) -> Option<Result<Triple, ParseError>> {
        Some(match &mut self.parser {
            TripleReaderKind::NTriples(parser) => match parser.next()? {
                Ok(triple) => Ok(self.mapper.triple(triple)),
                Err(e) => Err(e.into()),
            },
            TripleReaderKind::Turtle(parser) => match parser.next()? {
                Ok(triple) => Ok(self.mapper.triple(triple)),
                Err(e) => Err(e.into()),
            },
            TripleReaderKind::RdfXml(parser) => match parser.next()? {
                Ok(triple) => Ok(self.mapper.triple(triple)),
                Err(e) => Err(e.into()),
            },
        })
    }
}

/// A parser for RDF dataset serialization formats.
///
/// It currently supports the following formats:
/// * [N-Quads](https://www.w3.org/TR/n-quads/) ([`DatasetFormat::NQuads`])
/// * [TriG](https://www.w3.org/TR/trig/) ([`DatasetFormat::TriG`])
///
/// ```
/// use oxigraph::io::{DatasetFormat, DatasetParser};
///
/// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .";
///
/// let parser = DatasetParser::from_format(DatasetFormat::NQuads);
/// let quads = parser.read_quads(file.as_bytes()).collect::<Result<Vec<_>,_>>()?;
///
///assert_eq!(quads.len(), 1);
///assert_eq!(quads[0].subject.to_string(), "<http://example.com/s>");
/// # std::io::Result::Ok(())
/// ```
pub struct DatasetParser {
    inner: DatasetParserKind,
}

enum DatasetParserKind {
    NQuads(NQuadsParser),
    TriG(TriGParser),
}

impl DatasetParser {
    /// Builds a parser for the given format.
    #[inline]
    pub fn from_format(format: DatasetFormat) -> Self {
        Self {
            inner: match format {
                DatasetFormat::NQuads => {
                    DatasetParserKind::NQuads(NQuadsParser::new().with_quoted_triples())
                }
                DatasetFormat::TriG => {
                    DatasetParserKind::TriG(TriGParser::new().with_quoted_triples())
                }
            },
        }
    }

    /// Provides an IRI that could be used to resolve the file relative IRIs.
    ///
    /// ```
    /// use oxigraph::io::{DatasetFormat, DatasetParser};
    ///
    /// let file = "<g> { </s> </p> </o> }";
    ///
    /// let parser = DatasetParser::from_format(DatasetFormat::TriG).with_base_iri("http://example.com")?;
    /// let triples = parser.read_quads(file.as_bytes()).collect::<Result<Vec<_>,_>>()?;
    ///
    ///assert_eq!(triples.len(), 1);
    ///assert_eq!(triples[0].subject.to_string(), "<http://example.com/s>");
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_base_iri(self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        Ok(Self {
            inner: match self.inner {
                DatasetParserKind::NQuads(p) => DatasetParserKind::NQuads(p),
                DatasetParserKind::TriG(p) => DatasetParserKind::TriG(p.with_base_iri(base_iri)?),
            },
        })
    }

    /// Executes the parsing itself on a [`Read`] implementation and returns an iterator of quads.
    pub fn read_quads<R: Read>(&self, reader: R) -> QuadReader<R> {
        QuadReader {
            mapper: BlankNodeMapper::default(),
            parser: match &self.inner {
                DatasetParserKind::NQuads(p) => QuadReaderKind::NQuads(p.parse_read(reader)),
                DatasetParserKind::TriG(p) => QuadReaderKind::TriG(p.parse_read(reader)),
            },
        }
    }
}

/// An iterator yielding read quads.
/// Could be built using a [`DatasetParser`].
///
/// ```
/// use oxigraph::io::{DatasetFormat, DatasetParser};
///
/// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .";
///
/// let parser = DatasetParser::from_format(DatasetFormat::NQuads);
/// let quads = parser.read_quads(file.as_bytes()).collect::<Result<Vec<_>,_>>()?;
///
///assert_eq!(quads.len(), 1);
///assert_eq!(quads[0].subject.to_string(), "<http://example.com/s>");
/// # std::io::Result::Ok(())
/// ```
#[must_use]
pub struct QuadReader<R: Read> {
    mapper: BlankNodeMapper,
    parser: QuadReaderKind<R>,
}

enum QuadReaderKind<R: Read> {
    NQuads(FromReadNQuadsReader<R>),
    TriG(FromReadTriGReader<R>),
}

impl<R: Read> Iterator for QuadReader<R> {
    type Item = Result<Quad, ParseError>;

    fn next(&mut self) -> Option<Result<Quad, ParseError>> {
        Some(match &mut self.parser {
            QuadReaderKind::NQuads(parser) => match parser.next()? {
                Ok(quad) => Ok(self.mapper.quad(quad)),
                Err(e) => Err(e.into()),
            },
            QuadReaderKind::TriG(parser) => match parser.next()? {
                Ok(quad) => Ok(self.mapper.quad(quad)),
                Err(e) => Err(e.into()),
            },
        })
    }
}

#[derive(Default)]
struct BlankNodeMapper {
    bnode_map: HashMap<BlankNode, BlankNode>,
}

impl BlankNodeMapper {
    fn blank_node(&mut self, node: BlankNode) -> BlankNode {
        self.bnode_map
            .entry(node)
            .or_insert_with(BlankNode::default)
            .clone()
    }

    fn subject(&mut self, node: Subject) -> Subject {
        match node {
            Subject::NamedNode(node) => node.into(),
            Subject::BlankNode(node) => self.blank_node(node).into(),
            Subject::Triple(triple) => self.triple(*triple).into(),
        }
    }

    fn term(&mut self, node: Term) -> Term {
        match node {
            Term::NamedNode(node) => node.into(),
            Term::BlankNode(node) => self.blank_node(node).into(),
            Term::Literal(literal) => literal.into(),
            Term::Triple(triple) => self.triple(*triple).into(),
        }
    }

    fn triple(&mut self, triple: Triple) -> Triple {
        Triple {
            subject: self.subject(triple.subject),
            predicate: triple.predicate,
            object: self.term(triple.object),
        }
    }

    fn graph_name(&mut self, graph_name: GraphName) -> GraphName {
        match graph_name {
            GraphName::NamedNode(node) => node.into(),
            GraphName::BlankNode(node) => self.blank_node(node).into(),
            GraphName::DefaultGraph => GraphName::DefaultGraph,
        }
    }

    fn quad(&mut self, quad: Quad) -> Quad {
        Quad {
            subject: self.subject(quad.subject),
            predicate: quad.predicate,
            object: self.term(quad.object),
            graph_name: self.graph_name(quad.graph_name),
        }
    }
}
