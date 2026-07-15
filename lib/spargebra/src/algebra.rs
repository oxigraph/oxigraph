//! [SPARQL 1.1 Query Algebra](https://www.w3.org/TR/sparql11-query/#sparqlQuery) representation.

use crate::term::*;
use crate::vocab::sparql;
use oxrdf::OxString;
use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Write as _;

/// A [property path expression](https://www.w3.org/TR/sparql11-query/#defn_PropertyPathExpr).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum PropertyPathExpression {
    NamedNode(NamedNode),
    Reverse(Box<Self>),
    Sequence(Box<Self>, Box<Self>),
    Alternative(Box<Self>, Box<Self>),
    ZeroOrMore(Box<Self>),
    OneOrMore(Box<Self>),
    ZeroOrOne(Box<Self>),
    NegatedPropertySet(Vec<NamedNode>),
}

impl PropertyPathExpression {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::NamedNode(p) => write!(f, "{p}"),
            Self::Reverse(p) => {
                f.write_str("(reverse ")?;
                p.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Alternative(a, b) => {
                f.write_str("(alt ")?;
                a.fmt_sse(f)?;
                f.write_str(" ")?;
                b.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Sequence(a, b) => {
                f.write_str("(seq ")?;
                a.fmt_sse(f)?;
                f.write_str(" ")?;
                b.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::ZeroOrMore(p) => {
                f.write_str("(path* ")?;
                p.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::OneOrMore(p) => {
                f.write_str("(path+ ")?;
                p.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::ZeroOrOne(p) => {
                f.write_str("(path? ")?;
                p.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::NegatedPropertySet(p) => {
                f.write_str("(notoneof")?;
                for p in p {
                    write!(f, " {p}")?;
                }
                f.write_str(")")
            }
        }
    }
}

impl fmt::Display for PropertyPathExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(p) => p.fmt(f),
            Self::Reverse(p) => write!(f, "^({p})"),
            Self::Sequence(a, b) => write!(f, "({a} / {b})"),
            Self::Alternative(a, b) => write!(f, "({a} | {b})"),
            Self::ZeroOrMore(p) => write!(f, "({p})*"),
            Self::OneOrMore(p) => write!(f, "({p})+"),
            Self::ZeroOrOne(p) => write!(f, "({p})?"),
            Self::NegatedPropertySet(p) => {
                f.write_str("!(")?;
                for (i, c) in p.iter().enumerate() {
                    if i > 0 {
                        f.write_str(" | ")?;
                    }
                    write!(f, "{c}")?;
                }
                f.write_str(")")
            }
        }
    }
}

impl From<NamedNode> for PropertyPathExpression {
    fn from(p: NamedNode) -> Self {
        Self::NamedNode(p)
    }
}

/// An [expression](https://www.w3.org/TR/sparql11-query/#expressions).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum Expression {
    NamedNode(NamedNode),
    Literal(Literal),
    Variable(Variable),
    /// [Logical-or](https://www.w3.org/TR/sparql11-query/#func-logical-or).
    Or(Box<Self>, Box<Self>),
    /// [Logical-and](https://www.w3.org/TR/sparql11-query/#func-logical-and).
    And(Box<Self>, Box<Self>),
    /// [RDFterm-equal](https://www.w3.org/TR/sparql11-query/#func-RDFterm-equal) and all the XSD equalities.
    Equal(Box<Self>, Box<Self>),
    /// [sameTerm](https://www.w3.org/TR/sparql11-query/#func-sameTerm).
    SameTerm(Box<Self>, Box<Self>),
    /// [op:numeric-greater-than](https://www.w3.org/TR/xpath-functions-31/#func-numeric-greater-than) and other XSD greater than operators.
    Greater(Box<Self>, Box<Self>),
    GreaterOrEqual(Box<Self>, Box<Self>),
    /// [op:numeric-less-than](https://www.w3.org/TR/xpath-functions-31/#func-numeric-less-than) and other XSD greater than operators.
    Less(Box<Self>, Box<Self>),
    LessOrEqual(Box<Self>, Box<Self>),
    /// [IN](https://www.w3.org/TR/sparql11-query/#func-in)
    In(Box<Self>, Vec<Self>),
    /// [op:numeric-add](https://www.w3.org/TR/xpath-functions-31/#func-numeric-add) and other XSD additions.
    Add(Box<Self>, Box<Self>),
    /// [op:numeric-subtract](https://www.w3.org/TR/xpath-functions-31/#func-numeric-subtract) and other XSD subtractions.
    Subtract(Box<Self>, Box<Self>),
    /// [op:numeric-multiply](https://www.w3.org/TR/xpath-functions-31/#func-numeric-multiply) and other XSD multiplications.
    Multiply(Box<Self>, Box<Self>),
    /// [op:numeric-divide](https://www.w3.org/TR/xpath-functions-31/#func-numeric-divide) and other XSD divides.
    Divide(Box<Self>, Box<Self>),
    /// [op:numeric-unary-plus](https://www.w3.org/TR/xpath-functions-31/#func-numeric-unary-plus) and other XSD unary plus.
    UnaryPlus(Box<Self>),
    /// [op:numeric-unary-minus](https://www.w3.org/TR/xpath-functions-31/#func-numeric-unary-minus) and other XSD unary minus.
    UnaryMinus(Box<Self>),
    /// [fn:not](https://www.w3.org/TR/xpath-functions-31/#func-not).
    Not(Box<Self>),
    /// [EXISTS](https://www.w3.org/TR/sparql11-query/#func-filter-exists).
    Exists(Box<GraphPattern>),
    /// [BOUND](https://www.w3.org/TR/sparql11-query/#func-bound).
    Bound(Variable),
    /// [IF](https://www.w3.org/TR/sparql11-query/#func-if).
    If(Box<Self>, Box<Self>, Box<Self>),
    /// [COALESCE](https://www.w3.org/TR/sparql11-query/#func-coalesce).
    Coalesce(Vec<Self>),
    /// A regular function call.
    FunctionCall(NamedNode, Vec<Self>),
}

