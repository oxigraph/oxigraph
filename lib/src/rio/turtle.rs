//! Implementation of [Turtle](https://www.w3.org/TR/turtle/) RDF syntax

use crate::model::Triple;
use crate::rio::rio::convert_triple;
use crate::Result;
use rio_api::parser::TripleParser;
use rio_turtle::TurtleParser;
use std::collections::BTreeMap;
use std::io::BufRead;
use url::Url;

/// Reads a [Turtle](https://www.w3.org/TR/turtle/) file from a Rust `BufRead` and returns an iterator of the read `Triple`s
pub fn read_turtle<R: BufRead>(
    reader: R,
    base_url: Option<Url>,
) -> Result<impl Iterator<Item = Result<Triple>>> {
    let mut bnode_map = BTreeMap::default();
    Ok(
        TurtleParser::new(reader, base_url.as_ref().map_or("", |url| url.as_str()))?
            .into_iter(move |t| convert_triple(t, &mut bnode_map)),
    )
}
