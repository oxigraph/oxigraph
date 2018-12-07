mod grammar {
    #![allow(
        clippy::suspicious_else_formatting,
        clippy::len_zero,
        clippy::single_match,
        clippy::unit_arg,
        clippy::naive_bytecount,
        clippy::cyclomatic_complexity,
        clippy::many_single_char_names,
        clippy::type_complexity
    )]

    use crate::model::*;
    use crate::rio::utils::unescape_characters;
    use crate::rio::utils::unescape_unicode_codepoints;
    use crate::sparql::algebra::*;
    use crate::utils::StaticSliceMap;
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
        group: Option<(Vec<Expression>, Vec<(Expression, Variable)>)>,
        having: Option<Expression>,
        order_by: Option<Vec<OrderComparator>>,
        offset_limit: Option<(usize, Option<usize>)>,
        values: Option<GraphPattern>,
        state: &mut ParserState,
    ) -> GraphPattern {
        let mut p = wher;

        //GROUP BY
        if let Some((clauses, binds)) = group {
            for (e, v) in binds {
                p = GraphPattern::Extend(Box::new(p), v, e);
            }
            let g = GroupPattern(clauses, Box::new(p));
            p = GraphPattern::AggregateJoin(g, state.aggregations.clone());
            state.aggregations = BTreeMap::default();
        }
        if !state.aggregations.is_empty() {
            let g = GroupPattern(vec![Literal::from(1).into()], Box::new(p));
            p = GraphPattern::AggregateJoin(g, state.aggregations.clone());
            state.aggregations = BTreeMap::default();
        }

        //TODO: not aggregated vars

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
        base_uri: Option<Url>,
        namespaces: HashMap<String, String>,
        bnodes_map: BTreeMap<String, BlankNode>,
        aggregations: BTreeMap<Aggregation, Variable>,
    }

    impl ParserState {
        fn url_parser(&self) -> ParseOptions<'_> {
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

    fn unescape_echars(input: &str) -> Cow<'_, str> {
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

    pub fn unescape_pn_local(input: &str) -> Cow<'_, str> {
        unescape_characters(input, &UNESCAPE_PN_CHARACTERS, &UNESCAPE_PN_REPLACEMENT)
    }

    include!(concat!(env!("OUT_DIR"), "/sparql_grammar.rs"));

    pub fn read_sparql_query<'a, R: Read + 'a>(
        source: R,
        base_uri: impl Into<Option<Url>>,
    ) -> super::super::super::Result<Query> {
        let mut state = ParserState {
            base_uri: base_uri.into(),
            namespaces: HashMap::default(),
            bnodes_map: BTreeMap::default(),
            aggregations: BTreeMap::default(),
        };

        let mut string_buffer = String::default();
        BufReader::new(source).read_to_string(&mut string_buffer)?;

        Ok(QueryUnit(
            &unescape_unicode_codepoints(&string_buffer),
            &mut state,
        )?)
    }
}

pub use self::grammar::read_sparql_query;