impl Expression {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::NamedNode(node) => write!(f, "{node}"),
            Self::Literal(l) => write!(f, "{l}"),
            Self::Variable(var) => write!(f, "{var}"),
            Self::Or(a, b) => fmt_sse_binary_expression(f, "||", a, b),
            Self::And(a, b) => fmt_sse_binary_expression(f, "&&", a, b),
            Self::Equal(a, b) => fmt_sse_binary_expression(f, "=", a, b),
            Self::SameTerm(a, b) => fmt_sse_binary_expression(f, "sameTerm", a, b),
            Self::Greater(a, b) => fmt_sse_binary_expression(f, ">", a, b),
            Self::GreaterOrEqual(a, b) => fmt_sse_binary_expression(f, ">=", a, b),
            Self::Less(a, b) => fmt_sse_binary_expression(f, "<", a, b),
            Self::LessOrEqual(a, b) => fmt_sse_binary_expression(f, "<=", a, b),
            Self::In(a, b) => {
                f.write_str("(in ")?;
                a.fmt_sse(f)?;
                for p in b {
                    f.write_str(" ")?;
                    p.fmt_sse(f)?;
                }
                f.write_str(")")
            }
            Self::Add(a, b) => fmt_sse_binary_expression(f, "+", a, b),
            Self::Subtract(a, b) => fmt_sse_binary_expression(f, "-", a, b),
            Self::Multiply(a, b) => fmt_sse_binary_expression(f, "*", a, b),
            Self::Divide(a, b) => fmt_sse_binary_expression(f, "/", a, b),
            Self::UnaryPlus(e) => fmt_sse_unary_expression(f, "+", e),
            Self::UnaryMinus(e) => fmt_sse_unary_expression(f, "-", e),
            Self::Not(e) => fmt_sse_unary_expression(f, "!", e),
            Self::FunctionCall(function, parameters) => {
                f.write_str("(")?;
                write!(f, "{function}")?;
                for p in parameters {
                    f.write_str(" ")?;
                    p.fmt_sse(f)?;
                }
                f.write_str(")")
            }
            Self::Exists(p) => {
                f.write_str("(exists ")?;
                p.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Bound(v) => {
                write!(f, "(bound {v})")
            }
            Self::If(a, b, c) => {
                f.write_str("(if ")?;
                a.fmt_sse(f)?;
                f.write_str(" ")?;
                b.fmt_sse(f)?;
                f.write_str(" ")?;
                c.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Coalesce(parameters) => {
                f.write_str("(coalesce")?;
                for p in parameters {
                    f.write_str(" ")?;
                    p.fmt_sse(f)?;
                }
                f.write_str(")")
            }
        }
    }

    fn walk<'a>(&'a self, callback: &mut impl FnMut(&'a Self)) {
        callback(self);
        match self {
            Self::Variable(_)
            | Self::Bound(_)
            | Self::NamedNode(_)
            | Self::Literal(_)
            | Self::Exists(_) => (),
            Self::UnaryPlus(i) | Self::UnaryMinus(i) | Self::Not(i) => i.walk(callback),
            Self::Or(l, r)
            | Self::And(l, r)
            | Self::Equal(l, r)
            | Self::SameTerm(l, r)
            | Self::Greater(l, r)
            | Self::GreaterOrEqual(l, r)
            | Self::Less(l, r)
            | Self::LessOrEqual(l, r)
            | Self::Add(l, r)
            | Self::Subtract(l, r)
            | Self::Multiply(l, r)
            | Self::Divide(l, r) => {
                l.walk(callback);
                r.walk(callback);
            }
            Self::If(c, l, r) => {
                c.walk(callback);
                l.walk(callback);
                r.walk(callback);
            }
            Self::Coalesce(l) | Self::FunctionCall(_, l) => {
                for e in l {
                    e.walk(callback);
                }
            }
            Self::In(l, r) => {
                l.walk(callback);
                for e in r {
                    e.walk(callback);
                }
            }
        }
    }

    fn lookup_used_variables<'a>(&'a self, callback: &mut impl FnMut(&'a Variable)) {
        self.walk(&mut |e| {
            if let Self::Variable(v) | Self::Bound(v) = e {
                callback(v)
            } else if let Self::Exists(p) = e {
                p.lookup_used_variables(callback)
            }
        })
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::Literal(l) => l.fmt(f),
            Self::Variable(var) => var.fmt(f),
            Self::Or(a, b) => write!(f, "({a} || {b})"),
            Self::And(a, b) => write!(f, "({a} && {b})"),
            Self::Equal(a, b) => {
                write!(f, "({a} = {b})")
            }
            Self::SameTerm(a, b) => {
                write!(f, "sameTerm({a}, {b})")
            }
            Self::Greater(a, b) => {
                write!(f, "({a} > {b})")
            }
            Self::GreaterOrEqual(a, b) => write!(f, "({a} >= {b})"),
            Self::Less(a, b) => {
                write!(f, "({a} < {b})")
            }
            Self::LessOrEqual(a, b) => write!(f, "({a} <= {b})"),
            Self::In(a, b) => {
                write!(f, "({a} IN ")?;
                write_arg_list(b, f)?;
                f.write_str(")")
            }
            Self::Add(a, b) => {
                write!(f, "({a} + {b})")
            }
            Self::Subtract(a, b) => {
                write!(f, "({a} - {b})")
            }
            Self::Multiply(a, b) => {
                write!(f, "({a} * {b})")
            }
            Self::Divide(a, b) => {
                write!(f, "({a} / {b})")
            }
            Self::UnaryPlus(e) => write!(f, "+({e})"),
            Self::UnaryMinus(e) => write!(f, "-({e})"),
            Self::Not(e) => match &**e {
                Self::Exists(p) => write!(f, "NOT EXISTS {{ {p} }}"),
                _ => write!(f, "!({e})"),
            },
            Self::FunctionCall(function, parameters) => {
                if let Some(name) = function_name(function) {
                    f.write_str(name)?;
                } else {
                    write!(f, "{function}")?;
                }
                write_arg_list(parameters, f)
            }
            Self::Bound(v) => write!(f, "BOUND({v})"),
            Self::Exists(p) => write!(f, "EXISTS {{ {p} }}"),
            Self::If(a, b, c) => write!(f, "IF({a}, {b}, {c})"),
            Self::Coalesce(parameters) => {
                f.write_str("COALESCE")?;
                write_arg_list(parameters, f)
            }
        }
    }
}

