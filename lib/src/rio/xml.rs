//! Implementation of [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/) syntax

use crate::model::Triple;
use crate::rio::rio::convert_triple;
use crate::Result;
use rio_api::parser::TripleParser;
use rio_xml::RdfXmlParser;
use std::collections::BTreeMap;
use std::io::BufRead;
use url::Url;

/// Reads a [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/)  file from a Rust `BufRead` and returns an iterator of the read `Triple`s
pub fn read_rdf_xml<R: BufRead>(
    reader: R,
    base_url: Option<Url>,
) -> Result<impl Iterator<Item = Result<Triple>>> {
    let mut bnode_map = BTreeMap::default();
    Ok(
        RdfXmlParser::new(reader, base_url.as_ref().map_or("", |url| url.as_str()))?
            .into_iter(move |t| convert_triple(t, &mut bnode_map)),
    )
}
