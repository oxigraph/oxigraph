//! Statistics about the queried data, used by the [`Optimizer`](crate::Optimizer) to estimate cardinalities.

use oxrdf::{NamedNodeRef, TermRef};

/// Statistics about a dataset used by [`Optimizer::optimize_query_expression_with`](crate::Optimizer::optimize_query_expression_with)
/// to estimate the cardinality of query patterns.
///
/// All methods return `None` when the answer is not known, the optimizer then falls back to heuristics.
/// Returned values are allowed to be approximate.
pub trait Statistics {
    /// Number of quads in `graph` matching the given pattern, `None` matching any term.
    ///
    /// Implementations are expected to be fast: they might count up to a cap and return the cap if it is reached.
    fn quad_count(
        &self,
        _subject: Option<TermRef<'_>>,
        _predicate: Option<NamedNodeRef<'_>>,
        _object: Option<TermRef<'_>>,
        _graph: GraphScope<'_>,
    ) -> Option<u64> {
        None
    }

    /// Summary statistics about the triples using `predicate` in `graph`.
    fn predicate_statistics(
        &self,
        _predicate: NamedNodeRef<'_>,
        _graph: GraphScope<'_>,
    ) -> Option<PredicateStatistics> {
        None
    }

    /// Number of distinct predicates in `graph`.
    fn distinct_predicate_count(&self, _graph: GraphScope<'_>) -> Option<u64> {
        None
    }

    /// Number of named graphs in the dataset.
    fn named_graph_count(&self) -> Option<u64> {
        None
    }
}

/// Statistics that know nothing, the optimizer only uses its heuristics.
#[derive(Default, Clone, Copy, Debug)]
pub struct NoStatistics;

impl Statistics for NoStatistics {}

/// The graphs a pattern is evaluated against.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum GraphScope<'a> {
    /// The default graph of the query.
    DefaultGraph,
    /// A given named graph.
    NamedGraph(NamedNodeRef<'a>),
    /// Any of the named graphs (e.g. `GRAPH ?g { ... }`).
    AllNamedGraphs,
}

/// Summary of the triples using a given predicate (c.f. [`Statistics::predicate_statistics`]).
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct PredicateStatistics {
    /// Number of triples with this predicate.
    pub triples: u64,
    /// Number of distinct subjects of the triples with this predicate.
    pub distinct_subjects: u64,
    /// Number of distinct objects of the triples with this predicate.
    pub distinct_objects: u64,
}
