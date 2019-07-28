//! Wrapper for RIO parsers

use crate::model::*;
use crate::Result;
use rio_api::model as rio;
use std::collections::BTreeMap;
use std::str::FromStr;

pub fn convert_triple(
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
    NamedNode::from_str(value.iri)
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
