use crate::algebra::{
    AggregateExpression, AggregateFunction, Expression, Function, GraphPattern, OrderExpression,
    PropertyPathExpression, QueryDataset,
};
use crate::term::{
    GraphName, GraphNamePattern, GroundTerm, NamedNodePattern, QuadPattern, TermPattern,
    TriplePattern,
};
use crate::{GraphUpdateOperation, Query};
use chumsky::input::ValueInput;
use chumsky::pratt::{infix, left, postfix, prefix};
use chumsky::prelude::*;
use chumsky::text::ascii::ident;
use oxiri::{Iri, IriParseError};
use oxrdf::vocab::{rdf, xsd};
use oxrdf::{BlankNode, Literal, NamedNode, NamedOrBlankNode, Term, Triple, Variable};
use peg::str::LineCol;
use rand::random;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::fmt::Write;
use std::mem::take;
use std::rc::Rc;
use std::str::{Chars, FromStr};

/// Error returned during SPARQL parsing.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct SparqlSyntaxError {
    #[from]
    kind: SparqlSyntaxErrorKind,
}

#[derive(Debug, thiserror::Error)]
enum SparqlSyntaxErrorKind {
    #[error("Invalid SPARQL base IRI provided: {0}")]
    InvalidBaseIri(#[from] IriParseError),
    #[error(transparent)]
    Syntax(#[from] peg::error::ParseError<LineCol>),
    #[error("The blank node {0} cannot be shared by multiple blocks")]
    SharedBlankNode(BlankNode),
}

#[cfg(feature = "standard-unicode-escaping")]
fn unescape_unicode_codepoints(input: &str) -> Cow<'_, str> {
    if needs_unescape_unicode_codepoints(input) {
        UnescapeUnicodeCharIterator::new(input).collect()
    } else {
        input.into()
    }
}

#[cfg(feature = "standard-unicode-escaping")]
fn needs_unescape_unicode_codepoints(input: &str) -> bool {
    let bytes = input.as_bytes();
    for i in 1..bytes.len() {
        if (bytes[i] == b'u' || bytes[i] == b'U') && bytes[i - 1] == b'\\' {
            return true;
        }
    }
    false
}

#[cfg(feature = "standard-unicode-escaping")]
struct UnescapeUnicodeCharIterator<'a> {
    iter: Chars<'a>,
    buffer: String,
}

#[cfg(feature = "standard-unicode-escaping")]
impl<'a> UnescapeUnicodeCharIterator<'a> {
    fn new(string: &'a str) -> Self {
        Self {
            iter: string.chars(),
            buffer: String::with_capacity(9),
        }
    }
}

#[cfg(feature = "standard-unicode-escaping")]
impl<'a> Iterator for UnescapeUnicodeCharIterator<'a> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        let c = if self.buffer.is_empty() {
            self.iter.next()?
        } else {
            self.buffer.remove(0)
        };
        match c {
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
                    if let Some(c) = u32::from_str_radix(&self.buffer[1..], 16)
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
                    if let Some(c) = u32::from_str_radix(&self.buffer[1..], 16)
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
            _ => Some(c),
        }
    }
}

#[derive(Clone)]
struct ReifiedTerm {
    term: TermPattern,
    reifiers: Vec<TermPattern>,
}

#[derive(Default, Clone)]
struct FocusedTriplePattern<F> {
    focus: F,
    patterns: Vec<TriplePattern>,
}

impl<F> FocusedTriplePattern<F> {
    fn new(focus: impl Into<F>) -> Self {
        Self {
            focus: focus.into(),
            patterns: Vec::new(),
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

#[derive(Clone, Debug)]
enum VariableOrPropertyPath {
    Variable(Variable),
    PropertyPath(PropertyPathExpression),
}

impl From<Variable> for VariableOrPropertyPath {
    fn from(var: Variable) -> Self {
        Self::Variable(var)
    }
}

impl From<NamedNodePattern> for VariableOrPropertyPath {
    fn from(pattern: NamedNodePattern) -> Self {
        match pattern {
            NamedNodePattern::NamedNode(node) => PropertyPathExpression::from(node).into(),
            NamedNodePattern::Variable(v) => v.into(),
        }
    }
}

impl From<PropertyPathExpression> for VariableOrPropertyPath {
    fn from(path: PropertyPathExpression) -> Self {
        Self::PropertyPath(path)
    }
}

#[cfg_attr(feature = "sparql-12", expect(clippy::unnecessary_wraps))]
fn add_to_triple_patterns(
    subject: TermPattern,
    predicate: NamedNodePattern,
    object: ReifiedTerm,
    patterns: &mut Vec<TriplePattern>,
) -> Result<(), &'static str> {
    let triple = TriplePattern::new(subject, predicate, object.term);
    #[cfg(feature = "sparql-12")]
    for reifier in object.reifiers {
        patterns.push(TriplePattern {
            subject: reifier.clone(),
            predicate: rdf::REIFIES.into_owned().into(),
            object: triple.clone().into(),
        });
    }
    #[cfg(not(feature = "sparql-12"))]
    if !object.reifiers.is_empty() {
        return Err("Triple terms are only available in SPARQL 1.2");
    }
    patterns.push(triple);
    Ok(())
}

fn add_to_triple_or_path_patterns(
    subject: TermPattern,
    predicate: impl Into<VariableOrPropertyPath>,
    object: ReifiedTerm,
    patterns: &mut Vec<TripleOrPathPattern>,
) -> Result<(), &'static str> {
    match predicate.into() {
        VariableOrPropertyPath::Variable(p) => {
            add_triple_to_triple_or_path_patterns(subject, p, object, patterns)?;
        }
        VariableOrPropertyPath::PropertyPath(p) => match p {
            PropertyPathExpression::NamedNode(p) => {
                add_triple_to_triple_or_path_patterns(subject, p, object, patterns)?;
            }
            PropertyPathExpression::Reverse(p) => add_to_triple_or_path_patterns(
                object.term,
                *p,
                ReifiedTerm {
                    term: subject,
                    reifiers: object.reifiers,
                },
                patterns,
            )?,
            PropertyPathExpression::Sequence(a, b) => {
                if !object.reifiers.is_empty() {
                    return Err("Reifiers are not allowed on property paths");
                }
                let middle = BlankNode::default();
                add_to_triple_or_path_patterns(
                    subject,
                    *a,
                    ReifiedTerm {
                        term: middle.clone().into(),
                        reifiers: Vec::new(),
                    },
                    patterns,
                )?;
                add_to_triple_or_path_patterns(
                    middle.into(),
                    *b,
                    ReifiedTerm {
                        term: object.term,
                        reifiers: Vec::new(),
                    },
                    patterns,
                )?;
            }
            path => {
                if !object.reifiers.is_empty() {
                    return Err("Reifiers are not allowed on property paths");
                }
                patterns.push(TripleOrPathPattern::Path {
                    subject,
                    path,
                    object: object.term,
                })
            }
        },
    }
    Ok(())
}

#[cfg_attr(feature = "sparql-12", expect(clippy::unnecessary_wraps))]
fn add_triple_to_triple_or_path_patterns(
    subject: TermPattern,
    predicate: impl Into<NamedNodePattern>,
    object: ReifiedTerm,
    patterns: &mut Vec<TripleOrPathPattern>,
) -> Result<(), &'static str> {
    let triple = TriplePattern::new(subject, predicate, object.term);
    #[cfg(feature = "sparql-12")]
    for reifier in object.reifiers {
        patterns.push(
            TriplePattern {
                subject: reifier.clone(),
                predicate: rdf::REIFIES.into_owned().into(),
                object: triple.clone().into(),
            }
            .into(),
        );
    }
    #[cfg(not(feature = "sparql-12"))]
    if !object.reifiers.is_empty() {
        return Err("Triple terms are only available in SPARQL 1.2");
    }
    patterns.push(triple.into());
    Ok(())
}

fn build_bgp(patterns: Vec<TripleOrPathPattern>) -> GraphPattern {
    let mut bgp = Vec::new();
    let mut elements = Vec::with_capacity(patterns.len());
    for pattern in patterns {
        match pattern {
            TripleOrPathPattern::Triple(t) => bgp.push(t),
            TripleOrPathPattern::Path {
                subject,
                path,
                object,
            } => {
                if !bgp.is_empty() {
                    elements.push(GraphPattern::Bgp {
                        patterns: take(&mut bgp),
                    });
                }
                elements.push(GraphPattern::Path {
                    subject,
                    path,
                    object,
                })
            }
        }
    }
    if !bgp.is_empty() {
        elements.push(GraphPattern::Bgp { patterns: bgp });
    }
    elements.into_iter().reduce(new_join).unwrap_or_default()
}

#[derive(Debug, Clone)]
enum TripleOrPathPattern {
    Triple(TriplePattern),
    Path {
        subject: TermPattern,
        path: PropertyPathExpression,
        object: TermPattern,
    },
}

impl From<TriplePattern> for TripleOrPathPattern {
    fn from(tp: TriplePattern) -> Self {
        Self::Triple(tp)
    }
}

#[derive(Debug, Default, Clone)]
struct FocusedTripleOrPathPattern<F> {
    focus: F,
    patterns: Vec<TripleOrPathPattern>,
}

