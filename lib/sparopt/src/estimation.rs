//! Cardinality and cost estimation of query expressions.

use crate::algebra::{Expression, PropertyPathExpression, QueryExpression};
use crate::statistics::{GraphScope, Statistics};
use crate::type_inference::{VariableTypes, infer_query_expression_types};
use oxrdf::TermRef;
use oxrdf::vocab::rdf;
use spargebra::term::{NamedNodePattern, TermPattern};
use spargebra::vocab::sparql;

/// Cost of starting the evaluation of a pattern.
///
/// A for-loop join pays it for each row of its left side: it is the price of the index seek.
pub const SEEK_COST: f64 = 1.;

/// Selectivity of a filter we know nothing about.
const DEFAULT_SELECTIVITY: f64 = 0.5;
/// Selectivity of an equality, it is 1 / (number of distinct values).
const EQUALITY_SELECTIVITY: f64 = 0.1;
/// Selectivity of a string matching function like STRSTARTS or REGEX.
const STRING_MATCH_SELECTIVITY: f64 = 0.25;
/// Selectivity of a range comparison like <.
const RANGE_SELECTIVITY: f64 = 0.33;
/// Selectivity of !=.
const NOT_EQUAL_SELECTIVITY: f64 = 0.9;
/// The number of named graphs if we don't know it.
const DEFAULT_NAMED_GRAPH_COUNT: f64 = 100.;

/// The estimated cardinality and evaluation cost of a query expression, for one input row.
///
/// The cost unit is a row produced by an index scan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Estimate {
    pub cardinality: f64,
    pub cost: f64,
}

impl Estimate {
    pub fn new(cardinality: f64, cost: f64) -> Self {
        Self { cardinality, cost }
    }
}

/// Estimates query expression cardinalities and costs using [`Statistics`] and falling back to heuristics.
#[derive(Clone, Copy)]
pub struct Estimator<'a> {
    statistics: &'a dyn Statistics,
}

impl<'a> Estimator<'a> {
    pub fn new(statistics: &'a dyn Statistics) -> Self {
        Self { statistics }
    }

