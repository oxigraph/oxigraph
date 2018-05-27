use std::borrow::Cow;
use std::char;
use std::str::Chars;

mod grammar {
    use model::data::*;
    use rio::RioError;
    use rio::RioResult;
    use sparql::ast::*;
    use sparql::model::*;
    use sparql::parser::unescape_unicode_codepoints;
    use std::borrow::Cow;
    use std::collections::BTreeMap;
    use std::collections::HashMap;
    use std::io::BufReader;
    use std::io::Read;
    use url::ParseOptions;
    use url::Url;

    struct FocusedTriplePattern<F> {
        focus: F,
        patterns: Vec<TriplePattern>,
    }

    impl<F> FocusedTriplePattern<F> {
        fn new(focus: F) -> Self {
            Self {
                focus,
                patterns: Vec::default(),
            }
        }
    }

    impl<F: Default> Default for FocusedTriplePattern<F> {
        fn default() -> Self {
            Self {
                focus: F::default(),
                patterns: Vec::default(),
            }
        }
    }

    impl<F> From<FocusedTriplePattern<F>> for FocusedTriplePattern<Vec<F>> {
        fn from(input: FocusedTriplePattern<F>) -> Self {
            Self {
                focus: vec![input.focus],
                patterns: input.patterns,
            }
        }
    }

    struct FocusedPropertyPathPattern<F> {
        focus: F,
        patterns: Vec<PropertyPathPattern>,
    }

    impl<F> FocusedPropertyPathPattern<F> {
        fn new(focus: F) -> Self {
            Self {
                focus,
                patterns: Vec::default(),
            }
        }
    }

    impl<F: Default> Default for FocusedPropertyPathPattern<F> {
        fn default() -> Self {
            Self {
                focus: F::default(),
                patterns: Vec::default(),
            }
        }
    }

    impl<F> From<FocusedPropertyPathPattern<F>> for FocusedPropertyPathPattern<Vec<F>> {
        fn from(input: FocusedPropertyPathPattern<F>) -> Self {
            Self {
                focus: vec![input.focus],
                patterns: input.patterns,
            }
        }
    }

    impl<F, T: From<F>> From<FocusedTriplePattern<F>> for FocusedPropertyPathPattern<T> {
        fn from(input: FocusedTriplePattern<F>) -> Self {
            Self {
                focus: input.focus.into(),
                patterns: input.patterns.into_iter().map(|p| p.into()).collect(),
            }
        }
    }

    impl From<FocusedPropertyPathPattern<TermOrVariable>> for GraphPattern {
        fn from(input: FocusedPropertyPathPattern<TermOrVariable>) -> Self {
            if input.patterns.len() == 1 {
                input.patterns[0].clone().into()
            } else {
                GraphPattern::GroupPattern(input.patterns.into_iter().map(|p| p.into()).collect())
            }
        }
    }

    fn flatten_group_pattern(v: impl Iterator<Item = GraphPattern>) -> GraphPattern {
        let l: Vec<GraphPattern> = v.into_iter()
            .flat_map(|p| {
                if let GraphPattern::GroupPattern(v2) = p {
                    v2.into_iter()
                } else {
                    vec![p].into_iter()
                }
            })
            .collect();
        if l.len() == 1 {
            l[0].clone()
        } else {
            GraphPattern::GroupPattern(l)
        }
    }

    pub struct ParserState {
        base_uri: Option<Url>,
        namespaces: HashMap<String, String>,
        bnodes_map: BTreeMap<String, BlankNode>,
    }

    impl ParserState {
        fn url_parser<'a>(&'a self) -> ParseOptions<'a> {
            Url::options().base_url(self.base_uri.as_ref())
        }
    }

    include!(concat!(env!("OUT_DIR"), "/sparql_grammar.rs"));

    pub fn read_sparql_query<'a, R: Read + 'a>(
        source: R,
        base_uri: impl Into<Option<Url>>,
    ) -> RioResult<Query> {
        let mut state = ParserState {
            base_uri: base_uri.into(),
            namespaces: HashMap::default(),
            bnodes_map: BTreeMap::default(),
        };

        let mut string_buffer = String::default();
        BufReader::new(source).read_to_string(&mut string_buffer)?;

        match QueryUnit(
            &unescape_unicode_codepoints(Cow::from(string_buffer)),
            &mut state,
        ) {
            Ok(query) => Ok(query),
            Err(error) => Err(RioError::new(error)),
        }
    }
}

pub use sparql::parser::grammar::read_sparql_query;

fn needs_unescape_unicode_codepoints(input: &str) -> bool {
    let bytes = input.as_bytes();
    for i in 1..bytes.len() {
        if (bytes[i] == ('u' as u8) || bytes[i] == ('U' as u8)) && bytes[i - 1] == ('/' as u8) {
            return true;
        }
    }
    return false;
}

struct UnescapeUnicodeCharIterator<'a> {
    iter: Chars<'a>,
    buffer: String,
}

impl<'a> UnescapeUnicodeCharIterator<'a> {
    fn new(string: &'a str) -> Self {
        Self {
            iter: string.chars(),
            buffer: String::with_capacity(9),
        }
    }
}

impl<'a> Iterator for UnescapeUnicodeCharIterator<'a> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        if !self.buffer.is_empty() {
            return Some(self.buffer.remove(0));
        }
        match self.iter.next()? {
            '\\' => match self.iter.next() {
                Some('u') => {
                    self.buffer.push('u');
                    for _ in 0..4 {
                        if let Some(c) = self.iter.next() {
                            self.buffer.push(c);
                        } else {
                            return Some('\\');
                        }
                    }
                    if let Some(c) = u32::from_str_radix(&self.buffer[1..5], 16)
                        .ok()
                        .and_then(char::from_u32)
                    {
                        self.buffer.clear();
                        Some(c)
                    } else {
                        Some('\\')
                    }
                }
                Some('U') => {
                    self.buffer.push('U');
                    for _ in 0..8 {
                        if let Some(c) = self.iter.next() {
                            self.buffer.push(c);
                        } else {
                            return Some('\\');
                        }
                    }
                    if let Some(c) = u32::from_str_radix(&self.buffer[1..9], 16)
                        .ok()
                        .and_then(char::from_u32)
                    {
                        self.buffer.clear();
                        Some(c)
                    } else {
                        Some('\\')
                    }
                }
                Some(c) => {
                    self.buffer.push(c);
                    Some('\\')
                }
                None => Some('\\'),
            },
            c => Some(c),
        }
    }
}

fn unescape_unicode_codepoints<'a>(input: Cow<'a, str>) -> Cow<'a, str> {
    if needs_unescape_unicode_codepoints(&input) {
        UnescapeUnicodeCharIterator::new(&input).collect()
    } else {
        input
    }
}
