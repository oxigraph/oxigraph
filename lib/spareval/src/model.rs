use crate::error::QueryEvaluationError;
use oxrdf::{Triple, Variable};
pub use sparesults::QuerySolution;
use std::sync::Arc;

/// Results of a [SPARQL query](https://www.w3.org/TR/sparql11-query/).
pub enum QueryResults {
    /// Results of a [SELECT](https://www.w3.org/TR/sparql11-query/#select) query.
    Solutions(QuerySolutionIter),
    /// Result of a [ASK](https://www.w3.org/TR/sparql11-query/#ask) query.
    Boolean(bool),
    /// Results of a [CONSTRUCT](https://www.w3.org/TR/sparql11-query/#construct) or [DESCRIBE](https://www.w3.org/TR/sparql11-query/#describe) query.
    Graph(QueryTripleIter),
}

impl From<QuerySolutionIter> for QueryResults {
    #[inline]
    fn from(value: QuerySolutionIter) -> Self {
        Self::Solutions(value)
    }
}

/// An iterator over [`QuerySolution`]s.
///
/// ```
/// use oxrdf::Dataset;
/// use spareval::{QueryEvaluator, QueryResults};
/// use spargebra::Query;
///
/// let query = Query::parse("SELECT ?s ?o WHERE { ?s ?p ?o }", None)?;
/// if let QueryResults::Solutions(solutions) =
///     QueryEvaluator::new().execute(Dataset::new(), &query)?
/// {
///     for solution in solutions {
///         println!("{:?}", solution?.get("s"));
///     }
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct QuerySolutionIter {
    variables: Arc<[Variable]>,
    iter: Box<dyn Iterator<Item = Result<QuerySolution, QueryEvaluationError>>>,
}

impl QuerySolutionIter {
    /// Construct a new iterator of solution from an ordered list of solution variables and an iterator of solution tuples
    /// (each tuple using the same ordering as the variable list such that tuple element 0 is the value for the variable 0...)
    pub fn new(
        variables: Arc<[Variable]>,
        iter: impl Iterator<Item = Result<QuerySolution, QueryEvaluationError>> + 'static,
    ) -> Self {
        Self {
            variables,
            iter: Box::new(iter),
        }
    }

    /// The variables used in the solutions.
    ///
    /// ```
    /// use oxrdf::{Dataset, Variable};
    /// use spareval::{QueryEvaluator, QueryResults};
    /// use spargebra::Query;
    ///
    /// let query = Query::parse("SELECT ?s ?o WHERE { ?s ?p ?o }", None)?;
    /// if let QueryResults::Solutions(solutions) =
    ///     QueryEvaluator::new().execute(Dataset::new(), &query)?
    /// {
    ///     assert_eq!(
    ///         solutions.variables(),
    ///         &[Variable::new("s")?, Variable::new("o")?]
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }
}

impl Iterator for QuerySolutionIter {
    type Item = Result<QuerySolution, QueryEvaluationError>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// An iterator over the triples that compose a graph solution.
///
/// ```
/// use oxrdf::Dataset;
/// use spareval::{QueryEvaluator, QueryResults};
/// use spargebra::Query;
///
/// let query = Query::parse("CONSTRUCT WHERE { ?s ?p ?o }", None)?;
/// if let QueryResults::Graph(triples) = QueryEvaluator::new().execute(Dataset::new(), &query)? {
///     for triple in triples {
///         println!("{}", triple?);
///     }
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct QueryTripleIter {
    iter: Box<dyn Iterator<Item = Result<Triple, QueryEvaluationError>>>,
}

impl QueryTripleIter {
    pub(crate) fn new(
        iter: impl Iterator<Item = Result<Triple, QueryEvaluationError>> + 'static,
    ) -> Self {
        Self {
            iter: Box::new(iter),
        }
    }
}

impl Iterator for QueryTripleIter {
    type Item = Result<Triple, QueryEvaluationError>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}
