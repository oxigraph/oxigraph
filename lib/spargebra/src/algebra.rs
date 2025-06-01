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
    FunctionCall(Function, Vec<Self>),
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
                f.write_str("( ")?;
                function.fmt_sse(f)?;
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
                write!(f, "{a} + {b}")
            }
            Self::Subtract(a, b) => {
                write!(f, "{a} - {b}")
            }
            Self::Multiply(a, b) => {
                write!(f, "{a} * {b}")
            }
            Self::Divide(a, b) => {
                write!(f, "{a} / {b}")
            }
            Self::UnaryPlus(e) => write!(f, "+{e}"),
            Self::UnaryMinus(e) => write!(f, "-{e}"),
            Self::Not(e) => match e.as_ref() {
                Self::Exists(p) => write!(f, "NOT EXISTS {{ {p} }}"),
                e => write!(f, "!{e}"),
            },
            Self::FunctionCall(function, parameters) => {
                write!(f, "{function}")?;
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
    #[cfg(feature = "sparql-12")]
    Triple,
    #[cfg(feature = "sparql-12")]
    Subject,
    #[cfg(feature = "sparql-12")]
    Predicate,
    #[cfg(feature = "sparql-12")]
    Object,
    #[cfg(feature = "sparql-12")]
    IsTriple,
    #[cfg(feature = "sparql-12")]
    LangDir,
    #[cfg(feature = "sparql-12")]
    HasLang,
    #[cfg(feature = "sparql-12")]
    HasLangDir,
    #[cfg(feature = "sparql-12")]
    StrLangDir,
    #[cfg(feature = "sep-0002")]
    Adjust,
    Custom(NamedNode),
}

