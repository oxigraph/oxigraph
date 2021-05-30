use crate::error::invalid_input_error;
use crate::io::GraphFormat;
use crate::io::GraphSerializer;
use crate::model::*;
use crate::sparql::csv_results::{read_tsv_results, write_csv_results, write_tsv_results};
use crate::sparql::error::EvaluationError;
use crate::sparql::json_results::{read_json_results, write_json_results};
use crate::sparql::xml_results::{read_xml_results, write_xml_results};
use rand::random;
use std::error::Error;
use std::io::{BufRead, Write};
use std::rc::Rc;
use std::{fmt, io};

/// Results of a [SPARQL query](https://www.w3.org/TR/sparql11-query/)
pub enum QueryResults {
    /// Results of a [SELECT](https://www.w3.org/TR/sparql11-query/#select) query
    Solutions(QuerySolutionIter),
    /// Result of a [ASK](https://www.w3.org/TR/sparql11-query/#ask) query
    Boolean(bool),
    /// Results of a [CONSTRUCT](https://www.w3.org/TR/sparql11-query/#construct) or [DESCRIBE](https://www.w3.org/TR/sparql11-query/#describe) query
    Graph(QueryTripleIter),
}

impl QueryResults {
    /// Reads a SPARQL query results serialization
    pub fn read(
        reader: impl BufRead + 'static,
        format: QueryResultsFormat,
    ) -> Result<Self, io::Error> {
        match format {
            QueryResultsFormat::Xml => read_xml_results(reader),
            QueryResultsFormat::Json => read_json_results(reader),
            QueryResultsFormat::Csv => Err(invalid_input_error(
                "CSV SPARQL results format parsing is not implemented",
            )),
            QueryResultsFormat::Tsv => read_tsv_results(reader),
        }
    }

    /// Writes the query results (solutions or boolean)
    ///
    /// This method fails if it is called on the `Graph` results
    ///
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::QueryResultsFormat;
    ///
    /// let store = MemoryStore::new();
    /// let ex = NamedNode::new("http://example.com")?;
    /// store.insert(Quad::new(ex.clone(), ex.clone(), ex.clone(), None));
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
        match format {
            QueryResultsFormat::Xml => write_xml_results(self, writer),
            QueryResultsFormat::Json => write_json_results(self, writer),
            QueryResultsFormat::Csv => write_csv_results(self, writer),
            QueryResultsFormat::Tsv => write_tsv_results(self, writer),
        }
    }

    /// Writes the graph query results
    ///
    /// This method fails if it is called on the `Solution` or `Boolean` results
    ///
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::io::GraphFormat;
    /// use oxigraph::model::*;
    /// use std::io::Cursor;
    ///
    /// let graph = "<http://example.com> <http://example.com> <http://example.com> .\n".as_bytes();
    ///
    /// let store = MemoryStore::new();
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
            Err(
                invalid_input_error("Bindings or booleans could not be formatted as an RDF graph")
                    .into(),
            )
        }
    }
}

impl From<QuerySolutionIter> for QueryResults {
    #[inline]
    fn from(value: QuerySolutionIter) -> Self {
        QueryResults::Solutions(value)
    }
}

/// [SPARQL query](https://www.w3.org/TR/sparql11-query/) results serialization formats
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
#[non_exhaustive]
pub enum QueryResultsFormat {
    /// [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/)
    Xml,
    /// [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/)
    Json,
    /// [SPARQL Query Results CSV Format](https://www.w3.org/TR/sparql11-results-csv-tsv/)
    Csv,
    /// [SPARQL Query Results TSV Format](https://www.w3.org/TR/sparql11-results-csv-tsv/)
    Tsv,
}

