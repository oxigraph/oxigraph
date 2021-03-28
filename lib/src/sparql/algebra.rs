//! [SPARQL 1.1 Query Algebra](https://www.w3.org/TR/sparql11-query/#sparqlQuery)
//!
//! The root type for SPARQL queries is [`Query`] and the root type for updates is [`Update`].
//!
//! Warning: this implementation is an unstable work in progress

use crate::model::*;
use crate::sparql::model::*;
use crate::sparql::parser::{parse_query, parse_update, ParseError};
use oxiri::Iri;
use rio_api::model as rio;
use std::collections::BTreeSet;
use std::convert::TryFrom;
use std::fmt;
use std::rc::Rc;
use std::str::FromStr;

/// A parsed [SPARQL query](https://www.w3.org/TR/sparql11-query/)
///
/// ```
/// use oxigraph::model::NamedNode;
/// use oxigraph::sparql::Query;
///
/// let query_str = "SELECT ?s ?p ?o WHERE { ?s ?p ?o . }";
/// let mut query = Query::parse(query_str, None)?;
///
/// assert_eq!(query.to_string(), query_str);
///
/// // We edit the query dataset specification
/// query.dataset_mut().set_default_graph(vec![NamedNode::new("http://example.com").unwrap().into()]);
/// assert_eq!(query.to_string(), "SELECT ?s ?p ?o FROM <http://example.com> WHERE { ?s ?p ?o . }");
/// # Result::Ok::<_, Box<dyn std::error::Error>>(())
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum Query {
    /// [SELECT](https://www.w3.org/TR/sparql11-query/#select)
    Select {
        /// The [query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset)
        dataset: QueryDataset,
        /// The query selection graph pattern
        pattern: GraphPattern,
        /// The query base IRI
        base_iri: Option<Iri<String>>,
    },
    /// [CONSTRUCT](https://www.w3.org/TR/sparql11-query/#construct)
    Construct {
        /// The query construction template
        template: Vec<TriplePattern>,
        /// The [query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset)
        dataset: QueryDataset,
        /// The query selection graph pattern
        pattern: GraphPattern,
        /// The query base IRI
        base_iri: Option<Iri<String>>,
    },
    /// [DESCRIBE](https://www.w3.org/TR/sparql11-query/#describe)
    Describe {
        /// The [query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset)
        dataset: QueryDataset,
        /// The query selection graph pattern
        pattern: GraphPattern,
        /// The query base IRI
        base_iri: Option<Iri<String>>,
    },
    /// [ASK](https://www.w3.org/TR/sparql11-query/#ask)
    Ask {
        /// The [query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset)
        dataset: QueryDataset,
        /// The query selection graph pattern
        pattern: Rc<GraphPattern>,
        /// The query base IRI
        base_iri: Option<Iri<String>>,
    },
}

impl Query {
    /// Parses a SPARQL query with an optional base IRI to resolve relative IRIs in the query
    pub fn parse(query: &str, base_iri: Option<&str>) -> Result<Self, ParseError> {
        parse_query(query, base_iri)
    }

    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset)
    pub fn dataset(&self) -> &QueryDataset {
        match self {
            Query::Select { dataset, .. } => dataset,
            Query::Construct { dataset, .. } => dataset,
            Query::Describe { dataset, .. } => dataset,
            Query::Ask { dataset, .. } => dataset,
        }
    }

    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset)
    pub fn dataset_mut(&mut self) -> &mut QueryDataset {
        match self {
            Query::Select { dataset, .. } => dataset,
            Query::Construct { dataset, .. } => dataset,
            Query::Describe { dataset, .. } => dataset,
            Query::Ask { dataset, .. } => dataset,
        }
    }
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Query::Select {
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    writeln!(f, "BASE <{}>", base_iri)?;
                }
                write!(f, "{}", SparqlGraphRootPattern { pattern, dataset })
            }
            Query::Construct {
                template,
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    writeln!(f, "BASE <{}>", base_iri)?;
                }
                write!(f, "CONSTRUCT {{ ")?;
                for triple in template.iter() {
                    write!(f, "{} ", SparqlTriplePattern(triple))?;
                }
                write!(
                    f,
                    "}}{} WHERE {{ {} }}",
                    dataset,
                    SparqlGraphRootPattern {
                        pattern,
                        dataset: &QueryDataset::default()
                    }
                )
            }
            Query::Describe {
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    writeln!(f, "BASE <{}>", base_iri.as_str())?;
                }
                write!(
                    f,
                    "DESCRIBE *{} WHERE {{ {} }}",
                    dataset,
                    SparqlGraphRootPattern {
                        pattern,
                        dataset: &QueryDataset::default()
                    }
                )
            }
            Query::Ask {
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    writeln!(f, "BASE <{}>", base_iri)?;
                }
                write!(
                    f,
                    "ASK{} WHERE {{ {} }}",
                    dataset,
                    SparqlGraphRootPattern {
                        pattern,
                        dataset: &QueryDataset::default()
                    }
                )
            }
        }
    }
}

impl FromStr for Query {
    type Err = ParseError;

    fn from_str(query: &str) -> Result<Self, ParseError> {
        Self::parse(query, None)
    }
}

impl<'a> TryFrom<&'a str> for Query {
    type Error = ParseError;

    fn try_from(query: &str) -> Result<Self, ParseError> {
        Self::from_str(query)
    }
}

impl<'a> TryFrom<&'a String> for Query {
    type Error = ParseError;

    fn try_from(query: &String) -> Result<Self, ParseError> {
        Self::from_str(query)
    }
}

/// A parsed [SPARQL update](https://www.w3.org/TR/sparql11-update/)
///
/// ```
/// use oxigraph::sparql::Update;
///
/// let update_str = "CLEAR ALL ;";
/// let update = Update::parse(update_str, None)?;
///
/// assert_eq!(update.to_string().trim(), update_str);
/// # Result::Ok::<_, oxigraph::sparql::ParseError>(())
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct Update {
    /// The update base IRI
    pub base_iri: Option<Iri<String>>,
    /// The [update operations](https://www.w3.org/TR/sparql11-update/#formalModelGraphUpdate)
    pub operations: Vec<GraphUpdateOperation>,
}

impl Update {
    /// Parses a SPARQL update with an optional base IRI to resolve relative IRIs in the query
    pub fn parse(update: &str, base_iri: Option<&str>) -> Result<Self, ParseError> {
        parse_update(update, base_iri)
    }
}

impl fmt::Display for Update {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(base_iri) = &self.base_iri {
            writeln!(f, "BASE <{}>", base_iri)?;
        }
        for update in &self.operations {
            writeln!(f, "{} ;", update)?;
        }
        Ok(())
    }
}

impl FromStr for Update {
    type Err = ParseError;

    fn from_str(update: &str) -> Result<Self, ParseError> {
        Self::parse(update, None)
    }
}

impl<'a> TryFrom<&'a str> for Update {
    type Error = ParseError;

    fn try_from(update: &str) -> Result<Self, ParseError> {
        Self::from_str(update)
    }
}

impl<'a> TryFrom<&'a String> for Update {
    type Error = ParseError;