impl<F> FocusedTripleOrPathPattern<F> {
    fn new(focus: impl Into<F>) -> Self {
        Self {
            focus: focus.into(),
            patterns: Vec::new(),
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
            patterns: input.patterns.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
enum PartialGraphPattern {
    Optional(GraphPattern, Option<Expression>),
    #[cfg(feature = "sep-0006")]
    Lateral(GraphPattern),
    Minus(GraphPattern),
    Bind(Expression, Variable),
    Filter(Expression),
    Other(GraphPattern),
}

fn new_join(l: GraphPattern, r: GraphPattern) -> GraphPattern {
    // Avoid to output empty BGPs
    if let GraphPattern::Bgp { patterns: pl } = &l {
        if pl.is_empty() {
            return r;
        }
    }
    if let GraphPattern::Bgp { patterns: pr } = &r {
        if pr.is_empty() {
            return l;
        }
    }

    match (l, r) {
        (GraphPattern::Bgp { patterns: mut pl }, GraphPattern::Bgp { patterns: pr }) => {
            pl.extend(pr);
            GraphPattern::Bgp { patterns: pl }
        }
        (GraphPattern::Bgp { patterns }, other) | (other, GraphPattern::Bgp { patterns })
            if patterns.is_empty() =>
        {
            other
        }
        (l, r) => GraphPattern::Join {
            left: Box::new(l),
            right: Box::new(r),
        },
    }
}

#[derive(Default)]
enum SelectionOption {
    Distinct,
    Reduced,
    #[default]
    Default,
}

enum SelectionMember {
    Variable(Variable),
    Expression(Expression, Variable),
}

#[derive(Default)]
enum SelectionVariables {
    Explicit(Vec<SelectionMember>),
    #[default]
    Star,
}

#[derive(Default)]
struct Selection {
    pub option: SelectionOption,
    pub variables: SelectionVariables,
}

fn build_select(
    select: Selection,
    r#where: GraphPattern,
    mut group: Option<(Vec<Variable>, Vec<(Expression, Variable)>)>,
    having: Option<Expression>,
    order_by: Option<Vec<OrderExpression>>,
    offset_limit: Option<(usize, Option<usize>)>,
    values: Option<GraphPattern>,
    state: &RefCell<ParserState>,
) -> Result<GraphPattern, String> {
    let mut p = r#where;
    let mut with_aggregate = false;

    // GROUP BY
    let aggregates = take(&mut state.borrow_mut().aggregates);
    if group.is_none() && !aggregates.is_empty() {
        group = Some((vec![], vec![]));
    }

    if let Some((clauses, binds)) = group {
        for (expression, variable) in binds {
            p = GraphPattern::Extend {
                inner: Box::new(p),
                variable,
                expression,
            };
        }
        p = GraphPattern::Group {
            inner: Box::new(p),
            variables: clauses,
            aggregates,
        };
        with_aggregate = true;
    }

    // HAVING
    if let Some(expr) = having {
        p = GraphPattern::Filter {
            expr,
            inner: Box::new(p),
        };
    }

    // VALUES
    if let Some(data) = values {
        p = new_join(p, data);
    }

    // SELECT
    let mut pv = Vec::new();
    let with_project = match select.variables {
        SelectionVariables::Explicit(sel_items) => {
            let mut visible = HashSet::new();
            p.on_in_scope_variable(|v| {
                visible.insert(v.clone());
            });
            for sel_item in sel_items {
                let v = match sel_item {
                    SelectionMember::Variable(v) => {
                        if with_aggregate && !visible.contains(&v) {
                            // We validate projection variables if there is an aggregate
                            return Err(format!("The SELECT variable {v} is unbound"));
                        }
                        v
                    }
                    SelectionMember::Expression(expression, variable) => {
                        if visible.contains(&variable) {
                            // We disallow to override an existing variable with an expression
                            return Err(format!(
                                "The SELECT overrides {variable} using an expression even if it's already used"
                            ));
                        }
                        if with_aggregate {
                            // We validate projection variables if there is an aggregate
                            if let Some(v) = find_unbound_variable(&expression, &visible) {
                                return Err(format!(
                                    "The variable {v} is unbound in a SELECT expression",
                                ));
                            }
                        }
                        p = GraphPattern::Extend {
                            inner: Box::new(p),
                            variable: variable.clone(),
                            expression,
                        };
                        variable
                    }
                };
                if pv.contains(&v) {
                    return Err(format!("{v} is declared twice in SELECT"));
                }
                pv.push(v)
            }
            true
        }
        SelectionVariables::Star => {
            if with_aggregate {
                return Err(format!("SELECT * is not authorized if GROUP BY is present"));
            }
            // TODO: is it really useful to do a projection?
            p.on_in_scope_variable(|v| {
                if !pv.contains(v) {
                    pv.push(v.clone());
                }
            });
            pv.sort();
            true
        }
    };

    let mut m = p;

    // ORDER BY
    if let Some(expression) = order_by {
        m = GraphPattern::OrderBy {
            inner: Box::new(m),
            expression,
        };
    }

    // PROJECT
    if with_project {
        m = GraphPattern::Project {
            inner: Box::new(m),
            variables: pv,
        };
    }
    match select.option {
        SelectionOption::Distinct => m = GraphPattern::Distinct { inner: Box::new(m) },
        SelectionOption::Reduced => m = GraphPattern::Reduced { inner: Box::new(m) },
        SelectionOption::Default => (),
    }

    // OFFSET LIMIT
    if let Some((start, length)) = offset_limit {
        m = GraphPattern::Slice {
            inner: Box::new(m),
            start,
            length,
        }
    }
    Ok(m)
}

fn find_unbound_variable<'a>(
    expression: &'a Expression,
    variables: &HashSet<Variable>,
) -> Option<&'a Variable> {
    match expression {
        Expression::NamedNode(_)
        | Expression::Literal(_)
        | Expression::Bound(_)
        | Expression::Coalesce(_)
        | Expression::Exists(_) => None,
        Expression::Variable(var) => variables.contains(var).then_some(var),
        Expression::UnaryPlus(e) | Expression::UnaryMinus(e) | Expression::Not(e) => {
            find_unbound_variable(e, variables)
        }
        Expression::Or(a, b)
        | Expression::And(a, b)
        | Expression::Equal(a, b)
        | Expression::SameTerm(a, b)
        | Expression::Greater(a, b)
        | Expression::GreaterOrEqual(a, b)
        | Expression::Less(a, b)
        | Expression::LessOrEqual(a, b)
        | Expression::Add(a, b)
        | Expression::Subtract(a, b)
        | Expression::Multiply(a, b)
        | Expression::Divide(a, b) => {
            find_unbound_variable(a, variables)?;
            find_unbound_variable(b, variables)
        }
        Expression::In(a, b) => {
            find_unbound_variable(a, variables)?;
            b.iter()
                .filter_map(|b| find_unbound_variable(b, variables))
                .next()
        }
        Expression::FunctionCall(_, parameters) => parameters
            .iter()
            .filter_map(|p| find_unbound_variable(p, variables))
            .next(),
        Expression::If(a, b, c) => {
            find_unbound_variable(a, variables)?;
            find_unbound_variable(b, variables)?;
            find_unbound_variable(c, variables)
        }
    }
}

/// Called on every variable defined using "AS" or "VALUES"
#[cfg(feature = "sep-0006")]
fn add_defined_variables<'a>(pattern: &'a GraphPattern, set: &mut HashSet<&'a Variable>) {
    match pattern {
        GraphPattern::Bgp { .. } | GraphPattern::Path { .. } => {}
        GraphPattern::Join { left, right }
        | GraphPattern::LeftJoin { left, right, .. }
        | GraphPattern::Lateral { left, right }
        | GraphPattern::Union { left, right }
        | GraphPattern::Minus { left, right } => {
            add_defined_variables(left, set);
            add_defined_variables(right, set);
        }
        GraphPattern::Graph { inner, .. } => {
            add_defined_variables(inner, set);
        }
        GraphPattern::Extend {
            inner, variable, ..
        } => {
            set.insert(variable);
            add_defined_variables(inner, set);
        }
        GraphPattern::Group {
            variables,
            aggregates,
            inner,
        } => {
            for (v, _) in aggregates {
                set.insert(v);
            }
            let mut inner_variables = HashSet::new();
            add_defined_variables(inner, &mut inner_variables);
            for v in inner_variables {
                if variables.contains(v) {
                    set.insert(v);
                }
            }
        }
        GraphPattern::Values { variables, .. } => {
            for v in variables {
                set.insert(v);
            }
        }
        GraphPattern::Project { variables, inner } => {
            let mut inner_variables = HashSet::new();
            add_defined_variables(inner, &mut inner_variables);
            for v in inner_variables {
                if variables.contains(v) {
                    set.insert(v);
                }
            }
        }
        GraphPattern::Service { inner, .. }
        | GraphPattern::Filter { inner, .. }
        | GraphPattern::OrderBy { inner, .. }
        | GraphPattern::Distinct { inner }
        | GraphPattern::Reduced { inner }
        | GraphPattern::Slice { inner, .. } => add_defined_variables(inner, set),
    }
}

fn copy_graph(from: impl Into<GraphName>, to: impl Into<GraphNamePattern>) -> GraphUpdateOperation {
    let bgp = GraphPattern::Bgp {
        patterns: vec![TriplePattern::new(
            Variable::new_unchecked("s"),
            Variable::new_unchecked("p"),
            Variable::new_unchecked("o"),
        )],
    };
    GraphUpdateOperation::DeleteInsert {
        delete: Vec::new(),
        insert: vec![QuadPattern::new(
            Variable::new_unchecked("s"),
            Variable::new_unchecked("p"),
            Variable::new_unchecked("o"),
            to,
        )],
        using: None,
        pattern: Box::new(match from.into() {
            GraphName::NamedNode(from) => GraphPattern::Graph {
                name: from.into(),
                inner: Box::new(bgp),
            },
            GraphName::DefaultGraph => bgp,
        }),
    }
}

fn check_if_insert_data_are_sharing_blank_nodes(
    update: &[GraphUpdateOperation],
) -> Result<(), SparqlSyntaxError> {
    #[cfg(feature = "sparql-12")]
    fn add_triple_blank_nodes<'a>(triple: &'a Triple, bnodes: &mut HashSet<&'a BlankNode>) {
        if let NamedOrBlankNode::BlankNode(bnode) = &triple.subject {
            bnodes.insert(bnode);
        }
        if let Term::BlankNode(bnode) = &triple.object {
            bnodes.insert(bnode);
        } else if let Term::Triple(triple) = &triple.object {
            add_triple_blank_nodes(triple, bnodes);
        }
    }

    if update
        .iter()
        .filter(|op| matches!(op, GraphUpdateOperation::InsertData { .. }))
        .count()
        < 2
    {
        // Fast path, no need to validate
        return Ok(());
    }

    let mut existing_blank_nodes = HashSet::new();
    for operation in update {
        if let GraphUpdateOperation::InsertData { data } = operation {
            let mut new_blank_nodes = HashSet::new();
            for quad in data {
                if let NamedOrBlankNode::BlankNode(bnode) = &quad.subject {
                    new_blank_nodes.insert(bnode);
                }
                if let Term::BlankNode(bnode) = &quad.object {
                    new_blank_nodes.insert(bnode);
                }
                #[cfg(feature = "sparql-12")]
                if let Term::Triple(triple) = &quad.object {
                    add_triple_blank_nodes(triple, &mut new_blank_nodes);
                }
            }
            if let Some(error) = existing_blank_nodes.intersection(&new_blank_nodes).next() {
                return Err(SparqlSyntaxErrorKind::SharedBlankNode((**error).clone()).into());
            }
            existing_blank_nodes.extend(new_blank_nodes);
        }
    }
    Ok(())
}

enum Either<L, R> {
    Left(L),
    Right(R),
}

pub struct ParserState {
    base_iri: Option<Iri<String>>,
    prefixes: HashMap<String, String>,
    custom_aggregate_functions: HashSet<NamedNode>,
    used_bnodes: HashSet<BlankNode>,
    currently_used_bnodes: HashSet<BlankNode>,
    aggregates: Vec<(Variable, AggregateExpression)>,
}

impl ParserState {
    pub(crate) fn new(
        base_iri: Option<Iri<String>>,
        prefixes: HashMap<String, String>,
        custom_aggregate_functions: HashSet<NamedNode>,
    ) -> Self {
        Self {
            base_iri,
            prefixes,
            custom_aggregate_functions,
            used_bnodes: HashSet::new(),
            currently_used_bnodes: HashSet::new(),
            aggregates: Vec::new(),
        }
    }

    fn parse_iri(&self, iri: String) -> Result<Iri<String>, IriParseError> {
        if let Some(base_iri) = &self.base_iri {
            base_iri.resolve(&iri)
        } else {
            Iri::parse(iri)
        }
    }

    fn new_aggregation(&mut self, agg: AggregateExpression) -> Result<Variable, &'static str> {
        Ok(self
            .aggregates
            .iter()
            .find_map(|(v, a)| (a == &agg).then_some(v))
            .cloned()
            .unwrap_or_else(|| {
                let new_var = variable();
                self.aggregates.push((new_var.clone(), agg));
                new_var
            }))
    }
}

fn unescape_iriref(mut input: &str) -> Result<String, &'static str> {
    let mut output = String::with_capacity(input.len());
    while let Some((before, after)) = input.split_once('\\') {
        output.push_str(before);
        let mut after = after.chars();
        let (escape, after) = match after.next() {
            Some('u') => read_hex_char::<4>(after.as_str())?,
            Some('U') => read_hex_char::<8>(after.as_str())?,
            Some(_) => {
                return Err(
                    "IRIs are only allowed to contain escape sequences \\uXXXX and \\UXXXXXXXX",
                );
            }
            None => return Err("IRIs are not allowed to end with a '\'"),
        };
        output.push(escape);
        input = after;
    }
    output.push_str(input);
    Ok(output)
}

fn unescape_string(mut input: &str) -> Result<String, &'static str> {
    let mut output = String::with_capacity(input.len());
    while let Some((before, after)) = input.split_once('\\') {
        output.push_str(before);
        let mut after = after.chars();
        let (escape, after) = match after.next() {
            Some('t') => ('\u{0009}', after.as_str()),
            Some('b') => ('\u{0008}', after.as_str()),
            Some('n') => ('\u{000A}', after.as_str()),
            Some('r') => ('\u{000D}', after.as_str()),
            Some('f') => ('\u{000C}', after.as_str()),
            Some('"') => ('\u{0022}', after.as_str()),
            Some('\'') => ('\u{0027}', after.as_str()),
            Some('\\') => ('\u{005C}', after.as_str()),
            Some('u') => read_hex_char::<4>(after.as_str())?,
            Some('U') => read_hex_char::<8>(after.as_str())?,
            Some(_) => return Err("The character that can be escaped in strings are tbnrf\"'\\"),
            None => return Err("strings are not allowed to end with a '\'"),
        };
        output.push(escape);
        input = after;
    }
    output.push_str(input);
    Ok(output)
}

fn read_hex_char<const SIZE: usize>(input: &str) -> Result<(char, &str), &'static str> {
    if let Some(escape) = input.get(..SIZE) {
        if let Some(char) = u32::from_str_radix(escape, 16)
            .ok()
            .and_then(char::from_u32)
        {
            Ok((char, &input[SIZE..]))
        } else {
            Err("\\u escape sequence should be followed by hexadecimal digits")
        }
    } else {
        Err("\\u escape sequence should be followed by hexadecimal digits")
    }
}

fn variable() -> Variable {
    Variable::new_unchecked(format!("{:x}", random::<u128>()))
}

