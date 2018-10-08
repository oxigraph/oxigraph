use sparql::algebra::*;
use store::numeric_encoder::EncodedTerm;
use store::store::EncodedQuadsStore;
use Result;

pub type EncodedTuple = Vec<Option<EncodedTerm>>;

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum PlanNode {
    Init,
    StaticBindings {
        tuples: Vec<EncodedTuple>,
    },
    TriplePatternJoin {
        child: Box<PlanNode>,
        subject: PatternValue,
        predicate: PatternValue,
        object: PatternValue,
    },
    Filter {
        child: Box<PlanNode>,
        expression: PlanExpression,
    },
    Union {
        entry: Box<PlanNode>,
        children: Vec<PlanNode>,
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
    //In(Box<PlanExpression>, Vec<PlanExpression>),
    //NotIn(Box<PlanExpression>, Vec<PlanExpression>),
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
    Now(),
    UUID(),
    StrUUID(),
    MD5(Box<PlanExpression>),
    SHA1(Box<PlanExpression>),
    SHA256(Box<PlanExpression>),
    SHA384(Box<PlanExpression>),
    SHA512(Box<PlanExpression>),
    Coalesce(Vec<PlanExpression>),
    If(Box<PlanExpression>, Box<PlanExpression>, Box<PlanExpression>),
    StrLang(Box<PlanExpression>, Box<PlanExpression>),
    StrDT(Box<PlanExpression>, Box<PlanExpression>),*/
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
        )?;
        Ok((plan, variables))
    }

    fn build_for_graph_pattern(
        &self,
        pattern: &GraphPattern,
        input: PlanNode,
        variables: &mut Vec<Variable>,
    ) -> Result<PlanNode> {
        Ok(match pattern {
            GraphPattern::BGP(p) => {
                let mut plan = input;
                for pattern in p {
                    plan = match pattern {
                        TripleOrPathPattern::Triple(pattern) => PlanNode::TriplePatternJoin {
                            child: Box::new(plan),
                            subject: self
                                .pattern_value_from_term_or_variable(&pattern.subject, variables)?,
                            predicate: self.pattern_value_from_named_node_or_variable(
                                &pattern.predicate,
                                variables,
                            )?,
                            object: self
                                .pattern_value_from_term_or_variable(&pattern.object, variables)?,
                        },
                        TripleOrPathPattern::Path(pattern) => unimplemented!(),
                    }
                }
                plan
            }
            GraphPattern::Join(a, b) => self.build_for_graph_pattern(
                b,
                self.build_for_graph_pattern(a, input, variables)?,
                variables,
            )?,
            GraphPattern::LeftJoin(a, b, e) => unimplemented!(),
            GraphPattern::Filter(e, p) => PlanNode::Filter {
                child: Box::new(self.build_for_graph_pattern(p, input, variables)?),
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
                            a,
                            PlanNode::Init,
                            variables,
                        )?),
                    }
                }
                PlanNode::Union {
                    entry: Box::new(input),
                    children,
                }
            }
            GraphPattern::Graph(g, p) => unimplemented!(),
            GraphPattern::Extend(p, v, e) => unimplemented!(),
            GraphPattern::Minus(a, b) => unimplemented!(),
            GraphPattern::Service(n, p, s) => unimplemented!(),
            GraphPattern::AggregateJoin(g, a) => unimplemented!(),
            GraphPattern::Data(bs) => PlanNode::StaticBindings {
                tuples: self.encode_bindings(bs, variables)?,
            },
            GraphPattern::OrderBy(l, o) => self.build_for_graph_pattern(l, input, variables)?, //TODO
            GraphPattern::Project(l, new_variables) => PlanNode::Project {
                child: Box::new(self.build_for_graph_pattern(
                    l,
                    input,
                    &mut new_variables.clone(),
                )?),
                mapping: new_variables
                    .iter()
                    .map(|variable| variable_key(variables, variable))
                    .collect(),
            },
            GraphPattern::Distinct(l) => PlanNode::HashDeduplicate {
                child: Box::new(self.build_for_graph_pattern(l, input, variables)?),
            },
            GraphPattern::Reduced(l) => self.build_for_graph_pattern(l, input, variables)?,
            GraphPattern::Slice(l, start, length) => {
                let mut plan = self.build_for_graph_pattern(l, input, variables)?;
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
            Expression::ConstantExpression(t) => match t {
                TermOrVariable::Term(t) => {
                    PlanExpression::Constant(self.store.encoder().encode_term(t)?)
                }
                TermOrVariable::Variable(v) => PlanExpression::Variable(variable_key(variables, v)),
            },
            Expression::OrExpression(a, b) => PlanExpression::Or(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::AndExpression(a, b) => PlanExpression::And(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::EqualExpression(a, b) => PlanExpression::Equal(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::NotEqualExpression(a, b) => PlanExpression::NotEqual(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::GreaterExpression(a, b) => PlanExpression::Greater(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::GreaterOrEqExpression(a, b) => PlanExpression::GreaterOrEq(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::LowerExpression(a, b) => PlanExpression::Lower(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::LowerOrEqExpression(a, b) => PlanExpression::LowerOrEq(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::AddExpression(a, b) => PlanExpression::Add(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::SubExpression(a, b) => PlanExpression::Sub(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::MulExpression(a, b) => PlanExpression::Mul(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::DivExpression(a, b) => PlanExpression::Div(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::UnaryPlusExpression(e) => {
                PlanExpression::UnaryPlus(Box::new(self.build_for_expression(e, variables)?))
            }
            Expression::UnaryMinusExpression(e) => {
                PlanExpression::UnaryMinus(Box::new(self.build_for_expression(e, variables)?))
            }
            Expression::UnaryNotExpression(e) => {
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
            Expression::RegexFunctionCall(a, b, c) => PlanExpression::Regex(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
                match c {
                    Some(c) => Some(Box::new(self.build_for_expression(c, variables)?)),
                    None => None,
                },
            ),
            _ => unimplemented!(),
        })
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
