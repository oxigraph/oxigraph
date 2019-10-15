use crate::model::*;
use crate::sparql::json_results::write_json_results;
use crate::sparql::xml_results::{read_xml_results, write_xml_results};
use crate::{FileSyntax, GraphSyntax, Result};
use failure::format_err;
use rand::random;
use rio_api::formatter::TriplesFormatter;
use rio_turtle::{NTriplesFormatter, TurtleFormatter};
use rio_xml::RdfXmlFormatter;
use std::fmt;
use std::io::{BufRead, Write};

/// Results of a [SPARQL query](https://www.w3.org/TR/sparql11-query/)
pub enum QueryResult<'a> {
    /// Results of a [SELECT](https://www.w3.org/TR/sparql11-query/#select) query
    Bindings(BindingsIterator<'a>),
    /// Result of a [ASK](https://www.w3.org/TR/sparql11-query/#ask) query
    Boolean(bool),
    /// Results of a [CONSTRUCT](https://www.w3.org/TR/sparql11-query/#construct) or [DESCRIBE](https://www.w3.org/TR/sparql11-query/#describe) query
    Graph(Box<dyn Iterator<Item = Result<Triple>> + 'a>),
}

impl<'a> QueryResult<'a> {
    pub fn read(reader: impl BufRead + 'a, syntax: QueryResultSyntax) -> Result<Self> {
        match syntax {
            QueryResultSyntax::Xml => read_xml_results(reader),
            QueryResultSyntax::Json => unimplemented!(),
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
            Err(format_err!(
                "Bindings or booleans could not be formatted as an RDF graph"
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
        unimplemented!()
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
                "application/sparql-results+xml" => Some(QueryResultSyntax::Xml),
                "application/sparql-results+json" => Some(QueryResultSyntax::Json),
                _ => None,
            }
        } else {
            None
        }
    }
}

/// An iterator over results bindings
pub struct BindingsIterator<'a> {
    variables: Vec<Variable>,
    iter: Box<dyn Iterator<Item = Result<Vec<Option<Term>>>> + 'a>,
}

impl<'a> BindingsIterator<'a> {
    pub fn new(
        variables: Vec<Variable>,
        iter: Box<dyn Iterator<Item = Result<Vec<Option<Term>>>> + 'a>,
    ) -> Self {
        Self { variables, iter }
    }

    pub fn variables(&self) -> &[Variable] {
        &*self.variables
    }

    pub fn into_values_iter(self) -> Box<dyn Iterator<Item = Result<Vec<Option<Term>>>> + 'a> {
        self.iter
    }

    pub fn destruct(
        self,
    ) -> (
        Vec<Variable>,
        Box<dyn Iterator<Item = Result<Vec<Option<Term>>>> + 'a>,
    ) {
        (self.variables, self.iter)
    }
}

/// A SPARQL query variable
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum Variable {
    Variable { name: String },
    BlankNode { id: u128 },
    Internal { id: u128 },
}

impl Variable {
    pub fn new(name: impl Into<String>) -> Self {
        Variable::Variable { name: name.into() }
    }

    pub fn has_name(&self) -> bool {
        match self {
            Variable::Variable { .. } => true,
            _ => false,
        }
    }

    pub fn name(&self) -> Result<&str> {
        match self {
            Variable::Variable { name } => Ok(name),
            _ => Err(format_err!("The variable {} has no name", self)),
        }
    }
}

impl fmt::Display for Variable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Variable::Variable { name } => write!(f, "?{}", name),
            Variable::BlankNode { id } => write!(f, "_:{:x}", id),
            Variable::Internal { id } => write!(f, "?{:x}", id),
        }
    }
}

impl Default for Variable {
    fn default() -> Self {
        Variable::Internal {
            id: random::<u128>(),
        }
    }
}

impl From<BlankNode> for Variable {
    fn from(blank_node: BlankNode) -> Self {
        Variable::BlankNode {
            id: blank_node.id(),
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum NamedNodeOrVariable {
    NamedNode(NamedNode),
    Variable(Variable),
}

impl fmt::Display for NamedNodeOrVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NamedNodeOrVariable::NamedNode(node) => write!(f, "{}", node),
            NamedNodeOrVariable::Variable(var) => write!(f, "{}", var),
        }
    }
}

impl From<NamedNode> for NamedNodeOrVariable {
    fn from(node: NamedNode) -> Self {
        NamedNodeOrVariable::NamedNode(node)
    }
}

impl From<Variable> for NamedNodeOrVariable {
    fn from(var: Variable) -> Self {
        NamedNodeOrVariable::Variable(var)
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum TermOrVariable {
    Term(Term),
    Variable(Variable),
}

impl fmt::Display for TermOrVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TermOrVariable::Term(term) => write!(f, "{}", term),
            TermOrVariable::Variable(var) => write!(f, "{}", var),
        }
    }
}

impl From<NamedNode> for TermOrVariable {
    fn from(node: NamedNode) -> Self {
        TermOrVariable::Term(node.into())
    }
}

impl From<BlankNode> for TermOrVariable {
    fn from(node: BlankNode) -> Self {
        TermOrVariable::Variable(node.into())
    }
}

impl From<Literal> for TermOrVariable {
    fn from(literal: Literal) -> Self {
        TermOrVariable::Term(literal.into())
    }
}

impl From<Variable> for TermOrVariable {
    fn from(var: Variable) -> Self {
        TermOrVariable::Variable(var)
    }
}

impl From<Term> for TermOrVariable {
    fn from(term: Term) -> Self {
        match term {
            Term::NamedNode(node) => TermOrVariable::Term(node.into()),
            Term::BlankNode(node) => TermOrVariable::Variable(node.into()),
            Term::Literal(literal) => TermOrVariable::Term(literal.into()),
        }
    }
}

impl From<NamedNodeOrVariable> for TermOrVariable {
    fn from(element: NamedNodeOrVariable) -> Self {
        match element {
            NamedNodeOrVariable::NamedNode(node) => TermOrVariable::Term(node.into()),
            NamedNodeOrVariable::Variable(var) => TermOrVariable::Variable(var),
        }
    }
}
