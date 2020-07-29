use super::GraphSyntax;
use crate::model::*;
use crate::DatasetSyntax;
use oxiri::{Iri, IriParseError};
use rio_api::model as rio;
use rio_api::parser::{QuadsParser, TriplesParser};
use rio_turtle::{NQuadsParser, NTriplesParser, TriGParser, TurtleParser};
use rio_xml::RdfXmlParser;
use std::collections::HashMap;
use std::io;
use std::io::BufRead;
use std::iter::once;

/// A reader for RDF graph serialization formats.
///
/// It currently supports the following formats:
/// * [N-Triples](https://www.w3.org/TR/n-triples/) (`GraphSyntax::NTriples`)
/// * [Turtle](https://www.w3.org/TR/turtle/) (`GraphSyntax::Turtle`)
/// * [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/) (`GraphSyntax::RdfXml`)
///
/// ```
/// use oxigraph::io::{GraphSyntax, GraphParser};
/// use std::io::Cursor;
///
/// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> .";
///
/// let parser = GraphParser::from_syntax(GraphSyntax::NTriples);
/// let triples = parser.read(Cursor::new(file)).collect::<Result<Vec<_>,_>>()?;
///
///assert_eq!(triples.len(), 1);
///assert_eq!(triples[0].subject.to_string(), "<http://example.com/s>");
/// # std::io::Result::Ok(())
/// ```
pub struct GraphParser {
    syntax: GraphSyntax,
    base_iri: String,
}

impl GraphParser {
    pub fn from_syntax(syntax: GraphSyntax) -> Self {
        Self {
            syntax,
            base_iri: String::new(),
        }
    }

    /// Provides an IRI that could be used to resolve the file relative IRIs
    ///
    /// ```
    /// use oxigraph::io::{GraphSyntax, GraphParser};
    /// use std::io::Cursor;
    ///
    /// let file = "</s> </p> </o> .";
    ///
    /// let parser = GraphParser::from_syntax(GraphSyntax::Turtle).with_base_iri("http://example.com")?;
    /// let triples = parser.read(Cursor::new(file)).collect::<Result<Vec<_>,_>>()?;
    ///
    ///assert_eq!(triples.len(), 1);
    ///assert_eq!(triples[0].subject.to_string(), "<http://example.com/s>");
    /// # oxigraph::Result::Ok(())
    /// ```
    pub fn with_base_iri(mut self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        self.base_iri = Iri::parse(base_iri.into())?.into_inner();
        Ok(self)
    }

    /// Executes the parsing itself
    pub fn read<'a>(
        &self,
        reader: impl BufRead + 'a,
    ) -> impl Iterator<Item = Result<Triple, io::Error>> + 'a {
        match self.parse(reader) {
            Ok(iter) => iter,
            Err(error) => Box::new(once(Err(error))),
        }
    }

    fn parse<'a>(
        &self,
        reader: impl BufRead + 'a,
    ) -> Result<Box<dyn Iterator<Item = Result<Triple, io::Error>> + 'a>, io::Error> {
        Ok(match self.syntax {
            GraphSyntax::NTriples => {
                Box::new(self.parse_from_triple_parser(NTriplesParser::new(reader))?)
            }
            GraphSyntax::Turtle => {
                Box::new(self.parse_from_triple_parser(TurtleParser::new(reader, &self.base_iri))?)
            }
            GraphSyntax::RdfXml => {
                Box::new(self.parse_from_triple_parser(RdfXmlParser::new(reader, &self.base_iri))?)
            }
        })
    }

    fn parse_from_triple_parser<P: TriplesParser>(
        &self,
        parser: Result<P, P::Error>,
    ) -> Result<impl Iterator<Item = Result<Triple, io::Error>>, io::Error>
    where
        P::Error: Send + Sync + 'static,
    {
        let parser = parser.map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let mut mapper = RioMapper::default();
        Ok(parser
            .into_iter(move |t| Ok(mapper.triple(&t)))
            .map(|e: Result<_, P::Error>| {
                e.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
            }))
    }
}

/// A reader for RDF dataset serialization formats.
///
/// It currently supports the following formats:
/// * [N-Quads](https://www.w3.org/TR/n-quads/) (`DatasetSyntax::NQuads`)
/// * [TriG](https://www.w3.org/TR/trig/) (`DatasetSyntax::TriG`)
///
/// ```
/// use oxigraph::io::{DatasetSyntax, DatasetParser};
/// use std::io::Cursor;
///
/// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .";
///
/// let parser = DatasetParser::from_syntax(DatasetSyntax::NQuads);
/// let quads = parser.read(Cursor::new(file)).collect::<Result<Vec<_>,_>>()?;
///
///assert_eq!(quads.len(), 1);
///assert_eq!(quads[0].subject.to_string(), "<http://example.com/s>");
/// # std::io::Result::Ok(())
/// ```
pub struct DatasetParser {
    syntax: DatasetSyntax,
    base_iri: String,
}

