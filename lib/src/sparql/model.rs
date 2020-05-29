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
pub enum QueryResult<'a> {
    /// Results of a [SELECT](https://www.w3.org/TR/sparql11-query/#select) query
    Bindings(QuerySolutionsIterator<'a>),
    /// Result of a [ASK](https://www.w3.org/TR/sparql11-query/#ask) query
    Boolean(bool),
    /// Results of a [CONSTRUCT](https://www.w3.org/TR/sparql11-query/#construct) or [DESCRIBE](https://www.w3.org/TR/sparql11-query/#describe) query
    Graph(Box<dyn Iterator<Item = Result<Triple>> + 'a>),
}

impl<'a> QueryResult<'a> {
    pub fn read(reader: impl BufRead + 'a, syntax: QueryResultSyntax) -> Result<Self> {
        match syntax {
            QueryResultSyntax::Xml => read_xml_results(reader),
            QueryResultSyntax::Json => Err(Error::msg(
                //TODO: implement
                "JSON SPARQL results format parsing has not been implemented yet",
            )),
        }
    }

    pub fn write<W: Write>(self, writer: W, syntax: QueryResultSyntax) -> Result<W> {
        match syntax {
            QueryResultSyntax::Xml => write_xml_results(self, writer),
            QueryResultSyntax::Json => write_json_results(self, writer),
        }
    }

    pub fn write_graph<W: Write>(self, write: W, syntax: GraphSyntax) -> Result<W> {
        if let QueryResult::Graph(triples) = self {
            Ok(match syntax {
                GraphSyntax::NTriples => {
                    let mut formatter = NTriplesFormatter::new(write);
                    for triple in triples {
                        formatter.format(&(&triple?).into())?;
                    }
                    formatter.finish()
                }
                GraphSyntax::Turtle => {
                    let mut formatter = TurtleFormatter::new(write);
                    for triple in triples {
                        formatter.format(&(&triple?).into())?;
                    }
                    formatter.finish()?
                }
                GraphSyntax::RdfXml => {
                    let mut formatter = RdfXmlFormatter::new(write)?;
                    for triple in triples {
                        formatter.format(&(&triple?).into())?;
                    }
                    formatter.finish()?
                }
            })
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
                "application/xml" | "application/sparql-results+xml" => {
                    Some(QueryResultSyntax::Xml)
                }
                "application/json" | "application/sparql-results+json" => {
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
/// use oxigraph::{MemoryStore, Result};
/// use oxigraph::sparql::{PreparedQuery, QueryResult, QueryOptions, Variable};
///
/// let store = MemoryStore::new();
/// let prepared_query = store.prepare_query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())?;
/// if let QueryResult::Bindings(solutions) = prepared_query.exec()? {
///     for solution in solutions {
///         println!("{:?}", solution?.get("s"));
///     }
/// }
/// # Result::Ok(())
/// ```
pub struct QuerySolutionsIterator<'a> {
    variables: Rc<Vec<Variable>>,
    iter: Box<dyn Iterator<Item = Result<Vec<Option<Term>>>> + 'a>,
}

impl<'a> QuerySolutionsIterator<'a> {
    pub fn new(
        variables: Vec<Variable>,
        iter: Box<dyn Iterator<Item = Result<Vec<Option<Term>>>> + 'a>,
    ) -> Self {
        Self {
            variables: Rc::new(variables),
            iter,
        }
    }

    /// The variables used in the solutions
    ///
    /// ```
    /// use oxigraph::{MemoryStore, Result};
    /// use oxigraph::sparql::{PreparedQuery, QueryResult, QueryOptions, Variable};
    ///
    /// let store = MemoryStore::new();
    /// let prepared_query = store.prepare_query("SELECT ?s ?o WHERE { ?s ?p ?o }", QueryOptions::default())?;
    /// if let QueryResult::Bindings(solutions) = prepared_query.exec()? {
    ///     assert_eq!(solutions.variables(), &[Variable::new("s"), Variable::new("o")]);
    /// }
    /// # Result::Ok(())
    /// ```
    pub fn variables(&self) -> &[Variable] {
        &*self.variables
    }

    #[deprecated(note = "Please directly use QuerySolutionsIterator as an iterator instead")]
    pub fn into_values_iter(self) -> Box<dyn Iterator<Item = Result<Vec<Option<Term>>>> + 'a> {
        self.iter
    }

    pub fn destruct(
        self,
    ) -> (
        Vec<Variable>,
        Box<dyn Iterator<Item = Result<Vec<Option<Term>>>> + 'a>,
    ) {
        ((*self.variables).clone(), self.iter)
    }
}

impl<'a> Iterator for QuerySolutionsIterator<'a> {
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
