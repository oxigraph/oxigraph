use model::*;
use sparql::model::*;
use std::fmt;
use std::ops::Add;
use utils::Escaper;

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum PropertyPath {
    PredicatePath(NamedNodeOrVariable), //TODO: restrict to NamedNode
    InversePath(Box<PropertyPath>),
    SequencePath(Box<PropertyPath>, Box<PropertyPath>),
    AlternativePath(Box<PropertyPath>, Box<PropertyPath>),
    ZeroOrMorePath(Box<PropertyPath>),
    OneOrMorePath(Box<PropertyPath>),
    ZeroOrOnePath(Box<PropertyPath>),
    NegatedPropertySet(Vec<NamedNodeOrVariable>),
}

impl fmt::Display for PropertyPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PropertyPath::PredicatePath(p) => write!(f, "{}", p),
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

impl From<NamedNodeOrVariable> for PropertyPath {
    fn from(p: NamedNodeOrVariable) -> Self {
        PropertyPath::PredicatePath(p)
    }
}

impl From<NamedNode> for PropertyPath {
    fn from(p: NamedNode) -> Self {
        PropertyPath::PredicatePath(p.into())
    }
}

impl From<Variable> for PropertyPath {
    fn from(p: Variable) -> Self {
        PropertyPath::PredicatePath(p.into())
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct PropertyPathPattern {
    pub subject: TermOrVariable,
    pub path: PropertyPath,
    pub object: TermOrVariable,
}

impl fmt::Display for PropertyPathPattern {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {}", self.subject, self.path, self.object)
    }
}

impl PropertyPathPattern {
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

impl From<TriplePattern> for PropertyPathPattern {
    fn from(p: TriplePattern) -> Self {
        Self {
            subject: p.subject,
            path: p.predicate.into(),
            object: p.object,
        }
    }
}

struct SparqlPropertyPathPattern<'a>(&'a PropertyPathPattern);

impl<'a> fmt::Display for SparqlPropertyPathPattern<'a> {
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
    CountAggregate(Option<Box<Expression>>, bool),
    SumAggregate(Box<Expression>, bool),
    MinAggregate(Box<Expression>, bool),
    MaxAggregate(Box<Expression>, bool),
    AvgAggregate(Box<Expression>, bool),
    SampleAggregate(Box<Expression>, bool),
    GroupConcatAggregate(Box<Expression>, bool, Option<String>),
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
            Expression::BNodeFunctionCall(v) => v.as_ref()
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
            Expression::SubStrFunctionCall(a, b, c) => c.as_ref()
                .map(|cv| write!(f, "SUBSTR({}, {}, {})", a, b, cv))
                .unwrap_or_else(|| write!(f, "SUBSTR({}, {})", a, b)),
            Expression::StrLenFunctionCall(e) => write!(f, "STRLEN({})", e),
            Expression::ReplaceFunctionCall(a, b, c, d) => d.as_ref()
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
            Expression::RegexFunctionCall(a, b, c) => c.as_ref()
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
            Expression::CountAggregate(e, distinct) => if *distinct {
                e.as_ref()
                    .map(|ex| write!(f, "COUNT(DISTINCT {})", ex))
                    .unwrap_or_else(|| write!(f, "COUNT(DISTINCT *)"))
            } else {
                e.as_ref()
                    .map(|ex| write!(f, "COUNT({})", ex))
                    .unwrap_or_else(|| write!(f, "COUNT(*)"))
            },
            Expression::SumAggregate(e, distinct) => if *distinct {
                write!(f, "SUM(DISTINCT {})", e)
            } else {
                write!(f, "SUM({})", e)
            },
            Expression::MinAggregate(e, distinct) => if *distinct {
                write!(f, "MIN(DISTINCT {})", e)
            } else {
                write!(f, "MIN({})", e)
            },
            Expression::MaxAggregate(e, distinct) => if *distinct {
                write!(f, "MAX(DISTINCT {})", e)
            } else {
                write!(f, "MAX({})", e)
            },
            Expression::AvgAggregate(e, distinct) => if *distinct {
                write!(f, "AVG(DISTINCT {})", e)
            } else {
                write!(f, "AVG({})", e)
            },
            Expression::SampleAggregate(e, distinct) => if *distinct {
                write!(f, "SAMPLE(DISTINCT {})", e)
            } else {
                write!(f, "SAMPLE({})", e)
            },
            Expression::GroupConcatAggregate(e, distinct, sep) => if *distinct {
                sep.as_ref()
                    .map(|s| {
                        write!(
                            f,
                            "GROUP_CONCAT(DISTINCT {}; SEPARATOR = \"{}\")",
                            e,
                            s.escape()
                        )
                    })
                    .unwrap_or_else(|| write!(f, "GROUP_CONCAT(DISTINCT {})", e))
            } else {
                sep.as_ref()
                    .map(|s| write!(f, "GROUP_CONCAT({}; SEPARATOR = \"{}\")", e, s.escape()))
                    .unwrap_or_else(|| write!(f, "GROUP_CONCAT({})", e))
            },
        }
    }
}