impl From<NamedNode> for Expression {
    fn from(p: NamedNode) -> Self {
        Self::NamedNode(p)
    }
}

impl From<Literal> for Expression {
    fn from(p: Literal) -> Self {
        Self::Literal(p)
    }
}

impl From<Variable> for Expression {
    fn from(v: Variable) -> Self {
        Self::Variable(v)
    }
}

impl From<NamedNodePattern> for Expression {
    fn from(p: NamedNodePattern) -> Self {
        match p {
            NamedNodePattern::NamedNode(p) => p.into(),
            NamedNodePattern::Variable(p) => p.into(),
        }
    }
}

fn write_arg_list(
    params: impl IntoIterator<Item = impl fmt::Display>,
    f: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    f.write_str("(")?;
    let mut cont = false;
    for p in params {
        if cont {
            f.write_str(", ")?;
        }
        p.fmt(f)?;
        cont = true;
    }
    f.write_str(")")
}

fn function_name(function: &NamedNode) -> Option<&'static str> {
    Some(match function.as_str() {
        "http://www.w3.org/ns/sparql#str" => "STR",
        "http://www.w3.org/ns/sparql#lang" => "LANG",
        "http://www.w3.org/ns/sparql#langMatches" => "LANGMATCHES",
        "http://www.w3.org/ns/sparql#datatype" => "DATATYPE",
        "http://www.w3.org/ns/sparql#iri" => "IRI",
        "http://www.w3.org/ns/sparql#uri" => "URI",
        "http://www.w3.org/ns/sparql#bnode" => "BNODE",
        "http://www.w3.org/ns/sparql#rand" => "RAND",
        "http://www.w3.org/ns/sparql#abs" => "ABS",
        "http://www.w3.org/ns/sparql#ceil" => "CEIL",
        "http://www.w3.org/ns/sparql#floor" => "FLOOR",
        "http://www.w3.org/ns/sparql#round" => "ROUND",
        "http://www.w3.org/ns/sparql#concat" => "CONCAT",
        "http://www.w3.org/ns/sparql#substr" => "SUBSTR",
        "http://www.w3.org/ns/sparql#strlen" => "STRLEN",
        "http://www.w3.org/ns/sparql#replace" => "REPLACE",
        "http://www.w3.org/ns/sparql#ucase" => "UCASE",
        "http://www.w3.org/ns/sparql#lcase" => "LCASE",
        "http://www.w3.org/ns/sparql#encode" => "ENCODE_FOR_URI",
        "http://www.w3.org/ns/sparql#contains" => "CONTAINS",
        "http://www.w3.org/ns/sparql#strstarts" => "STRSTARTS",
        "http://www.w3.org/ns/sparql#strends" => "STRENDS",
        "http://www.w3.org/ns/sparql#strbefore" => "STRBEFORE",
        "http://www.w3.org/ns/sparql#strafter" => "STRAFTER",
        "http://www.w3.org/ns/sparql#year" => "YEAR",
        "http://www.w3.org/ns/sparql#month" => "MONTH",
        "http://www.w3.org/ns/sparql#day" => "DAY",
        "http://www.w3.org/ns/sparql#hours" => "HOURS",
        "http://www.w3.org/ns/sparql#minutes" => "MINUTES",
        "http://www.w3.org/ns/sparql#seconds" => "SECONDS",
        "http://www.w3.org/ns/sparql#timezone" => "TIMEZONE",
        "http://www.w3.org/ns/sparql#tz" => "TZ",
        "http://www.w3.org/ns/sparql#now" => "NOW",
        "http://www.w3.org/ns/sparql#uuid" => "UUID",
        "http://www.w3.org/ns/sparql#struuid" => "STRUUID",
        "http://www.w3.org/ns/sparql#md5" => "MD5",
        "http://www.w3.org/ns/sparql#sha1" => "SHA1",
        "http://www.w3.org/ns/sparql#sha256" => "SHA256",
        "http://www.w3.org/ns/sparql#sha384" => "SHA384",
        "http://www.w3.org/ns/sparql#sha512" => "SHA512",
        "http://www.w3.org/ns/sparql#strlang" => "STRLANG",
        "http://www.w3.org/ns/sparql#strdt" => "STRDT",
        "http://www.w3.org/ns/sparql#isIRI" => "isIRI",
        "http://www.w3.org/ns/sparql#isURI" => "isURI",
        "http://www.w3.org/ns/sparql#isBlank" => "isBLANK",
        "http://www.w3.org/ns/sparql#isLiteral" => "isLITERAL",
        "http://www.w3.org/ns/sparql#isNumeric" => "isNUMERIC",
        "http://www.w3.org/ns/sparql#regex" => "REGEX",
        #[cfg(feature = "sparql-12")]
        "http://www.w3.org/ns/sparql#triple" => "TRIPLE",
        #[cfg(feature = "sparql-12")]
        "http://www.w3.org/ns/sparql#subject" => "SUBJECT",
        #[cfg(feature = "sparql-12")]
        "http://www.w3.org/ns/sparql#predicate" => "PREDICATE",
        #[cfg(feature = "sparql-12")]
        "http://www.w3.org/ns/sparql#object" => "OBJECT",
        #[cfg(feature = "sparql-12")]
        "http://www.w3.org/ns/sparql#isTriple" => "isTRIPLE",
        #[cfg(feature = "sparql-12")]
        "http://www.w3.org/ns/sparql#langdir" => "LANGDIR",
        #[cfg(feature = "sparql-12")]
        "http://www.w3.org/ns/sparql#hasLang" => "hasLANG",
        #[cfg(feature = "sparql-12")]
        "http://www.w3.org/ns/sparql#hasLangdir" => "hasLANGDIR",
        #[cfg(feature = "sparql-12")]
        "http://www.w3.org/ns/sparql#strlangdir" => "STRLANGDIR",
        #[cfg(feature = "sep-0002")]
        "http://www.w3.org/ns/sparql#adjust" => "ADJUST",
        _ => return None,
    })
}

