use crate::io::GraphFormat;
use crate::io::GraphSerializer;
use crate::model::*;
use crate::sparql::error::EvaluationError;
use oxrdf::{Variable, VariableRef};
pub use sparesults::QuerySolution;
use sparesults::{
    ParseError, QueryResultsFormat, QueryResultsParser, QueryResultsReader, QueryResultsSerializer,
    SolutionsReader,
};
use std::io::{BufRead, Write};
use std::rc::Rc;

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
        reader: impl BufRead + 'static,
        format: QueryResultsFormat,
    ) -> Result<Self, ParseError> {
        Ok(QueryResultsParser::from_format(format)
            .read_results(reader)?
            .into())
    }

    /// Writes the query results (solutions or boolean).
    ///
    /// This method fails if it is called on the `Graph` results.
    ///
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::QueryResultsFormat;
    ///
    /// let store = Store::new()?;
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// store.insert(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?;
    ///
    /// let mut results = Vec::new();
    /// store.query("SELECT ?s WHERE { ?s ?p ?o }")?.write(&mut results, QueryResultsFormat::Json)?;
    /// assert_eq!(results, "{\"head\":{\"vars\":[\"s\"]},\"results\":{\"bindings\":[{\"s\":{\"type\":\"uri\",\"value\":\"http://example.com\"}}]}}".as_bytes());
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn write(
        self,
        writer: impl Write,
        format: QueryResultsFormat,
    ) -> Result<(), EvaluationError> {
        let serializer = QueryResultsSerializer::from_format(format);
        match self {
            Self::Boolean(value) => {
                serializer.write_boolean_result(writer, value)?;
            }
            QueryResults::Solutions(solutions) => {
                let mut writer =
                    serializer.solutions_writer(writer, solutions.variables().to_vec())?;
                for solution in solutions {
                    writer.write(&solution?)?;
                }
                writer.finish()?;
            }
            QueryResults::Graph(triples) => {
                let s = VariableRef::new_unchecked("subject");
                let p = VariableRef::new_unchecked("predicate");
                let o = VariableRef::new_unchecked("object");
                let mut writer = serializer.solutions_writer(
                    writer,
                    vec![s.into_owned(), p.into_owned(), o.into_owned()],
                )?;
                for triple in triples {
                    let triple = triple?;
                    writer.write([
                        (s, &triple.subject.into()),
                        (p, &triple.predicate.into()),
                        (o, &triple.object),
                    ])?;
                }
                writer.finish()?;
            }
        }
        Ok(())
    }

    /// Writes the graph query results.
    ///
    /// This method fails if it is called on the `Solution` or `Boolean` results.
    ///
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::io::GraphFormat;
    /// use oxigraph::model::*;
    /// use std::io::Cursor;
    ///
    /// let graph = "<http://example.com> <http://example.com> <http://example.com> .\n".as_bytes();
    ///
    /// let store = Store::new()?;
    /// store.load_graph(Cursor::new(graph), GraphFormat::NTriples, GraphNameRef::DefaultGraph, None)?;
    ///
    /// let mut results = Vec::new();
    /// store.query("CONSTRUCT WHERE { ?s ?p ?o }")?.write_graph(&mut results, GraphFormat::NTriples)?;
    /// assert_eq!(results, graph);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn write_graph(
        self,
        write: impl Write,
        format: GraphFormat,
    ) -> Result<(), EvaluationError> {
        if let QueryResults::Graph(triples) = self {
            let mut writer = GraphSerializer::from_format(format).triple_writer(write)?;
            for triple in triples {
                writer.write(&triple?)?;
            }
            writer.finish()?;
            Ok(())
        } else {
            Err(EvaluationError::msg(
                "Bindings or booleans could not be formatted as an RDF graph",
            ))
        }
    }
}

impl From<QuerySolutionIter> for QueryResults {
    #[inline]
    fn from(value: QuerySolutionIter) -> Self {
        Self::Solutions(value)
    }
}

impl<R: BufRead + 'static> From<QueryResultsReader<R>> for QueryResults {
    fn from(reader: QueryResultsReader<R>) -> Self {
        match reader {
            QueryResultsReader::Solutions(s) => Self::Solutions(s.into()),
            QueryResultsReader::Boolean(v) => Self::Boolean(v),
        }
    }
}

/// An iterator over [`QuerySolution`]s.
///
/// ```
/// use oxigraph::store::Store;
/// use oxigraph::sparql::QueryResults;
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
    variables: Rc<Vec<Variable>>,
    iter: Box<dyn Iterator<Item = Result<QuerySolution, EvaluationError>>>,
}

impl QuerySolutionIter {
    pub fn new(
        variables: Rc<Vec<Variable>>,
        iter: impl Iterator<Item = Result<Vec<Option<Term>>, EvaluationError>> + 'static,
    ) -> Self {
        Self {
            variables: variables.clone(),
            iter: Box::new(iter.map(move |t| t.map(|values| (variables.clone(), values).into()))),
        }
    }

    /// The variables used in the solutions.
    ///
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::sparql::{QueryResults, Variable};
    ///
    /// let store = Store::new()?;
    /// if let QueryResults::Solutions(solutions) = store.query("SELECT ?s ?o WHERE { ?s ?p ?o }")? {
    ///     assert_eq!(solutions.variables(), &[Variable::new("s")?, Variable::new("o")?]);
    /// }
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn variables(&self) -> &[Variable] {
        &*self.variables
    }
}

impl<R: BufRead + 'static> From<SolutionsReader<R>> for QuerySolutionIter {
    fn from(reader: SolutionsReader<R>) -> Self {
        Self {
            variables: Rc::new(reader.variables().to_vec()),
            iter: Box::new(reader.map(|t| t.map_err(EvaluationError::from))),
        }
    }
}

impl Iterator for QuerySolutionIter {
    type Item = Result<QuerySolution, EvaluationError>;

    #[inline]
    fn next(&mut self) -> Option<Result<QuerySolution, EvaluationError>> {
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
/// use oxigraph::store::Store;
/// use oxigraph::sparql::QueryResults;
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
    fn next(&mut self) -> Option<Result<Triple, EvaluationError>> {
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

#[test]
fn test_serialization_roundtrip() -> Result<(), EvaluationError> {
    use std::io::Cursor;
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
                Rc::new(vec![
                    Variable::new_unchecked("foo"),
                    Variable::new_unchecked("bar"),
                ]),
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
                                Literal::new_language_tagged_literal_unchecked("foo", "fr").into(),
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