    /// Estimates the cardinality and cost of an expression evaluated with the variables bound in `input_types`.
    pub fn estimate(
        &self,
        query_expression: &QueryExpression,
        input_types: &VariableTypes,
    ) -> Estimate {
        match query_expression {
            QueryExpression::Values {
                variables,
                bindings,
            } => {
                let size = usize_to_f64(bindings.len());
                // Each bound variable makes the table act as an equality filter
                let selectivity = variables
                    .iter()
                    .filter(|v| !input_types.get(v).undef)
                    .fold(1., |selectivity, _| selectivity * EQUALITY_SELECTIVITY);
                Estimate::new(size * selectivity, size)
            }
            QueryExpression::QuadPattern {
                subject,
                predicate,
                object,
                graph_name,
            } => {
                let cardinality = self.quad_pattern_cardinality(
                    subject,
                    predicate,
                    object,
                    graph_name.as_ref(),
                    input_types,
                );
                Estimate::new(cardinality, cardinality)
            }
            QueryExpression::Path {
                subject,
                path,
                object,
            } => {
                let cardinality = self.path_cardinality(
                    Position::from_term_pattern(subject, input_types),
                    path,
                    Position::from_term_pattern(object, input_types),
                );
                Estimate::new(cardinality, cardinality)
            }
            QueryExpression::Graph { graph_name, inner } => {
                let inner = self.estimate(inner, input_types);
                if Position::from_named_node_pattern(graph_name, input_types).is_bound() {
                    inner
                } else {
                    let graph_count = self
                        .statistics
                        .named_graph_count()
                        .map_or(DEFAULT_NAMED_GRAPH_COUNT, count_to_f64);
                    Estimate::new(inner.cardinality * graph_count, inner.cost * graph_count)
                }
            }
            QueryExpression::Join { left, right, .. } => {
                let left_estimate = self.estimate(left, input_types);
                let left_types = infer_query_expression_types(left, input_types.clone());
                let right_estimate = self.estimate(right, input_types);
                Estimate::new(
                    left_estimate.cardinality * self.estimate(right, &left_types).cardinality,
                    left_estimate.cost
                        + right_estimate.cost
                        + left_estimate.cardinality
                        + right_estimate.cardinality,
                )
            }
            #[cfg(feature = "sep-0006")]
            QueryExpression::Lateral { left, right } => {
                let left_estimate = self.estimate(left, input_types);
                let left_types = infer_query_expression_types(left, input_types.clone());
                let right_estimate = self.estimate_lateral_right(right, &left_types);
                Estimate::new(
                    left_estimate.cardinality * right_estimate.cardinality,
                    left_estimate.cost
                        + left_estimate.cardinality * (SEEK_COST + right_estimate.cost),
                )
            }
            QueryExpression::LeftJoin {
                left,
                right,
                expression,
                ..
            } => {
                let left_estimate = self.estimate(left, input_types);
                let left_types = infer_query_expression_types(left, input_types.clone());
                let matched_cardinality =
                    self.estimate(right, &left_types).cardinality * filter_selectivity(expression);
                let right_estimate = self.estimate(right, input_types);
                Estimate::new(
                    left_estimate.cardinality * matched_cardinality.max(1.),
                    left_estimate.cost
                        + right_estimate.cost
                        + left_estimate.cardinality
                        + right_estimate.cardinality,
                )
            }
            QueryExpression::Filter { inner, expression } => {
                let inner = self.estimate(inner, input_types);
                Estimate::new(
                    inner.cardinality * filter_selectivity(expression),
                    inner.cost,
                )
            }
            QueryExpression::Union { inner } => inner
                .iter()
                .map(|inner| self.estimate(inner, input_types))
                .fold(Estimate::new(0., 0.), |a, b| {
                    Estimate::new(a.cardinality + b.cardinality, a.cost + b.cost)
                }),
            QueryExpression::Minus { left, right, .. } => {
                let left = self.estimate(left, input_types);
                let right = self.estimate(right, input_types);
                Estimate::new(
                    left.cardinality,
                    left.cost + right.cost + left.cardinality + right.cardinality,
                )
            }
            QueryExpression::Extend { inner, .. }
            | QueryExpression::Distinct { inner }
            | QueryExpression::Reduced { inner }
            | QueryExpression::Service { inner, .. } => self.estimate(inner, input_types),
            QueryExpression::OrderBy { inner, .. } => {
                let inner = self.estimate(inner, input_types);
                Estimate::new(inner.cardinality, inner.cost + inner.cardinality)
            }
            QueryExpression::Project { inner, variables } => {
                self.estimate(inner, &input_types.restricted_to(variables))
            }
            QueryExpression::Slice {
                inner,
                offset,
                limit,
            } => {
                let inner = self.estimate(inner, input_types);
                let mut cardinality = (inner.cardinality - count_to_f64(*offset)).max(0.);
                if let Some(limit) = limit {
                    cardinality = cardinality.min(count_to_f64(*limit));
                }
                Estimate::new(cardinality, inner.cost)
            }
            QueryExpression::Group {
                inner, variables, ..
            } => {
                let inner = self.estimate(inner, &input_types.restricted_to(variables));
                let cardinality = if variables.is_empty() {
                    1.
                } else if variables.iter().all(|v| !input_types.get(v).undef) {
                    // A single group
                    inner.cardinality.min(1.)
                } else {
                    (inner.cardinality / 10.).max(inner.cardinality.min(1.))
                };
                Estimate::new(cardinality, inner.cost + inner.cardinality)
            }
        }
    }

    /// Estimates the right side of a lateral join, taking care of the for-loop left join special case
    #[cfg(feature = "sep-0006")]
    fn estimate_lateral_right(
        &self,
        right: &QueryExpression,
        left_types: &VariableTypes,
    ) -> Estimate {
        if let QueryExpression::LeftJoin {
            left: nested_left,
            right: nested_right,
            expression,
            ..
        } = right
        {
            if nested_left.is_empty_singleton() {
                // We are in a ForLoopLeftJoin: no hash table, the left row is returned if there are no matches
                let right = self.estimate(nested_right, left_types);
                return Estimate::new(
                    (right.cardinality * filter_selectivity(expression)).max(1.),
                    right.cost,
                );
            }
        }
        self.estimate(right, left_types)
    }