impl QueryResultsFormat {
    /// The format canonical IRI according to the [Unique URIs for file formats registry](https://www.w3.org/ns/formats/).
    ///
    /// ```
    /// use oxigraph::sparql::QueryResultsFormat;
    ///
    /// assert_eq!(QueryResultsFormat::Json.iri(), "http://www.w3.org/ns/formats/SPARQL_Results_JSON")
    /// ```
    #[inline]
    pub fn iri(self) -> &'static str {
        match self {
            QueryResultsFormat::Xml => "http://www.w3.org/ns/formats/SPARQL_Results_XML",
            QueryResultsFormat::Json => "http://www.w3.org/ns/formats/SPARQL_Results_JSON",
            QueryResultsFormat::Csv => "http://www.w3.org/ns/formats/SPARQL_Results_CSV",
            QueryResultsFormat::Tsv => "http://www.w3.org/ns/formats/SPARQL_Results_TSV",
        }
    }
    /// The format [IANA media type](https://tools.ietf.org/html/rfc2046).
    ///
    /// ```
    /// use oxigraph::sparql::QueryResultsFormat;
    ///
    /// assert_eq!(QueryResultsFormat::Json.media_type(), "application/sparql-results+json")
    /// ```
    #[inline]
    pub fn media_type(self) -> &'static str {
        match self {
            QueryResultsFormat::Xml => "application/sparql-results+xml",
            QueryResultsFormat::Json => "application/sparql-results+json",
            QueryResultsFormat::Csv => "text/csv; charset=utf-8",
            QueryResultsFormat::Tsv => "text/tab-separated-values; charset=utf-8",
        }
    }

    /// The format [IANA-registered](https://tools.ietf.org/html/rfc2046) file extension.
    ///
    /// ```
    /// use oxigraph::sparql::QueryResultsFormat;
    ///
    /// assert_eq!(QueryResultsFormat::Json.file_extension(), "srj")
    /// ```
    #[inline]
    pub fn file_extension(self) -> &'static str {
        match self {
            QueryResultsFormat::Xml => "srx",
            QueryResultsFormat::Json => "srj",
            QueryResultsFormat::Csv => "csv",
            QueryResultsFormat::Tsv => "tsv",
        }
    }

    /// Looks for a known format from a media type.
    ///
    /// It supports some media type aliases.
    /// For example "application/xml" is going to return `Xml` even if it is not its canonical media type.
    ///
    /// Example:
    /// ```
    /// use oxigraph::sparql::QueryResultsFormat;
    ///
    /// assert_eq!(QueryResultsFormat::from_media_type("application/sparql-results+json; charset=utf-8"), Some(QueryResultsFormat::Json))
    /// ```
    pub fn from_media_type(media_type: &str) -> Option<Self> {
        if let Some(base_type) = media_type.split(';').next() {
            match base_type {
                "application/sparql-results+xml" | "application/xml" | "text/xml" => {
                    Some(QueryResultsFormat::Xml)
                }
                "application/sparql-results+json" | "application/json" | "text/json" => {
                    Some(QueryResultsFormat::Json)
                }
                "text/csv" => Some(QueryResultsFormat::Csv),
                "text/tab-separated-values" | "text/tsv" => Some(QueryResultsFormat::Tsv),
                _ => None,
            }
        } else {
            None
        }
    }
}

/// An iterator over [`QuerySolution`]s
///
/// ```
/// use oxigraph::MemoryStore;
/// use oxigraph::sparql::QueryResults;
///
/// let store = MemoryStore::new();
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

    /// The variables used in the solutions
    ///
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::sparql::{QueryResults, Variable};
    ///
    /// let store = MemoryStore::new();
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
    values: Vec<Option<Term>>,
    variables: Rc<Vec<Variable>>,
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
        self.values.get(index.index(self)?).and_then(|e| e.as_ref())
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
        self.values.iter().map(|v| v.as_ref())
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

/// An iterator over the triples that compose a graph solution
///
/// ```
/// use oxigraph::MemoryStore;
/// use oxigraph::sparql::QueryResults;
///
/// let store = MemoryStore::new();
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
    fn fold<Acc, G>(self, init: Acc, mut g: G) -> Acc
    where
        G: FnMut(Acc, Self::Item) -> Acc,
    {
        self.iter.fold(init, |acc, elt| g(acc, elt))
    }
}

/// A SPARQL query variable
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
        Variable { name: name.into() }
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.name
    }

    #[inline]
    pub fn into_string(self) -> String {
        self.name
    }

    #[inline]
    pub(crate) fn new_random() -> Self {
        Self::new_unchecked(format!("{:x}", random::<u128>()))
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

    for format in &[
        QueryResultsFormat::Xml,
        QueryResultsFormat::Json,
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
                    ]
                    .into_iter(),
                ),
            )),
        ];

        for ex in results {
            let mut buffer = Vec::new();
            ex.write(&mut buffer, *format)?;
            let ex2 = QueryResults::read(Cursor::new(buffer.clone()), *format)?;
            let mut buffer2 = Vec::new();
            ex2.write(&mut buffer2, *format)?;
            assert_eq!(
                str::from_utf8(&buffer).unwrap(),
                str::from_utf8(&buffer2).unwrap()
            );
        }
    }

    Ok(())
}