    fn try_from(update: &String) -> Result<Self, ParseError> {
        Self::from_str(update)
    }
}

/// The union of [`NamedNode`]s and [`Variable`]s
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum NamedNodeOrVariable {
    NamedNode(NamedNode),
    Variable(Variable),
}

impl fmt::Display for NamedNodeOrVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NamedNodeOrVariable::NamedNode(node) => node.fmt(f),
            NamedNodeOrVariable::Variable(var) => var.fmt(f),
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

/// The union of [`Term`]s and [`Variable`]s
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum TermOrVariable {
    Term(Term),
    Variable(Variable),
}

impl fmt::Display for TermOrVariable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TermOrVariable::Term(term) => term.fmt(f),
            TermOrVariable::Variable(var) => var.fmt(f),
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
        TermOrVariable::Term(node.into())
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
        TermOrVariable::Term(term)
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

/// A [triple pattern](https://www.w3.org/TR/sparql11-query/#defn_TriplePattern)
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct TriplePattern {
    pub subject: TermOrVariable,
    pub predicate: NamedNodeOrVariable,
    pub object: TermOrVariable,
}

impl TriplePattern {
    pub(crate) fn new(
        subject: impl Into<TermOrVariable>,
        predicate: impl Into<NamedNodeOrVariable>,
        object: impl Into<TermOrVariable>,
    ) -> Self {
        Self {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
        }
    }
}

impl fmt::Display for TriplePattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "(triple {} {} {})",
            self.subject, self.predicate, self.object
        )
    }
}

struct SparqlTriplePattern<'a>(&'a TriplePattern);

impl<'a> fmt::Display for SparqlTriplePattern<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} .",
            self.0.subject, self.0.predicate, self.0.object
        )
    }
}

/// A [triple pattern](https://www.w3.org/TR/sparql11-query/#defn_TriplePattern) in a specific graph
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct QuadPattern {
    pub subject: TermOrVariable,
    pub predicate: NamedNodeOrVariable,
    pub object: TermOrVariable,
    pub graph_name: Option<NamedNodeOrVariable>,
}

impl QuadPattern {
    pub(crate) fn new(
        subject: impl Into<TermOrVariable>,
        predicate: impl Into<NamedNodeOrVariable>,
        object: impl Into<TermOrVariable>,
        graph_name: Option<NamedNodeOrVariable>,
    ) -> Self {
        Self {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
            graph_name,
        }
    }
}

impl fmt::Display for QuadPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(graph_name) = &self.graph_name {
            write!(
                f,
                "(graph {} (triple {} {} {}))",
                graph_name, self.subject, self.predicate, self.object
            )
        } else {
            write!(
                f,
                "(triple {} {} {})",
                self.subject, self.predicate, self.object
            )
        }
    }
}

struct SparqlQuadPattern<'a>(&'a QuadPattern);

impl<'a> fmt::Display for SparqlQuadPattern<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(graph_name) = &self.0.graph_name {
            write!(
                f,
                "GRAPH {} {{ {} {} {} }}",
                graph_name, self.0.subject, self.0.predicate, self.0.object
            )
        } else {
            write!(
                f,
                "{} {} {} .",
                self.0.subject, self.0.predicate, self.0.object
            )
        }
    }
}

/// A [property path expression](https://www.w3.org/TR/sparql11-query/#defn_PropertyPathExpr)
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum PropertyPathExpression {
    NamedNode(NamedNode),
    Reverse(Box<PropertyPathExpression>),
    Sequence(Box<PropertyPathExpression>, Box<PropertyPathExpression>),
    Alternative(Box<PropertyPathExpression>, Box<PropertyPathExpression>),
    ZeroOrMore(Box<PropertyPathExpression>),
    OneOrMore(Box<PropertyPathExpression>),
    ZeroOrOne(Box<PropertyPathExpression>),
    NegatedPropertySet(Vec<NamedNode>),
}

impl fmt::Display for PropertyPathExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PropertyPathExpression::NamedNode(p) => p.fmt(f),
            PropertyPathExpression::Reverse(p) => write!(f, "(reverse {})", p),
            PropertyPathExpression::Alternative(a, b) => write!(f, "(alt {} {})", a, b),
            PropertyPathExpression::Sequence(a, b) => write!(f, "(seq {} {})", a, b),
            PropertyPathExpression::ZeroOrMore(p) => write!(f, "(path* {})", p),
            PropertyPathExpression::OneOrMore(p) => write!(f, "(path+ {})", p),
            PropertyPathExpression::ZeroOrOne(p) => write!(f, "(path? {})", p),
            PropertyPathExpression::NegatedPropertySet(p) => {
                write!(f, "(notoneof ")?;
                for p in p {
                    write!(f, " {}", p)?;
                }
                write!(f, ")")
            }
        }
    }
}

struct SparqlPropertyPath<'a>(&'a PropertyPathExpression);

impl<'a> fmt::Display for SparqlPropertyPath<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            PropertyPathExpression::NamedNode(p) => p.fmt(f),
            PropertyPathExpression::Reverse(p) => write!(f, "^{}", SparqlPropertyPath(&*p)),
            PropertyPathExpression::Sequence(a, b) => write!(
                f,
                "({} / {})",
                SparqlPropertyPath(&*a),
                SparqlPropertyPath(&*b)
            ),
            PropertyPathExpression::Alternative(a, b) => write!(
                f,
                "({} | {})",
                SparqlPropertyPath(&*a),
                SparqlPropertyPath(&*b)
            ),
            PropertyPathExpression::ZeroOrMore(p) => write!(f, "{}*", SparqlPropertyPath(&*p)),
            PropertyPathExpression::OneOrMore(p) => write!(f, "{}+", SparqlPropertyPath(&*p)),
            PropertyPathExpression::ZeroOrOne(p) => write!(f, "{}?", SparqlPropertyPath(&*p)),
            PropertyPathExpression::NegatedPropertySet(p) => write!(
                f,
                "!({})",
                p.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(" | ")
            ),
        }
    }
}

impl From<NamedNode> for PropertyPathExpression {
    fn from(p: NamedNode) -> Self {
        PropertyPathExpression::NamedNode(p)
    }
}