    fn quad_pattern_cardinality(
        &self,
        subject: &TermPattern,
        predicate: &NamedNodePattern,
        object: &TermPattern,
        graph_name: Option<&NamedNodePattern>,
        input_types: &VariableTypes,
    ) -> f64 {
        let subject = Position::from_term_pattern(subject, input_types);
        let predicate = Position::from_named_node_pattern(predicate, input_types);
        let object = Position::from_term_pattern(object, input_types);
        let (graph, graph_bound) = match graph_name {
            None => (GraphScope::DefaultGraph, false),
            Some(NamedNodePattern::NamedNode(graph_name)) => {
                (GraphScope::NamedGraph(graph_name.as_ref()), false)
            }
            Some(NamedNodePattern::Variable(graph_name)) => (
                GraphScope::AllNamedGraphs,
                !input_types.get(graph_name).undef,
            ),
        };
        if let Some(cardinality) = self.statistics_cardinality(subject, predicate, object, graph) {
            return if graph_bound {
                // On average, a named graph contains this fraction of the matching quads
                cardinality / self.named_graph_count()
            } else {
                cardinality
            };
        }
        let cardinality = heuristic_triple_pattern_cardinality(
            subject,
            predicate,
            object,
            matches!(predicate, Position::Constant(TermRef::NamedNode(p)) if p == rdf::TYPE),
        );
        if graph_bound || matches!(graph, GraphScope::NamedGraph(_)) {
            // A single graph is only a part of the dataset
            cardinality / self.named_graph_count()
        } else {
            cardinality
        }
    }

    fn named_graph_count(&self) -> f64 {
        self.statistics
            .named_graph_count()
            .map_or(DEFAULT_NAMED_GRAPH_COUNT, |c| count_to_f64(c.max(1)))
    }

    /// Estimates a triple pattern cardinality from the statistics, `None` if they are not enough
    fn statistics_cardinality(
        &self,
        subject: Position<'_>,
        predicate: Position<'_>,
        object: Position<'_>,
        graph: GraphScope<'_>,
    ) -> Option<f64> {
        let predicate = match predicate {
            Position::Constant(TermRef::NamedNode(predicate)) => Some(predicate),
            Position::Constant(_) => return Some(0.), // Never matches
            Position::Bound => {
                // The average predicate
                if subject.is_variable_bound() || object.is_variable_bound() {
                    return None;
                }
                return Some(
                    count_to_f64(self.statistics.quad_count(
                        subject.constant(),
                        None,
                        object.constant(),
                        graph,
                    )?) / count_to_f64(self.statistics.distinct_predicate_count(graph)?.max(1)),
                );
            }
            Position::Free => None,
        };
        if !subject.is_variable_bound() && !object.is_variable_bound() {
            // Only constants: we can count
            return self
                .statistics
                .quad_count(subject.constant(), predicate, object.constant(), graph)
                .map(count_to_f64);
        }
        // Some variables are bound, we use the average number of triples per value
        let statistics = self.statistics.predicate_statistics(predicate?, graph)?;
        let triples = count_to_f64(statistics.triples);
        let subject_count = count_to_f64(statistics.distinct_subjects.max(1));
        let object_count = count_to_f64(statistics.distinct_objects.max(1));
        Some(match (subject, object) {
            (Position::Bound, Position::Bound) => triples / subject_count / object_count,
            (Position::Bound, Position::Free) => triples / subject_count,
            (Position::Free, Position::Bound) => triples / object_count,
            (Position::Bound, Position::Constant(object)) => {
                count_to_f64(
                    self.statistics
                        .quad_count(None, predicate, Some(object), graph)?,
                ) / subject_count
            }
            (Position::Constant(subject), Position::Bound) => {
                count_to_f64(
                    self.statistics
                        .quad_count(Some(subject), predicate, None, graph)?,
                ) / object_count
            }
            _ => return None,
        })
    }

