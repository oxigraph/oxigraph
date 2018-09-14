///Implements https://www.w3.org/TR/n-triples/

mod grammar {
    #![allow(unknown_lints)]
    #![allow(clippy)]
    include!(concat!(env!("OUT_DIR"), "/ntriples_grammar.rs"));
}

use errors::*;
use model::*;
use std::collections::BTreeMap;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;

pub(crate) type ParseError = self::grammar::ParseError;

struct NTriplesIterator<R: Read> {
    buffer: String,
    reader: BufReader<R>,
    bnodes_map: BTreeMap<String, BlankNode>,
}

impl<R: Read> Iterator for NTriplesIterator<R> {
    type Item = Result<Triple>;

    fn next(&mut self) -> Option<Result<Triple>> {
        match self.reader.read_line(&mut self.buffer) {
            Ok(line_count) => if line_count == 0 {
                None
            } else {
                let result = grammar::triple(&self.buffer, &mut self.bnodes_map);
                self.buffer.clear();
                match result {
                    Ok(Some(triple)) => Some(Ok(triple)),
                    Ok(None) => self.next(),
                    Err(error) => Some(Err(error.into())),
                }
            },
            Err(error) => Some(Err(error.into())),
        }
    }
}

pub fn read_ntriples<'a, R: Read + 'a>(source: R) -> impl Iterator<Item = Result<Triple>> {
    NTriplesIterator {
        buffer: String::default(),
        reader: BufReader::new(source),
        bnodes_map: BTreeMap::default(),
    }
}
