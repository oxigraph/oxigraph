use crate::model::*;
use crate::sparql::xml_results::{read_xml_results, write_xml_results};
use crate::Result;
use failure::format_err;
use std::fmt;
use std::io::{BufRead, Write};
use uuid::Uuid;

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
        }
    }

    pub fn write<W: Write>(self, writer: W, syntax: QueryResultSyntax) -> Result<W> {
        match syntax {
            QueryResultSyntax::Xml => write_xml_results(self, writer),
        }
    }
}

/// [SPARQL query](https://www.w3.org/TR/sparql11-query/) serialization formats
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum QueryResultSyntax {
    /// [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/)
    Xml,
}

/// An iterator over results bindings
pub struct BindingsIterator<'a> {
    variables: Vec<Variable>,
    iter: Box<dyn Iterator<Item = Result<Vec<Option<Term>>>> + 'a>,
}

impl<'a> BindingsIterator<'a> {
    pub(crate) fn new(
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
    BlankNode { id: Uuid },
    Internal { id: Uuid },
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
            Variable::BlankNode { id } => write!(f, "_:{}", id.to_simple()),
            Variable::Internal { id } => write!(f, "?{}", id.to_simple()),
        }
    }
}

impl Default for Variable {
    fn default() -> Self {
        Variable::Internal { id: Uuid::new_v4() }
    }
}

impl From<BlankNode> for Variable {
    fn from(blank_node: BlankNode) -> Self {
        Variable::BlankNode {
            id: *blank_node.as_uuid(),
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