/// An [expression](https://www.w3.org/TR/sparql11-query/#expressions)
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum Expression {
    NamedNode(NamedNode),
    Literal(Literal),
    Variable(Variable),
    /// [Logical-or](https://www.w3.org/TR/sparql11-query/#func-logical-or)
    Or(Box<Expression>, Box<Expression>),
    /// [Logical-and](https://www.w3.org/TR/sparql11-query/#func-logical-and)
    And(Box<Expression>, Box<Expression>),
    /// [RDFterm-equal](https://www.w3.org/TR/sparql11-query/#func-RDFterm-equal) and all the XSD equalities
    Equal(Box<Expression>, Box<Expression>),
    /// [sameTerm](https://www.w3.org/TR/sparql11-query/#func-sameTerm)
    SameTerm(Box<Expression>, Box<Expression>),
    /// [op:numeric-greater-than](https://www.w3.org/TR/xpath-functions/#func-numeric-greater-than) and other XSD greater than operators
    Greater(Box<Expression>, Box<Expression>),
    GreaterOrEqual(Box<Expression>, Box<Expression>),
    /// [op:numeric-less-than](https://www.w3.org/TR/xpath-functions/#func-numeric-less-than) and other XSD greater than operators
    Less(Box<Expression>, Box<Expression>),
    LessOrEqual(Box<Expression>, Box<Expression>),
    /// [IN](https://www.w3.org/TR/sparql11-query/#func-in)
    In(Box<Expression>, Vec<Expression>),
    /// [op:numeric-add](https://www.w3.org/TR/xpath-functions/#func-numeric-add) and other XSD additions
    Add(Box<Expression>, Box<Expression>),
    /// [op:numeric-subtract](https://www.w3.org/TR/xpath-functions/#func-numeric-subtract) and other XSD subtractions
    Subtract(Box<Expression>, Box<Expression>),
    /// [op:numeric-multiply](https://www.w3.org/TR/xpath-functions/#func-numeric-multiply) and other XSD multiplications
    Multiply(Box<Expression>, Box<Expression>),
    /// [op:numeric-divide](https://www.w3.org/TR/xpath-functions/#func-numeric-divide) and other XSD divides
    Divide(Box<Expression>, Box<Expression>),
    /// [op:numeric-unary-plus](https://www.w3.org/TR/xpath-functions/#func-numeric-unary-plus) and other XSD unary plus
    UnaryPlus(Box<Expression>),
    /// [op:numeric-unary-minus](https://www.w3.org/TR/xpath-functions/#func-numeric-unary-minus) and other XSD unary minus
    UnaryMinus(Box<Expression>),
    /// [fn:not](https://www.w3.org/TR/xpath-functions/#func-not)
    Not(Box<Expression>),
    /// [EXISTS](https://www.w3.org/TR/sparql11-query/#func-filter-exists)
    Exists(Box<GraphPattern>),
    /// [BOUND](https://www.w3.org/TR/sparql11-query/#func-bound)
    Bound(Variable),
    /// [IF](https://www.w3.org/TR/sparql11-query/#func-if)
    If(Box<Expression>, Box<Expression>, Box<Expression>),
    /// [COALESCE](https://www.w3.org/TR/sparql11-query/#func-coalesce)
    Coalesce(Vec<Expression>),
    /// A regular function call
    FunctionCall(Function, Vec<Expression>),
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expression::NamedNode(node) => node.fmt(f),
            Expression::Literal(l) => l.fmt(f),
            Expression::Variable(var) => var.fmt(f),
            Expression::Or(a, b) => write!(f, "(|| {} {})", a, b),
            Expression::And(a, b) => write!(f, "(&& {} {})", a, b),
            Expression::Equal(a, b) => write!(f, "(= {} {})", a, b),
            Expression::SameTerm(a, b) => write!(f, "(sameTerm {} {})", a, b),
            Expression::Greater(a, b) => write!(f, "(> {} {})", a, b),
            Expression::GreaterOrEqual(a, b) => write!(f, "(>= {} {})", a, b),
            Expression::Less(a, b) => write!(f, "(< {} {})", a, b),
            Expression::LessOrEqual(a, b) => write!(f, "(<= {} {})", a, b),
            Expression::In(a, b) => {
                write!(f, "(in {}", a)?;
                for p in b {
                    write!(f, " {}", p)?;
                }
                write!(f, ")")
            }
            Expression::Add(a, b) => write!(f, "(+ {} {})", a, b),
            Expression::Subtract(a, b) => write!(f, "(- {} {})", a, b),
            Expression::Multiply(a, b) => write!(f, "(* {} {})", a, b),
            Expression::Divide(a, b) => write!(f, "(/ {} {})", a, b),
            Expression::UnaryPlus(e) => write!(f, "(+ {})", e),
            Expression::UnaryMinus(e) => write!(f, "(- {})", e),
            Expression::Not(e) => write!(f, "(! {})", e),
            Expression::FunctionCall(function, parameters) => {
                write!(f, "({}", function)?;
                for p in parameters {
                    write!(f, " {}", p)?;
                }
                write!(f, ")")
            }
            Expression::Exists(p) => write!(f, "(exists {})", p),
            Expression::Bound(v) => write!(f, "(bound {})", v),
            Expression::If(a, b, c) => write!(f, "(if {} {} {})", a, b, c),
            Expression::Coalesce(parameters) => {
                write!(f, "(coalesce")?;
                for p in parameters {
                    write!(f, " {}", p)?;
                }
                write!(f, ")")
            }
        }
    }
}

impl From<NamedNode> for Expression {
    fn from(p: NamedNode) -> Self {
        Expression::NamedNode(p)
    }
}

impl From<Literal> for Expression {
    fn from(p: Literal) -> Self {
        Expression::Literal(p)
    }
}

impl From<Variable> for Expression {
    fn from(v: Variable) -> Self {
        Expression::Variable(v)
    }
}

struct SparqlExpression<'a>(&'a Expression);

