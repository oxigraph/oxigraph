#![allow(clippy::ignored_unit_patterns)]
use crate::algebra::*;
use crate::query::*;
use crate::term::*;
use crate::update::*;
use oxilangtag::LanguageTag;
use oxiri::{Iri, IriParseError};
use oxrdf::vocab::{rdf, xsd};
use peg::parser;
use peg::str::LineCol;
use rand::random;
use std::char;
use std::collections::{HashMap, HashSet};
use std::mem::take;
use std::str::FromStr;

/// A SPARQL parser
///
/// ```
/// use spargebra::SparqlParser;
///
/// let query_str = "SELECT ?s ?p ?o WHERE { ?s ?p ?o . }";
/// let query = SparqlParser::new().parse_query(query_str)?;
/// assert_eq!(query.to_string(), query_str);
/// # Ok::<_, spargebra::SparqlSyntaxError>(())
/// ```
#[must_use]
#[derive(Clone, Default)]
pub struct SparqlParser {
    base_iri: Option<Iri<String>>,
    prefixes: HashMap<String, String>,
    custom_aggregate_functions: HashSet<NamedNode>,
}

impl SparqlParser {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Provides an IRI that could be used to resolve the operation relative IRIs.
    ///
    /// ```
    /// use spargebra::SparqlParser;
    ///
    /// let query = SparqlParser::new().with_base_iri("http://example.com/")?.parse_query("SELECT * WHERE { <s> <p> <o> }")?;
    /// assert_eq!(query.to_string(), "BASE <http://example.com/>\nSELECT * WHERE { <http://example.com/s> <http://example.com/p> <http://example.com/o> . }");
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_base_iri(mut self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        self.base_iri = Some(Iri::parse(base_iri.into())?);
        Ok(self)
    }

    /// Set a default IRI prefix used during parsing.
    ///
    /// ```
    /// use spargebra::SparqlParser;
    ///
    /// let query = SparqlParser::new()
    ///     .with_prefix("ex", "http://example.com/")?
    ///     .parse_query("SELECT * WHERE { ex:s ex:p ex:o }")?;
    /// assert_eq!(
    ///     query.to_string(),
    ///     "SELECT * WHERE { <http://example.com/s> <http://example.com/p> <http://example.com/o> . }"
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_prefix(
        mut self,
        prefix_name: impl Into<String>,
        prefix_iri: impl Into<String>,
    ) -> Result<Self, IriParseError> {
        self.prefixes.insert(
            prefix_name.into(),
            Iri::parse(prefix_iri.into())?.into_inner(),
        );
        Ok(self)
    }

    /// Adds a new function to be parsed as a custom aggregate function and not as a regular custom function.
    ///
    /// ```
    /// use oxrdf::NamedNode;
    /// use spargebra::SparqlParser;
    ///
    /// SparqlParser::new()
    ///     .with_custom_aggregate_function(NamedNode::new("http://example.com/concat")?)
    ///     .parse_query(
    ///         "PREFIX ex: <http://example.com/> SELECT (ex:concat(?o) AS ?concat) WHERE { ex:s ex:p ex:o }",
    ///     )?;
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_custom_aggregate_function(mut self, name: impl Into<NamedNode>) -> Self {
        self.custom_aggregate_functions.insert(name.into());
        self
    }

    /// Parse the given query string using the already set options.
    ///
    /// ```
    /// use spargebra::SparqlParser;
    ///
    /// let query_str = "SELECT ?s ?p ?o WHERE { ?s ?p ?o . }";
    /// let query = SparqlParser::new().parse_query(query_str)?;
    /// assert_eq!(query.to_string(), query_str);
    /// # Ok::<_, spargebra::SparqlSyntaxError>(())
    /// ```
    pub fn parse_query(self, query: &str) -> Result<Query, SparqlSyntaxError> {
        let mut state = ParserState::new(
            self.base_iri,
            self.prefixes,
            self.custom_aggregate_functions,
        );
        Ok(parser::QueryUnit(query, &mut state).map_err(SparqlSyntaxErrorKind::Syntax)?)
    }

    /// Parse the given update string using the already set options.
    ///
    /// ```
    /// use spargebra::SparqlParser;
    ///
    /// let update_str = "CLEAR ALL ;";
    /// let update = SparqlParser::new().parse_update(update_str)?;
    /// assert_eq!(update.to_string().trim(), update_str);
    /// # Ok::<_, spargebra::SparqlSyntaxError>(())
    /// ```
    pub fn parse_update(self, update: &str) -> Result<Update, SparqlSyntaxError> {
        let mut state = ParserState::new(
            self.base_iri,
            self.prefixes,
            self.custom_aggregate_functions,
        );
        let operations =
            parser::UpdateInit(update, &mut state).map_err(SparqlSyntaxErrorKind::Syntax)?;
        check_if_insert_data_are_sharing_blank_nodes(&operations)?;
        Ok(Update {
            operations,
            base_iri: state.base_iri,
        })
    }
}

/// Error returned during SPARQL parsing.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct SparqlSyntaxError {
    #[from]
    kind: SparqlSyntaxErrorKind,
}

impl SparqlSyntaxError {
    pub(crate) fn from_bad_base_iri(e: IriParseError) -> Self {
        SparqlSyntaxErrorKind::InvalidBaseIri(e).into()
    }
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

struct ReifiedTerm {
    term: TermPattern,
    reifiers: Vec<TermPattern>,
}

#[derive(Default)]
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
                object: triple.clone().object,
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

#[derive(Debug)]
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

#[derive(Debug, Default)]
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

enum SelectionVariables {
    Explicit(Vec<SelectionMember>),
    Star,
    Everything,
}

struct Selection {
    pub option: SelectionOption,
    pub variables: SelectionVariables,
}

impl Selection {
    fn no_op() -> Self {
        Self {
            option: SelectionOption::Default,
            variables: SelectionVariables::Everything,
        }
    }
}

fn build_select(
    select: Selection,
    r#where: GraphPattern,
    mut group: Option<(Vec<Variable>, Vec<(Expression, Variable)>)>,
    having: Option<Expression>,
    order_by: Option<Vec<OrderExpression>>,
    offset_limit: Option<(usize, Option<usize>)>,
    values: Option<GraphPattern>,
    state: &mut ParserState,
) -> Result<GraphPattern, &'static str> {
    let mut p = r#where;
    let mut with_aggregate = false;