fn function_from_name(name: &str) -> Option<Function> {
    // TODO: put it in the parser as keyword(value).map(|()| Function::value)
    if name.eq_ignore_ascii_case("STR") {
        return Some(Function::Str);
    }
    if name.eq_ignore_ascii_case("LANG") {
        return Some(Function::Lang);
    }
    if name.eq_ignore_ascii_case("LANGMATCHES") {
        return Some(Function::LangMatches);
    }
    #[cfg(feature = "sparql-12")]
    if name.eq_ignore_ascii_case("LANGDIR") {
        return Some(Function::LangDir);
    }
    if name.eq_ignore_ascii_case("DATATYPE") {
        return Some(Function::Datatype);
    }
    if name.eq_ignore_ascii_case("IRI") || name.eq_ignore_ascii_case("URI") {
        return Some(Function::Iri);
    }
    if name.eq_ignore_ascii_case("BNODE") {
        return Some(Function::BNode);
    }
    if name.eq_ignore_ascii_case("RAND") {
        return Some(Function::Rand);
    }
    if name.eq_ignore_ascii_case("ABS") {
        return Some(Function::Abs);
    }
    if name.eq_ignore_ascii_case("CEIL") {
        return Some(Function::Ceil);
    }
    if name.eq_ignore_ascii_case("FLOOR") {
        return Some(Function::Floor);
    }
    if name.eq_ignore_ascii_case("ROUND") {
        return Some(Function::Round);
    }
    if name.eq_ignore_ascii_case("CONCAT") {
        return Some(Function::Concat);
    }
    if name.eq_ignore_ascii_case("SUBSTR") {
        return Some(Function::SubStr);
    }
    if name.eq_ignore_ascii_case("STRLEN") {
        return Some(Function::StrLen);
    }
    if name.eq_ignore_ascii_case("REPLACE") {
        return Some(Function::Replace);
    }
    if name.eq_ignore_ascii_case("UCASE") {
        return Some(Function::UCase);
    }
    if name.eq_ignore_ascii_case("LCASE") {
        return Some(Function::LCase);
    }
    if name.eq_ignore_ascii_case("ENCODE_FOR_URI") {
        return Some(Function::EncodeForUri);
    }
    if name.eq_ignore_ascii_case("CONTAINS") {
        return Some(Function::Contains);
    }
    if name.eq_ignore_ascii_case("STRSTARTS") {
        return Some(Function::StrStarts);
    }
    if name.eq_ignore_ascii_case("STRENDS") {
        return Some(Function::StrEnds);
    }
    if name.eq_ignore_ascii_case("STRBEFORE") {
        return Some(Function::StrBefore);
    }
    if name.eq_ignore_ascii_case("STRAFTER") {
        return Some(Function::StrAfter);
    }
    if name.eq_ignore_ascii_case("YEAR") {
        return Some(Function::Year);
    }
    if name.eq_ignore_ascii_case("MONTH") {
        return Some(Function::Month);
    }
    if name.eq_ignore_ascii_case("DAY") {
        return Some(Function::Day);
    }
    if name.eq_ignore_ascii_case("HOURS") {
        return Some(Function::Hours);
    }
    if name.eq_ignore_ascii_case("MINUTES") {
        return Some(Function::Minutes);
    }
    if name.eq_ignore_ascii_case("SECONDS") {
        return Some(Function::Seconds);
    }
    if name.eq_ignore_ascii_case("TIMEZONE") {
        return Some(Function::Timezone);
    }
    if name.eq_ignore_ascii_case("TZ") {
        return Some(Function::Tz);
    }
    if name.eq_ignore_ascii_case("NOW") {
        return Some(Function::Now);
    }
    if name.eq_ignore_ascii_case("UUID") {
        return Some(Function::Uuid);
    }
    if name.eq_ignore_ascii_case("STRUUID") {
        return Some(Function::StrUuid);
    }
    if name.eq_ignore_ascii_case("MD5") {
        return Some(Function::Md5);
    }
    if name.eq_ignore_ascii_case("SHA1") {
        return Some(Function::Sha1);
    }
    if name.eq_ignore_ascii_case("SHA256") {
        return Some(Function::Sha256);
    }
    if name.eq_ignore_ascii_case("SHA384") {
        return Some(Function::Sha384);
    }
    if name.eq_ignore_ascii_case("SHA512") {
        return Some(Function::Sha512);
    }
    if name.eq_ignore_ascii_case("STRLANG") {
        return Some(Function::StrLang);
    }
    #[cfg(feature = "sparql-12")]
    if name.eq_ignore_ascii_case("STRLANGDIR") {
        return Some(Function::StrLangDir);
    }
    if name.eq_ignore_ascii_case("STRDT") {
        return Some(Function::StrDt);
    }
    if name.eq_ignore_ascii_case("isIRI") || name.eq_ignore_ascii_case("isURI") {
        return Some(Function::IsIri);
    }
    if name.eq_ignore_ascii_case("isBLANK") {
        return Some(Function::IsBlank);
    }
    if name.eq_ignore_ascii_case("isLITERAL") {
        return Some(Function::IsLiteral);
    }
    if name.eq_ignore_ascii_case("isNUMERIC") {
        return Some(Function::IsNumeric);
    }
    #[cfg(feature = "sparql-12")]
    if name.eq_ignore_ascii_case("hasLANG") {
        return Some(Function::HasLang);
    }
    #[cfg(feature = "sparql-12")]
    if name.eq_ignore_ascii_case("hasLANGDIR") {
        return Some(Function::HasLangDir);
    }
    if name.eq_ignore_ascii_case("REGEX") {
        return Some(Function::Regex);
    }
    #[cfg(feature = "sparql-12")]
    if name.eq_ignore_ascii_case("isTRIPLE") {
        return Some(Function::IsTriple);
    }
    #[cfg(feature = "sparql-12")]
    if name.eq_ignore_ascii_case("TRIPLE") {
        return Some(Function::Triple);
    }
    #[cfg(feature = "sparql-12")]
    if name.eq_ignore_ascii_case("SUBJECT") {
        return Some(Function::Subject);
    }
    #[cfg(feature = "sparql-12")]
    if name.eq_ignore_ascii_case("PREDICATE") {
        return Some(Function::Predicate);
    }
    #[cfg(feature = "sparql-12")]
    if name.eq_ignore_ascii_case("OBJECT") {
        return Some(Function::Object);
    }
    None
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Token<'a> {
    IriRef(&'a str),
    PnameLn(&'a str, &'a str),
    PnameNs(&'a str),
    BlankNodeLabel(&'a str),
    StringLiteral1(&'a str),
    StringLiteral2(&'a str),
    StringLiteralLong1(&'a str),
    StringLiteralLong2(&'a str),
    LangDir(&'a str),
    Integer(&'a str),
    Decimal(&'a str),
    Double(&'a str),
    IntegerPositive(&'a str),
    DecimalPositive(&'a str),
    DoublePositive(&'a str),
    IntegerNegative(&'a str),
    DecimalNegative(&'a str),
    DoubleNegative(&'a str),
    Var1(&'a str),
    Var2(&'a str),
    Nil,
    Anon,
    Keyword(&'a str),
    Operator(&'a str),
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IriRef(iri) => write!(f, "<{iri}>"),
            Self::PnameLn(k, v) => write!(f, "{k}:{v}"),
            Self::PnameNs(k) => write!(f, "{k}:"),
            Self::BlankNodeLabel(id) => write!(f, "_:{id}"),
            Self::StringLiteral1(s) => write!(f, "'{s}'"),
            Self::StringLiteral2(s) => write!(f, "\"{s}\""),
            Self::StringLiteralLong1(s) => write!(f, "'''{s}'''"),
            Self::StringLiteralLong2(s) => write!(f, "\"\"\"{s}\"\"\""),
            Self::LangDir(lang) => write!(f, "@{lang}"),
            Self::Var1(v) => write!(f, "?{v}"),
            Self::Var2(v) => write!(f, "${v}"),
            Self::Nil => f.write_str("()"),
            Self::Anon => f.write_str("[]"),
            Self::Integer(v)
            | Self::Decimal(v)
            | Self::Double(v)
            | Self::IntegerPositive(v)
            | Self::DecimalPositive(v)
            | Self::DoublePositive(v)
            | Self::IntegerNegative(v)
            | Self::DecimalNegative(v)
            | Self::DoubleNegative(v)
            | Self::Keyword(v)
            | Self::Operator(v) => f.write_str(v),
        }
    }
}

fn query_unit<'src, I: ValueInput<'src, Token = Token<'src>, Span = SimpleSpan>>(
    state: Rc<RefCell<ParserState>>,
) -> impl Parser<'src, I, Query, extra::Err<Rich<'src, Token<'src>, SimpleSpan>>> {
    let nil = select! { Token::Nil => () };

    // [158]   	BlankNode 	  ::=   	BLANK_NODE_LABEL | ANON
    let blank_node = select! {
        Token::BlankNodeLabel(id) => Some(id),
        Token::Anon => None
    }
    .try_map({
        let state = Rc::clone(&state);
        move |id, span| {
            let Some(id) = id else {
                return Ok(BlankNode::default());
            };
            let node = BlankNode::new_unchecked(id);
            let mut state = state.borrow_mut();
            if state.used_bnodes.contains(&node) {
                Err(Rich::custom(
                    span,
                    format!("{node} has already been use used in an other graph pattern"),
                ))
            } else {
                state.currently_used_bnodes.insert(node.clone());
                Ok(node)
            }
        }
    });

    // [157]   	PrefixedName 	  ::=   	PNAME_LN | PNAME_NS
    // [156]   	iri 	  ::=   	IRIREF | PrefixedName
    let iri = select! {
        Token::IriRef(v) => (None, Some(v)),
        Token::PnameLn(k, v) => (Some(k), Some(v)),
        Token::PnameNs(k) => (Some(k), None)
    }
    .try_map({
        let state = Rc::clone(&state);
        move |(ns, local), span| {
            let state = state.borrow();
            let Some(ns) = ns else {
                let iri = local.unwrap();
                return if let Some(base_iri) = &state.base_iri {
                    Ok(NamedNode::new_unchecked(
                        base_iri
                            .resolve(&iri)
                            .map_err(|e| Rich::custom(span, format!("Failed to parse IRI: {e}")))?
                            .into_inner(),
                    ))
                } else {
                    NamedNode::new(iri)
                        .map_err(|e| Rich::custom(span, format!("Failed to parse IRI: {e}")))
                };
            };
            let base = state
                .prefixes
                .get(ns)
                .ok_or_else(|| Rich::custom(span, format!("Prefix {ns}: not found")))?;
            let Some(local) = local else {
                return Ok(NamedNode::new_unchecked(ns));
            };
            let mut iri = String::with_capacity(base.len() + local.len());
            iri.push_str(base);
            for chunk in local.split('\\') {
                // We remove \
                iri.push_str(chunk);
            }
            NamedNode::new(local)
                .map_err(|e| Rich::custom(span, format!("Failed to parse IRI: {e}")))
        }
    });

    // [155]   	String 	  ::=   	STRING_LITERAL1 | STRING_LITERAL2 | STRING_LITERAL_LONG1 | STRING_LITERAL_LONG2
    let string = select! {
        Token::StringLiteral1(s) => s,
        Token::StringLiteral2(s) => s,
        Token::StringLiteralLong1(s) => s,
        Token::StringLiteralLong2(s) => s,
    };

    // [154]   	BooleanLiteral 	  ::=   	'true' | 'false'
    let boolean_literal = case_sensitive_keyword("true")
        .map(|()| Literal::new_typed_literal("true", xsd::BOOLEAN))
        .or(case_sensitive_keyword("false")
            .map(|()| Literal::new_typed_literal("false", xsd::BOOLEAN)));

    // [153]   	NumericLiteralNegative 	  ::=   	INTEGER_NEGATIVE | DECIMAL_NEGATIVE | DOUBLE_NEGATIVE
    let numeric_literal_negative = select! {
        Token::IntegerNegative(v) => Literal::new_typed_literal(v, xsd::INTEGER),
        Token::DecimalNegative(v) => Literal::new_typed_literal(v, xsd::DECIMAL),
        Token::DoubleNegative(v) => Literal::new_typed_literal(v, xsd::DOUBLE),
    };

    // [152]   	NumericLiteralPositive 	  ::=   	INTEGER_POSITIVE | DECIMAL_POSITIVE | DOUBLE_POSITIVE
    let numeric_literal_positive = select! {
        Token::IntegerPositive(v) => Literal::new_typed_literal(v, xsd::INTEGER),
        Token::DecimalPositive(v) => Literal::new_typed_literal(v, xsd::DECIMAL),
        Token::DoublePositive(v) => Literal::new_typed_literal(v, xsd::DOUBLE),
    };

    // [151]   	NumericLiteralUnsigned 	  ::=   	INTEGER | DECIMAL | DOUBLE
    let numeric_literal_unsigned = select! {
        Token::Integer(v) => Literal::new_typed_literal(v, xsd::INTEGER),
        Token::Decimal(v) => Literal::new_typed_literal(v, xsd::DECIMAL),
        Token::Double(v) => Literal::new_typed_literal(v, xsd::DOUBLE),
    };

    // [150]   	NumericLiteral 	  ::=   	NumericLiteralUnsigned | NumericLiteralPositive | NumericLiteralNegative
    let numeric_literal = numeric_literal_unsigned
        .or(numeric_literal_positive)
        .or(numeric_literal_negative);

    // [149]   	RDFLiteral 	  ::=   	String ( LANG_DIR | '^^' iri )?
    let rdf_literal = string
        .then(
            select! {
                Token::LangDir(l) => Either::Left(l),
            }
            .or(operator("^^").ignore_then(iri.clone()).map(Either::Right))
            .or_not(),
        )
        .try_map(|(string, extra), span| match extra {
            Some(Either::Left(l)) => Literal::new_language_tagged_literal(string, l)
                .map_err(|e| Rich::custom(span, format!("Failed to parse language tag: {e}"))),
            Some(Either::Right(t)) => Ok(Literal::new_typed_literal(string, t)),
            None => Ok(Literal::new_simple_literal(string)),
        });

    // [126]   	Var 	  ::=   	VAR1 | VAR2
    let var = select! {
        Token::Var1(v) => Variable::new_unchecked(v),
        Token::Var2(v) => Variable::new_unchecked(v),
    };

    let mut expression = Recursive::declare();

    // [78]   	ExpressionList 	  ::=   	NIL | '(' Expression ( ',' Expression )* ')'
    let expression_list = nil.map(|()| Vec::new()).or(expression
        .clone()
        .separated_by(operator(","))
        .at_least(1)
        .collect()
        .delimited_by(operator("("), operator(")")));

    // [148]   	iriOrFunction 	  ::=   	iri ArgList?
    let iri_or_function = iri.clone().then(expression_list.clone().or_not()).try_map({
        let state = Rc::clone(&state);
        move |(name, args), span| {
            if let Some(args) = args {
                if state.borrow().custom_aggregate_functions.contains(&name) {
                    Err(Rich::custom(
                        span,
                        format!("{name} is an aggregate function and not a regular function"),
                    ))
                } else {
                    Ok(Expression::FunctionCall(Function::Custom(name), args))
                }
            } else {
                Ok(Expression::NamedNode(name))
            }
        }
    });

    // [140]   	BrackettedExpression 	  ::=   	'(' Expression ')'
    let bracketted_expression = expression
        .clone()
        .delimited_by(operator("("), operator(")"));

    // [147]   	Aggregate 	  ::=   	  'COUNT' '(' 'DISTINCT'? ( '*' | Expression ) ')' | 'SUM' '(' 'DISTINCT'? Expression ')' | 'MIN' '(' 'DISTINCT'? Expression ')' | 'MAX' '(' 'DISTINCT'? Expression ')' | 'AVG' '(' 'DISTINCT'? Expression ')' | 'SAMPLE' '(' 'DISTINCT'? Expression ')' | 'GROUP_CONCAT' '(' 'DISTINCT'? Expression ( ';' 'SEPARATOR' '=' String )? ')'
    let aggregate = keyword("count")
        .ignore_then(keyword("distinct").or_not())
        .then_ignore(operator("*"))
        .map(|distinct| AggregateExpression::CountSolutions {
            distinct: distinct.is_some(),
        })
        .or(keyword("count")
            .map(|()| AggregateFunction::Sum)
            .or(keyword("sum").map(|()| AggregateFunction::Sum))
            .or(keyword("min").map(|()| AggregateFunction::Min))
            .or(keyword("max").map(|()| AggregateFunction::Max))
            .or(keyword("avg").map(|()| AggregateFunction::Avg))
            .or(keyword("sample").map(|()| AggregateFunction::Sample))
            .then(keyword("distinct").or_not())
            .then(
                expression
                    .clone()
                    .delimited_by(operator("("), operator(")")),
            )
            .map(
                |((name, distinct), expr)| AggregateExpression::FunctionCall {
                    name,
                    expr,
                    distinct: distinct.is_some(),
                },
            ))
        .or(keyword("group_concat")
            .ignore_then(keyword("distinct").or_not())
            .then(
                expression
                    .clone()
                    .then(
                        operator(";")
                            .ignore_then(keyword("separator"))
                            .ignore_then(operator("="))
                            .ignore_then(string.clone())
                            .or_not(),
                    )
                    .delimited_by(operator("("), operator(")")),
            )
            .map(
                |(distinct, (expr, separator))| AggregateExpression::FunctionCall {
                    name: AggregateFunction::GroupConcat {
                        separator: separator.map(Into::into),
                    },
                    expr,
                    distinct: distinct.is_some(),
                },
            ));

    // [145]   	ExistsFunc 	  ::=   	'EXISTS' GroupGraphPattern
    // [146]   	NotExistsFunc 	  ::=   	'NOT' 'EXISTS' GroupGraphPattern
    let mut group_graph_pattern = Recursive::declare();
    let exists = keyword("NOT")
        .ignored()
        .or_not()
        .then_ignore(keyword("EXISTS"))
        .then(group_graph_pattern.clone())
        .map(|(neg, e)| {
            let e = Expression::Exists(Box::new(e));
            if neg.is_some() {
                Expression::Not(Box::new(e))
            } else {
                e
            }
        });

    // [141]   	BuiltInCall 	  ::=   	  Aggregate | 'STR' '(' Expression ')' | 'LANG' | 'LANGMATCHES' '(' Expression ',' Expression ')' | 'LANGDIR' '(' Expression ')' | 'DATATYPE' '(' Expression ')' | 'BOUND' '(' Var ')' | 'IRI' '(' Expression ')' | 'URI' '(' Expression ')' | 'BNODE' ( '(' Expression ')' | NIL ) | 'RAND' NIL | 'ABS' '(' Expression ')' | 'CEIL' '(' Expression ')' | 'FLOOR' '(' Expression ')' | 'ROUND' '(' Expression ')' | 'CONCAT' ExpressionList | SubstringExpression | 'STRLEN' '(' Expression ')' | StrReplaceExpression | 'UCASE' '(' Expression ')' | 'LCASE' '(' Expression ')' | 'ENCODE_FOR_URI' '(' Expression ')' | 'CONTAINS' '(' Expression ',' Expression ')' | 'STRSTARTS' '(' Expression ',' Expression ')' | 'STRENDS' '(' Expression ',' Expression ')' | 'STRBEFORE' '(' Expression ',' Expression ')' | 'STRAFTER' '(' Expression ',' Expression ')' | 'YEAR' '(' Expression ')' | 'MONTH' '(' Expression ')' | 'DAY' '(' Expression ')' | 'HOURS' '(' Expression ')' | 'MINUTES' '(' Expression ')' | 'SECONDS' '(' Expression ')' | 'TIMEZONE' '(' Expression ')' | 'TZ' '(' Expression ')' | 'NOW' NIL | 'UUID' NIL | 'STRUUID' NIL | 'MD5' '(' Expression ')' | 'SHA1' '(' Expression ')' | 'SHA256' '(' Expression ')' | 'SHA384' '(' Expression ')' | 'SHA512' '(' Expression ')' | 'COALESCE' ExpressionList | 'IF' '(' Expression ',' Expression ',' Expression ')' | 'STRLANG' '(' Expression ',' Expression ')' | 'STRLANGDIR' '(' Expression ',' Expression ',' Expression ')' | 'STRDT' '(' Expression ',' Expression ')' | 'sameTerm' '(' Expression ',' Expression ')' | 'isIRI' '(' Expression ')' | 'isURI' '(' Expression ')' | 'isBLANK' '(' Expression ')' | 'isLITERAL' '(' Expression ')' | 'isNUMERIC' '(' Expression ')' | 'hasLANG' '(' Expression ')' | 'hasLANGDIR' '(' Expression ')' | RegexExpression | ExistsFunc | NotExistsFunc | 'isTRIPLE' '(' Expression ')' | 'TRIPLE' '(' Expression ',' Expression ',' Expression ')' | 'SUBJECT' '(' Expression ')' | 'PREDICATE' '(' Expression ')' | 'OBJECT' '(' Expression ')'
    // [142]   	RegexExpression 	  ::=   	'REGEX' '(' Expression ',' Expression ( ',' Expression )? ')'
    // [143]   	SubstringExpression 	  ::=   	'SUBSTR' '(' Expression ',' Expression ( ',' Expression )? ')'
    // [144]   	StrReplaceExpression 	  ::=   	'REPLACE' '(' Expression ',' Expression ',' Expression ( ',' Expression )? ')'
    let built_in_call = aggregate
        .try_map({
            let state = Rc::clone(&state);
            move |a, span| {
                Ok(state
                    .borrow_mut()
                    .new_aggregation(a)
                    .map_err(|e| Rich::custom(span, e))?
                    .into())
            }
        })
        .or(keyword("BOUND")
            .ignore_then(var.delimited_by(operator("("), operator(")")))
            .map(Expression::Bound))
        .or(keyword("COALESCE")
            .ignore_then(expression_list.clone())
            .map(|e| Expression::Coalesce(e)))
        .or(keyword("IF")
            .ignore_then(
                expression
                    .clone()
                    .then_ignore(operator(","))
                    .then(expression.clone())
                    .then_ignore(operator(","))
                    .then(expression.clone())
                    .delimited_by(operator("("), operator(")")),
            )
            .map(|((a, b), c)| Expression::If(Box::new(a), Box::new(b), Box::new(c))))
        .or(keyword("sameTerm")
            .ignore_then(
                expression
                    .clone()
                    .then_ignore(operator(","))
                    .then(expression.clone())
                    .delimited_by(operator("("), operator(")")),
            )
            .map(|(a, b)| Expression::SameTerm(Box::new(a), Box::new(b))))
        .or(select! { Token::Keyword(name) => name }
            .then(expression_list.clone())
            .try_map(|(name, args), span| {
                let Some(name) = function_from_name(name) else {
                    return Err(Rich::custom(
                        span,
                        format!("The built-in function {name} does not exist"),
                    ));
                };
                // TODO: check cardinality
                Ok(Expression::FunctionCall(name, args))
            }))
        .or(exists);

    // [136]   	PrimaryExpression 	  ::=   	BrackettedExpression | BuiltInCall | iriOrFunction | RDFLiteral | NumericLiteral | BooleanLiteral | Var | ExprTripleTerm
    let primary_expression = bracketted_expression
        .clone()
        .or(built_in_call.clone())
        .or(iri_or_function)
        .or(rdf_literal.clone().map(Expression::Literal))
        .or(numeric_literal.clone().map(Expression::Literal))
        .or(boolean_literal.clone().map(Expression::Literal))
        .or(var.map(Expression::Variable)); // TODO ExprTripleTerm

    expression.define(
        primary_expression.boxed().pratt((
            // [127]   	Expression 	  ::=   	ConditionalOrExpression

            // [128]   	ConditionalOrExpression 	  ::=   	ConditionalAndExpression ( '||' ConditionalAndExpression )*
            infix(left(1), operator("||"), |l, _, r, _| {
                Expression::Or(Box::new(l), Box::new(r))
            }),
            // [129]   	ConditionalAndExpression 	  ::=   	ValueLogical ( '&&' ValueLogical )*
            infix(left(2), operator("&&"), |l, _, r, _| {
                Expression::And(Box::new(l), Box::new(r))
            }),
            // [130]   	ValueLogical 	  ::=   	RelationalExpression
            // [131]   	RelationalExpression 	  ::=   	NumericExpression ( '=' NumericExpression | '!=' NumericExpression | '<' NumericExpression | '>' NumericExpression | '<=' NumericExpression | '>=' NumericExpression | 'IN' ExpressionList | 'NOT' 'IN' ExpressionList )?
            infix(left(3), operator("="), |l, _, r, _| {
                Expression::Equal(Box::new(l), Box::new(r))
            }),
            infix(left(3), operator("!="), |l, _, r, _| {
                Expression::Not(Box::new(Expression::Equal(Box::new(l), Box::new(r))))
            }),
            infix(left(3), operator("<"), |l, _, r, _| {
                Expression::Less(Box::new(l), Box::new(r))
            }),
            infix(left(3), operator(">"), |l, _, r, _| {
                Expression::Greater(Box::new(l), Box::new(r))
            }),
            infix(left(3), operator("<="), |l, _, r, _| {
                Expression::LessOrEqual(Box::new(l), Box::new(r))
            }),
            infix(left(3), operator("=>"), |l, _, r, _| {
                Expression::GreaterOrEqual(Box::new(l), Box::new(r))
            }),
            postfix(
                3,
                keyword("in").ignore_then(expression_list.clone()),
                |l, r, _| Expression::In(Box::new(l), r),
            ),
            postfix(
                3,
                keyword("not")
                    .ignore_then(keyword("in"))
                    .ignore_then(expression_list.clone()),
                |l, r, _| Expression::In(Box::new(l), r),
            ),
            // [132]   	NumericExpression 	  ::=   	AdditiveExpression
            // [133]   	AdditiveExpression 	  ::=   	MultiplicativeExpression ( '+' MultiplicativeExpression | '-' MultiplicativeExpression | ( NumericLiteralPositive | NumericLiteralNegative ) ( ( '*' UnaryExpression ) | ( '/' UnaryExpression ) )* )*
            infix(left(4), operator("+"), |l, _, r, _| {
                Expression::Add(Box::new(l), Box::new(r))
            }),
            infix(left(4), operator("-"), |l, _, r, _| {
                Expression::Subtract(Box::new(l), Box::new(r))
            }),
            // [134]   	MultiplicativeExpression 	  ::=   	UnaryExpression ( '*' UnaryExpression | '/' UnaryExpression )*
            infix(left(5), operator("*"), |l, _, r, _| {
                Expression::Multiply(Box::new(l), Box::new(r))
            }),
            infix(left(5), operator("/"), |l, _, r, _| {
                Expression::Divide(Box::new(l), Box::new(r))
            }),
            // [135]   	UnaryExpression 	  ::=   	  '!' UnaryExpression | '+' PrimaryExpression | '-' PrimaryExpression | PrimaryExpression
            prefix(
                6,
                one_of([
                    Token::Operator("!"),
                    Token::Operator("+"),
                    Token::Operator("-"),
                ]),
                |o, a, _| match o {
                    Token::Operator("!") => Expression::Not(Box::new(a)),
                    Token::Operator("+") => Expression::UnaryPlus(Box::new(a)),
                    Token::Operator("-") => Expression::UnaryMinus(Box::new(a)),
                    _ => unreachable!(),
                },
            ),
        )),
    );

    // [125]   	VarOrIri 	  ::=   	Var | iri
    let var_or_iri = var
        .map(NamedNodePattern::Variable)
        .or(iri.clone().map(NamedNodePattern::NamedNode));

    // [115]   	VarOrTerm 	  ::=   	Var | iri | RDFLiteral | NumericLiteral | BooleanLiteral | BlankNode | NIL | TripleTerm
    let var_or_term = var
        .map(TermPattern::Variable)
        .or(iri.clone().map(TermPattern::NamedNode))
        .or(rdf_literal.clone().map(TermPattern::Literal))
        .or(numeric_literal.map(TermPattern::Literal))
        .or(boolean_literal.clone().map(TermPattern::Literal))
        .or(blank_node.map(TermPattern::BlankNode))
        .or(nil.map(|()| rdf::NIL.into_owned().into()))
        .boxed(); // TODO: TripleTerm

    // [114]   	GraphNodePath 	  ::=   	VarOrTerm | TriplesNodePath | ReifiedTriple
    let mut triples_node_path = Recursive::declare();
    let graph_node_path = var_or_term
        .clone()
        .map(FocusedTripleOrPathPattern::<TermPattern>::new)
        .or(triples_node_path.clone()); // TODO: ReifiedTriple

    // [113]   	GraphNode 	  ::=   	VarOrTerm | TriplesNode | ReifiedTriple
    let mut triples_node = Recursive::declare();
    let graph_node = var_or_term
        .clone()
        .map(FocusedTriplePattern::<TermPattern>::new)
        .or(triples_node.clone()); // TODO: ReifiedTriple

    // [108]   	CollectionPath 	  ::=   	'(' GraphNodePath+ ')'
    let collection_path = graph_node_path
        .clone()
        .repeated()
        .at_least(1)
        .collect::<Vec<_>>()
        .delimited_by(operator("("), operator(")"))
        .map(|o| {
            let mut patterns: Vec<TripleOrPathPattern> = Vec::new();
            let mut current_list_node = TermPattern::from(rdf::NIL.into_owned());
            for obj_with_patterns in o.into_iter().rev() {
                let new_blank_node = TermPattern::from(BlankNode::default());
                patterns.push(
                    TriplePattern::new(
                        new_blank_node.clone(),
                        rdf::FIRST.into_owned(),
                        obj_with_patterns.focus.clone(),
                    )
                    .into(),
                );
                patterns.push(
                    TriplePattern::new(
                        new_blank_node.clone(),
                        rdf::REST.into_owned(),
                        current_list_node,
                    )
                    .into(),
                );
                current_list_node = new_blank_node;
                patterns.extend(obj_with_patterns.patterns);
            }
            FocusedTripleOrPathPattern {
                focus: current_list_node,
                patterns,
            }
        });

    // [107]   	Collection 	  ::=   	'(' GraphNode+ ')'
    let collection = graph_node
        .clone()
        .repeated()
        .at_least(1)
        .collect::<Vec<_>>()
        .delimited_by(operator("("), operator(")"))
        .map(|o| {
            let mut patterns: Vec<TriplePattern> = Vec::new();
            let mut current_list_node = TermPattern::from(rdf::NIL.into_owned());
            for obj_with_patterns in o.into_iter().rev() {
                let new_blank_node = TermPattern::from(BlankNode::default());
                patterns.push(
                    TriplePattern::new(
                        new_blank_node.clone(),
                        rdf::FIRST.into_owned(),
                        obj_with_patterns.focus.clone(),
                    )
                    .into(),
                );
                patterns.push(
                    TriplePattern::new(
                        new_blank_node.clone(),
                        rdf::REST.into_owned(),
                        current_list_node,
                    )
                    .into(),
                );
                current_list_node = new_blank_node;
                patterns.extend(obj_with_patterns.patterns);
            }
            FocusedTriplePattern {
                focus: current_list_node,
                patterns,
            }
        });

    // [106]   	BlankNodePropertyListPath 	  ::=   	'[' PropertyListPathNotEmpty ']'
    let mut property_list_path_not_empty = Recursive::declare();
    let blank_node_property_list_path = property_list_path_not_empty
        .clone()
        .delimited_by(operator("["), operator("]"))
        .try_map(
            |po: FocusedTripleOrPathPattern<Vec<(VariableOrPropertyPath, Vec<ReifiedTerm>)>>,
             span| {
                let mut patterns = po.patterns;
                let bnode = TermPattern::from(BlankNode::default());
                for (p, os) in po.focus {
                    for o in os {
                        add_to_triple_or_path_patterns(bnode.clone(), p.clone(), o, &mut patterns)
                            .map_err(|e| Rich::custom(span, e))?;
                    }
                }
                Ok(FocusedTripleOrPathPattern {
                    focus: bnode,
                    patterns,
                })
            },
        );

    // [105]   	TriplesNodePath 	  ::=   	CollectionPath | BlankNodePropertyListPath
    triples_node_path.define(collection_path.or(blank_node_property_list_path));

    // [104]   	BlankNodePropertyList 	  ::=   	'[' PropertyListNotEmpty ']'
    let mut property_list_not_empty = Recursive::declare();
    let blank_node_property_list = property_list_not_empty
        .clone()
        .delimited_by(operator("["), operator("]"))
        .try_map(
            |po: FocusedTriplePattern<Vec<(NamedNodePattern, Vec<ReifiedTerm>)>>, span| {
                let mut patterns = po.patterns;
                let bnode = TermPattern::from(BlankNode::default());
                for (p, os) in po.focus {
                    for o in os {
                        add_to_triple_patterns(bnode.clone(), p.clone(), o, &mut patterns)
                            .map_err(|e| Rich::custom(span, e))?;
                    }
                }
                Ok(FocusedTriplePattern {
                    focus: bnode,
                    patterns,
                })
            },
        );

    // [103]   	TriplesNode 	  ::=   	Collection | BlankNodePropertyList
    triples_node.define(collection.or(blank_node_property_list));

    let path = recursive(|path| {
        // [102]   	PathOneInPropertySet 	  ::=   	iri | 'a' | '^' ( iri | 'a' )
        let path_one_in_property_set = iri
            .clone()
            .map(Either::Left)
            .or(case_sensitive_keyword("a").map(|()| Either::Left(rdf::TYPE.into_owned())))
            .or(operator("^").ignore_then(iri.clone()).map(Either::Right))
            .or(operator("^")
                .ignore_then(case_sensitive_keyword("a"))
                .map(|()| Either::Left(rdf::TYPE.into_owned())));

        // [101]   	PathNegatedPropertySet 	  ::=   	PathOneInPropertySet | '(' ( PathOneInPropertySet ( '|' PathOneInPropertySet )* )? ')'
        let path_negated_property_set = path_one_in_property_set
            .clone()
            .map(|p| match p {
                Either::Left(a) => PropertyPathExpression::NegatedPropertySet(vec![a]),
                Either::Right(b) => PropertyPathExpression::Reverse(Box::new(
                    PropertyPathExpression::NegatedPropertySet(vec![b]),
                )),
            })
            .or(path_one_in_property_set
                .separated_by(operator("|"))
                .at_least(1)
                .collect::<Vec<_>>()
                .delimited_by(operator("("), operator(")"))
                .map(|p| {
                    let mut direct = Vec::new();
                    let mut inverse = Vec::new();
                    for e in p {
                        match e {
                            Either::Left(a) => direct.push(a),
                            Either::Right(b) => inverse.push(b),
                        }
                    }
                    if inverse.is_empty() {
                        PropertyPathExpression::NegatedPropertySet(direct)
                    } else if direct.is_empty() {
                        PropertyPathExpression::Reverse(Box::new(
                            PropertyPathExpression::NegatedPropertySet(inverse),
                        ))
                    } else {
                        PropertyPathExpression::Alternative(
                            Box::new(PropertyPathExpression::NegatedPropertySet(direct)),
                            Box::new(PropertyPathExpression::Reverse(Box::new(
                                PropertyPathExpression::NegatedPropertySet(inverse),
                            ))),
                        )
                    }
                }));

        // [100]   	PathPrimary 	  ::=   	iri | 'a' | '!' PathNegatedPropertySet | '(' Path ')'
        let path_primary = iri
            .clone()
            .map(PropertyPathExpression::from)
            .or(case_sensitive_keyword("a").map(|()| rdf::TYPE.into_owned().into()))
            .or(operator("!").ignore_then(path_negated_property_set))
            .or(path.delimited_by(operator("("), operator(")")));

        // [94]   	Path 	  ::=   	PathAlternative
        // [95]   	PathAlternative 	  ::=   	PathSequence ( '|' PathSequence )*
        // [96]   	PathSequence 	  ::=   	PathEltOrInverse ( '/' PathEltOrInverse )*
        // [97]   	PathElt 	  ::=   	PathPrimary PathMod?
        // [98]   	PathEltOrInverse 	  ::=   	PathElt | '^' PathElt
        // [99]   	PathMod 	  ::=   	'?' | '*' | '+'
        path_primary
            .pratt((
                infix(left(1), operator("|"), |l, _, r, _| {
                    PropertyPathExpression::Alternative(Box::new(l), Box::new(r))
                }),
                infix(left(2), operator("/"), |l, _, r, _| {
                    PropertyPathExpression::Sequence(Box::new(l), Box::new(r))
                }),
                prefix(3, operator("^"), |_, e, _| {
                    PropertyPathExpression::Reverse(Box::new(e))
                }),
                postfix(4, operator("?"), |e, _, _| {
                    PropertyPathExpression::ZeroOrOne(Box::new(e))
                }),
                postfix(4, operator("*"), |e, _, _| {
                    PropertyPathExpression::ZeroOrMore(Box::new(e))
                }),
                postfix(4, operator("+"), |e, _, _| {
                    PropertyPathExpression::OneOrMore(Box::new(e))
                }),
            ))
            .map(|o: PropertyPathExpression| o)
    });

    // [93]   	ObjectPath 	  ::=   	GraphNodePath AnnotationPath
    let object_path = graph_node_path.map(|g| {
        let patterns = g.patterns;
        // TODO patterns.extend(a.patterns);
        FocusedTripleOrPathPattern {
            focus: ReifiedTerm {
                term: g.focus,
                reifiers: Vec::new(), // TODO reifiers: a.focus,
            },
            patterns,
        }
    }); // TODO .then(annotation_path);

    // [92]   	ObjectListPath 	  ::=   	ObjectPath ( ',' ObjectPath )*
    let object_list_path = object_path.separated_by(operator(",")).at_least(1).fold(
        FocusedTripleOrPathPattern::<Vec<_>>::default(),
        |mut l, r| {
            l.focus.push(r.focus);
            l.patterns.extend(r.patterns);
            l
        },
    );

    // [91]   	VerbSimple 	  ::=   	Var
    let verb_simple = var.map(VariableOrPropertyPath::Variable);

    // [90]   	VerbPath 	  ::=   	Path
    let verb_path = path.map(VariableOrPropertyPath::PropertyPath);

    // [89]   	PropertyListPathNotEmpty 	  ::=   	( VerbPath | VerbSimple ) ObjectListPath ( ';' ( ( VerbPath | VerbSimple ) ObjectListPath )? )*
    property_list_path_not_empty.define(
        verb_simple
            .or(verb_path)
            .then(object_list_path)
            .separated_by(operator(";"))
            .allow_trailing()
            .at_least(1)
            .fold(
                FocusedTripleOrPathPattern::<Vec<_>>::default(),
                |mut a, (p, o)| {
                    a.focus.push((p, o.focus));
                    a.patterns.extend(o.patterns);
                    a
                },
            ),
    );

    // [88]   	PropertyListPath 	  ::=   	PropertyListPathNotEmpty?
    let property_list_path = property_list_path_not_empty
        .clone()
        .or_not()
        .map(|po| po.unwrap_or_default());

    // [87]   	TriplesSameSubjectPath 	  ::=   	VarOrTerm PropertyListPathNotEmpty | TriplesNodePath PropertyListPath | ReifiedTripleBlockPath
    let triples_same_subject_path = var_or_term
        .clone()
        .then(property_list_path_not_empty)
        .try_map(|(s, po), span| {
            let mut patterns = po.patterns;
            for (p, os) in po.focus {
                for o in os {
                    add_to_triple_or_path_patterns(s.clone(), p.clone(), o, &mut patterns)
                        .map_err(|e| Rich::custom(span, e))?;
                }
            }
            Ok(patterns)
        })
        .or(triples_node_path
            .then(property_list_path)
            .try_map(|(s, po), span| {
                let mut patterns = s.patterns;
                patterns.extend(po.patterns);
                for (p, os) in po.focus {
                    for o in os {
                        add_to_triple_or_path_patterns(
                            s.focus.clone(),
                            p.clone(),
                            o,
                            &mut patterns,
                        )
                        .map_err(|e| Rich::custom(span, e))?;
                    }
                }
                Ok(patterns)
            }));

    // [86]   	Object 	  ::=   	GraphNode Annotation
    let object = graph_node.map(|g| {
        // TODO: Annotation
        let mut patterns = g.patterns;
        // TODO patterns.extend(a.patterns);
        FocusedTriplePattern {
            focus: ReifiedTerm {
                term: g.focus,
                reifiers: Vec::new(), // TODO a.focus
            },
            patterns,
        }
    });

    // [85]   	ObjectList 	  ::=   	Object ( ',' Object )*
    let object_list = object.separated_by(operator(",")).at_least(1).fold(
        FocusedTriplePattern::<Vec<_>>::default(),
        |mut l, r| {
            l.focus.push(r.focus);
            l.patterns.extend_from_slice(&r.patterns);
            l
        },
    );

    // [84]   	Verb 	  ::=   	VarOrIri | 'a'
    let verb = var_or_iri
        .clone()
        .or(case_sensitive_keyword("a").map(|()| rdf::TYPE.into_owned().into()));

    // [83]   	PropertyListNotEmpty 	  ::=   	Verb ObjectList ( ';' ( Verb ObjectList )? )*
    property_list_not_empty.define(
        verb.then(object_list)
            .map(|(p, o)| FocusedTriplePattern {
                focus: (p, o.focus),
                patterns: o.patterns,
            })
            .separated_by(operator(";"))
            .allow_trailing()
            .at_least(1)
            .fold(FocusedTriplePattern::<Vec<_>>::default(), |mut l, r| {
                l.focus.push(r.focus);
                l.patterns.extend(r.patterns);
                l
            }),
    );

    // [82]   	PropertyList 	  ::=   	PropertyListNotEmpty?
    let property_list = property_list_not_empty
        .clone()
        .or_not()
        .map(|po| po.unwrap_or_default());

    // [81]   	TriplesSameSubject 	  ::=   	VarOrTerm PropertyListNotEmpty | TriplesNode PropertyList | ReifiedTripleBlock
    let triples_same_subject = var_or_term
        .then(property_list_not_empty)
        .try_map(|(s, po), span| {
            let mut patterns = po.patterns;
            for (p, os) in po.focus {
                for o in os {
                    add_to_triple_patterns(s.clone(), p.clone(), o, &mut patterns)
                        .map_err(|e| Rich::custom(span, e))?
                }
            }
            Ok(patterns)
        })
        .or(triples_node.then(property_list).try_map(|(s, po), span| {
            let mut patterns = s.patterns;
            patterns.extend(po.patterns);
            for (p, os) in po.focus {
                for o in os {
                    add_to_triple_patterns(s.focus.clone(), p.clone(), o, &mut patterns)
                        .map_err(|e| Rich::custom(span, e))?
                }
            }
            Ok(patterns)
        })); // TODO ReifiedTripleBlock

    // [80]   	ConstructTriples 	  ::=   	TriplesSameSubject ( '.' ConstructTriples? )?
    // also TriplesSameSubject ("." TriplesSameSubject?)*
    let construct_triples = triples_same_subject
        .clone()
        .separated_by(operator("."))
        .allow_trailing()
        .at_least(1)
        .fold(Vec::new(), |mut l, r| {
            l.extend(r);
            l
        });

    // [79]   	ConstructTemplate 	  ::=   	'{' ConstructTriples? '}'
    let construct_template = construct_triples
        .or_not()
        .delimited_by(operator("{"), operator("}"))
        .map(|t| t.unwrap_or_default());

    // [76]   	FunctionCall 	  ::=   	iri ArgList
    let function_call = iri.clone().then(expression_list).try_map({
        let state = Rc::clone(&state);
        move |(name, args), span| {
            if state.borrow().custom_aggregate_functions.contains(&name) {
                Err(Rich::custom(
                    span,
                    format!("{name} is an aggregate function and not a regular function"),
                ))
            } else {
                Ok(Expression::FunctionCall(Function::Custom(name), args))
            }
        }
    });

    // [75]   	Constraint 	  ::=   	BrackettedExpression | BuiltInCall | FunctionCall
    let constraint = bracketted_expression
        .clone()
        .or(built_in_call.clone())
        .or(function_call.clone());

    // [74]   	Filter 	  ::=   	'FILTER' Constraint
    let filter = keyword("filter")
        .ignore_then(constraint.clone())
        .map(PartialGraphPattern::Filter);

    // [73]   	GroupOrUnionGraphPattern 	  ::=   	GroupGraphPattern ( 'UNION' GroupGraphPattern )*
    let group_or_union_graph_pattern = group_graph_pattern
        .clone()
        .separated_by(keyword("union"))
        .at_least(1)
        .fold(None, |a, b| {
            Some(if let Some(a) = a {
                GraphPattern::Union {
                    left: Box::new(a),
                    right: Box::new(b),
                }
            } else {
                b
            })
        })
        .map(|p| PartialGraphPattern::Other(p.unwrap_or_default()));

    // [72]   	MinusGraphPattern 	  ::=   	'MINUS' GroupGraphPattern
    let minus_graph_pattern = keyword("minus")
        .ignore_then(group_graph_pattern.clone())
        .map(PartialGraphPattern::Minus);

    // [69]   	DataBlockValue 	  ::=   	iri | RDFLiteral | NumericLiteral | BooleanLiteral | 'UNDEF' | TripleTermData
    let data_block_value = iri
        .clone()
        .map(|t| Some(GroundTerm::NamedNode(t)))
        .or(rdf_literal
            .or(numeric_literal)
            .or(boolean_literal)
            .map(|t| Some(GroundTerm::Literal(t))))
        .or(keyword("undef").map(|()| None));

    // [68]   	InlineDataFull 	  ::=   	( NIL | '(' Var* ')' ) '{' ( '(' DataBlockValue* ')' | NIL )* '}'
    let inline_data_full = nil
        .map(|()| Vec::new())
        .or(var
            .clone()
            .repeated()
            .collect()
            .delimited_by(operator("("), operator(")")))
        .then(
            nil.map(|()| Vec::new())
                .or(data_block_value
                    .clone()
                    .repeated()
                    .collect()
                    .delimited_by(operator("("), operator(")")))
                .repeated()
                .collect::<Vec<_>>()
                .delimited_by(operator("{"), operator("}")),
        ).try_map(|(variables, bindings), span| {
            if let Some((_, v)) = variables.iter().enumerate().find(|(i, vl)| variables[i+1..].contains(vl)) {
                Err(Rich::custom(span, format!("{v} is repeated, this is not allowed")))
            } else if bindings.iter().any(|vs| vs.len() != variables.len()) {
                Err(Rich::custom(span, "The VALUES clause rows should have exactly the same number of values as there are variables. To set a value to undefined use UNDEF."))
            } else {
                Ok(GraphPattern::Values {
                    variables,
                    bindings,
                })
            }
        });

    // [67]   	InlineDataOneVar 	  ::=   	Var '{' DataBlockValue* '}'
    let inline_data_one_var = var
        .then(
            data_block_value
                .map(|v| vec![v])
                .repeated()
                .collect()
                .delimited_by(operator("{"), operator("}")),
        )
        .map(|(v, bindings)| GraphPattern::Values {
            variables: vec![v],
            bindings,
        });

    // [66]   	DataBlock 	  ::=   	InlineDataOneVar | InlineDataFull
    let data_block = inline_data_one_var.or(inline_data_full);

    // [65]   	InlineData 	  ::=   	'VALUES' DataBlock
    let inline_data = keyword("values")
        .ignore_then(data_block.clone())
        .map(PartialGraphPattern::Other);

    // [64]   	Bind 	  ::=   	'BIND' '(' Expression 'AS' Var ')'
    let bind = keyword("bind")
        .ignore_then(
            expression
                .clone()
                .then_ignore(keyword("as"))
                .then(var)
                .delimited_by(operator("("), operator(")")),
        )
        .map(|(e, v)| PartialGraphPattern::Bind(e, v));

    // [63]   	ServiceGraphPattern 	  ::=   	'SERVICE' 'SILENT'? VarOrIri GroupGraphPattern
    let service_graph_pattern = keyword("service")
        .ignore_then(keyword("silent").or_not())
        .then(var_or_iri.clone())
        .then(group_graph_pattern.clone())
        .map(|((silent, name), p)| {
            PartialGraphPattern::Other(GraphPattern::Service {
                name,
                inner: Box::new(p),
                silent: silent.is_some(),
            })
        });

    // [62]   	GraphGraphPattern 	  ::=   	'GRAPH' VarOrIri GroupGraphPattern
    let graph_graph_pattern = keyword("graph")
        .ignore_then(var_or_iri.clone())
        .then(group_graph_pattern.clone())
        .map(|(name, p)| {
            PartialGraphPattern::Other(GraphPattern::Graph {
                name,
                inner: Box::new(p),
            })
        });

    // [61]   	OptionalGraphPattern 	  ::=   	'OPTIONAL' GroupGraphPattern
    let optional_graph_pattern = keyword("optional")
        .ignore_then(group_graph_pattern.clone())
        .map(|p| {
            if let GraphPattern::Filter { expr, inner } = p {
                PartialGraphPattern::Optional(*inner, Some(expr))
            } else {
                PartialGraphPattern::Optional(p, None)
            }
        });

    // [60]   	GraphPatternNotTriples 	  ::=   	GroupOrUnionGraphPattern | OptionalGraphPattern | MinusGraphPattern | GraphGraphPattern | ServiceGraphPattern | Filter | Bind | InlineData
    let graph_pattern_not_triples = group_or_union_graph_pattern
        .or(optional_graph_pattern)
        .or(minus_graph_pattern)
        .or(graph_graph_pattern)
        .or(service_graph_pattern)
        .or(filter)
        .or(bind)
        .or(inline_data);

    // [57]   	TriplesBlock 	  ::=   	TriplesSameSubjectPath ( '.' TriplesBlock? )?
    // also TriplesSameSubjectPath ( '.' TriplesSameSubjectPath? )*
    let triples_block = triples_same_subject_path
        .separated_by(operator("."))
        .allow_trailing()
        .at_least(1)
        .fold(Vec::new(), |mut a, b| {
            a.extend(b);
            a
        })
        .map(|bgp| PartialGraphPattern::Other(build_bgp(bgp)))
        .boxed();

    // [56]   	GroupGraphPatternSub 	  ::=   	TriplesBlock? ( GraphPatternNotTriples '.'? TriplesBlock? )*
    let group_graph_pattern_sub = triples_block
            .clone()
            .or_not()
            .map(|p| p.into_iter().collect::<Vec<_>>())
            .foldl(
                graph_pattern_not_triples
                    .then_ignore(operator(".").or_not())
                    .then(triples_block.or_not())
                    .map(|(l, r)| (l, r))
                    .repeated(),
                |mut a, (b, c)| {
                    a.push(b);
                    if let Some(c) = c {
                        a.push(c);
                    }
                    a
                },
            ).try_map(|es, span| {
            let mut filter: Option<Expression> = None;
            let mut g = GraphPattern::default();
            for e in es {
                match e {
                    PartialGraphPattern::Optional(p, f) => {
                        g = GraphPattern::LeftJoin { left: Box::new(g), right: Box::new(p), expression: f }
                    }
                    #[cfg(feature = "sep-0006")]
                    PartialGraphPattern::Lateral(p) => {
                        let mut defined_variables = HashSet::new();
                        add_defined_variables(&p, &mut defined_variables);
                        let mut overridden_variable = None;
                        g.on_in_scope_variable(|v| {
                            if defined_variables.contains(v) {
                                overridden_variable = Some(v.clone());
                            }
                        });
                        if let Some(overridden_variable) = overridden_variable {
                            return Err(Rich::custom(span, format!("{overridden_variable} is overridden in the right side of LATERAL")));
                        }
                        g = GraphPattern::Lateral { left: Box::new(g), right: Box::new(p) }
                    }
                    PartialGraphPattern::Minus(p) => {
                        g = GraphPattern::Minus { left: Box::new(g), right: Box::new(p) }
                    }
                    PartialGraphPattern::Bind(expression, variable) => {
                        let mut is_variable_overridden = false;
                        g.on_in_scope_variable(|v| {
                            if *v == variable {
                                is_variable_overridden = true;
                            }
                        });
                        if is_variable_overridden {
                            return Err(Rich::custom(span, format!("{variable} is already in scoped and cannot be overridden by BIND")));
                        }
                        g = GraphPattern::Extend { inner: Box::new(g), variable, expression }
                    }
                    PartialGraphPattern::Filter(expr) => filter = Some(if let Some(f) = filter {
                        Expression::And(Box::new(f), Box::new(expr))
                    } else {
                        expr
                    }),
                    PartialGraphPattern::Other(e) => g = new_join(g, e),
                }
            }

            Ok(if let Some(expr) = filter {
                GraphPattern::Filter { expr, inner: Box::new(g) }
            } else {
                g
            })
        });

    // [55]   	GroupGraphPattern 	  ::=   	'{' ( SubSelect | GroupGraphPatternSub ) '}'
    group_graph_pattern.define(
        group_graph_pattern_sub.delimited_by(operator("{"), operator("}")) // TODO
            .map({
                let state = Rc::clone(&state);
                move |g| {
                    // We deal with blank nodes aliases rule
                    let mut state = state.borrow_mut();
                    let current_bnodes = take(&mut state.currently_used_bnodes);
                    state.used_bnodes.extend(current_bnodes);
                    g
                }
            }),
    );

    // [54]   	TriplesTemplate 	  ::=   	TriplesSameSubject ( '.' TriplesTemplate? )?
    let triples_template = triples_same_subject
        .separated_by(operator("."))
        .allow_trailing()
        .at_least(1)
        .fold(Vec::new(), |mut l, r| {
            l.extend(r);
            l
        });

    // [30]   	ValuesClause 	  ::=   	( 'VALUES' DataBlock )?
    let values_clause = keyword("values").ignore_then(data_block).or_not().boxed();

    // [29]   	OffsetClause 	  ::=   	'OFFSET' INTEGER
    let offset_clause = keyword("offset")
        .ignore_then(select! {
            Token::Integer(v) => v,
        })
        .try_map(|o, span| {
            usize::from_str(o).map_err(|_| {
                Rich::custom(
                    span,
                    format!("The query offset must be a non negative integer, found {o}"),
                )
            })
        });

    // [28]   	LimitClause 	  ::=   	'LIMIT' INTEGER
    let limit_clause = keyword("limit")
        .ignore_then(select! {
            Token::Integer(v) => v,
        })
        .try_map(|l, span| {
            usize::from_str(l).map_err(|_| {
                Rich::custom(
                    span,
                    format!("The query limit must be a non negative integer, found {l}"),
                )
            })
        });

    // [27]   	LimitOffsetClauses 	  ::=   	LimitClause OffsetClause? | OffsetClause LimitClause?
    let limit_offset_clauses = limit_clause
        .clone()
        .then(offset_clause.clone().or_not())
        .map(|(l, o)| (o.unwrap_or(0), Some(l)))
        .or(offset_clause.then(limit_clause.or_not()));

    // [26]   	OrderCondition 	  ::=   	( ( 'ASC' | 'DESC' ) BrackettedExpression ) | ( Constraint | Var )
    let order_condition = keyword("asc")
        .ignore_then(bracketted_expression.clone())
        .map(OrderExpression::Asc)
        .or(keyword("desc")
            .ignore_then(bracketted_expression)
            .map(OrderExpression::Desc))
        .or(constraint.clone().map(OrderExpression::Asc))
        .or(var.clone().map(|v| OrderExpression::Asc(v.into())));

    // [25]   	OrderClause 	  ::=   	'ORDER' 'BY' OrderCondition+
    let order_clause = keyword("order")
        .ignore_then(keyword("by"))
        .ignore_then(order_condition.repeated().at_least(1).collect::<Vec<_>>());

    // [24]   	HavingCondition 	  ::=   	Constraint
    let having_condition = constraint;

    // [23]   	HavingClause 	  ::=   	'HAVING' HavingCondition+
    let having_clause = keyword("having").ignore_then(
        having_condition
            .repeated()
            .at_least(1)
            .fold(None, |l, r| {
                Some(if let Some(l) = l {
                    Expression::And(Box::new(l), Box::new(r))
                } else {
                    r
                })
            })
            .map(|e| e.expect("this is not empty")),
    );

    // [22]   	GroupCondition 	  ::=   	BuiltInCall | FunctionCall | '(' Expression ( 'AS' Var )? ')' | Var
    let group_condition = built_in_call
        .map(|e| (e, None))
        .or(function_call.map(|e| (e, None)))
        .or(expression
            .clone()
            .then(keyword("as").ignore_then(var).or_not()))
        .or(var.map(|e| (e.into(), None)));

    // [21]   	GroupClause 	  ::=   	'GROUP' 'BY' GroupCondition+
    let group_clause = keyword("group")
        .ignore_then(keyword("by"))
        .ignore_then(group_condition.repeated().at_least(1).collect::<Vec<_>>())
        .map(|c| {
            let mut projections: Vec<(Expression, Variable)> = Vec::new();
            let clauses = c
                .into_iter()
                .map(|(e, vo)| {
                    if let Expression::Variable(v) = e {
                        v
                    } else {
                        let v = vo.unwrap_or_else(variable);
                        projections.push((e, v.clone()));
                        v
                    }
                })
                .collect::<Vec<_>>();
            (clauses, projections)
        });

    // [20]   	SolutionModifier 	  ::=   	GroupClause? HavingClause? OrderClause? LimitOffsetClauses?
    let solution_modifier = group_clause
        .or_not()
        .then(having_clause.or_not())
        .then(order_clause.or_not())
        .then(limit_offset_clauses.or_not())
        .boxed();

    // [19]   	WhereClause 	  ::=   	'WHERE'? GroupGraphPattern
    let where_clause = keyword("where")
        .or_not()
        .ignore_then(group_graph_pattern)
        .boxed();

    // [18]   	SourceSelector 	  ::=   	iri
    let source_selector = iri;

    // [17]   	NamedGraphClause 	  ::=   	'NAMED' SourceSelector
    let named_graph_clause = keyword("named")
        .ignore_then(source_selector.clone())
        .map(|s| (None, Some(s)));

    // [16]   	DefaultGraphClause 	  ::=   	SourceSelector
    let default_graph_clause = source_selector.map(|s| (Some(s), None));

    // [15]   	DatasetClause 	  ::=   	'FROM' ( DefaultGraphClause | NamedGraphClause )
    let dataset_clause = keyword("from").ignore_then(default_graph_clause.or(named_graph_clause));
    let dataset_clauses = dataset_clause
        .repeated()
        .collect::<Vec<_>>()
        .map(|d| {
            if d.is_empty() {
                return None;
            }
            let mut default = Vec::new();
            let mut named = Vec::new();
            for (d, n) in d {
                if let Some(d) = d {
                    default.push(d);
                }
                if let Some(n) = n {
                    named.push(n);
                }
            }
            Some(QueryDataset {
                default,
                named: Some(named),
            })
        })
        .boxed();

    // [14]   	AskQuery 	  ::=   	'ASK' DatasetClause* WhereClause SolutionModifier
    let ask_query = keyword("ask")
        .ignore_then(dataset_clauses.clone())
        .then(where_clause.clone())
        .then(solution_modifier.clone())
        .then(values_clause.clone())
        .try_map({
            let state = Rc::clone(&state);
            move |(((dataset, w), (((g, h), o), l)), v), span| {
                Ok(Query::Ask {
                    dataset,
                    pattern: build_select(Selection::default(), w, g, h, o, l, v, &state)
                        .map_err(|e| Rich::custom(span, e))?,
                    base_iri: state.borrow().base_iri.clone(),
                })
            }
        });

    // [13]   	DescribeQuery 	  ::=   	'DESCRIBE' ( VarOrIri+ | '*' ) DatasetClause* WhereClause? SolutionModifier
    let describe_query = keyword("describe")
        .ignore_then(
            var_or_iri
                .repeated()
                .at_least(1)
                .collect::<Vec<_>>()
                .map(|p| Selection {
                    option: SelectionOption::Default,
                    variables: SelectionVariables::Explicit(
                        p.into_iter()
                            .map(|var_or_iri| match var_or_iri {
                                NamedNodePattern::NamedNode(n) => {
                                    SelectionMember::Expression(n.into(), variable())
                                }
                                NamedNodePattern::Variable(v) => SelectionMember::Variable(v),
                            })
                            .collect(),
                    ),
                })
                .or(operator("*").map(|()| Selection::default())),
        )
        .then(dataset_clauses.clone())
        .then(where_clause.clone().or_not())
        .then(solution_modifier.clone())
        .then(values_clause.clone())
        .try_map({
            let state = Rc::clone(&state);
            move |((((s, dataset), w), (((g, h), o), l)), v), span| {
                Ok(Query::Describe {
                    dataset,
                    pattern: build_select(s, w.unwrap_or_default(), g, h, o, l, v, &state)
                        .map_err(|e| Rich::custom(span, e))?,
                    base_iri: state.borrow().base_iri.clone(),
                })
            }
        });

    // [12]   	ConstructQuery 	  ::=   	'CONSTRUCT' ( ConstructTemplate DatasetClause* WhereClause SolutionModifier | DatasetClause* 'WHERE' '{' TriplesTemplate? '}' SolutionModifier )
    let construct_query = keyword("construct")
        .ignore_then(
            construct_template
                .then(dataset_clauses.clone())
                .then(where_clause.clone())
                .then(solution_modifier.clone())
                .then(values_clause.clone())
                .try_map({
                    let state = Rc::clone(&state);
                    move |((((c, dataset), w), (((g, h), o), l)), v), span| {
                        Ok(Query::Construct {
                            template: c,
                            dataset,
                            pattern: build_select(Selection::default(), w, g, h, o, l, v, &state)
                                .map_err(|e| Rich::custom(span, e))?,
                            base_iri: state.borrow().base_iri.clone(),
                        })
                    }
                }),
        )
        .or(dataset_clauses
            .clone()
            .then_ignore(keyword("where"))
            .then(
                triples_template
                    .or_not()
                    .delimited_by(operator("{"), operator("}")),
            )
            .then(solution_modifier.clone())
            .then(values_clause.clone())
            .try_map({
                let state = Rc::clone(&state);
                move |(((dataset, c), (((g, h), o), l)), v), span| {
                    Ok(Query::Construct {
                        template: c.clone().unwrap_or_default(),
                        dataset,
                        pattern: build_select(
                            Selection::default(),
                            GraphPattern::Bgp {
                                patterns: c.unwrap_or_default(),
                            },
                            g,
                            h,
                            o,
                            l,
                            v,
                            &state,
                        )
                        .map_err(|e| Rich::custom(span, e))?,
                        base_iri: state.borrow().base_iri.clone(),
                    })
                }
            }));

    // [11]   	SelectClause 	  ::=   	'SELECT' ( 'DISTINCT' | 'REDUCED' )? ( ( Var | ( '(' Expression 'AS' Var ')' ) )+ | '*' )
    let select_clause = keyword("select")
        .ignore_then(
            keyword("distinct")
                .map(|()| SelectionOption::Distinct)
                .or(keyword("reduced").map(|()| SelectionOption::Reduced))
                .or_not()
                .map(|s| s.unwrap_or_default()),
        )
        .then(
            var.clone()
                .map(SelectionMember::Variable)
                .or(expression
                    .clone()
                    .then_ignore(keyword("as"))
                    .then(var.clone())
                    .map(|(e, v)| SelectionMember::Expression(e, v))
                    .delimited_by(operator("("), operator(")")))
                .repeated()
                .at_least(1)
                .collect()
                .map(SelectionVariables::Explicit)
                .or(operator("*").map(|()| SelectionVariables::Star)),
        )
        .map(|(option, variables)| Selection { option, variables });

    // [9]   	SelectQuery 	  ::=   	SelectClause DatasetClause* WhereClause SolutionModifier
    let select_query = select_clause
        .then(dataset_clauses)
        .then(where_clause)
        .then(solution_modifier)
        .then(values_clause)
        .try_map({
            let state = Rc::clone(&state);
            move |((((s, dataset), w), (((g, h), o), l)), v), span| {
                Ok(Query::Select {
                    dataset,
                    pattern: build_select(s, w, g, h, o, l, v, &state)
                        .map_err(|e| Rich::custom(span, e))?,
                    base_iri: state.borrow().base_iri.clone(),
                })
            }
        });

    // [8]   	VersionSpecifier 	  ::=   	STRING_LITERAL1 | STRING_LITERAL2
    let version_specifier = select! {
        Token::StringLiteral1(_) | Token::StringLiteral2(_) => ()
    };

    // [7]   	VersionDecl 	  ::=   	'VERSION' VersionSpecifier
    let version_decl = keyword("version").ignore_then(version_specifier);

    // [6]   	PrefixDecl 	  ::=   	'PREFIX' PNAME_NS IRIREF
    let prefix_decl = keyword("prefix")
        .ignore_then(select! {
            Token::PnameNs(prefix) => prefix
        })
        .then(select! {
            Token::IriRef(iri) => iri
        })
        .map({
            let state = Rc::clone(&state);
            move |(prefix, iri)| {
                state
                    .borrow_mut()
                    .prefixes
                    .insert(prefix.into(), iri.into());
                ()
            }
        });

    // [5]   	BaseDecl 	  ::=   	'BASE' IRIREF
    let base_decl = keyword("base")
        .ignore_then(select! {
            Token::IriRef(iri) => iri
        })
        .try_map({
            let state = Rc::clone(&state);
            move |iri, span| {
                state.borrow_mut().base_iri = Some(
                    Iri::parse(iri.to_owned())
                        .map_err(|e| Rich::custom(span, format!("Failed to parse IRI: {e}")))?,
                );
                Ok(())
            }
        });

    // [4]   	Prologue 	  ::=   	( BaseDecl | PrefixDecl | VersionDecl )*
    let prologue = base_decl.or(prefix_decl).or(version_decl).repeated();

    // [2]   	Query 	  ::=   	Prologue ( SelectQuery | ConstructQuery | DescribeQuery | AskQuery ) ValuesClause
    // We put the ValuesClause in each query
    let query = prologue.ignore_then(
        select_query
            .or(construct_query)
            .or(describe_query)
            .or(ask_query),
    ); // TODO

    query
}

fn lexer<'src>()
-> impl Parser<'src, &'src str, Vec<Token<'src>>, extra::Err<Rich<'src, char, SimpleSpan>>> {
    // [193]   	PN_LOCAL_ESC 	  ::=   	'\' ( '_' | '~' | '.' | '-' | '!' | '$' | '&' | "'" | '(' | ')' | '*' | '+' | ',' | ';' | '=' | '/' | '?' | '#' | '@' | '%' )
    let pn_local_esc = just('\\').then(one_of([
        '_', '~', '.', '-', '!', '$', '&', '\'', '(', ')', '*', '+', ',', ';', '=', '/', '?', '#',
        '@', '%',
    ]));

    // [192]   	HEX 	  ::=   	[0-9] | [A-F] | [a-f]
    let hex = one_of('0'..'9').or(one_of('A'..='F')).or(one_of('a'..='f'));

    // [191]   	PERCENT 	  ::=   	'%' HEX HEX
    let percent = just('%').then(hex.clone()).then(hex);

    // [190]   	PLX 	  ::=   	PERCENT | PN_LOCAL_ESC
    let plx = percent.ignored().or(pn_local_esc.ignored());

    // [184]   	PN_CHARS_BASE 	  ::=   	[A-Z] | [a-z] | [#x00C0-#x00D6] | [#x00D8-#x00F6] | [#x00F8-#x02FF] | [#x0370-#x037D] | [#x037F-#x1FFF] | [#x200C-#x200D] | [#x2070-#x218F] | [#x2C00-#x2FEF] | [#x3001-#xD7FF] | [#xF900-#xFDCF] | [#xFDF0-#xFFFD] | [#x10000-#xEFFFF]
    let pn_chars_base = any().filter(|c| {
        // TODO: use the same kind of matching for the other variable ranges?
        matches!(c,
        'A'..='Z'
        | 'a'..='z'
        | '\u{00C0}'..='\u{00D6}'
        | '\u{00D8}'..='\u{00F6}'
        | '\u{00F8}'..='\u{02FF}'
        | '\u{0370}'..='\u{037D}'
        | '\u{037F}'..='\u{1FFF}'
        | '\u{200C}'..='\u{200D}'
        | '\u{2070}'..='\u{218F}'
        | '\u{2C00}'..='\u{2FEF}'
        | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}'
        | '\u{FDF0}'..='\u{FFFD}'
        | '\u{10000}'..='\u{EFFFF}')
    });

    // [185]   	PN_CHARS_U 	  ::=   	PN_CHARS_BASE | '_'
    let pn_chars_u = pn_chars_base.or(just('_'));

    // [187]   	PN_CHARS 	  ::=   	PN_CHARS_U | '-' | [0-9] | #x00B7 | [#x0300-#x036F] | [#x203F-#x2040]
    let pn_chars = pn_chars_u
        .or(just('-'))
        .or(one_of('0'..='9'))
        .or(one_of('\u{00B7}'))
        .or(one_of('\u{0300}'..='\u{036F}'))
        .or(one_of('\u{203F}'..='\u{2040}'));

    // [189]   	PN_LOCAL 	  ::=   	(PN_CHARS_U | ':' | [0-9] | PLX ) ((PN_CHARS | '.' | ':' | PLX)* (PN_CHARS | ':' | PLX) )?
    let pn_local = pn_chars_u
        .or(just(':'))
        .or(one_of('0'..='9'))
        .ignored()
        .or(plx.clone())
        .then(
            pn_chars
                .clone()
                .or(just('.'))
                .or(just(':'))
                .ignored()
                .or(plx.clone())
                .repeated()
                .then(pn_chars.clone().or(just(':')).ignored().or(plx))
                .or_not(),
        )
        .to_slice();

    // [188]   	PN_PREFIX 	  ::=   	PN_CHARS_BASE ((PN_CHARS|'.')* PN_CHARS)?
    let pn_prefix = pn_chars_base.then(
        pn_chars
            .clone()
            .or(just('.'))
            .repeated()
            .then(pn_chars.clone())
            .or_not(),
    );

    // [186]   	VARNAME 	  ::=   	( PN_CHARS_U | [0-9] ) ( PN_CHARS_U | [0-9] | #x00B7 | [#x0300-#x036F] | [#x203F-#x2040] )*
    let varname = pn_chars_u
        .or(one_of('0'..='9'))
        .then(
            pn_chars_u
                .or(one_of('0'..='9'))
                .or(one_of('\u{00B7}'))
                .or(one_of('\u{0300}'..='\u{036F}'))
                .or(one_of('\u{203F}'..='\u{2040}'))
                .repeated(),
        )
        .to_slice();

    // [182]   	WS 	  ::=   	#x20 | #x9 | #xD | #xA
    let ws = one_of([' ', '\t', '\r', '\n']);

    // [183]   	ANON 	  ::=   	'[' WS* ']'
    let anon = just('[')
        .then(ws.repeated())
        .then(just(']'))
        .ignored()
        .map(|()| Token::Anon);

    // [181]   	NIL 	  ::=   	'(' WS* ')'
    let nil = just('(')
        .then(ws.repeated())
        .then(just(')'))
        .ignored()
        .map(|()| Token::Nil);

    // [180]   	ECHAR 	  ::=   	'\' [tbnrf\"']
    let echar = just('\\').then(one_of(['t', 'b', 'n', 'r', 'f', '"', '\'', '\\']));

    // [179]   	STRING_LITERAL_LONG2 	  ::=   	'"""' ( ( '"' | '""' )? ( [^"\] | ECHAR ) )* '"""'
    let string_literal_long2 = just("\"\"\"")
        .ignore_then(
            just('"')
                .then(just('"').or_not())
                .or_not()
                .then(none_of(['"', '\\']).ignored().or(echar.ignored()))
                .repeated()
                .to_slice(),
        )
        .then_ignore(just("\"\"\""))
        .map(Token::StringLiteralLong2);

    // [178]   	STRING_LITERAL_LONG1 	  ::=   	"'''" ( ( "'" | "''" )? ( [^'\] | ECHAR ) )* "'''"
    let string_literal_long1 = just("'''")
        .ignore_then(
            just('\'')
                .then(just('\'').or_not())
                .or_not()
                .then(none_of(['\'', '\\']).ignored().or(echar.ignored()))
                .repeated()
                .to_slice(),
        )
        .then_ignore(just("'''"))
        .map(Token::StringLiteralLong1);

    // [177]   	STRING_LITERAL2 	  ::=   	'"' ( ([^#x22#x5C#xA#xD]) | ECHAR )* '"'
    let string_literal2 = just('"')
        .ignore_then(
            none_of(['\x22', '\x5C', '\x0A', '\x0D'])
                .ignored()
                .or(echar.ignored())
                .repeated()
                .to_slice(),
        )
        .then_ignore(just('"'))
        .map(Token::StringLiteral2);

    // [176]   	STRING_LITERAL1 	  ::=   	"'" ( ([^#x27#x5C#xA#xD]) | ECHAR )* "'"
    let string_literal1 = just('\'')
        .ignore_then(
            none_of(['\x27', '\x5C', '\x0A', '\x0D'])
                .ignored()
                .or(echar.ignored())
                .repeated()
                .to_slice(),
        )
        .then_ignore(just('\''))
        .map(Token::StringLiteral1);

    // [169]   	EXPONENT 	  ::=   	[eE] [+-]? [0-9]+
    let exponent = one_of(['e', 'E'])
        .then(one_of(['+', '=']).or_not())
        .then(one_of('0'..='9').repeated().at_least(1));

    // [168]   	DOUBLE 	  ::=   	( ([0-9]+ ('.'[0-9]*)? ) | ( '.' ([0-9])+ ) ) EXPONENT
    let double = one_of('0'..='9')
        .repeated()
        .at_least(1)
        .then(just('.').then(one_of('0'..='9').repeated()).or_not())
        .ignored()
        .or(just('.')
            .then(one_of('0'..='9').repeated().at_least(1))
            .ignored())
        .then(exponent)
        .to_slice()
        .map(Token::Double);

    // [167]   	DECIMAL 	  ::=   	[0-9]* '.' [0-9]+
    let decimal = one_of('0'..='9')
        .repeated()
        .then(just('.'))
        .then(one_of('0'..='9').repeated().at_least(1))
        .to_slice()
        .map(Token::Decimal);

    // [166]   	INTEGER 	  ::=   	[0-9]+
    let integer = one_of('0'..='9')
        .repeated()
        .at_least(1)
        .to_slice()
        .map(Token::Integer);

    // [175]   	DOUBLE_NEGATIVE 	  ::=   	'-' DOUBLE
    let double_negative = just('-')
        .then(double.clone())
        .to_slice()
        .map(Token::DoubleNegative);

    // [174]   	DECIMAL_NEGATIVE 	  ::=   	'-' DECIMAL
    let decimal_negative = just('-')
        .then(decimal.clone())
        .to_slice()
        .map(Token::DecimalNegative);

    // [173]   	INTEGER_NEGATIVE 	  ::=   	'-' INTEGER
    let integer_negative = just('-')
        .then(integer.clone())
        .to_slice()
        .map(Token::IntegerNegative);

    // [172]   	DOUBLE_POSITIVE 	  ::=   	'+' DOUBLE
    let double_positive = just('+')
        .then(double.clone())
        .to_slice()
        .map(Token::DoublePositive);

    // [171]   	DECIMAL_POSITIVE 	  ::=   	'+' DECIMAL
    let decimal_positive = just('+')
        .then(decimal.clone())
        .to_slice()
        .map(Token::DecimalPositive);

    // [170]   	INTEGER_POSITIVE 	  ::=   	'+' INTEGER
    let integer_positive = just('+')
        .then(integer.clone())
        .to_slice()
        .map(Token::IntegerPositive);

    // [165]   	LANG_DIR 	  ::=   	'@' [a-zA-Z]+ ('-' [a-zA-Z0-9]+)* ('--' [a-zA-Z]+)?
    let lang_dir = just('@')
        .ignore_then(
            one_of('a'..='z')
                .or(one_of('A'..='A'))
                .repeated()
                .at_least(1)
                .then(
                    just('-')
                        .then(
                            one_of('a'..='z')
                                .or(one_of('A'..='A'))
                                .or(one_of('0'..='9'))
                                .repeated()
                                .at_least(1),
                        )
                        .repeated(),
                )
                .to_slice(),
        )
        .map(Token::LangDir); // TODO ('--' [a-zA-Z]+)?

    // [164]   	VAR2 	  ::=   	'$' VARNAME
    let var2 = just('$').ignore_then(varname.clone()).map(Token::Var2);

    // [163]   	VAR1 	  ::=   	'?' VARNAME
    let var1 = just('?').ignore_then(varname).map(Token::Var1);
    let var = var1.or(var2).labelled("variable like ?foo or $foo");

    // [162]   	BLANK_NODE_LABEL 	  ::=   	'_:' ( PN_CHARS_U | [0-9] ) ((PN_CHARS|'.')* PN_CHARS)?
    let blank_node_label = just("_:")
        .ignore_then(
            pn_chars_u
                .or(one_of('0'..='9'))
                .then(
                    pn_chars
                        .clone()
                        .or(just('.'))
                        .repeated()
                        .then(pn_chars)
                        .or_not(),
                )
                .to_slice(),
        )
        .map(Token::BlankNodeLabel);

    // [160]   	PNAME_NS 	  ::=   	PN_PREFIX? ':'
    let pname_ns = pn_prefix
        .or_not()
        .to_slice()
        .then_ignore(just(':'))
        .map(Token::PnameNs);

    // [161]   	PNAME_LN 	  ::=   	PNAME_NS PN_LOCAL
    let pname_ln = pname_ns
        .clone()
        .to_slice()
        .then(pn_local)
        .map(|(k, v)| Token::PnameLn(k, v));

    // [159]   	IRIREF 	  ::=   	'<' ([^<>"{}|^`\]-[#x00-#x20])* '>'
    // We do not validate the content with chumsky because we validate the IRI after
    let iri_ref = just('<')
        .ignore_then(none_of(['>']).repeated().to_slice())
        .then_ignore(just('>'))
        .map(Token::IriRef);

    let keyword = ident().map(Token::Keyword);

    let operator = just("||")
        .or(just("&&"))
        .or(just("^^"))
        .or(just("<="))
        .or(just(">="))
        .or(just("!="))
        .or(one_of([
            '{', '}', '[', ']', '(', ')', ';', ',', '.', '+', '-', '*', '/', '<', '>', '=', '!',
            '^', '|',
        ])
        .to_slice())
        .map(Token::Operator);

    let token = iri_ref
        .or(blank_node_label)
        .or(lang_dir)
        .or(var)
        .or(string_literal_long1)
        .or(string_literal_long2)
        .or(string_literal1)
        .or(string_literal2)
        .or(nil)
        .or(anon)
        .or(double_positive)
        .or(decimal_positive)
        .or(integer_positive)
        .or(double_negative)
        .or(decimal_negative)
        .or(integer_negative)
        .or(double)
        .or(decimal)
        .or(integer)
        .or(pname_ln)
        .or(pname_ns)
        .or(keyword)
        .or(operator);

    let comment = just('#').then(just('\n').not().repeated()).padded();

    token
        .padded_by(comment.repeated())
        .padded()
        .recover_with(skip_then_retry_until(any().ignored(), end()))
        .repeated()
        .collect()
}

fn keyword<'src, I: ValueInput<'src, Token = Token<'src>, Span = SimpleSpan>>(
    keyword: &'static str,
) -> impl Parser<'src, I, (), extra::Err<Rich<'src, Token<'src>, SimpleSpan>>> + Clone {
    any()
        .filter(|t| {
            if let Token::Keyword(v) = t {
                v.eq_ignore_ascii_case(keyword)
            } else {
                false
            }
        })
        .ignored()
        .labelled(keyword)
}

fn case_sensitive_keyword<'src, I: ValueInput<'src, Token = Token<'src>, Span = SimpleSpan>>(
    keyword: &'static str,
) -> impl Parser<'src, I, (), extra::Err<Rich<'src, Token<'src>, SimpleSpan>>> + Clone {
    any()
        .filter(move |t| {
            if let Token::Keyword(v) = t {
                *v == keyword
            } else {
                false
            }
        })
        .ignored()
        .labelled(keyword)
}

fn operator<'src, I: ValueInput<'src, Token = Token<'src>, Span = SimpleSpan>>(
    op: &'static str,
) -> impl Parser<'src, I, (), extra::Err<Rich<'src, Token<'src>, SimpleSpan>>> + Clone {
    just(Token::Operator(op)).ignored().labelled(op)
}

fn parse(slice: &str) -> Result<Query, Vec<String>> {
    let state = Rc::new(RefCell::new(ParserState::new(
        None,
        HashMap::new(),
        HashSet::new(),
    )));
    let (tokens, lexer_errors) = lexer().parse(slice).into_output_errors();
    let (query, parse_errors) = if let Some(tokens) = &tokens {
        query_unit(Rc::clone(&state))
            .parse(tokens.as_slice())
            .into_output_errors()
    } else {
        (None, Vec::new())
    };
    let errors = lexer_errors
        .into_iter()
        .map(|e| e.to_string())
        .chain(parse_errors.into_iter().map(|e| e.to_string()))
        .collect::<Vec<_>>();
    if errors.is_empty() {
        if let Some(query) = query {
            return Ok(query);
        }
    }
    Err(errors)
}
#[test]
fn test() {
    parse("PREFIX ex: <http://example.com/> SELECT * WHERE { ?s ?p ex:foo , \"bar\" , 1.2 , _:a FILTER(true || 1 IN (2, 3)) } ORDER BY ?s LIMIT 1")
        .unwrap();
}