impl<'a> fmt::Display for SparqlExpression<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Expression::NamedNode(node) => node.fmt(f),
            Expression::Literal(l) => l.fmt(f),
            Expression::Variable(var) => var.fmt(f),
            Expression::Or(a, b) => write!(
                f,
                "({} || {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::And(a, b) => write!(
                f,
                "({} && {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::Equal(a, b) => {
                write!(f, "({} = {})", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::SameTerm(a, b) => {
                write!(
                    f,
                    "sameTerm({}, {})",
                    SparqlExpression(&*a),
                    SparqlExpression(&*b)
                )
            }
            Expression::Greater(a, b) => {
                write!(f, "({} > {})", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::GreaterOrEqual(a, b) => write!(
                f,
                "({} >= {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::Less(a, b) => {
                write!(f, "({} < {})", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::LessOrEqual(a, b) => write!(
                f,
                "({} <= {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::In(a, b) => {
                write!(f, "({} IN ", SparqlExpression(&*a))?;
                write_arg_list(b.iter().map(|p| SparqlExpression(&*p)), f)?;
                write!(f, ")")
            }
            Expression::Add(a, b) => {
                write!(f, "{} + {}", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::Subtract(a, b) => {
                write!(f, "{} - {}", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::Multiply(a, b) => {
                write!(f, "{} * {}", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::Divide(a, b) => {
                write!(f, "{} / {}", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::UnaryPlus(e) => write!(f, "+{}", SparqlExpression(&*e)),
            Expression::UnaryMinus(e) => write!(f, "-{}", SparqlExpression(&*e)),
            Expression::Not(e) => match e.as_ref() {
                Expression::Exists(p) => write!(f, "NOT EXISTS {{ {} }}", SparqlGraphPattern(&*p)),
                e => write!(f, "!{}", SparqlExpression(&*e)),
            },
            Expression::FunctionCall(function, parameters) => {
                write!(f, "{}", function)?;
                write_arg_list(parameters.iter().map(|p| SparqlExpression(&*p)), f)
            }
            Expression::Bound(v) => write!(f, "BOUND({})", v),
            Expression::Exists(p) => write!(f, "EXISTS {{ {} }}", SparqlGraphPattern(&*p)),
            Expression::If(a, b, c) => write!(
                f,
                "IF({}, {}, {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b),
                SparqlExpression(&*c)
            ),
            Expression::Coalesce(parameters) => {
                write!(f, "COALESCE")?;
                write_arg_list(parameters.iter().map(|p| SparqlExpression(&*p)), f)
            }
        }
    }
}

fn write_arg_list(
    params: impl IntoIterator<Item = impl fmt::Display>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    write!(f, "(")?;
    let mut cont = false;
    for p in params {
        if cont {
            write!(f, ", ")?;
        }
        p.fmt(f)?;
        cont = true;
    }
    write!(f, ")")
}

/// A function name
#[allow(clippy::upper_case_acronyms)] //TODO: Fix on the next breaking release
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum Function {
    Str,
    Lang,
    LangMatches,
    Datatype,
    IRI,
    BNode,
    Rand,
    Abs,
    Ceil,
    Floor,
    Round,
    Concat,
    SubStr,
    StrLen,
    Replace,
    UCase,
    LCase,
    EncodeForURI,
    Contains,
    StrStarts,
    StrEnds,
    StrBefore,
    StrAfter,
    Year,
    Month,
    Day,
    Hours,
    Minutes,
    Seconds,
    Timezone,
    Tz,
    Now,
    UUID,
    StrUUID,
    MD5,
    SHA1,
    SHA256,
    SHA384,
    SHA512,
    StrLang,
    StrDT,
    IsIRI,
    IsBlank,
    IsLiteral,
    IsNumeric,
    Regex,
    Custom(NamedNode),
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Function::Str => write!(f, "STR"),
            Function::Lang => write!(f, "LANG"),
            Function::LangMatches => write!(f, "LANGMATCHES"),
            Function::Datatype => write!(f, "DATATYPE"),
            Function::IRI => write!(f, "IRI"),
            Function::BNode => write!(f, "BNODE"),
            Function::Rand => write!(f, "RAND"),
            Function::Abs => write!(f, "ABS"),
            Function::Ceil => write!(f, "CEIL"),
            Function::Floor => write!(f, "FLOOR"),
            Function::Round => write!(f, "ROUND"),
            Function::Concat => write!(f, "CONCAT"),
            Function::SubStr => write!(f, "SUBSTR"),
            Function::StrLen => write!(f, "STRLEN"),
            Function::Replace => write!(f, "REPLACE"),
            Function::UCase => write!(f, "UCASE"),
            Function::LCase => write!(f, "LCASE"),
            Function::EncodeForURI => write!(f, "ENCODE_FOR_URI"),
            Function::Contains => write!(f, "CONTAINS"),
            Function::StrStarts => write!(f, "STRSTATS"),
            Function::StrEnds => write!(f, "STRENDS"),
            Function::StrBefore => write!(f, "STRBEFORE"),
            Function::StrAfter => write!(f, "STRAFTER"),
            Function::Year => write!(f, "YEAR"),
            Function::Month => write!(f, "MONTH"),
            Function::Day => write!(f, "DAY"),
            Function::Hours => write!(f, "HOURS"),
            Function::Minutes => write!(f, "MINUTES"),
            Function::Seconds => write!(f, "SECONDS"),
            Function::Timezone => write!(f, "TIMEZONE"),
            Function::Tz => write!(f, "TZ"),
            Function::Now => write!(f, "NOW"),
            Function::UUID => write!(f, "UUID"),
            Function::StrUUID => write!(f, "STRUUID"),
            Function::MD5 => write!(f, "MD5"),
            Function::SHA1 => write!(f, "SHA1"),
            Function::SHA256 => write!(f, "SHA256"),
            Function::SHA384 => write!(f, "SHA384"),
            Function::SHA512 => write!(f, "SHA512"),
            Function::StrLang => write!(f, "STRLANG"),
            Function::StrDT => write!(f, "STRDT"),
            Function::IsIRI => write!(f, "isIRI"),
            Function::IsBlank => write!(f, "isBLANK"),
            Function::IsLiteral => write!(f, "isLITERAL"),
            Function::IsNumeric => write!(f, "isNUMERIC"),
            Function::Regex => write!(f, "REGEX"),
            Function::Custom(iri) => iri.fmt(f),
        }
    }
}

/// A SPARQL query [graph pattern](https://www.w3.org/TR/sparql11-query/#sparqlQuery)
#[allow(clippy::upper_case_acronyms)] //TODO: Fix on the next breaking release
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GraphPattern {
    /// A [basic graph pattern](https://www.w3.org/TR/sparql11-query/#defn_BasicGraphPattern)
    BGP(Vec<TriplePattern>),
    /// A [property path pattern](https://www.w3.org/TR/sparql11-query/#defn_evalPP_predicate)
    Path {
        subject: TermOrVariable,
        path: PropertyPathExpression,
        object: TermOrVariable,
    },
    /// [Join](https://www.w3.org/TR/sparql11-query/#defn_algJoin)
    Join {
        left: Box<GraphPattern>,
        right: Box<GraphPattern>,
    },
    /// [LeftJoin](https://www.w3.org/TR/sparql11-query/#defn_algLeftJoin)
    LeftJoin {
        left: Box<GraphPattern>,
        right: Box<GraphPattern>,
        expr: Option<Expression>,
    },
    /// [Filter](https://www.w3.org/TR/sparql11-query/#defn_algFilter)
    Filter {
        expr: Expression,
        inner: Box<GraphPattern>,
    },
    /// [Union](https://www.w3.org/TR/sparql11-query/#defn_algUnion)
    Union {
        left: Box<GraphPattern>,
        right: Box<GraphPattern>,
    },
    Graph {
        graph_name: NamedNodeOrVariable,
        inner: Box<GraphPattern>,
    },
    /// [Extend](https://www.w3.org/TR/sparql11-query/#defn_extend)
    Extend {
        inner: Box<GraphPattern>,
        var: Variable,
        expr: Expression,
    },
    /// [Minus](https://www.w3.org/TR/sparql11-query/#defn_algMinus)
    Minus {
        left: Box<GraphPattern>,
        right: Box<GraphPattern>,
    },
    /// A table used to provide inline values
    Table {
        variables: Vec<Variable>,
        rows: Vec<Vec<Option<Term>>>,
    },
    /// [OrderBy](https://www.w3.org/TR/sparql11-query/#defn_algOrdered)
    OrderBy {
        inner: Box<GraphPattern>,
        condition: Vec<OrderComparator>,
    },
    /// [Project](https://www.w3.org/TR/sparql11-query/#defn_algProjection)
    Project {
        inner: Box<GraphPattern>,
        projection: Vec<Variable>,
    },
    /// [Distinct](https://www.w3.org/TR/sparql11-query/#defn_algDistinct)
    Distinct { inner: Box<GraphPattern> },
    /// [Reduced](https://www.w3.org/TR/sparql11-query/#defn_algReduced)
    Reduced { inner: Box<GraphPattern> },
    /// [Slice](https://www.w3.org/TR/sparql11-query/#defn_algSlice)
    Slice {
        inner: Box<GraphPattern>,
        start: usize,
        length: Option<usize>,
    },
    /// [Group](https://www.w3.org/TR/sparql11-federated-query/#aggregateAlgebra)
    Group {
        inner: Box<GraphPattern>,
        by: Vec<Variable>,
        aggregates: Vec<(Variable, AggregationFunction)>,
    },
    /// [Service](https://www.w3.org/TR/sparql11-federated-query/#defn_evalService)
    Service {
        name: NamedNodeOrVariable,
        pattern: Box<GraphPattern>,
        silent: bool,
    },
}

impl fmt::Display for GraphPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphPattern::BGP(p) => {
                write!(f, "(bgp")?;
                for pattern in p {
                    write!(f, " {}", pattern)?;
                }
                write!(f, ")")
            }
            GraphPattern::Path {
                subject,
                path,
                object,
            } => write!(f, "(path {} {} {})", subject, path, object),
            GraphPattern::Join { left, right } => write!(f, "(join {} {})", left, right),
            GraphPattern::LeftJoin { left, right, expr } => {
                if let Some(expr) = expr {
                    write!(f, "(leftjoin {} {} {})", left, right, expr)
                } else {
                    write!(f, "(leftjoin {} {})", left, right)
                }
            }
            GraphPattern::Filter { expr, inner } => write!(f, "(filter {} {})", expr, inner),
            GraphPattern::Union { left, right } => write!(f, "(union {} {})", left, right),
            GraphPattern::Graph { graph_name, inner } => {
                write!(f, "(graph {} {})", graph_name, inner)
            }
            GraphPattern::Extend { inner, var, expr } => {
                write!(f, "(extend ({} {}) {})", var, expr, inner)
            }
            GraphPattern::Minus { left, right } => write!(f, "(minus {} {})", left, right),
            GraphPattern::Service {
                name,
                pattern,
                silent,
            } => {
                if *silent {
                    write!(f, "(service silent {} {})", name, pattern)
                } else {
                    write!(f, "(service {} {})", name, pattern)
                }
            }
            GraphPattern::Group {
                inner,
                by,
                aggregates,
            } => write!(
                f,
                "(group ({}) ({}) {})",
                by.iter()
                    .map(|v| v.as_str())
                    .collect::<Vec<&str>>()
                    .join(" "),
                aggregates
                    .iter()
                    .map(|(a, v)| format!("({} {})", v, a))
                    .collect::<Vec<String>>()
                    .join(" "),
                inner
            ),
            GraphPattern::Table { variables, rows } => {
                write!(f, "(table (vars")?;
                for var in variables {
                    write!(f, " {}", var)?;
                }
                write!(f, ")")?;
                for row in rows {
                    write!(f, " (row")?;
                    for (value, var) in row.iter().zip(variables) {
                        if let Some(value) = value {
                            write!(f, " ({} {})", var, value)?;
                        }
                    }
                    write!(f, ")")?;
                }
                write!(f, ")")
            }
            GraphPattern::OrderBy { inner, condition } => write!(
                f,
                "(order ({}) {})",
                condition
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<String>>()
                    .join(" "),
                inner
            ),
            GraphPattern::Project { inner, projection } => write!(
                f,
                "(project ({}) {})",
                projection
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(" "),
                inner
            ),
            GraphPattern::Distinct { inner } => write!(f, "(distinct {})", inner),
            GraphPattern::Reduced { inner } => write!(f, "(reduced {})", inner),
            GraphPattern::Slice {
                inner,
                start,
                length,
            } => write!(
                f,
                "(slice {} {} {})",
                start,
                length
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| '_'.to_string()),
                inner
            ),
        }
    }
}

impl Default for GraphPattern {
    fn default() -> Self {
        GraphPattern::BGP(Vec::default())
    }
}

impl GraphPattern {
    pub fn visible_variables(&self) -> BTreeSet<&Variable> {
        let mut vars = BTreeSet::default();
        self.add_visible_variables(&mut vars);
        vars
    }

    fn add_visible_variables<'a>(&'a self, vars: &mut BTreeSet<&'a Variable>) {
        match self {
            GraphPattern::BGP(p) => {
                for pattern in p {
                    if let TermOrVariable::Variable(s) = &pattern.subject {
                        vars.insert(s);
                    }
                    if let NamedNodeOrVariable::Variable(p) = &pattern.predicate {
                        vars.insert(p);
                    }
                    if let TermOrVariable::Variable(o) = &pattern.object {
                        vars.insert(o);
                    }
                }
            }
            GraphPattern::Path {
                subject, object, ..
            } => {
                if let TermOrVariable::Variable(s) = subject {
                    vars.insert(s);
                }
                if let TermOrVariable::Variable(o) = object {
                    vars.insert(o);
                }
            }
            GraphPattern::Join { left, right }
            | GraphPattern::LeftJoin { left, right, .. }
            | GraphPattern::Union { left, right } => {
                left.add_visible_variables(vars);
                right.add_visible_variables(vars);
            }
            GraphPattern::Filter { inner, .. } => inner.add_visible_variables(vars),
            GraphPattern::Graph { graph_name, inner } => {
                if let NamedNodeOrVariable::Variable(ref g) = graph_name {
                    vars.insert(g);
                }
                inner.add_visible_variables(vars);
            }
            GraphPattern::Extend { inner, var, .. } => {
                vars.insert(var);
                inner.add_visible_variables(vars);
            }
            GraphPattern::Minus { left, .. } => left.add_visible_variables(vars),
            GraphPattern::Service { pattern, .. } => pattern.add_visible_variables(vars),
            GraphPattern::Group { by, aggregates, .. } => {
                vars.extend(by);
                for (v, _) in aggregates {
                    vars.insert(v);
                }
            }
            GraphPattern::Table { variables, .. } => vars.extend(variables),
            GraphPattern::Project { projection, .. } => vars.extend(projection.iter()),
            GraphPattern::OrderBy { inner, .. }
            | GraphPattern::Distinct { inner }
            | GraphPattern::Reduced { inner }
            | GraphPattern::Slice { inner, .. } => inner.add_visible_variables(vars),
        }
    }
}

struct SparqlGraphPattern<'a>(&'a GraphPattern);

impl<'a> fmt::Display for SparqlGraphPattern<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            GraphPattern::BGP(p) => {
                for pattern in p {
                    write!(f, "{}", SparqlTriplePattern(pattern))?
                }
                Ok(())
            }
            GraphPattern::Path {
                subject,
                path,
                object,
            } => write!(f, "{} {} {} .", subject, SparqlPropertyPath(path), object),
            GraphPattern::Join { left, right } => write!(
                f,
                "{} {}",
                SparqlGraphPattern(&*left),
                SparqlGraphPattern(&*right)
            ),
            GraphPattern::LeftJoin { left, right, expr } => {
                if let Some(expr) = expr {
                    write!(
                        f,
                        "{} OPTIONAL {{ {} FILTER({}) }}",
                        SparqlGraphPattern(&*left),
                        SparqlGraphPattern(&*right),
                        SparqlExpression(expr)
                    )
                } else {
                    write!(
                        f,
                        "{} OPTIONAL {{ {} }}",
                        SparqlGraphPattern(&*left),
                        SparqlGraphPattern(&*right)
                    )
                }
            }
            GraphPattern::Filter { expr, inner } => write!(
                f,
                "{} FILTER({})",
                SparqlGraphPattern(&*inner),
                SparqlExpression(expr)
            ),
            GraphPattern::Union { left, right } => write!(
                f,
                "{{ {} }} UNION {{ {} }}",
                SparqlGraphPattern(&*left),
                SparqlGraphPattern(&*right),
            ),
            GraphPattern::Graph { graph_name, inner } => {
                write!(
                    f,
                    "GRAPH {} {{ {} }}",
                    graph_name,
                    SparqlGraphPattern(&*inner)
                )
            }
            GraphPattern::Extend { inner, var, expr } => write!(
                f,
                "{} BIND({} AS {})",
                SparqlGraphPattern(&*inner),
                SparqlExpression(expr),
                var
            ),
            GraphPattern::Minus { left, right } => write!(
                f,
                "{} MINUS {{ {} }}",
                SparqlGraphPattern(&*left),
                SparqlGraphPattern(&*right)
            ),
            GraphPattern::Service {
                name,
                pattern,
                silent,
            } => {
                if *silent {
                    write!(
                        f,
                        "SERVICE SILENT {} {{ {} }}",
                        name,
                        SparqlGraphPattern(&*pattern)
                    )
                } else {
                    write!(
                        f,
                        "SERVICE {} {{ {} }}",
                        name,
                        SparqlGraphPattern(&*pattern)
                    )
                }
            }
            GraphPattern::Table { variables, rows } => {
                write!(f, "VALUES ( ")?;
                for var in variables {
                    write!(f, "{} ", var)?;
                }
                write!(f, ") {{ ")?;
                for row in rows {
                    write!(f, "( ")?;
                    for val in row {
                        match val {
                            Some(val) => write!(f, "{} ", val),
                            None => write!(f, "UNDEF "),
                        }?;
                    }
                    write!(f, ") ")?;
                }
                write!(f, " }}")
            }
            GraphPattern::Group {
                inner,
                by,
                aggregates,
            } => write!(
                f,
                "{{ SELECT {} WHERE {{ {} }} GROUP BY {} }}",
                aggregates
                    .iter()
                    .map(|(v, a)| format!("({} AS {})", SparqlAggregationFunction(a), v))
                    .chain(by.iter().map(|e| e.to_string()))
                    .collect::<Vec<String>>()
                    .join(" "),
                SparqlGraphPattern(&*inner),
                by.iter()
                    .map(|e| format!("({})", e.to_string()))
                    .collect::<Vec<String>>()
                    .join(" ")
            ),
            p => write!(
                f,
                "{{ {} }}",
                SparqlGraphRootPattern {
                    pattern: p,
                    dataset: &QueryDataset::default()
                }
            ),
        }
    }
}

