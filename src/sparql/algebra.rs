use model::*;
use sparql::model::*;
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt;
use std::ops::Add;
use utils::Escaper;
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
}

impl fmt::Display for Variable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
        Variable::BlankNode { id: *blank_node }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum NamedNodeOrVariable {
    NamedNode(NamedNode),
    Variable(Variable),
}

impl fmt::Display for NamedNodeOrVariable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
    NamedNode(NamedNode),
    Literal(Literal),
    Variable(Variable),
}

impl fmt::Display for TermOrVariable {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TermOrVariable::NamedNode(node) => write!(f, "{}", node),
            TermOrVariable::Literal(node) => write!(f, "{}", node),
            TermOrVariable::Variable(var) => write!(f, "{}", var),
        }
    }
}

impl From<NamedNode> for TermOrVariable {
    fn from(node: NamedNode) -> Self {
        TermOrVariable::NamedNode(node)
    }
}

impl From<BlankNode> for TermOrVariable {
    fn from(node: BlankNode) -> Self {
        TermOrVariable::Variable(node.into())
    }
}

impl From<Literal> for TermOrVariable {
    fn from(literal: Literal) -> Self {
        TermOrVariable::Literal(literal)
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
            Term::NamedNode(node) => TermOrVariable::NamedNode(node),
            Term::BlankNode(node) => TermOrVariable::Variable(node.into()),
            Term::Literal(literal) => TermOrVariable::Literal(literal),
        }
    }
}