    fn path_cardinality(
        &self,
        start: Position<'_>,
        path: &PropertyPathExpression,
        end: Position<'_>,
    ) -> f64 {
        match path {
            PropertyPathExpression::Link(predicate) => {
                let predicate = Position::Constant(predicate.as_ref().into());
                self.statistics_cardinality(start, predicate, end, GraphScope::DefaultGraph)
                    .unwrap_or_else(|| {
                        heuristic_triple_pattern_cardinality(
                            start,
                            predicate,
                            end,
                            matches!(predicate, Position::Constant(TermRef::NamedNode(p)) if p == rdf::TYPE),
                        )
                    })
            }
            PropertyPathExpression::Inv(p) => self.path_cardinality(end, p, start),
            PropertyPathExpression::Seq(a, b) => {
                // We do a for loop join in the best direction
                (self.path_cardinality(start, a, Position::Free)
                    * self.path_cardinality(Position::Bound, b, end))
                .min(
                    self.path_cardinality(start, a, Position::Bound)
                        * self.path_cardinality(Position::Free, b, end),
                )
            }
            PropertyPathExpression::Alt(a, b) => {
                self.path_cardinality(start, a, end) + self.path_cardinality(start, b, end)
            }
            PropertyPathExpression::ZeroOrMorePath(p) => {
                if start.is_bound() && end.is_bound() {
                    1.
                } else if start.is_bound() || end.is_bound() {
                    self.path_cardinality(start, p, end) * 1000.
                } else {
                    1_000_000_000.
                }
            }
            PropertyPathExpression::OneOrMorePath(p) => {
                if start.is_bound() && end.is_bound() {
                    1.
                } else {
                    self.path_cardinality(start, p, end) * 1000.
                }
            }
            PropertyPathExpression::ZeroOrOnePath(p) => {
                if start.is_bound() && end.is_bound() {
                    1.
                } else if start.is_bound() || end.is_bound() {
                    self.path_cardinality(start, p, end)
                } else {
                    1_000_000_000.
                }
            }
            PropertyPathExpression::Nps(_) => {
                heuristic_triple_pattern_cardinality(start, Position::Free, end, false)
            }
        }
    }
}

/// The probability for a row to pass a filter
pub fn filter_selectivity(expression: &Expression) -> f64 {
    match expression {
        Expression::And(inner) => inner.iter().map(filter_selectivity).product(),
        Expression::Or(inner) => {
            1. - inner
                .iter()
                .map(|e| 1. - filter_selectivity(e))
                .product::<f64>()
        }
        Expression::FunctionCall(name, args) => {
            if *name == sparql::LOGICAL_NOT {
                if let [arg] = args.as_slice() {
                    return 1. - filter_selectivity(arg);
                }
                DEFAULT_SELECTIVITY
            } else if *name == sparql::SAME_TERM || *name == sparql::EQUALS {
                if args.iter().any(
                    |arg| matches!(arg, Expression::FunctionCall(name, _) if *name == sparql::LANG),
                ) {
                    STRING_MATCH_SELECTIVITY
                } else {
                    EQUALITY_SELECTIVITY
                }
            } else if *name == sparql::NOT_EQUALS {
                NOT_EQUAL_SELECTIVITY
            } else if [
                sparql::GREATER_THAN,
                sparql::GREATER_THAN_OR_EQUAL,
                sparql::LESS_THAN,
                sparql::LESS_THAN_OR_EQUAL,
            ]
            .contains(name)
            {
                RANGE_SELECTIVITY
            } else if [
                sparql::STRSTARTS,
                sparql::STRENDS,
                sparql::CONTAINS,
                sparql::REGEX,
                sparql::LANG_MATCHES,
            ]
            .contains(name)
            {
                STRING_MATCH_SELECTIVITY
            } else {
                DEFAULT_SELECTIVITY
            }
        }
        Expression::Literal(_) => match expression.effective_boolean_value() {
            Some(true) => 1.,
            Some(false) => 0.,
            None => DEFAULT_SELECTIVITY,
        },
        Expression::NamedNode(_)
        | Expression::Variable(_)
        | Expression::Exists(_)
        | Expression::Bound(_)
        | Expression::If(_, _, _)
        | Expression::Coalesce(_) => DEFAULT_SELECTIVITY,
    }
}