impl From<Literal> for Expression {
    fn from(p: Literal) -> Self {
        Expression::ConstantExpression(p.into())
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
            Expression::BNodeFunctionCall(v) => v.as_ref()
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
            Expression::SubStrFunctionCall(a, b, c) => c.as_ref()
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
            Expression::ReplaceFunctionCall(a, b, c, d) => d.as_ref()
                .map(|dv| {
                    write!(
                        f,
                        "REPLACE({}, {}, {}, {})",
                        SparqlExpression(&*a),
                        SparqlExpression(&*b),
                        SparqlExpression(&*c),
                        dv
                    )
                })
                .unwrap_or_else(|| {
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
            Expression::RegexFunctionCall(a, b, c) => c.as_ref()
                .map(|cv| {
                    write!(
                        f,
                        "REGEX({}, {}, {})",
                        SparqlExpression(&*a),
                        SparqlExpression(&*b),
                        cv
                    )
                })
                .unwrap_or_else(|| {
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
            Expression::CountAggregate(e, distinct) => if *distinct {
                e.as_ref()
                    .map(|ex| write!(f, "COUNT(DISTINCT {})", SparqlExpression(ex)))
                    .unwrap_or_else(|| write!(f, "COUNT(DISTINCT *)"))
            } else {
                e.as_ref()
                    .map(|ex| write!(f, "COUNT({})", SparqlExpression(ex)))
                    .unwrap_or_else(|| write!(f, "COUNT(*)"))
            },
            Expression::SumAggregate(e, distinct) => if *distinct {
                write!(f, "SUM(DISTINCT {})", SparqlExpression(e))
            } else {
                write!(f, "SUM({})", SparqlExpression(e))
            },
            Expression::MinAggregate(e, distinct) => if *distinct {
                write!(f, "MIN(DISTINCT {})", SparqlExpression(e))
            } else {
                write!(f, "MIN({})", SparqlExpression(e))
            },
            Expression::MaxAggregate(e, distinct) => if *distinct {
                write!(f, "MAX(DISTINCT {})", SparqlExpression(e))
            } else {
                write!(f, "MAX({})", SparqlExpression(e))
            },
            Expression::AvgAggregate(e, distinct) => if *distinct {
                write!(f, "AVG(DISTINCT {})", SparqlExpression(e))
            } else {
                write!(f, "AVG({})", SparqlExpression(e))
            },
            Expression::SampleAggregate(e, distinct) => if *distinct {
                write!(f, "SAMPLE(DISTINCT {})", SparqlExpression(e))
            } else {
                write!(f, "SAMPLE({})", SparqlExpression(e))
            },
            Expression::GroupConcatAggregate(e, distinct, sep) => if *distinct {
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
                    })
                    .unwrap_or_else(|| write!(f, "GROUP_CONCAT({})", SparqlExpression(e)))
            },
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum MultiSetPattern {
    BGP(Vec<PropertyPathPattern>),
    Join(Box<MultiSetPattern>, Box<MultiSetPattern>),
    LeftJoin(Box<MultiSetPattern>, Box<MultiSetPattern>, Expression),
    Filter(Expression, Box<MultiSetPattern>),
    Union(Box<MultiSetPattern>, Box<MultiSetPattern>),
    Graph(NamedNodeOrVariable, Box<MultiSetPattern>),
    Extend(Box<MultiSetPattern>, Variable, Expression),
    Minus(Box<MultiSetPattern>, Box<MultiSetPattern>),
    ToMultiSet(Box<ListPattern>),
    Service(NamedNodeOrVariable, Box<MultiSetPattern>, bool),
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
        }
    }
}

impl Default for MultiSetPattern {
    fn default() -> Self {
        MultiSetPattern::BGP(Vec::default())
    }
}

impl From<PropertyPathPattern> for MultiSetPattern {
    fn from(p: PropertyPathPattern) -> Self {
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
                            .map(|v| SparqlPropertyPathPattern(v).to_string())
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
            MultiSetPattern::ToMultiSet(l) => write!(f, "{}", SparqlListPattern(&l)),
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
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum ListPattern {
    Data(Vec<Binding>),
    ToList(MultiSetPattern),
    Group(),
    Aggregation(),
    AggregateJoin(),
    OrderBy(Box<MultiSetPattern>),
    Project(Box<MultiSetPattern>),
    Distinct(Box<MultiSetPattern>),
    Reduced(Box<MultiSetPattern>),
    Slice(Box<MultiSetPattern>, usize, usize),
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

impl fmt::Display for ListPattern {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            ListPattern::ToList(l) => write!(f, "{}", l),
            _ => Ok(()), //TODO
        }
    }
}

struct SparqlListPattern<'a>(&'a ListPattern);

impl<'a> fmt::Display for SparqlListPattern<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            ListPattern::ToList(l) => write!(f, "{}", SparqlMultiSetPattern(&l)),
            _ => Ok(()), //TODO
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash, Default)]
pub struct Dataset {
    pub default: Vec<NamedNode>,
    pub named: Vec<NamedNode>,
}