/// A SPARQL query [graph pattern](https://www.w3.org/TR/sparql11-query/#sparqlQuery).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GraphPattern {
    /// A [basic graph pattern](https://www.w3.org/TR/sparql11-query/#defn_BasicGraphPattern).
    Bgp { patterns: Vec<TriplePattern> },
    /// A [property path pattern](https://www.w3.org/TR/sparql11-query/#defn_evalPP_predicate).
    Path {
        subject: TermPattern,
        path: PropertyPathExpression,
        object: TermPattern,
    },
    /// [Join](https://www.w3.org/TR/sparql11-query/#defn_algJoin).
    Join { left: Box<Self>, right: Box<Self> },
    /// [LeftJoin](https://www.w3.org/TR/sparql11-query/#defn_algLeftJoin).
    LeftJoin {
        left: Box<Self>,
        right: Box<Self>,
        expression: Option<Expression>,
    },
    /// Lateral join i.e. evaluate right for all result row of left
    #[cfg(feature = "sep-0006")]
    Lateral { left: Box<Self>, right: Box<Self> },
    /// [Filter](https://www.w3.org/TR/sparql11-query/#defn_algFilter).
    Filter { expr: Expression, inner: Box<Self> },
    /// [Union](https://www.w3.org/TR/sparql11-query/#defn_algUnion).
    Union { left: Box<Self>, right: Box<Self> },
    Graph {
        name: NamedNodePattern,
        inner: Box<Self>,
    },
    /// [Extend](https://www.w3.org/TR/sparql11-query/#defn_extend).
    Extend {
        inner: Box<Self>,
        variable: Variable,
        expression: Expression,
    },
    /// [Minus](https://www.w3.org/TR/sparql11-query/#defn_algMinus).
    Minus { left: Box<Self>, right: Box<Self> },
    /// A table used to provide inline values
    Values {
        variables: Vec<Variable>,
        bindings: Vec<Vec<Option<GroundTerm>>>,
    },
    /// [OrderBy](https://www.w3.org/TR/sparql11-query/#defn_algOrdered).
    OrderBy {
        inner: Box<Self>,
        expression: Vec<OrderExpression>,
    },
    /// [Project](https://www.w3.org/TR/sparql11-query/#defn_algProjection).
    Project {
        inner: Box<Self>,
        variables: Vec<Variable>,
    },
    /// [Distinct](https://www.w3.org/TR/sparql11-query/#defn_algDistinct).
    Distinct { inner: Box<Self> },
    /// [Reduced](https://www.w3.org/TR/sparql11-query/#defn_algReduced).
    Reduced { inner: Box<Self> },
    /// [Slice](https://www.w3.org/TR/sparql11-query/#defn_algSlice).
    Slice {
        inner: Box<Self>,
        start: u64,
        length: Option<u64>,
    },
    /// [Group](https://www.w3.org/TR/sparql11-query/#aggregateAlgebra).
    Group {
        inner: Box<Self>,
        variables: Vec<Variable>,
        aggregates: Vec<(Variable, AggregateExpression)>,
    },
    /// [Service](https://www.w3.org/TR/sparql11-federated-query/#defn_evalService).
    Service {
        name: NamedNodePattern,
        inner: Box<Self>,
        silent: bool,
    },
}

impl fmt::Display for GraphPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bgp { patterns } => {
                for pattern in patterns {
                    write!(f, "{pattern} .")?
                }
                Ok(())
            }
            Self::Path {
                subject,
                path,
                object,
            } => write!(f, "{subject} {path} {object} ."),
            Self::Join { left, right } => {
                if matches!(left.as_ref(), Self::Filter { .. }) {
                    write!(f, "{{ {left} }}") // We put brackets to avoid the filter to be pushed at the root
                } else {
                    left.fmt(f)
                }?;
                f.write_char(' ')?;
                if matches!(
                    right.as_ref(),
                    Self::Bgp { .. }
                        | Self::Path { .. }
                        | Self::Values { .. }
                        | Self::Service { .. }
                        | Self::Graph { .. }
                        | Self::Union { .. }
                        | Self::Group { .. }
                        | Self::Project { .. }
                        | Self::Distinct { .. }
                        | Self::Reduced { .. }
                        | Self::OrderBy { .. }
                        | Self::Slice { .. }
                ) {
                    right.fmt(f)
                } else {
                    write!(f, "{{ {right} }}") // No brackets, the reconstruction will be wrong
                }
            }
            Self::LeftJoin {
                left,
                right,
                expression,
            } => {
                if matches!(left.as_ref(), Self::Filter { .. }) {
                    write!(f, "{{ {left} }}") // We put brackets to avoid the filter to be pushed at the root
                } else {
                    left.fmt(f)
                }?;
                f.write_str(" OPTIONAL { ")?;
                if matches!(right.as_ref(), Self::Filter { .. }) {
                    write!(f, "{{ {right} }}") // We put brackets to avoid the filter to be considered as the left join filter
                } else {
                    right.fmt(f)
                }?;
                if let Some(expression) = expression {
                    write!(f, " FILTER({expression})")?;
                }
                f.write_str(" }")
            }
            #[cfg(feature = "sep-0006")]
            Self::Lateral { left, right } => {
                if matches!(left.as_ref(), Self::Filter { .. }) {
                    write!(f, "{{ {left} }}") // We put brackets to avoid the filter to be pushed at the root
                } else {
                    left.fmt(f)
                }?;
                write!(f, " LATERAL {{ {right} }}")
            }
            Self::Filter { expr, inner } => {
                if matches!(inner.as_ref(), Self::Filter { .. }) {
                    write!(f, "{{ {inner} }}") // We put brackets to avoid the filter to be pushed at the root
                } else {
                    inner.fmt(f)
                }?;
                write!(f, " FILTER({expr})")
            }
            Self::Union { left, right } => write!(f, "{{ {left} }} UNION {{ {right} }}"),
            Self::Graph { name, inner } => {
                write!(f, "GRAPH {name} {{ {inner} }}")
            }
            Self::Extend {
                inner,
                variable,
                expression,
            } => {
                if matches!(inner.as_ref(), Self::Filter { .. }) {
                    write!(f, "{{ {inner} }}") // We put brackets to avoid the filter to be pushed at the root
                } else {
                    inner.fmt(f)
                }?;
                write!(f, " BIND({expression} AS {variable})")
            }
            Self::Minus { left, right } => {
                if matches!(left.as_ref(), Self::Filter { .. }) {
                    write!(f, "{{ {left} }}") // We put brackets to avoid the filter to be pushed at the root
                } else {
                    left.fmt(f)
                }?;
                write!(f, " MINUS {{ {right} }}")
            }
            Self::Service {
                name,
                inner,
                silent,
            } => {
                if *silent {
                    write!(f, "SERVICE SILENT {name} {{ {inner} }}")
                } else {
                    write!(f, "SERVICE {name} {{ {inner} }}")
                }
            }
            Self::Values {
                variables,
                bindings,
            } => {
                f.write_str("VALUES ( ")?;
                for var in variables {
                    write!(f, "{var} ")?;
                }
                f.write_str(") { ")?;
                for row in bindings {
                    f.write_str("( ")?;
                    for val in row {
                        match val {
                            Some(val) => write!(f, "{val} "),
                            None => f.write_str("UNDEF "),
                        }?;
                    }
                    f.write_str(") ")?;
                }
                f.write_str(" }")
            }
            Self::Group {
                inner,
                variables,
                aggregates,
            } => {
                f.write_str("{ SELECT")?;
                for (a, v) in aggregates {
                    write!(f, " ({v} AS {a})")?;
                }
                for v in variables {
                    write!(f, " {v}")?;
                }
                write!(f, " WHERE {{ {inner} }}")?;
                if !variables.is_empty() {
                    f.write_str(" GROUP BY")?;
                    for v in variables {
                        write!(f, " {v}")?;
                    }
                }
                f.write_str(" }")
            }
            p => write!(f, "{{ {} }}", SparqlGraphRootPattern::new(p, None)?),
        }
    }
}

