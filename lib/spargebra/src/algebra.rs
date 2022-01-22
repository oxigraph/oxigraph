//! [SPARQL 1.1 Query Algebra](https://www.w3.org/TR/sparql11-query/#sparqlQuery) representation.

use crate::term::*;
use oxrdf::LiteralRef;
use std::fmt;

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
            Self::NamedNode(p) => write!(f, "{}", p),
            Self::Reverse(p) => {
                write!(f, "(reverse ")?;
                p.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Alternative(a, b) => {
                write!(f, "(alt ")?;
                a.fmt_sse(f)?;
                write!(f, " ")?;
                b.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Sequence(a, b) => {
                write!(f, "(seq ")?;
                a.fmt_sse(f)?;
                write!(f, " ")?;
                b.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::ZeroOrMore(p) => {
                write!(f, "(path* ")?;
                p.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::OneOrMore(p) => {
                write!(f, "(path+ ")?;
                p.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::ZeroOrOne(p) => {
                write!(f, "(path? ")?;
                p.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::NegatedPropertySet(p) => {
                write!(f, "(notoneof")?;
                for p in p {
                    write!(f, " {}", p)?;
                }
                write!(f, ")")
            }
        }
    }
}

impl fmt::Display for PropertyPathExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(p) => p.fmt(f),
            Self::Reverse(p) => write!(f, "^{}", p),
            Self::Sequence(a, b) => write!(f, "({} / {})", a, b),
            Self::Alternative(a, b) => write!(f, "({} | {})", a, b),
            Self::ZeroOrMore(p) => write!(f, "{}*", p),
            Self::OneOrMore(p) => write!(f, "{}+", p),
            Self::ZeroOrOne(p) => write!(f, "{}?", p),
            Self::NegatedPropertySet(p) => {
                write!(f, "!(")?;
                for (i, c) in p.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", c)?;
                }
                write!(f, ")")
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
    /// [op:numeric-greater-than](https://www.w3.org/TR/xpath-functions/#func-numeric-greater-than) and other XSD greater than operators.
    Greater(Box<Self>, Box<Self>),
    GreaterOrEqual(Box<Self>, Box<Self>),
    /// [op:numeric-less-than](https://www.w3.org/TR/xpath-functions/#func-numeric-less-than) and other XSD greater than operators.
    Less(Box<Self>, Box<Self>),
    LessOrEqual(Box<Self>, Box<Self>),
    /// [IN](https://www.w3.org/TR/sparql11-query/#func-in)
    In(Box<Self>, Vec<Self>),
    /// [op:numeric-add](https://www.w3.org/TR/xpath-functions/#func-numeric-add) and other XSD additions.
    Add(Box<Self>, Box<Self>),
    /// [op:numeric-subtract](https://www.w3.org/TR/xpath-functions/#func-numeric-subtract) and other XSD subtractions.
    Subtract(Box<Self>, Box<Self>),
    /// [op:numeric-multiply](https://www.w3.org/TR/xpath-functions/#func-numeric-multiply) and other XSD multiplications.
    Multiply(Box<Self>, Box<Self>),
    /// [op:numeric-divide](https://www.w3.org/TR/xpath-functions/#func-numeric-divide) and other XSD divides.
    Divide(Box<Self>, Box<Self>),
    /// [op:numeric-unary-plus](https://www.w3.org/TR/xpath-functions/#func-numeric-unary-plus) and other XSD unary plus.
    UnaryPlus(Box<Self>),
    /// [op:numeric-unary-minus](https://www.w3.org/TR/xpath-functions/#func-numeric-unary-minus) and other XSD unary minus.
    UnaryMinus(Box<Self>),
    /// [fn:not](https://www.w3.org/TR/xpath-functions/#func-not).
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
    FunctionCall(Function, Vec<Self>),
}

impl Expression {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::NamedNode(node) => write!(f, "{}", node),
            Self::Literal(l) => write!(f, "{}", l),
            Self::Variable(var) => write!(f, "{}", var),
            Self::Or(a, b) => fmt_sse_binary_expression(f, "||", a, b),
            Self::And(a, b) => fmt_sse_binary_expression(f, "&&", a, b),
            Self::Equal(a, b) => fmt_sse_binary_expression(f, "=", a, b),
            Self::SameTerm(a, b) => fmt_sse_binary_expression(f, "sameTerm", a, b),
            Self::Greater(a, b) => fmt_sse_binary_expression(f, ">", a, b),
            Self::GreaterOrEqual(a, b) => fmt_sse_binary_expression(f, ">=", a, b),
            Self::Less(a, b) => fmt_sse_binary_expression(f, "<", a, b),
            Self::LessOrEqual(a, b) => fmt_sse_binary_expression(f, "<=", a, b),
            Self::In(a, b) => {
                write!(f, "(in ")?;
                a.fmt_sse(f)?;
                for p in b {
                    write!(f, " ")?;
                    p.fmt_sse(f)?;
                }
                write!(f, ")")
            }
            Self::Add(a, b) => fmt_sse_binary_expression(f, "+", a, b),
            Self::Subtract(a, b) => fmt_sse_binary_expression(f, "-", a, b),
            Self::Multiply(a, b) => fmt_sse_binary_expression(f, "*", a, b),
            Self::Divide(a, b) => fmt_sse_binary_expression(f, "/", a, b),
            Self::UnaryPlus(e) => fmt_sse_unary_expression(f, "+", e),
            Self::UnaryMinus(e) => fmt_sse_unary_expression(f, "-", e),
            Self::Not(e) => fmt_sse_unary_expression(f, "!", e),
            Self::FunctionCall(function, parameters) => {
                write!(f, "( ")?;
                function.fmt_sse(f)?;
                for p in parameters {
                    write!(f, " ")?;
                    p.fmt_sse(f)?;
                }
                write!(f, ")")
            }
            Self::Exists(p) => {
                write!(f, "(exists ")?;
                p.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Bound(v) => {
                write!(f, "(bound {})", v)
            }
            Self::If(a, b, c) => {
                write!(f, "(if ")?;
                a.fmt_sse(f)?;
                write!(f, " ")?;
                b.fmt_sse(f)?;
                write!(f, " ")?;
                c.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Coalesce(parameters) => {
                write!(f, "(coalesce")?;
                for p in parameters {
                    write!(f, " ")?;
                    p.fmt_sse(f)?;
                }
                write!(f, ")")
            }
        }
    }
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(node) => node.fmt(f),
            Self::Literal(l) => l.fmt(f),
            Self::Variable(var) => var.fmt(f),
            Self::Or(a, b) => write!(f, "({} || {})", a, b),
            Self::And(a, b) => write!(f, "({} && {})", a, b),
            Self::Equal(a, b) => {
                write!(f, "({} = {})", a, b)
            }
            Self::SameTerm(a, b) => {
                write!(f, "sameTerm({}, {})", a, b)
            }
            Self::Greater(a, b) => {
                write!(f, "({} > {})", a, b)
            }
            Self::GreaterOrEqual(a, b) => write!(f, "({} >= {})", a, b),
            Self::Less(a, b) => {
                write!(f, "({} < {})", a, b)
            }
            Self::LessOrEqual(a, b) => write!(f, "({} <= {})", a, b),
            Self::In(a, b) => {
                write!(f, "({} IN ", a)?;
                write_arg_list(b, f)?;
                write!(f, ")")
            }
            Self::Add(a, b) => {
                write!(f, "{} + {}", a, b)
            }
            Self::Subtract(a, b) => {
                write!(f, "{} - {}", a, b)
            }
            Self::Multiply(a, b) => {
                write!(f, "{} * {}", a, b)
            }
            Self::Divide(a, b) => {
                write!(f, "{} / {}", a, b)
            }
            Self::UnaryPlus(e) => write!(f, "+{}", e),
            Self::UnaryMinus(e) => write!(f, "-{}", e),
            Self::Not(e) => match e.as_ref() {
                Self::Exists(p) => write!(f, "NOT EXISTS {{ {} }}", p),
                e => write!(f, "!{}", e),
            },
            Self::FunctionCall(function, parameters) => {
                write!(f, "{}", function)?;
                write_arg_list(parameters, f)
            }
            Self::Bound(v) => write!(f, "BOUND({})", v),
            Self::Exists(p) => write!(f, "EXISTS {{ {} }}", p),
            Self::If(a, b, c) => write!(f, "IF({}, {}, {})", a, b, c),
            Self::Coalesce(parameters) => {
                write!(f, "COALESCE")?;
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

/// A function name.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum Function {
    Str,
    Lang,
    LangMatches,
    Datatype,
    Iri,
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
    EncodeForUri,
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
    Uuid,
    StrUuid,
    Md5,
    Sha1,
    Sha256,
    Sha384,
    Sha512,
    StrLang,
    StrDt,
    IsIri,
    IsBlank,
    IsLiteral,
    IsNumeric,
    Regex,
    #[cfg(feature = "rdf-star")]
    Triple,
    #[cfg(feature = "rdf-star")]
    Subject,
    #[cfg(feature = "rdf-star")]
    Predicate,
    #[cfg(feature = "rdf-star")]
    Object,
    #[cfg(feature = "rdf-star")]
    IsTriple,
    Custom(NamedNode),
}

impl Function {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::Str => write!(f, "str"),
            Self::Lang => write!(f, "lang"),
            Self::LangMatches => write!(f, "langmatches"),
            Self::Datatype => write!(f, "datatype"),
            Self::Iri => write!(f, "iri"),
            Self::BNode => write!(f, "bnode"),
            Self::Rand => write!(f, "rand"),
            Self::Abs => write!(f, "abs"),
            Self::Ceil => write!(f, "ceil"),
            Self::Floor => write!(f, "floor"),
            Self::Round => write!(f, "round"),
            Self::Concat => write!(f, "concat"),
            Self::SubStr => write!(f, "substr"),
            Self::StrLen => write!(f, "strlen"),
            Self::Replace => write!(f, "replace"),
            Self::UCase => write!(f, "ucase"),
            Self::LCase => write!(f, "lcase"),
            Self::EncodeForUri => write!(f, "encode_for_uri"),
            Self::Contains => write!(f, "contains"),
            Self::StrStarts => write!(f, "strstarts"),
            Self::StrEnds => write!(f, "strends"),
            Self::StrBefore => write!(f, "strbefore"),
            Self::StrAfter => write!(f, "strafter"),
            Self::Year => write!(f, "year"),
            Self::Month => write!(f, "month"),
            Self::Day => write!(f, "day"),
            Self::Hours => write!(f, "hours"),
            Self::Minutes => write!(f, "minutes"),
            Self::Seconds => write!(f, "seconds"),
            Self::Timezone => write!(f, "timezone"),
            Self::Tz => write!(f, "tz"),
            Self::Now => write!(f, "now"),
            Self::Uuid => write!(f, "uuid"),
            Self::StrUuid => write!(f, "struuid"),
            Self::Md5 => write!(f, "md5"),
            Self::Sha1 => write!(f, "sha1"),
            Self::Sha256 => write!(f, "sha256"),
            Self::Sha384 => write!(f, "sha384"),
            Self::Sha512 => write!(f, "sha512"),
            Self::StrLang => write!(f, "strlang"),
            Self::StrDt => write!(f, "strdt"),
            Self::IsIri => write!(f, "isiri"),
            Self::IsBlank => write!(f, "isblank"),
            Self::IsLiteral => write!(f, "isliteral"),
            Self::IsNumeric => write!(f, "isnumeric"),
            Self::Regex => write!(f, "regex"),
            #[cfg(feature = "rdf-star")]
            Self::Triple => write!(f, "triple"),
            #[cfg(feature = "rdf-star")]
            Self::Subject => write!(f, "subject"),
            #[cfg(feature = "rdf-star")]
            Self::Predicate => write!(f, "predicate"),
            #[cfg(feature = "rdf-star")]
            Self::Object => write!(f, "object"),
            #[cfg(feature = "rdf-star")]
            Self::IsTriple => write!(f, "istriple"),
            Self::Custom(iri) => write!(f, "{}", iri),
        }
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Str => write!(f, "STR"),
            Self::Lang => write!(f, "LANG"),
            Self::LangMatches => write!(f, "LANGMATCHES"),
            Self::Datatype => write!(f, "DATATYPE"),
            Self::Iri => write!(f, "IRI"),
            Self::BNode => write!(f, "BNODE"),
            Self::Rand => write!(f, "RAND"),
            Self::Abs => write!(f, "ABS"),
            Self::Ceil => write!(f, "CEIL"),
            Self::Floor => write!(f, "FLOOR"),
            Self::Round => write!(f, "ROUND"),
            Self::Concat => write!(f, "CONCAT"),
            Self::SubStr => write!(f, "SUBSTR"),
            Self::StrLen => write!(f, "STRLEN"),
            Self::Replace => write!(f, "REPLACE"),
            Self::UCase => write!(f, "UCASE"),
            Self::LCase => write!(f, "LCASE"),
            Self::EncodeForUri => write!(f, "ENCODE_FOR_URI"),
            Self::Contains => write!(f, "CONTAINS"),
            Self::StrStarts => write!(f, "STRSTATS"),
            Self::StrEnds => write!(f, "STRENDS"),
            Self::StrBefore => write!(f, "STRBEFORE"),
            Self::StrAfter => write!(f, "STRAFTER"),
            Self::Year => write!(f, "YEAR"),
            Self::Month => write!(f, "MONTH"),
            Self::Day => write!(f, "DAY"),
            Self::Hours => write!(f, "HOURS"),
            Self::Minutes => write!(f, "MINUTES"),
            Self::Seconds => write!(f, "SECONDS"),
            Self::Timezone => write!(f, "TIMEZONE"),
            Self::Tz => write!(f, "TZ"),
            Self::Now => write!(f, "NOW"),
            Self::Uuid => write!(f, "UUID"),
            Self::StrUuid => write!(f, "STRUUID"),
            Self::Md5 => write!(f, "MD5"),
            Self::Sha1 => write!(f, "SHA1"),
            Self::Sha256 => write!(f, "SHA256"),
            Self::Sha384 => write!(f, "SHA384"),
            Self::Sha512 => write!(f, "SHA512"),
            Self::StrLang => write!(f, "STRLANG"),
            Self::StrDt => write!(f, "STRDT"),
            Self::IsIri => write!(f, "isIRI"),
            Self::IsBlank => write!(f, "isBLANK"),
            Self::IsLiteral => write!(f, "isLITERAL"),
            Self::IsNumeric => write!(f, "isNUMERIC"),
            Self::Regex => write!(f, "REGEX"),
            #[cfg(feature = "rdf-star")]
            Self::Triple => write!(f, "TRIPLE"),
            #[cfg(feature = "rdf-star")]
            Self::Subject => write!(f, "SUBJECT"),
            #[cfg(feature = "rdf-star")]
            Self::Predicate => write!(f, "PREDICATE"),
            #[cfg(feature = "rdf-star")]
            Self::Object => write!(f, "OBJECT"),
            #[cfg(feature = "rdf-star")]
            Self::IsTriple => write!(f, "isTRIPLE"),
            Self::Custom(iri) => iri.fmt(f),
        }
    }
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
        start: usize,
        length: Option<usize>,
    },
    /// [Group](https://www.w3.org/TR/sparql11-federated-query/#aggregateAlgebra).
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