struct SparqlGraphRootPattern<'a> {
    pattern: &'a GraphPattern,
    dataset: &'a QueryDataset,
}

impl<'a> fmt::Display for SparqlGraphRootPattern<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut distinct = false;
        let mut reduced = false;
        let mut order = None;
        let mut start = 0;
        let mut length = None;
        let mut project: &[Variable] = &[];

        let mut child = self.pattern;
        loop {
            match child {
                GraphPattern::OrderBy { inner, condition } => {
                    order = Some(condition);
                    child = &*inner;
                }
                GraphPattern::Project { inner, projection } if project.is_empty() => {
                    project = projection;
                    child = &*inner;
                }
                GraphPattern::Distinct { inner } => {
                    distinct = true;
                    child = &*inner;
                }
                GraphPattern::Reduced { inner } => {
                    reduced = true;
                    child = &*inner;
                }
                GraphPattern::Slice {
                    inner,
                    start: s,
                    length: l,
                } => {
                    start = *s;
                    length = *l;
                    child = inner;
                }
                p => {
                    write!(f, "SELECT ")?;
                    if distinct {
                        write!(f, "DISTINCT ")?;
                    }
                    if reduced {
                        write!(f, "REDUCED ")?;
                    }
                    write!(
                        f,
                        "{}{} WHERE {{ {} }}",
                        build_sparql_select_arguments(project),
                        self.dataset,
                        SparqlGraphPattern(p)
                    )?;
                    if let Some(order) = order {
                        write!(
                            f,
                            " ORDER BY {}",
                            order
                                .iter()
                                .map(|c| SparqlOrderComparator(c).to_string())
                                .collect::<Vec<String>>()
                                .join(" ")
                        )?;
                    }
                    if start > 0 {
                        write!(f, " OFFSET {}", start)?;
                    }
                    if let Some(length) = length {
                        write!(f, " LIMIT {}", length)?;
                    }
                    return Ok(());
                }
            }
        }
    }
}

