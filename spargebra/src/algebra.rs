//! [SPARQL 1.1 Query Algebra](https://www.w3.org/TR/sparql11-query/#sparqlQuery) representation

use crate::term::print_quoted_str;
use crate::term::*;
use std::collections::BTreeSet;
use std::fmt;

pub(crate) struct SparqlTriplePattern<'a>(pub(crate) &'a TriplePattern);

impl<'a> fmt::Display for SparqlTriplePattern<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {} .",
            self.0.subject, self.0.predicate, self.0.object
        )
    }
}

pub(crate) struct SparqlQuadPattern<'a>(pub(crate) &'a QuadPattern);

impl<'a> fmt::Display for SparqlQuadPattern<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.graph_name == GraphNamePattern::DefaultGraph {
            write!(
                f,
                "{} {} {} .",
                self.0.subject, self.0.predicate, self.0.object
            )
        } else {
            write!(
                f,
                "GRAPH {} {{ {} {} {} }}",
                self.0.graph_name, self.0.subject, self.0.predicate, self.0.object
            )
        }
    }
}

pub(crate) struct SparqlGroundQuadPattern<'a>(pub(crate) &'a GroundQuadPattern);

impl<'a> fmt::Display for SparqlGroundQuadPattern<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.graph_name == GraphNamePattern::DefaultGraph {
            write!(
                f,
                "{} {} {} .",
                self.0.subject, self.0.predicate, self.0.object
            )
        } else {
            write!(
                f,
                "GRAPH {} {{ {} {} {} }}",
                self.0.graph_name, self.0.subject, self.0.predicate, self.0.object
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
    Triple,
    Subject,
    Predicate,
    Object,
    IsTriple,
    Custom(NamedNode),
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Function::Str => write!(f, "STR"),
            Function::Lang => write!(f, "LANG"),
            Function::LangMatches => write!(f, "LANGMATCHES"),
            Function::Datatype => write!(f, "DATATYPE"),
            Function::Iri => write!(f, "IRI"),
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
            Function::EncodeForUri => write!(f, "ENCODE_FOR_URI"),
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
            Function::Uuid => write!(f, "UUID"),
            Function::StrUuid => write!(f, "STRUUID"),
            Function::Md5 => write!(f, "MD5"),
            Function::Sha1 => write!(f, "SHA1"),
            Function::Sha256 => write!(f, "SHA256"),
            Function::Sha384 => write!(f, "SHA384"),
            Function::Sha512 => write!(f, "SHA512"),
            Function::StrLang => write!(f, "STRLANG"),
            Function::StrDt => write!(f, "STRDT"),
            Function::IsIri => write!(f, "isIRI"),
            Function::IsBlank => write!(f, "isBLANK"),
            Function::IsLiteral => write!(f, "isLITERAL"),
            Function::IsNumeric => write!(f, "isNUMERIC"),
            Function::Regex => write!(f, "REGEX"),
            Function::Triple => write!(f, "TRIPLE"),
            Function::Subject => write!(f, "SUBJECT"),
            Function::Predicate => write!(f, "PREDICATE"),
            Function::Object => write!(f, "OBJECT"),
            Function::IsTriple => write!(f, "isTRIPLE"),
            Function::Custom(iri) => iri.fmt(f),
        }
    }
}

/// A SPARQL query [graph pattern](https://www.w3.org/TR/sparql11-query/#sparqlQuery)
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GraphPattern {
    /// A [basic graph pattern](https://www.w3.org/TR/sparql11-query/#defn_BasicGraphPattern)
    Bgp(Vec<TriplePattern>),
    /// A [property path pattern](https://www.w3.org/TR/sparql11-query/#defn_evalPP_predicate)
    Path {
        subject: TermPattern,
        path: PropertyPathExpression,
        object: TermPattern,
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
        graph_name: NamedNodePattern,
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
        rows: Vec<Vec<Option<GroundTerm>>>,
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
        name: NamedNodePattern,
        pattern: Box<GraphPattern>,
        silent: bool,
    },
}

impl fmt::Display for GraphPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphPattern::Bgp(p) => {
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
                    .map(|v| v.name.as_str())
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
        GraphPattern::Bgp(Vec::default())
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
            GraphPattern::Bgp(p) => {
                for pattern in p {
                    if let TermPattern::Variable(s) = &pattern.subject {
                        vars.insert(s);
                    }
                    if let NamedNodePattern::Variable(p) = &pattern.predicate {
                        vars.insert(p);
                    }
                    if let TermPattern::Variable(o) = &pattern.object {
                        vars.insert(o);
                    }
                }
            }
            GraphPattern::Path {
                subject, object, ..
            } => {
                if let TermPattern::Variable(s) = subject {
                    vars.insert(s);
                }
                if let TermPattern::Variable(o) = object {
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
                if let NamedNodePattern::Variable(ref g) = graph_name {
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
            GraphPattern::Bgp(p) => {
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
                    dataset: None
                }
            ),
        }
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
                    build_sparql_select_arguments(project).fmt(f)?;
                    if let Some(dataset) = self.dataset {
                        dataset.fmt(f)?;
                    }
                    write!(f, " WHERE {{ {} }}", SparqlGraphPattern(p))?;
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
                        write!(f, "(group_concat distinct {} ", expr)?;
                        print_quoted_str(separator, f)?;
                        write!(f, ")")
                    } else {
                        write!(f, "(group_concat distinct {})", expr)
                    }
                } else if let Some(separator) = separator {
                    write!(f, "(group_concat {} ", expr)?;
                    print_quoted_str(separator, f)?;
                    write!(f, ")")
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
                            "GROUP_CONCAT(DISTINCT {}; SEPARATOR = ",
                            SparqlExpression(expr)
                        )?;
                        print_quoted_str(separator, f)?;
                        write!(f, ")")
                    } else {
                        write!(f, "GROUP_CONCAT(DISTINCT {})", SparqlExpression(expr))
                    }
                } else if let Some(separator) = separator {
                    write!(f, "GROUP_CONCAT({}; SEPARATOR = ", SparqlExpression(expr))?;
                    print_quoted_str(separator, f)?;
                    write!(f, ")")
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
    pub default: Vec<NamedNode>,
    pub named: Option<Vec<NamedNode>>,
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

impl From<GraphName> for GraphTarget {
    fn from(graph_name: GraphName) -> Self {
        match graph_name {
            GraphName::NamedNode(node) => Self::NamedNode(node),
            GraphName::DefaultGraph => Self::DefaultGraph,
        }
    }
}
