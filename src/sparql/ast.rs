use model::*;
use sparql::model::*;
use std::fmt;
use std::ops::Add;
use utils::Escaper;

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct TriplePattern {
    subject: TermOrVariable,
    predicate: NamedNodeOrVariable,
    object: TermOrVariable,
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
    PredicatePath(NamedNodeOrVariable),
    InversePath(Box<PropertyPath>),
    SequencePath(Vec<PropertyPath>),
    AlternativePath(Vec<PropertyPath>),
    ZeroOrMorePath(Box<PropertyPath>),
    OneOrMorePath(Box<PropertyPath>),
    ZeroOrOnePath(Box<PropertyPath>),
    NegatedPath(Box<PropertyPath>),
}

impl fmt::Display for PropertyPath {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            PropertyPath::PredicatePath(p) => write!(f, "{}", p),
            PropertyPath::InversePath(p) => write!(f, "^{}", p),
            PropertyPath::SequencePath(ps) => write!(
                f,
                "({})",
                ps.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(" / ")
            ),
            PropertyPath::AlternativePath(ps) => write!(
                f,
                "({})",
                ps.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(" | ")
            ),
            PropertyPath::ZeroOrMorePath(p) => write!(f, "{}*", p),
            PropertyPath::OneOrMorePath(p) => write!(f, "{}+", p),
            PropertyPath::ZeroOrOnePath(p) => write!(f, "{}?", p),
            PropertyPath::NegatedPath(p) => write!(f, "!{}", p),
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
    subject: TermOrVariable,
    path: PropertyPath,
    object: TermOrVariable,
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

impl fmt::Display for PropertyPathPattern {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {} {}", self.subject, self.path, self.object)
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

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum Expression {
    ConstantExpression(TermOrVariable),
    OrExpression(Vec<Expression>),
    AndExpression(Vec<Expression>),
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
    ExistsFunctionCall(Box<GraphPattern>),
    NotExistsFunctionCall(Box<GraphPattern>),
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
            Expression::OrExpression(e) => write!(
                f,
                "({})",
                e.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(" || ")
            ),
            Expression::AndExpression(e) => write!(
                f,
                "({})",
                e.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(" && ")
            ),
            Expression::EqualExpression(a, b) => write!(f, "{} = {}", a, b),
            Expression::NotEqualExpression(a, b) => write!(f, "{} != {}", a, b),
            Expression::GreaterExpression(a, b) => write!(f, "{} > {}", a, b),
            Expression::GreaterOrEqExpression(a, b) => write!(f, "{} >= {}", a, b),
            Expression::LowerExpression(a, b) => write!(f, "{} < {}", a, b),
            Expression::LowerOrEqExpression(a, b) => write!(f, "{} <= {}", a, b),
            Expression::InExpression(a, b) => write!(
                f,
                "{} IN ({})",
                a,
                b.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Expression::NotInExpression(a, b) => write!(
                f,
                "{} NOT IN ({})",
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
            Expression::NotExistsFunctionCall(p) => write!(f, "NOT EXISTS {{ {} }}", p),
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

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub enum GraphPattern {
    GroupPattern(Vec<GraphPattern>),
    PropertyPathPattern(PropertyPathPattern),
    OptionalPattern(Box<GraphPattern>),
    UnionPattern(Vec<GraphPattern>),
    GraphPattern(NamedNodeOrVariable, Box<GraphPattern>),
    BindPattern(Expression, Variable),
    ValuesPattern(Vec<Variable>, Vec<Vec<Option<Term>>>),
    GroupByPattern(Expression),
    HavingPattern(Expression),
    MinusPattern(Box<GraphPattern>),
    FilterPattern(Expression),
    SubSelectPattern {
        selection: Selection,
        filter: Box<GraphPattern>,
    },
    ServicePattern(NamedNodeOrVariable, Box<GraphPattern>),
}

impl Default for GraphPattern {
    fn default() -> Self {
        GraphPattern::GroupPattern(Vec::default())
    }
}

impl From<PropertyPathPattern> for GraphPattern {
    fn from(p: PropertyPathPattern) -> Self {
        GraphPattern::PropertyPathPattern(p)
    }
}

impl fmt::Display for GraphPattern {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GraphPattern::GroupPattern(p) => write!(
                f,
                "{}",
                p.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(" . ")
            ),
            GraphPattern::PropertyPathPattern(p) => write!(f, "{}", p),
            GraphPattern::OptionalPattern(p) => write!(f, "OPTIONAL {{ {} }}", p),
            GraphPattern::UnionPattern(ps) => write!(
                f,
                "{{ {} }}",
                ps.iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<String>>()
                    .join(" } UNION { ")
            ),
            GraphPattern::GraphPattern(g, p) => write!(f, "GRAPH {} {{ {} }}", g, p),
            GraphPattern::BindPattern(e, v) => write!(f, "BIND({} AS {})", e, v),
            GraphPattern::ValuesPattern(vars, vals) => write!(
                f,
                "VALUES ({}) {{ {} }}",
                vars.iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<String>>()
                    .join(" "),
                vals.iter()
                    .map(|r| format!(
                        "({})",
                        r.iter()
                            .map(|vop| vop.as_ref()
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "UNDEF".to_string()))
                            .collect::<Vec<String>>()
                            .join(" ")
                    ))
                    .collect::<Vec<String>>()
                    .join(" ")
            ),
            GraphPattern::GroupByPattern(g) => write!(f, "GROUP BY ({})", g),
            GraphPattern::HavingPattern(e) => write!(f, "HAVING({})", e),
            GraphPattern::MinusPattern(p) => write!(f, "MINUS {{ {} }}", p),
            GraphPattern::FilterPattern(p) => write!(f, "FILTER({})", p),
            GraphPattern::SubSelectPattern { selection, filter } => {
                write!(f, "{{ SELECT {} WHERE {{ {} }} }}", selection, filter)
            }
            GraphPattern::ServicePattern(s, p) => write!(f, "SERVICE {} {{ {} }}", s, p),
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
        filter: GraphPattern,
    },
    ConstructQuery {
        construct: Vec<TriplePattern>,
        dataset: Dataset,
        filter: GraphPattern,
    },
    DescribeQuery {
        dataset: Dataset,
        filter: GraphPattern,
    },
    AskQuery {
        dataset: Dataset,
        filter: GraphPattern,
    },
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Query::SelectQuery {
                selection,
                dataset,
                filter,
            } => write!(f, "SELECT {} {} WHERE {{ {} }}", selection, dataset, filter),
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
                filter
            ),
            Query::DescribeQuery { dataset, filter } => {
                write!(f, "DESCRIBE {} WHERE {{ {} }}", dataset, filter)
            }
            Query::AskQuery { dataset, filter } => {
                write!(f, "ASK {} WHERE {{ {} }}", dataset, filter)
            }
        }
    }
}
