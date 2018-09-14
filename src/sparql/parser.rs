use std::borrow::Cow;
use std::char;
use std::str::Chars;

mod grammar {
    #![allow(unknown_lints)]
    #![allow(clippy)]

    use model::*;
    use sparql::algebra::*;
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

    #[derive(Clone)]
    enum VariableOrPropertyPath {
        Variable(Variable),
        PropertyPath(PropertyPath),
    }

    fn to_triple_or_path_pattern(
        s: TermOrVariable,
        p: VariableOrPropertyPath,
        o: TermOrVariable,
    ) -> TripleOrPathPattern {
        match p {
            VariableOrPropertyPath::Variable(p) => TriplePattern::new(s, p, o).into(),
            VariableOrPropertyPath::PropertyPath(p) => PathPattern::new(s, p, o).into(),
        }
    }

    struct FocusedTripleOrPathPattern<F> {
        focus: F,
        patterns: Vec<TripleOrPathPattern>,
    }

    impl<F> FocusedTripleOrPathPattern<F> {
        fn new(focus: F) -> Self {
            Self {
                focus,
                patterns: Vec::default(),
            }
        }
    }

    impl<F: Default> Default for FocusedTripleOrPathPattern<F> {
        fn default() -> Self {
            Self {
                focus: F::default(),
                patterns: Vec::default(),
            }
        }
    }

    impl<F> From<FocusedTripleOrPathPattern<F>> for FocusedTripleOrPathPattern<Vec<F>> {
        fn from(input: FocusedTripleOrPathPattern<F>) -> Self {
            Self {
                focus: vec![input.focus],
                patterns: input.patterns,
            }
        }
    }

    impl<F, T: From<F>> From<FocusedTriplePattern<F>> for FocusedTripleOrPathPattern<T> {
        fn from(input: FocusedTriplePattern<F>) -> Self {
            Self {
                focus: input.focus.into(),
                patterns: input.patterns.into_iter().map(|p| p.into()).collect(),
            }
        }
    }

    #[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
    enum PartialGraphPattern {
        Optional(MultiSetPattern),
        Minus(MultiSetPattern),
        Bind(Expression, Variable),
        Filter(Expression),
        Other(MultiSetPattern),
    }

    fn new_join(l: MultiSetPattern, r: MultiSetPattern) -> MultiSetPattern {
        //Avoid to output empty BGPs
        if let MultiSetPattern::BGP(pl) = &l {
            if pl.is_empty() {
                return r;
            }
        }
        if let MultiSetPattern::BGP(pr) = &r {
            if pr.is_empty() {
                return l;
            }
        }

        //Merge BGPs
        match (l, r) {
            (MultiSetPattern::BGP(mut pl), MultiSetPattern::BGP(pr)) => {
                pl.extend_from_slice(&pr);
                MultiSetPattern::BGP(pl)
            }
            (l, r) => MultiSetPattern::Join(Box::new(l), Box::new(r)),
        }
    }

