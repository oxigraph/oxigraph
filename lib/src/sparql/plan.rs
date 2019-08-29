use crate::store::numeric_encoder::EncodedTerm;
use std::collections::BTreeSet;

pub type EncodedTuple = Vec<Option<EncodedTerm>>;

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum PlanNode {
    Init,
    StaticBindings {
        tuples: Vec<EncodedTuple>,
    },
    QuadPatternJoin {
        child: Box<PlanNode>,
        subject: PatternValue,
        predicate: PatternValue,
        object: PatternValue,
        graph_name: PatternValue,
    },
    Join {
        left: Box<PlanNode>,
        right: Box<PlanNode>,
    },
    Filter {
        child: Box<PlanNode>,
        expression: PlanExpression,
    },
    Union {
        entry: Box<PlanNode>,
        children: Vec<PlanNode>,
    },
    LeftJoin {
        left: Box<PlanNode>,
        right: Box<PlanNode>,
        possible_problem_vars: Vec<usize>, //Variables that should not be part of the entry of the left join
    },
    Extend {
        child: Box<PlanNode>,
        position: usize,
        expression: PlanExpression,
    },
    Sort {
        child: Box<PlanNode>,
        by: Vec<Comparator>,
    },
    HashDeduplicate {
        child: Box<PlanNode>,
    },
    Skip {
        child: Box<PlanNode>,
        count: usize,
    },
    Limit {
        child: Box<PlanNode>,
        count: usize,
    },
    Project {
        child: Box<PlanNode>,
        mapping: Vec<usize>, // for each key in children the key of the returned vector (children is sliced at the vector length)
    },
}

impl PlanNode {
    pub fn variables(&self) -> BTreeSet<usize> {
        let mut set = BTreeSet::default();
        self.add_variables(&mut set);
        set
    }

