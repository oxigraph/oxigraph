pub trait Escaper {
    fn escape(&self) -> String;
}

impl<'a> Escaper for &'a str {
    fn escape(&self) -> String {
        self.chars().flat_map(|c| EscapeRDF::new(c)).collect()
    }
}

impl Escaper for String {
    fn escape(&self) -> String {
        self.chars().flat_map(|c| EscapeRDF::new(c)).collect()
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