impl From<NamedNodeOrVariable> for TermOrVariable {
    fn from(element: NamedNodeOrVariable) -> Self {
        match element {
            NamedNodeOrVariable::NamedNode(node) => TermOrVariable::NamedNode(node),
            NamedNodeOrVariable::Variable(var) => TermOrVariable::Variable(var),
        }
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            TripleOrPathPattern::Triple(tp) => write!(f, "{}", tp),
            TripleOrPathPattern::Path(ppp) => write!(f, "{}", SparqlPathPattern(&ppp)),
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum Expression {
    ConstantExpression(TermOrVariable),
    OrExpression(Box<Expression>, Box<Expression>),
    AndExpression(Box<Expression>, Box<Expression>),
    EqualExpression(Box<Expression>, Box<Expression>),
    NotEqualExpression(Box<Expression>, Box<Expression>),
    GreaterExpression(Box<Expression>, Box<Expression>),
    GreaterOrEqExpression(Box<Expression>, Box<Expression>),
    LowerExpression(Box<Expression>, Box<Expression>),
    LowerOrEqExpression(Box<Expression>, Box<Expression>),
    InExpression(Box<Expression>, Vec<Expression>),
    NotInExpression(Box<Expression>, Vec<Expression>),
    AddExpression(Box<Expression>, Box<Expression>),
    SubExpression(Box<Expression>, Box<Expression>),
    MulExpression(Box<Expression>, Box<Expression>),
    DivExpression(Box<Expression>, Box<Expression>),
    UnaryPlusExpression(Box<Expression>),
    UnaryMinusExpression(Box<Expression>),
    UnaryNotExpression(Box<Expression>),
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
    ExistsFunctionCall(Box<MultiSetPattern>),
}

impl fmt::Display for Expression {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Expression::ConstantExpression(t) => write!(f, "{}", t),
            Expression::OrExpression(a, b) => write!(f, "({} || {})", a, b),
            Expression::AndExpression(a, b) => write!(f, "({} && {})", a, b),
            Expression::EqualExpression(a, b) => write!(f, "({} = {})", a, b),
            Expression::NotEqualExpression(a, b) => write!(f, "({} != {})", a, b),
            Expression::GreaterExpression(a, b) => write!(f, "({} > {})", a, b),
            Expression::GreaterOrEqExpression(a, b) => write!(f, "({} >= {})", a, b),
            Expression::LowerExpression(a, b) => write!(f, "({} < {})", a, b),
            Expression::LowerOrEqExpression(a, b) => write!(f, "({} <= {})", a, b),
            Expression::InExpression(a, b) => write!(
                f,
                "({} IN ({}))",
                a,
                b.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Expression::NotInExpression(a, b) => write!(
                f,
                "({} NOT IN ({}))",
                a,
                b.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Expression::AddExpression(a, b) => write!(f, "{} + {}", a, b),
            Expression::SubExpression(a, b) => write!(f, "{} - {}", a, b),
            Expression::MulExpression(a, b) => write!(f, "{} * {}", a, b),
            Expression::DivExpression(a, b) => write!(f, "{} / {}", a, b),
            Expression::UnaryPlusExpression(e) => write!(f, "+{}", e),
            Expression::UnaryMinusExpression(e) => write!(f, "-{}", e),
            Expression::UnaryNotExpression(e) => write!(f, "!{}", e),
            Expression::StrFunctionCall(e) => write!(f, "STR({})", e),
            Expression::LangFunctionCall(e) => write!(f, "LANG({})", e),
            Expression::LangMatchesFunctionCall(a, b) => write!(f, "LANGMATCHES({}, {})", a, b),
            Expression::DatatypeFunctionCall(e) => write!(f, "DATATYPE({})", e),
            Expression::BoundFunctionCall(v) => write!(f, "BOUND({})", v),
            Expression::IRIFunctionCall(e) => write!(f, "IRI({})", e),
            Expression::BNodeFunctionCall(v) => v
                .as_ref()
                .map(|id| write!(f, "BOUND({})", id))
                .unwrap_or_else(|| write!(f, "BOUND()")),
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
            Expression::ReplaceFunctionCall(a, b, c, d) => d
                .as_ref()
                .map(|dv| write!(f, "REPLACE({}, {}, {}, {})", a, b, c, dv))
                .unwrap_or_else(|| write!(f, "REPLACE({}, {}, {})", a, b, c)),
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
            Expression::RegexFunctionCall(a, b, c) => c
                .as_ref()
                .map(|cv| write!(f, "REGEX({}, {}, {})", a, b, cv))
                .unwrap_or_else(|| write!(f, "REGEX({}, {})", a, b)),
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
        Expression::ConstantExpression(p.into())
    }
}

impl From<Literal> for Expression {
    fn from(p: Literal) -> Self {
        Expression::ConstantExpression(p.into())
    }
}

impl From<Variable> for Expression {
    fn from(v: Variable) -> Self {
        Expression::ConstantExpression(v.into())
    }
}

struct SparqlExpression<'a>(&'a Expression);

impl<'a> fmt::Display for SparqlExpression<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Expression::ConstantExpression(t) => write!(f, "{}", t),
            Expression::OrExpression(a, b) => write!(
                f,
                "({} || {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::AndExpression(a, b) => write!(
                f,
                "({} && {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::EqualExpression(a, b) => {
                write!(f, "({} = {})", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::NotEqualExpression(a, b) => write!(
                f,
                "({} != {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::GreaterExpression(a, b) => {
                write!(f, "({} > {})", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::GreaterOrEqExpression(a, b) => write!(
                f,
                "({} >= {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::LowerExpression(a, b) => {
                write!(f, "({} < {})", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::LowerOrEqExpression(a, b) => write!(
                f,
                "({} <= {})",
                SparqlExpression(&*a),
                SparqlExpression(&*b)
            ),
            Expression::InExpression(a, b) => write!(
                f,
                "({} IN ({}))",
                a,
                b.iter()
                    .map(|v| SparqlExpression(v).to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Expression::NotInExpression(a, b) => write!(
                f,
                "({} NOT IN ({}))",
                a,
                b.iter()
                    .map(|v| SparqlExpression(v).to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Expression::AddExpression(a, b) => {
                write!(f, "{} + {}", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::SubExpression(a, b) => {
                write!(f, "{} - {}", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::MulExpression(a, b) => {
                write!(f, "{} * {}", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::DivExpression(a, b) => {
                write!(f, "{} / {}", SparqlExpression(&*a), SparqlExpression(&*b))
            }
            Expression::UnaryPlusExpression(e) => write!(f, "+{}", SparqlExpression(&*e)),
            Expression::UnaryMinusExpression(e) => write!(f, "-{}", SparqlExpression(&*e)),
            Expression::UnaryNotExpression(e) => match e.as_ref() {
                Expression::ExistsFunctionCall(p) => {
                    write!(f, "NOT EXISTS {{ {} }}", SparqlMultiSetPattern(&*p))
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
                }).unwrap_or_else(|| {
                    write!(
                        f,
                        "SUBSTR({}, {})",
                        SparqlExpression(&*a),
                        SparqlExpression(&*b)
                    )
                }),
            Expression::StrLenFunctionCall(e) => write!(f, "STRLEN({})", SparqlExpression(&*e)),
            Expression::ReplaceFunctionCall(a, b, c, d) => d
                .as_ref()
                .map(|dv| {
                    write!(
                        f,
                        "REPLACE({}, {}, {}, {})",
                        SparqlExpression(&*a),
                        SparqlExpression(&*b),
                        SparqlExpression(&*c),
                        dv
                    )
                }).unwrap_or_else(|| {
                    write!(
                        f,
                        "REPLACE({}, {}, {})",
                        SparqlExpression(&*a),
                        SparqlExpression(&*b),
                        SparqlExpression(&*c)
                    )
                }),
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
            Expression::RegexFunctionCall(a, b, c) => c
                .as_ref()
                .map(|cv| {
                    write!(
                        f,
                        "REGEX({}, {}, {})",
                        SparqlExpression(&*a),
                        SparqlExpression(&*b),
                        cv
                    )
                }).unwrap_or_else(|| {
                    write!(
                        f,
                        "REGEX({}, {})",
                        SparqlExpression(&*a),
                        SparqlExpression(&*b)
                    )
                }),
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
                write!(f, "EXISTS {{ {} }}", SparqlMultiSetPattern(&*p))
            }
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum MultiSetPattern {
    BGP(Vec<TripleOrPathPattern>),
    Join(Box<MultiSetPattern>, Box<MultiSetPattern>),
    LeftJoin(Box<MultiSetPattern>, Box<MultiSetPattern>, Expression),
    Filter(Expression, Box<MultiSetPattern>),
    Union(Box<MultiSetPattern>, Box<MultiSetPattern>),
    Graph(NamedNodeOrVariable, Box<MultiSetPattern>),
    Extend(Box<MultiSetPattern>, Variable, Expression),
    Minus(Box<MultiSetPattern>, Box<MultiSetPattern>),
    ToMultiSet(Box<ListPattern>),
    Service(NamedNodeOrVariable, Box<MultiSetPattern>, bool),
    AggregateJoin(GroupPattern, BTreeMap<Aggregation, Variable>),
}

impl fmt::Display for MultiSetPattern {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MultiSetPattern::BGP(p) => write!(
                f,
                "BGP({})",
                p.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(" . ")
            ),
            MultiSetPattern::Join(a, b) => write!(f, "Join({}, {})", a, b),
            MultiSetPattern::LeftJoin(a, b, e) => write!(f, "LeftJoin({}, {}, {})", a, b, e),
            MultiSetPattern::Filter(e, p) => write!(f, "Filter({}, {})", e, p),
            MultiSetPattern::Union(a, b) => write!(f, "Union({}, {})", a, b),
            MultiSetPattern::Graph(g, p) => write!(f, "Graph({}, {})", g, p),
            MultiSetPattern::Extend(p, v, e) => write!(f, "Extend({}), {}, {})", p, v, e),
            MultiSetPattern::Minus(a, b) => write!(f, "Minus({}, {})", a, b),
            MultiSetPattern::ToMultiSet(l) => write!(f, "{}", l),
            MultiSetPattern::Service(n, p, s) => write!(f, "Service({}, {}, {})", n, p, s),
            MultiSetPattern::AggregateJoin(g, a) => write!(
                f,
                "AggregateJoin({}, {})",
                g,
                a.iter()
                    .map(|(a, v)| format!("{}: {}", v, a))
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        }
    }
}

impl Default for MultiSetPattern {
    fn default() -> Self {
        MultiSetPattern::BGP(Vec::default())
    }
}

impl From<TripleOrPathPattern> for MultiSetPattern {
    fn from(p: TripleOrPathPattern) -> Self {
        MultiSetPattern::BGP(vec![p])
    }
}

impl From<ListPattern> for MultiSetPattern {
    fn from(pattern: ListPattern) -> Self {
        match pattern {
            ListPattern::ToList(pattern) => pattern,
            pattern => MultiSetPattern::ToMultiSet(Box::new(pattern)),
        }
    }
}

impl MultiSetPattern {
    pub fn visible_variables(&self) -> BTreeSet<&Variable> {
        let mut vars = BTreeSet::default();
        self.add_visible_variables(&mut vars);
        vars
    }

    fn add_visible_variables<'a>(&'a self, vars: &mut BTreeSet<&'a Variable>) {
        match self {
            MultiSetPattern::BGP(p) => {
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
            MultiSetPattern::Join(a, b) => {
                a.add_visible_variables(vars);
                b.add_visible_variables(vars);
            }
            MultiSetPattern::LeftJoin(a, b, _) => {
                a.add_visible_variables(vars);
                b.add_visible_variables(vars);
            }
            MultiSetPattern::Filter(_, p) => p.add_visible_variables(vars),
            MultiSetPattern::Union(a, b) => {
                a.add_visible_variables(vars);
                b.add_visible_variables(vars);
            }
            MultiSetPattern::Graph(_, p) => p.add_visible_variables(vars),
            MultiSetPattern::Extend(p, v, _) => {
                p.add_visible_variables(vars);
                adds_if_has_name(vars, &v);
            }
            MultiSetPattern::Minus(a, _) => a.add_visible_variables(vars),
            MultiSetPattern::ToMultiSet(l) => l.add_visible_variables(vars),
            MultiSetPattern::Service(_, p, _) => p.add_visible_variables(vars),
            MultiSetPattern::AggregateJoin(_, a) => vars.extend(a.iter().map(|(_, v)| v)),
        }
    }
}

fn adds_if_has_name<'a>(vars: &mut BTreeSet<&'a Variable>, var: &'a Variable) {
    if var.has_name() {
        vars.insert(var);
    }
}

struct SparqlMultiSetPattern<'a>(&'a MultiSetPattern);

impl<'a> fmt::Display for SparqlMultiSetPattern<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            MultiSetPattern::BGP(p) => {
                if p.is_empty() {
                    write!(f, "{{}}")
                } else {
                    write!(
                        f,
                        "{}",
                        p.iter()
                            .map(|v| SparqlTripleOrPathPattern(v).to_string())
                            .collect::<Vec<String>>()
                            .join(" . ")
                    )
                }
            }
            MultiSetPattern::Join(a, b) => write!(
                f,
                "{} {}",
                SparqlMultiSetPattern(&*a),
                SparqlMultiSetPattern(&*b)
            ),
            MultiSetPattern::LeftJoin(a, b, e) => write!(
                f,
                "{} OPTIONAL {{ {} FILTER({}) }}",
                SparqlMultiSetPattern(&*a),
                SparqlMultiSetPattern(&*b),
                SparqlExpression(e)
            ),
            MultiSetPattern::Filter(e, p) => write!(
                f,
                "{} FILTER({})",
                SparqlMultiSetPattern(&*p),
                SparqlExpression(e)
            ),
            MultiSetPattern::Union(a, b) => write!(
                f,
                "{{ {} }} UNION {{ {} }}",
                SparqlMultiSetPattern(&*a),
                SparqlMultiSetPattern(&*b)
            ),
            MultiSetPattern::Graph(g, p) => {
                write!(f, "GRAPH {} {{ {} }}", g, SparqlMultiSetPattern(&*p))
            }
            MultiSetPattern::Extend(p, v, e) => write!(
                f,
                "{} BIND({} AS {})",
                SparqlMultiSetPattern(&*p),
                SparqlExpression(e),
                v
            ),
            MultiSetPattern::Minus(a, b) => write!(
                f,
                "{} MINUS {{ {} }}",
                SparqlMultiSetPattern(&*a),
                SparqlMultiSetPattern(&*b)
            ),
            MultiSetPattern::ToMultiSet(l) => write!(
                f,
                "{{ {} }}",
                SparqlListPattern {
                    algebra: &l,
                    dataset: &EMPTY_DATASET
                }
            ),
            MultiSetPattern::Service(n, p, s) => if *s {
                write!(
                    f,
                    "SERVICE SILENT {} {{ {} }}",
                    n,
                    SparqlMultiSetPattern(&*p)
                )
            } else {
                write!(f, "SERVICE {} {{ {} }}", n, SparqlMultiSetPattern(&*p))
            },
            MultiSetPattern::AggregateJoin(GroupPattern(group, p), agg) => write!(
                f,
                "{{ SELECT {} WHERE {{ {} }} GROUP BY {} }}",
                agg.iter()
                    .map(|(a, v)| format!("({} AS {})", SparqlAggregation(&a), v))
                    .collect::<Vec<String>>()
                    .join(" "),
                SparqlMultiSetPattern(p),
                group
                    .iter()
                    .map(|e| format!("({})", e.to_string()))
                    .collect::<Vec<String>>()
                    .join(" ")
            ),
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct GroupPattern(pub Vec<Expression>, pub Box<MultiSetPattern>);

impl fmt::Display for GroupPattern {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum ListPattern {
    Data(Vec<Binding>),
    ToList(MultiSetPattern),
    OrderBy(Box<ListPattern>, Vec<OrderComparator>),
    Project(Box<ListPattern>, Vec<Variable>),
    Distinct(Box<ListPattern>),
    Reduced(Box<ListPattern>),
    Slice(Box<ListPattern>, usize, Option<usize>),
}

impl fmt::Display for ListPattern {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ListPattern::Data(bs) => write!(
                f,
                "{{ {} }}",
                bs.iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<String>>()
                    .join(" ")
            ),
            ListPattern::ToList(l) => write!(f, "{}", l),
            ListPattern::OrderBy(l, o) => write!(
                f,
                "OrderBy({}, ({}))",
                l,
                o.iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            ListPattern::Project(l, pv) => write!(
                f,
                "Project({}, ({}))",
                l,
                pv.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            ListPattern::Distinct(l) => write!(f, "Distinct({})", l),
            ListPattern::Reduced(l) => write!(f, "Reduce({})", l),
            ListPattern::Slice(l, start, length) => write!(
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

impl Default for ListPattern {
    fn default() -> Self {
        ListPattern::Data(Vec::default())
    }
}

impl From<MultiSetPattern> for ListPattern {
    fn from(pattern: MultiSetPattern) -> Self {
        match pattern {
            MultiSetPattern::ToMultiSet(pattern) => *pattern,
            pattern => ListPattern::ToList(pattern),
        }
    }
}

impl ListPattern {
    pub fn visible_variables<'a>(&'a self) -> BTreeSet<&'a Variable> {
        let mut vars = BTreeSet::default();
        self.add_visible_variables(&mut vars);
        vars
    }

    fn add_visible_variables<'a>(&'a self, vars: &mut BTreeSet<&'a Variable>) {
        match self {
            ListPattern::Data(b) => {
                for binding in b {
                    for (var, _) in binding {
                        vars.insert(var);
                    }
                }
            }
            ListPattern::ToList(p) => p.add_visible_variables(vars),
            ListPattern::OrderBy(l, _) => l.add_visible_variables(vars),
            ListPattern::Project(_, pv) => vars.extend(pv.iter()),
            ListPattern::Distinct(l) => l.add_visible_variables(vars),
            ListPattern::Reduced(l) => l.add_visible_variables(vars),
            ListPattern::Slice(l, _, _) => l.add_visible_variables(vars),
        }
    }
}

struct SparqlListPattern<'a> {
    algebra: &'a ListPattern,
    dataset: &'a DatasetSpec,
}

impl<'a> fmt::Display for SparqlListPattern<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.algebra {
            ListPattern::Data(bs) => if bs.is_empty() {
                Ok(())
            } else {
                let vars: Vec<&Variable> = bs[0].iter().map(|(v, _)| v).collect();
                write!(f, "VALUES ( ")?;
                for var in &vars {
                    write!(f, "{} ", var)?;
                }
                write!(f, ") {{ ")?;
                for b in bs {
                    write!(f, "( ")?;
                    for var in &vars {
                        b.get(var)
                            .map(|v| write!(f, "{} ", v))
                            .unwrap_or_else(|| write!(f, "UNDEF "))?;
                    }
                    write!(f, ") ")?;
                }
                write!(f, " }}")
            },
            ListPattern::ToList(l) => write!(f, "{}", SparqlMultiSetPattern(&*l)),
            ListPattern::OrderBy(l, o) => write!(
                f,
                "{} ORDER BY {}",
                SparqlListPattern {
                    algebra: &*l,
                    dataset: self.dataset
                },
                o.iter()
                    .map(|c| SparqlOrderComparator(c).to_string())
                    .collect::<Vec<String>>()
                    .join(" ")
            ),
            ListPattern::Project(l, pv) => write!(
                f,
                "SELECT {} {} WHERE {{ {} }}",
                build_sparql_select_arguments(pv),
                self.dataset,
                SparqlListPattern {
                    algebra: &*l,
                    dataset: &EMPTY_DATASET
                }
            ),
            ListPattern::Distinct(l) => match l.as_ref() {
                ListPattern::Project(l, pv) => write!(
                    f,
                    "SELECT DISTINCT {} {} WHERE {{ {} }}",
                    build_sparql_select_arguments(pv),
                    self.dataset,
                    SparqlListPattern {
                        algebra: &*l,
                        dataset: &EMPTY_DATASET
                    }
                ),
                l => write!(
                    f,
                    "DISTINCT {}",
                    SparqlListPattern {
                        algebra: &l,
                        dataset: self.dataset
                    }
                ),
            },
            ListPattern::Reduced(l) => match l.as_ref() {
                ListPattern::Project(l, pv) => write!(
                    f,
                    "SELECT REDUCED {} {} WHERE {{ {} }}",
                    build_sparql_select_arguments(pv),
                    self.dataset,
                    SparqlListPattern {
                        algebra: &*l,
                        dataset: &EMPTY_DATASET
                    }
                ),
                l => write!(
                    f,
                    "REDUCED {}",
                    SparqlListPattern {
                        algebra: &l,
                        dataset: self.dataset
                    }
                ),
            },
            ListPattern::Slice(l, start, length) => length
                .map(|length| {
                    write!(
                        f,
                        "{} LIMIT {} OFFSET {}",
                        SparqlListPattern {
                            algebra: &*l,
                            dataset: self.dataset
                        },
                        start,
                        length
                    )
                }).unwrap_or_else(|| {
                    write!(
                        f,
                        "{} LIMIT {}",
                        SparqlListPattern {
                            algebra: &*l,
                            dataset: self.dataset
                        },
                        start
                    )
                }),
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Aggregation::Count(e, distinct) => if *distinct {
                e.as_ref()
                    .map(|ex| write!(f, "COUNT(DISTINCT {})", ex))
                    .unwrap_or_else(|| write!(f, "COUNT(DISTINCT *)"))
            } else {
                e.as_ref()
                    .map(|ex| write!(f, "COUNT({})", ex))
                    .unwrap_or_else(|| write!(f, "COUNT(*)"))
            },
            Aggregation::Sum(e, distinct) => if *distinct {
                write!(f, "Aggregation(Distinct({}), Sum, {{}})", e)
            } else {
                write!(f, "Aggregation({}, Sum, {{}})", e)
            },
            Aggregation::Min(e, distinct) => if *distinct {
                write!(f, "Aggregation(Distinct({}), Min, {{}})", e)
            } else {
                write!(f, "Aggregation({}, Min, {{}})", e)
            },
            Aggregation::Max(e, distinct) => if *distinct {
                write!(f, "Aggregation(Distinct({}), Max, {{}})", e)
            } else {
                write!(f, "Aggregation({}, Max, {{}})", e)
            },
            Aggregation::Avg(e, distinct) => if *distinct {
                write!(f, "Aggregation(Distinct({}), Avg, {{}})", e)
            } else {
                write!(f, "Aggregation({}, Avg, {{}})", e)
            },
            Aggregation::Sample(e, distinct) => if *distinct {
                write!(f, "Aggregation(Distinct({}), Sum, {{}})", e)
            } else {
                write!(f, "Aggregation({}, Sample, {{}})", e)
            },
            Aggregation::GroupConcat(e, distinct, sep) => if *distinct {
                sep.as_ref()
                    .map(|s| {
                        write!(
                            f,
                            "Aggregation(Distinct({}), GroupConcat, {{\"separator\" → \"{}\"}})",
                            e,
                            s.escape()
                        )
                    })
                    .unwrap_or_else(|| write!(f, "Aggregation(Distinct({}), GroupConcat, {{}})", e))
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
                    .unwrap_or_else(|| write!(f, "Aggregation(Distinct({}), GroupConcat, {{}})", e))
            },
        }
    }
}

struct SparqlAggregation<'a>(&'a Aggregation);

impl<'a> fmt::Display for SparqlAggregation<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            Aggregation::Count(e, distinct) => if *distinct {
                e.as_ref()
                    .map(|ex| write!(f, "COUNT(DISTINCT {})", SparqlExpression(ex)))
                    .unwrap_or_else(|| write!(f, "COUNT(DISTINCT *)"))
            } else {
                e.as_ref()
                    .map(|ex| write!(f, "COUNT({})", SparqlExpression(ex)))
                    .unwrap_or_else(|| write!(f, "COUNT(*)"))
            },
            Aggregation::Sum(e, distinct) => if *distinct {
                write!(f, "SUM(DISTINCT {})", SparqlExpression(e))
            } else {
                write!(f, "SUM({})", SparqlExpression(e))
            },
            Aggregation::Min(e, distinct) => if *distinct {
                write!(f, "MIN(DISTINCT {})", SparqlExpression(e))
            } else {
                write!(f, "MIN({})", SparqlExpression(e))
            },
            Aggregation::Max(e, distinct) => if *distinct {
                write!(f, "MAX(DISTINCT {})", SparqlExpression(e))
            } else {
                write!(f, "MAX({})", SparqlExpression(e))
            },
            Aggregation::Avg(e, distinct) => if *distinct {
                write!(f, "AVG(DISTINCT {})", SparqlExpression(e))
            } else {
                write!(f, "AVG({})", SparqlExpression(e))
            },
            Aggregation::Sample(e, distinct) => if *distinct {
                write!(f, "SAMPLE(DISTINCT {})", SparqlExpression(e))
            } else {
                write!(f, "SAMPLE({})", SparqlExpression(e))
            },
            Aggregation::GroupConcat(e, distinct, sep) => if *distinct {
                sep.as_ref()
                    .map(|s| {
                        write!(
                            f,
                            "GROUP_CONCAT(DISTINCT {}; SEPARATOR = \"{}\")",
                            SparqlExpression(e),
                            s.escape()
                        )
                    })
                    .unwrap_or_else(|| write!(f, "GROUP_CONCAT(DISTINCT {})", SparqlExpression(e)))
            } else {
                sep.as_ref()
                    .map(|s| {
                        write!(
                            f,
                            "GROUP_CONCAT({}; SEPARATOR = \"{}\")",
                            SparqlExpression(e),
                            s.escape()
                        )
                    }).unwrap_or_else(|| write!(f, "GROUP_CONCAT({})", SparqlExpression(e)))
            },
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum OrderComparator {
    Asc(Expression),
    Desc(Expression),
}

impl fmt::Display for OrderComparator {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

    fn add(mut self, rhs: DatasetSpec) -> Self {
        self.default.extend_from_slice(&rhs.default);
        self.named.extend_from_slice(&rhs.named);
        self
    }
}

impl fmt::Display for DatasetSpec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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
    SelectQuery {
        dataset: DatasetSpec,
        algebra: ListPattern,
    },
    ConstructQuery {
        construct: Vec<TriplePattern>,
        dataset: DatasetSpec,
        algebra: ListPattern,
    },
    DescribeQuery {
        dataset: DatasetSpec,
        algebra: ListPattern,
    },
    AskQuery {
        dataset: DatasetSpec,
        algebra: ListPattern,
    },
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Query::SelectQuery { dataset, algebra } => write!(
                f,
                "{}",
                SparqlListPattern {
                    algebra: &algebra,
                    dataset: &dataset
                }
            ),
            Query::ConstructQuery {
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
                SparqlListPattern {
                    algebra: &algebra,
                    dataset: &EMPTY_DATASET
                }
            ),
            Query::DescribeQuery { dataset, algebra } => write!(
                f,
                "DESCRIBE {} WHERE {{ {} }}",
                dataset,
                SparqlListPattern {
                    algebra: &algebra,
                    dataset: &EMPTY_DATASET
                }
            ),
            Query::AskQuery { dataset, algebra } => write!(
                f,
                "ASK {} WHERE {{ {} }}",
                dataset,
                SparqlListPattern {
                    algebra: &algebra,
                    dataset: &EMPTY_DATASET
                }
            ),
        }
    }
}