    fn add_variables(&self, set: &mut BTreeSet<usize>) {
        match self {
            PlanNode::Init => (),
            PlanNode::StaticBindings { tuples } => {
                for tuple in tuples {
                    for (key, value) in tuple.iter().enumerate() {
                        if value.is_some() {
                            set.insert(key);
                        }
                    }
                }
            }
            PlanNode::QuadPatternJoin {
                child,
                subject,
                predicate,
                object,
                graph_name,
            } => {
                if let PatternValue::Variable(var) = subject {
                    set.insert(*var);
                }
                if let PatternValue::Variable(var) = predicate {
                    set.insert(*var);
                }
                if let PatternValue::Variable(var) = object {
                    set.insert(*var);
                }
                if let PatternValue::Variable(var) = graph_name {
                    set.insert(*var);
                }
                child.add_variables(set);
            }
            PlanNode::Filter { child, expression } => {
                child.add_variables(set);
                expression.add_variables(set);
            } //TODO: condition vars
            PlanNode::Union { entry, children } => {
                entry.add_variables(set);
                for child in children {
                    child.add_variables(set);
                }
            }
            PlanNode::Join { left, right } => {
                left.add_variables(set);
                right.add_variables(set);
            }
            PlanNode::LeftJoin { left, right, .. } => {
                left.add_variables(set);
                right.add_variables(set);
            }
            PlanNode::Extend {
                child, position, ..
            } => {
                set.insert(*position);
                child.add_variables(set);
            }
            PlanNode::Sort { child, .. } => child.add_variables(set),
            PlanNode::HashDeduplicate { child } => child.add_variables(set),
            PlanNode::Skip { child, .. } => child.add_variables(set),
            PlanNode::Limit { child, .. } => child.add_variables(set),
            PlanNode::Project { child: _, mapping } => {
                for i in 0..mapping.len() {
                    set.insert(i);
                }
            }
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub enum PatternValue {
    Constant(EncodedTerm),
    Variable(usize),
}

impl PatternValue {
    pub fn is_var(&self) -> bool {
        match self {
            PatternValue::Constant(_) => false,
            PatternValue::Variable(_) => true,
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum PlanExpression {
    Constant(EncodedTerm),
    Variable(usize),
    Exists(Box<PlanNode>),
    Or(Box<PlanExpression>, Box<PlanExpression>),
    And(Box<PlanExpression>, Box<PlanExpression>),
    Equal(Box<PlanExpression>, Box<PlanExpression>),
    NotEqual(Box<PlanExpression>, Box<PlanExpression>),
    Greater(Box<PlanExpression>, Box<PlanExpression>),
    GreaterOrEq(Box<PlanExpression>, Box<PlanExpression>),
    Lower(Box<PlanExpression>, Box<PlanExpression>),
    LowerOrEq(Box<PlanExpression>, Box<PlanExpression>),
    In(Box<PlanExpression>, Vec<PlanExpression>),
    Add(Box<PlanExpression>, Box<PlanExpression>),
    Sub(Box<PlanExpression>, Box<PlanExpression>),
    Mul(Box<PlanExpression>, Box<PlanExpression>),
    Div(Box<PlanExpression>, Box<PlanExpression>),
    UnaryPlus(Box<PlanExpression>),
    UnaryMinus(Box<PlanExpression>),
    UnaryNot(Box<PlanExpression>),
    Str(Box<PlanExpression>),
    Lang(Box<PlanExpression>),
    LangMatches(Box<PlanExpression>, Box<PlanExpression>),
    Datatype(Box<PlanExpression>),
    Bound(usize),
    IRI(Box<PlanExpression>),
    BNode(Option<Box<PlanExpression>>),
    /*Rand(),
    Abs(Box<PlanExpression>),
    Ceil(Box<PlanExpression>),
    Floor(Box<PlanExpression>),
    Round(Box<PlanExpression>),
    Concat(Vec<PlanExpression>),
    SubStr(Box<PlanExpression>, Box<PlanExpression>, Option<Box<PlanExpression>>),
    StrLen(Box<PlanExpression>),
    Replace(
        Box<PlanExpression>,
        Box<PlanExpression>,
        Box<PlanExpression>,
        Option<Box<PlanExpression>>,
    ),
    UCase(Box<PlanExpression>),
    LCase(Box<PlanExpression>),
    EncodeForURI(Box<PlanExpression>),
    Contains(Box<PlanExpression>, Box<PlanExpression>),
    StrStarts(Box<PlanExpression>, Box<PlanExpression>),
    StrEnds(Box<PlanExpression>, Box<PlanExpression>),
    StrBefore(Box<PlanExpression>, Box<PlanExpression>),
    StrAfter(Box<PlanExpression>, Box<PlanExpression>),*/
    Year(Box<PlanExpression>),
    Month(Box<PlanExpression>),
    Day(Box<PlanExpression>),
    Hours(Box<PlanExpression>),
    Minutes(Box<PlanExpression>),
    Seconds(Box<PlanExpression>),
    /*Timezone(Box<PlanExpression>),
    Tz(Box<PlanExpression>),
    Now(),*/
    UUID(),
    StrUUID(),
    /*MD5(Box<PlanExpression>),
    SHA1(Box<PlanExpression>),
    SHA256(Box<PlanExpression>),
    SHA384(Box<PlanExpression>),
    SHA512(Box<PlanExpression>),*/
    Coalesce(Vec<PlanExpression>),
    If(
        Box<PlanExpression>,
        Box<PlanExpression>,
        Box<PlanExpression>,
    ),
    StrLang(Box<PlanExpression>, Box<PlanExpression>),
    //StrDT(Box<PlanExpression>, Box<PlanExpression>),
    SameTerm(Box<PlanExpression>, Box<PlanExpression>),
    IsIRI(Box<PlanExpression>),
    IsBlank(Box<PlanExpression>),
    IsLiteral(Box<PlanExpression>),
    IsNumeric(Box<PlanExpression>),
    Regex(
        Box<PlanExpression>,
        Box<PlanExpression>,
        Option<Box<PlanExpression>>,
    ),
    BooleanCast(Box<PlanExpression>),
    DoubleCast(Box<PlanExpression>),
    FloatCast(Box<PlanExpression>),
    DecimalCast(Box<PlanExpression>),
    IntegerCast(Box<PlanExpression>),
    DateCast(Box<PlanExpression>),
    TimeCast(Box<PlanExpression>),
    DateTimeCast(Box<PlanExpression>),
    StringCast(Box<PlanExpression>),
}

impl PlanExpression {
    fn add_variables(&self, set: &mut BTreeSet<usize>) {
        match self {
            PlanExpression::Constant(_)
            | PlanExpression::BNode(None)
            | PlanExpression::UUID()
            | PlanExpression::StrUUID() => (),
            PlanExpression::Variable(v) | PlanExpression::Bound(v) => {
                set.insert(*v);
            }
            PlanExpression::Or(a, b)
            | PlanExpression::And(a, b)
            | PlanExpression::Equal(a, b)
            | PlanExpression::NotEqual(a, b)
            | PlanExpression::Greater(a, b)
            | PlanExpression::GreaterOrEq(a, b)
            | PlanExpression::Lower(a, b)
            | PlanExpression::LowerOrEq(a, b)
            | PlanExpression::Add(a, b)
            | PlanExpression::Sub(a, b)
            | PlanExpression::Mul(a, b)
            | PlanExpression::Div(a, b)
            | PlanExpression::SameTerm(a, b)
            | PlanExpression::LangMatches(a, b)
            | PlanExpression::StrLang(a, b)
            | PlanExpression::Regex(a, b, None) => {
                a.add_variables(set);
                b.add_variables(set);
            }
            PlanExpression::UnaryPlus(e)
            | PlanExpression::UnaryMinus(e)
            | PlanExpression::UnaryNot(e)
            | PlanExpression::Str(e)
            | PlanExpression::Lang(e)
            | PlanExpression::Datatype(e)
            | PlanExpression::IRI(e)
            | PlanExpression::BNode(Some(e))
            | PlanExpression::Year(e)
            | PlanExpression::Month(e)
            | PlanExpression::Day(e)
            | PlanExpression::Hours(e)
            | PlanExpression::Minutes(e)
            | PlanExpression::Seconds(e)
            | PlanExpression::IsIRI(e)
            | PlanExpression::IsBlank(e)
            | PlanExpression::IsLiteral(e)
            | PlanExpression::IsNumeric(e)
            | PlanExpression::BooleanCast(e)
            | PlanExpression::DoubleCast(e)
            | PlanExpression::FloatCast(e)
            | PlanExpression::IntegerCast(e)
            | PlanExpression::DecimalCast(e)
            | PlanExpression::DateCast(e)
            | PlanExpression::TimeCast(e)
            | PlanExpression::DateTimeCast(e)
            | PlanExpression::StringCast(e) => {
                e.add_variables(set);
            }
            PlanExpression::Coalesce(l) => {
                for e in l {
                    e.add_variables(set);
                }
            }
            PlanExpression::If(a, b, c) => {
                a.add_variables(set);
                b.add_variables(set);
                c.add_variables(set);
            }
            PlanExpression::Regex(a, b, Some(c)) => {
                a.add_variables(set);
                b.add_variables(set);
                c.add_variables(set);
            }
            PlanExpression::In(e, l) => {
                e.add_variables(set);
                for e in l {
                    e.add_variables(set);
                }
            }
            PlanExpression::Exists(n) => n.add_variables(set),
        }
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum Comparator {
    Asc(PlanExpression),
    Desc(PlanExpression),
}

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub struct TripleTemplate {
    pub subject: TripleTemplateValue,
    pub predicate: TripleTemplateValue,
    pub object: TripleTemplateValue,
}

#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub enum TripleTemplateValue {
    Constant(EncodedTerm),
    BlankNode(usize),
    Variable(usize),
}
