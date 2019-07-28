//! Implementation of [N-Triples](https://www.w3.org/TR/n-triples/) RDF syntax

use crate::model::Triple;
use crate::rio::rio::convert_triple;
use crate::Result;
use rio_api::parser::TripleParser;
use rio_turtle::NTriplesParser;
use std::collections::BTreeMap;
use std::io::BufRead;

/// Reads a [N-Triples](https://www.w3.org/TR/n-triples/) file from a Rust `BufRead` and returns an iterator of the read `Triple`s
pub fn read_ntriples<R: BufRead>(reader: R) -> Result<impl Iterator<Item = Result<Triple>>> {
    let mut bnode_map = BTreeMap::default();
    Ok(NTriplesParser::new(reader)?.into_iter(move |t| convert_triple(t, &mut bnode_map)))
}
