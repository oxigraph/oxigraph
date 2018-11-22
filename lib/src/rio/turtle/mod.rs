//! Implementation of [Turtle](https://www.w3.org/TR/turtle/) RDF syntax

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

    use model::*;
    use rio::utils::unescape_characters;
    use std::borrow::Cow;
    use std::collections::BTreeMap;
    use std::collections::HashMap;
    use std::io::BufReader;
    use std::io::Read;
    use url::ParseOptions;
    use url::Url;
    use utils::StaticSliceMap;

    include!(concat!(env!("OUT_DIR"), "/turtle_grammar.rs"));

    pub struct ParserState {
        base_uri: Option<Url>,
        namespaces: HashMap<String, Url>,
        cur_subject: Vec<NamedOrBlankNode>,
        cur_predicate: Vec<NamedNode>,
        bnodes_map: BTreeMap<String, BlankNode>,
    }

    impl ParserState {
        fn url_parser(&self) -> ParseOptions {
            Url::options().base_url(self.base_uri.as_ref())
        }
    }

    /// Reads a [Turtle](https://www.w3.org/TR/turtle/) file from a Rust `Read` and returns an iterator on the read `Triple`s
    ///
    /// Warning: this implementation has not been optimized yet and stores all the found triples in memory.
    /// This implementation also requires that blank node ids are valid UTF-8
    pub fn read_turtle<'a, R: Read + 'a>(
        source: R,
        base_uri: impl Into<Option<Url>>,
    ) -> super::super::super::Result<impl Iterator<Item = Triple>> {
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

        turtleDoc(&string_buffer, &mut state, &mut triple_buffer)?;
        Ok(triple_buffer.into_iter())
    }

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

    fn unescape_echars(input: &str) -> Cow<str> {
        unescape_characters(input, &UNESCAPE_CHARACTERS, &UNESCAPE_REPLACEMENT)
    }

    const UNESCAPE_PN_CHARACTERS: [u8; 20] = [
        b'_', b'~', b'.', b'-', b'!', b'$', b'&', b'\'', b'(', b')', b'*', b'+', b',', b';', b'=',
        b'/', b'?', b'#', b'@', b'%',
    ];
    lazy_static! {
        static ref UNESCAPE_PN_REPLACEMENT: StaticSliceMap<char, char> = StaticSliceMap::new(
            &[
                '_', '~', '.', '-', '!', '$', '&', '\'', '(', ')', '*', '+', ',', ';', '=', '/',
                '?', '#', '@', '%'
            ],
            &[
                '_', '~', '.', '-', '!', '$', '&', '\'', '(', ')', '*', '+', ',', ';', '=', '/',
                '?', '#', '@', '%'
            ]
        );
    }

    pub fn unescape_pn_local(input: &str) -> Cow<str> {
        unescape_characters(input, &UNESCAPE_PN_CHARACTERS, &UNESCAPE_PN_REPLACEMENT)
    }
}

pub use self::grammar::read_turtle;
