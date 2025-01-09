use crate::io::{RdfFormat, RdfSerializer};
use crate::model::*;
use crate::sparql::error::EvaluationError;
use crate::sparql::results::{
    QueryResultsFormat, QueryResultsParseError, QueryResultsParser, QueryResultsSerializer,
    ReaderQueryResultsParserOutput, ReaderSolutionsParser,
};
pub use sparesults::QuerySolution;
use spareval::{
    QueryEvaluationError, QueryResults as EvalQueryResults,
    QuerySolutionIter as EvalQuerySolutionIter, QueryTripleIter as EvalQueryTripleIter,
};
use std::io::{Read, Write};
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

impl QueryResults {
    /// Reads a SPARQL query results serialization.
    pub fn read(
        reader: impl Read + 'static,
        format: QueryResultsFormat,
    ) -> Result<Self, QueryResultsParseError> {
        Ok(QueryResultsParser::from_format(format)
            .for_reader(reader)?
            .into())
    }

    /// Writes the query results (solutions or boolean).
    ///
    /// This method fails if it is called on the `Graph` results.
    ///
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::results::QueryResultsFormat;
    ///
    /// let store = Store::new()?;
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// store.insert(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?;
    ///
    /// let results = store.query("SELECT ?s WHERE { ?s ?p ?o }")?;
    /// assert_eq!(
    ///     results.write(Vec::new(), QueryResultsFormat::Json)?,
    ///     r#"{"head":{"vars":["s"]},"results":{"bindings":[{"s":{"type":"uri","value":"http://example.com"}}]}}"#.as_bytes()
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn write<W: Write>(
        self,
        writer: W,
        format: QueryResultsFormat,
    ) -> Result<W, EvaluationError> {
        let serializer = QueryResultsSerializer::from_format(format);
        match self {
            Self::Boolean(value) => serializer.serialize_boolean_to_writer(writer, value),
            Self::Solutions(solutions) => {
                let mut serializer = serializer
                    .serialize_solutions_to_writer(writer, solutions.variables().to_vec())
                    .map_err(EvaluationError::ResultsSerialization)?;
                for solution in solutions {
                    serializer
                        .serialize(&solution?)
                        .map_err(EvaluationError::ResultsSerialization)?;
                }
                serializer.finish()
            }
            Self::Graph(triples) => {
                let s = VariableRef::new_unchecked("subject");
                let p = VariableRef::new_unchecked("predicate");
                let o = VariableRef::new_unchecked("object");
                let mut serializer = serializer
                    .serialize_solutions_to_writer(
                        writer,
                        vec![s.into_owned(), p.into_owned(), o.into_owned()],
                    )
                    .map_err(EvaluationError::ResultsSerialization)?;
                for triple in triples {
                    let triple = triple?;
                    serializer
                        .serialize([
                            (s, &triple.subject.into()),
                            (p, &triple.predicate.into()),
                            (o, &triple.object),
                        ])
                        .map_err(EvaluationError::ResultsSerialization)?;
                }
                serializer.finish()
            }
        }
        .map_err(EvaluationError::ResultsSerialization)
    }

    /// Writes the graph query results.
    ///
    /// This method fails if it is called on the `Solution` or `Boolean` results.
    ///
    /// ```
    /// use oxigraph::io::RdfFormat;
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
    ///
    /// let graph = "<http://example.com> <http://example.com> <http://example.com> .\n";
    ///
    /// let store = Store::new()?;
    /// store.load_graph(
    ///     graph.as_bytes(),
    ///     RdfFormat::NTriples,
    ///     GraphName::DefaultGraph,
    ///     None,
    /// )?;
    ///
    /// let results = store.query("CONSTRUCT WHERE { ?s ?p ?o }")?;
    /// assert_eq!(
    ///     results.write_graph(Vec::new(), RdfFormat::NTriples)?,
    ///     graph.as_bytes()
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn write_graph<W: Write>(
        self,
        writer: W,
        format: impl Into<RdfFormat>,
    ) -> Result<W, EvaluationError> {
        if let Self::Graph(triples) = self {
            let mut serializer = RdfSerializer::from_format(format.into()).for_writer(writer);
            for triple in triples {
                serializer
                    .serialize_triple(&triple?)
                    .map_err(EvaluationError::ResultsSerialization)?;
            }
            serializer
                .finish()
                .map_err(EvaluationError::ResultsSerialization)
        } else {
            Err(EvaluationError::NotAGraph)
        }
    }
}

