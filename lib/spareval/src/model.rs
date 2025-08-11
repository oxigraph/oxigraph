use crate::error::QueryEvaluationError;
use oxrdf::{Term, Triple, Variable};
pub use sparesults::QuerySolution;
use sparesults::{
    ReaderQueryResultsParserOutput, ReaderSolutionsParser, SliceQueryResultsParserOutput,
    SliceSolutionsParser,
};
use std::io::Read;
use std::sync::Arc;

/// Results of a [SPARQL query](https://www.w3.org/TR/sparql11-query/).
pub enum QueryResults<'a> {
    /// Results of a [SELECT](https://www.w3.org/TR/sparql11-query/#select) query.
    Solutions(QuerySolutionIter<'a>),
    /// Result of a [ASK](https://www.w3.org/TR/sparql11-query/#ask) query.
    Boolean(bool),
    /// Results of a [CONSTRUCT](https://www.w3.org/TR/sparql11-query/#construct) or [DESCRIBE](https://www.w3.org/TR/sparql11-query/#describe) query.
    Graph(QueryTripleIter<'a>),
}

impl<'a> From<QuerySolutionIter<'a>> for QueryResults<'a> {
    #[inline]
    fn from(value: QuerySolutionIter<'a>) -> Self {
        Self::Solutions(value)
    }
}

impl From<bool> for QueryResults<'_> {
    #[inline]
    fn from(value: bool) -> Self {
        Self::Boolean(value)
    }
}

impl<'a> From<QueryTripleIter<'a>> for QueryResults<'a> {
    #[inline]
    fn from(value: QueryTripleIter<'a>) -> Self {
        Self::Graph(value)
    }
}

impl<'a, R: Read + 'a> From<ReaderQueryResultsParserOutput<R>> for QueryResults<'a> {
    #[inline]
    fn from(output: ReaderQueryResultsParserOutput<R>) -> Self {
        match output {
            ReaderQueryResultsParserOutput::Solutions(output) => Self::Solutions(output.into()),
            ReaderQueryResultsParserOutput::Boolean(output) => Self::Boolean(output),
        }
    }
}

impl<'a> From<SliceQueryResultsParserOutput<'a>> for QueryResults<'a> {
    #[inline]
    fn from(output: SliceQueryResultsParserOutput<'a>) -> Self {
        match output {
            SliceQueryResultsParserOutput::Solutions(output) => Self::Solutions(output.into()),
            SliceQueryResultsParserOutput::Boolean(output) => Self::Boolean(output),
        }
    }
}

/// An iterator over [`QuerySolution`]s.
///
/// ```
/// use oxrdf::Dataset;
/// use spareval::{QueryEvaluator, QueryResults};
/// use spargebra::SparqlParser;
///
/// let query = SparqlParser::new().parse_query("SELECT ?s ?o WHERE { ?s ?p ?o }")?;
/// if let QueryResults::Solutions(solutions) =
///     QueryEvaluator::new().execute(&Dataset::new(), &query)?
/// {
///     for solution in solutions {
///         println!("{:?}", solution?.get("s"));
///     }
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct QuerySolutionIter<'a> {
    variables: Arc<[Variable]>,
    iter: Box<dyn Iterator<Item = Result<QuerySolution, QueryEvaluationError>> + 'a>,
}

impl<'a> QuerySolutionIter<'a> {
    /// Construct a new iterator of solutions from an ordered list of solution variables and an iterator of solutions
    pub fn new(
        variables: Arc<[Variable]>,
        iter: impl IntoIterator<Item = Result<QuerySolution, QueryEvaluationError>> + 'a,
    ) -> Self {
        Self {
            variables,
            iter: Box::new(iter.into_iter()),
        }
    }

    /// Construct a new iterator of solutions from an ordered list of solution variables and an iterator of solution tuples
    /// (each tuple using the same ordering as the variable list such that tuple element 0 is the value for the variable 0...)
    pub fn from_tuples(
        variables: Arc<[Variable]>,
        iter: impl IntoIterator<Item = Result<Vec<Option<Term>>, QueryEvaluationError>> + 'a,
    ) -> Self {
        Self::new(
            Arc::clone(&variables),
            iter.into_iter()
                .map(move |values| Ok((Arc::clone(&variables), values?).into())),
        )
    }

    /// The variables used in the solutions.
    ///
    /// ```
    /// use oxrdf::{Dataset, Variable};
    /// use spareval::{QueryEvaluator, QueryResults};
    /// use spargebra::SparqlParser;
    ///
    /// let query = SparqlParser::new().parse_query("SELECT ?s ?o WHERE { ?s ?p ?o }")?;
    /// if let QueryResults::Solutions(solutions) =
    ///     QueryEvaluator::new().execute(&Dataset::new(), &query)?
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

impl Iterator for QuerySolutionIter<'_> {
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

impl<'a, R: Read + 'a> From<ReaderSolutionsParser<R>> for QuerySolutionIter<'a> {
    #[inline]
    fn from(parser: ReaderSolutionsParser<R>) -> Self {
        Self {
            variables: parser.variables().into(),
            iter: Box::new(
                parser.map(|r| r.map_err(|e| QueryEvaluationError::Unexpected(e.into()))),
            ),
        }
    }
}

impl<'a> From<SliceSolutionsParser<'a>> for QuerySolutionIter<'a> {
    #[inline]
    fn from(parser: SliceSolutionsParser<'a>) -> Self {
        Self {
            variables: parser.variables().into(),
            iter: Box::new(
                parser.map(|r| r.map_err(|e| QueryEvaluationError::Unexpected(e.into()))),
            ),
        }
    }
}

/// An iterator over the triples that compose a graph solution.
///
/// ```
/// use oxrdf::Dataset;
/// use spareval::{QueryEvaluator, QueryResults};
/// use spargebra::SparqlParser;
///
/// let query = SparqlParser::new().parse_query("CONSTRUCT WHERE { ?s ?p ?o }")?;
/// if let QueryResults::Graph(triples) = QueryEvaluator::new().execute(&Dataset::new(), &query)? {
///     for triple in triples {
///         println!("{}", triple?);
///     }
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct QueryTripleIter<'a> {
    iter: Box<dyn Iterator<Item = Result<Triple, QueryEvaluationError>> + 'a>,
}

impl<'a> QueryTripleIter<'a> {
    pub(crate) fn new(
        iter: impl Iterator<Item = Result<Triple, QueryEvaluationError>> + 'a,
    ) -> Self {
        Self {
            iter: Box::new(iter),
        }
    }
}

impl Iterator for QueryTripleIter<'_> {
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