impl Function {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::Str => f.write_str("str"),
            Self::Lang => f.write_str("lang"),
            Self::LangMatches => f.write_str("langmatches"),
            Self::Datatype => f.write_str("datatype"),
            Self::Iri => f.write_str("iri"),
            Self::BNode => f.write_str("bnode"),
            Self::Rand => f.write_str("rand"),
            Self::Abs => f.write_str("abs"),
            Self::Ceil => f.write_str("ceil"),
            Self::Floor => f.write_str("floor"),
            Self::Round => f.write_str("round"),
            Self::Concat => f.write_str("concat"),
            Self::SubStr => f.write_str("substr"),
            Self::StrLen => f.write_str("strlen"),
            Self::Replace => f.write_str("replace"),
            Self::UCase => f.write_str("ucase"),
            Self::LCase => f.write_str("lcase"),
            Self::EncodeForUri => f.write_str("encode_for_uri"),
            Self::Contains => f.write_str("contains"),
            Self::StrStarts => f.write_str("strstarts"),
            Self::StrEnds => f.write_str("strends"),
            Self::StrBefore => f.write_str("strbefore"),
            Self::StrAfter => f.write_str("strafter"),
            Self::Year => f.write_str("year"),
            Self::Month => f.write_str("month"),
            Self::Day => f.write_str("day"),
            Self::Hours => f.write_str("hours"),
            Self::Minutes => f.write_str("minutes"),
            Self::Seconds => f.write_str("seconds"),
            Self::Timezone => f.write_str("timezone"),
            Self::Tz => f.write_str("tz"),
            Self::Now => f.write_str("now"),
            Self::Uuid => f.write_str("uuid"),
            Self::StrUuid => f.write_str("struuid"),
            Self::Md5 => f.write_str("md5"),
            Self::Sha1 => f.write_str("sha1"),
            Self::Sha256 => f.write_str("sha256"),
            Self::Sha384 => f.write_str("sha384"),
            Self::Sha512 => f.write_str("sha512"),
            Self::StrLang => f.write_str("strlang"),
            Self::StrDt => f.write_str("strdt"),
            Self::IsIri => f.write_str("isiri"),
            Self::IsBlank => f.write_str("isblank"),
            Self::IsLiteral => f.write_str("isliteral"),
            Self::IsNumeric => f.write_str("isnumeric"),
            Self::Regex => f.write_str("regex"),
            #[cfg(feature = "sparql-12")]
            Self::Triple => f.write_str("triple"),
            #[cfg(feature = "sparql-12")]
            Self::Subject => f.write_str("subject"),
            #[cfg(feature = "sparql-12")]
            Self::Predicate => f.write_str("predicate"),
            #[cfg(feature = "sparql-12")]
            Self::Object => f.write_str("object"),
            #[cfg(feature = "sparql-12")]
            Self::IsTriple => f.write_str("istriple"),
            #[cfg(feature = "sparql-12")]
            Function::LangDir => f.write_str("langdir"),
            #[cfg(feature = "sparql-12")]
            Function::HasLang => f.write_str("haslang"),
            #[cfg(feature = "sparql-12")]
            Function::HasLangDir => f.write_str("haslangdir"),
            #[cfg(feature = "sparql-12")]
            Function::StrLangDir => f.write_str("strlangdir"),
            #[cfg(feature = "sep-0002")]
            Self::Adjust => f.write_str("adjust"),
            Self::Custom(iri) => write!(f, "{iri}"),
        }
    }
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Str => f.write_str("STR"),
            Self::Lang => f.write_str("LANG"),
            Self::LangMatches => f.write_str("LANGMATCHES"),
            Self::Datatype => f.write_str("DATATYPE"),
            Self::Iri => f.write_str("IRI"),
            Self::BNode => f.write_str("BNODE"),
            Self::Rand => f.write_str("RAND"),
            Self::Abs => f.write_str("ABS"),
            Self::Ceil => f.write_str("CEIL"),
            Self::Floor => f.write_str("FLOOR"),
            Self::Round => f.write_str("ROUND"),
            Self::Concat => f.write_str("CONCAT"),
            Self::SubStr => f.write_str("SUBSTR"),
            Self::StrLen => f.write_str("STRLEN"),
            Self::Replace => f.write_str("REPLACE"),
            Self::UCase => f.write_str("UCASE"),
            Self::LCase => f.write_str("LCASE"),
            Self::EncodeForUri => f.write_str("ENCODE_FOR_URI"),
            Self::Contains => f.write_str("CONTAINS"),
            Self::StrStarts => f.write_str("STRSTARTS"),
            Self::StrEnds => f.write_str("STRENDS"),
            Self::StrBefore => f.write_str("STRBEFORE"),
            Self::StrAfter => f.write_str("STRAFTER"),
            Self::Year => f.write_str("YEAR"),
            Self::Month => f.write_str("MONTH"),
            Self::Day => f.write_str("DAY"),
            Self::Hours => f.write_str("HOURS"),
            Self::Minutes => f.write_str("MINUTES"),
            Self::Seconds => f.write_str("SECONDS"),
            Self::Timezone => f.write_str("TIMEZONE"),
            Self::Tz => f.write_str("TZ"),
            Self::Now => f.write_str("NOW"),
            Self::Uuid => f.write_str("UUID"),
            Self::StrUuid => f.write_str("STRUUID"),
            Self::Md5 => f.write_str("MD5"),
            Self::Sha1 => f.write_str("SHA1"),
            Self::Sha256 => f.write_str("SHA256"),
            Self::Sha384 => f.write_str("SHA384"),
            Self::Sha512 => f.write_str("SHA512"),
            Self::StrLang => f.write_str("STRLANG"),
            Self::StrDt => f.write_str("STRDT"),
            Self::IsIri => f.write_str("isIRI"),
            Self::IsBlank => f.write_str("isBLANK"),
            Self::IsLiteral => f.write_str("isLITERAL"),
            Self::IsNumeric => f.write_str("isNUMERIC"),
            Self::Regex => f.write_str("REGEX"),
            #[cfg(feature = "sparql-12")]
            Self::Triple => f.write_str("TRIPLE"),
            #[cfg(feature = "sparql-12")]
            Self::Subject => f.write_str("SUBJECT"),
            #[cfg(feature = "sparql-12")]
            Self::Predicate => f.write_str("PREDICATE"),
            #[cfg(feature = "sparql-12")]
            Self::Object => f.write_str("OBJECT"),
            #[cfg(feature = "sparql-12")]
            Self::IsTriple => f.write_str("isTRIPLE"),
            #[cfg(feature = "sparql-12")]
            Function::LangDir => f.write_str("LANGDIR"),
            #[cfg(feature = "sparql-12")]
            Function::HasLang => f.write_str("hasLANG"),
            #[cfg(feature = "sparql-12")]
            Function::HasLangDir => f.write_str("hasLANGDIR"),
            #[cfg(feature = "sparql-12")]
            Function::StrLangDir => f.write_str("STRLANGDIR"),
            #[cfg(feature = "sep-0002")]
            Self::Adjust => f.write_str("ADJUST"),
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
        start: usize,
        length: Option<usize>,
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
                match right.as_ref() {
                    Self::LeftJoin { .. }
                    | Self::Minus { .. }
                    | Self::Extend { .. }
                    | Self::Filter { .. } => {
                        // The second block might be considered as a modification of the first one.
                        write!(f, "{left} {{ {right} }}")
                    }
                    #[cfg(feature = "sep-0006")]
                    Self::Lateral { .. } => {
                        write!(f, "{left} {{ {right} }}")
                    }
                    _ => write!(f, "{left} {right}"),
                }
            }
            Self::LeftJoin {
                left,
                right,
                expression,
            } => {
                if let Some(expr) = expression {
                    write!(f, "{left} OPTIONAL {{ {right} FILTER({expr}) }}")
                } else {
                    write!(f, "{left} OPTIONAL {{ {right} }}")
                }
            }
            #[cfg(feature = "sep-0006")]
            Self::Lateral { left, right } => {
                write!(f, "{left} LATERAL {{ {right} }}")
            }
            Self::Filter { expr, inner } => {
                write!(f, "{inner} FILTER({expr})")
            }
            Self::Union { left, right } => write!(f, "{{ {left} }} UNION {{ {right} }}"),
            Self::Graph { name, inner } => {
                write!(f, "GRAPH {name} {{ {inner} }}")
            }
            Self::Extend {
                inner,
                variable,
                expression,
            } => write!(f, "{inner} BIND({expression} AS {variable})"),
            Self::Minus { left, right } => write!(f, "{left} MINUS {{ {right} }}"),
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
                f.write_str("{SELECT")?;
                for (a, v) in aggregates {
                    write!(f, " ({v} AS {a})")?;
                }
                for b in variables {
                    write!(f, " {b}")?;
                }
                write!(f, " WHERE {{ {inner} }}")?;
                if !variables.is_empty() {
                    f.write_str(" GROUP BY")?;
                    for v in variables {
                        write!(f, " {v}")?;
                    }
                }
                f.write_str("}")
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
    pattern: &'a GraphPattern,
    dataset: Option<&'a QueryDataset>,
}

