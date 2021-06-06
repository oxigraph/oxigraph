//! Utilities for I/O from the store

use crate::error::invalid_input_error;
use crate::io::{DatasetFormat, DatasetSerializer, GraphFormat, GraphSerializer};
use crate::model::{BlankNode, GraphNameRef, LiteralRef, NamedNodeRef, Quad, QuadRef, Triple};
use crate::storage::StorageLike;
use oxiri::Iri;
use rio_api::model as rio;
use rio_api::parser::{QuadsParser, TriplesParser};
use rio_turtle::{NQuadsParser, NTriplesParser, TriGParser, TurtleError, TurtleParser};
use rio_xml::{RdfXmlError, RdfXmlParser};
use std::collections::HashMap;
use std::io;
use std::io::{BufRead, Write};

pub(crate) fn load_graph<S: StorageLike>(
    storage: &S,
    reader: impl BufRead,
    format: GraphFormat,
    to_graph_name: GraphNameRef<'_>,
    base_iri: Option<&str>,
) -> Result<(), StoreOrParseError<S::Error>> {
    let base_iri = if let Some(base_iri) = base_iri {
        Some(Iri::parse(base_iri.into()).map_err(invalid_input_error)?)
    } else {
        None
    };
    match format {
        GraphFormat::NTriples => {
            load_from_triple_parser(storage, NTriplesParser::new(reader), to_graph_name)
        }
        GraphFormat::Turtle => {
            load_from_triple_parser(storage, TurtleParser::new(reader, base_iri), to_graph_name)
        }
        GraphFormat::RdfXml => {
            load_from_triple_parser(storage, RdfXmlParser::new(reader, base_iri), to_graph_name)
        }
    }
}

fn load_from_triple_parser<S: StorageLike, P: TriplesParser>(
    storage: &S,
    mut parser: P,
    to_graph_name: GraphNameRef<'_>,
) -> Result<(), StoreOrParseError<S::Error>>
where
    StoreOrParseError<S::Error>: From<P::Error>,
{
    let mut bnode_map = HashMap::default();
    parser.parse_all(&mut move |t| {
        storage
            .insert(quad_from_rio_triple(&t, to_graph_name, &mut bnode_map))
            .map_err(StoreOrParseError::Store)?;
        Ok(())
    })
}

fn quad_from_rio_triple<'a>(
    triple: &rio::Triple<'a>,
    graph_name: GraphNameRef<'a>,
    bnode_map: &'a mut HashMap<String, BlankNode>,
) -> QuadRef<'a> {
    // we insert the blank nodes
    if let rio::NamedOrBlankNode::BlankNode(node) = triple.subject {
        bnode_map.entry(node.id.to_owned()).or_default();
    }
    if let rio::Term::BlankNode(node) = triple.object {
        bnode_map.entry(node.id.to_owned()).or_default();
    }
    QuadRef {
        subject: match triple.subject {
            rio::NamedOrBlankNode::NamedNode(node) => NamedNodeRef::new_unchecked(node.iri).into(),
            rio::NamedOrBlankNode::BlankNode(node) => bnode_map[node.id].as_ref().into(),
        },
        predicate: NamedNodeRef::new_unchecked(triple.predicate.iri),
        object: match triple.object {
            rio::Term::NamedNode(node) => NamedNodeRef::new_unchecked(node.iri).into(),
            rio::Term::BlankNode(node) => bnode_map[node.id].as_ref().into(),
            rio::Term::Literal(literal) => match literal {
                rio::Literal::Simple { value } => LiteralRef::new_simple_literal(value),
                rio::Literal::LanguageTaggedString { value, language } => {
                    LiteralRef::new_language_tagged_literal_unchecked(value, language)
                }
                rio::Literal::Typed { value, datatype } => {
                    LiteralRef::new_typed_literal(value, NamedNodeRef::new_unchecked(datatype.iri))
                }
            }
            .into(),
        },
        graph_name,
    }
}

pub fn dump_graph(
    triples: impl Iterator<Item = io::Result<Triple>>,
    writer: impl Write,
    format: GraphFormat,
) -> io::Result<()> {
    let mut writer = GraphSerializer::from_format(format).triple_writer(writer)?;
    for triple in triples {
        writer.write(&triple?)?;
    }
    writer.finish()
}

