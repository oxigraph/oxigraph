pub trait Escaper {
    fn escape(&self) -> String;
}

impl<'a> Escaper for &'a str {
    fn escape(&self) -> String {
        self.chars().flat_map(EscapeRDF::new).collect()
    }
}

impl Escaper for String {
    fn escape(&self) -> String {
        self.chars().flat_map(EscapeRDF::new).collect()
    }
}

/// Customized version of EscapeDefault of the Rust standard library
struct EscapeRDF {
    state: EscapeRdfState,
}

enum EscapeRdfState {
    Done,
    Char(char),
    Backslash(char),
}

impl EscapeRDF {
    fn new(c: char) -> EscapeRDF {
        EscapeRDF {
            state: match c {
                '\t' => EscapeRdfState::Backslash('t'),
                '\u{08}' => EscapeRdfState::Backslash('b'),
                '\n' => EscapeRdfState::Backslash('n'),
                '\r' => EscapeRdfState::Backslash('r'),
                '\u{0C}' => EscapeRdfState::Backslash('f'),
                '\\' | '\'' | '"' => EscapeRdfState::Backslash(c),
                c => EscapeRdfState::Char(c),
            },
        }
    }
}

impl Iterator for EscapeRDF {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        match self.state {
            EscapeRdfState::Backslash(c) => {
                self.state = EscapeRdfState::Char(c);
                Some('\\')
            }
            EscapeRdfState::Char(c) => {
                self.state = EscapeRdfState::Done;
                Some(c)
            }
            EscapeRdfState::Done => None,
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.len();
        (n, Some(n))
    }

    fn count(self) -> usize {
        self.len()
    }
}

impl ExactSizeIterator for EscapeRDF {
    fn len(&self) -> usize {
        match self.state {
            EscapeRdfState::Done => 0,
            EscapeRdfState::Char(_) => 1,
            EscapeRdfState::Backslash(_) => 2,
        }
    }
}

pub struct StaticSliceMap<K: 'static + Copy + Eq, V: 'static + Copy> {
    keys: &'static [K],
    values: &'static [V],
}

impl<K: 'static + Copy + Eq, V: 'static + Copy> StaticSliceMap<K, V> {
    pub fn new(keys: &'static [K], values: &'static [V]) -> Self {
        assert_eq!(
            keys.len(),
            values.len(),
            "keys and values slices of StaticSliceMap should have the same size"
        );
        Self { keys, values }
    }

    pub fn get(&self, key: K) -> Option<V> {
        for i in 0..self.keys.len() {
            if self.keys[i] == key {
                return Some(self.values[i]);
            }
        }
        None
    }
}
