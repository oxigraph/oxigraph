//! [SPARQL 1.1 Query Algebra](https://www.w3.org/TR/sparql11-query/#sparqlQuery) AST

use crate::model::*;
use crate::utils::Escaper;
use crate::Result;
use failure::format_err;
use lazy_static::lazy_static;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;
use std::ops::Add;
use uuid::Uuid;

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

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct StaticBindings {
    variables: Vec<Variable>,
    values: Vec<Vec<Option<Term>>>,
}

impl StaticBindings {
    pub fn new(variables: Vec<Variable>, values: Vec<Vec<Option<Term>>>) -> Self {
        Self { variables, values }
    }

    pub fn variables(&self) -> &[Variable] {
        &*self.variables
    }

    pub fn variables_iter(&self) -> impl Iterator<Item = &Variable> {
        self.variables.iter()
    }

    pub fn values_iter(&self) -> impl Iterator<Item = &Vec<Option<Term>>> {
        self.values.iter()
    }

    pub fn into_iterator(self) -> BindingsIterator<'static> {
        BindingsIterator {
            variables: self.variables,
            iter: Box::new(self.values.into_iter().map(Ok)),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl Default for StaticBindings {
    fn default() -> Self {
        Self {
            variables: Vec::default(),
            values: Vec::default(),
        }
    }
}

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

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct TriplePattern {
    pub subject: TermOrVariable,
    pub predicate: NamedNodeOrVariable,
    pub object: TermOrVariable,
}

impl TriplePattern {
    pub fn new(
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
        write!(f, "{} {} {}", self.subject, self.predicate, self.object)
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum PropertyPath {
    PredicatePath(NamedNode),
    InversePath(Box<PropertyPath>),
    SequencePath(Box<PropertyPath>, Box<PropertyPath>),
    AlternativePath(Box<PropertyPath>, Box<PropertyPath>),
    ZeroOrMorePath(Box<PropertyPath>),
    OneOrMorePath(Box<PropertyPath>),
    ZeroOrOnePath(Box<PropertyPath>),
    NegatedPropertySet(Vec<NamedNode>),
}

impl fmt::Display for PropertyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PropertyPath::PredicatePath(p) => write!(f, "link({})", p),
            PropertyPath::InversePath(p) => write!(f, "inv({})", p),
            PropertyPath::AlternativePath(a, b) => write!(f, "alt({}, {})", a, b),
            PropertyPath::SequencePath(a, b) => write!(f, "seq({}, {})", a, b),
            PropertyPath::ZeroOrMorePath(p) => write!(f, "ZeroOrMorePath({})", p),
            PropertyPath::OneOrMorePath(p) => write!(f, "OneOrMorePath({})", p),
            PropertyPath::ZeroOrOnePath(p) => write!(f, "ZeroOrOnePath({})", p),
            PropertyPath::NegatedPropertySet(p) => write!(
                f,
                "NPS({{ {} }})",
                p.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(" ")
            ),
        }
    }
}

struct SparqlPropertyPath<'a>(&'a PropertyPath);

impl<'a> fmt::Display for SparqlPropertyPath<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            PropertyPath::PredicatePath(p) => write!(f, "{}", p),
            PropertyPath::InversePath(p) => write!(f, "^{}", SparqlPropertyPath(&*p)),
            PropertyPath::SequencePath(a, b) => write!(
                f,
                "({} / {})",
                SparqlPropertyPath(&*a),
                SparqlPropertyPath(&*b)
            ),
            PropertyPath::AlternativePath(a, b) => write!(
                f,
                "({} | {})",
                SparqlPropertyPath(&*a),
                SparqlPropertyPath(&*b)
            ),
            PropertyPath::ZeroOrMorePath(p) => write!(f, "{}*", SparqlPropertyPath(&*p)),
            PropertyPath::OneOrMorePath(p) => write!(f, "{}+", SparqlPropertyPath(&*p)),
            PropertyPath::ZeroOrOnePath(p) => write!(f, "{}?", SparqlPropertyPath(&*p)),
            PropertyPath::NegatedPropertySet(p) => write!(
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

impl From<NamedNode> for PropertyPath {
    fn from(p: NamedNode) -> Self {
        PropertyPath::PredicatePath(p)
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct PathPattern {
    pub subject: TermOrVariable,
    pub path: PropertyPath,
    pub object: TermOrVariable,
}

impl fmt::Display for PathPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Path({} {} {})", self.subject, self.path, self.object)
    }
}

impl PathPattern {
    pub fn new(
        subject: impl Into<TermOrVariable>,
        path: impl Into<PropertyPath>,
        object: impl Into<TermOrVariable>,
    ) -> Self {
        Self {
            subject: subject.into(),
            path: path.into(),
            object: object.into(),
        }
    }
}

struct SparqlPathPattern<'a>(&'a PathPattern);

impl<'a> fmt::Display for SparqlPathPattern<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} {}",
            self.0.subject,
            SparqlPropertyPath(&self.0.path),
            self.0.object
        )
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum TripleOrPathPattern {
    Triple(TriplePattern),
    Path(PathPattern),
}

impl<'a> fmt::Display for TripleOrPathPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TripleOrPathPattern::Triple(tp) => write!(f, "{}", tp),
            TripleOrPathPattern::Path(ppp) => write!(f, "{}", ppp),
        }
    }
}

impl From<TriplePattern> for TripleOrPathPattern {
    fn from(tp: TriplePattern) -> Self {
        TripleOrPathPattern::Triple(tp)
    }
}

impl From<PathPattern> for TripleOrPathPattern {
    fn from(ppp: PathPattern) -> Self {
        TripleOrPathPattern::Path(ppp)
    }
}

