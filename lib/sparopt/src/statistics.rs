use oxrdf::{GraphName, NamedNode, Term};

/// A statistic value, that is either exact, an estimate, or unknown.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum Precision<T> {
    Exact(T),
    Inexact(T),
    Absent,
}

impl<T> Precision<T> {
    /// The value, exact or not.
    pub fn value(self) -> Option<T> {
        if let Self::Exact(value) | Self::Inexact(value) = self {
            Some(value)
        } else {
            None
        }
    }
}

/// Statistics about the queried dataset, used by [`Optimizer::optimize_query_expression_with`](crate::Optimizer::optimize_query_expression_with) to estimate cardinalities.
///
/// `graph_name` is the default graph or a named graph, or `None` for the union of all the named graphs.
/// The optimizer uses its default estimates where a method returns [`Precision::Absent`].
pub trait Statistics {
    /// The number of quads matching the pattern, `None` matching any term.
    fn quad_count(
        &self,
        _subject: Option<&Term>,
        _predicate: Option<&NamedNode>,
        _object: Option<&Term>,
        _graph_name: Option<&GraphName>,
    ) -> Precision<u64> {
        Precision::Absent
    }

    /// The number of distinct subjects of the triples with this predicate, `None` for any predicate.
    fn distinct_subject_count(
        &self,
        _predicate: Option<&NamedNode>,
        _graph_name: Option<&GraphName>,
    ) -> Precision<u64> {
        Precision::Absent
    }

    /// The number of distinct objects of the triples with this predicate, `None` for any predicate.
    fn distinct_object_count(
        &self,
        _predicate: Option<&NamedNode>,
        _graph_name: Option<&GraphName>,
    ) -> Precision<u64> {
        Precision::Absent
    }
}

pub(crate) struct NoStatistics;

impl Statistics for NoStatistics {}