impl DatasetParser {
    pub fn from_syntax(syntax: DatasetSyntax) -> Self {
        Self {
            syntax,
            base_iri: String::new(),
        }
    }

    /// Provides an IRI that could be used to resolve the file relative IRIs
    ///
    /// ```
    /// use oxigraph::io::{DatasetSyntax, DatasetParser};
    /// use std::io::Cursor;
    ///
    /// let file = "<g> { </s> </p> </o> }";
    ///
    /// let parser = DatasetParser::from_syntax(DatasetSyntax::TriG).with_base_iri("http://example.com")?;
    /// let triples = parser.read(Cursor::new(file)).collect::<Result<Vec<_>,_>>()?;
    ///
    ///assert_eq!(triples.len(), 1);
    ///assert_eq!(triples[0].subject.to_string(), "<http://example.com/s>");
    /// # oxigraph::Result::Ok(())
    /// ```
    pub fn with_base_iri(mut self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        self.base_iri = Iri::parse(base_iri.into())?.into_inner();
        Ok(self)
    }

    /// Executes the parsing itself
    pub fn read<'a>(
        &self,
        reader: impl BufRead + 'a,
    ) -> impl Iterator<Item = Result<Quad, io::Error>> + 'a {
        match self.parse(reader) {
            Ok(iter) => iter,
            Err(error) => Box::new(once(Err(error))),
        }
    }

    fn parse<'a>(
        &self,
        reader: impl BufRead + 'a,
    ) -> Result<Box<dyn Iterator<Item = Result<Quad, io::Error>> + 'a>, io::Error> {
        Ok(match self.syntax {
            DatasetSyntax::NQuads => {
                Box::new(self.parse_from_quad_parser(NQuadsParser::new(reader))?)
            }
            DatasetSyntax::TriG => {
                Box::new(self.parse_from_quad_parser(TriGParser::new(reader, &self.base_iri))?)
            }
        })
    }

    fn parse_from_quad_parser<P: QuadsParser>(
        &self,
        parser: Result<P, P::Error>,
    ) -> Result<impl Iterator<Item = Result<Quad, io::Error>>, io::Error>
    where
        P::Error: Send + Sync + 'static,
    {
        let parser = parser.map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        let mut mapper = RioMapper::default();
        Ok(parser
            .into_iter(move |q| Ok(mapper.quad(&q)))
            .map(|e: Result<_, P::Error>| {
                e.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
            }))
    }
}

#[derive(Default)]
struct RioMapper {
    bnode_map: HashMap<String, BlankNode>,
}

impl<'a> RioMapper {
    fn named_node(&self, node: rio::NamedNode<'a>) -> NamedNode {
        NamedNode::new_unchecked(node.iri)
    }

    fn blank_node(&mut self, node: rio::BlankNode<'a>) -> BlankNode {
        self.bnode_map
            .entry(node.id.to_owned())
            .or_insert_with(BlankNode::default)
            .clone()
    }

    fn literal(&self, literal: rio::Literal<'a>) -> Literal {
        match literal {
            rio::Literal::Simple { value } => Literal::new_simple_literal(value),
            rio::Literal::LanguageTaggedString { value, language } => {
                Literal::new_language_tagged_literal_unchecked(value, language)
            }
            rio::Literal::Typed { value, datatype } => {
                Literal::new_typed_literal(value, self.named_node(datatype))
            }
        }
    }

    fn named_or_blank_node(&mut self, node: rio::NamedOrBlankNode<'a>) -> NamedOrBlankNode {
        match node {
            rio::NamedOrBlankNode::NamedNode(node) => self.named_node(node).into(),
            rio::NamedOrBlankNode::BlankNode(node) => self.blank_node(node).into(),
        }
    }

    fn term(&mut self, node: rio::Term<'a>) -> Term {
        match node {
            rio::Term::NamedNode(node) => self.named_node(node).into(),
            rio::Term::BlankNode(node) => self.blank_node(node).into(),
            rio::Term::Literal(literal) => self.literal(literal).into(),
        }
    }

    fn triple(&mut self, triple: &rio::Triple<'a>) -> Triple {
        Triple {
            subject: self.named_or_blank_node(triple.subject),
            predicate: self.named_node(triple.predicate),
            object: self.term(triple.object),
        }
    }

    fn graph_name(&mut self, graph_name: Option<rio::NamedOrBlankNode<'a>>) -> GraphName {
        match graph_name {
            Some(rio::NamedOrBlankNode::NamedNode(node)) => self.named_node(node).into(),
            Some(rio::NamedOrBlankNode::BlankNode(node)) => self.blank_node(node).into(),
            None => GraphName::DefaultGraph,
        }
    }

    fn quad(&mut self, quad: &rio::Quad<'a>) -> Quad {
        Quad {
            subject: self.named_or_blank_node(quad.subject),
            predicate: self.named_node(quad.predicate),
            object: self.term(quad.object),
            graph_name: self.graph_name(quad.graph_name),
        }
    }
}
