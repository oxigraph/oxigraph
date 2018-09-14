/// Implements https://www.w3.org/TR/turtle/

mod grammar {
    #![allow(unknown_lints)]
    #![allow(clippy)]

    use model::*;
    use std::collections::BTreeMap;
    use std::collections::HashMap;
    use std::io::BufReader;
    use std::io::Read;
    use url::ParseOptions;
    use url::Url;

    include!(concat!(env!("OUT_DIR"), "/turtle_grammar.rs"));

    pub struct ParserState {
        base_uri: Option<Url>,
        namespaces: HashMap<String, String>,
        cur_subject: Vec<NamedOrBlankNode>,
        cur_predicate: Vec<NamedNode>,
        bnodes_map: BTreeMap<String, BlankNode>,
    }

    impl ParserState {
        fn url_parser(&self) -> ParseOptions {
            Url::options().base_url(self.base_uri.as_ref())
        }
    }

    pub fn read_turtle<'a, R: Read + 'a>(
        source: R,
        base_uri: impl Into<Option<Url>>,
    ) -> super::super::super::errors::Result<impl Iterator<Item = Triple>> {
        let mut state = ParserState {
            base_uri: base_uri.into(),
            namespaces: HashMap::default(),
            cur_subject: Vec::default(),
            cur_predicate: Vec::default(),
            bnodes_map: BTreeMap::default(),
        };
        let mut triple_buffer = Vec::default();

        let mut string_buffer = String::default();
        BufReader::new(source).read_to_string(&mut string_buffer)?;

        match turtleDoc(&string_buffer, &mut state, &mut triple_buffer) {
            Ok(_) => Ok(triple_buffer.into_iter()),
            Err(error) => Err(error.into()),
        }
    }
}

pub(crate) type ParseError = self::grammar::ParseError;
pub use self::grammar::read_turtle;