impl Default for GraphPattern {
    fn default() -> Self {
        Self::Bgp {
            patterns: Vec::new(),
        }
    }
}

impl GraphPattern {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::Bgp { patterns } => {
                f.write_str("(bgp")?;
                for pattern in patterns {
                    f.write_str(" ")?;
                    pattern.fmt_sse(f)?;
                }
                f.write_str(")")
            }
            Self::Path {
                subject,
                path,
                object,
            } => {
                f.write_str("(path ")?;
                subject.fmt_sse(f)?;
                f.write_str(" ")?;
                path.fmt_sse(f)?;
                f.write_str(" ")?;
                object.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Join { left, right } => {
                f.write_str("(join ")?;
                left.fmt_sse(f)?;
                f.write_str(" ")?;
                right.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::LeftJoin {
                left,
                right,
                expression,
            } => {
                f.write_str("(leftjoin ")?;
                left.fmt_sse(f)?;
                f.write_str(" ")?;
                right.fmt_sse(f)?;
                if let Some(expr) = expression {
                    f.write_str(" ")?;
                    expr.fmt_sse(f)?;
                }
                f.write_str(")")
            }
            #[cfg(feature = "sep-0006")]
            Self::Lateral { left, right } => {
                f.write_str("(lateral ")?;
                left.fmt_sse(f)?;
                f.write_str(" ")?;
                right.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Filter { expr, inner } => {
                f.write_str("(filter ")?;
                expr.fmt_sse(f)?;
                f.write_str(" ")?;
                inner.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Union { left, right } => {
                f.write_str("(union ")?;
                left.fmt_sse(f)?;
                f.write_str(" ")?;
                right.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Graph { name, inner } => {
                f.write_str("(graph ")?;
                name.fmt_sse(f)?;
                f.write_str(" ")?;
                inner.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Extend {
                inner,
                variable,
                expression,
            } => {
                write!(f, "(extend (({variable} ")?;
                expression.fmt_sse(f)?;
                f.write_str(")) ")?;
                inner.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Minus { left, right } => {
                f.write_str("(minus ")?;
                left.fmt_sse(f)?;
                f.write_str(" ")?;
                right.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Service {
                name,
                inner,
                silent,
            } => {
                f.write_str("(service ")?;
                if *silent {
                    f.write_str("silent ")?;
                }
                name.fmt_sse(f)?;
                f.write_str(" ")?;
                inner.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Group {
                inner,
                variables,
                aggregates,
            } => {
                f.write_str("(group (")?;
                for (i, v) in variables.iter().enumerate() {
                    if i > 0 {
                        f.write_str(" ")?;
                    }
                    write!(f, "{v}")?;
                }
                f.write_str(") (")?;
                for (i, (v, a)) in aggregates.iter().enumerate() {
                    if i > 0 {
                        f.write_str(" ")?;
                    }
                    f.write_str("(")?;
                    a.fmt_sse(f)?;
                    write!(f, " {v})")?;
                }
                f.write_str(") ")?;
                inner.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Values {
                variables,
                bindings,
            } => {
                f.write_str("(table (vars")?;
                for var in variables {
                    write!(f, " {var}")?;
                }
                f.write_str(")")?;
                for row in bindings {
                    f.write_str(" (row")?;
                    for (value, var) in row.iter().zip(variables) {
                        if let Some(value) = value {
                            write!(f, " ({var} {value})")?;
                        }
                    }
                    f.write_str(")")?;
                }
                f.write_str(")")
            }
            Self::OrderBy { inner, expression } => {
                f.write_str("(order (")?;
                for (i, c) in expression.iter().enumerate() {
                    if i > 0 {
                        f.write_str(" ")?;
                    }
                    c.fmt_sse(f)?;
                }
                f.write_str(") ")?;
                inner.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Project { inner, variables } => {
                f.write_str("(project (")?;
                for (i, v) in variables.iter().enumerate() {
                    if i > 0 {
                        f.write_str(" ")?;
                    }
                    write!(f, "{v}")?;
                }
                f.write_str(") ")?;
                inner.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Distinct { inner } => {
                f.write_str("(distinct ")?;
                inner.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Reduced { inner } => {
                f.write_str("(reduced ")?;
                inner.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Slice {
                inner,
                start,
                length,
            } => {
                if let Some(length) = length {
                    write!(f, "(slice {start} {length} ")?;
                } else {
                    write!(f, "(slice {start} _ ")?;
                }
                inner.fmt_sse(f)?;
                f.write_str(")")
            }
        }
    }

    /// Calls `callback` on each [in-scope variable](https://www.w3.org/TR/sparql11-query/#variableScope) occurrence.
    pub fn on_in_scope_variable<'a>(&'a self, mut callback: impl FnMut(&'a Variable)) {
        self.lookup_in_scope_variables(&mut callback)
    }

    fn lookup_in_scope_variables<'a>(&'a self, callback: &mut impl FnMut(&'a Variable)) {
        match self {
            Self::Bgp { patterns } => {
                for pattern in patterns {
                    lookup_triple_pattern_variables(pattern, callback)
                }
            }
            Self::Path {
                subject, object, ..
            } => {
                if let TermPattern::Variable(s) = subject {
                    callback(s);
                }
                #[cfg(feature = "sparql-12")]
                if let TermPattern::Triple(s) = subject {
                    lookup_triple_pattern_variables(s, callback)
                }
                if let TermPattern::Variable(o) = object {
                    callback(o);
                }
                #[cfg(feature = "sparql-12")]
                if let TermPattern::Triple(o) = object {
                    lookup_triple_pattern_variables(o, callback)
                }
            }
            Self::Join { left, right }
            | Self::LeftJoin { left, right, .. }
            | Self::Union { left, right } => {
                left.lookup_in_scope_variables(callback);
                right.lookup_in_scope_variables(callback);
            }
            #[cfg(feature = "sep-0006")]
            Self::Lateral { left, right } => {
                left.lookup_in_scope_variables(callback);
                right.lookup_in_scope_variables(callback);
            }
            Self::Graph { name, inner } => {
                if let NamedNodePattern::Variable(g) = &name {
                    callback(g);
                }
                inner.lookup_in_scope_variables(callback);
            }
            Self::Extend {
                inner, variable, ..
            } => {
                callback(variable);
                inner.lookup_in_scope_variables(callback);
            }
            Self::Minus { left, .. } => left.lookup_in_scope_variables(callback),
            Self::Group {
                variables,
                aggregates,
                ..
            } => {
                for v in variables {
                    callback(v);
                }
                for (v, _) in aggregates {
                    callback(v);
                }
            }
            Self::Values { variables, .. } | Self::Project { variables, .. } => {
                for v in variables {
                    callback(v);
                }
            }
            Self::Service { inner, .. }
            | Self::Filter { inner, .. }
            | Self::OrderBy { inner, .. }
            | Self::Distinct { inner }
            | Self::Reduced { inner }
            | Self::Slice { inner, .. } => inner.lookup_in_scope_variables(callback),
        }
    }

    fn lookup_used_variables<'a>(&'a self, callback: &mut impl FnMut(&'a Variable)) {
        match self {
            Self::Bgp { patterns } => {
                for pattern in patterns {
                    lookup_triple_pattern_variables(pattern, callback)
                }
            }
            Self::Path {
                subject, object, ..
            } => {
                if let TermPattern::Variable(s) = subject {
                    callback(s);
                }
                #[cfg(feature = "sparql-12")]
                if let TermPattern::Triple(s) = subject {
                    lookup_triple_pattern_variables(s, callback)
                }
                if let TermPattern::Variable(o) = object {
                    callback(o);
                }
                #[cfg(feature = "sparql-12")]
                if let TermPattern::Triple(o) = object {
                    lookup_triple_pattern_variables(o, callback)
                }
            }
            Self::Join { left, right }
            | Self::Minus { left, right }
            | Self::Union { left, right } => {
                left.lookup_used_variables(callback);
                right.lookup_used_variables(callback);
            }
            Self::LeftJoin {
                left,
                right,
                expression,
            } => {
                left.lookup_used_variables(callback);
                right.lookup_used_variables(callback);
                if let Some(expr) = expression {
                    expr.lookup_used_variables(callback);
                }
            }
            #[cfg(feature = "sep-0006")]
            Self::Lateral { left, right } => {
                left.lookup_used_variables(callback);
                right.lookup_used_variables(callback);
            }
            Self::Graph { name, inner } => {
                if let NamedNodePattern::Variable(g) = &name {
                    callback(g);
                }
                inner.lookup_used_variables(callback);
            }
            Self::Extend {
                inner,
                variable,
                expression,
            } => {
                callback(variable);
                expression.lookup_used_variables(callback);
                inner.lookup_used_variables(callback);
            }
            Self::Group {
                inner,
                variables,
                aggregates,
            } => {
                inner.lookup_used_variables(callback);
                for v in variables {
                    callback(v);
                }
                for (v, expr) in aggregates {
                    callback(v);
                    expr.lookup_used_variables(callback);
                }
            }
            Self::Values { variables, .. } | Self::Project { variables, .. } => {
                for v in variables {
                    callback(v);
                }
            }
            Self::Filter { inner, expr } => {
                expr.lookup_used_variables(callback);
                inner.lookup_used_variables(callback);
            }
            Self::Service { inner, name, .. } => {
                if let NamedNodePattern::Variable(s) = &name {
                    callback(s);
                }
                inner.lookup_used_variables(callback);
            }
            Self::OrderBy { inner, expression } => {
                for e in expression {
                    e.lookup_used_variables(callback);
                }
                inner.lookup_used_variables(callback);
            }
            Self::Distinct { inner } | Self::Reduced { inner } | Self::Slice { inner, .. } => {
                inner.lookup_used_variables(callback)
            }
        }
    }
}

fn lookup_triple_pattern_variables<'a>(
    pattern: &'a TriplePattern,
    callback: &mut impl FnMut(&'a Variable),
) {
    if let TermPattern::Variable(s) = &pattern.subject {
        callback(s);
    }
    #[cfg(feature = "sparql-12")]
    if let TermPattern::Triple(s) = &pattern.subject {
        lookup_triple_pattern_variables(s, callback)
    }
    if let NamedNodePattern::Variable(p) = &pattern.predicate {
        callback(p);
    }
    if let TermPattern::Variable(o) = &pattern.object {
        callback(o);
    }
    #[cfg(feature = "sparql-12")]
    if let TermPattern::Triple(o) = &pattern.object {
        lookup_triple_pattern_variables(o, callback)
    }
}

pub(crate) struct SparqlGraphRootPattern<'a> {
    option: SelectionOption,
    project: Option<Vec<(&'a Variable, Option<ExpressionOrAggregate<'a>>)>>,
    pattern: &'a GraphPattern,
    dataset: Option<&'a QueryDataset>,
    group_by: &'a [Variable],
    order: &'a [OrderExpression],
    start: u64,
    length: Option<u64>,
}

impl<'a> SparqlGraphRootPattern<'a> {
    pub fn new(
        mut pattern: &'a GraphPattern,
        dataset: Option<&'a QueryDataset>,
    ) -> Result<Self, fmt::Error> {
        let mut option = SelectionOption::Default;
        let mut start = 0;
        let mut length = None;
        let mut group_by = [].as_slice();

        // Before project
        loop {
            match pattern {
                GraphPattern::Distinct { inner } if option == SelectionOption::Default => {
                    option = SelectionOption::Distinct;
                    pattern = inner;
                }
                GraphPattern::Reduced { inner } if option == SelectionOption::Default => {
                    option = SelectionOption::Reduced;
                    pattern = inner;
                }
                GraphPattern::Slice {
                    inner,
                    start: s,
                    length: l,
                } if start == 0 && length.is_none() => {
                    start = *s;
                    length = *l;
                    pattern = inner;
                }
                _ => break,
            }
        }
        let (project, order) = if let GraphPattern::Project { inner, variables } = pattern {
            // We have the projection
            let mut project = variables.iter().map(|v| (v, None)).collect::<Vec<_>>();
            pattern = inner;

            // we collect extends
            while let GraphPattern::Extend {
                inner,
                expression,
                variable,
            } = pattern
            {
                if !project.iter().any(|(v, _)| *v == variable)
                    || project.iter().any(|(_, expr)| {
                        expr.as_ref().is_some_and(|expr| {
                            let mut found = false;
                            match expr {
                                ExpressionOrAggregate::Expression(expr) => {
                                    expr.lookup_used_variables(&mut |v| found |= v == variable)
                                }
                                ExpressionOrAggregate::Aggregate(expr) => {
                                    expr.lookup_used_variables(&mut |v| found |= v == variable)
                                }
                            }
                            found
                        })
                    })
                {
                    // This simplification only works if the extended variable is in the projection and not used in another expression of the projection.
                    break;
                }
                project
                    .iter_mut()
                    .find(|(v, _)| *v == variable)
                    .ok_or(fmt::Error)?
                    .1 = Some(ExpressionOrAggregate::Expression(expression));
                pattern = inner
            }

            // Order by
            let order = if let GraphPattern::OrderBy { inner, expression } = pattern {
                pattern = inner;
                expression
            } else {
                [].as_slice()
            };

            // And aggregates
            if let GraphPattern::Group {
                inner,
                variables,
                aggregates,
            } = pattern
            {
                // Currently, we only do this simplification if aggregates are directly projected
                if aggregates.iter().all(|(agg_var, _)| {
                    project.iter().all(|(_, project_expr)| {
                        project_expr
                            .as_ref()
                            .is_none_or(|project_expr| match project_expr {
                                ExpressionOrAggregate::Expression(project_expr) => {
                                    let mut contains_variable = false;
                                    project_expr.lookup_used_variables(&mut |v| {
                                        contains_variable |= v == agg_var
                                    });
                                    !contains_variable
                                        || if let Expression::Variable(project_var) = project_expr {
                                            agg_var == project_var
                                        } else {
                                            false
                                        }
                                }
                                ExpressionOrAggregate::Aggregate(_) => unreachable!(),
                            })
                    })
                }) {
                    for (project_var, project_expr) in &mut project {
                        if let Some((_, agg_expr)) = aggregates.iter().find(|(agg_var, _)| {
                            project_expr.as_ref().map_or_else(
                                || agg_var == *project_var,
                                |project_expr| {
                                    if let ExpressionOrAggregate::Expression(
                                        Expression::Variable(project_var),
                                    ) = project_expr
                                    {
                                        agg_var == project_var
                                    } else {
                                        false
                                    }
                                },
                            )
                        }) {
                            *project_expr = Some(ExpressionOrAggregate::Aggregate(agg_expr));
                        }
                    }
                    group_by = variables.as_slice();
                    pattern = inner;
                }
            }
            (Some(project), order)
        } else if let GraphPattern::OrderBy { inner, expression } = pattern {
            pattern = inner;
            (None, expression.as_slice())
        } else {
            (None, [].as_slice())
        };
        Ok(Self {
            option,
            project,
            pattern,
            dataset,
            group_by,
            order,
            start,
            length,
        })
    }
}

impl fmt::Display for SparqlGraphRootPattern<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SELECT")?;
        match self.option {
            SelectionOption::Default => (),
            SelectionOption::Distinct => f.write_str(" DISTINCT")?,
            SelectionOption::Reduced => f.write_str(" REDUCED")?,
        }
        if let Some(project) = &self.project {
            if project.is_empty() {
                // We make sure there is no in-scope variable, if yes, it's not serializable
                let mut with_in_scope = false;
                self.pattern.on_in_scope_variable(|_| with_in_scope = true);
                if with_in_scope {
                    return Err(fmt::Error);
                }
                f.write_str(" *")?;
            } else {
                for (variable, expr) in project {
                    if let Some(expr) = expr {
                        match expr {
                            ExpressionOrAggregate::Expression(expr) => {
                                write!(f, " ({expr} AS {variable})")
                            }
                            ExpressionOrAggregate::Aggregate(expr) => {
                                write!(f, " ({expr} AS {variable})")
                            }
                        }
                    } else {
                        write!(f, " {variable}")
                    }?;
                }
            }
        } else {
            f.write_str(" *")?;
        }
        if let Some(dataset) = self.dataset {
            write!(f, " {dataset}")?;
        }
        write!(f, " WHERE {{ {} }}", self.pattern)?;
        if !self.group_by.is_empty() {
            f.write_str(" GROUP BY")?;
            for v in self.group_by {
                write!(f, " {v}")?;
            }
        }
        if !self.order.is_empty() {
            f.write_str(" ORDER BY")?;
            for c in self.order {
                write!(f, " {c}")?;
            }
        }
        if self.start > 0 {
            write!(f, " OFFSET {}", self.start)?;
        }
        if let Some(length) = self.length {
            write!(f, " LIMIT {length}")?;
        }
        Ok(())
    }
}

#[derive(Eq, PartialEq)]
enum SelectionOption {
    Default,
    Distinct,
    Reduced,
}

enum ExpressionOrAggregate<'a> {
    Expression(&'a Expression),
    Aggregate(&'a AggregateExpression),
}

/// A set function used in aggregates (c.f. [`GraphPattern::Group`]).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum AggregateExpression {
    /// [Count](https://www.w3.org/TR/sparql11-query/#defn_aggCount) with *.
    CountSolutions { distinct: bool },
    FunctionCall {
        name: NamedNode,
        expr: Expression,
        distinct: bool,
        /// Optional parameters to the aggregate. Currently only "separator" for GROUP_CONCAT is used.
        scalarvals: BTreeMap<OxString, OxString>,
    },
}

impl AggregateExpression {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::CountSolutions { distinct } => {
                f.write_str("(count")?;
                if *distinct {
                    f.write_str(" distinct")?;
                }
                f.write_str(")")
            }
            Self::FunctionCall {
                name,
                expr,
                distinct,
                scalarvals,
            } => {
                f.write_str("(")?;
                write!(f, "{name} ")?;
                if *distinct {
                    f.write_str("distinct ")?;
                }
                expr.fmt_sse(f)?;
                for v in scalarvals.values() {
                    write!(f, " {}", Literal::new_simple_literal(v.clone()))?;
                }
                f.write_str(")")
            }
        }
    }

    fn lookup_used_variables<'a>(&'a self, callback: &mut impl FnMut(&'a Variable)) {
        if let Self::FunctionCall { expr, .. } = self {
            expr.lookup_used_variables(callback);
        }
    }
}

impl fmt::Display for AggregateExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CountSolutions { distinct } => {
                if *distinct {
                    f.write_str("COUNT(DISTINCT *)")
                } else {
                    f.write_str("COUNT(*)")
                }
            }
            Self::FunctionCall {
                name,
                expr,
                distinct,
                scalarvals,
            } => {
                if *name == sparql::AGG_COUNT {
                    f.write_str("COUNT")
                } else if *name == sparql::AGG_SUM {
                    f.write_str("SUM")
                } else if *name == sparql::AGG_AVG {
                    f.write_str("AVG")
                } else if *name == sparql::AGG_MIN {
                    f.write_str("MIN")
                } else if *name == sparql::AGG_MAX {
                    f.write_str("MAX")
                } else if *name == sparql::AGG_GROUP_CONCAT {
                    f.write_str("GROUP_CONCAT")
                } else if *name == sparql::AGG_SAMPLE {
                    f.write_str("SAMPLE")
                } else {
                    name.fmt(f)
                }?;
                f.write_char('(')?;
                if *distinct {
                    f.write_str("DISTINCT ")?;
                }
                expr.fmt(f)?;
                for (k, v) in scalarvals {
                    write!(
                        f,
                        "; {} = {}",
                        k.to_uppercase(),
                        Literal::new_simple_literal(v.clone())
                    )?;
                }
                f.write_char(')')
            }
        }
    }
}

/// An ordering comparator used by [`GraphPattern::OrderBy`].
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum OrderExpression {
    /// Ascending order
    Asc(Expression),
    /// Descending order
    Desc(Expression),
}

impl OrderExpression {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::Asc(e) => {
                f.write_str("(asc ")?;
                e.fmt_sse(f)?;
                f.write_str(")")
            }
            Self::Desc(e) => {
                f.write_str("(desc ")?;
                e.fmt_sse(f)?;
                f.write_str(")")
            }
        }
    }

    fn lookup_used_variables<'a>(&'a self, callback: &mut impl FnMut(&'a Variable)) {
        let (Self::Asc(e) | Self::Desc(e)) = self;
        e.lookup_used_variables(callback);
    }
}

impl fmt::Display for OrderExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asc(e) => write!(f, "ASC({e})"),
            Self::Desc(e) => write!(f, "DESC({e})"),
        }
    }
}