/// The state of a pattern position at evaluation time.
#[derive(Clone, Copy)]
enum Position<'a> {
    /// A constant term
    Constant(TermRef<'a>),
    /// A variable bound before the pattern is evaluated
    Bound,
    /// A variable not bound yet
    Free,
}

impl<'a> Position<'a> {
    fn from_term_pattern(pattern: &'a TermPattern, input_types: &VariableTypes) -> Self {
        match pattern {
            TermPattern::NamedNode(node) => Self::Constant(node.as_ref().into()),
            TermPattern::Literal(literal) => Self::Constant(literal.as_ref().into()),
            TermPattern::Variable(v) => {
                if input_types.get(v).undef {
                    Self::Free
                } else {
                    Self::Bound
                }
            }
            #[cfg(feature = "sparql-12")]
            TermPattern::Triple(t) => {
                if Self::from_term_pattern(&t.subject, input_types).is_bound()
                    && Self::from_named_node_pattern(&t.predicate, input_types).is_bound()
                    && Self::from_term_pattern(&t.object, input_types).is_bound()
                {
                    Self::Bound
                } else {
                    Self::Free
                }
            }
        }
    }

    fn from_named_node_pattern(pattern: &'a NamedNodePattern, input_types: &VariableTypes) -> Self {
        match pattern {
            NamedNodePattern::NamedNode(node) => Self::Constant(node.as_ref().into()),
            NamedNodePattern::Variable(v) => {
                if input_types.get(v).undef {
                    Self::Free
                } else {
                    Self::Bound
                }
            }
        }
    }

    fn is_bound(self) -> bool {
        !matches!(self, Self::Free)
    }

    fn is_variable_bound(self) -> bool {
        matches!(self, Self::Bound)
    }

    fn constant(self) -> Option<TermRef<'a>> {
        if let Self::Constant(term) = self {
            Some(term)
        } else {
            None
        }
    }
}

/// Fixed cardinalities used when we don't have statistics.
///
/// They assume that attributes are functional (`s p ?` is 1) so that long chains of attributes do not look expensive,
/// that a fully bound pattern is a filter,
/// that constant objects are classes or enumeration values that are shared by many subjects (`? p o` is large)
/// and that a variable bound in object position usually comes from a "foreign key" (`? p ?o` with ?o bound is small).
fn heuristic_triple_pattern_cardinality(
    subject: Position<'_>,
    predicate: Position<'_>,
    object: Position<'_>,
    is_rdf_type: bool,
) -> f64 {
    let cardinality = match (subject.is_bound(), predicate.is_bound(), object.is_bound()) {
        (true, true, true) => 0.5,
        (true, true, false) | (true, false, true) => 1.,
        (false, true, true) => {
            if object.is_variable_bound() && !is_rdf_type {
                10.
            } else {
                1_000.
            }
        }
        (true, false, false) => 20.,
        (false, false, false) => 1_000_000_000.,
        (false, true, false) => 1_000_000.,
        (false, false, true) => 10_000.,
    };
    if is_rdf_type && !(subject.is_bound() && object.is_bound()) {
        // Type patterns are often less selective than the other patterns
        cardinality + 1.
    } else {
        cardinality
    }
}

#[expect(clippy::cast_precision_loss)]
fn count_to_f64(count: u64) -> f64 {
    count as f64
}

fn usize_to_f64(count: usize) -> f64 {
    count_to_f64(count.try_into().unwrap_or(u64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::statistics::NoStatistics;

    fn estimate_slice_size(offset: u64, limit: Option<u64>) -> f64 {
        Estimator::new(&NoStatistics)
            .estimate(
                &QueryExpression::slice(
                    QueryExpression::values(Vec::new(), vec![Vec::new(); 5]),
                    offset,
                    limit,
                ),
                &VariableTypes::default(),
            )
            .cardinality
    }

    #[test]
    fn slice_size_applies_offset_before_limit() {
        assert!((estimate_slice_size(2, Some(1)) - 1.).abs() < f64::EPSILON);
        assert!(estimate_slice_size(6, Some(1)).abs() < f64::EPSILON);
        assert!((estimate_slice_size(2, None) - 3.).abs() < f64::EPSILON);
    }
}