impl<'a> SparqlGraphRootPattern<'a> {
    pub fn new(pattern: &'a GraphPattern, dataset: Option<&'a QueryDataset>) -> Self {
        Self { pattern, dataset }
    }
}

impl fmt::Display for SparqlGraphRootPattern<'_> {
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
                    child = inner;
                }
                GraphPattern::Project { inner, variables } if project.is_empty() => {
                    project = variables;
                    child = inner;
                }
                GraphPattern::Distinct { inner } => {
                    distinct = true;
                    child = inner;
                }
                GraphPattern::Reduced { inner } => {
                    reduced = true;
                    child = inner;
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
                    f.write_str("SELECT")?;
                    if distinct {
                        f.write_str(" DISTINCT")?;
                    }
                    if reduced {
                        f.write_str(" REDUCED")?;
                    }
                    if project.is_empty() {
                        f.write_str(" *")?;
                    } else {
                        for v in project {
                            write!(f, " {v}")?;
                        }
                    }
                    if let Some(dataset) = self.dataset {
                        write!(f, " {dataset}")?;
                    }
                    write!(f, " WHERE {{ {p} }}")?;
                    if let Some(order) = order {
                        f.write_str(" ORDER BY")?;
                        for c in order {
                            write!(f, " {c}")?;
                        }
                    }
                    if start > 0 {
                        write!(f, " OFFSET {start}")?;
                    }
                    if let Some(length) = length {
                        write!(f, " LIMIT {length}")?;
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
    /// [Count](https://www.w3.org/TR/sparql11-query/#defn_aggCount) with *.
    CountSolutions { distinct: bool },
    FunctionCall {
        name: AggregateFunction,
        expr: Expression,
        distinct: bool,
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
                name:
                    AggregateFunction::GroupConcat {
                        separator: Some(separator),
                    },
                expr,
                distinct,
            } => {
                f.write_str("(group_concat ")?;
                if *distinct {
                    f.write_str("distinct ")?;
                }
                expr.fmt_sse(f)?;
                write!(f, " {})", LiteralRef::new_simple_literal(separator))
            }
            Self::FunctionCall {
                name,
                expr,
                distinct,
            } => {
                f.write_str("(")?;
                name.fmt_sse(f)?;
                f.write_str(" ")?;
                if *distinct {
                    f.write_str("distinct ")?;
                }
                expr.fmt_sse(f)?;
                f.write_str(")")
            }
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
                name:
                    AggregateFunction::GroupConcat {
                        separator: Some(separator),
                    },
                expr,
                distinct,
            } => {
                if *distinct {
                    write!(
                        f,
                        "GROUP_CONCAT(DISTINCT {}; SEPARATOR = {})",
                        expr,
                        LiteralRef::new_simple_literal(separator)
                    )
                } else {
                    write!(
                        f,
                        "GROUP_CONCAT({}; SEPARATOR = {})",
                        expr,
                        LiteralRef::new_simple_literal(separator)
                    )
                }
            }
            Self::FunctionCall {
                name,
                expr,
                distinct,
            } => {
                if *distinct {
                    write!(f, "{name}(DISTINCT {expr})")
                } else {
                    write!(f, "{name}({expr})")
                }
            }
        }
    }
}