struct SparqlTripleOrPathPattern<'a>(&'a TripleOrPathPattern);

impl<'a> fmt::Display for SparqlTripleOrPathPattern<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            TripleOrPathPattern::Triple(tp) => write!(f, "{}", tp),
            TripleOrPathPattern::Path(ppp) => write!(f, "{}", SparqlPathPattern(&ppp)),
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum Expression {
    Constant(TermOrVariable),
    Or(Box<Expression>, Box<Expression>),
    And(Box<Expression>, Box<Expression>),
    Equal(Box<Expression>, Box<Expression>),
    NotEqual(Box<Expression>, Box<Expression>),
    Greater(Box<Expression>, Box<Expression>),
    GreaterOrEq(Box<Expression>, Box<Expression>),
    Lower(Box<Expression>, Box<Expression>),
    LowerOrEq(Box<Expression>, Box<Expression>),
    In(Box<Expression>, Vec<Expression>),
    NotIn(Box<Expression>, Vec<Expression>),
    Add(Box<Expression>, Box<Expression>),
    Sub(Box<Expression>, Box<Expression>),
    Mul(Box<Expression>, Box<Expression>),
    Div(Box<Expression>, Box<Expression>),
    UnaryPlus(Box<Expression>),
    UnaryMinus(Box<Expression>),
    UnaryNot(Box<Expression>),
    StrFunctionCall(Box<Expression>),
    LangFunctionCall(Box<Expression>),
    LangMatchesFunctionCall(Box<Expression>, Box<Expression>),
    DatatypeFunctionCall(Box<Expression>),
    BoundFunctionCall(Variable),
    IRIFunctionCall(Box<Expression>),
    BNodeFunctionCall(Option<Box<Expression>>),
    RandFunctionCall(),
    AbsFunctionCall(Box<Expression>),
    CeilFunctionCall(Box<Expression>),
    FloorFunctionCall(Box<Expression>),
    RoundFunctionCall(Box<Expression>),
    ConcatFunctionCall(Vec<Expression>),
    SubStrFunctionCall(Box<Expression>, Box<Expression>, Option<Box<Expression>>),
    StrLenFunctionCall(Box<Expression>),
    ReplaceFunctionCall(
        Box<Expression>,
        Box<Expression>,
        Box<Expression>,
        Option<Box<Expression>>,
    ),
    UCaseFunctionCall(Box<Expression>),
    LCaseFunctionCall(Box<Expression>),
    EncodeForURIFunctionCall(Box<Expression>),
    ContainsFunctionCall(Box<Expression>, Box<Expression>),
    StrStartsFunctionCall(Box<Expression>, Box<Expression>),
    StrEndsFunctionCall(Box<Expression>, Box<Expression>),
    StrBeforeFunctionCall(Box<Expression>, Box<Expression>),
    StrAfterFunctionCall(Box<Expression>, Box<Expression>),
    YearFunctionCall(Box<Expression>),
    MonthFunctionCall(Box<Expression>),
    DayFunctionCall(Box<Expression>),
    HoursFunctionCall(Box<Expression>),
    MinutesFunctionCall(Box<Expression>),
    SecondsFunctionCall(Box<Expression>),
    TimezoneFunctionCall(Box<Expression>),
    TzFunctionCall(Box<Expression>),
    NowFunctionCall(),
    UUIDFunctionCall(),
    StrUUIDFunctionCall(),
    MD5FunctionCall(Box<Expression>),
    SHA1FunctionCall(Box<Expression>),
    SHA256FunctionCall(Box<Expression>),
    SHA384FunctionCall(Box<Expression>),
    SHA512FunctionCall(Box<Expression>),
    CoalesceFunctionCall(Vec<Expression>),
    IfFunctionCall(Box<Expression>, Box<Expression>, Box<Expression>),
    StrLangFunctionCall(Box<Expression>, Box<Expression>),
    StrDTFunctionCall(Box<Expression>, Box<Expression>),
    SameTermFunctionCall(Box<Expression>, Box<Expression>),
    IsIRIFunctionCall(Box<Expression>),
    IsBlankFunctionCall(Box<Expression>),
    IsLiteralFunctionCall(Box<Expression>),
    IsNumericFunctionCall(Box<Expression>),
    RegexFunctionCall(Box<Expression>, Box<Expression>, Option<Box<Expression>>),
    CustomFunctionCall(NamedNode, Vec<Expression>),
    ExistsFunctionCall(Box<GraphPattern>),
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expression::Constant(t) => write!(f, "{}", t),
            Expression::Or(a, b) => write!(f, "({} || {})", a, b),
            Expression::And(a, b) => write!(f, "({} && {})", a, b),
            Expression::Equal(a, b) => write!(f, "({} = {})", a, b),
            Expression::NotEqual(a, b) => write!(f, "({} != {})", a, b),
            Expression::Greater(a, b) => write!(f, "({} > {})", a, b),
            Expression::GreaterOrEq(a, b) => write!(f, "({} >= {})", a, b),
            Expression::Lower(a, b) => write!(f, "({} < {})", a, b),
            Expression::LowerOrEq(a, b) => write!(f, "({} <= {})", a, b),
            Expression::In(a, b) => write!(
                f,
                "({} IN ({}))",
                a,
                b.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Expression::NotIn(a, b) => write!(
                f,
                "({} NOT IN ({}))",
                a,
                b.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Expression::Add(a, b) => write!(f, "{} + {}", a, b),
            Expression::Sub(a, b) => write!(f, "{} - {}", a, b),
            Expression::Mul(a, b) => write!(f, "{} * {}", a, b),
            Expression::Div(a, b) => write!(f, "{} / {}", a, b),
            Expression::UnaryPlus(e) => write!(f, "+{}", e),
            Expression::UnaryMinus(e) => write!(f, "-{}", e),
            Expression::UnaryNot(e) => write!(f, "!{}", e),
            Expression::StrFunctionCall(e) => write!(f, "STR({})", e),
            Expression::LangFunctionCall(e) => write!(f, "LANG({})", e),
            Expression::LangMatchesFunctionCall(a, b) => write!(f, "LANGMATCHES({}, {})", a, b),
            Expression::DatatypeFunctionCall(e) => write!(f, "DATATYPE({})", e),
            Expression::BoundFunctionCall(v) => write!(f, "BOUND({})", v),
            Expression::IRIFunctionCall(e) => write!(f, "IRI({})", e),
            Expression::BNodeFunctionCall(v) => v
                .as_ref()
                .map(|id| write!(f, "BNODE({})", id))
                .unwrap_or_else(|| write!(f, "BNODE()")),
            Expression::RandFunctionCall() => write!(f, "RAND()"),
            Expression::AbsFunctionCall(e) => write!(f, "ABS({})", e),
            Expression::CeilFunctionCall(e) => write!(f, "CEIL({})", e),
            Expression::FloorFunctionCall(e) => write!(f, "FLOOR({})", e),
            Expression::RoundFunctionCall(e) => write!(f, "ROUND({})", e),
            Expression::ConcatFunctionCall(e) => write!(
                f,
                "CONCAT({})",
                e.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Expression::SubStrFunctionCall(a, b, c) => c
                .as_ref()
                .map(|cv| write!(f, "SUBSTR({}, {}, {})", a, b, cv))
                .unwrap_or_else(|| write!(f, "SUBSTR({}, {})", a, b)),
            Expression::StrLenFunctionCall(e) => write!(f, "STRLEN({})", e),
            Expression::ReplaceFunctionCall(arg, pattern, replacement, flags) => match flags {
                Some(flags) => write!(
                    f,
                    "REPLACE({}, {}, {}, {})",
                    arg, pattern, replacement, flags
                ),
                None => write!(f, "REPLACE({}, {}, {})", arg, pattern, replacement),
            },
            Expression::UCaseFunctionCall(e) => write!(f, "UCASE({})", e),
            Expression::LCaseFunctionCall(e) => write!(f, "LCASE({})", e),
            Expression::EncodeForURIFunctionCall(e) => write!(f, "ENCODE_FOR_URI({})", e),
            Expression::ContainsFunctionCall(a, b) => write!(f, "CONTAINS({}, {})", a, b),
            Expression::StrStartsFunctionCall(a, b) => write!(f, "STRSTATS({}, {})", a, b),
            Expression::StrEndsFunctionCall(a, b) => write!(f, "STRENDS({}, {})", a, b),
            Expression::StrBeforeFunctionCall(a, b) => write!(f, "STRBEFORE({}, {})", a, b),
            Expression::StrAfterFunctionCall(a, b) => write!(f, "STRAFTER({}, {})", a, b),
            Expression::YearFunctionCall(e) => write!(f, "YEAR({})", e),
            Expression::MonthFunctionCall(e) => write!(f, "MONTH({})", e),
            Expression::DayFunctionCall(e) => write!(f, "DAY({})", e),
            Expression::HoursFunctionCall(e) => write!(f, "HOURS({})", e),
            Expression::MinutesFunctionCall(e) => write!(f, "MINUTES({})", e),
            Expression::SecondsFunctionCall(e) => write!(f, "SECONDS({})", e),
            Expression::TimezoneFunctionCall(e) => write!(f, "TIMEZONE({})", e),
            Expression::TzFunctionCall(e) => write!(f, "TZ({})", e),
            Expression::NowFunctionCall() => write!(f, "NOW()"),
            Expression::UUIDFunctionCall() => write!(f, "UUID()"),
            Expression::StrUUIDFunctionCall() => write!(f, "STRUUID()"),
            Expression::MD5FunctionCall(e) => write!(f, "MD5({})", e),
            Expression::SHA1FunctionCall(e) => write!(f, "SHA1({})", e),
            Expression::SHA256FunctionCall(e) => write!(f, "SHA256({})", e),
            Expression::SHA384FunctionCall(e) => write!(f, "SHA384({})", e),
            Expression::SHA512FunctionCall(e) => write!(f, "SHA512({})", e),
            Expression::CoalesceFunctionCall(e) => write!(
                f,
                "COALESCE({})",
                e.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Expression::IfFunctionCall(a, b, c) => write!(f, "IF({}, {}, {})", a, b, c),
            Expression::StrLangFunctionCall(a, b) => write!(f, "STRLANG({}, {})", a, b),
            Expression::StrDTFunctionCall(a, b) => write!(f, "STRDT({}, {})", a, b),
            Expression::SameTermFunctionCall(a, b) => write!(f, "sameTerm({}, {})", a, b),
            Expression::IsIRIFunctionCall(e) => write!(f, "isIRI({})", e),
            Expression::IsBlankFunctionCall(e) => write!(f, "isBLANK({})", e),
            Expression::IsLiteralFunctionCall(e) => write!(f, "isLITERAL({})", e),
            Expression::IsNumericFunctionCall(e) => write!(f, "isNUMERIC({})", e),
            Expression::RegexFunctionCall(text, pattern, flags) => match flags {
                Some(flags) => write!(f, "REGEX({}, {}, {})", text, pattern, flags),
                None => write!(f, "REGEX({}, {})", text, pattern),
            },
            Expression::CustomFunctionCall(iri, args) => write!(
                f,
                "{}({})",
                iri,
                args.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Expression::ExistsFunctionCall(p) => write!(f, "EXISTS {{ {} }}", p),
        }
    }
}

impl From<NamedNode> for Expression {
    fn from(p: NamedNode) -> Self {
        Expression::Constant(p.into())
    }
}

impl From<Literal> for Expression {
    fn from(p: Literal) -> Self {
        Expression::Constant(p.into())
    }
}

impl From<Variable> for Expression {
    fn from(v: Variable) -> Self {
        Expression::Constant(v.into())
    }
}

struct SparqlExpression<'a>(&'a Expression);

impl<'a> fmt::Display for SparqlExpression<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Expression::Constant(t) => write!(f, "{}", t),
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
            Expression::NotEqual(a, b) => write!(
                f,
                "({} != {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::Greater(a, b) => {
                write!(f, "({} > {})", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::GreaterOrEq(a, b) => write!(
                f,
                "({} >= {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::Lower(a, b) => {
                write!(f, "({} < {})", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::LowerOrEq(a, b) => write!(
                f,
                "({} <= {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::In(a, b) => write!(
                f,
                "({} IN ({}))",
                a,
                b.iter()
                    .map(|v| SparqlExpression(v).to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Expression::NotIn(a, b) => write!(
                f,
                "({} NOT IN ({}))",
                a,
                b.iter()
                    .map(|v| SparqlExpression(v).to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Expression::Add(a, b) => {
                write!(f, "{} + {}", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::Sub(a, b) => {
                write!(f, "{} - {}", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::Mul(a, b) => {
                write!(f, "{} * {}", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::Div(a, b) => {
                write!(f, "{} / {}", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::UnaryPlus(e) => write!(f, "+{}", SparqlExpression(&*e)),
            Expression::UnaryMinus(e) => write!(f, "-{}", SparqlExpression(&*e)),
            Expression::UnaryNot(e) => match e.as_ref() {
                Expression::ExistsFunctionCall(p) => {
                    write!(f, "NOT EXISTS {{ {} }}", SparqlGraphPattern(&*p))
                }
                e => write!(f, "!{}", e),
            },
            Expression::StrFunctionCall(e) => write!(f, "STR({})", SparqlExpression(&*e)),
            Expression::LangFunctionCall(e) => write!(f, "LANG({})", SparqlExpression(&*e)),
            Expression::LangMatchesFunctionCall(a, b) => write!(
                f,
                "LANGMATCHES({}, {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::DatatypeFunctionCall(e) => write!(f, "DATATYPE({})", SparqlExpression(&*e)),
            Expression::BoundFunctionCall(v) => write!(f, "BOUND({})", v),
            Expression::IRIFunctionCall(e) => write!(f, "IRI({})", SparqlExpression(&*e)),
            Expression::BNodeFunctionCall(v) => v
                .as_ref()
                .map(|id| write!(f, "BOUND({})", SparqlExpression(&*id)))
                .unwrap_or_else(|| write!(f, "BOUND()")),
            Expression::RandFunctionCall() => write!(f, "RAND()"),
            Expression::AbsFunctionCall(e) => write!(f, "ABS({})", SparqlExpression(&*e)),
            Expression::CeilFunctionCall(e) => write!(f, "CEIL({})", SparqlExpression(&*e)),
            Expression::FloorFunctionCall(e) => write!(f, "FLOOR({})", SparqlExpression(&*e)),
            Expression::RoundFunctionCall(e) => write!(f, "ROUND({})", SparqlExpression(&*e)),
            Expression::ConcatFunctionCall(e) => write!(
                f,
                "CONCAT({})",
                e.iter()
                    .map(|v| SparqlExpression(v).to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Expression::SubStrFunctionCall(a, b, c) => c
                .as_ref()
                .map(|cv| {
                    write!(
                        f,
                        "SUBSTR({}, {}, {})",
                        SparqlExpression(&*a),
                        SparqlExpression(&*b),
                        SparqlExpression(cv)
                    )
                })
                .unwrap_or_else(|| {
                    write!(
                        f,
                        "SUBSTR({}, {})",
                        SparqlExpression(&*a),
                        SparqlExpression(&*b)
                    )
                }),
            Expression::StrLenFunctionCall(e) => write!(f, "STRLEN({})", SparqlExpression(&*e)),
            Expression::ReplaceFunctionCall(arg, pattern, replacement, flags) => match flags {
                Some(flags) => write!(
                    f,
                    "REPLACE({}, {}, {}, {})",
                    SparqlExpression(&*arg),
                    SparqlExpression(&*pattern),
                    SparqlExpression(&*replacement),
                    flags
                ),
                None => write!(
                    f,
                    "REPLACE({}, {}, {})",
                    SparqlExpression(&*arg),
                    SparqlExpression(&*pattern),
                    SparqlExpression(&*replacement)
                ),
            },
            Expression::UCaseFunctionCall(e) => write!(f, "UCASE({})", SparqlExpression(&*e)),
            Expression::LCaseFunctionCall(e) => write!(f, "LCASE({})", SparqlExpression(&*e)),
            Expression::EncodeForURIFunctionCall(e) => {
                write!(f, "ENCODE_FOR_URI({})", SparqlExpression(&*e))
            }
            Expression::ContainsFunctionCall(a, b) => write!(
                f,
                "CONTAINS({}, {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::StrStartsFunctionCall(a, b) => write!(
                f,
                "STRSTATS({}, {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::StrEndsFunctionCall(a, b) => write!(
                f,
                "STRENDS({}, {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::StrBeforeFunctionCall(a, b) => write!(
                f,
                "STRBEFORE({}, {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::StrAfterFunctionCall(a, b) => write!(
                f,
                "STRAFTER({}, {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::YearFunctionCall(e) => write!(f, "YEAR({})", SparqlExpression(&*e)),
            Expression::MonthFunctionCall(e) => write!(f, "MONTH({})", SparqlExpression(&*e)),
            Expression::DayFunctionCall(e) => write!(f, "DAY({})", SparqlExpression(&*e)),
            Expression::HoursFunctionCall(e) => write!(f, "HOURS({})", SparqlExpression(&*e)),
            Expression::MinutesFunctionCall(e) => write!(f, "MINUTES({})", SparqlExpression(&*e)),
            Expression::SecondsFunctionCall(e) => write!(f, "SECONDS({})", SparqlExpression(&*e)),
            Expression::TimezoneFunctionCall(e) => write!(f, "TIMEZONE({})", SparqlExpression(&*e)),
            Expression::TzFunctionCall(e) => write!(f, "TZ({})", SparqlExpression(&*e)),
            Expression::NowFunctionCall() => write!(f, "NOW()"),
            Expression::UUIDFunctionCall() => write!(f, "UUID()"),
            Expression::StrUUIDFunctionCall() => write!(f, "STRUUID()"),
            Expression::MD5FunctionCall(e) => write!(f, "MD5({})", SparqlExpression(&*e)),
            Expression::SHA1FunctionCall(e) => write!(f, "SHA1({})", SparqlExpression(&*e)),
            Expression::SHA256FunctionCall(e) => write!(f, "SHA256({})", SparqlExpression(&*e)),
            Expression::SHA384FunctionCall(e) => write!(f, "SHA384({})", SparqlExpression(&*e)),
            Expression::SHA512FunctionCall(e) => write!(f, "SHA512({})", SparqlExpression(&*e)),
            Expression::CoalesceFunctionCall(e) => write!(
                f,
                "COALESCE({})",
                e.iter()
                    .map(|v| SparqlExpression(&*v).to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Expression::IfFunctionCall(a, b, c) => write!(
                f,
                "IF({}, {}, {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b),
                SparqlExpression(&*c)
            ),
            Expression::StrLangFunctionCall(a, b) => write!(
                f,
                "STRLANG({}, {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::StrDTFunctionCall(a, b) => write!(
                f,
                "STRDT({}, {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::SameTermFunctionCall(a, b) => write!(
                f,
                "sameTerm({}, {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::IsIRIFunctionCall(e) => write!(f, "isIRI({})", SparqlExpression(&*e)),
            Expression::IsBlankFunctionCall(e) => write!(f, "isBLANK({})", SparqlExpression(&*e)),
            Expression::IsLiteralFunctionCall(e) => {
                write!(f, "isLITERAL({})", SparqlExpression(&*e))
            }
            Expression::IsNumericFunctionCall(e) => {
                write!(f, "isNUMERIC({})", SparqlExpression(&*e))
            }
            Expression::RegexFunctionCall(text, pattern, flags) => match flags {
                Some(flags) => write!(
                    f,
                    "REGEX({}, {}, {})",
                    SparqlExpression(&*text),
                    SparqlExpression(&*pattern),
                    flags
                ),
                None => write!(
                    f,
                    "REGEX({}, {})",
                    SparqlExpression(&*text),
                    SparqlExpression(&*pattern)
                ),
            },
            Expression::CustomFunctionCall(iri, args) => write!(
                f,
                "{}({})",
                iri,
                args.iter()
                    .map(|v| SparqlExpression(v).to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Expression::ExistsFunctionCall(p) => {
                write!(f, "EXISTS {{ {} }}", SparqlGraphPattern(&*p))
            }
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum GraphPattern {
    BGP(Vec<TripleOrPathPattern>),
    Join(Box<GraphPattern>, Box<GraphPattern>),
    LeftJoin(Box<GraphPattern>, Box<GraphPattern>, Expression),
    Filter(Expression, Box<GraphPattern>),
    Union(Box<GraphPattern>, Box<GraphPattern>),
    Graph(NamedNodeOrVariable, Box<GraphPattern>),
    Extend(Box<GraphPattern>, Variable, Expression),
    Minus(Box<GraphPattern>, Box<GraphPattern>),
    Service(NamedNodeOrVariable, Box<GraphPattern>, bool),
    AggregateJoin(GroupPattern, BTreeMap<Aggregation, Variable>),
    Data(StaticBindings),
    OrderBy(Box<GraphPattern>, Vec<OrderComparator>),
    Project(Box<GraphPattern>, Vec<Variable>),
    Distinct(Box<GraphPattern>),
    Reduced(Box<GraphPattern>),
    Slice(Box<GraphPattern>, usize, Option<usize>),
}

impl fmt::Display for GraphPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphPattern::BGP(p) => write!(
                f,
                "BGP({})",
                p.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(" . ")
            ),
            GraphPattern::Join(a, b) => write!(f, "Join({}, {})", a, b),
            GraphPattern::LeftJoin(a, b, e) => write!(f, "LeftJoin({}, {}, {})", a, b, e),
            GraphPattern::Filter(e, p) => write!(f, "Filter({}, {})", e, p),
            GraphPattern::Union(a, b) => write!(f, "Union({}, {})", a, b),
            GraphPattern::Graph(g, p) => write!(f, "Graph({}, {})", g, p),
            GraphPattern::Extend(p, v, e) => write!(f, "Extend({}), {}, {})", p, v, e),
            GraphPattern::Minus(a, b) => write!(f, "Minus({}, {})", a, b),
            GraphPattern::Service(n, p, s) => write!(f, "Service({}, {}, {})", n, p, s),
            GraphPattern::AggregateJoin(g, a) => write!(
                f,
                "AggregateJoin({}, {})",
                g,
                a.iter()
                    .map(|(a, v)| format!("{}: {}", v, a))
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            GraphPattern::Data(bs) => {
                let variables = bs.variables();
                write!(f, "{{ ")?;
                for values in bs.values_iter() {
                    write!(f, "{{")?;
                    for i in 0..values.len() {
                        if let Some(ref val) = values[i] {
                            write!(f, " {} → {} ", variables[i], val)?;
                        }
                    }
                    write!(f, "}}")?;
                }
                write!(f, "}}")
            }
            GraphPattern::OrderBy(l, o) => write!(
                f,
                "OrderBy({}, ({}))",
                l,
                o.iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            GraphPattern::Project(l, pv) => write!(
                f,
                "Project({}, ({}))",
                l,
                pv.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            GraphPattern::Distinct(l) => write!(f, "Distinct({})", l),
            GraphPattern::Reduced(l) => write!(f, "Reduce({})", l),
            GraphPattern::Slice(l, start, length) => write!(
                f,
                "Slice({}, {}, {})",
                l,
                start,
                length
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| '?'.to_string())
            ),
        }
    }
}

impl Default for GraphPattern {
    fn default() -> Self {
        GraphPattern::BGP(Vec::default())
    }
}

impl From<TripleOrPathPattern> for GraphPattern {
    fn from(p: TripleOrPathPattern) -> Self {
        GraphPattern::BGP(vec![p])
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
                    match pattern {
                        TripleOrPathPattern::Triple(tp) => {
                            if let TermOrVariable::Variable(ref s) = tp.subject {
                                adds_if_has_name(vars, s);
                            }
                            if let NamedNodeOrVariable::Variable(ref p) = tp.predicate {
                                adds_if_has_name(vars, p);
                            }
                            if let TermOrVariable::Variable(ref o) = tp.object {
                                adds_if_has_name(vars, o);
                            }
                        }
                        TripleOrPathPattern::Path(ppp) => {
                            if let TermOrVariable::Variable(ref s) = ppp.subject {
                                adds_if_has_name(vars, s);
                            }
                            if let TermOrVariable::Variable(ref o) = ppp.object {
                                adds_if_has_name(vars, o);
                            }
                        }
                    }
                }
            }
            GraphPattern::Join(a, b) => {
                a.add_visible_variables(vars);
                b.add_visible_variables(vars);
            }
            GraphPattern::LeftJoin(a, b, _) => {
                a.add_visible_variables(vars);
                b.add_visible_variables(vars);
            }
            GraphPattern::Filter(_, p) => p.add_visible_variables(vars),
            GraphPattern::Union(a, b) => {
                a.add_visible_variables(vars);
                b.add_visible_variables(vars);
            }
            GraphPattern::Graph(g, p) => {
                if let NamedNodeOrVariable::Variable(ref g) = g {
                    adds_if_has_name(vars, g);
                }
                p.add_visible_variables(vars);
            }
            GraphPattern::Extend(p, v, _) => {
                p.add_visible_variables(vars);
                adds_if_has_name(vars, &v);
            }
            GraphPattern::Minus(a, _) => a.add_visible_variables(vars),
            GraphPattern::Service(_, p, _) => p.add_visible_variables(vars),
            GraphPattern::AggregateJoin(_, a) => vars.extend(a.iter().map(|(_, v)| v)),
            GraphPattern::Data(b) => vars.extend(b.variables_iter()),
            GraphPattern::OrderBy(l, _) => l.add_visible_variables(vars),
            GraphPattern::Project(_, pv) => vars.extend(pv.iter()),
            GraphPattern::Distinct(l) => l.add_visible_variables(vars),
            GraphPattern::Reduced(l) => l.add_visible_variables(vars),
            GraphPattern::Slice(l, _, _) => l.add_visible_variables(vars),
        }
    }
}

fn adds_if_has_name<'a>(vars: &mut BTreeSet<&'a Variable>, var: &'a Variable) {
    if var.has_name() {
        vars.insert(var);
    }
}

struct SparqlGraphPattern<'a>(&'a GraphPattern);

impl<'a> fmt::Display for SparqlGraphPattern<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            GraphPattern::BGP(p) => {
                for pattern in p {
                    write!(f, "{} .", SparqlTripleOrPathPattern(pattern))?
                }
                Ok(())
            }
            GraphPattern::Join(a, b) => write!(
                f,
                "{{ {} }} {{ {} }}",
                SparqlGraphPattern(&*a),
                SparqlGraphPattern(&*b)
            ),
            GraphPattern::LeftJoin(a, b, e) => write!(
                f,
                "{} OPTIONAL {{ {} FILTER({}) }}",
                SparqlGraphPattern(&*a),
                SparqlGraphPattern(&*b),
                SparqlExpression(e)
            ),
            GraphPattern::Filter(e, p) => write!(
                f,
                "{} FILTER({})",
                SparqlGraphPattern(&*p),
                SparqlExpression(e)
            ),
            GraphPattern::Union(a, b) => write!(
                f,
                "{{ {} }} UNION {{ {} }}",
                SparqlGraphPattern(&*a),
                SparqlGraphPattern(&*b),
            ),
            GraphPattern::Graph(g, p) => {
                write!(f, "GRAPH {} {{ {} }}", g, SparqlGraphPattern(&*p),)
            }
            GraphPattern::Extend(p, v, e) => write!(
                f,
                "{} BIND({} AS {})",
                SparqlGraphPattern(&*p),
                SparqlExpression(e),
                v
            ),
            GraphPattern::Minus(a, b) => write!(
                f,
                "{} MINUS {{ {} }}",
                SparqlGraphPattern(&*a),
                SparqlGraphPattern(&*b)
            ),
            GraphPattern::Service(n, p, s) => {
                if *s {
                    write!(f, "SERVICE SILENT {} {{ {} }}", n, SparqlGraphPattern(&*p))
                } else {
                    write!(f, "SERVICE {} {{ {} }}", n, SparqlGraphPattern(&*p))
                }
            }
            GraphPattern::Data(bs) => {
                if bs.is_empty() {
                    Ok(())
                } else {
                    write!(f, "VALUES ( ")?;
                    for var in bs.variables() {
                        write!(f, "{} ", var)?;
                    }
                    write!(f, ") {{ ")?;
                    for values in bs.values_iter() {
                        write!(f, "( ")?;
                        for val in values {
                            match val {
                                Some(val) => write!(f, "{} ", val),
                                None => write!(f, "UNDEF "),
                            }?;
                        }
                        write!(f, ") ")?;
                    }
                    write!(f, " }}")
                }
            }
            GraphPattern::AggregateJoin(GroupPattern(group, p), agg) => write!(
                f,
                "{{ SELECT {} WHERE {{ {} }} GROUP BY {} }}",
                agg.iter()
                    .map(|(a, v)| format!("({} AS {})", SparqlAggregation(&a), v))
                    .collect::<Vec<String>>()
                    .join(" "),
                SparqlGraphPattern(&*p),
                group
                    .iter()
                    .map(|e| format!("({})", e.to_string()))
                    .collect::<Vec<String>>()
                    .join(" ")
            ),
            p => write!(
                f,
                "{{ {} }}",
                SparqlGraphRootPattern {
                    algebra: p,
                    dataset: &EMPTY_DATASET
                }
            ),
        }
    }
}

struct SparqlGraphRootPattern<'a> {
    algebra: &'a GraphPattern,
    dataset: &'a DatasetSpec,
}

impl<'a> fmt::Display for SparqlGraphRootPattern<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut distinct = false;
        let mut reduced = false;
        let mut order = None;
        let mut start = 0;
        let mut length = None;
        let mut project: &[Variable] = &[];

        let mut child = self.algebra;
        loop {
            match child {
                GraphPattern::OrderBy(l, o) => {
                    order = Some(o);
                    child = &*l;
                }
                GraphPattern::Project(l, pv) if project.is_empty() => {
                    project = pv;
                    child = &*l;
                }
                GraphPattern::Distinct(l) => {
                    distinct = true;
                    child = &*l;
                }
                GraphPattern::Reduced(l) => {
                    reduced = true;
                    child = &*l;
                }
                GraphPattern::Slice(l, s, len) => {
                    start = *s;
                    length = *len;
                    child = l;
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
                        "{} {} WHERE {{ {} }}",
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

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct GroupPattern(pub Vec<Expression>, pub Box<GraphPattern>);

impl fmt::Display for GroupPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Group(({}), {})",
            self.0
                .iter()
                .map(|c| c.to_string())
                .collect::<Vec<String>>()
                .join(", "),
            self.1
        )
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

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum Aggregation {
    Count(Option<Box<Expression>>, bool),
    Sum(Box<Expression>, bool),
    Min(Box<Expression>, bool),
    Max(Box<Expression>, bool),
    Avg(Box<Expression>, bool),
    Sample(Box<Expression>, bool),
    GroupConcat(Box<Expression>, bool, Option<String>),
}

impl fmt::Display for Aggregation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Aggregation::Count(e, distinct) => {
                if *distinct {
                    e.as_ref()
                        .map(|ex| write!(f, "COUNT(DISTINCT {})", ex))
                        .unwrap_or_else(|| write!(f, "COUNT(DISTINCT *)"))
                } else {
                    e.as_ref()
                        .map(|ex| write!(f, "COUNT({})", ex))
                        .unwrap_or_else(|| write!(f, "COUNT(*)"))
                }
            }
            Aggregation::Sum(e, distinct) => {
                if *distinct {
                    write!(f, "Aggregation(Distinct({}), Sum, {{}})", e)
                } else {
                    write!(f, "Aggregation({}, Sum, {{}})", e)
                }
            }
            Aggregation::Min(e, distinct) => {
                if *distinct {
                    write!(f, "Aggregation(Distinct({}), Min, {{}})", e)
                } else {
                    write!(f, "Aggregation({}, Min, {{}})", e)
                }
            }
            Aggregation::Max(e, distinct) => {
                if *distinct {
                    write!(f, "Aggregation(Distinct({}), Max, {{}})", e)
                } else {
                    write!(f, "Aggregation({}, Max, {{}})", e)
                }
            }
            Aggregation::Avg(e, distinct) => {
                if *distinct {
                    write!(f, "Aggregation(Distinct({}), Avg, {{}})", e)
                } else {
                    write!(f, "Aggregation({}, Avg, {{}})", e)
                }
            }
            Aggregation::Sample(e, distinct) => {
                if *distinct {
                    write!(f, "Aggregation(Distinct({}), Sum, {{}})", e)
                } else {
                    write!(f, "Aggregation({}, Sample, {{}})", e)
                }
            }
            Aggregation::GroupConcat(e, distinct, sep) => {
                if *distinct {
                    sep.as_ref()
                        .map(|s| {
                            write!(
                            f,
                            "Aggregation(Distinct({}), GroupConcat, {{\"separator\" → \"{}\"}})",
                            e,
                            s.escape()
                        )
                        })
                        .unwrap_or_else(|| {
                            write!(f, "Aggregation(Distinct({}), GroupConcat, {{}})", e)
                        })
                } else {
                    sep.as_ref()
                        .map(|s| {
                            write!(
                                f,
                                "Aggregation({}, GroupConcat, {{\"separator\" → \"{}\"}})",
                                e,
                                s.escape()
                            )
                        })
                        .unwrap_or_else(|| {
                            write!(f, "Aggregation(Distinct({}), GroupConcat, {{}})", e)
                        })
                }
            }
        }
    }
}

struct SparqlAggregation<'a>(&'a Aggregation);

impl<'a> fmt::Display for SparqlAggregation<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            Aggregation::Count(e, distinct) => {
                if *distinct {
                    if let Some(e) = e {
                        write!(f, "COUNT(DISTINCT {})", SparqlExpression(e))
                    } else {
                        write!(f, "COUNT(DISTINCT *)")
                    }
                } else if let Some(e) = e {
                    write!(f, "COUNT({})", SparqlExpression(e))
                } else {
                    write!(f, "COUNT(*)")
                }
            }
            Aggregation::Sum(e, distinct) => {
                if *distinct {
                    write!(f, "SUM(DISTINCT {})", SparqlExpression(e))
                } else {
                    write!(f, "SUM({})", SparqlExpression(e))
                }
            }
            Aggregation::Min(e, distinct) => {
                if *distinct {
                    write!(f, "MIN(DISTINCT {})", SparqlExpression(e))
                } else {
                    write!(f, "MIN({})", SparqlExpression(e))
                }
            }
            Aggregation::Max(e, distinct) => {
                if *distinct {
                    write!(f, "MAX(DISTINCT {})", SparqlExpression(e))
                } else {
                    write!(f, "MAX({})", SparqlExpression(e))
                }
            }
            Aggregation::Avg(e, distinct) => {
                if *distinct {
                    write!(f, "AVG(DISTINCT {})", SparqlExpression(e))
                } else {
                    write!(f, "AVG({})", SparqlExpression(e))
                }
            }
            Aggregation::Sample(e, distinct) => {
                if *distinct {
                    write!(f, "SAMPLE(DISTINCT {})", SparqlExpression(e))
                } else {
                    write!(f, "SAMPLE({})", SparqlExpression(e))
                }
            }
            Aggregation::GroupConcat(e, distinct, sep) => {
                if *distinct {
                    if let Some(sep) = sep {
                        write!(
                            f,
                            "GROUP_CONCAT(DISTINCT {}; SEPARATOR = \"{}\")",
                            SparqlExpression(e),
                            sep.escape()
                        )
                    } else {
                        write!(f, "GROUP_CONCAT(DISTINCT {})", SparqlExpression(e))
                    }
                } else if let Some(sep) = sep {
                    write!(
                        f,
                        "GROUP_CONCAT({}; SEPARATOR = \"{}\")",
                        SparqlExpression(e),
                        sep.escape()
                    )
                } else {
                    write!(f, "GROUP_CONCAT({})", SparqlExpression(e))
                }
            }
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum OrderComparator {
    Asc(Expression),
    Desc(Expression),
}

impl fmt::Display for OrderComparator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OrderComparator::Asc(e) => write!(f, "ASC({})", e),
            OrderComparator::Desc(e) => write!(f, "DESC({})", e),
        }
    }
}

impl From<Expression> for OrderComparator {
    fn from(e: Expression) -> Self {
        OrderComparator::Asc(e)
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

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash, Default)]
pub struct DatasetSpec {
    pub default: Vec<NamedNode>,
    pub named: Vec<NamedNode>,
}

impl DatasetSpec {
    pub fn new_with_default(graph: NamedNode) -> Self {
        Self {
            default: vec![graph],
            named: Vec::default(),
        }
    }

    pub fn new_with_named(graph: NamedNode) -> Self {
        Self {
            default: Vec::default(),
            named: vec![graph],
        }
    }
}

impl Add for DatasetSpec {
    type Output = Self;

    fn add(mut self, rhs: Self) -> Self {
        self.default.extend_from_slice(&rhs.default);
        self.named.extend_from_slice(&rhs.named);
        self
    }
}

impl fmt::Display for DatasetSpec {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for g in &self.default {
            write!(f, "FROM {} ", g)?;
        }
        for g in &self.named {
            write!(f, "FROM NAMED {} ", g)?;
        }
        Ok(())
    }
}

lazy_static! {
    static ref EMPTY_DATASET: DatasetSpec = DatasetSpec::default();
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum Query {
    Select {
        dataset: DatasetSpec,
        algebra: GraphPattern,
    },
    Construct {
        construct: Vec<TriplePattern>,
        dataset: DatasetSpec,
        algebra: GraphPattern,
    },
    Describe {
        dataset: DatasetSpec,
        algebra: GraphPattern,
    },
    Ask {
        dataset: DatasetSpec,
        algebra: GraphPattern,
    },
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Query::Select { dataset, algebra } => write!(
                f,
                "{}",
                SparqlGraphRootPattern {
                    algebra: &algebra,
                    dataset: &dataset
                }
            ),
            Query::Construct {
                construct,
                dataset,
                algebra,
            } => write!(
                f,
                "CONSTRUCT {{ {} }} {} WHERE {{ {} }}",
                construct
                    .iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<String>>()
                    .join(" . "),
                dataset,
                SparqlGraphRootPattern {
                    algebra: &algebra,
                    dataset: &EMPTY_DATASET
                }
            ),
            Query::Describe { dataset, algebra } => write!(
                f,
                "DESCRIBE * {} WHERE {{ {} }}",
                dataset,
                SparqlGraphRootPattern {
                    algebra: &algebra,
                    dataset: &EMPTY_DATASET
                }
            ),
            Query::Ask { dataset, algebra } => write!(
                f,
                "ASK {} WHERE {{ {} }}",
                dataset,
                SparqlGraphRootPattern {
                    algebra: &algebra,
                    dataset: &EMPTY_DATASET
                }
            ),
        }
    }
}

pub enum QueryResult<'a> {
    Bindings(BindingsIterator<'a>),
    Boolean(bool),
    Graph(Box<dyn Iterator<Item = Result<Triple>> + 'a>),
}