pub(crate) fn load_dataset<S: StorageLike>(
    store: &S,
    reader: impl BufRead,
    format: DatasetFormat,
    base_iri: Option<&str>,
) -> Result<(), StoreOrParseError<S::Error>> {
    let base_iri = if let Some(base_iri) = base_iri {
        Some(Iri::parse(base_iri.into()).map_err(invalid_input_error)?)
    } else {
        None
    };
    match format {
        DatasetFormat::NQuads => load_from_quad_parser(store, NQuadsParser::new(reader)),
        DatasetFormat::TriG => load_from_quad_parser(store, TriGParser::new(reader, base_iri)),
    }
}

fn load_from_quad_parser<S: StorageLike, P: QuadsParser>(
    store: &S,
    mut parser: P,
) -> Result<(), StoreOrParseError<S::Error>>
where
    StoreOrParseError<S::Error>: From<P::Error>,
{
    let mut bnode_map = HashMap::default();
    parser.parse_all(&mut move |q| {
        store
            .insert(quad_from_rio(&q, &mut bnode_map))
            .map_err(StoreOrParseError::Store)?;
        Ok(())
    })
}

fn quad_from_rio<'a>(
    quad: &rio::Quad<'a>,
    bnode_map: &'a mut HashMap<String, BlankNode>,
) -> QuadRef<'a> {
    // we insert the blank nodes
    if let rio::NamedOrBlankNode::BlankNode(node) = quad.subject {
        bnode_map.entry(node.id.to_owned()).or_default();
    }
    if let rio::Term::BlankNode(node) = quad.object {
        bnode_map.entry(node.id.to_owned()).or_default();
    }
    if let Some(rio::NamedOrBlankNode::BlankNode(node)) = quad.graph_name {
        bnode_map.entry(node.id.to_owned()).or_default();
    }
    QuadRef {
        subject: match quad.subject {
            rio::NamedOrBlankNode::NamedNode(node) => NamedNodeRef::new_unchecked(node.iri).into(),
            rio::NamedOrBlankNode::BlankNode(node) => bnode_map[node.id].as_ref().into(),
        },
        predicate: NamedNodeRef::new_unchecked(quad.predicate.iri),
        object: match quad.object {
            rio::Term::NamedNode(node) => NamedNodeRef::new_unchecked(node.iri).into(),
            rio::Term::BlankNode(node) => bnode_map[node.id].as_ref().into(),
            rio::Term::Literal(literal) => match literal {
                rio::Literal::Simple { value } => LiteralRef::new_simple_literal(value),
                rio::Literal::LanguageTaggedString { value, language } => {
                    LiteralRef::new_language_tagged_literal_unchecked(value, language)
                }
                rio::Literal::Typed { value, datatype } => {
                    LiteralRef::new_typed_literal(value, NamedNodeRef::new_unchecked(datatype.iri))
                }
            }
            .into(),
        },
        graph_name: match quad.graph_name {
            Some(rio::NamedOrBlankNode::NamedNode(node)) => {
                NamedNodeRef::new_unchecked(node.iri).into()
            }
            Some(rio::NamedOrBlankNode::BlankNode(node)) => bnode_map[node.id].as_ref().into(),
            None => GraphNameRef::DefaultGraph,
        },
    }
}

pub fn dump_dataset(
    quads: impl Iterator<Item = io::Result<Quad>>,
    writer: impl Write,
    format: DatasetFormat,
) -> io::Result<()> {
    let mut writer = DatasetSerializer::from_format(format).quad_writer(writer)?;
    for quad in quads {
        writer.write(&quad?)?;
    }
    writer.finish()
}

pub(crate) enum StoreOrParseError<S> {
    Store(S),
    Parse(io::Error),
}

impl<S> From<TurtleError> for StoreOrParseError<S> {
    fn from(error: TurtleError) -> Self {
        Self::Parse(error.into())
    }
}

impl<S> From<RdfXmlError> for StoreOrParseError<S> {
    fn from(error: RdfXmlError) -> Self {
        Self::Parse(error.into())
    }
}

impl<S> From<io::Error> for StoreOrParseError<S> {
    fn from(error: io::Error) -> Self {
        Self::Parse(error)
    }
}

impl From<StoreOrParseError<io::Error>> for io::Error {
    fn from(error: StoreOrParseError<io::Error>) -> Self {
        match error {
            StoreOrParseError::Store(error) => error,
            StoreOrParseError::Parse(error) => error,
        }
    }
}
