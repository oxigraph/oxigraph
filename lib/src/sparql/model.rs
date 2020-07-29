use crate::model::*;
use crate::sparql::json_results::write_json_results;
use crate::sparql::xml_results::{read_xml_results, write_xml_results};
use crate::Error;
use crate::{FileSyntax, GraphSyntax, Result};
use rand::random;
use rio_api::formatter::TriplesFormatter;
use rio_turtle::{NTriplesFormatter, TurtleFormatter};
use rio_xml::RdfXmlFormatter;
use std::fmt;
use std::io::{BufRead, Write};
use std::rc::Rc;

/// Results of a [SPARQL query](https://www.w3.org/TR/sparql11-query/)
pub enum QueryResult {
    /// Results of a [SELECT](https://www.w3.org/TR/sparql11-query/#select) query
    Solutions(QuerySolutionsIterator),
    /// Result of a [ASK](https://www.w3.org/TR/sparql11-query/#ask) query
    Boolean(bool),
    /// Results of a [CONSTRUCT](https://www.w3.org/TR/sparql11-query/#construct) or [DESCRIBE](https://www.w3.org/TR/sparql11-query/#describe) query
    Graph(Box<dyn Iterator<Item = Result<Triple>>>),
}

impl QueryResult {
    pub fn read(reader: impl BufRead + 'static, syntax: QueryResultSyntax) -> Result<Self> {
        match syntax {
            QueryResultSyntax::Xml => read_xml_results(reader),
            QueryResultSyntax::Json => Err(Error::msg(
                //TODO: implement
                "JSON SPARQL results format parsing has not been implemented yet",
            )),
        }
    }

    /// Writes the query results (solutions or boolean)
    ///
    /// This method fails if it is called on the `Graph` results
    ///
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::{QueryOptions, QueryResultSyntax};
    ///
    /// let store = MemoryStore::new();
    /// let ex = NamedNode::new("http://example.com")?;
    /// store.insert(Quad::new(ex.clone(), ex.clone(), ex.clone(), None));
    ///
    /// let mut results = Vec::new();
    /// store.query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())?.write(&mut results, QueryResultSyntax::Json)?;
    /// assert_eq!(results, "{\"head\":{\"vars\":[\"s\"]},\"results\":{\"bindings\":[{\"s\":{\"type\":\"uri\",\"value\":\"http://example.com\"}}]}}".as_bytes());
    /// # oxigraph::Result::Ok(())
    /// ```
    pub fn write(self, writer: &mut impl Write, syntax: QueryResultSyntax) -> Result<()> {
        match syntax {
            QueryResultSyntax::Xml => write_xml_results(self, writer),
            QueryResultSyntax::Json => write_json_results(self, writer),
        }
    }

    /// Writes the graph query results
    ///
    /// This method fails if it is called on the `Solution` or `Boolean` results
    ///
    /// ```
    /// use oxigraph::{MemoryStore, GraphSyntax};
    /// use oxigraph::sparql::QueryOptions;
    /// use oxigraph::model::*;
    /// use std::io::Cursor;
    ///
    /// let graph = "<http://example.com> <http://example.com> <http://example.com> .\n".as_bytes();
    ///
    /// let store = MemoryStore::new();
    /// store.load_graph(Cursor::new(graph), GraphSyntax::NTriples, &GraphName::DefaultGraph, None)?;
    ///
    /// let mut results = Vec::new();
    /// store.query("CONSTRUCT WHERE { ?s ?p ?o }", QueryOptions::default())?.write_graph(&mut results, GraphSyntax::NTriples)?;
    /// assert_eq!(results, graph);
    /// # oxigraph::Result::Ok(())
    /// ```
    pub fn write_graph(self, write: &mut impl Write, syntax: GraphSyntax) -> Result<()> {
        if let QueryResult::Graph(triples) = self {
            match syntax {
                GraphSyntax::NTriples => {
                    let mut formatter = NTriplesFormatter::new(write);
                    for triple in triples {
                        formatter.format(&(&triple?).into())?;
                    }
                    formatter.finish();
                }
                GraphSyntax::Turtle => {
                    let mut formatter = TurtleFormatter::new(write);
                    for triple in triples {
                        formatter.format(&(&triple?).into())?;
                    }
                    formatter.finish()?;
                }
                GraphSyntax::RdfXml => {
                    let mut formatter = RdfXmlFormatter::new(write)?;
                    for triple in triples {
                        formatter.format(&(&triple?).into())?;
                    }
                    formatter.finish()?;
                }
            }
            Ok(())
        } else {
            Err(Error::msg(
                "Bindings or booleans could not be formatted as an RDF graph",
            ))
        }
    }
}

/// [SPARQL query](https://www.w3.org/TR/sparql11-query/) serialization formats
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub enum QueryResultSyntax {
    /// [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/)
    Xml,
    /// [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/)
    Json,
}