    fn not_empty_fold<T>(
        iter: impl Iterator<Item = T>,
        combine: impl Fn(T, T) -> T,
    ) -> Result<T, &'static str> {
        iter.fold(None, |a, b| match a {
            Some(av) => Some(combine(av, b)),
            None => Some(b),
        }).ok_or("The iterator should not be empty")
    }

    enum SelectionOption {
        Distinct,
        Reduced,
        Default,
    }

    enum SelectionMember {
        Variable(Variable),
        Expression(Expression, Variable),
    }

    struct Selection {
        pub option: SelectionOption,
        pub variables: Option<Vec<SelectionMember>>,
    }

    impl Default for Selection {
        fn default() -> Self {
            Self {
                option: SelectionOption::Default,
                variables: None,
            }
        }
    }

    fn build_select(
        select: Selection,
        wher: MultiSetPattern,
        group: Option<(Vec<Expression>, Vec<(Expression, Variable)>)>,
        having: Option<Expression>,
        order_by: Option<Vec<OrderComparator>>,
        offset_limit: Option<(usize, Option<usize>)>,
        values: Option<MultiSetPattern>,
        state: &mut ParserState,
    ) -> ListPattern {
        let mut p = wher;

        //GROUP BY
        if let Some((clauses, binds)) = group {
            for (e, v) in binds {
                p = MultiSetPattern::Extend(Box::new(p), v, e);
            }
            let g = GroupPattern(clauses, Box::new(p));
            p = MultiSetPattern::AggregateJoin(g, state.aggregations.clone());
            state.aggregations = BTreeMap::default();
        }
        if !state.aggregations.is_empty() {
            let g = GroupPattern(vec![Literal::from(1).into()], Box::new(p));
            p = MultiSetPattern::AggregateJoin(g, state.aggregations.clone());
            state.aggregations = BTreeMap::default();
        }

        //TODO: not aggregated vars

        //HAVING
        if let Some(ex) = having {
            p = MultiSetPattern::Filter(ex, Box::new(p));
        }

        //VALUES
        if let Some(data) = values {
            p = MultiSetPattern::Join(Box::new(p), Box::new(data));
        }

        //SELECT
        let mut pv: Vec<Variable> = Vec::default();
        match select.variables {
            Some(sel_items) => {
                for sel_item in sel_items {
                    match sel_item {
                        SelectionMember::Variable(v) => pv.push(v),
                        SelectionMember::Expression(e, v) => if pv.contains(&v) {
                            //TODO: fail
                        } else {
                            p = MultiSetPattern::Extend(Box::new(p), v.clone(), e);
                            pv.push(v);
                        },
                    }
                }
            }
            None => {
                pv.extend(p.visible_variables().into_iter().cloned()) //TODO: is it really useful to do a projection?
            }
        }
        let mut m = ListPattern::from(p);

        //ORDER BY
        if let Some(order) = order_by {
            m = ListPattern::OrderBy(Box::new(m), order);
        }

        //PROJECT
        m = ListPattern::Project(Box::new(m), pv);
        match select.option {
            SelectionOption::Distinct => m = ListPattern::Distinct(Box::new(m)),
            SelectionOption::Reduced => m = ListPattern::Reduced(Box::new(m)),
            SelectionOption::Default => (),
        }

        //OFFSET LIMIT
        if let Some((offset, limit)) = offset_limit {
            m = ListPattern::Slice(Box::new(m), offset, limit)
        }
        m
    }

    enum Either<L, R> {
        Left(L),
        Right(R),
    }

    pub struct ParserState {
        base_uri: Option<Url>,
        namespaces: HashMap<String, String>,
        bnodes_map: BTreeMap<String, BlankNode>,
        aggregations: BTreeMap<Aggregation, Variable>,
    }

    impl ParserState {
        fn url_parser(&self) -> ParseOptions {
            Url::options().base_url(self.base_uri.as_ref())
        }

        fn new_aggregation(&mut self, agg: Aggregation) -> Variable {
            self.aggregations.get(&agg).cloned().unwrap_or_else(|| {
                let new_var = Variable::default();
                self.aggregations.insert(agg, new_var.clone());
                new_var
            })
        }
    }

    include!(concat!(env!("OUT_DIR"), "/sparql_grammar.rs"));

    pub fn read_sparql_query<'a, R: Read + 'a>(
        source: R,
        base_uri: impl Into<Option<Url>>,
    ) -> super::super::super::errors::Result<Query> {
        let mut state = ParserState {
            base_uri: base_uri.into(),
            namespaces: HashMap::default(),
            bnodes_map: BTreeMap::default(),
            aggregations: BTreeMap::default(),
        };

        let mut string_buffer = String::default();
        BufReader::new(source).read_to_string(&mut string_buffer)?;

        match QueryUnit(
            &unescape_unicode_codepoints(Cow::from(string_buffer)),
            &mut state,
        ) {
            Ok(query) => Ok(query),
            Err(error) => Err(error.into()),
        }
    }
}

pub(crate) type ParseError = self::grammar::ParseError;
pub use self::grammar::read_sparql_query;

fn needs_unescape_unicode_codepoints(input: &str) -> bool {
    let bytes = input.as_bytes();
    for i in 1..bytes.len() {
        if (bytes[i] == b'u' || bytes[i] == b'U') && bytes[i - 1] == b'/' {
            return true;
        }
    }
    false
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

fn unescape_unicode_codepoints(input: Cow<str>) -> Cow<str> {
    if needs_unescape_unicode_codepoints(&input) {
        UnescapeUnicodeCharIterator::new(&input).collect()
    } else {
        input
    }
}