impl GraphPattern {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::Bgp { patterns } => {
                write!(f, "(bgp")?;
                for pattern in patterns {
                    write!(f, " ")?;
                    pattern.fmt_sse(f)?;
                }
                write!(f, ")")
            }
            Self::Path {
                subject,
                path,
                object,
            } => {
                write!(f, "(path ")?;
                subject.fmt_sse(f)?;
                write!(f, " ")?;
                path.fmt_sse(f)?;
                write!(f, " ")?;
                object.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Join { left, right } => {
                write!(f, "(join ")?;
                left.fmt_sse(f)?;
                write!(f, " ")?;
                right.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::LeftJoin {
                left,
                right,
                expression,
            } => {
                write!(f, "(leftjoin ")?;
                left.fmt_sse(f)?;
                write!(f, " ")?;
                right.fmt_sse(f)?;
                if let Some(expr) = expression {
                    write!(f, " ")?;
                    expr.fmt_sse(f)?;
                }
                write!(f, ")")
            }
            Self::Filter { expr, inner } => {
                write!(f, "(filter ")?;
                expr.fmt_sse(f)?;
                write!(f, " ")?;
                inner.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Union { left, right } => {
                write!(f, "(union ")?;
                left.fmt_sse(f)?;
                write!(f, " ")?;
                right.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Graph { name, inner } => {
                write!(f, "(graph ")?;
                name.fmt_sse(f)?;
                write!(f, " ")?;
                inner.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Extend {
                inner,
                variable,
                expression,
            } => {
                write!(f, "(extend (({} ", variable)?;
                expression.fmt_sse(f)?;
                write!(f, ")) ")?;
                inner.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Minus { left, right } => {
                write!(f, "(minus ")?;
                left.fmt_sse(f)?;
                write!(f, " ")?;
                right.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Service {
                name,
                inner,
                silent,
            } => {
                write!(f, "(service ")?;
                if *silent {
                    write!(f, "silent ")?;
                }
                name.fmt_sse(f)?;
                write!(f, " ")?;
                inner.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Group {
                inner,
                variables,
                aggregates,
            } => {
                write!(f, "(group (")?;
                for (i, v) in variables.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, ") (")?;
                for (i, (v, a)) in aggregates.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "(")?;
                    a.fmt_sse(f)?;
                    write!(f, " {})", v)?;
                }
                write!(f, ") ")?;
                inner.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Values {
                variables,
                bindings,
            } => {
                write!(f, "(table (vars")?;
                for var in variables {
                    write!(f, " {}", var)?;
                }
                write!(f, ")")?;
                for row in bindings {
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
            Self::OrderBy { inner, expression } => {
                write!(f, "(order (")?;
                for (i, c) in expression.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    c.fmt_sse(f)?;
                }
                write!(f, ") ")?;
                inner.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Project { inner, variables } => {
                write!(f, "(project (")?;
                for (i, v) in variables.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    write!(f, "{}", v)?;
                }
                write!(f, ") ")?;
                inner.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Distinct { inner } => {
                write!(f, "(distinct ")?;
                inner.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Reduced { inner } => {
                write!(f, "(reduced ")?;
                inner.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Slice {
                inner,
                start,
                length,
            } => {
                if let Some(length) = length {
                    write!(f, "(slice {} {} ", start, length)?;
                } else {
                    write!(f, "(slice {} _ ", start)?;
                }
                inner.fmt_sse(f)?;
                write!(f, ")")
            }
        }
    }
}

impl fmt::Display for GraphPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bgp { patterns } => {
                for pattern in patterns {
                    write!(f, "{} .", pattern)?
                }
                Ok(())
            }
            Self::Path {
                subject,
                path,
                object,
            } => write!(f, "{} {} {} .", subject, path, object),
            Self::Join { left, right } => {
                if matches!(
                    right.as_ref(),
                    Self::LeftJoin { .. }
                        | Self::Minus { .. }
                        | Self::Extend { .. }
                        | Self::Filter { .. }
                ) {
                    // The second block might be considered as a modification of the first one.
                    write!(f, "{} {{ {} }}", left, right)
                } else {
                    write!(f, "{} {}", left, right)
                }
            }
            Self::LeftJoin {
                left,
                right,
                expression,
            } => {
                if let Some(expr) = expression {
                    write!(f, "{} OPTIONAL {{ {} FILTER({}) }}", left, right, expr)
                } else {
                    write!(f, "{} OPTIONAL {{ {} }}", left, right)
                }
            }
            Self::Filter { expr, inner } => {
                write!(f, "{} FILTER({})", inner, expr)
            }
            Self::Union { left, right } => write!(f, "{{ {} }} UNION {{ {} }}", left, right,),
            Self::Graph { name, inner } => {
                write!(f, "GRAPH {} {{ {} }}", name, inner)
            }
            Self::Extend {
                inner,
                variable,
                expression,
            } => write!(f, "{} BIND({} AS {})", inner, expression, variable),
            Self::Minus { left, right } => write!(f, "{} MINUS {{ {} }}", left, right),
            Self::Service {
                name,
                inner,
                silent,
            } => {
                if *silent {
                    write!(f, "SERVICE SILENT {} {{ {} }}", name, inner)
                } else {
                    write!(f, "SERVICE {} {{ {} }}", name, inner)
                }
            }
            Self::Values {
                variables,
                bindings,
            } => {
                write!(f, "VALUES ( ")?;
                for var in variables {
                    write!(f, "{} ", var)?;
                }
                write!(f, ") {{ ")?;
                for row in bindings {
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
            Self::Group {
                inner,
                variables,
                aggregates,
            } => {
                write!(f, "{{SELECT")?;
                for (a, v) in aggregates {
                    write!(f, " ({} AS {})", v, a)?;
                }
                for b in variables {
                    write!(f, " {}", b)?;
                }
                write!(f, " WHERE {{ {} }}", inner)?;
                if !variables.is_empty() {
                    write!(f, " GROUP BY")?;
                    for v in variables {
                        write!(f, " {}", v)?;
                    }
                }
                write!(f, "}}")
            }
            p => write!(
                f,
                "{{ {} }}",
                SparqlGraphRootPattern {
                    pattern: p,
                    dataset: None
                }
            ),
        }
    }
}

impl Default for GraphPattern {
    fn default() -> Self {
        Self::Bgp {
            patterns: Vec::default(),
        }
    }
}

impl GraphPattern {
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
                #[cfg(feature = "rdf-star")]
                if let TermPattern::Triple(s) = subject {
                    lookup_triple_pattern_variables(s, callback)
                }
                if let TermPattern::Variable(o) = object {
                    callback(o);
                }
                #[cfg(feature = "rdf-star")]
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
            Self::Graph { name, inner } => {
                if let NamedNodePattern::Variable(ref g) = name {
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
            Self::Service { inner, .. } => inner.lookup_in_scope_variables(callback),
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
            Self::Values { variables, .. } => {
                for v in variables {
                    callback(v);
                }
            }
            Self::Project { variables, .. } => {
                for v in variables {
                    callback(v);
                }
            }
            Self::Filter { inner, .. }
            | Self::OrderBy { inner, .. }
            | Self::Distinct { inner }
            | Self::Reduced { inner }
            | Self::Slice { inner, .. } => inner.lookup_in_scope_variables(callback),
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
    #[cfg(feature = "rdf-star")]
    if let TermPattern::Triple(s) = &pattern.subject {
        lookup_triple_pattern_variables(s, callback)
    }
    if let NamedNodePattern::Variable(p) = &pattern.predicate {
        callback(p);
    }
    if let TermPattern::Variable(o) = &pattern.object {
        callback(o);
    }
    #[cfg(feature = "rdf-star")]
    if let TermPattern::Triple(o) = &pattern.object {
        lookup_triple_pattern_variables(o, callback)
    }
}

pub(crate) struct SparqlGraphRootPattern<'a> {
    pub(crate) pattern: &'a GraphPattern,
    pub(crate) dataset: Option<&'a QueryDataset>,
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
                GraphPattern::OrderBy { inner, expression } => {
                    order = Some(expression);
                    child = &*inner;
                }
                GraphPattern::Project { inner, variables } if project.is_empty() => {
                    project = variables;
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
                    write!(f, "SELECT")?;
                    if distinct {
                        write!(f, " DISTINCT")?;
                    }
                    if reduced {
                        write!(f, " REDUCED")?;
                    }
                    if project.is_empty() {
                        write!(f, " *")?;
                    } else {
                        for v in project {
                            write!(f, " {}", v)?;
                        }
                    }
                    if let Some(dataset) = self.dataset {
                        write!(f, " {}", dataset)?;
                    }
                    write!(f, " WHERE {{ {} }}", p)?;
                    if let Some(order) = order {
                        write!(f, " ORDER BY")?;
                        for c in order {
                            write!(f, " {}", c)?;
                        }
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

/// A set function used in aggregates (c.f. [`GraphPattern::Group`]).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum AggregateExpression {
    /// [Count](https://www.w3.org/TR/sparql11-query/#defn_aggCount).
    Count {
        expr: Option<Box<Expression>>,
        distinct: bool,
    },
    /// [Sum](https://www.w3.org/TR/sparql11-query/#defn_aggSum).
    Sum {
        expr: Box<Expression>,
        distinct: bool,
    },
    /// [Avg](https://www.w3.org/TR/sparql11-query/#defn_aggAvg).
    Avg {
        expr: Box<Expression>,
        distinct: bool,
    },
    /// [Min](https://www.w3.org/TR/sparql11-query/#defn_aggMin).
    Min {
        expr: Box<Expression>,
        distinct: bool,
    },
    /// [Max](https://www.w3.org/TR/sparql11-query/#defn_aggMax).
    Max {
        expr: Box<Expression>,
        distinct: bool,
    },
    /// [GroupConcat](https://www.w3.org/TR/sparql11-query/#defn_aggGroupConcat).
    GroupConcat {
        expr: Box<Expression>,
        distinct: bool,
        separator: Option<String>,
    },
    /// [Sample](https://www.w3.org/TR/sparql11-query/#defn_aggSample).
    Sample {
        expr: Box<Expression>,
        distinct: bool,
    },
    /// Custom function.
    Custom {
        name: NamedNode,
        expr: Box<Expression>,
        distinct: bool,
    },
}

impl AggregateExpression {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::Count { expr, distinct } => {
                write!(f, "(sum")?;
                if *distinct {
                    write!(f, " distinct")?;
                }
                if let Some(expr) = expr {
                    write!(f, " ")?;
                    expr.fmt_sse(f)?;
                }
                write!(f, ")")
            }
            Self::Sum { expr, distinct } => {
                write!(f, "(sum ")?;
                if *distinct {
                    write!(f, "distinct ")?;
                }
                expr.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Avg { expr, distinct } => {
                write!(f, "(avg ")?;
                if *distinct {
                    write!(f, "distinct ")?;
                }
                expr.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Min { expr, distinct } => {
                write!(f, "(min ")?;
                if *distinct {
                    write!(f, "distinct ")?;
                }
                expr.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Max { expr, distinct } => {
                write!(f, "(max ")?;
                if *distinct {
                    write!(f, "distinct ")?;
                }
                expr.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Sample { expr, distinct } => {
                write!(f, "(sample ")?;
                if *distinct {
                    write!(f, "distinct ")?;
                }
                expr.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::GroupConcat {
                expr,
                distinct,
                separator,
            } => {
                write!(f, "(group_concat ")?;
                if *distinct {
                    write!(f, "distinct ")?;
                }
                expr.fmt_sse(f)?;
                if let Some(separator) = separator {
                    write!(f, " {}", LiteralRef::new_simple_literal(separator))?;
                }
                write!(f, ")")
            }
            Self::Custom {
                name,
                expr,
                distinct,
            } => {
                write!(f, "({}", name)?;
                if *distinct {
                    write!(f, " distinct")?;
                }
                write!(f, " ")?;
                expr.fmt_sse(f)?;
                write!(f, ")")
            }
        }
    }
}

impl fmt::Display for AggregateExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Count { expr, distinct } => {
                if *distinct {
                    if let Some(expr) = expr {
                        write!(f, "COUNT(DISTINCT {})", expr)
                    } else {
                        write!(f, "COUNT(DISTINCT *)")
                    }
                } else if let Some(expr) = expr {
                    write!(f, "COUNT({})", expr)
                } else {
                    write!(f, "COUNT(*)")
                }
            }
            Self::Sum { expr, distinct } => {
                if *distinct {
                    write!(f, "SUM(DISTINCT {})", expr)
                } else {
                    write!(f, "SUM({})", expr)
                }
            }
            Self::Min { expr, distinct } => {
                if *distinct {
                    write!(f, "MIN(DISTINCT {})", expr)
                } else {
                    write!(f, "MIN({})", expr)
                }
            }
            Self::Max { expr, distinct } => {
                if *distinct {
                    write!(f, "MAX(DISTINCT {})", expr)
                } else {
                    write!(f, "MAX({})", expr)
                }
            }
            Self::Avg { expr, distinct } => {
                if *distinct {
                    write!(f, "AVG(DISTINCT {})", expr)
                } else {
                    write!(f, "AVG({})", expr)
                }
            }
            Self::Sample { expr, distinct } => {
                if *distinct {
                    write!(f, "SAMPLE(DISTINCT {})", expr)
                } else {
                    write!(f, "SAMPLE({})", expr)
                }
            }
            Self::GroupConcat {
                expr,
                distinct,
                separator,
            } => {
                if *distinct {
                    if let Some(separator) = separator {
                        write!(
                            f,
                            "GROUP_CONCAT(DISTINCT {}; SEPARATOR = {})",
                            expr,
                            LiteralRef::new_simple_literal(separator)
                        )
                    } else {
                        write!(f, "GROUP_CONCAT(DISTINCT {})", expr)
                    }
                } else if let Some(separator) = separator {
                    write!(
                        f,
                        "GROUP_CONCAT({}; SEPARATOR = {})",
                        expr,
                        LiteralRef::new_simple_literal(separator)
                    )
                } else {
                    write!(f, "GROUP_CONCAT({})", expr)
                }
            }
            Self::Custom {
                name,
                expr,
                distinct,
            } => {
                if *distinct {
                    write!(f, "{}(DISTINCT {})", name, expr)
                } else {
                    write!(f, "{}({})", name, expr)
                }
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
                write!(f, "(asc ")?;
                e.fmt_sse(f)?;
                write!(f, ")")
            }
            Self::Desc(e) => {
                write!(f, "(desc ")?;
                e.fmt_sse(f)?;
                write!(f, ")")
            }
        }
    }
}

impl fmt::Display for OrderExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Asc(e) => write!(f, "ASC({})", e),
            Self::Desc(e) => write!(f, "DESC({})", e),
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
        write!(f, "(")?;
        for (i, graph_name) in self.default.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            write!(f, "{}", graph_name)?;
        }
        if let Some(named) = &self.named {
            for (i, graph_name) in named.iter().enumerate() {
                if !self.default.is_empty() || i > 0 {
                    write!(f, " ")?;
                }
                write!(f, "(named {})", graph_name)?;
            }
        }
        write!(f, ")")
    }
}

impl fmt::Display for QueryDataset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for g in &self.default {
            write!(f, " FROM {}", g)?;
        }
        if let Some(named) = &self.named {
            for g in named {
                write!(f, " FROM NAMED {}", g)?;
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
            Self::NamedNode(node) => write!(f, "{}", node),
            Self::DefaultGraph => write!(f, "default"),
            Self::NamedGraphs => write!(f, "named"),
            Self::AllGraphs => write!(f, "all"),
        }
    }
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
    write!(f, "({} ", name)?;
    e.fmt_sse(f)?;
    write!(f, ")")
}

#[inline]
fn fmt_sse_binary_expression(
    f: &mut impl fmt::Write,
    name: &str,
    a: &Expression,
    b: &Expression,
) -> fmt::Result {
    write!(f, "({} ", name)?;
    a.fmt_sse(f)?;
    write!(f, " ")?;
    b.fmt_sse(f)?;
    write!(f, ")")
}