/// An aggregate function name.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum AggregateFunction {
    /// [Count](https://www.w3.org/TR/sparql11-query/#defn_aggCount) with *.
    Count,
    /// [Sum](https://www.w3.org/TR/sparql11-query/#defn_aggSum).
    Sum,
    /// [Avg](https://www.w3.org/TR/sparql11-query/#defn_aggAvg).
    Avg,
    /// [Min](https://www.w3.org/TR/sparql11-query/#defn_aggMin).
    Min,
    /// [Max](https://www.w3.org/TR/sparql11-query/#defn_aggMax).
    Max,
    /// [GroupConcat](https://www.w3.org/TR/sparql11-query/#defn_aggGroupConcat).
    GroupConcat {
        separator: Option<String>,
    },
    /// [Sample](https://www.w3.org/TR/sparql11-query/#defn_aggSample).
    Sample,
    Custom(NamedNode),
}

impl AggregateFunction {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub(crate) fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::Count => f.write_str("count"),
            Self::Sum => f.write_str("sum"),
            Self::Avg => f.write_str("avg"),
            Self::Min => f.write_str("min"),
            Self::Max => f.write_str("max"),
            Self::GroupConcat { .. } => f.write_str("group_concat"),
            Self::Sample => f.write_str("sample"),
            Self::Custom(iri) => write!(f, "{iri}"),
        }
    }
}

impl fmt::Display for AggregateFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Count => f.write_str("COUNT"),
            Self::Sum => f.write_str("SUM"),
            Self::Avg => f.write_str("AVG"),
            Self::Min => f.write_str("MIN"),
            Self::Max => f.write_str("MAX"),
            Self::GroupConcat { .. } => f.write_str("GROUP_CONCAT"),
            Self::Sample => f.write_str("SAMPLE"),
            Self::Custom(iri) => iri.fmt(f),
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