impl Dataset {
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

impl Add for Dataset {
    type Output = Self;

    fn add(mut self, rhs: Dataset) -> Self {
        self.default.extend_from_slice(&rhs.default);
        self.named.extend_from_slice(&rhs.named);
        self
    }
}

impl fmt::Display for Dataset {
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

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum SelectionOption {
    Distinct,
    Reduced,
    Default,
}

impl fmt::Display for SelectionOption {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            SelectionOption::Distinct => write!(f, "DISTINCT"),
            SelectionOption::Reduced => write!(f, "REDUCED"),
            SelectionOption::Default => Ok(()),
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum SelectionMember {
    Variable(Variable),
    Expression(Expression, Variable),
}

impl fmt::Display for SelectionMember {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            SelectionMember::Variable(v) => write!(f, "{}", v),
            SelectionMember::Expression(e, v) => write!(f, "({} AS {})", e, v),
        }
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct Selection {
    pub option: SelectionOption,
    pub variables: Option<Vec<SelectionMember>>,
}

impl fmt::Display for Selection {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.variables
            .as_ref()
            .map(|vars| {
                write!(
                    f,
                    "{} {}",
                    self.option,
                    vars.iter()
                        .map(|v| v.to_string())
                        .collect::<Vec<String>>()
                        .join(" ")
                )
            })
            .unwrap_or_else(|| write!(f, "{} *", self.option))
    }
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum Query {
    SelectQuery {
        selection: Selection,
        dataset: Dataset,
        filter: ListPattern,
    },
    ConstructQuery {
        construct: Vec<TriplePattern>,
        dataset: Dataset,
        filter: ListPattern,
    },
    DescribeQuery {
        dataset: Dataset,
        filter: ListPattern,
    },
    AskQuery {
        dataset: Dataset,
        filter: ListPattern,
    },
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Query::SelectQuery {
                selection,
                dataset,
                filter,
            } => write!(
                f,
                "SELECT {} {} WHERE {{ {} }}",
                selection,
                dataset,
                SparqlListPattern(&filter)
            ),
            Query::ConstructQuery {
                construct,
                dataset,
                filter,
            } => write!(
                f,
                "CONSTRUCT {{ {} }} {} WHERE {{ {} }}",
                construct
                    .iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<String>>()
                    .join(" . "),
                dataset,
                SparqlListPattern(&filter)
            ),
            Query::DescribeQuery { dataset, filter } => write!(
                f,
                "DESCRIBE {} WHERE {{ {} }}",
                dataset,
                SparqlListPattern(&filter)
            ),
            Query::AskQuery { dataset, filter } => write!(
                f,
                "ASK {} WHERE {{ {} }}",
                dataset,
                SparqlListPattern(&filter)
            ),
        }
    }
}

/* TODO: tests
/// Implementation of https://www.w3.org/TR/2013/REC-sparql11-query-20130321/#sparqlAlgebraExamples
#[test]
fn test_sparql_algebra_examples() {
    assert_eq!(
        ast::GraphPattern::PropertyPathPattern(ast::PropertyPathPattern::new(
            Variable::new("s"),
            Variable::new("p"),
            Variable::new("o")
        )).try_into(),
        Ok(GraphPattern::BGP(vec![PropertyPathPattern::new(
            Variable::new("s"),
            Variable::new("p"),
            Variable::new("o"),
        )]))
    );

    assert_eq!(
        ast::GraphPattern::GroupPattern(vec![
            ast::GraphPattern::PropertyPathPattern(ast::PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p1"),
                Variable::new("v1"),
            )),
            ast::GraphPattern::PropertyPathPattern(ast::PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p2"),
                Variable::new("v2"),
            )),
        ]).try_into(),
        Ok(GraphPattern::BGP(vec![
            PropertyPathPattern::new(Variable::new("s"), Variable::new("p1"), Variable::new("v1")),
            PropertyPathPattern::new(Variable::new("s"), Variable::new("p2"), Variable::new("v2")),
        ]))
    );

    assert_eq!(
        ast::GraphPattern::UnionPattern(vec![
            ast::GraphPattern::PropertyPathPattern(ast::PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p1"),
                Variable::new("v1"),
            )),
            ast::GraphPattern::PropertyPathPattern(ast::PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p2"),
                Variable::new("v2"),
            )),
        ]).try_into(),
        Ok(GraphPattern::Union(
            Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p1"),
                Variable::new("v1"),
            )])),
            Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p2"),
                Variable::new("v2"),
            )])),
        ))
    );

    assert_eq!(
        ast::GraphPattern::UnionPattern(vec![
            ast::GraphPattern::PropertyPathPattern(ast::PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p1"),
                Variable::new("v1"),
            )),
            ast::GraphPattern::PropertyPathPattern(ast::PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p2"),
                Variable::new("v2"),
            )),
            ast::GraphPattern::PropertyPathPattern(ast::PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p3"),
                Variable::new("v3"),
            )),
        ]).try_into(),
        Ok(GraphPattern::Union(
            Box::new(GraphPattern::Union(
                Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                    Variable::new("s"),
                    Variable::new("p1"),
                    Variable::new("v1"),
                )])),
                Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                    Variable::new("s"),
                    Variable::new("p2"),
                    Variable::new("v2"),
                )])),
            )),
            Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p3"),
                Variable::new("v3"),
            )])),
        ))
    );

    assert_eq!(
        ast::GraphPattern::GroupPattern(vec![
            ast::GraphPattern::PropertyPathPattern(ast::PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p1"),
                Variable::new("v1"),
            )),
            ast::GraphPattern::OptionalPattern(Box::new(ast::GraphPattern::PropertyPathPattern(
                ast::PropertyPathPattern::new(
                    Variable::new("s"),
                    Variable::new("p2"),
                    Variable::new("v2"),
                ),
            ))),
        ]).try_into(),
        Ok(GraphPattern::LeftJoin(
            Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p1"),
                Variable::new("v1"),
            )])),
            Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p2"),
                Variable::new("v2"),
            )])),
            ast::Expression::ConstantExpression(Literal::from(true).into()),
        ))
    );

    assert_eq!(
        ast::GraphPattern::GroupPattern(vec![
            ast::GraphPattern::PropertyPathPattern(ast::PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p1"),
                Variable::new("v1"),
            )),
            ast::GraphPattern::OptionalPattern(Box::new(ast::GraphPattern::PropertyPathPattern(
                ast::PropertyPathPattern::new(
                    Variable::new("s"),
                    Variable::new("p2"),
                    Variable::new("v2"),
                ),
            ))),
            ast::GraphPattern::OptionalPattern(Box::new(ast::GraphPattern::PropertyPathPattern(
                ast::PropertyPathPattern::new(
                    Variable::new("s"),
                    Variable::new("p3"),
                    Variable::new("v3"),
                ),
            ))),
        ]).try_into(),
        Ok(GraphPattern::LeftJoin(
            Box::new(GraphPattern::LeftJoin(
                Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                    Variable::new("s"),
                    Variable::new("p1"),
                    Variable::new("v1"),
                )])),
                Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                    Variable::new("s"),
                    Variable::new("p2"),
                    Variable::new("v2"),
                )])),
                ast::Expression::ConstantExpression(Literal::from(true).into()),
            )),
            Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p3"),
                Variable::new("v3"),
            )])),
            ast::Expression::ConstantExpression(Literal::from(true).into()),
        ))
    );

    assert_eq!(
        ast::GraphPattern::GroupPattern(vec![
            ast::GraphPattern::PropertyPathPattern(ast::PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p1"),
                Variable::new("v1"),
            )),
            ast::GraphPattern::OptionalPattern(Box::new(ast::GraphPattern::GroupPattern(vec![
                ast::GraphPattern::PropertyPathPattern(ast::PropertyPathPattern::new(
                    Variable::new("s"),
                    Variable::new("p2"),
                    Variable::new("v2"),
                )),
                ast::GraphPattern::FilterPattern(ast::Expression::LowerExpression(
                    Box::new(ast::Expression::ConstantExpression(
                        Variable::new("v1").into(),
                    )),
                    Box::new(ast::Expression::ConstantExpression(Literal::from(3).into())),
                )),
            ]))),
        ]).try_into(),
        Ok(GraphPattern::LeftJoin(
            Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p1"),
                Variable::new("v1"),
            )])),
            Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p2"),
                Variable::new("v2"),
            )])),
            ast::Expression::LowerExpression(
                Box::new(ast::Expression::ConstantExpression(
                    Variable::new("v1").into(),
                )),
                Box::new(ast::Expression::ConstantExpression(Literal::from(3).into())),
            ),
        ))
    );

    assert_eq!(
        ast::GraphPattern::GroupPattern(vec![
            ast::GraphPattern::UnionPattern(vec![
                ast::GraphPattern::PropertyPathPattern(ast::PropertyPathPattern::new(
                    Variable::new("s"),
                    Variable::new("p1"),
                    Variable::new("v1"),
                )),
                ast::GraphPattern::PropertyPathPattern(ast::PropertyPathPattern::new(
                    Variable::new("s"),
                    Variable::new("p2"),
                    Variable::new("v2"),
                )),
            ]),
            ast::GraphPattern::OptionalPattern(Box::new(ast::GraphPattern::PropertyPathPattern(
                ast::PropertyPathPattern::new(
                    Variable::new("s"),
                    Variable::new("p3"),
                    Variable::new("v3"),
                ),
            ))),
        ]).try_into(),
        Ok(GraphPattern::LeftJoin(
            Box::new(GraphPattern::Union(
                Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                    Variable::new("s"),
                    Variable::new("p1"),
                    Variable::new("v1"),
                )])),
                Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                    Variable::new("s"),
                    Variable::new("p2"),
                    Variable::new("v2"),
                )])),
            )),
            Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p3"),
                Variable::new("v3"),
            )])),
            ast::Expression::ConstantExpression(Literal::from(true).into()),
        ))
    );

    assert_eq!(
        ast::GraphPattern::GroupPattern(vec![
            ast::GraphPattern::PropertyPathPattern(ast::PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p1"),
                Variable::new("v1"),
            )),
            ast::GraphPattern::FilterPattern(ast::Expression::LowerExpression(
                Box::new(ast::Expression::ConstantExpression(
                    Variable::new("v1").into(),
                )),
                Box::new(ast::Expression::ConstantExpression(Literal::from(3).into())),
            )),
            ast::GraphPattern::OptionalPattern(Box::new(ast::GraphPattern::PropertyPathPattern(
                ast::PropertyPathPattern::new(
                    Variable::new("s"),
                    Variable::new("p2"),
                    Variable::new("v2"),
                ),
            ))),
        ]).try_into(),
        Ok(GraphPattern::Filter(
            ast::Expression::LowerExpression(
                Box::new(ast::Expression::ConstantExpression(
                    Variable::new("v1").into(),
                )),
                Box::new(ast::Expression::ConstantExpression(Literal::from(3).into())),
            ),
            Box::new(GraphPattern::LeftJoin(
                Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                    Variable::new("s"),
                    Variable::new("p1"),
                    Variable::new("v1"),
                )])),
                Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                    Variable::new("s"),
                    Variable::new("p2"),
                    Variable::new("v2"),
                )])),
                ast::Expression::ConstantExpression(Literal::from(true).into()),
            )),
        ))
    );

    assert_eq!(
        ast::GraphPattern::GroupPattern(vec![
            ast::GraphPattern::PropertyPathPattern(ast::PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p"),
                Variable::new("v"),
            )),
            ast::GraphPattern::BindPattern(
                ast::Expression::MulExpression(
                    Box::new(ast::Expression::ConstantExpression(Literal::from(2).into())),
                    Box::new(ast::Expression::ConstantExpression(
                        Variable::new("v").into(),
                    )),
                ),
                Variable::new("v2"),
            ),
            ast::GraphPattern::PropertyPathPattern(ast::PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p1"),
                Variable::new("v2"),
            )),
        ]).try_into(),
        Ok(GraphPattern::Join(
            Box::new(GraphPattern::Extend(
                Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                    Variable::new("s"),
                    Variable::new("p"),
                    Variable::new("v"),
                )])),
                Variable::new("v2"),
                ast::Expression::MulExpression(
                    Box::new(ast::Expression::ConstantExpression(Literal::from(2).into())),
                    Box::new(ast::Expression::ConstantExpression(
                        Variable::new("v").into(),
                    )),
                ),
            )),
            Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p1"),
                Variable::new("v2"),
            )])),
        ))
    );

    //TODO:  { ?s :p ?v . {} BIND (2*?v AS ?v2) }

    assert_eq!(
        ast::GraphPattern::GroupPattern(vec![
            ast::GraphPattern::PropertyPathPattern(ast::PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p"),
                Variable::new("v"),
            )),
            ast::GraphPattern::MinusPattern(Box::new(ast::GraphPattern::PropertyPathPattern(
                ast::PropertyPathPattern::new(
                    Variable::new("s"),
                    Variable::new("p1"),
                    Variable::new("v2"),
                ),
            ))),
        ]).try_into(),
        Ok(GraphPattern::Minus(
            Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p"),
                Variable::new("v"),
            )])),
            Box::new(GraphPattern::BGP(vec![PropertyPathPattern::new(
                Variable::new("s"),
                Variable::new("p1"),
                Variable::new("v2"),
            )])),
        ))
    );

    //TODO  { ?s :p ?o . {SELECT DISTINCT ?o {?o ?p ?z} } }
}*/