impl FileSyntax for QueryResultSyntax {
    fn iri(self) -> &'static str {
        match self {
            QueryResultSyntax::Xml => "http://www.w3.org/ns/formats/SPARQL_Results_XML",
            QueryResultSyntax::Json => "http://www.w3.org/ns/formats/SPARQL_Results_JSON",
        }
    }

    fn media_type(self) -> &'static str {
        match self {
            QueryResultSyntax::Xml => "application/sparql-results+xml",
            QueryResultSyntax::Json => "application/sparql-results+json",
        }
    }

    fn file_extension(self) -> &'static str {
        match self {
            QueryResultSyntax::Xml => "srx",
            QueryResultSyntax::Json => "srj",
        }
    }

    fn from_mime_type(media_type: &str) -> Option<Self> {
        if let Some(base_type) = media_type.split(';').next() {
            match base_type {
                "application/sparql-results+xml" | "application/xml" | "text/xml" => {
                    Some(QueryResultSyntax::Xml)
                }
                "application/sparql-results+json" | "application/json" | "text/json" => {
                    Some(QueryResultSyntax::Json)
                }
                _ => None,
            }
        } else {
            None
        }
    }
}

/// An iterator over query result solutions
///
/// ```
/// use oxigraph::MemoryStore;
/// use oxigraph::sparql::{QueryResult, QueryOptions};
///
/// let store = MemoryStore::new();
/// if let QueryResult::Solutions(solutions) = store.query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())? {
///     for solution in solutions {
///         println!("{:?}", solution?.get("s"));
///     }
/// }
/// # oxigraph::Result::Ok(())
/// ```
pub struct QuerySolutionsIterator {
    variables: Rc<Vec<Variable>>,
    iter: Box<dyn Iterator<Item = Result<Vec<Option<Term>>>>>,
}

impl QuerySolutionsIterator {
    pub fn new(
        variables: Rc<Vec<Variable>>,
        iter: Box<dyn Iterator<Item = Result<Vec<Option<Term>>>>>,
    ) -> Self {
        Self { variables, iter }
    }

    /// The variables used in the solutions
    ///
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::sparql::{QueryResult, QueryOptions, Variable};
    ///
    /// let store = MemoryStore::new();
    /// if let QueryResult::Solutions(solutions) = store.query("SELECT ?s ?o WHERE { ?s ?p ?o }", QueryOptions::default())? {
    ///     assert_eq!(solutions.variables(), &[Variable::new("s"), Variable::new("o")]);
    /// }
    /// # oxigraph::Result::Ok(())
    /// ```
    pub fn variables(&self) -> &[Variable] {
        &*self.variables
    }

    #[deprecated(note = "Please directly use QuerySolutionsIterator as an iterator instead")]
    pub fn into_values_iter(self) -> Box<dyn Iterator<Item = Result<Vec<Option<Term>>>>> {
        self.iter
    }

    #[deprecated(note = "Please directly use QuerySolutionsIterator as an iterator instead")]
    pub fn destruct(
        self,
    ) -> (
        Vec<Variable>,
        Box<dyn Iterator<Item = Result<Vec<Option<Term>>>>>,
    ) {
        ((*self.variables).clone(), self.iter)
    }
}

impl Iterator for QuerySolutionsIterator {
    type Item = Result<QuerySolution>;

    fn next(&mut self) -> Option<Result<QuerySolution>> {
        Some(self.iter.next()?.map(|values| QuerySolution {
            values,
            variables: self.variables.clone(),
        }))
    }

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
    /// Returns a value for a given position in the tuple (`usize`) or a given variable name (`&str` or `Variable`)
    ///
    /// ```ignore
    /// let foo = solution.get("foo"); // Get the value of the variable ?foo if it exists
    /// let first = solution.get(1); // Get the value of the second column if it exists
    /// ```
    pub fn get(&self, index: impl VariableSolutionIndex) -> Option<&Term> {
        self.values.get(index.index(self)?).and_then(|e| e.as_ref())
    }

    /// The number of variables which are bind
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Is this binding empty?
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Returns an iterator over bound variables
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
    fn index(self, _: &QuerySolution) -> Option<usize> {
        Some(self)
    }
}

impl VariableSolutionIndex for &str {
    fn index(self, solution: &QuerySolution) -> Option<usize> {
        solution.variables.iter().position(|v| v.as_str() == self)
    }
}

impl VariableSolutionIndex for &Variable {
    fn index(self, solution: &QuerySolution) -> Option<usize> {
        solution.variables.iter().position(|v| v == self)
    }
}

impl VariableSolutionIndex for Variable {
    fn index(self, solution: &QuerySolution) -> Option<usize> {
        (&self).index(solution)
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
    pub fn new(name: impl Into<String>) -> Self {
        Variable { name: name.into() }
    }

    pub fn as_str(&self) -> &str {
        &self.name
    }

    #[deprecated(note = "Please use as_str instead")]
    pub fn name(&self) -> Result<&str> {
        Ok(self.as_str())
    }

    pub fn into_string(self) -> String {
        self.name
    }

    pub(crate) fn new_random() -> Self {
        Self::new(format!("{:x}", random::<u128>()))
    }
}

impl fmt::Display for Variable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "?{}", self.name)
    }
}
