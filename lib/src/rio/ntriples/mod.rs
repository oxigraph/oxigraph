//! Implementation of [N-Triples](https://www.w3.org/TR/n-triples/) RDF syntax

mod grammar {
    #![cfg_attr(
        feature = "cargo-clippy",
        allow(
            suspicious_else_formatting,
            len_zero,
            single_match,
            unit_arg,
            naive_bytecount
        )
    )]

    use rio::utils::unescape_characters;
    use std::borrow::Cow;
    use utils::StaticSliceMap;

    const UNESCAPE_CHARACTERS: [u8; 8] = [b't', b'b', b'n', b'r', b'f', b'"', b'\'', b'\\'];
    lazy_static! {
        static ref UNESCAPE_REPLACEMENT: StaticSliceMap<char, char> = StaticSliceMap::new(
            &['t', 'b', 'n', 'r', 'f', '"', '\'', '\\'],
            &[
                '\u{0009}', '\u{0008}', '\u{000A}', '\u{000D}', '\u{000C}', '\u{0022}', '\u{0027}',
                '\u{005C}'
            ]
        );
    }

    pub fn unescape_echars(input: &str) -> Cow<str> {
        unescape_characters(input, &UNESCAPE_CHARACTERS, &UNESCAPE_REPLACEMENT)
    }

    include!(concat!(env!("OUT_DIR"), "/ntriples_grammar.rs"));
}

use model::*;
use std::collections::BTreeMap;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;
use Result;

struct NTriplesIterator<R: Read> {
    buffer: String,
    reader: BufReader<R>,
    bnodes_map: BTreeMap<String, BlankNode>,
}

impl<R: Read> Iterator for NTriplesIterator<R> {
    type Item = Result<Triple>;

    fn next(&mut self) -> Option<Result<Triple>> {
        if let Err(error) = self.reader.read_line(&mut self.buffer) {
            return Some(Err(error.into()));
        }
        if self.buffer.is_empty() {
            return None; //End of file
        }
        let result = grammar::triple(&self.buffer, &mut self.bnodes_map);
        self.buffer.clear();
        match result {
            Ok(Some(triple)) => Some(Ok(triple)),
            Ok(None) => self.next(),
            Err(error) => Some(Err(error.into())),
        }
    }
}

/// Reads a [N-Triples](https://www.w3.org/TR/n-triples/) file from a Rust `Read` and returns an iterator of the read `Triple`s
pub fn read_ntriples<'a, R: Read + 'a>(source: R) -> impl Iterator<Item = Result<Triple>> {
    NTriplesIterator {
        buffer: String::default(),
        reader: BufReader::new(source),
        bnodes_map: BTreeMap::default(),
    }
}
