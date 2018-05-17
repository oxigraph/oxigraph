///Implements https://www.w3.org/TR/n-triples/

mod grammar {
    include!(concat!(env!("OUT_DIR"), "/ntriples_grammar.rs"));
}

use model::data::*;
use rio::*;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Read;

pub fn read_ntriples<'a, R: Read + 'a>(
    source: R,
    data_factory: &'a DataFactory,
) -> impl Iterator<Item = RioResult<Triple>> {
    let factory = data_factory.clone(); //TODO: try to avoid clone here
                                        //TODO: use read_lines to avoid allocations
    BufReader::new(source)
        .lines()
        .flat_map(move |line| match line {
            Ok(line) => match grammar::triple(line.as_str(), &factory) {
                Ok(triple) => Some(Ok(triple?)),
                Err(error) => Some(Err(RioError::new(error))),
            },
            Err(error) => Some(Err(RioError::new(error))),
        })
}
