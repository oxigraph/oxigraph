///Implements https://www.w3.org/TR/n-triples/

mod grammar {
    include!(concat!(env!("OUT_DIR"), "/ntriples_grammar.rs"));
}

use model::data::*;
use rio::*;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use std::collections::BTreeMap;

pub fn read_ntriples<'a, R: Read + 'a>(source: R) -> impl Iterator<Item = RioResult<Triple>> {
    //TODO: use read_lines to avoid allocations
    let lines = BufReader::new(source).lines();
    let mut bnodes_map: BTreeMap<String, BlankNode> = BTreeMap::default();
    lines.flat_map(move |line| match line {
            Ok(line) => match grammar::triple(line.as_str(), &mut bnodes_map) {
                Ok(triple) => Some(Ok(triple?)),
                Err(error) => Some(Err(RioError::new(error))),
            },
            Err(error) => Some(Err(error.into())),
        })
}
