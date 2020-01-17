mod grammar {
    #![allow(
        clippy::suspicious_else_formatting,
        clippy::len_zero,
        clippy::single_match,
        clippy::unit_arg,
        clippy::naive_bytecount,
        clippy::cognitive_complexity,
        clippy::many_single_char_names,
        clippy::type_complexity,
        ellipsis_inclusive_range_patterns
    )]

    use crate::model::*;
    use crate::sparql::algebra::*;
    use crate::sparql::model::*;
    use rio_api::iri::{Iri, IriParseError};
    use std::borrow::Cow;
    use std::char;
    use std::collections::HashMap;
    use std::collections::{BTreeMap, BTreeSet};
    use std::str::Chars;

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

    impl From<Variable> for VariableOrPropertyPath {
        fn from(var: Variable) -> Self {
            VariableOrPropertyPath::Variable(var)
        }
    }

    impl From<PropertyPath> for VariableOrPropertyPath {
        fn from(path: PropertyPath) -> Self {
            VariableOrPropertyPath::PropertyPath(path)
        }
    }

    fn add_to_triple_or_path_patterns(
        s: TermOrVariable,
        p: impl Into<VariableOrPropertyPath>,
        o: TermOrVariable,
        patterns: &mut Vec<TripleOrPathPattern>,
    ) {
        match p.into() {
            VariableOrPropertyPath::Variable(p) => {
                patterns.push(TriplePattern::new(s, p, o).into())
            }
            VariableOrPropertyPath::PropertyPath(p) => match p {
                PropertyPath::PredicatePath(p) => patterns.push(TriplePattern::new(s, p, o).into()),
                PropertyPath::InversePath(p) => add_to_triple_or_path_patterns(o, *p, s, patterns),
                PropertyPath::SequencePath(a, b) => {
                    let middle = Variable::default();
                    add_to_triple_or_path_patterns(s, *a, middle.clone().into(), patterns);
                    add_to_triple_or_path_patterns(middle.into(), *b, o, patterns);
                }
                p => patterns.push(PathPattern::new(s, p, o).into()),
            },
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
        Optional(GraphPattern),
        Minus(GraphPattern),
        Bind(Expression, Variable),
        Filter(Expression),
        Other(GraphPattern),
    }

    fn new_join(l: GraphPattern, r: GraphPattern) -> GraphPattern {
        //Avoid to output empty BGPs
        if let GraphPattern::BGP(pl) = &l {
            if pl.is_empty() {
                return r;
            }
        }
        if let GraphPattern::BGP(pr) = &r {
            if pr.is_empty() {
                return l;
            }
        }

        //Merge BGPs
        match (l, r) {
            (GraphPattern::BGP(mut pl), GraphPattern::BGP(pr)) => {
                pl.extend_from_slice(&pr);
                GraphPattern::BGP(pl)
            }
            (l, r) => GraphPattern::Join(Box::new(l), Box::new(r)),
        }
    }

    fn not_empty_fold<T>(
        iter: impl Iterator<Item = T>,
        combine: impl Fn(T, T) -> T,
    ) -> Result<T, &'static str> {
        iter.fold(None, |a, b| match a {
            Some(av) => Some(combine(av, b)),
            None => Some(b),
        })
        .ok_or("The iterator should not be empty")
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
        wher: GraphPattern,
        mut group: Option<(Vec<Variable>, Vec<(Expression, Variable)>)>,
        having: Option<Expression>,
        order_by: Option<Vec<OrderComparator>>,
        offset_limit: Option<(usize, Option<usize>)>,
        values: Option<GraphPattern>,
        state: &mut ParserState,
    ) -> GraphPattern {
        let mut p = wher;

        //GROUP BY
        let aggregations = state.aggregations.pop().unwrap_or_else(BTreeMap::default);
        if group.is_none() && !aggregations.is_empty() {
            let const_variable = Variable::default();
            group = Some((
                vec![const_variable.clone()],
                vec![(Literal::from(1).into(), const_variable)],
            ));
        }

        if let Some((clauses, binds)) = group {
            for (e, v) in binds {
                p = GraphPattern::Extend(Box::new(p), v, e);
            }
            let g = GroupPattern(clauses, Box::new(p));
            p = GraphPattern::AggregateJoin(g, aggregations);
        }

        //HAVING
        if let Some(ex) = having {
            p = GraphPattern::Filter(ex, Box::new(p));
        }

        //VALUES
        if let Some(data) = values {
            p = new_join(p, data);
        }

        //SELECT
        let mut pv: Vec<Variable> = Vec::default();
        match select.variables {
            Some(sel_items) => {
                for sel_item in sel_items {
                    match sel_item {
                        SelectionMember::Variable(v) => pv.push(v),
                        SelectionMember::Expression(e, v) => {
                            if pv.contains(&v) {
                                //TODO: fail
                            } else {
                                p = GraphPattern::Extend(Box::new(p), v.clone(), e);
                                pv.push(v);
                            }
                        }
                    }
                }
            }
            None => {
                pv.extend(p.visible_variables().into_iter().cloned()) //TODO: is it really useful to do a projection?
            }
        }
        let mut m = p;

        //ORDER BY
        if let Some(order) = order_by {
            m = GraphPattern::OrderBy(Box::new(m), order);
        }

        //PROJECT
        m = GraphPattern::Project(Box::new(m), pv);
        match select.option {
            SelectionOption::Distinct => m = GraphPattern::Distinct(Box::new(m)),
            SelectionOption::Reduced => m = GraphPattern::Reduced(Box::new(m)),
            SelectionOption::Default => (),
        }

        //OFFSET LIMIT
        if let Some((offset, limit)) = offset_limit {
            m = GraphPattern::Slice(Box::new(m), offset, limit)
        }
        m
    }

    enum Either<L, R> {
        Left(L),
        Right(R),
    }

    pub struct ParserState {
        base_iri: Option<Iri<String>>,
        namespaces: HashMap<String, String>,
        bnodes_map: BTreeMap<String, BlankNode>,
        used_bnodes: BTreeSet<String>,
        aggregations: Vec<BTreeMap<Aggregation, Variable>>,
    }

    impl ParserState {
        fn parse_iri(&self, iri: &str) -> Result<Iri<String>, IriParseError> {
            if let Some(base_iri) = &self.base_iri {
                base_iri.resolve(iri)
            } else {
                Iri::parse(iri.to_owned())
            }
        }

        fn new_aggregation(&mut self, agg: Aggregation) -> Result<Variable, &'static str> {
            let aggregations = self
                .aggregations
                .last_mut()
                .ok_or_else(|| "Unexpected aggregate")?;
            Ok(aggregations.get(&agg).cloned().unwrap_or_else(|| {
                let new_var = Variable::default();
                aggregations.insert(agg, new_var.clone());
                new_var
            }))
        }
    }

    pub fn unescape_unicode_codepoints(input: &str) -> Cow<'_, str> {
        if needs_unescape_unicode_codepoints(input) {
            UnescapeUnicodeCharIterator::new(input).collect()
        } else {
            input.into()
        }
    }

    fn needs_unescape_unicode_codepoints(input: &str) -> bool {
        let bytes = input.as_bytes();
        for i in 1..bytes.len() {
            if (bytes[i] == b'u' || bytes[i] == b'U') && bytes[i - 1] == b'\\' {
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

    pub fn unescape_characters<'a>(
        input: &'a str,
        characters: &'static [u8],
        replacement: &'static StaticCharSliceMap,
    ) -> Cow<'a, str> {
        if needs_unescape_characters(input, characters) {
            UnescapeCharsIterator::new(input, replacement).collect()
        } else {
            input.into()
        }
    }

    fn needs_unescape_characters(input: &str, characters: &[u8]) -> bool {
        let bytes = input.as_bytes();
        for i in 1..bytes.len() {
            if bytes[i - 1] == b'\\' && characters.contains(&bytes[i]) {
                return true;
            }
        }
        false
    }

    struct UnescapeCharsIterator<'a> {
        iter: Chars<'a>,
        buffer: Option<char>,
        replacement: &'static StaticCharSliceMap,
    }

    impl<'a> UnescapeCharsIterator<'a> {
        fn new(string: &'a str, replacement: &'static StaticCharSliceMap) -> Self {
            Self {
                iter: string.chars(),
                buffer: None,
                replacement,
            }
        }
    }

    impl<'a> Iterator for UnescapeCharsIterator<'a> {
        type Item = char;

        fn next(&mut self) -> Option<char> {
            if let Some(ch) = self.buffer {
                self.buffer = None;
                return Some(ch);
            }
            match self.iter.next()? {
                '\\' => match self.iter.next() {
                    Some(ch) => match self.replacement.get(ch) {
                        Some(replace) => Some(replace),
                        None => {
                            self.buffer = Some(ch);
                            Some('\\')
                        }
                    },
                    None => Some('\\'),
                },
                c => Some(c),
            }
        }
    }

    pub struct StaticCharSliceMap {
        keys: &'static [char],
        values: &'static [char],
    }

    impl StaticCharSliceMap {
        pub const fn new(keys: &'static [char], values: &'static [char]) -> Self {
            Self { keys, values }
        }

        pub fn get(&self, key: char) -> Option<char> {
            for i in 0..self.keys.len() {
                if self.keys[i] == key {
                    return Some(self.values[i]);
                }
            }
            None
        }
    }

    const UNESCAPE_CHARACTERS: [u8; 8] = [b't', b'b', b'n', b'r', b'f', b'"', b'\'', b'\\'];
    const UNESCAPE_REPLACEMENT: StaticCharSliceMap = StaticCharSliceMap::new(
        &['t', 'b', 'n', 'r', 'f', '"', '\'', '\\'],
        &[
            '\u{0009}', '\u{0008}', '\u{000A}', '\u{000D}', '\u{000C}', '\u{0022}', '\u{0027}',
            '\u{005C}',
        ],
    );

    fn unescape_echars(input: &str) -> Cow<'_, str> {
        unescape_characters(input, &UNESCAPE_CHARACTERS, &UNESCAPE_REPLACEMENT)
    }

    const UNESCAPE_PN_CHARACTERS: [u8; 20] = [
        b'_', b'~', b'.', b'-', b'!', b'$', b'&', b'\'', b'(', b')', b'*', b'+', b',', b';', b'=',
        b'/', b'?', b'#', b'@', b'%',
    ];
    const UNESCAPE_PN_REPLACEMENT: StaticCharSliceMap = StaticCharSliceMap::new(
        &[
            '_', '~', '.', '-', '!', '$', '&', '\'', '(', ')', '*', '+', ',', ';', '=', '/', '?',
            '#', '@', '%',
        ],
        &[
            '_', '~', '.', '-', '!', '$', '&', '\'', '(', ')', '*', '+', ',', ';', '=', '/', '?',
            '#', '@', '%',
        ],
    );

    pub fn unescape_pn_local(input: &str) -> Cow<'_, str> {
        unescape_characters(input, &UNESCAPE_PN_CHARACTERS, &UNESCAPE_PN_REPLACEMENT)
    }

    include!(concat!(env!("OUT_DIR"), "/sparql_grammar.rs"));

    pub fn read_sparql_query(
        query: &str,
        base_iri: Option<&str>,
    ) -> super::super::super::Result<QueryVariants> {
        let mut state = ParserState {
            base_iri: if let Some(base_iri) = base_iri {
                Some(Iri::parse(base_iri.to_owned())?)
            } else {
                None
            },
            namespaces: HashMap::default(),
            bnodes_map: BTreeMap::default(),
            used_bnodes: BTreeSet::default(),
            aggregations: Vec::default(),
        };

        Ok(QueryUnit(&unescape_unicode_codepoints(query), &mut state)?)
    }
}

pub use self::grammar::read_sparql_query;