fn build_sparql_select_arguments(args: &[Variable]) -> String {
    if args.is_empty() {
        "*".to_owned()
    } else {
        args.iter()
            .map(|v| v.to_string())
            .collect::<Vec<String>>()
            .join(" ")
    }
}

/// A set function used in aggregates (c.f. [`GraphPattern::Group`])
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum AggregationFunction {
    /// [Count](https://www.w3.org/TR/sparql11-query/#defn_aggCount)
    Count {
        expr: Option<Box<Expression>>,
        distinct: bool,
    },
    /// [Sum](https://www.w3.org/TR/sparql11-query/#defn_aggSum)
    Sum {
        expr: Box<Expression>,
        distinct: bool,
    },
    /// [Avg](https://www.w3.org/TR/sparql11-query/#defn_aggAvg)
    Avg {
        expr: Box<Expression>,
        distinct: bool,
    },
    /// [Min](https://www.w3.org/TR/sparql11-query/#defn_aggMin)
    Min {
        expr: Box<Expression>,
        distinct: bool,
    },
    /// [Max](https://www.w3.org/TR/sparql11-query/#defn_aggMax)
    Max {
        expr: Box<Expression>,
        distinct: bool,
    },
    /// [GroupConcat](https://www.w3.org/TR/sparql11-query/#defn_aggGroupConcat)
    GroupConcat {
        expr: Box<Expression>,
        distinct: bool,
        separator: Option<String>,
    },
    /// [Sample](https://www.w3.org/TR/sparql11-query/#defn_aggSample)
    Sample {
        expr: Box<Expression>,
        distinct: bool,
    },
    /// Custom function
    Custom {
        name: NamedNode,
        expr: Box<Expression>,
        distinct: bool,
    },
}