impl From<EvalQueryResults> for QueryResults {
    #[inline]
    fn from(results: EvalQueryResults) -> Self {
        match results {
            EvalQueryResults::Solutions(iter) => Self::Solutions(iter.into()),
            EvalQueryResults::Boolean(value) => Self::Boolean(value),
            EvalQueryResults::Graph(iter) => Self::Graph(iter.into()),
        }
    }
}

impl From<QuerySolutionIter> for QueryResults {
    #[inline]
    fn from(value: QuerySolutionIter) -> Self {
        Self::Solutions(value)
    }
}

impl<R: Read + 'static> From<ReaderQueryResultsParserOutput<R>> for QueryResults {
    fn from(reader: ReaderQueryResultsParserOutput<R>) -> Self {
        match reader {
            ReaderQueryResultsParserOutput::Solutions(s) => Self::Solutions(s.into()),
            ReaderQueryResultsParserOutput::Boolean(v) => Self::Boolean(v),
        }
    }
}

/// An iterator over [`QuerySolution`]s.
///
/// ```
/// use oxigraph::sparql::QueryResults;
/// use oxigraph::store::Store;
///
/// let store = Store::new()?;
/// if let QueryResults::Solutions(solutions) = store.query("SELECT ?s WHERE { ?s ?p ?o }")? {
///     for solution in solutions {
///         println!("{:?}", solution?.get("s"));
///     }
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct QuerySolutionIter {
    inner: EvalQuerySolutionIter,
}

impl QuerySolutionIter {
    /// Construct a new iterator of solution from an ordered list of solution variables and an iterator of solution tuples
    /// (each tuple using the same ordering as the variable list such that tuple element 0 is the value for the variable 0...)
    pub fn new(
        variables: Arc<[Variable]>,
        iter: impl Iterator<Item = Result<Vec<Option<Term>>, EvaluationError>> + 'static,
    ) -> Self {
        Self {
            inner: EvalQuerySolutionIter::new(
                Arc::clone(&variables),
                Box::new(iter.map(move |t| match t {
                    Ok(values) => Ok((Arc::clone(&variables), values).into()),
                    Err(e) => Err(QueryEvaluationError::Service(Box::new(e))),
                })),
            ),
        }
    }

    /// The variables used in the solutions.
    ///
    /// ```
    /// use oxigraph::sparql::{QueryResults, Variable};
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    /// if let QueryResults::Solutions(solutions) = store.query("SELECT ?s ?o WHERE { ?s ?p ?o }")? {
    ///     assert_eq!(
    ///         solutions.variables(),
    ///         &[Variable::new("s")?, Variable::new("o")?]
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn variables(&self) -> &[Variable] {
        self.inner.variables()
    }
}

impl From<EvalQuerySolutionIter> for QuerySolutionIter {
    #[inline]
    fn from(inner: EvalQuerySolutionIter) -> Self {
        Self { inner }
    }
}

impl From<QuerySolutionIter> for EvalQuerySolutionIter {
    #[inline]
    fn from(iter: QuerySolutionIter) -> Self {
        iter.inner
    }
}

impl<R: Read + 'static> From<ReaderSolutionsParser<R>> for QuerySolutionIter {
    fn from(reader: ReaderSolutionsParser<R>) -> Self {
        Self {
            inner: EvalQuerySolutionIter::new(
                reader.variables().into(),
                Box::new(reader.map(|t| t.map_err(|e| QueryEvaluationError::Service(Box::new(e))))),
            ),
        }
    }
}