/// A SPARQL query [dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct QueryDataset {
    pub default: Vec<NamedNode>,
    pub named: Option<Vec<NamedNode>>,
}

impl QueryDataset {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        f.write_str("(")?;
        for (i, graph_name) in self.default.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{graph_name}")?;
        }
        if let Some(named) = &self.named {
            for (i, graph_name) in named.iter().enumerate() {
                if !self.default.is_empty() || i > 0 {
                    f.write_str(" ")?;
                }
                write!(f, "(named {graph_name})")?;
            }
        }
        f.write_str(")")
    }
}

impl fmt::Display for QueryDataset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for g in &self.default {
            write!(f, " FROM {g}")?;
        }
        if let Some(named) = &self.named {
            for g in named {
                write!(f, " FROM NAMED {g}")?;
            }
        }
        Ok(())
    }
}

/// A target RDF graph for update operations.
///
/// Could be a specific graph, all named graphs or the complete dataset.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GraphTarget {
    NamedNode(NamedNode),
    DefaultGraph,
    NamedGraphs,
    AllGraphs,
}

impl GraphTarget {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::NamedNode(node) => write!(f, "{node}"),
            Self::DefaultGraph => f.write_str("default"),
            Self::NamedGraphs => f.write_str("named"),
            Self::AllGraphs => f.write_str("all"),
        }
    }
}

impl fmt::Display for GraphTarget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => write!(f, "GRAPH {node}"),
            Self::DefaultGraph => f.write_str("DEFAULT"),
            Self::NamedGraphs => f.write_str("NAMED"),
            Self::AllGraphs => f.write_str("ALL"),
        }
    }
}

impl From<NamedNode> for GraphTarget {
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

impl From<GraphName> for GraphTarget {
    fn from(graph_name: GraphName) -> Self {
        match graph_name {
            GraphName::NamedNode(node) => Self::NamedNode(node),
            GraphName::DefaultGraph => Self::DefaultGraph,
        }
    }
}

#[inline]
fn fmt_sse_unary_expression(f: &mut impl fmt::Write, name: &str, e: &Expression) -> fmt::Result {
    write!(f, "({name} ")?;
    e.fmt_sse(f)?;
    f.write_str(")")
}

#[inline]
fn fmt_sse_binary_expression(
    f: &mut impl fmt::Write,
    name: &str,
    a: &Expression,
    b: &Expression,
) -> fmt::Result {
    write!(f, "({name} ")?;
    a.fmt_sse(f)?;
    f.write_str(" ")?;
    b.fmt_sse(f)?;
    f.write_str(")")
}
