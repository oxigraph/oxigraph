use model::vocab::xsd;
use model::Literal;
use sparql::algebra::*;
use std::collections::BTreeSet;
use store::encoded::EncodedQuadsStore;
use store::numeric_encoder::EncodedTerm;
use store::numeric_encoder::ENCODED_DEFAULT_GRAPH;
use Result;

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
    fn variables(&self) -> BTreeSet<usize> {
        let mut set = BTreeSet::default();
        self.add_variables(&mut set);
        set
    }

    fn add_variables(&self, set: &mut BTreeSet<usize>) {
        match self {
            PlanNode::Init => (),
            PlanNode::StaticBindings { tuples } => {
                for tuple in tuples {
                    for (key, value) in tuple.into_iter().enumerate() {
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
            PlanNode::Project { child, mapping } => {
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
    StrAfter(Box<PlanExpression>, Box<PlanExpression>),
    Year(Box<PlanExpression>),
    Month(Box<PlanExpression>),
    Day(Box<PlanExpression>),
    Hours(Box<PlanExpression>),
    Minutes(Box<PlanExpression>),
    Seconds(Box<PlanExpression>),
    Timezone(Box<PlanExpression>),
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
            | PlanExpression::IsIRI(e)
            | PlanExpression::IsBlank(e)
            | PlanExpression::IsLiteral(e)
            | PlanExpression::IsNumeric(e)
            | PlanExpression::BooleanCast(e)
            | PlanExpression::DoubleCast(e)
            | PlanExpression::FloatCast(e)
            | PlanExpression::IntegerCast(e)
            | PlanExpression::DecimalCast(e)
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

pub struct PlanBuilder<'a, S: EncodedQuadsStore> {
    store: &'a S,
}

impl<'a, S: EncodedQuadsStore> PlanBuilder<'a, S> {
    pub fn build(store: &S, pattern: &GraphPattern) -> Result<(PlanNode, Vec<Variable>)> {
        let mut variables = Vec::default();
        let plan = PlanBuilder { store }.build_for_graph_pattern(
            pattern,
            PlanNode::Init,
            &mut variables,
            PatternValue::Constant(ENCODED_DEFAULT_GRAPH),
        )?;
        Ok((plan, variables))
    }

    pub fn build_graph_template(
        store: &S,
        template: &[TriplePattern],
        mut variables: Vec<Variable>,
    ) -> Result<Vec<TripleTemplate>> {
        PlanBuilder { store }.build_for_graph_template(template, &mut variables)
    }

    fn build_for_graph_pattern(
        &self,
        pattern: &GraphPattern,
        input: PlanNode,
        variables: &mut Vec<Variable>,
        graph_name: PatternValue,
    ) -> Result<PlanNode> {
        Ok(match pattern {
            GraphPattern::BGP(p) => {
                let mut plan = input;
                for pattern in p {
                    plan = match pattern {
                        TripleOrPathPattern::Triple(pattern) => PlanNode::QuadPatternJoin {
                            child: Box::new(plan),
                            subject: self
                                .pattern_value_from_term_or_variable(&pattern.subject, variables)?,
                            predicate: self.pattern_value_from_named_node_or_variable(
                                &pattern.predicate,
                                variables,
                            )?,
                            object: self
                                .pattern_value_from_term_or_variable(&pattern.object, variables)?,
                            graph_name,
                        },
                        TripleOrPathPattern::Path(pattern) => unimplemented!(),
                    }
                }
                plan
            }
            GraphPattern::Join(a, b) => PlanNode::Join {
                left: Box::new(self.build_for_graph_pattern(
                    a,
                    input.clone(),
                    variables,
                    graph_name,
                )?),
                right: Box::new(self.build_for_graph_pattern(b, input, variables, graph_name)?),
            },
            GraphPattern::LeftJoin(a, b, e) => {
                let left = self.build_for_graph_pattern(a, input, variables, graph_name)?;
                let right =
                    self.build_for_graph_pattern(b, PlanNode::Init, variables, graph_name)?;
                //We add the extra filter if needed
                let right = if *e == Expression::from(Literal::from(true)) {
                    right
                } else {
                    PlanNode::Filter {
                        child: Box::new(right),
                        expression: self.build_for_expression(e, variables)?,
                    }
                };
                let possible_problem_vars = right
                    .variables()
                    .difference(&left.variables())
                    .cloned()
                    .collect();

                PlanNode::LeftJoin {
                    left: Box::new(left),
                    right: Box::new(right),
                    possible_problem_vars,
                }
            }
            GraphPattern::Filter(e, p) => PlanNode::Filter {
                child: Box::new(self.build_for_graph_pattern(p, input, variables, graph_name)?),
                expression: self.build_for_expression(e, variables)?,
            },
            GraphPattern::Union(a, b) => {
                //We flatten the UNIONs
                let mut stack: Vec<&GraphPattern> = vec![a, b];
                let mut children = vec![];
                loop {
                    match stack.pop() {
                        None => break,
                        Some(GraphPattern::Union(a, b)) => {
                            stack.push(a);
                            stack.push(b);
                        }
                        Some(p) => children.push(self.build_for_graph_pattern(
                            p,
                            PlanNode::Init,
                            variables,
                            graph_name,
                        )?),
                    }
                }
                PlanNode::Union {
                    entry: Box::new(input),
                    children,
                }
            }
            GraphPattern::Graph(g, p) => {
                let graph_name = self.pattern_value_from_named_node_or_variable(g, variables)?;
                self.build_for_graph_pattern(p, input, variables, graph_name)?
            }
            GraphPattern::Extend(p, v, e) => PlanNode::Extend {
                child: Box::new(self.build_for_graph_pattern(p, input, variables, graph_name)?),
                position: variable_key(variables, &v),
                expression: self.build_for_expression(e, variables)?,
            },
            GraphPattern::Minus(a, b) => unimplemented!(),
            GraphPattern::Service(n, p, s) => unimplemented!(),
            GraphPattern::AggregateJoin(g, a) => unimplemented!(),
            GraphPattern::Data(bs) => PlanNode::StaticBindings {
                tuples: self.encode_bindings(bs, variables)?,
            },
            GraphPattern::OrderBy(l, o) => {
                let by: Result<Vec<_>> = o
                    .into_iter()
                    .map(|comp| match comp {
                        OrderComparator::Asc(e) => {
                            Ok(Comparator::Asc(self.build_for_expression(e, variables)?))
                        }
                        OrderComparator::Desc(e) => {
                            Ok(Comparator::Desc(self.build_for_expression(e, variables)?))
                        }
                    }).collect();
                PlanNode::Sort {
                    child: Box::new(self.build_for_graph_pattern(l, input, variables, graph_name)?),
                    by: by?,
                }
            }
            GraphPattern::Project(l, new_variables) => PlanNode::Project {
                child: Box::new(self.build_for_graph_pattern(
                    l,
                    input,
                    &mut new_variables.clone(),
                    graph_name,
                )?),
                mapping: new_variables
                    .iter()
                    .map(|variable| variable_key(variables, variable))
                    .collect(),
            },
            GraphPattern::Distinct(l) => PlanNode::HashDeduplicate {
                child: Box::new(self.build_for_graph_pattern(l, input, variables, graph_name)?),
            },
            GraphPattern::Reduced(l) => {
                self.build_for_graph_pattern(l, input, variables, graph_name)?
            }
            GraphPattern::Slice(l, start, length) => {
                let mut plan = self.build_for_graph_pattern(l, input, variables, graph_name)?;
                if *start > 0 {
                    plan = PlanNode::Skip {
                        child: Box::new(plan),
                        count: *start,
                    };
                }
                if let Some(length) = length {
                    plan = PlanNode::Limit {
                        child: Box::new(plan),
                        count: *length,
                    };
                }
                plan
            }
        })
    }

    fn build_for_expression(
        &self,
        expression: &Expression,
        variables: &mut Vec<Variable>,
    ) -> Result<PlanExpression> {
        Ok(match expression {
            Expression::Constant(t) => match t {
                TermOrVariable::Term(t) => {
                    PlanExpression::Constant(self.store.encoder().encode_term(t)?)
                }
                TermOrVariable::Variable(v) => PlanExpression::Variable(variable_key(variables, v)),
            },
            Expression::Or(a, b) => PlanExpression::Or(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::And(a, b) => PlanExpression::And(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::Equal(a, b) => PlanExpression::Equal(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::NotEqual(a, b) => PlanExpression::NotEqual(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::Greater(a, b) => PlanExpression::Greater(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::GreaterOrEq(a, b) => PlanExpression::GreaterOrEq(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::Lower(a, b) => PlanExpression::Lower(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::LowerOrEq(a, b) => PlanExpression::LowerOrEq(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::In(e, l) => PlanExpression::In(
                Box::new(self.build_for_expression(e, variables)?),
                self.expression_list(l, variables)?,
            ),
            Expression::NotIn(e, l) => PlanExpression::UnaryNot(Box::new(PlanExpression::In(
                Box::new(self.build_for_expression(e, variables)?),
                self.expression_list(l, variables)?,
            ))),
            Expression::Add(a, b) => PlanExpression::Add(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::Sub(a, b) => PlanExpression::Sub(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::Mul(a, b) => PlanExpression::Mul(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::Div(a, b) => PlanExpression::Div(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::UnaryPlus(e) => {
                PlanExpression::UnaryPlus(Box::new(self.build_for_expression(e, variables)?))
            }
            Expression::UnaryMinus(e) => {
                PlanExpression::UnaryMinus(Box::new(self.build_for_expression(e, variables)?))
            }
            Expression::UnaryNot(e) => {
                PlanExpression::UnaryNot(Box::new(self.build_for_expression(e, variables)?))
            }
            Expression::StrFunctionCall(e) => {
                PlanExpression::Str(Box::new(self.build_for_expression(e, variables)?))
            }
            Expression::LangFunctionCall(e) => {
                PlanExpression::Lang(Box::new(self.build_for_expression(e, variables)?))
            }
            Expression::LangMatchesFunctionCall(a, b) => PlanExpression::LangMatches(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::DatatypeFunctionCall(e) => {
                PlanExpression::Datatype(Box::new(self.build_for_expression(e, variables)?))
            }
            Expression::BoundFunctionCall(v) => PlanExpression::Bound(variable_key(variables, v)),
            Expression::IRIFunctionCall(e) => {
                PlanExpression::IRI(Box::new(self.build_for_expression(e, variables)?))
            }
            Expression::BNodeFunctionCall(e) => PlanExpression::BNode(match e {
                Some(e) => Some(Box::new(self.build_for_expression(e, variables)?)),
                None => None,
            }),
            Expression::UUIDFunctionCall() => PlanExpression::UUID(),
            Expression::StrUUIDFunctionCall() => PlanExpression::StrUUID(),
            Expression::CoalesceFunctionCall(l) => {
                PlanExpression::Coalesce(self.expression_list(l, variables)?)
            }
            Expression::IfFunctionCall(a, b, c) => PlanExpression::If(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
                Box::new(self.build_for_expression(c, variables)?),
            ),
            Expression::StrLangFunctionCall(a, b) => PlanExpression::StrLang(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::SameTermFunctionCall(a, b) => PlanExpression::SameTerm(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::IsIRIFunctionCall(e) => {
                PlanExpression::IsIRI(Box::new(self.build_for_expression(e, variables)?))
            }
            Expression::IsBlankFunctionCall(e) => {
                PlanExpression::IsBlank(Box::new(self.build_for_expression(e, variables)?))
            }
            Expression::IsLiteralFunctionCall(e) => {
                PlanExpression::IsLiteral(Box::new(self.build_for_expression(e, variables)?))
            }
            Expression::IsNumericFunctionCall(e) => {
                PlanExpression::IsNumeric(Box::new(self.build_for_expression(e, variables)?))
            }
            Expression::RegexFunctionCall(text, pattern, flags) => PlanExpression::Regex(
                Box::new(self.build_for_expression(text, variables)?),
                Box::new(self.build_for_expression(pattern, variables)?),
                match flags {
                    Some(flags) => Some(Box::new(self.build_for_expression(flags, variables)?)),
                    None => None,
                },
            ),
            Expression::CustomFunctionCall(name, parameters) => if *name == *xsd::BOOLEAN {
                self.build_cast(
                    parameters,
                    PlanExpression::BooleanCast,
                    variables,
                    "boolean",
                )?
            } else if *name == *xsd::DOUBLE {
                self.build_cast(parameters, PlanExpression::DoubleCast, variables, "double")?
            } else if *name == *xsd::FLOAT {
                self.build_cast(parameters, PlanExpression::FloatCast, variables, "float")?
            } else if *name == *xsd::DECIMAL {
                self.build_cast(
                    parameters,
                    PlanExpression::DecimalCast,
                    variables,
                    "decimal",
                )?
            } else if *name == *xsd::INTEGER {
                self.build_cast(
                    parameters,
                    PlanExpression::IntegerCast,
                    variables,
                    "integer",
                )?
            } else if *name == *xsd::DATE_TIME {
                self.build_cast(
                    parameters,
                    PlanExpression::DateTimeCast,
                    variables,
                    "dateTime",
                )?
            } else if *name == *xsd::STRING {
                self.build_cast(parameters, PlanExpression::StringCast, variables, "string")?
            } else {
                Err(format_err!("Not supported custom function {}", expression))?
            },
            _ => unimplemented!(),
        })
    }

    fn build_cast(
        &self,
        parameters: &[Expression],
        constructor: impl Fn(Box<PlanExpression>) -> PlanExpression,
        variables: &mut Vec<Variable>,
        name: &'static str,
    ) -> Result<PlanExpression> {
        if parameters.len() == 1 {
            Ok(constructor(Box::new(
                self.build_for_expression(&parameters[0], variables)?,
            )))
        } else {
            Err(format_err!(
                "The xsd:{} casting takes only one parameter",
                name
            ))
        }
    }

    fn expression_list(
        &self,
        l: &[Expression],
        variables: &mut Vec<Variable>,
    ) -> Result<Vec<PlanExpression>> {
        l.iter()
            .map(|e| self.build_for_expression(e, variables))
            .collect()
    }

    fn pattern_value_from_term_or_variable(
        &self,
        term_or_variable: &TermOrVariable,
        variables: &mut Vec<Variable>,
    ) -> Result<PatternValue> {
        Ok(match term_or_variable {
            TermOrVariable::Term(term) => {
                PatternValue::Constant(self.store.encoder().encode_term(term)?)
            }
            TermOrVariable::Variable(variable) => {
                PatternValue::Variable(variable_key(variables, variable))
            }
        })
    }

    fn pattern_value_from_named_node_or_variable(
        &self,
        named_node_or_variable: &NamedNodeOrVariable,
        variables: &mut Vec<Variable>,
    ) -> Result<PatternValue> {
        Ok(match named_node_or_variable {
            NamedNodeOrVariable::NamedNode(named_node) => {
                PatternValue::Constant(self.store.encoder().encode_named_node(named_node)?)
            }
            NamedNodeOrVariable::Variable(variable) => {
                PatternValue::Variable(variable_key(variables, variable))
            }
        })
    }

    fn encode_bindings(
        &self,
        bindings: &StaticBindings,
        variables: &mut Vec<Variable>,
    ) -> Result<Vec<EncodedTuple>> {
        let encoder = self.store.encoder();
        let bindings_variables = bindings.variables();
        bindings
            .values_iter()
            .map(move |values| {
                let mut result = vec![None; variables.len()];
                for (key, value) in values.iter().enumerate() {
                    if let Some(term) = value {
                        result[variable_key(variables, &bindings_variables[key])] =
                            Some(encoder.encode_term(term)?);
                    }
                }
                Ok(result)
            }).collect()
    }

    fn build_for_graph_template(
        &self,
        template: &[TriplePattern],
        variables: &mut Vec<Variable>,
    ) -> Result<Vec<TripleTemplate>> {
        let mut bnodes = Vec::default();
        template
            .into_iter()
            .map(|triple| {
                Ok(TripleTemplate {
                    subject: self.template_value_from_term_or_variable(
                        &triple.subject,
                        variables,
                        &mut bnodes,
                    )?,
                    predicate: self.template_value_from_named_node_or_variable(
                        &triple.predicate,
                        variables,
                        &mut bnodes,
                    )?,
                    object: self.template_value_from_term_or_variable(
                        &triple.object,
                        variables,
                        &mut bnodes,
                    )?,
                })
            }).collect()
    }

    fn template_value_from_term_or_variable(
        &self,
        term_or_variable: &TermOrVariable,
        variables: &mut Vec<Variable>,
        bnodes: &mut Vec<Variable>,
    ) -> Result<TripleTemplateValue> {
        Ok(match term_or_variable {
            TermOrVariable::Term(term) => {
                TripleTemplateValue::Constant(self.store.encoder().encode_term(term)?)
            }
            TermOrVariable::Variable(variable) => if variable.has_name() {
                TripleTemplateValue::Variable(variable_key(variables, variable))
            } else {
                TripleTemplateValue::BlankNode(variable_key(bnodes, variable))
            },
        })
    }

    fn template_value_from_named_node_or_variable(
        &self,
        named_node_or_variable: &NamedNodeOrVariable,
        variables: &mut Vec<Variable>,
        bnodes: &mut Vec<Variable>,
    ) -> Result<TripleTemplateValue> {
        Ok(match named_node_or_variable {
            NamedNodeOrVariable::NamedNode(term) => {
                TripleTemplateValue::Constant(self.store.encoder().encode_named_node(term)?)
            }
            NamedNodeOrVariable::Variable(variable) => if variable.has_name() {
                TripleTemplateValue::Variable(variable_key(variables, variable))
            } else {
                TripleTemplateValue::BlankNode(variable_key(bnodes, variable))
            },
        })
    }
}

fn variable_key(variables: &mut Vec<Variable>, variable: &Variable) -> usize {
    match slice_key(variables, variable) {
        Some(key) => key,
        None => {
            variables.push(variable.clone());
            variables.len() - 1
        }
    }
}

fn slice_key<T: Eq>(slice: &[T], element: &T) -> Option<usize> {
    for (i, item) in slice.iter().enumerate() {
        if item == element {
            return Some(i);
        }
    }
    None
}