impl Iterator for QuerySolutionIter {
    type Item = Result<QuerySolution, EvaluationError>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        Some(self.inner.next()?.map_err(Into::into))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

/// An iterator over the triples that compose a graph solution.
///
/// ```
/// use oxigraph::sparql::QueryResults;
/// use oxigraph::store::Store;
///
/// let store = Store::new()?;
/// if let QueryResults::Graph(triples) = store.query("CONSTRUCT WHERE { ?s ?p ?o }")? {
///     for triple in triples {
///         println!("{}", triple?);
///     }
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct QueryTripleIter {
    inner: EvalQueryTripleIter,
}

impl From<EvalQueryTripleIter> for QueryTripleIter {
    #[inline]
    fn from(inner: EvalQueryTripleIter) -> Self {
        Self { inner }
    }
}

impl From<QueryTripleIter> for EvalQueryTripleIter {
    #[inline]
    fn from(iter: QueryTripleIter) -> Self {
        iter.inner
    }
}

impl Iterator for QueryTripleIter {
    type Item = Result<Triple, EvaluationError>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        Some(self.inner.next()?.map_err(Into::into))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

#[cfg(test)]
#[allow(clippy::panic_in_result_fn)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_send_sync() {
        fn is_send_sync<T: Send + Sync>() {}
        is_send_sync::<QuerySolution>();
    }

    #[test]
    fn test_serialization_roundtrip() -> Result<(), EvaluationError> {
        use std::str;

        for format in [
            QueryResultsFormat::Json,
            QueryResultsFormat::Xml,
            QueryResultsFormat::Tsv,
        ] {
            let results = vec![
                QueryResults::Boolean(true),
                QueryResults::Boolean(false),
                QueryResults::Solutions(QuerySolutionIter::new(
                    [
                        Variable::new_unchecked("foo"),
                        Variable::new_unchecked("bar"),
                    ]
                    .as_ref()
                    .into(),
                    Box::new(
                        vec![
                            Ok(vec![None, None]),
                            Ok(vec![
                                Some(NamedNode::new_unchecked("http://example.com").into()),
                                None,
                            ]),
                            Ok(vec![
                                None,
                                Some(NamedNode::new_unchecked("http://example.com").into()),
                            ]),
                            Ok(vec![
                                Some(BlankNode::new_unchecked("foo").into()),
                                Some(BlankNode::new_unchecked("bar").into()),
                            ]),
                            Ok(vec![Some(Literal::new_simple_literal("foo").into()), None]),
                            Ok(vec![
                                Some(
                                    Literal::new_language_tagged_literal_unchecked("foo", "fr")
                                        .into(),
                                ),
                                None,
                            ]),
                            Ok(vec![
                                Some(Literal::from(1).into()),
                                Some(Literal::from(true).into()),
                            ]),
                            Ok(vec![
                                Some(Literal::from(1.33).into()),
                                Some(Literal::from(false).into()),
                            ]),
                            Ok(vec![
                                Some(
                                    Triple::new(
                                        NamedNode::new_unchecked("http://example.com/s"),
                                        NamedNode::new_unchecked("http://example.com/p"),
                                        Triple::new(
                                            NamedNode::new_unchecked("http://example.com/os"),
                                            NamedNode::new_unchecked("http://example.com/op"),
                                            NamedNode::new_unchecked("http://example.com/oo"),
                                        ),
                                    )
                                    .into(),
                                ),
                                None,
                            ]),
                        ]
                        .into_iter(),
                    ),
                )),
            ];

            for ex in results {
                let mut buffer = Vec::new();
                ex.write(&mut buffer, format)?;
                let ex2 = QueryResults::read(Cursor::new(buffer.clone()), format)?;
                let mut buffer2 = Vec::new();
                ex2.write(&mut buffer2, format)?;
                assert_eq!(
                    str::from_utf8(&buffer).unwrap(),
                    str::from_utf8(&buffer2).unwrap()
                );
            }
        }

        Ok(())
    }
}
