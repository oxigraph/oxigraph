/// Implements https://www.w3.org/TR/turtle/

mod grammar {
    include!(concat!(env!("OUT_DIR"), "/turtle_grammar.rs"));
}

use model::data::*;
use rio::*;
use std::collections::HashMap;
use std::io::BufReader;
use std::io::Read;
use url::Url;
use url::ParseOptions;

//TODO: make private
pub struct ParserState {
    pub base_uri: Option<Url>,
    pub namespaces: HashMap<String, String>,
    pub cur_subject: Vec<NamedOrBlankNode>,
    pub cur_predicate: Vec<NamedNode>,
}

impl ParserState {
    fn url_parser<'a>(&'a self) -> ParseOptions<'a> {
        Url::options().base_url(self.base_uri.as_ref())
    }
}

pub fn read_turtle<'a, R: Read + 'a>(
    source: R,
    data_factory: &'a DataFactory,
    base_uri: impl Into<Option<Url>>
) -> RioResult<impl Iterator<Item = Triple>> {
    let factory = data_factory.clone(); //TODO: try to avoid clone here
    let mut state = ParserState {
        base_uri: base_uri.into(),
        namespaces: HashMap::default(),
        cur_subject: Vec::default(),
        cur_predicate: Vec::default(),
    };
    let mut string_buffer = String::default();
    let mut triple_buffer = Vec::default();
    match BufReader::new(source).read_to_string(&mut string_buffer) {
        Ok(_) => match grammar::turtleDoc(&string_buffer, &mut state, &mut triple_buffer, &factory)
        {
            Ok(_) => Ok(triple_buffer.into_iter()),
            Err(error) => Err(RioError::new(error)),
        },
        Err(error) => Err(RioError::new(error)),
    }
}
