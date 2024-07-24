use crate::io::{RdfFormat, RdfSerializer};
use crate::model::*;
use crate::sparql::error::EvaluationError;
use crate::sparql::results::{
    FromReadQueryResultsReader, FromReadSolutionsReader, QueryResultsFormat,
    QueryResultsParseError, QueryResultsParser, QueryResultsSerializer,
};
pub use sparesults::QuerySolution;
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
        read: impl Read + 'static,
        format: QueryResultsFormat,
    ) -> Result<Self, QueryResultsParseError> {
        Ok(QueryResultsParser::from_format(format)
            .parse_read(read)?
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
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn write<W: Write>(
        self,
        write: W,
        format: QueryResultsFormat,
    ) -> Result<W, EvaluationError> {
        let serializer = QueryResultsSerializer::from_format(format);
        match self {
            Self::Boolean(value) => serializer.serialize_boolean_to_write(write, value),
            Self::Solutions(solutions) => {
                let mut writer = serializer
                    .serialize_solutions_to_write(write, solutions.variables().to_vec())
                    .map_err(EvaluationError::ResultsSerialization)?;
                for solution in solutions {
                    writer
                        .write(&solution?)
                        .map_err(EvaluationError::ResultsSerialization)?;
                }
                writer.finish()
            }
            Self::Graph(triples) => {
                let s = VariableRef::new_unchecked("subject");
                let p = VariableRef::new_unchecked("predicate");
                let o = VariableRef::new_unchecked("object");
                let mut writer = serializer
                    .serialize_solutions_to_write(
                        write,
                        vec![s.into_owned(), p.into_owned(), o.into_owned()],
                    )
                    .map_err(EvaluationError::ResultsSerialization)?;
                for triple in triples {
                    let triple = triple?;
                    writer
                        .write([
                            (s, &triple.subject.into()),
                            (p, &triple.predicate.into()),
                            (o, &triple.object),
                        ])
                        .map_err(EvaluationError::ResultsSerialization)?;
                }
                writer.finish()
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
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn write_graph<W: Write>(
        self,
        write: W,
        format: impl Into<RdfFormat>,
    ) -> Result<W, EvaluationError> {
        if let Self::Graph(triples) = self {
            let mut writer = RdfSerializer::from_format(format.into()).serialize_to_write(write);
            for triple in triples {
                writer
                    .write_triple(&triple?)
                    .map_err(EvaluationError::ResultsSerialization)?;
            }
            writer
                .finish()
                .map_err(EvaluationError::ResultsSerialization)
        } else {
            Err(EvaluationError::NotAGraph)
        }
    }
}

impl From<QuerySolutionIter> for QueryResults {
    #[inline]
    fn from(value: QuerySolutionIter) -> Self {
        Self::Solutions(value)
    }
}

impl<R: Read + 'static> From<FromReadQueryResultsReader<R>> for QueryResults {
    fn from(reader: FromReadQueryResultsReader<R>) -> Self {
        match reader {
            FromReadQueryResultsReader::Solutions(s) => Self::Solutions(s.into()),
            FromReadQueryResultsReader::Boolean(v) => Self::Boolean(v),
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
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct QuerySolutionIter {
    variables: Arc<[Variable]>,
    iter: Box<dyn Iterator<Item = Result<QuerySolution, EvaluationError>>>,
}

impl QuerySolutionIter {
    /// Construct a new iterator of solution from an ordered list of solution variables and an iterator of solution tuples
    /// (each tuple using the same ordering as the variable list such that tuple element 0 is the value for the variable 0...)
    pub fn new(
        variables: Arc<[Variable]>,
        iter: impl Iterator<Item = Result<Vec<Option<Term>>, EvaluationError>> + 'static,
    ) -> Self {
        Self {
            variables: Arc::clone(&variables),
            iter: Box::new(
                iter.map(move |t| t.map(|values| (Arc::clone(&variables), values).into())),
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
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }
}

impl<R: Read + 'static> From<FromReadSolutionsReader<R>> for QuerySolutionIter {
    fn from(reader: FromReadSolutionsReader<R>) -> Self {
        Self {
            variables: reader.variables().into(),
            iter: Box::new(reader.map(|t| t.map_err(EvaluationError::from))),
        }
    }
}

impl Iterator for QuerySolutionIter {
    type Item = Result<QuerySolution, EvaluationError>;

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
/// use oxigraph::sparql::QueryResults;
/// use oxigraph::store::Store;
///
/// let store = Store::new()?;
/// if let QueryResults::Graph(triples) = store.query("CONSTRUCT WHERE { ?s ?p ?o }")? {
///     for triple in triples {
///         println!("{}", triple?);
///     }
/// }
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct QueryTripleIter {
    pub(crate) iter: Box<dyn Iterator<Item = Result<Triple, EvaluationError>>>,
}

impl Iterator for QueryTripleIter {
    type Item = Result<Triple, EvaluationError>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    #[inline]
    fn fold<Acc, G>(self, init: Acc, g: G) -> Acc
    where
        G: FnMut(Acc, Self::Item) -> Acc,
    {
        self.iter.fold(init, g)
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
