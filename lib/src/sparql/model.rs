use crate::io::GraphFormat;
use crate::io::GraphSerializer;
use crate::model::*;
use crate::sparql::error::EvaluationError;
use crate::sparql::io::{QueryResultsFormat, QueryResultsParser, QueryResultsSerializer};
use std::error::Error;
use std::io::{BufRead, Write};
use std::rc::Rc;
use std::{fmt, io};

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
    pub fn read(reader: impl BufRead + 'static, format: QueryResultsFormat) -> io::Result<Self> {
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
                let mut writer = serializer.solutions_writer(writer, solutions.variables())?;
                for solution in solutions {
                    writer.write(
                        solution?
                            .values
                            .iter()
                            .map(|t| t.as_ref().map(|t| t.as_ref())),
                    )?;
                }
                writer.finish()?;
            }
            QueryResults::Graph(triples) => {
                let mut writer = serializer.solutions_writer(
                    writer,
                    &[
                        Variable::new_unchecked("subject"),
                        Variable::new_unchecked("predicate"),
                        Variable::new_unchecked("object"),
                    ],
                )?;
                for triple in triples {
                    let triple = triple?;
                    writer.write([
                        Some(triple.subject.as_ref().into()),
                        Some(triple.predicate.as_ref().into()),
                        Some(triple.object.as_ref()),
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
    /// store.load_graph(Cursor::new(graph), GraphFormat::NTriples, &GraphName::DefaultGraph, None)?;
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
    iter: Box<dyn Iterator<Item = Result<Vec<Option<Term>>, EvaluationError>>>,
}

impl QuerySolutionIter {
    pub fn new(
        variables: Rc<Vec<Variable>>,
        iter: Box<dyn Iterator<Item = Result<Vec<Option<Term>>, EvaluationError>>>,
    ) -> Self {
        Self { variables, iter }
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

impl Iterator for QuerySolutionIter {
    type Item = Result<QuerySolution, EvaluationError>;

    #[inline]
    fn next(&mut self) -> Option<Result<QuerySolution, EvaluationError>> {
        Some(self.iter.next()?.map(|values| QuerySolution {
            values,
            variables: self.variables.clone(),
        }))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// Tuple associating variables and terms that are the result of a SPARQL query.
///
/// It is the equivalent of a row in SQL.
pub struct QuerySolution {
    pub(super) values: Vec<Option<Term>>,
    pub(super) variables: Rc<Vec<Variable>>,
}

impl QuerySolution {
    /// Returns a value for a given position in the tuple ([`usize`](std::usize)) or a given variable name ([`&str`](std::str) or [`Variable`])
    ///
    /// ```ignore
    /// let foo = solution.get("foo"); // Get the value of the variable ?foo if it exists
    /// let first = solution.get(1); // Get the value of the second column if it exists
    /// ```
    #[inline]
    pub fn get(&self, index: impl VariableSolutionIndex) -> Option<&Term> {
        self.values
            .get(index.index(self)?)
            .and_then(std::option::Option::as_ref)
    }

    /// The number of variables which could be bound
    #[inline]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Is this binding empty?
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns an iterator over bound variables
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&Variable, &Term)> {
        self.values
            .iter()
            .enumerate()
            .filter_map(move |(i, value)| value.as_ref().map(|value| (&self.variables[i], value)))
    }

    /// Returns an iterator over all values, bound or not
    #[inline]
    pub fn values(&self) -> impl Iterator<Item = Option<&Term>> {
        self.values.iter().map(std::option::Option::as_ref)
    }
}

/// A utility trait to get values for a given variable or tuple position
pub trait VariableSolutionIndex {
    fn index(self, solution: &QuerySolution) -> Option<usize>;
}

impl VariableSolutionIndex for usize {
    #[inline]
    fn index(self, _: &QuerySolution) -> Option<usize> {
        Some(self)
    }
}

impl VariableSolutionIndex for &str {
    #[inline]
    fn index(self, solution: &QuerySolution) -> Option<usize> {
        solution.variables.iter().position(|v| v.as_str() == self)
    }
}

impl VariableSolutionIndex for &Variable {
    #[inline]
    fn index(self, solution: &QuerySolution) -> Option<usize> {
        solution.variables.iter().position(|v| v == self)
    }
}

impl VariableSolutionIndex for Variable {
    #[inline]
    fn index(self, solution: &QuerySolution) -> Option<usize> {
        (&self).index(solution)
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

/// A SPARQL query variable.
///
/// ```
/// use oxigraph::sparql::Variable;
///
/// assert_eq!(
///     "?foo",
///     Variable::new("foo")?.to_string()
/// );
/// # Result::<_,oxigraph::sparql::VariableNameParseError>::Ok(())
/// ```
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct Variable {
    name: String,
}

impl Variable {
    /// Creates a variable name from a unique identifier.
    ///
    /// The variable identifier must be valid according to the SPARQL grammar.
    pub fn new(name: impl Into<String>) -> Result<Self, VariableNameParseError> {
        let name = name.into();
        validate_variable_identifier(&name)?;
        Ok(Self::new_unchecked(name))
    }

    /// Creates a variable name from a unique identifier without validation.
    ///
    /// It is the caller's responsibility to ensure that `id` is a valid blank node identifier
    /// according to the SPARQL grammar.
    ///
    /// [`Variable::new()`] is a safe version of this constructor and should be used for untrusted data.
    #[inline]
    pub fn new_unchecked(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.name
    }

    #[inline]
    pub fn into_string(self) -> String {
        self.name
    }
}

impl fmt::Display for Variable {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?{}", self.name)
    }
}

fn validate_variable_identifier(id: &str) -> Result<(), VariableNameParseError> {
    let mut chars = id.chars();
    let front = chars.next().ok_or(VariableNameParseError {})?;
    match front {
        '0'..='9'
        | '_'
        | ':'
        | 'A'..='Z'
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
        | '\u{10000}'..='\u{EFFFF}' => (),
        _ => return Err(VariableNameParseError {}),
    }
    for c in chars {
        match c {
            '0'..='9'
            | '\u{00B7}'
            | '\u{00300}'..='\u{036F}'
            | '\u{203F}'..='\u{2040}'
            | '_'
            | 'A'..='Z'
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
            | '\u{10000}'..='\u{EFFFF}' => (),
            _ => return Err(VariableNameParseError {}),
        }
    }
    Ok(())
}

/// An error raised during [`Variable`] name validation.
#[allow(missing_copy_implementations)]
#[derive(Debug)]
pub struct VariableNameParseError {}

impl fmt::Display for VariableNameParseError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "The variable name is invalid")
    }
}

impl Error for VariableNameParseError {}

#[test]
fn test_serialization_rountrip() -> Result<(), EvaluationError> {
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