    // GROUP BY
    let aggregates = state.aggregates.pop().unwrap_or_default();
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
                            return Err("The SELECT contains a variable that is unbound");
                        }
                        v
                    }
                    SelectionMember::Expression(expression, variable) => {
                        if visible.contains(&variable) {
                            // We disallow to override an existing variable with an expression
                            return Err(
                                "The SELECT overrides an existing variable using an expression",
                            );
                        }
                        if with_aggregate && !are_variables_bound(&expression, &visible) {
                            // We validate projection variables if there is an aggregate
                            return Err(
                                "The SELECT contains an expression with a variable that is unbound",
                            );
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
                    return Err("Duplicated variable name in SELECT");
                }
                pv.push(v)
            }
            true
        }
        SelectionVariables::Star => {
            if with_aggregate {
                return Err("SELECT * is not authorized with GROUP BY");
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
        SelectionVariables::Everything => false,
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

fn are_variables_bound(expression: &Expression, variables: &HashSet<Variable>) -> bool {
    match expression {
        Expression::NamedNode(_)
        | Expression::Literal(_)
        | Expression::Bound(_)
        | Expression::Coalesce(_)
        | Expression::Exists(_) => true,
        Expression::Variable(var) => variables.contains(var),
        Expression::UnaryPlus(e) | Expression::UnaryMinus(e) | Expression::Not(e) => {
            are_variables_bound(e, variables)
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
            are_variables_bound(a, variables) && are_variables_bound(b, variables)
        }
        Expression::In(a, b) => {
            are_variables_bound(a, variables) && b.iter().all(|b| are_variables_bound(b, variables))
        }
        Expression::FunctionCall(_, parameters) => {
            parameters.iter().all(|p| are_variables_bound(p, variables))
        }
        Expression::If(a, b, c) => {
            are_variables_bound(a, variables)
                && are_variables_bound(b, variables)
                && are_variables_bound(c, variables)
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
    aggregates: Vec<Vec<(Variable, AggregateExpression)>>,
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
        let aggregates = self.aggregates.last_mut().ok_or("Unexpected aggregate")?;
        Ok(aggregates
            .iter()
            .find_map(|(v, a)| (a == &agg).then_some(v))
            .cloned()
            .unwrap_or_else(|| {
                let new_var = variable();
                aggregates.push((new_var.clone(), agg));
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

parser! {
    //See https://www.w3.org/TR/turtle/#sec-grammar
    grammar parser(state: &mut ParserState) for str {
        pub rule QueryUnit() -> Query = Query()

        rule Query() -> Query = _ Prologue() _ q:(SelectQuery() / ConstructQuery() / DescribeQuery() / AskQuery()) _ {
            q
        }

        pub rule UpdateInit() -> Vec<GraphUpdateOperation> = Update()

        rule Prologue() = (BaseDecl() _ / PrefixDecl() _)* {}

        rule BaseDecl() = i("BASE") _ i:IRIREF() {
            state.base_iri = Some(i)
        }

        rule PrefixDecl() = i("PREFIX") _ ns:PNAME_NS() _ i:IRIREF() {
            state.prefixes.insert(ns.into(), i.into_inner());
        }

        rule SelectQuery() -> Query = s:SelectClause() _ d:DatasetClauses() _ w:WhereClause() _ g:GroupClause()? _ h:HavingClause()? _ o:OrderClause()? _ l:LimitOffsetClauses()? _ v:ValuesClause() {?
            Ok(Query::Select {
                dataset: d,
                pattern: build_select(s, w, g, h, o, l, v, state)?,
                base_iri: state.base_iri.clone()
            })
        }

        rule SubSelect() -> GraphPattern = s:SelectClause() _ w:WhereClause() _ g:GroupClause()? _ h:HavingClause()? _ o:OrderClause()? _ l:LimitOffsetClauses()? _ v:ValuesClause() {?
            build_select(s, w, g, h, o, l, v, state)
        }

        rule SelectClause() -> Selection = i("SELECT") _ Selection_init() o:SelectClause_option() _ v:SelectClause_variables() {
            Selection {
                option: o,
                variables: v
            }
        }
        rule Selection_init() = {
            state.aggregates.push(Vec::new())
        }
        rule SelectClause_option() -> SelectionOption =
            i("DISTINCT") { SelectionOption::Distinct } /
            i("REDUCED") { SelectionOption::Reduced } /
            { SelectionOption::Default }
        rule SelectClause_variables() -> SelectionVariables =
            "*" { SelectionVariables::Star } /
            p:SelectClause_member()+ { SelectionVariables::Explicit(p) }
        rule SelectClause_member() -> SelectionMember =
            v:Var() _ { SelectionMember::Variable(v) } /
            "(" _ e:Expression() _ i("AS") _ v:Var() _ ")" _ { SelectionMember::Expression(e, v) }

        rule ConstructQuery() -> Query =
            i("CONSTRUCT") _ c:ConstructTemplate() ConstructQuery_clear() _ d:DatasetClauses() _ w:WhereClause() _ g:GroupClause()? _ h:HavingClause()? _ o:OrderClause()? _ l:LimitOffsetClauses()? _ v:ValuesClause() {?
                Ok(Query::Construct {
                    template: c,
                    dataset: d,
                    pattern: build_select(Selection::no_op(), w, g, h, o, l, v, state)?,
                    base_iri: state.base_iri.clone()
                })
            } /
            i("CONSTRUCT") _ d:DatasetClauses() _ i("WHERE") _ "{" _ c:ConstructQuery_optional_triple_template() _ "}" _ g:GroupClause()? _ h:HavingClause()? _ o:OrderClause()? _ l:LimitOffsetClauses()? _ v:ValuesClause() {?
                Ok(Query::Construct {
                    template: c.clone(),
                    dataset: d,
                    pattern: build_select(
                        Selection::no_op(),
                        GraphPattern::Bgp { patterns: c },
                        g, h, o, l, v, state
                    )?,
                    base_iri: state.base_iri.clone()
                })
            }
        rule ConstructQuery_clear() = {
            state.currently_used_bnodes.clear();
        }

        rule ConstructQuery_optional_triple_template() -> Vec<TriplePattern> = TriplesTemplate() / { Vec::new() }

        rule DescribeQuery() -> Query =
            i("DESCRIBE") _ "*" _ d:DatasetClauses() _ w:WhereClause()? _ g:GroupClause()? _ h:HavingClause()? _ o:OrderClause()? _ l:LimitOffsetClauses()? _ v:ValuesClause() {?
                Ok(Query::Describe {
                    dataset: d,
                    pattern: build_select(Selection::no_op(), w.unwrap_or_default(), g, h, o, l, v, state)?,
                    base_iri: state.base_iri.clone()
                })
            } /
            i("DESCRIBE") _ p:DescribeQuery_item()+ _ d:DatasetClauses() _ w:WhereClause()? _ g:GroupClause()? _ h:HavingClause()? _ o:OrderClause()? _ l:LimitOffsetClauses()? _ v:ValuesClause() {?
                Ok(Query::Describe {
                    dataset: d,
                    pattern: build_select(Selection {
                        option: SelectionOption::Default,
                        variables: SelectionVariables::Explicit(p.into_iter().map(|var_or_iri| match var_or_iri {
                            NamedNodePattern::NamedNode(n) => SelectionMember::Expression(n.into(), variable()),
                            NamedNodePattern::Variable(v) => SelectionMember::Variable(v)
                        }).collect())
                    }, w.unwrap_or_default(), g, h, o, l, v, state)?,
                    base_iri: state.base_iri.clone()
                })
            }
        rule DescribeQuery_item() -> NamedNodePattern = i:VarOrIri() _ { i }

        rule AskQuery() -> Query = i("ASK") _ d:DatasetClauses() _ w:WhereClause() _ g:GroupClause()? _ h:HavingClause()? _ o:OrderClause()? _ l:LimitOffsetClauses()? _ v:ValuesClause() {?
            Ok(Query::Ask {
                dataset: d,
                pattern: build_select(Selection::no_op(), w, g, h, o, l, v, state)?,
                base_iri: state.base_iri.clone()
            })
        }

        rule DatasetClause() -> (Option<NamedNode>, Option<NamedNode>) = i("FROM") _ d:(DefaultGraphClause() / NamedGraphClause()) { d }
        rule DatasetClauses() -> Option<QueryDataset> = d:DatasetClause() ** (_) {
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
                default, named: Some(named)
            })
        }

        rule DefaultGraphClause() -> (Option<NamedNode>, Option<NamedNode>) = s:SourceSelector() {
            (Some(s), None)
        }

        rule NamedGraphClause() -> (Option<NamedNode>, Option<NamedNode>) = i("NAMED") _ s:SourceSelector() {
            (None, Some(s))
        }

        rule SourceSelector() -> NamedNode = iri()

        rule WhereClause() -> GraphPattern = i("WHERE")? _ p:GroupGraphPattern() {
            p
        }

        rule GroupClause() -> (Vec<Variable>, Vec<(Expression,Variable)>) = i("GROUP") _ i("BY") _ c:GroupCondition_item()+ {
            let mut projections: Vec<(Expression,Variable)> = Vec::new();
            let clauses = c.into_iter().map(|(e, vo)| {
                if let Expression::Variable(v) = e {
                    v
                } else {
                    let v = vo.unwrap_or_else(variable);
                    projections.push((e, v.clone()));
                    v
                }
            }).collect();
            (clauses, projections)
        }
        rule GroupCondition_item() -> (Expression, Option<Variable>) = c:GroupCondition() _ { c }

        rule GroupCondition() -> (Expression, Option<Variable>) =
            e:BuiltInCall() { (e, None) } /
            e:FunctionCall() { (e, None) } /
            "(" _ e:Expression() _ v:GroupCondition_as()? ")" { (e, v) } /
            e:Var() { (e.into(), None) }
        rule GroupCondition_as() -> Variable = i("AS") _ v:Var() _ { v }

        rule HavingClause() -> Expression = i("HAVING") _ e:HavingCondition()+ {?
            not_empty_fold(e.into_iter(), |a, b| Expression::And(Box::new(a), Box::new(b)))
        }

        rule HavingCondition() -> Expression = Constraint()

        rule OrderClause() -> Vec<OrderExpression> = i("ORDER") _ i("BY") _ c:OrderClause_item()+ { c }
        rule OrderClause_item() -> OrderExpression = c:OrderCondition() _ { c }

        rule OrderCondition() -> OrderExpression =
            i("ASC") _ e: BrackettedExpression() { OrderExpression::Asc(e) } /
            i("DESC") _ e: BrackettedExpression() { OrderExpression::Desc(e) } /
            e: Constraint() { OrderExpression::Asc(e) } /
            v: Var() { OrderExpression::Asc(Expression::from(v)) }

        rule LimitOffsetClauses() -> (usize, Option<usize>) =
            l:LimitClause() _ o:OffsetClause()? { (o.unwrap_or(0), Some(l)) } /
            o:OffsetClause() _ l:LimitClause()? { (o, l) }

        rule LimitClause() -> usize = i("LIMIT") _ l:$(INTEGER()) {?
            usize::from_str(l).map_err(|_| "The query limit should be a non negative integer")
        }

        rule OffsetClause() -> usize = i("OFFSET") _ o:$(INTEGER()) {?
            usize::from_str(o).map_err(|_| "The query offset should be a non negative integer")
        }

        rule ValuesClause() -> Option<GraphPattern> =
            i("VALUES") _ p:DataBlock() { Some(p) } /
            { None }

        rule Update() -> Vec<GraphUpdateOperation> = _ Prologue() _ u:(Update_item() ** (";" _))  ( ";" _)? { u.into_iter().flatten().collect() }
        rule Update_item() -> Vec<GraphUpdateOperation> = u:Update1() Update_clear() _ { u }
        rule Update_clear() = {
            state.used_bnodes.clear();
            state.currently_used_bnodes.clear();
        }

        rule Update1() -> Vec<GraphUpdateOperation> = Load() / Clear() / Drop() / Add() / Move() / Copy() / Create() / InsertData() / DeleteData() / DeleteWhere() / Modify()
        rule Update1_silent() -> bool = i("SILENT") { true } / { false }

        rule Load() -> Vec<GraphUpdateOperation> = i("LOAD") _ silent:Update1_silent() _ source:iri() _ destination:Load_to()? {
            vec![GraphUpdateOperation::Load { silent, source, destination: destination.map_or(GraphName::DefaultGraph, GraphName::NamedNode) }]
        }
        rule Load_to() -> NamedNode = i("INTO") _ g: GraphRef() { g }

        rule Clear() -> Vec<GraphUpdateOperation> = i("CLEAR") _ silent:Update1_silent() _ graph:GraphRefAll() {
            vec![GraphUpdateOperation::Clear { silent, graph }]
        }

        rule Drop() -> Vec<GraphUpdateOperation> = i("DROP") _ silent:Update1_silent() _ graph:GraphRefAll() {
            vec![GraphUpdateOperation::Drop { silent, graph }]
        }

        rule Create() -> Vec<GraphUpdateOperation> = i("CREATE") _ silent:Update1_silent() _ graph:GraphRef() {
            vec![GraphUpdateOperation::Create { silent, graph }]
        }

        rule Add() -> Vec<GraphUpdateOperation> = i("ADD") _ silent:Update1_silent() _ from:GraphOrDefault() _ i("TO") _ to:GraphOrDefault() {
            // Rewriting defined by https://www.w3.org/TR/sparql11-update/#add
            if from == to {
                Vec::new() // identity case
            } else {
                let bgp = GraphPattern::Bgp { patterns: vec![TriplePattern::new(Variable::new_unchecked("s"), Variable::new_unchecked("p"), Variable::new_unchecked("o"))] };
                vec![copy_graph(from, to)]
            }
        }

        rule Move() -> Vec<GraphUpdateOperation> = i("MOVE") _ silent:Update1_silent() _ from:GraphOrDefault() _ i("TO") _ to:GraphOrDefault() {
            // Rewriting defined by https://www.w3.org/TR/sparql11-update/#move
            if from == to {
                Vec::new() // identity case
            } else {
                let bgp = GraphPattern::Bgp { patterns: vec![TriplePattern::new(Variable::new_unchecked("s"), Variable::new_unchecked("p"), Variable::new_unchecked("o"))] };
                vec![GraphUpdateOperation::Drop { silent: true, graph: to.clone().into() }, copy_graph(from.clone(), to), GraphUpdateOperation::Drop { silent, graph: from.into() }]
            }
        }

        rule Copy() -> Vec<GraphUpdateOperation> = i("COPY") _ silent:Update1_silent() _ from:GraphOrDefault() _ i("TO") _ to:GraphOrDefault() {
            // Rewriting defined by https://www.w3.org/TR/sparql11-update/#copy
            if from == to {
                Vec::new() // identity case
            } else {
                let bgp = GraphPattern::Bgp { patterns: vec![TriplePattern::new(Variable::new_unchecked("s"), Variable::new_unchecked("p"), Variable::new_unchecked("o"))] };
                vec![GraphUpdateOperation::Drop { silent: true, graph: to.clone().into() }, copy_graph(from, to)]
            }
        }

        rule InsertData() -> Vec<GraphUpdateOperation> = i("INSERT") _ i("DATA") _ data:QuadData() {
            vec![GraphUpdateOperation::InsertData { data }]
        }

        rule DeleteData() -> Vec<GraphUpdateOperation> = i("DELETE") _ i("DATA") _ data:GroundQuadData() {
            vec![GraphUpdateOperation::DeleteData { data }]
        }

        rule DeleteWhere() -> Vec<GraphUpdateOperation> = i("DELETE") _ i("WHERE") _ d:QuadPattern() {?
            let pattern = d.iter().map(|q| {
                let bgp = GraphPattern::Bgp { patterns: vec![TriplePattern::new(q.subject.clone(), q.predicate.clone(), q.object.clone())] };
                match &q.graph_name {
                    GraphNamePattern::NamedNode(graph_name) => GraphPattern::Graph { name: graph_name.clone().into(), inner: Box::new(bgp) },
                    GraphNamePattern::DefaultGraph => bgp,
                    GraphNamePattern::Variable(graph_name) => GraphPattern::Graph { name: graph_name.clone().into(), inner: Box::new(bgp) },
                }
            }).reduce(new_join).unwrap_or_default();
            let delete = d.into_iter().map(GroundQuadPattern::try_from).collect::<Result<Vec<_>,_>>().map_err(|()| "Blank nodes are not allowed in DELETE WHERE")?;
            Ok(vec![GraphUpdateOperation::DeleteInsert {
                delete,
                insert: Vec::new(),
                using: None,
                pattern: Box::new(pattern)
            }])
        }

        rule Modify() -> Vec<GraphUpdateOperation> = with:Modify_with()? _ c:Modify_clauses() _ u:(UsingClause() ** (_)) _ i("WHERE") _ pattern:GroupGraphPattern() {
            let (delete, insert) = c;
            let mut delete = delete.unwrap_or_default();
            let mut insert = insert.unwrap_or_default();
            #[expect(clippy::shadow_same)]
            let mut pattern = pattern;

            let mut using = if u.is_empty() {
                None
            } else {
                let mut default = Vec::new();
                let mut named = Vec::new();
                for (d, n) in u {
                    if let Some(d) = d {
                        default.push(d)
                    }
                    if let Some(n) = n {
                        named.push(n)
                    }
                }
                Some(QueryDataset { default, named: Some(named) })
            };

            if let Some(with) = with {
                // We inject WITH everywhere
                delete = delete.into_iter().map(|q| if q.graph_name == GraphNamePattern::DefaultGraph {
                    GroundQuadPattern {
                        subject: q.subject,
                        predicate: q.predicate,
                        object: q.object,
                        graph_name: with.clone().into()
                    }
                } else {
                    q
                }).collect();
                insert = insert.into_iter().map(|q| if q.graph_name == GraphNamePattern::DefaultGraph {
                    QuadPattern {
                        subject: q.subject,
                        predicate: q.predicate,
                        object: q.object,
                        graph_name: with.clone().into()
                    }
                } else {
                    q
                }).collect();
                if using.is_none() {
                    using = Some(QueryDataset { default: vec![with], named: None });
                }
            }

            vec![GraphUpdateOperation::DeleteInsert {
                delete,
                insert,
                using,
                pattern: Box::new(pattern)
            }]
        }
        rule Modify_with() -> NamedNode = i("WITH") _ i:iri() _ { i }
        rule Modify_clauses() -> (Option<Vec<GroundQuadPattern>>, Option<Vec<QuadPattern>>) = d:DeleteClause() Modify_clear() _ i:InsertClause()? Modify_clear() {
            (Some(d), i)
        } / i:InsertClause() Modify_clear() {
            (None, Some(i))
        }
        rule Modify_clear() = {
            state.currently_used_bnodes.clear();
        }

        rule DeleteClause() -> Vec<GroundQuadPattern> = i("DELETE") _ q:QuadPattern() {?
            q.into_iter().map(GroundQuadPattern::try_from).collect::<Result<Vec<_>,_>>().map_err(|()| "Blank nodes are not allowed in DELETE WHERE")
        }

        rule InsertClause() -> Vec<QuadPattern> = i("INSERT") _ q:QuadPattern() { q }

        rule UsingClause() -> (Option<NamedNode>, Option<NamedNode>) = i("USING") _ d:(UsingClause_default() / UsingClause_named()) { d }
        rule UsingClause_default() -> (Option<NamedNode>, Option<NamedNode>) = i:iri() {
            (Some(i), None)
        }
        rule UsingClause_named() -> (Option<NamedNode>, Option<NamedNode>) = i("NAMED") _ i:iri() {
            (None, Some(i))
        }

        rule GraphOrDefault() -> GraphName = i("DEFAULT") {
            GraphName::DefaultGraph
        } / (i("GRAPH") _)? g:iri() {
            GraphName::NamedNode(g)
        }

        rule GraphRef() -> NamedNode = i("GRAPH") _ g:iri() { g }

        rule GraphRefAll() -> GraphTarget  = i: GraphRef() { i.into() }
            / i("DEFAULT") { GraphTarget::DefaultGraph }
            / i("NAMED") { GraphTarget::NamedGraphs }
            / i("ALL") { GraphTarget::AllGraphs }

        rule QuadPattern() -> Vec<QuadPattern> = "{" _ q:Quads() _ "}" { q }

        rule QuadData() -> Vec<Quad> = "{" _ q:Quads() _ "}" {?
            q.into_iter().map(Quad::try_from).collect::<Result<Vec<_>, ()>>().map_err(|()| "Variables are not allowed in INSERT DATA")
        }
        rule GroundQuadData() -> Vec<GroundQuad> = "{" _ q:Quads() _ "}" {?
            q.into_iter().map(|q| GroundQuad::try_from(Quad::try_from(q)?)).collect::<Result<Vec<_>, ()>>().map_err(|()| "Variables and blank nodes are not allowed in DELETE DATA")
        }

        rule Quads() -> Vec<QuadPattern> = q:(Quads_TriplesTemplate() / Quads_QuadsNotTriples()) ** (_) {
            q.into_iter().flatten().collect()
        }
        rule Quads_TriplesTemplate() -> Vec<QuadPattern> = t:TriplesTemplate() {
            t.into_iter().map(|t| QuadPattern::new(t.subject, t.predicate, t.object, GraphNamePattern::DefaultGraph)).collect()
        } //TODO: return iter?
        rule Quads_QuadsNotTriples() -> Vec<QuadPattern> = q:QuadsNotTriples() _ "."? { q }

        rule QuadsNotTriples() -> Vec<QuadPattern> = i("GRAPH") _ g:VarOrIri() _ "{" _ t:TriplesTemplate()? _ "}" {
            t.unwrap_or_default().into_iter().map(|t| QuadPattern::new(t.subject, t.predicate, t.object, g.clone())).collect()
        }

        rule TriplesTemplate() -> Vec<TriplePattern> = ts:TriplesTemplate_inner() ++ (".") ("." _)? {
            ts.into_iter().flatten().collect()
        }
        rule TriplesTemplate_inner() -> Vec<TriplePattern> = _ t:TriplesSameSubject() _ { t }

        rule GroupGraphPattern() -> GraphPattern =
            "{" _ GroupGraphPattern_clear() p:GroupGraphPatternSub() GroupGraphPattern_clear() _ "}" { p } /
            "{" _ GroupGraphPattern_clear() p:SubSelect() GroupGraphPattern_clear() _ "}" { p }
        rule GroupGraphPattern_clear() = {
             // We deal with blank nodes aliases rule
            state.used_bnodes.extend(state.currently_used_bnodes.iter().cloned());
            state.currently_used_bnodes.clear();
        }

        rule GroupGraphPatternSub() -> GraphPattern = a:TriplesBlock()? _ b:GroupGraphPatternSub_item()* {?
            let mut filter: Option<Expression> = None;
            let mut g = a.map_or_else(GraphPattern::default, build_bgp);
            for e in b.into_iter().flatten() {
                match e {
                    PartialGraphPattern::Optional(p, f) => {
                        g = GraphPattern::LeftJoin { left: Box::new(g), right: Box::new(p), expression: f }
                    }
                    #[cfg(feature = "sep-0006")]
                    PartialGraphPattern::Lateral(p) => {
                        let mut defined_variables = HashSet::new();
                        add_defined_variables(&p, &mut defined_variables);
                        let mut contains = false;
                        g.on_in_scope_variable(|v| {
                            if defined_variables.contains(v) {
                                contains = true;
                            }
                        });
                        if contains {
                            return Err("An existing variable is overridden in the right side of LATERAL");
                        }
                        g = GraphPattern::Lateral { left: Box::new(g), right: Box::new(p) }
                    }
                    PartialGraphPattern::Minus(p) => {
                        g = GraphPattern::Minus { left: Box::new(g), right: Box::new(p) }
                    }
                    PartialGraphPattern::Bind(expression, variable) => {
                        let mut contains = false;
                        g.on_in_scope_variable(|v| {
                            if *v == variable {
                                contains = true;
                            }
                        });
                        if contains {
                            return Err("BIND is overriding an existing variable")
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
        }
        rule GroupGraphPatternSub_item() -> Vec<PartialGraphPattern> = a:GraphPatternNotTriples() _ ("." _)? b:TriplesBlock()? _ {
            let mut result = vec![a];
            if let Some(v) = b {
                result.push(PartialGraphPattern::Other(build_bgp(v)));
            }
            result
        }

        rule TriplesBlock() -> Vec<TripleOrPathPattern> = hs:TriplesBlock_inner() ++ (".") ("." _)? {
            hs.into_iter().flatten().collect()
        }
        rule TriplesBlock_inner() -> Vec<TripleOrPathPattern> = _ h:TriplesSameSubjectPath() _ { h }

        rule ReifiedTripleBlock() -> Vec<TriplePattern> = s:ReifiedTriple() _ po:PropertyList() {?
            let mut patterns = po.patterns;
            patterns.extend(s.patterns);
            for (p, os) in po.focus {
                for o in os {
                    add_to_triple_patterns(s.focus.clone(), p.clone(), o, &mut patterns)?;
                }
            }
            Ok(patterns)
        }

        rule ReifiedTripleBlockPath() -> Vec<TripleOrPathPattern> = s:ReifiedTriple() _ po:PropertyListPath() {?
            let mut patterns = po.patterns;
            patterns.extend(s.patterns.into_iter().map(Into::into));
            for (p, os) in po.focus {
                for o in os {
                    add_to_triple_or_path_patterns(s.focus.clone(), p.clone(), o, &mut patterns)?;
                }
            }
            Ok(patterns)
        }

        rule GraphPatternNotTriples() -> PartialGraphPattern = GroupOrUnionGraphPattern() / OptionalGraphPattern() / LateralGraphPattern() / MinusGraphPattern() / GraphGraphPattern() / ServiceGraphPattern() / Filter() / Bind() / InlineData()

        rule OptionalGraphPattern() -> PartialGraphPattern = i("OPTIONAL") _ p:GroupGraphPattern() {
            if let GraphPattern::Filter { expr, inner } =  p {
               PartialGraphPattern::Optional(*inner, Some(expr))
            } else {
               PartialGraphPattern::Optional(p, None)
            }
        }

        rule LateralGraphPattern() -> PartialGraphPattern = i("LATERAL") _ p:GroupGraphPattern() {?
                #[cfg(feature = "sep-0006")]{Ok(PartialGraphPattern::Lateral(p))}
                #[cfg(not(feature = "sep-0006"))]{Err("The LATERAL modifier is not supported")}
        }

        rule GraphGraphPattern() -> PartialGraphPattern = i("GRAPH") _ name:VarOrIri() _ p:GroupGraphPattern() {
            PartialGraphPattern::Other(GraphPattern::Graph { name, inner: Box::new(p) })
        }

        rule ServiceGraphPattern() -> PartialGraphPattern =
            i("SERVICE") _ i("SILENT") _ name:VarOrIri() _ p:GroupGraphPattern() { PartialGraphPattern::Other(GraphPattern::Service { name, inner: Box::new(p), silent: true }) } /
            i("SERVICE") _ name:VarOrIri() _ p:GroupGraphPattern() { PartialGraphPattern::Other(GraphPattern::Service{ name, inner: Box::new(p), silent: false }) }

        rule Bind() -> PartialGraphPattern = i("BIND") _ "(" _ e:Expression() _ i("AS") _ v:Var() _ ")" {
            PartialGraphPattern::Bind(e, v)
        }

        rule InlineData() -> PartialGraphPattern = i("VALUES") _ p:DataBlock() { PartialGraphPattern::Other(p) }

        rule DataBlock() -> GraphPattern = l:(InlineDataOneVar() / InlineDataFull()) {
            GraphPattern::Values { variables: l.0, bindings: l.1 }
        }

        rule InlineDataOneVar() -> (Vec<Variable>, Vec<Vec<Option<GroundTerm>>>) = var:Var() _ "{" _ d:InlineDataOneVar_value()* "}" {
            (vec![var], d)
        }
        rule InlineDataOneVar_value() -> Vec<Option<GroundTerm>> = t:DataBlockValue() _ { vec![t] }

        rule InlineDataFull() -> (Vec<Variable>, Vec<Vec<Option<GroundTerm>>>) = "(" _ vars:InlineDataFull_var()* _ ")" _ "{" _ vals:InlineDataFull_values()* "}" {?
            if vals.iter().all(|vs| vs.len() == vars.len()) {
                Ok((vars, vals))
            } else {
                Err("The VALUES clause rows should have exactly the same number of values as there are variables. To set a value to undefined use UNDEF.")
            }
        }
        rule InlineDataFull_var() -> Variable = v:Var() _ { v }
        rule InlineDataFull_values() -> Vec<Option<GroundTerm>> = "(" _ v:InlineDataFull_value()* _ ")" _ { v }
        rule InlineDataFull_value() -> Option<GroundTerm> = v:DataBlockValue() _ { v }

        rule DataBlockValue() -> Option<GroundTerm> =
            t:TripleTermData() {?
                #[cfg(feature = "sparql-12")]{Ok(Some(t.into()))}
                #[cfg(not(feature = "sparql-12"))]{Err("Triple terms are only available in SPARQL 1.2")}
            } /
            i:iri() { Some(i.into()) } /
            l:RDFLiteral() { Some(l.into()) } /
            l:NumericLiteral() { Some(l.into()) } /
            l:BooleanLiteral() { Some(l.into()) } /
            i("UNDEF") { None }

        rule Reifier() -> TermPattern = "~" _ v:VarOrReifierId()? { v.unwrap_or_else(|| BlankNode::default().into()) }

        rule VarOrReifierId() -> TermPattern =
            v:Var() { v.into() } /
            i:iri() { i.into() } /
            b:BlankNode() { b.into() }

        rule MinusGraphPattern() -> PartialGraphPattern = i("MINUS") _ p: GroupGraphPattern() {
            PartialGraphPattern::Minus(p)
        }

        rule GroupOrUnionGraphPattern() -> PartialGraphPattern = p:GroupOrUnionGraphPattern_item() **<1,> (i("UNION") _) {?
            not_empty_fold(p.into_iter(), |a, b| {
                GraphPattern::Union { left: Box::new(a), right: Box::new(b) }
            }).map(PartialGraphPattern::Other)
        }
        rule GroupOrUnionGraphPattern_item() -> GraphPattern = p:GroupGraphPattern() _ { p }

        rule Filter() -> PartialGraphPattern = i("FILTER") _ c:Constraint() {
            PartialGraphPattern::Filter(c)
        }

        rule Constraint() -> Expression = BrackettedExpression() / FunctionCall() / BuiltInCall()

        rule FunctionCall() -> Expression = f:iri() _ a:ArgList() {?
            if state.custom_aggregate_functions.contains(&f) {
                Err("This custom function is an aggregate function and not a regular function")
            } else {
                Ok(Expression::FunctionCall(Function::Custom(f), a))
            }
        }

        rule ArgList() -> Vec<Expression> =
            "(" _ e:ArgList_item() **<1,> ("," _) _ ")" { e } /
            NIL() { Vec::new() }
        rule ArgList_item() -> Expression = e:Expression() _ { e }

        rule ExpressionList() -> Vec<Expression> =
            "(" _ e:ExpressionList_item() **<1,> ("," _) ")" { e } /
            NIL() { Vec::new() }
        rule ExpressionList_item() -> Expression = e:Expression() _ { e }

        rule ConstructTemplate() -> Vec<TriplePattern> = "{" _ t:ConstructTriples() _ "}" { t }

        rule ConstructTriples() -> Vec<TriplePattern> = p:ConstructTriples_item() ** ("." _) "."? {
            p.into_iter().flatten().collect()
        }
        rule ConstructTriples_item() -> Vec<TriplePattern> = t:TriplesSameSubject() _ { t }

        rule TriplesSameSubject() -> Vec<TriplePattern> =
            ReifiedTripleBlock() /
            s:VarOrTerm() _ po:PropertyListNotEmpty() {?
                let mut patterns = po.patterns;
                for (p, os) in po.focus {
                    for o in os {
                        add_to_triple_patterns(s.clone(), p.clone(), o, &mut patterns)?
                    }
                }
                Ok(patterns)
            } /
            s:TriplesNode() _ po:PropertyList() {?
                let mut patterns = s.patterns;
                patterns.extend(po.patterns);
                for (p, os) in po.focus {
                    for o in os {
                        add_to_triple_patterns(s.focus.clone(), p.clone(), o, &mut patterns)?
                    }
                }
                Ok(patterns)
            }

        rule PropertyList() -> FocusedTriplePattern<Vec<(NamedNodePattern,Vec<ReifiedTerm>)>> =
            PropertyListNotEmpty() /
            { FocusedTriplePattern::default() }

        rule PropertyListNotEmpty() -> FocusedTriplePattern<Vec<(NamedNodePattern,Vec<ReifiedTerm>)>> = hp:Verb() _ ho:ObjectList() _ l:PropertyListNotEmpty_item()* {
            l.into_iter().flatten().fold(FocusedTriplePattern {
                focus: vec![(hp, ho.focus)],
                patterns: ho.patterns
            }, |mut a, b| {
                a.focus.push(b.focus);
                a.patterns.extend(b.patterns);
                a
            })
        }
        rule PropertyListNotEmpty_item() -> Option<FocusedTriplePattern<(NamedNodePattern,Vec<ReifiedTerm>)>> = ";" _ c:PropertyListNotEmpty_item_content()? {
            c
        }
        rule PropertyListNotEmpty_item_content() -> FocusedTriplePattern<(NamedNodePattern,Vec<ReifiedTerm>)> = p:Verb() _ o:ObjectList() _ {
            FocusedTriplePattern {
                focus: (p, o.focus),
                patterns: o.patterns
            }
        }

        rule Verb() -> NamedNodePattern = VarOrIri() / "a" { rdf::TYPE.into_owned().into() }

        rule ObjectList() -> FocusedTriplePattern<Vec<ReifiedTerm >> = o:ObjectList_item() **<1,> ("," _) {
            o.into_iter().fold(FocusedTriplePattern::<Vec<ReifiedTerm >>::default(), |mut a, b| {
                a.focus.push(b.focus);
                a.patterns.extend_from_slice(&b.patterns);
                a
            })
        }
        rule ObjectList_item() -> FocusedTriplePattern<ReifiedTerm> = o:Object() _ { o }

        rule Object() -> FocusedTriplePattern<ReifiedTerm> = g:GraphNode() _ a:Annotation()? {
            if let Some(a) = a {
                let mut patterns = g.patterns;
                patterns.extend(a.patterns);
                FocusedTriplePattern {
                    focus: ReifiedTerm {
                        term: g.focus,
                        reifiers: a.focus
                    },
                    patterns
                }
            } else {
                FocusedTriplePattern {
                    focus: ReifiedTerm {
                        term: g.focus,
                        reifiers: Vec::new()
                    },
                    patterns: g.patterns
                }
            }
        }

        rule TriplesSameSubjectPath() -> Vec<TripleOrPathPattern> =
            ReifiedTripleBlockPath() /
            s:VarOrTerm() _ po:PropertyListPathNotEmpty() {?
                let mut patterns = po.patterns;
                for (p, os) in po.focus {
                    for o in os {
                        add_to_triple_or_path_patterns(s.clone(), p.clone(), o, &mut patterns)?;
                    }
                }
                Ok(patterns)
            } /
            s:TriplesNodePath() _ po:PropertyListPath() {?
                let mut patterns = s.patterns;
                patterns.extend(po.patterns);
                for (p, os) in po.focus {
                    for o in os {
                        add_to_triple_or_path_patterns(s.focus.clone(), p.clone(), o, &mut patterns)?;
                    }
                }
                Ok(patterns)
            }

        rule PropertyListPath() -> FocusedTripleOrPathPattern<Vec<(VariableOrPropertyPath,Vec<ReifiedTerm>)>> =
            PropertyListPathNotEmpty() /
            { FocusedTripleOrPathPattern::default() }

        rule PropertyListPathNotEmpty() -> FocusedTripleOrPathPattern<Vec<(VariableOrPropertyPath,Vec<ReifiedTerm>)>> = hp:(VerbPath() / VerbSimple()) _ ho:ObjectListPath() _ t:PropertyListPathNotEmpty_item()* {
                t.into_iter().flatten().fold(FocusedTripleOrPathPattern {
                    focus: vec![(hp, ho.focus)],
                    patterns: ho.patterns
                }, |mut a, b| {
                    a.focus.push(b.focus);
                    a.patterns.extend(b.patterns);
                    a
                })
        }
        rule PropertyListPathNotEmpty_item() -> Option<FocusedTripleOrPathPattern<(VariableOrPropertyPath,Vec<ReifiedTerm>)>> = ";" _ c:PropertyListPathNotEmpty_item_content()? {
            c
        }
        rule PropertyListPathNotEmpty_item_content() -> FocusedTripleOrPathPattern<(VariableOrPropertyPath,Vec<ReifiedTerm>)> = p:(VerbPath() / VerbSimple()) _ o:ObjectListPath() _ {
            FocusedTripleOrPathPattern {
                focus: (p, o.focus),
                patterns: o.patterns
            }
        }

        rule VerbPath() -> VariableOrPropertyPath = p:Path() {
            p.into()
        }

        rule VerbSimple() -> VariableOrPropertyPath = v:Var() {
            v.into()
        }

        rule ObjectListPath() -> FocusedTripleOrPathPattern<Vec<ReifiedTerm>> = o:ObjectListPath_item() **<1,> ("," _) {
            o.into_iter().fold(FocusedTripleOrPathPattern::<Vec<ReifiedTerm>>::default(), |mut a, b| {
                a.focus.push(b.focus);
                a.patterns.extend(b.patterns);
                a
            })
        }
        rule ObjectListPath_item() -> FocusedTripleOrPathPattern<ReifiedTerm> = o:ObjectPath() _ { o }

        rule ObjectPath() -> FocusedTripleOrPathPattern<ReifiedTerm> = g:GraphNodePath() _ a:AnnotationPath()? {
             if let Some(a) = a {
                let mut patterns = g.patterns;
                patterns.extend(a.patterns);
                FocusedTripleOrPathPattern {
                    focus: ReifiedTerm {
                        term: g.focus,
                        reifiers: a.focus
                    },
                    patterns
                }
            } else {
                FocusedTripleOrPathPattern {
                    focus: ReifiedTerm {
                        term: g.focus,
                        reifiers: Vec::new()
                    },
                    patterns: g.patterns
                }
            }
        }

        rule Path() -> PropertyPathExpression = PathAlternative()

        rule PathAlternative() -> PropertyPathExpression = p:PathAlternative_item() **<1,> ("|" _) {?
            not_empty_fold(p.into_iter(), |a, b| {
                PropertyPathExpression::Alternative(Box::new(a), Box::new(b))
            })
        }
        rule PathAlternative_item() -> PropertyPathExpression = p:PathSequence() _ { p }

        rule PathSequence() -> PropertyPathExpression = p:PathSequence_item() **<1,> ("/" _) {?
            not_empty_fold(p.into_iter(), |a, b| {
                PropertyPathExpression::Sequence(Box::new(a), Box::new(b))
            })
        }
        rule PathSequence_item() -> PropertyPathExpression = p:PathEltOrInverse() _ { p }

        rule PathElt() -> PropertyPathExpression = p:PathPrimary() _ o:PathElt_op()? {
            match o {
                Some('?') => PropertyPathExpression::ZeroOrOne(Box::new(p)),
                Some('*') => PropertyPathExpression::ZeroOrMore(Box::new(p)),
                Some('+') => PropertyPathExpression::OneOrMore(Box::new(p)),
                Some(_) => unreachable!(),
                None => p
            }
        }
        rule PathElt_op() -> char =
            "*" { '*' } /
            "+" { '+' } /
            "?" !(['0'..='9'] / PN_CHARS_U()) { '?' } // We mandate that this is not a variable

        rule PathEltOrInverse() -> PropertyPathExpression =
            "^" _ p:PathElt() { PropertyPathExpression::Reverse(Box::new(p)) } /
            PathElt()

        rule PathPrimary() -> PropertyPathExpression =
            v:iri() { v.into() } /
            "a" { rdf::TYPE.into_owned().into() } /
            "!" _ p:PathNegatedPropertySet() { p } /
            "(" _ p:Path() _ ")" { p }

        rule PathNegatedPropertySet() -> PropertyPathExpression =
            "(" _ p:PathNegatedPropertySet_item() **<1,> ("|" _) ")" {
                let mut direct = Vec::new();
                let mut inverse = Vec::new();
                for e in p {
                    match e {
                        Either::Left(a) => direct.push(a),
                        Either::Right(b) => inverse.push(b)
                    }
                }
                if inverse.is_empty() {
                    PropertyPathExpression::NegatedPropertySet(direct)
                } else if direct.is_empty() {
                   PropertyPathExpression::Reverse(Box::new(PropertyPathExpression::NegatedPropertySet(inverse)))
                } else {
                    PropertyPathExpression::Alternative(
                        Box::new(PropertyPathExpression::NegatedPropertySet(direct)),
                        Box::new(PropertyPathExpression::Reverse(Box::new(PropertyPathExpression::NegatedPropertySet(inverse))))
                    )
                }
            } /
            p:PathOneInPropertySet() {
                match p {
                    Either::Left(a) => PropertyPathExpression::NegatedPropertySet(vec![a]),
                    Either::Right(b) => PropertyPathExpression::Reverse(Box::new(PropertyPathExpression::NegatedPropertySet(vec![b]))),
                }
            }
        rule PathNegatedPropertySet_item() -> Either<NamedNode,NamedNode> = p:PathOneInPropertySet() _ { p }

        rule PathOneInPropertySet() -> Either<NamedNode,NamedNode> =
            "^" _ v:iri() { Either::Right(v) } /
            "^" _ "a" { Either::Right(rdf::TYPE.into()) } /
            v:iri() { Either::Left(v) } /
            "a" { Either::Left(rdf::TYPE.into()) }

        rule TriplesNode() -> FocusedTriplePattern<TermPattern> = Collection() / BlankNodePropertyList()

        rule BlankNodePropertyList() -> FocusedTriplePattern<TermPattern> = "[" _ po:PropertyListNotEmpty() _ "]" {?
            let mut patterns = po.patterns;
            let mut bnode = TermPattern::from(BlankNode::default());
            for (p, os) in po.focus {
                for o in os {
                    add_to_triple_patterns(bnode.clone(), p.clone(), o, &mut patterns)?;
                }
            }
            Ok(FocusedTriplePattern {
                focus: bnode,
                patterns
            })
        }

        rule TriplesNodePath() -> FocusedTripleOrPathPattern<TermPattern> = CollectionPath() / BlankNodePropertyListPath()

        rule BlankNodePropertyListPath() -> FocusedTripleOrPathPattern<TermPattern> = "[" _ po:PropertyListPathNotEmpty() _ "]" {?
            let mut patterns = po.patterns;
            let mut bnode = TermPattern::from(BlankNode::default());
            for (p, os) in po.focus {
                for o in os {
                    add_to_triple_or_path_patterns(bnode.clone(), p.clone(), o, &mut patterns)?;
                }
            }
            Ok(FocusedTripleOrPathPattern {
                focus: bnode,
                patterns
            })
        }

        rule Collection() -> FocusedTriplePattern<TermPattern> = "(" _ o:Collection_item()+ ")" {
            let mut patterns: Vec<TriplePattern> = Vec::new();
            let mut current_list_node = TermPattern::from(rdf::NIL.into_owned());
            for objWithPatterns in o.into_iter().rev() {
                let new_blank_node = TermPattern::from(BlankNode::default());
                patterns.push(TriplePattern::new(new_blank_node.clone(), rdf::FIRST.into_owned(), objWithPatterns.focus.clone()));
                patterns.push(TriplePattern::new(new_blank_node.clone(), rdf::REST.into_owned(), current_list_node));
                current_list_node = new_blank_node;
                patterns.extend_from_slice(&objWithPatterns.patterns);
            }
            FocusedTriplePattern {
                focus: current_list_node,
                patterns
            }
        }
        rule Collection_item() -> FocusedTriplePattern<TermPattern> = o:GraphNode() _ { o }

        rule CollectionPath() -> FocusedTripleOrPathPattern<TermPattern> = "(" _ o:CollectionPath_item()+ _ ")" {
            let mut patterns: Vec<TripleOrPathPattern> = Vec::new();
            let mut current_list_node = TermPattern::from(rdf::NIL.into_owned());
            for objWithPatterns in o.into_iter().rev() {
                let new_blank_node = TermPattern::from(BlankNode::default());
                patterns.push(TriplePattern::new(new_blank_node.clone(), rdf::FIRST.into_owned(), objWithPatterns.focus.clone()).into());
                patterns.push(TriplePattern::new(new_blank_node.clone(), rdf::REST.into_owned(), current_list_node).into());
                current_list_node = new_blank_node;
                patterns.extend(objWithPatterns.patterns);
            }
            FocusedTripleOrPathPattern {
                focus: current_list_node,
                patterns
            }
        }
        rule CollectionPath_item() -> FocusedTripleOrPathPattern<TermPattern> = p:GraphNodePath() _ { p }

        rule VarOrTerm() -> TermPattern =
            v:Var() { v.into() } /
            t:TripleTerm() {?
                #[cfg(feature = "sparql-12")]{Ok(t.into())}
                #[cfg(not(feature = "sparql-12"))]{Err("Triple terms are only available in SPARQL 1.2")}
            } /
            i:iri() { i.into() } /
            l:RDFLiteral() { l.into() } /
            l:NumericLiteral() { l.into() } /
            l:BooleanLiteral() { l.into() } /
            b:BlankNode() { b.into() } /
            NIL() { rdf::NIL.into_owned().into() }

        rule AnnotationPath() -> FocusedTripleOrPathPattern<Vec<TermPattern>> = a:AnnotationPath_e()* {
            let mut output: FocusedTripleOrPathPattern<Vec<TermPattern>> = FocusedTripleOrPathPattern::new(Vec::new());
            for a in a {
                output.focus.push(a.focus);
                output.patterns.extend(a.patterns);
            }
            output
        }
        rule AnnotationPath_e() -> FocusedTripleOrPathPattern<TermPattern> =
            r:Reifier() _ a:AnnotationBlockPath()? _ {?
                let mut output: FocusedTripleOrPathPattern<TermPattern> = FocusedTripleOrPathPattern::new(r);
                if let Some(annotations) = a {
                    for (p, os) in annotations.focus {
                        for o in os {
                            add_to_triple_or_path_patterns(output.focus.clone(), p.clone(), o, &mut output.patterns)?;
                        }
                    }
                    output.patterns.extend(annotations.patterns);
                }
                Ok(output)
            } /
            a:AnnotationBlockPath() _ {?
                let mut output: FocusedTripleOrPathPattern<TermPattern> = FocusedTripleOrPathPattern::new(BlankNode::default());
                for (p, os) in a.focus {
                    for o in os {
                        add_to_triple_or_path_patterns(output.focus.clone(), p.clone(), o, &mut output.patterns)?;
                    }
                }
                output.patterns.extend(a.patterns);
                Ok(output)
            }

        rule AnnotationBlockPath() -> FocusedTripleOrPathPattern<Vec<(VariableOrPropertyPath,Vec<ReifiedTerm>)>> = "{|" _ a:PropertyListPathNotEmpty() _ "|}" { a }

        rule Annotation() -> FocusedTriplePattern<Vec<TermPattern>> = a:Annotation_e()* {
            let mut output: FocusedTriplePattern<Vec<TermPattern>> = FocusedTriplePattern::new(Vec::new());
            for a in a {
                output.focus.push(a.focus);
                output.patterns.extend(a.patterns);
            }
            output
        }
        rule Annotation_e() -> FocusedTriplePattern<TermPattern> =
            r:Reifier() _ a:AnnotationBlock()? _ {?
                let mut output: FocusedTriplePattern<TermPattern> = FocusedTriplePattern::new(r);
                if let Some(annotations) = a {
                    for (p, os) in annotations.focus {
                        for o in os {
                            add_to_triple_patterns(output.focus.clone(), p.clone(), o, &mut output.patterns)?;
                        }
                    }
                    output.patterns.extend(annotations.patterns);
                }
                Ok(output)
            } /
            a:AnnotationBlock() _ {?
                let mut output: FocusedTriplePattern<TermPattern> = FocusedTriplePattern::new(BlankNode::default());
                for (p, os) in a.focus {
                    for o in os {
                        add_to_triple_patterns(output.focus.clone(), p.clone(), o, &mut output.patterns)?;
                    }
                }
                output.patterns.extend(a.patterns);
                Ok(output)
            }

        rule AnnotationBlock() -> FocusedTriplePattern<Vec<(NamedNodePattern,Vec<ReifiedTerm>)>> = "{|" _ a:PropertyListNotEmpty() _ "|}" { a }

        rule GraphNode() -> FocusedTriplePattern<TermPattern> =
            ReifiedTriple() /
            t:VarOrTerm() { FocusedTriplePattern::new(t) } /
            TriplesNode()

        rule GraphNodePath() -> FocusedTripleOrPathPattern<TermPattern> =
            t:ReifiedTriple() { t.into() } /
            t:VarOrTerm() { FocusedTripleOrPathPattern::new(t) } /
            TriplesNodePath()

        rule ReifiedTriple() -> FocusedTriplePattern<TermPattern> = "<<" _ s:ReifiedTripleSubject() _ p:Verb() _ o:ReifiedTripleObject() _ r:Reifier()? _ ">>" {?
            #[cfg(feature = "sparql-12")]
            {
                let r = r.unwrap_or_else(|| BlankNode::default().into());
                let mut output = FocusedTriplePattern::new(r.clone());
                output.patterns.push(TriplePattern {
                        subject: r,
                        predicate: rdf::REIFIES.into_owned().into(),
                        object: TriplePattern {
                            subject: s.focus,
                            predicate: p,
                            object: o.focus
                        }.into()
                    });
                output.patterns.extend(s.patterns);
                output.patterns.extend(o.patterns);
                Ok(output)
            }
            #[cfg(not(feature = "sparql-12"))]
            {
                Err("Reified triples are only available in SPARQL 1.2")
            }
        }

        rule ReifiedTripleSubject() -> FocusedTriplePattern<TermPattern> =
            v:Var() { FocusedTriplePattern::new(v) } /
            ReifiedTriple() /
            i:iri() { FocusedTriplePattern::new(i) } /
            l:RDFLiteral() { FocusedTriplePattern::new(l) } /
            l:NumericLiteral() { FocusedTriplePattern::new(l) } /
            l:BooleanLiteral() { FocusedTriplePattern::new(l) } /
            b:BlankNode() { FocusedTriplePattern::new(b) }

        rule ReifiedTripleObject() -> FocusedTriplePattern<TermPattern> =
            v:Var() { FocusedTriplePattern::new(v) } /
            t:TripleTerm() {?
                #[cfg(feature = "sparql-12")]{Ok(FocusedTriplePattern::new(t))}
                #[cfg(not(feature = "sparql-12"))]{Err("Triples terms are only available in SPARQL 1.2")}
            } /
            ReifiedTriple() /
            i:iri() { FocusedTriplePattern::new(i) } /
            l:RDFLiteral() { FocusedTriplePattern::new(l) } /
            l:NumericLiteral() { FocusedTriplePattern::new(l) } /
            l:BooleanLiteral() { FocusedTriplePattern::new(l) } /
            b:BlankNode() { FocusedTriplePattern::new(b) }

        rule TripleTerm() -> TriplePattern = "<<(" _ s:TripleTermSubject() _ p:Verb() _ o:TripleTermObject() _ ")>>" {
            TriplePattern {
                subject: s,
                predicate: p,
                object: o
            }
        }

        rule TripleTermSubject() -> TermPattern =
            v:Var() { v.into() } /
            i:iri() { i.into() } /
            l:RDFLiteral() { l.into() } /
            l:NumericLiteral() { l.into() } /
            l:BooleanLiteral() { l.into() } /
            b:BlankNode() { b.into() }

        rule TripleTermObject() -> TermPattern =
            v:Var() { v.into() } /
            t:TripleTerm() {?
                #[cfg(feature = "sparql-12")]{Ok(t.into())}
                #[cfg(not(feature = "sparql-12"))]{Err("Triples terms are only available in SPARQL 1.2")}
            } /
            i:iri() { i.into() } /
            l:RDFLiteral() { l.into() } /
            l:NumericLiteral() { l.into() } /
            l:BooleanLiteral() { l.into() } /
            b:BlankNode() { b.into() }

        rule TripleTermData() -> GroundTriple = "<<(" _ s:TripleTermDataSubject() _ p:TripleTermData_p() _ o:TripleTermDataObject() _ ")>>" {?
            Ok(GroundTriple {
                subject: if let GroundTerm::NamedNode(s) = s { s } else { return Err("Literals are not allowed in subject position of nested patterns") },
                predicate: p,
                object: o
            })
        }
        rule TripleTermData_p() -> NamedNode = i: iri() { i } / "a" { rdf::TYPE.into() }

        rule TripleTermDataSubject() -> GroundTerm =
            i:iri() { i.into() } /
            l:RDFLiteral() { l.into() } /
            l:NumericLiteral() { l.into() } /
            l:BooleanLiteral() { l.into() }

        rule TripleTermDataObject() -> GroundTerm =
            t:TripleTermData() {?
                #[cfg(feature = "sparql-12")]{Ok(t.into())}
                #[cfg(not(feature = "sparql-12"))]{Err("Triples terms are only available in SPARQL 1.2")}
            } /
            i:iri() { i.into() } /
            l:RDFLiteral() { l.into() } /
            l:NumericLiteral() { l.into() } /
            l:BooleanLiteral() { l.into() }

        rule VarOrIri() -> NamedNodePattern =
            v:Var() { v.into() } /
            i:iri() { i.into() }

        rule Var() -> Variable = name:(VAR1() / VAR2()) { Variable::new_unchecked(name) }

        rule Expression() -> Expression = e:ConditionalOrExpression() {e}

        rule ConditionalOrExpression() -> Expression = e:ConditionalOrExpression_item() **<1,> ("||" _) {?
            not_empty_fold(e.into_iter(), |a, b| Expression::Or(Box::new(a), Box::new(b)))
        }
        rule ConditionalOrExpression_item() -> Expression = e:ConditionalAndExpression() _ { e }

        rule ConditionalAndExpression() -> Expression = e:ConditionalAndExpression_item() **<1,> ("&&" _) {?
            not_empty_fold(e.into_iter(), |a, b| Expression::And(Box::new(a), Box::new(b)))
        }
        rule ConditionalAndExpression_item() -> Expression = e:ValueLogical() _ { e }

        rule ValueLogical() -> Expression = RelationalExpression()

        rule RelationalExpression() -> Expression = a:NumericExpression() _ o: RelationalExpression_inner()? { match o {
            Some(("=", Some(b), None)) => Expression::Equal(Box::new(a), Box::new(b)),
            Some(("!=", Some(b), None)) => Expression::Not(Box::new(Expression::Equal(Box::new(a), Box::new(b)))),
            Some((">", Some(b), None)) => Expression::Greater(Box::new(a), Box::new(b)),
            Some((">=", Some(b), None)) => Expression::GreaterOrEqual(Box::new(a), Box::new(b)),
            Some(("<", Some(b), None)) => Expression::Less(Box::new(a), Box::new(b)),
            Some(("<=", Some(b), None)) => Expression::LessOrEqual(Box::new(a), Box::new(b)),
            Some(("IN", None, Some(l))) => Expression::In(Box::new(a), l),
            Some(("NOT IN", None, Some(l))) => Expression::Not(Box::new(Expression::In(Box::new(a), l))),
            Some(_) => unreachable!(),
            None => a
        } }
        rule RelationalExpression_inner() -> (&'input str, Option<Expression>, Option<Vec<Expression>>) =
            s: $("="  / "!=" / ">=" / ">" / "<=" / "<") _ e:NumericExpression() { (s, Some(e), None) } /
            i("IN") _ l:ExpressionList() { ("IN", None, Some(l)) } /
            i("NOT") _ i("IN") _ l:ExpressionList() { ("NOT IN", None, Some(l)) }

        rule NumericExpression() -> Expression = AdditiveExpression()

        rule AdditiveExpression() -> Expression = a:MultiplicativeExpression() _ o:AdditiveExpression_inner()? { match o {
            Some(("+", b)) => Expression::Add(Box::new(a), Box::new(b)),
            Some(("-", b)) => Expression::Subtract(Box::new(a), Box::new(b)),
            Some(_) => unreachable!(),
            None => a,
        } }
        rule AdditiveExpression_inner() -> (&'input str, Expression) = s: $("+" / "-") _ e:AdditiveExpression() {
            (s, e)
        }

        rule MultiplicativeExpression() -> Expression = a:UnaryExpression() _ o: MultiplicativeExpression_inner()? { match o {
            Some(("*", b)) => Expression::Multiply(Box::new(a), Box::new(b)),
            Some(("/", b)) => Expression::Divide(Box::new(a), Box::new(b)),
            Some(_) => unreachable!(),
            None => a
        } }
        rule MultiplicativeExpression_inner() -> (&'input str, Expression) = s: $("*" / "/") _ e:MultiplicativeExpression() {
            (s, e)
        }

        rule UnaryExpression() -> Expression = s: $("!" / "+" / "-")? _ e:PrimaryExpression() { match s {
            Some("!") => Expression::Not(Box::new(e)),
            Some("+") => Expression::UnaryPlus(Box::new(e)),
            Some("-") => Expression::UnaryMinus(Box::new(e)),
            Some(_) => unreachable!(),
            None => e,
        } }

        rule PrimaryExpression() -> Expression =
            BrackettedExpression() /
            ExprTripleTerm() /
            iriOrFunction() /
            v:Var() { v.into() } /
            l:RDFLiteral() { l.into() } /
            l:NumericLiteral() { l.into() } /
            l:BooleanLiteral() { l.into() } /
            BuiltInCall()

        rule ExprTripleTerm() -> Expression = "<<(" _ s:ExprTripleTermSubject() _ p:Verb() _ o:ExprTripleTermObject() _ ")>>" {?
            #[cfg(feature = "sparql-12")]{Ok(Expression::FunctionCall(Function::Triple, vec![s, p.into(), o]))}
            #[cfg(not(feature = "sparql-12"))]{Err("Triple terms are only available in SPARQL 1.2")}
        }

        rule ExprTripleTermSubject() -> Expression =
            i:iri() { i.into() } /
            l:RDFLiteral() { l.into() } /
            l:NumericLiteral() { l.into() } /
            l:BooleanLiteral() { l.into() } /
            v:Var() { v.into() }

        rule ExprTripleTermObject() -> Expression =
            ExprTripleTerm() /
            i:iri() { i.into() } /
            l:RDFLiteral() { l.into() } /
            l:NumericLiteral() { l.into() } /
            l:BooleanLiteral() { l.into() } /
            v:Var() { v.into() }

        rule BrackettedExpression() -> Expression = "(" _ e:Expression() _ ")" { e }

        rule BuiltInCall() -> Expression =
            a:Aggregate() {? state.new_aggregation(a).map(Into::into) } /
            i("STR") _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Str, vec![e]) } /
            i("LANG") _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Lang, vec![e]) } /
            i("LANGMATCHES") _ "(" _ a:Expression() _ "," _ b:Expression() _ ")" { Expression::FunctionCall(Function::LangMatches, vec![a, b]) } /
            i("LANGDIR") "(" _ e:Expression() _ ")" {?
                #[cfg(feature = "sparql-12")]{Ok(Expression::FunctionCall(Function::LangDir, vec![e]))}
                #[cfg(not(feature = "sparql-12"))]{Err("The LANGDIR function is only available in SPARQL 1.2")}
            } /
            i("DATATYPE") _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Datatype, vec![e]) } /
            i("BOUND") _ "(" _ v:Var() _ ")" { Expression::Bound(v) } /
            (i("IRI") / i("URI")) _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Iri, vec![e]) } /
            i("BNODE") "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::BNode, vec![e]) } /
            i("BNODE") NIL() { Expression::FunctionCall(Function::BNode, vec![]) }  /
            i("RAND") _ NIL() { Expression::FunctionCall(Function::Rand, vec![]) } /
            i("ABS") _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Abs, vec![e]) } /
            i("CEIL") _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Ceil, vec![e]) } /
            i("FLOOR") _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Floor, vec![e]) } /
            i("ROUND") _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Round, vec![e]) } /
            i("CONCAT") e:ExpressionList() { Expression::FunctionCall(Function::Concat, e) } /
            SubstringExpression() /
            i("STRLEN") _ "(" _ e: Expression() _ ")" { Expression::FunctionCall(Function::StrLen, vec![e]) } /
            StrReplaceExpression() /
            i("UCASE") _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::UCase, vec![e]) } /
            i("LCASE") _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::LCase, vec![e]) } /
            i("ENCODE_FOR_URI") "(" _ e: Expression() _ ")" { Expression::FunctionCall(Function::EncodeForUri, vec![e]) } /
            i("CONTAINS") _ "(" _ a:Expression() _ "," _ b:Expression() _ ")" { Expression::FunctionCall(Function::Contains, vec![a, b]) } /
            i("STRSTARTS") _ "(" _ a:Expression() _ "," _ b:Expression() _ ")" { Expression::FunctionCall(Function::StrStarts, vec![a, b]) } /
            i("STRENDS") _ "(" _ a:Expression() _ "," _ b:Expression() _ ")" { Expression::FunctionCall(Function::StrEnds, vec![a, b]) } /
            i("STRBEFORE") _ "(" _ a:Expression() _ "," _ b:Expression() _ ")" { Expression::FunctionCall(Function::StrBefore, vec![a, b]) } /
            i("STRAFTER") _ "(" _ a:Expression() _ "," _ b:Expression() _ ")" { Expression::FunctionCall(Function::StrAfter, vec![a, b]) } /
            i("YEAR") _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Year, vec![e]) } /
            i("MONTH") _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Month, vec![e]) } /
            i("DAY") _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Day, vec![e]) } /
            i("HOURS") _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Hours, vec![e]) } /
            i("MINUTES") _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Minutes, vec![e]) } /
            i("SECONDS") _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Seconds, vec![e]) } /
            i("TIMEZONE") _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Timezone, vec![e]) } /
            i("TZ") _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Tz, vec![e]) } /
            i("NOW") _ NIL() { Expression::FunctionCall(Function::Now, vec![]) } /
            i("UUID") _ NIL() { Expression::FunctionCall(Function::Uuid, vec![]) }/
            i("STRUUID") _ NIL() { Expression::FunctionCall(Function::StrUuid, vec![]) } /
            i("MD5") "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Md5, vec![e]) } /
            i("SHA1") "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Sha1, vec![e]) } /
            i("SHA256") "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Sha256, vec![e]) } /
            i("SHA384") "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Sha384, vec![e]) } /
            i("SHA512") "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::Sha512, vec![e]) } /
            i("COALESCE") e:ExpressionList() { Expression::Coalesce(e) } /
            i("IF") _ "(" _ a:Expression() _ "," _ b:Expression() _ "," _ c:Expression() _ ")" { Expression::If(Box::new(a), Box::new(b), Box::new(c)) } /
            i("STRLANG") _ "(" _ a:Expression() _ "," _ b:Expression() _ ")" { Expression::FunctionCall(Function::StrLang, vec![a, b]) }  /
            i("STRLANGDIR") "(" _ a:Expression() _ "," _ b:Expression() _ "," _ c:Expression() _ ")" {?
                #[cfg(feature = "sparql-12")]{Ok(Expression::FunctionCall(Function::StrLangDir, vec![a, b, c]))}
                #[cfg(not(feature = "sparql-12"))]{Err("The STRLANGDIR function is only available in SPARQL 1.2")}
            } /
            i("STRDT") _ "(" _ a:Expression() _ "," _ b:Expression() _ ")" { Expression::FunctionCall(Function::StrDt, vec![a, b]) } /
            i("sameTerm") "(" _ a:Expression() _ "," _ b:Expression() _ ")" { Expression::SameTerm(Box::new(a), Box::new(b)) } /
            (i("isIRI") / i("isURI")) _ "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::IsIri, vec![e]) } /
            i("isBLANK") "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::IsBlank, vec![e]) } /
            i("isLITERAL") "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::IsLiteral, vec![e]) } /
            i("isNUMERIC") "(" _ e:Expression() _ ")" { Expression::FunctionCall(Function::IsNumeric, vec![e]) } /
            i("hasLang") "(" _ e:Expression() _ ")" {?
                #[cfg(feature = "sparql-12")]{Ok(Expression::FunctionCall(Function::HasLang, vec![e]))}
                #[cfg(not(feature = "sparql-12"))]{Err("The hasLang function is only available in SPARQL 1.2")}
            } /
            i("hasLangDir") "(" _ e:Expression() _ ")" {?
                #[cfg(feature = "sparql-12")]{Ok(Expression::FunctionCall(Function::HasLangDir, vec![e]))}
                #[cfg(not(feature = "sparql-12"))]{Err("The hasLangDir function is only available in SPARQL 1.2")}
            } /
            RegexExpression() /
            ExistsFunc() /
            NotExistsFunc() /
            i("TRIPLE") "(" _ s:Expression() _ "," _ p:Expression() "," _ o:Expression() ")" {?
                #[cfg(feature = "sparql-12")]{Ok(Expression::FunctionCall(Function::Triple, vec![s, p, o]))}
                #[cfg(not(feature = "sparql-12"))]{Err("The TRIPLE function is only available in SPARQL 1.2")}
            } /
            i("SUBJECT") "(" _ e:Expression() _ ")" {?
                #[cfg(feature = "sparql-12")]{Ok(Expression::FunctionCall(Function::Subject, vec![e]))}
                #[cfg(not(feature = "sparql-12"))]{Err("The SUBJECT function is only available in SPARQL 1.2")}
            } /
            i("PREDICATE") "(" _ e:Expression() _ ")" {?
                #[cfg(feature = "sparql-12")]{Ok(Expression::FunctionCall(Function::Predicate, vec![e]))}
                #[cfg(not(feature = "sparql-12"))]{Err("The PREDICATE function is only available in SPARQL 1.2")}
            } /
            i("OBJECT") "(" _ e:Expression() _ ")" {?
                #[cfg(feature = "sparql-12")]{Ok(Expression::FunctionCall(Function::Object, vec![e]))}
                #[cfg(not(feature = "sparql-12"))]{Err("The OBJECT function is only available in SPARQL 1.2")}
            } /
            i("isTriple") "(" _ e:Expression() _ ")" {?
                #[cfg(feature = "sparql-12")]{Ok(Expression::FunctionCall(Function::IsTriple, vec![e]))}
                #[cfg(not(feature = "sparql-12"))]{Err("The isTriple function is only available in SPARQL 1.2")}
            } /
            i("ADJUST") "("  _ a:Expression() _ "," _ b:Expression() _ ")" {?
                #[cfg(feature = "sep-0002")]{Ok(Expression::FunctionCall(Function::Adjust, vec![a, b]))}
                #[cfg(not(feature = "sep-0002"))]{Err("The ADJUST function is only available in SPARQL-dev SEP 0002")}
            }

        rule RegexExpression() -> Expression =
            i("REGEX") _ "(" _ a:Expression() _ "," _ b:Expression() _ "," _ c:Expression() _ ")" { Expression::FunctionCall(Function::Regex, vec![a, b, c]) } /
            i("REGEX") _ "(" _ a:Expression() _ "," _ b:Expression() _ ")" { Expression::FunctionCall(Function::Regex, vec![a, b]) }


        rule SubstringExpression() -> Expression =
            i("SUBSTR") _ "(" _ a:Expression() _ "," _ b:Expression() _ "," _ c:Expression() _ ")" { Expression::FunctionCall(Function::SubStr, vec![a, b, c]) } /
            i("SUBSTR") _ "(" _ a:Expression() _ "," _ b:Expression() _ ")" { Expression::FunctionCall(Function::SubStr, vec![a, b]) }


        rule StrReplaceExpression() -> Expression =
            i("REPLACE") _ "(" _ a:Expression() _ "," _ b:Expression() _ "," _ c:Expression() _ "," _ d:Expression() _ ")" { Expression::FunctionCall(Function::Replace, vec![a, b, c, d]) } /
            i("REPLACE") _ "(" _ a:Expression() _ "," _ b:Expression() _ "," _ c:Expression() _ ")" { Expression::FunctionCall(Function::Replace, vec![a, b, c]) }

        rule ExistsFunc() -> Expression = i("EXISTS") _ p:GroupGraphPattern() { Expression::Exists(Box::new(p)) }

        rule NotExistsFunc() -> Expression = i("NOT") _ i("EXISTS") _ p:GroupGraphPattern() { Expression::Not(Box::new(Expression::Exists(Box::new(p)))) }

        rule Aggregate() -> AggregateExpression =
            i("COUNT") _ "(" _ i("DISTINCT") _ "*" _ ")" { AggregateExpression::CountSolutions { distinct: true } } /
            i("COUNT") _ "(" _ i("DISTINCT") _ expr:Expression() _ ")" { AggregateExpression::FunctionCall { name: AggregateFunction::Count, expr, distinct: true } } /
            i("COUNT") _ "(" _ "*" _ ")" { AggregateExpression::CountSolutions { distinct: false } } /
            i("COUNT") _ "(" _ expr:Expression() _ ")" { AggregateExpression::FunctionCall { name: AggregateFunction::Count, expr, distinct: false } } /
            i("SUM") _ "(" _ i("DISTINCT") _ expr:Expression() _ ")" { AggregateExpression::FunctionCall { name: AggregateFunction::Sum, expr, distinct: true } } /
            i("SUM") _ "(" _ expr:Expression() _ ")" { AggregateExpression::FunctionCall { name: AggregateFunction::Sum, expr, distinct: false } } /
            i("MIN") _ "(" _ i("DISTINCT") _ expr:Expression() _ ")" { AggregateExpression::FunctionCall { name: AggregateFunction::Min, expr, distinct: true } } /
            i("MIN") _ "(" _ expr:Expression() _ ")" { AggregateExpression::FunctionCall { name: AggregateFunction::Min, expr, distinct: false } } /
            i("MAX") _ "(" _ i("DISTINCT") _ expr:Expression() _ ")" { AggregateExpression::FunctionCall { name: AggregateFunction::Max, expr, distinct: true } } /
            i("MAX") _ "(" _ expr:Expression() _ ")" { AggregateExpression::FunctionCall { name: AggregateFunction::Max, expr, distinct: false } } /
            i("AVG") _ "(" _ i("DISTINCT") _ expr:Expression() _ ")" { AggregateExpression::FunctionCall { name: AggregateFunction::Avg, expr, distinct: true } } /
            i("AVG") _ "(" _ expr:Expression() _ ")" { AggregateExpression::FunctionCall { name: AggregateFunction::Avg, expr, distinct: false } } /
            i("SAMPLE") _ "(" _ i("DISTINCT") _ expr:Expression() _ ")" { AggregateExpression::FunctionCall { name: AggregateFunction::Sample, expr, distinct: true } } /
            i("SAMPLE") _ "(" _ expr:Expression() _ ")" { AggregateExpression::FunctionCall { name: AggregateFunction::Sample, expr, distinct: false } } /
            i("GROUP_CONCAT") _ "(" _ i("DISTINCT") _ expr:Expression() _ ";" _ i("SEPARATOR") _ "=" _ s:String() _ ")" { AggregateExpression::FunctionCall { name: AggregateFunction::GroupConcat { separator: Some(s) }, expr, distinct: true } } /
            i("GROUP_CONCAT") _ "(" _ i("DISTINCT") _ expr:Expression() _ ")" { AggregateExpression::FunctionCall { name: AggregateFunction::GroupConcat { separator: None }, expr, distinct: true } } /
            i("GROUP_CONCAT") _ "(" _ expr:Expression() _ ";" _ i("SEPARATOR") _ "=" _ s:String() _ ")" { AggregateExpression::FunctionCall { name: AggregateFunction::GroupConcat { separator: Some(s) }, expr, distinct: false } } /
            i("GROUP_CONCAT") _ "(" _ expr:Expression() _ ")" { AggregateExpression::FunctionCall { name: AggregateFunction::GroupConcat { separator: None }, expr, distinct: false } } /
            name:iri() _ "(" _ i("DISTINCT") _ expr:Expression() _ ")" {?
                if state.custom_aggregate_functions.contains(&name) {
                    Ok(AggregateExpression::FunctionCall { name: AggregateFunction::Custom(name), expr, distinct: true })
                } else {
                    Err("This custom function is a regular function and not an aggregate function")
                }
            } /
            name:iri() _ "(" _ expr:Expression() _ ")" {?
                if state.custom_aggregate_functions.contains(&name) {
                    Ok(AggregateExpression::FunctionCall { name: AggregateFunction::Custom(name), expr, distinct: false })
                } else {
                    Err("This custom function is a regular function and not an aggregate function")
                }
            }

        rule iriOrFunction() -> Expression = i: iri() _ a: ArgList()? {?
            if let Some(a) = a {
                if state.custom_aggregate_functions.contains(&i) {
                    Err("This custom function is an aggregate function and not a regular function")
                } else {
                    Ok(Expression::FunctionCall(Function::Custom(i), a))
                }
            } else {
                Ok(i.into())
            }
        }

        rule RDFLiteral() -> Literal =
            value:String() _ "^^" _ datatype:iri() { Literal::new_typed_literal(value, datatype) } /
            value:String() _ language_and_direction:LANGDIR() {?
                let (language, direction) = language_and_direction;
                #[cfg(feature = "sparql-12")]
                if let Some(is_ltr) = direction {
                    return Ok(Literal::new_directional_language_tagged_literal_unchecked(value, language.into_inner(), if is_ltr { oxrdf::BaseDirection::Ltr } else { oxrdf::BaseDirection::Rtl }))
                }
                #[cfg(not(feature = "sparql-12"))]
                if direction.is_some() {
                    return Err("Literal base directions are only supported in SPARQL 1.2")
                }
                Ok(Literal::new_language_tagged_literal_unchecked(value, language.into_inner()))
            } /
            value:String() { Literal::new_simple_literal(value) }

        rule NumericLiteral() -> Literal  = NumericLiteralUnsigned() / NumericLiteralPositive() / NumericLiteralNegative()

        rule NumericLiteralUnsigned() -> Literal =
            d:$(DOUBLE()) { Literal::new_typed_literal(d, xsd::DOUBLE) } /
            d:$(DECIMAL()) { Literal::new_typed_literal(d, xsd::DECIMAL) } /
            i:$(INTEGER()) { Literal::new_typed_literal(i, xsd::INTEGER) }

        rule NumericLiteralPositive() -> Literal =
            d:$(DOUBLE_POSITIVE()) { Literal::new_typed_literal(d, xsd::DOUBLE) } /
            d:$(DECIMAL_POSITIVE()) { Literal::new_typed_literal(d, xsd::DECIMAL) } /
            i:$(INTEGER_POSITIVE()) { Literal::new_typed_literal(i, xsd::INTEGER) }


        rule NumericLiteralNegative() -> Literal =
            d:$(DOUBLE_NEGATIVE()) { Literal::new_typed_literal(d, xsd::DOUBLE) } /
            d:$(DECIMAL_NEGATIVE()) { Literal::new_typed_literal(d, xsd::DECIMAL) } /
            i:$(INTEGER_NEGATIVE()) { Literal::new_typed_literal(i, xsd::INTEGER) }

        rule BooleanLiteral() -> Literal =
            "true" { Literal::new_typed_literal("true", xsd::BOOLEAN) } /
            "false" { Literal::new_typed_literal("false", xsd::BOOLEAN) }

        rule String() -> String = STRING_LITERAL_LONG1() / STRING_LITERAL_LONG2() / STRING_LITERAL1() / STRING_LITERAL2()

        rule iri() -> NamedNode = i:(IRIREF() / PrefixedName()) {
            NamedNode::from(i)
        }

        rule PrefixedName() -> Iri<String> = PNAME_LN() /
            ns:PNAME_NS() {? if let Some(iri) = state.prefixes.get(ns).cloned() {
                Iri::parse(iri).map_err(|_| "prefix IRI parsing failed")
            } else {
                Err("Prefix not found")
            } }

        rule BlankNode() -> BlankNode = id:BLANK_NODE_LABEL() {?
            let node = BlankNode::new_unchecked(id);
            if state.used_bnodes.contains(&node) {
                Err("Already used blank node id")
            } else {
                state.currently_used_bnodes.insert(node.clone());
                Ok(node)
            }
        } / ANON() { BlankNode::default() }

        rule IRIREF() -> Iri<String> = "<" i:$((!['>'] [_])*) ">" {?
            state.parse_iri(unescape_iriref(i)?).map_err(|_| "IRI parsing failed")
        }

        rule PNAME_NS() -> &'input str = ns:$(PN_PREFIX()?) ":" {
            ns
        }

        rule PNAME_LN() -> Iri<String> = ns:PNAME_NS() local:$(PN_LOCAL()) {?
            if let Some(base) = state.prefixes.get(ns) {
                let mut iri = String::with_capacity(base.len() + local.len());
                iri.push_str(base);
                for chunk in local.split('\\') { // We remove \
                    iri.push_str(chunk);
                }
                Iri::parse(iri).map_err(|_| "IRI parsing failed")
            } else {
                Err("Prefix not found")
            }
        }

        rule BLANK_NODE_LABEL() -> &'input str = "_:" b:$((['0'..='9'] / PN_CHARS_U()) PN_CHARS()* ("."+ PN_CHARS()+)*) {
            b
        }

        rule VAR1() -> &'input str = "?" v:$(VARNAME()) { v }

        rule VAR2() -> &'input str = "$" v:$(VARNAME()) { v }

        rule LANGDIR() -> (LanguageTag<String>, Option<bool>) = "@" l:$(['a' ..= 'z' | 'A' ..= 'Z']+ ("-" ['a' ..= 'z' | 'A' ..= 'Z' | '0' ..= '9']+)*) d:$("--" ['a' ..= 'z' | 'A' ..= 'Z']+)? {?
            Ok((
                LanguageTag::parse(l.to_ascii_lowercase()).map_err(|_| "language tag parsing failed")?,
                d.map(|d| match d {
                    "--ltr" => Ok(true),
                    "--rtl" => Ok(false),
                    _ => Err("the only base directions allowed are 'rtl' and 'ltr'")
                }).transpose()?
            ))
        }

        rule INTEGER() = ['0'..='9']+

        rule DECIMAL() = ['0'..='9']* "." ['0'..='9']+

        rule DOUBLE() = (['0'..='9']+ "." ['0'..='9']* / "." ['0'..='9']+ / ['0'..='9']+) EXPONENT()

        rule INTEGER_POSITIVE() = "+" _ INTEGER()

        rule DECIMAL_POSITIVE() = "+" _ DECIMAL()

        rule DOUBLE_POSITIVE() = "+" _ DOUBLE()

        rule INTEGER_NEGATIVE() = "-" _ INTEGER()

        rule DECIMAL_NEGATIVE() = "-" _ DECIMAL()

        rule DOUBLE_NEGATIVE() = "-" _ DOUBLE()

        rule EXPONENT() = ['e' | 'E'] ['+' | '-']? ['0'..='9']+

        rule STRING_LITERAL1() -> String = "'" l:$((STRING_LITERAL1_simple_char() / ECHAR() / UCHAR())*) "'" {?
             unescape_string(l)
        }
        rule STRING_LITERAL1_simple_char() = !['\u{27}' | '\u{5C}' | '\u{0A}' | '\u{0D}'] [_]


        rule STRING_LITERAL2() -> String = "\"" l:$((STRING_LITERAL2_simple_char() / ECHAR() / UCHAR())*) "\"" {?
             unescape_string(l)
        }
        rule STRING_LITERAL2_simple_char() = !['\u{22}' | '\u{5C}' | '\u{0A}' | '\u{0D}'] [_]

        rule STRING_LITERAL_LONG1() -> String = "'''" l:$(STRING_LITERAL_LONG1_inner()*) "'''" {?
             unescape_string(l)
        }
        rule STRING_LITERAL_LONG1_inner() = ("''" / "'")? (STRING_LITERAL_LONG1_simple_char() / ECHAR() / UCHAR())
        rule STRING_LITERAL_LONG1_simple_char() = !['\'' | '\\'] [_]

        rule STRING_LITERAL_LONG2() -> String = "\"\"\"" l:$(STRING_LITERAL_LONG2_inner()*) "\"\"\"" {?
             unescape_string(l)
        }
        rule STRING_LITERAL_LONG2_inner() = ("\"\"" / "\"")? (STRING_LITERAL_LONG2_simple_char() / ECHAR() / UCHAR())
        rule STRING_LITERAL_LONG2_simple_char() = !['"' | '\\'] [_]

        rule UCHAR() = "\\u" HEX() HEX() HEX() HEX() / "\\U" HEX() HEX() HEX() HEX() HEX() HEX() HEX() HEX()

        rule ECHAR() = "\\" ['t' | 'b' | 'n' | 'r' | 'f' | '"' |'\'' | '\\']

        rule NIL() = "(" WS()* ")"

        rule WS() = quiet! { ['\u{20}' | '\u{09}' | '\u{0D}' | '\u{0A}'] }

        rule ANON() = "[" WS()* "]"

        rule PN_CHARS_BASE() = ['A' ..= 'Z' | 'a' ..= 'z' | '\u{00C0}'..='\u{00D6}' | '\u{00D8}'..='\u{00F6}' | '\u{00F8}'..='\u{02FF}' | '\u{0370}'..='\u{037D}' | '\u{037F}'..='\u{1FFF}' | '\u{200C}'..='\u{200D}' | '\u{2070}'..='\u{218F}' | '\u{2C00}'..='\u{2FEF}' | '\u{3001}'..='\u{D7FF}' | '\u{F900}'..='\u{FDCF}' | '\u{FDF0}'..='\u{FFFD}']

        rule PN_CHARS_U() = ['_'] / PN_CHARS_BASE()

        rule VARNAME() = (['0'..='9'] / PN_CHARS_U()) (['0' ..= '9' | '\u{00B7}' | '\u{0300}'..='\u{036F}' | '\u{203F}'..='\u{2040}'] / PN_CHARS_U())*

        rule PN_CHARS() = ['-' | '0' ..= '9' | '\u{00B7}' | '\u{0300}'..='\u{036F}' | '\u{203F}'..='\u{2040}'] / PN_CHARS_U()

        rule PN_PREFIX() = PN_CHARS_BASE() PN_CHARS()* ("."+ PN_CHARS()+)*

        rule PN_LOCAL() = (PN_CHARS_U() / [':' | '0'..='9'] / PLX()) (PN_CHARS() / [':'] / PLX())* (['.']+ (PN_CHARS() / [':'] / PLX())+)?

        rule PLX() = PERCENT() / PN_LOCAL_ESC()

        rule PERCENT() = ['%'] HEX() HEX()

        rule HEX() = ['0' ..= '9' | 'A' ..= 'F' | 'a' ..= 'f']

        rule PN_LOCAL_ESC() = ['\\'] ['_' | '~' | '.' | '-' | '!' | '$' | '&' | '\'' | '(' | ')' | '*' | '+' | ',' | ';' | '=' | '/' | '?' | '#' | '@' | '%'] //TODO: added '/' to make tests pass but is it valid?

        //space
        rule _() = quiet! { ([' ' | '\t' | '\n' | '\r'] / comment())* }

        //comment
        rule comment() = quiet! { ['#'] (!['\r' | '\n'] [_])* }

        rule i(literal: &'static str) = input: $([_]*<{literal.len()}>) {?
            if input.eq_ignore_ascii_case(literal) {
                Ok(())
            } else {
                Err(literal)
            }
        }
    }
}
