use crate::error::invalid_input_error;
use crate::io::GraphFormat;
use crate::io::GraphSerializer;
use crate::model::*;
use crate::sparql::error::EvaluationError;
use crate::sparql::json_results::write_json_results;
use crate::sparql::xml_results::{read_xml_results, write_xml_results};
use rand::random;
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
            QueryResultsFormat::Json => Err(invalid_input_error(
                "JSON SPARQL results format parsing has not been implemented yet",
            )), //TODO: implement
        }
    }

    /// Writes the query results (solutions or boolean)
    ///
    /// This method fails if it is called on the `Graph` results
    ///
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::{QueryOptions, QueryResultsFormat};
    ///
    /// let store = MemoryStore::new();
    /// let ex = NamedNode::new("http://example.com")?;
    /// store.insert(Quad::new(ex.clone(), ex.clone(), ex.clone(), None));
    ///
    /// let mut results = Vec::new();
    /// store.query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())?.write(&mut results, QueryResultsFormat::Json)?;
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
        }
    }

    /// Writes the graph query results
    ///
    /// This method fails if it is called on the `Solution` or `Boolean` results
    ///
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::io::GraphFormat;
    /// use oxigraph::sparql::QueryOptions;
    /// use oxigraph::model::*;
    /// use std::io::Cursor;
    ///
    /// let graph = "<http://example.com> <http://example.com> <http://example.com> .\n".as_bytes();
    ///
    /// let store = MemoryStore::new();
    /// store.load_graph(Cursor::new(graph), GraphFormat::NTriples, &GraphName::DefaultGraph, None)?;
    ///
    /// let mut results = Vec::new();
    /// store.query("CONSTRUCT WHERE { ?s ?p ?o }", QueryOptions::default())?.write_graph(&mut results, GraphFormat::NTriples)?;
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
///
/// This enumeration is non exhaustive. New formats like CSV will be added in the future.
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
#[non_exhaustive]
pub enum QueryResultsFormat {
    /// [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/)
    Xml,
    /// [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/)
    Json,
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
                _ => None,
            }
        } else {
            None
        }
    }
}

/// An iterator over [`QuerySolution`s](struct.QuerySolution.html)
///
/// ```
/// use oxigraph::MemoryStore;
/// use oxigraph::sparql::{QueryResults, QueryOptions};
///
/// let store = MemoryStore::new();
/// if let QueryResults::Solutions(solutions) = store.query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())? {
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
    /// use oxigraph::sparql::{QueryResults, QueryOptions, Variable};
    ///
    /// let store = MemoryStore::new();
    /// if let QueryResults::Solutions(solutions) = store.query("SELECT ?s ?o WHERE { ?s ?p ?o }", QueryOptions::default())? {
    ///     assert_eq!(solutions.variables(), &[Variable::new("s"), Variable::new("o")]);
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
    /// Returns a value for a given position in the tuple ([`usize`](https://doc.rust-lang.org/std/primitive.usize.html)) or a given variable name ([`&str`](https://doc.rust-lang.org/std/primitive.str.html) or [`Variable`](struct.Variable.html))
    ///
    /// ```ignore
    /// let foo = solution.get("foo"); // Get the value of the variable ?foo if it exists
    /// let first = solution.get(1); // Get the value of the second column if it exists
    /// ```
    #[inline]
    pub fn get(&self, index: impl VariableSolutionIndex) -> Option<&Term> {
        self.values.get(index.index(self)?).and_then(|e| e.as_ref())
    }

    /// The number of variables which are bind
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
            .filter_map(move |(i, value)| {
                if let Some(value) = value {
                    Some((&self.variables[i], value))
                } else {
                    None
                }
            })
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
/// use oxigraph::sparql::{QueryResults, QueryOptions};
///
/// let store = MemoryStore::new();
/// if let QueryResults::Graph(triples) = store.query("CONSTRUCT WHERE { ?s ?p ?o }", QueryOptions::default())? {
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
///     Variable::new("foo").to_string()
/// )
/// ```
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct Variable {
    name: String,
}

impl Variable {
    #[inline]
    pub fn new(name: impl Into<String>) -> Self {
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
        Self::new(format!("{:x}", random::<u128>()))
    }
}

impl fmt::Display for Variable {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?{}", self.name)
    }
}