impl fmt::Display for AggregationFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AggregationFunction::Count { expr, distinct } => {
                if *distinct {
                    if let Some(expr) = expr {
                        write!(f, "(count distinct {})", expr)
                    } else {
                        write!(f, "(count distinct)")
                    }
                } else if let Some(expr) = expr {
                    write!(f, "(count {})", expr)
                } else {
                    write!(f, "(count)")
                }
            }
            AggregationFunction::Sum { expr, distinct } => {
                if *distinct {
                    write!(f, "(sum distinct {})", expr)
                } else {
                    write!(f, "(sum {})", expr)
                }
            }
            AggregationFunction::Avg { expr, distinct } => {
                if *distinct {
                    write!(f, "(avg distinct {})", expr)
                } else {
                    write!(f, "(avg {})", expr)
                }
            }
            AggregationFunction::Min { expr, distinct } => {
                if *distinct {
                    write!(f, "(min distinct {})", expr)
                } else {
                    write!(f, "(min {})", expr)
                }
            }
            AggregationFunction::Max { expr, distinct } => {
                if *distinct {
                    write!(f, "(max distinct {})", expr)
                } else {
                    write!(f, "(max {})", expr)
                }
            }
            AggregationFunction::Sample { expr, distinct } => {
                if *distinct {
                    write!(f, "(sample distinct {})", expr)
                } else {
                    write!(f, "(sample {})", expr)
                }
            }
            AggregationFunction::GroupConcat {
                expr,
                distinct,
                separator,
            } => {
                if *distinct {
                    if let Some(separator) = separator {
                        write!(f, "(group_concat distinct {} {})", expr, fmt_str(separator))
                    } else {
                        write!(f, "(group_concat distinct {})", expr)
                    }
                } else if let Some(separator) = separator {
                    write!(f, "(group_concat {} {})", expr, fmt_str(separator))
                } else {
                    write!(f, "(group_concat {})", expr)
                }
            }
            AggregationFunction::Custom {
                name,
                expr,
                distinct,
            } => {
                if *distinct {
                    write!(f, "({} distinct {})", name, expr)
                } else {
                    write!(f, "({} {})", name, expr)
                }
            }
        }
    }
}

struct SparqlAggregationFunction<'a>(&'a AggregationFunction);

impl<'a> fmt::Display for SparqlAggregationFunction<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            AggregationFunction::Count { expr, distinct } => {
                if *distinct {
                    if let Some(expr) = expr {
                        write!(f, "COUNT(DISTINCT {})", SparqlExpression(expr))
                    } else {
                        write!(f, "COUNT(DISTINCT *)")
                    }
                } else if let Some(expr) = expr {
                    write!(f, "COUNT({})", SparqlExpression(expr))
                } else {
                    write!(f, "COUNT(*)")
                }
            }
            AggregationFunction::Sum { expr, distinct } => {
                if *distinct {
                    write!(f, "SUM(DISTINCT {})", SparqlExpression(expr))
                } else {
                    write!(f, "SUM({})", SparqlExpression(expr))
                }
            }
            AggregationFunction::Min { expr, distinct } => {
                if *distinct {
                    write!(f, "MIN(DISTINCT {})", SparqlExpression(expr))
                } else {
                    write!(f, "MIN({})", SparqlExpression(expr))
                }
            }
            AggregationFunction::Max { expr, distinct } => {
                if *distinct {
                    write!(f, "MAX(DISTINCT {})", SparqlExpression(expr))
                } else {
                    write!(f, "MAX({})", SparqlExpression(expr))
                }
            }
            AggregationFunction::Avg { expr, distinct } => {
                if *distinct {
                    write!(f, "AVG(DISTINCT {})", SparqlExpression(expr))
                } else {
                    write!(f, "AVG({})", SparqlExpression(expr))
                }
            }
            AggregationFunction::Sample { expr, distinct } => {
                if *distinct {
                    write!(f, "SAMPLE(DISTINCT {})", SparqlExpression(expr))
                } else {
                    write!(f, "SAMPLE({})", SparqlExpression(expr))
                }
            }
            AggregationFunction::GroupConcat {
                expr,
                distinct,
                separator,
            } => {
                if *distinct {
                    if let Some(separator) = separator {
                        write!(
                            f,
                            "GROUP_CONCAT(DISTINCT {}; SEPARATOR = {})",
                            SparqlExpression(expr),
                            fmt_str(separator)
                        )
                    } else {
                        write!(f, "GROUP_CONCAT(DISTINCT {})", SparqlExpression(expr))
                    }
                } else if let Some(separator) = separator {
                    write!(
                        f,
                        "GROUP_CONCAT({}; SEPARATOR = {})",
                        SparqlExpression(expr),
                        fmt_str(separator)
                    )
                } else {
                    write!(f, "GROUP_CONCAT({})", SparqlExpression(expr))
                }
            }
            AggregationFunction::Custom {
                name,
                expr,
                distinct,
            } => {
                if *distinct {
                    write!(f, "{}(DISTINCT {})", name, SparqlExpression(expr))
                } else {
                    write!(f, "{}({})", name, SparqlExpression(expr))
                }
            }
        }
    }
}

fn fmt_str(value: &str) -> rio::Literal<'_> {
    rio::Literal::Simple { value }
}

/// An ordering comparator used by [`GraphPattern::OrderBy`]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum OrderComparator {
    /// Ascending order
    Asc(Expression),
    /// Descending order
    Desc(Expression),
}

impl fmt::Display for OrderComparator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderComparator::Asc(e) => write!(f, "(asc {})", e),
            OrderComparator::Desc(e) => write!(f, "(desc {})", e),
        }
    }
}

struct SparqlOrderComparator<'a>(&'a OrderComparator);

impl<'a> fmt::Display for SparqlOrderComparator<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            OrderComparator::Asc(e) => write!(f, "ASC({})", SparqlExpression(e)),
            OrderComparator::Desc(e) => write!(f, "DESC({})", SparqlExpression(e)),
        }
    }
}

/// A SPARQL query [dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset)
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct QueryDataset {
    default: Option<Vec<GraphName>>,
    named: Option<Vec<NamedOrBlankNode>>,
}

impl Default for QueryDataset {
    fn default() -> Self {
        Self {
            default: Some(vec![GraphName::DefaultGraph]),
            named: None,
        }
    }
}

