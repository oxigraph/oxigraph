//! Implementations of serializers and deserializers for usual RDF syntaxes

use crate::model::*;
use crate::Result;
use rio_api::model as rio;
use rio_api::parser::TripleParser;
use rio_turtle::{NTriplesParser, TurtleParser};
use rio_xml::RdfXmlParser;
use std::collections::BTreeMap;
use std::io::BufRead;

/// Reads a [N-Triples](https://www.w3.org/TR/n-triples/) file from a Rust `BufRead` and returns an iterator of the read `Triple`s
pub fn read_ntriples<R: BufRead>(reader: R) -> Result<impl Iterator<Item = Result<Triple>>> {
    let mut bnode_map = BTreeMap::default();
    Ok(NTriplesParser::new(reader)?.into_iter(move |t| convert_triple(t, &mut bnode_map)))
}

/// Reads a [Turtle](https://www.w3.org/TR/turtle/) file from a Rust `BufRead` and returns an iterator of the read `Triple`s
pub fn read_turtle<'a, R: BufRead + 'a>(
    reader: R,
    base_url: Option<&'a str>,
) -> Result<impl Iterator<Item = Result<Triple>> + 'a> {
    let mut bnode_map = BTreeMap::default();
    Ok(TurtleParser::new(reader, base_url.unwrap_or(""))?
        .into_iter(move |t| convert_triple(t, &mut bnode_map)))
}

/// Reads a [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/) file from a Rust `BufRead` and returns an iterator of the read `Triple`s
pub fn read_rdf_xml<'a, R: BufRead + 'a>(
    reader: R,
    base_url: Option<&'a str>,
) -> Result<impl Iterator<Item = Result<Triple>> + 'a> {
    let mut bnode_map = BTreeMap::default();
    Ok(RdfXmlParser::new(reader, base_url.unwrap_or(""))?
        .into_iter(move |t| convert_triple(t, &mut bnode_map)))
}

fn convert_triple(
    value: rio::Triple,
    bnodes_map: &mut BTreeMap<String, BlankNode>,
) -> Result<Triple> {
    let t = Triple::new(
        convert_named_or_blank_node(value.subject, bnodes_map)?,
        convert_named_node(value.predicate)?,
        convert_term(value.object, bnodes_map)?,
    );
    // println!("{}", t);
    Ok(t)
}

fn convert_term(value: rio::Term, bnodes_map: &mut BTreeMap<String, BlankNode>) -> Result<Term> {
    Ok(match value {
        rio::Term::NamedNode(v) => convert_named_node(v)?.into(),
        rio::Term::BlankNode(v) => convert_blank_node(v, bnodes_map).into(),
        rio::Term::Literal(v) => convert_literal(v)?.into(),
    })
}

fn convert_named_or_blank_node(
    value: rio::NamedOrBlankNode,
    bnodes_map: &mut BTreeMap<String, BlankNode>,
) -> Result<NamedOrBlankNode> {
    Ok(match value {
        rio::NamedOrBlankNode::NamedNode(v) => convert_named_node(v)?.into(),
        rio::NamedOrBlankNode::BlankNode(v) => convert_blank_node(v, bnodes_map).into(),
    })
}

fn convert_named_node(value: rio::NamedNode) -> Result<NamedNode> {
    Ok(NamedNode::new(value.iri))
}

fn convert_blank_node(
    value: rio::BlankNode,
    bnodes_map: &mut BTreeMap<String, BlankNode>,
) -> BlankNode {
    bnodes_map
        .entry(value.id.to_string())
        .or_insert_with(BlankNode::default)
        .clone()
}

fn convert_literal(value: rio::Literal) -> Result<Literal> {
    Ok(match value {
        rio::Literal::Simple { value } => Literal::new_simple_literal(value),
        rio::Literal::LanguageTaggedString { value, language } => {
            Literal::new_language_tagged_literal(value, LanguageTag::parse(language)?)
        }
        rio::Literal::Typed { value, datatype } => {
            Literal::new_typed_literal(value, convert_named_node(datatype)?)
        }
    })
}