impl QueryDataset {
    /// Checks if this dataset specification is the default one
    /// (i.e. the default graph is the store default graph and all the store named graphs are available)
    ///
    /// ```
    /// use oxigraph::sparql::Query;
    ///
    /// assert!(Query::parse("SELECT ?s ?p ?o WHERE { ?s ?p ?o . }", None)?.dataset().is_default_dataset());
    /// assert!(!Query::parse("SELECT ?s ?p ?o FROM <http://example.com> WHERE { ?s ?p ?o . }", None)?.dataset().is_default_dataset());
    ///
    /// # Result::Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn is_default_dataset(&self) -> bool {
        self.default
            .as_ref()
            .map_or(false, |t| t == &[GraphName::DefaultGraph])
            && self.named.is_none()
    }

    /// Returns the list of the store graphs that are available to the query as the default graph or `None` if the union of all graphs is used as the default graph
    /// This list is by default only the store default graph
    pub fn default_graph_graphs(&self) -> Option<&[GraphName]> {
        self.default.as_deref()
    }

    /// Sets if the default graph for the query should be the union of all the graphs in the queried store
    pub fn set_default_graph_as_union(&mut self) {
        self.default = None;
    }

    /// Sets the list of graphs the query should consider as being part of the default graph.
    ///
    /// By default only the store default graph is considered.
    /// ```
    /// use oxigraph::model::NamedNode;
    /// use oxigraph::sparql::Query;
    ///
    /// let mut query = Query::parse("SELECT ?s ?p ?o WHERE { ?s ?p ?o . }", None)?;
    /// query.dataset_mut().set_default_graph(vec![NamedNode::new("http://example.com")?.into()]);
    /// assert_eq!(query.to_string(), "SELECT ?s ?p ?o FROM <http://example.com> WHERE { ?s ?p ?o . }");
    ///
    /// # Result::Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn set_default_graph(&mut self, graphs: Vec<GraphName>) {
        self.default = Some(graphs)
    }

    /// Returns the list of the available named graphs for the query or `None` if all graphs are available
    pub fn available_named_graphs(&self) -> Option<&[NamedOrBlankNode]> {
        self.named.as_deref()
    }

    /// Sets the list of allowed named graphs in the query.
    ///
    /// ```
    /// use oxigraph::model::NamedNode;
    /// use oxigraph::sparql::Query;
    ///
    /// let mut query = Query::parse("SELECT ?s ?p ?o WHERE { ?s ?p ?o . }", None)?;
    /// query.dataset_mut().set_available_named_graphs(vec![NamedNode::new("http://example.com")?.into()]);
    /// assert_eq!(query.to_string(), "SELECT ?s ?p ?o FROM NAMED <http://example.com> WHERE { ?s ?p ?o . }");
    ///
    /// # Result::Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn set_available_named_graphs(&mut self, named_graphs: Vec<NamedOrBlankNode>) {
        self.named = Some(named_graphs);
    }
}

impl fmt::Display for QueryDataset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        //TODO: does not encode everything
        if let Some(graphs) = &self.default {
            for g in graphs {
                if !g.is_default_graph() {
                    write!(f, " FROM {}", g)?;
                }
            }
        }
        if let Some(graphs) = &self.named {
            for g in graphs {
                write!(f, " FROM NAMED {}", g)?;
            }
        }
        Ok(())
    }
}

/// The [graph update operations](https://www.w3.org/TR/sparql11-update/#formalModelGraphUpdate)
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GraphUpdateOperation {
    /// [insert data](https://www.w3.org/TR/sparql11-update/#def_insertdataoperation)
    InsertData { data: Vec<Quad> },
    /// [delete data](https://www.w3.org/TR/sparql11-update/#def_deletedataoperation)
    DeleteData { data: Vec<Quad> },
    /// [delete insert](https://www.w3.org/TR/sparql11-update/#def_deleteinsertoperation)
    DeleteInsert {
        delete: Vec<QuadPattern>,
        insert: Vec<QuadPattern>,
        using: QueryDataset,
        pattern: Box<GraphPattern>,
    },
    /// [load](https://www.w3.org/TR/sparql11-update/#def_loadoperation)
    Load {
        silent: bool,
        from: NamedNode,
        to: Option<NamedNode>,
    },
    /// [clear](https://www.w3.org/TR/sparql11-update/#def_clearoperation)
    Clear { silent: bool, graph: GraphTarget },
    /// [create](https://www.w3.org/TR/sparql11-update/#def_createoperation)
    Create { silent: bool, graph: NamedNode },
    /// [drop](https://www.w3.org/TR/sparql11-update/#def_dropoperation)
    Drop { silent: bool, graph: GraphTarget },
}

impl fmt::Display for GraphUpdateOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphUpdateOperation::InsertData { data } => {
                writeln!(f, "INSERT DATA {{")?;
                write_quads(data, f)?;
                write!(f, "}}")
            }
            GraphUpdateOperation::DeleteData { data } => {
                writeln!(f, "DELETE DATA {{")?;
                write_quads(data, f)?;
                write!(f, "}}")
            }
            GraphUpdateOperation::DeleteInsert {
                delete,
                insert,
                using,
                pattern,
            } => {
                if !delete.is_empty() {
                    writeln!(f, "DELETE {{")?;
                    for quad in delete {
                        writeln!(f, "\t{}", SparqlQuadPattern(quad))?;
                    }
                    writeln!(f, "}}")?;
                }
                if !insert.is_empty() {
                    writeln!(f, "INSERT {{")?;
                    for quad in insert {
                        writeln!(f, "\t{}", SparqlQuadPattern(quad))?;
                    }
                    writeln!(f, "}}")?;
                }
                if let Some(using_default) = using.default_graph_graphs() {
                    for g in using_default {
                        if !g.is_default_graph() {
                            writeln!(f, "USING {}", g)?;
                        }
                    }
                }
                if let Some(using_named) = using.available_named_graphs() {
                    for g in using_named {
                        writeln!(f, "USING NAMED {}", g)?;
                    }
                }
                write!(
                    f,
                    "WHERE {{ {} }}",
                    SparqlGraphRootPattern {
                        pattern,
                        dataset: &QueryDataset::default()
                    }
                )
            }
            GraphUpdateOperation::Load { silent, from, to } => {
                write!(f, "LOAD ")?;
                if *silent {
                    write!(f, "SILENT ")?;
                }
                write!(f, "{}", from)?;
                if let Some(to) = to {
                    write!(f, " INTO GRAPH {}", to)?;
                }
                Ok(())
            }
            GraphUpdateOperation::Clear { silent, graph } => {
                write!(f, "CLEAR ")?;
                if *silent {
                    write!(f, "SILENT ")?;
                }
                write!(f, "{}", graph)
            }
            GraphUpdateOperation::Create { silent, graph } => {
                write!(f, "CREATE ")?;
                if *silent {
                    write!(f, "SILENT ")?;
                }
                write!(f, "GRAPH {}", graph)
            }
            GraphUpdateOperation::Drop { silent, graph } => {
                write!(f, "DROP ")?;
                if *silent {
                    write!(f, "SILENT ")?;
                }
                write!(f, "{}", graph)
            }
        }
    }
}

fn write_quads(quads: &[Quad], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    for quad in quads {
        if quad.graph_name == GraphName::DefaultGraph {
            writeln!(f, "\t{} {} {} .", quad.subject, quad.predicate, quad.object)?;
        } else {
            writeln!(
                f,
                "\tGRAPH {} {{ {} {} {} }}",
                quad.graph_name, quad.subject, quad.predicate, quad.object
            )?;
        }
    }
    Ok(())
}

/// A target RDF graph for update operations
///
/// Could be a specific graph, all named graphs or the complete dataset.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GraphTarget {
    NamedNode(NamedNode),
    DefaultGraph,
    NamedGraphs,
    AllGraphs,
}

impl fmt::Display for GraphTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => write!(f, "GRAPH {}", node),
            Self::DefaultGraph => write!(f, "DEFAULT"),
            Self::NamedGraphs => write!(f, "NAMED"),
            Self::AllGraphs => write!(f, "ALL"),
        }
    }
}

impl From<NamedNode> for GraphTarget {
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}
