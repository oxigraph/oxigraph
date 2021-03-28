use crate::model::{BlankNode, Literal, NamedNode, Term};
use crate::sparql::algebra::*;
use crate::sparql::error::EvaluationError;
use crate::sparql::model::*;
use crate::sparql::plan::*;
use crate::store::numeric_encoder::{EncodedTerm, WriteEncoder};
use std::collections::{BTreeSet, HashSet};
use std::rc::Rc;

pub(crate) struct PlanBuilder<E: WriteEncoder> {
    encoder: E,
}

impl<E: WriteEncoder<Error = EvaluationError>> PlanBuilder<E> {
    pub fn build(
        encoder: E,
        pattern: &GraphPattern,
    ) -> Result<(PlanNode<E::StrId>, Vec<Variable>), EvaluationError> {
        let mut variables = Vec::default();
        let plan = PlanBuilder { encoder }.build_for_graph_pattern(
            pattern,
            &mut variables,
            PatternValue::Constant(EncodedTerm::DefaultGraph),
        )?;
        Ok((plan, variables))
    }

    pub fn build_graph_template(
        encoder: E,
        template: &[TriplePattern],
        mut variables: Vec<Variable>,
    ) -> Result<Vec<TripleTemplate<E::StrId>>, EvaluationError> {
        PlanBuilder { encoder }.build_for_graph_template(template, &mut variables)
    }

    fn build_for_graph_pattern(
        &mut self,
        pattern: &GraphPattern,
        variables: &mut Vec<Variable>,
        graph_name: PatternValue<E::StrId>,
    ) -> Result<PlanNode<E::StrId>, EvaluationError> {
        Ok(match pattern {
            GraphPattern::BGP(p) => self.build_for_bgp(p, variables, graph_name)?,
            GraphPattern::Path {
                subject,
                path,
                object,
            } => PlanNode::PathPatternJoin {
                child: Rc::new(PlanNode::Init),
                subject: self.pattern_value_from_term_or_variable(subject, variables)?,
                path: Rc::new(self.build_for_path(path)?),
                object: self.pattern_value_from_term_or_variable(object, variables)?,
                graph_name,
            },
            GraphPattern::Join { left, right } => {
                //TODO: improve
                if let GraphPattern::Path {
                    subject,
                    path,
                    object,
                } = right.as_ref()
                {
                    let left = self.build_for_graph_pattern(left, variables, graph_name)?;
                    PlanNode::PathPatternJoin {
                        child: Rc::new(left),
                        subject: self.pattern_value_from_term_or_variable(subject, variables)?,
                        path: Rc::new(self.build_for_path(path)?),
                        object: self.pattern_value_from_term_or_variable(object, variables)?,
                        graph_name,
                    }
                } else {
                    PlanNode::Join {
                        left: Rc::new(self.build_for_graph_pattern(left, variables, graph_name)?),
                        right: Rc::new(self.build_for_graph_pattern(right, variables, graph_name)?),
                    }
                }
            }
            GraphPattern::LeftJoin { left, right, expr } => {
                let left = self.build_for_graph_pattern(left, variables, graph_name)?;
                let right = self.build_for_graph_pattern(right, variables, graph_name)?;

                let mut possible_problem_vars = BTreeSet::new();
                self.add_left_join_problematic_variables(&right, &mut possible_problem_vars);

                //We add the extra filter if needed
                let right = if let Some(expr) = expr {
                    PlanNode::Filter {
                        child: Rc::new(right),
                        expression: Rc::new(
                            self.build_for_expression(expr, variables, graph_name)?,
                        ),
                    }
                } else {
                    right
                };

                PlanNode::LeftJoin {
                    left: Rc::new(left),
                    right: Rc::new(right),
                    possible_problem_vars: Rc::new(possible_problem_vars.into_iter().collect()),
                }
            }
            GraphPattern::Filter { expr, inner } => PlanNode::Filter {
                child: Rc::new(self.build_for_graph_pattern(inner, variables, graph_name)?),
                expression: Rc::new(self.build_for_expression(expr, variables, graph_name)?),
            },
            GraphPattern::Union { left, right } => {
                //We flatten the UNIONs
                let mut stack: Vec<&GraphPattern> = vec![left, right];
                let mut children = vec![];
                loop {
                    match stack.pop() {
                        None => break,
                        Some(GraphPattern::Union { left, right }) => {
                            stack.push(left);
                            stack.push(right);
                        }
                        Some(p) => children.push(Rc::new(
                            self.build_for_graph_pattern(p, variables, graph_name)?,
                        )),
                    }
                }
                PlanNode::Union { children }
            }
            GraphPattern::Graph { graph_name, inner } => {
                let graph_name =
                    self.pattern_value_from_named_node_or_variable(graph_name, variables)?;
                self.build_for_graph_pattern(inner, variables, graph_name)?
            }
            GraphPattern::Extend { inner, var, expr } => PlanNode::Extend {
                child: Rc::new(self.build_for_graph_pattern(inner, variables, graph_name)?),
                position: variable_key(variables, var),
                expression: Rc::new(self.build_for_expression(expr, variables, graph_name)?),
            },
            GraphPattern::Minus { left, right } => PlanNode::AntiJoin {
                left: Rc::new(self.build_for_graph_pattern(left, variables, graph_name)?),
                right: Rc::new(self.build_for_graph_pattern(right, variables, graph_name)?),
            },
            GraphPattern::Service {
                name,
                pattern,
                silent,
            } => {
                // Child building should be at the begging in order for `variables` to be filled
                let child = self.build_for_graph_pattern(pattern, variables, graph_name)?;
                let service_name =
                    self.pattern_value_from_named_node_or_variable(name, variables)?;
                PlanNode::Service {
                    service_name,
                    variables: Rc::new(variables.clone()),
                    child: Rc::new(child),
                    graph_pattern: Rc::new(*pattern.clone()),
                    silent: *silent,
                }
            }
            GraphPattern::Group {
                inner,
                by,
                aggregates,
            } => {
                let mut inner_variables = by.clone();
                let inner_graph_name =
                    self.convert_pattern_value_id(graph_name, variables, &mut inner_variables);

                PlanNode::Aggregate {
                    child: Rc::new(self.build_for_graph_pattern(
                        inner,
                        &mut inner_variables,
                        inner_graph_name,
                    )?),
                    key_mapping: Rc::new(
                        by.iter()
                            .map(|k| {
                                (
                                    variable_key(&mut inner_variables, k),
                                    variable_key(variables, k),
                                )
                            })
                            .collect(),
                    ),
                    aggregates: Rc::new(
                        aggregates
                            .iter()
                            .map(|(v, a)| {
                                Ok((
                                    self.build_for_aggregate(a, &mut inner_variables, graph_name)?,
                                    variable_key(variables, v),
                                ))
                            })
                            .collect::<Result<Vec<_>, EvaluationError>>()?,
                    ),
                }
            }
            GraphPattern::Table {
                variables: table_variables,
                rows,
            } => PlanNode::StaticBindings {
                tuples: self.encode_bindings(table_variables, rows, variables)?,
            },
            GraphPattern::OrderBy { inner, condition } => {
                let condition: Result<Vec<_>, EvaluationError> = condition
                    .iter()
                    .map(|comp| match comp {
                        OrderComparator::Asc(e) => Ok(Comparator::Asc(
                            self.build_for_expression(e, variables, graph_name)?,
                        )),
                        OrderComparator::Desc(e) => Ok(Comparator::Desc(
                            self.build_for_expression(e, variables, graph_name)?,
                        )),
                    })
                    .collect();
                PlanNode::Sort {
                    child: Rc::new(self.build_for_graph_pattern(inner, variables, graph_name)?),
                    by: condition?,
                }
            }
            GraphPattern::Project { inner, projection } => {
                let mut inner_variables = projection.clone();
                let inner_graph_name =
                    self.convert_pattern_value_id(graph_name, variables, &mut inner_variables);
                PlanNode::Project {
                    child: Rc::new(self.build_for_graph_pattern(
                        inner,
                        &mut inner_variables,
                        inner_graph_name,
                    )?),
                    mapping: Rc::new(
                        projection
                            .iter()
                            .enumerate()
                            .map(|(new_variable, variable)| {
                                (new_variable, variable_key(variables, variable))
                            })
                            .collect(),
                    ),
                }
            }
            GraphPattern::Distinct { inner } => PlanNode::HashDeduplicate {
                child: Rc::new(self.build_for_graph_pattern(inner, variables, graph_name)?),
            },
            GraphPattern::Reduced { inner } => {
                self.build_for_graph_pattern(inner, variables, graph_name)?
            }
            GraphPattern::Slice {
                inner,
                start,
                length,
            } => {
                let mut plan = self.build_for_graph_pattern(inner, variables, graph_name)?;
                if *start > 0 {
                    plan = PlanNode::Skip {
                        child: Rc::new(plan),
                        count: *start,
                    };
                }
                if let Some(length) = length {
                    plan = PlanNode::Limit {
                        child: Rc::new(plan),
                        count: *length,
                    };
                }
                plan
            }
        })
    }

    fn build_for_bgp(
        &mut self,
        p: &[TriplePattern],
        variables: &mut Vec<Variable>,
        graph_name: PatternValue<E::StrId>,
    ) -> Result<PlanNode<E::StrId>, EvaluationError> {
        let mut plan = PlanNode::Init;
        for pattern in sort_bgp(p) {
            plan = PlanNode::QuadPatternJoin {
                child: Rc::new(plan),
                subject: self.pattern_value_from_term_or_variable(&pattern.subject, variables)?,
                predicate: self
                    .pattern_value_from_named_node_or_variable(&pattern.predicate, variables)?,
                object: self.pattern_value_from_term_or_variable(&pattern.object, variables)?,
                graph_name,
            }
        }
        Ok(plan)
    }

    fn build_for_path(
        &mut self,
        path: &PropertyPathExpression,
    ) -> Result<PlanPropertyPath<E::StrId>, EvaluationError> {
        Ok(match path {
            PropertyPathExpression::NamedNode(p) => {
                PlanPropertyPath::Path(self.build_named_node(p)?)
            }
            PropertyPathExpression::Reverse(p) => {
                PlanPropertyPath::Reverse(Rc::new(self.build_for_path(p)?))
            }
            PropertyPathExpression::Alternative(a, b) => PlanPropertyPath::Alternative(
                Rc::new(self.build_for_path(a)?),
                Rc::new(self.build_for_path(b)?),
            ),
            PropertyPathExpression::Sequence(a, b) => PlanPropertyPath::Sequence(
                Rc::new(self.build_for_path(a)?),
                Rc::new(self.build_for_path(b)?),
            ),
            PropertyPathExpression::ZeroOrMore(p) => {
                PlanPropertyPath::ZeroOrMore(Rc::new(self.build_for_path(p)?))
            }
            PropertyPathExpression::OneOrMore(p) => {
                PlanPropertyPath::OneOrMore(Rc::new(self.build_for_path(p)?))
            }
            PropertyPathExpression::ZeroOrOne(p) => {
                PlanPropertyPath::ZeroOrOne(Rc::new(self.build_for_path(p)?))
            }
            PropertyPathExpression::NegatedPropertySet(p) => {
                PlanPropertyPath::NegatedPropertySet(Rc::new(
                    p.iter()
                        .map(|p| self.build_named_node(p))
                        .collect::<Result<Vec<_>, _>>()?,
                ))
            }
        })
    }

    fn build_for_expression(
        &mut self,
        expression: &Expression,
        variables: &mut Vec<Variable>,
        graph_name: PatternValue<E::StrId>,
    ) -> Result<PlanExpression<E::StrId>, EvaluationError> {
        Ok(match expression {
            Expression::NamedNode(node) => PlanExpression::Constant(self.build_named_node(node)?),
            Expression::Literal(l) => PlanExpression::Constant(self.build_literal(l)?),
            Expression::Variable(v) => PlanExpression::Variable(variable_key(variables, v)),
            Expression::Or(a, b) => PlanExpression::Or(
                Box::new(self.build_for_expression(a, variables, graph_name)?),
                Box::new(self.build_for_expression(b, variables, graph_name)?),
            ),
            Expression::And(a, b) => PlanExpression::And(
                Box::new(self.build_for_expression(a, variables, graph_name)?),
                Box::new(self.build_for_expression(b, variables, graph_name)?),
            ),
            Expression::Equal(a, b) => PlanExpression::Equal(
                Box::new(self.build_for_expression(a, variables, graph_name)?),
                Box::new(self.build_for_expression(b, variables, graph_name)?),
            ),
            Expression::SameTerm(a, b) => PlanExpression::SameTerm(
                Box::new(self.build_for_expression(a, variables, graph_name)?),
                Box::new(self.build_for_expression(b, variables, graph_name)?),
            ),
            Expression::Greater(a, b) => PlanExpression::Greater(
                Box::new(self.build_for_expression(a, variables, graph_name)?),
                Box::new(self.build_for_expression(b, variables, graph_name)?),
            ),
            Expression::GreaterOrEqual(a, b) => PlanExpression::GreaterOrEqual(
                Box::new(self.build_for_expression(a, variables, graph_name)?),
                Box::new(self.build_for_expression(b, variables, graph_name)?),
            ),
            Expression::Less(a, b) => PlanExpression::Less(
                Box::new(self.build_for_expression(a, variables, graph_name)?),
                Box::new(self.build_for_expression(b, variables, graph_name)?),
            ),
            Expression::LessOrEqual(a, b) => PlanExpression::LessOrEqual(
                Box::new(self.build_for_expression(a, variables, graph_name)?),
                Box::new(self.build_for_expression(b, variables, graph_name)?),
            ),
            Expression::In(e, l) => PlanExpression::In(
                Box::new(self.build_for_expression(e, variables, graph_name)?),
                self.expression_list(l, variables, graph_name)?,
            ),
            Expression::Add(a, b) => PlanExpression::Add(
                Box::new(self.build_for_expression(a, variables, graph_name)?),
                Box::new(self.build_for_expression(b, variables, graph_name)?),
            ),
            Expression::Subtract(a, b) => PlanExpression::Subtract(
                Box::new(self.build_for_expression(a, variables, graph_name)?),
                Box::new(self.build_for_expression(b, variables, graph_name)?),
            ),
            Expression::Multiply(a, b) => PlanExpression::Multiply(
                Box::new(self.build_for_expression(a, variables, graph_name)?),
                Box::new(self.build_for_expression(b, variables, graph_name)?),
            ),
            Expression::Divide(a, b) => PlanExpression::Divide(
                Box::new(self.build_for_expression(a, variables, graph_name)?),
                Box::new(self.build_for_expression(b, variables, graph_name)?),
            ),
            Expression::UnaryPlus(e) => PlanExpression::UnaryPlus(Box::new(
                self.build_for_expression(e, variables, graph_name)?,
            )),
            Expression::UnaryMinus(e) => PlanExpression::UnaryMinus(Box::new(
                self.build_for_expression(e, variables, graph_name)?,
            )),
            Expression::Not(e) => PlanExpression::Not(Box::new(
                self.build_for_expression(e, variables, graph_name)?,
            )),
            Expression::FunctionCall(function, parameters) => match function {
                Function::Str => PlanExpression::Str(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::Lang => PlanExpression::Lang(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::LangMatches => PlanExpression::LangMatches(
                    Box::new(self.build_for_expression(&parameters[0], variables, graph_name)?),
                    Box::new(self.build_for_expression(&parameters[1], variables, graph_name)?),
                ),
                Function::Datatype => PlanExpression::Datatype(Box::new(
                    self.build_for_expression(&parameters[0], variables, graph_name)?,
                )),
                Function::IRI => PlanExpression::Iri(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::BNode => PlanExpression::BNode(match parameters.get(0) {
                    Some(e) => Some(Box::new(
                        self.build_for_expression(e, variables, graph_name)?,
                    )),
                    None => None,
                }),
                Function::Rand => PlanExpression::Rand,
                Function::Abs => PlanExpression::Abs(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::Ceil => PlanExpression::Ceil(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::Floor => PlanExpression::Floor(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::Round => PlanExpression::Round(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::Concat => {
                    PlanExpression::Concat(self.expression_list(parameters, variables, graph_name)?)
                }
                Function::SubStr => PlanExpression::SubStr(
                    Box::new(self.build_for_expression(&parameters[0], variables, graph_name)?),
                    Box::new(self.build_for_expression(&parameters[1], variables, graph_name)?),
                    match parameters.get(2) {
                        Some(flags) => Some(Box::new(
                            self.build_for_expression(flags, variables, graph_name)?,
                        )),
                        None => None,
                    },
                ),
                Function::StrLen => PlanExpression::StrLen(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::Replace => PlanExpression::Replace(
                    Box::new(self.build_for_expression(&parameters[0], variables, graph_name)?),
                    Box::new(self.build_for_expression(&parameters[1], variables, graph_name)?),
                    Box::new(self.build_for_expression(&parameters[2], variables, graph_name)?),
                    match parameters.get(3) {
                        Some(flags) => Some(Box::new(
                            self.build_for_expression(flags, variables, graph_name)?,
                        )),
                        None => None,
                    },
                ),
                Function::UCase => PlanExpression::UCase(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::LCase => PlanExpression::LCase(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::EncodeForURI => PlanExpression::EncodeForUri(Box::new(
                    self.build_for_expression(&parameters[0], variables, graph_name)?,
                )),
                Function::Contains => PlanExpression::Contains(
                    Box::new(self.build_for_expression(&parameters[0], variables, graph_name)?),
                    Box::new(self.build_for_expression(&parameters[1], variables, graph_name)?),
                ),
                Function::StrStarts => PlanExpression::StrStarts(
                    Box::new(self.build_for_expression(&parameters[0], variables, graph_name)?),
                    Box::new(self.build_for_expression(&parameters[1], variables, graph_name)?),
                ),
                Function::StrEnds => PlanExpression::StrEnds(
                    Box::new(self.build_for_expression(&parameters[0], variables, graph_name)?),
                    Box::new(self.build_for_expression(&parameters[1], variables, graph_name)?),
                ),
                Function::StrBefore => PlanExpression::StrBefore(
                    Box::new(self.build_for_expression(&parameters[0], variables, graph_name)?),
                    Box::new(self.build_for_expression(&parameters[1], variables, graph_name)?),
                ),
                Function::StrAfter => PlanExpression::StrAfter(
                    Box::new(self.build_for_expression(&parameters[0], variables, graph_name)?),
                    Box::new(self.build_for_expression(&parameters[1], variables, graph_name)?),
                ),
                Function::Year => PlanExpression::Year(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::Month => PlanExpression::Month(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::Day => PlanExpression::Day(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::Hours => PlanExpression::Hours(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::Minutes => PlanExpression::Minutes(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::Seconds => PlanExpression::Seconds(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::Timezone => PlanExpression::Timezone(Box::new(
                    self.build_for_expression(&parameters[0], variables, graph_name)?,
                )),
                Function::Tz => PlanExpression::Tz(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::Now => PlanExpression::Now,
                Function::UUID => PlanExpression::Uuid,
                Function::StrUUID => PlanExpression::StrUuid,
                Function::MD5 => PlanExpression::Md5(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::SHA1 => PlanExpression::Sha1(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::SHA256 => PlanExpression::Sha256(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::SHA384 => PlanExpression::Sha384(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::SHA512 => PlanExpression::Sha512(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::StrLang => PlanExpression::StrLang(
                    Box::new(self.build_for_expression(&parameters[0], variables, graph_name)?),
                    Box::new(self.build_for_expression(&parameters[1], variables, graph_name)?),
                ),
                Function::StrDT => PlanExpression::StrDt(
                    Box::new(self.build_for_expression(&parameters[0], variables, graph_name)?),
                    Box::new(self.build_for_expression(&parameters[1], variables, graph_name)?),
                ),
                Function::IsIRI => PlanExpression::IsIri(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::IsBlank => PlanExpression::IsBlank(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::IsLiteral => PlanExpression::IsLiteral(Box::new(
                    self.build_for_expression(&parameters[0], variables, graph_name)?,
                )),
                Function::IsNumeric => PlanExpression::IsNumeric(Box::new(
                    self.build_for_expression(&parameters[0], variables, graph_name)?,
                )),
                Function::Regex => PlanExpression::Regex(
                    Box::new(self.build_for_expression(&parameters[0], variables, graph_name)?),
                    Box::new(self.build_for_expression(&parameters[1], variables, graph_name)?),
                    match parameters.get(2) {
                        Some(flags) => Some(Box::new(
                            self.build_for_expression(flags, variables, graph_name)?,
                        )),
                        None => None,
                    },
                ),
                Function::Custom(name) => {
                    if name == "http://www.w3.org/2001/XMLSchema#boolean" {
                        self.build_cast(
                            parameters,
                            PlanExpression::BooleanCast,
                            variables,
                            graph_name,
                            "boolean",
                        )?
                    } else if name == "http://www.w3.org/2001/XMLSchema#double" {
                        self.build_cast(
                            parameters,
                            PlanExpression::DoubleCast,
                            variables,
                            graph_name,
                            "double",
                        )?
                    } else if name == "http://www.w3.org/2001/XMLSchema#float" {
                        self.build_cast(
                            parameters,
                            PlanExpression::FloatCast,
                            variables,
                            graph_name,
                            "float",
                        )?
                    } else if name == "http://www.w3.org/2001/XMLSchema#decimal" {
                        self.build_cast(
                            parameters,
                            PlanExpression::DecimalCast,
                            variables,
                            graph_name,
                            "decimal",
                        )?
                    } else if name == "http://www.w3.org/2001/XMLSchema#integer" {
                        self.build_cast(
                            parameters,
                            PlanExpression::IntegerCast,
                            variables,
                            graph_name,
                            "integer",
                        )?
                    } else if name == "http://www.w3.org/2001/XMLSchema#date" {
                        self.build_cast(
                            parameters,
                            PlanExpression::DateCast,
                            variables,
                            graph_name,
                            "date",
                        )?
                    } else if name == "http://www.w3.org/2001/XMLSchema#time" {
                        self.build_cast(
                            parameters,
                            PlanExpression::TimeCast,
                            variables,
                            graph_name,
                            "time",
                        )?
                    } else if name == "http://www.w3.org/2001/XMLSchema#dateTime" {
                        self.build_cast(
                            parameters,
                            PlanExpression::DateTimeCast,
                            variables,
                            graph_name,
                            "dateTime",
                        )?
                    } else if name == "http://www.w3.org/2001/XMLSchema#duration" {
                        self.build_cast(
                            parameters,
                            PlanExpression::DurationCast,
                            variables,
                            graph_name,
                            "duration",
                        )?
                    } else if name == "http://www.w3.org/2001/XMLSchema#yearMonthDuration" {
                        self.build_cast(
                            parameters,
                            PlanExpression::YearMonthDurationCast,
                            variables,
                            graph_name,
                            "yearMonthDuration",
                        )?
                    } else if name == "http://www.w3.org/2001/XMLSchema#dayTimeDuration" {
                        self.build_cast(
                            parameters,
                            PlanExpression::DayTimeDurationCast,
                            variables,
                            graph_name,
                            "dayTimeDuration",
                        )?
                    } else if name == "http://www.w3.org/2001/XMLSchema#string" {
                        self.build_cast(
                            parameters,
                            PlanExpression::StringCast,
                            variables,
                            graph_name,
                            "string",
                        )?
                    } else {
                        return Err(EvaluationError::msg(format!(
                            "Not supported custom function {}",
                            expression
                        )));
                    }
                }
            },
            Expression::Bound(v) => PlanExpression::Bound(variable_key(variables, v)),
            Expression::If(a, b, c) => PlanExpression::If(
                Box::new(self.build_for_expression(a, variables, graph_name)?),
                Box::new(self.build_for_expression(b, variables, graph_name)?),
                Box::new(self.build_for_expression(c, variables, graph_name)?),
            ),
            Expression::Exists(n) => PlanExpression::Exists(Rc::new(
                self.build_for_graph_pattern(n, variables, graph_name)?,
            )),
            Expression::Coalesce(parameters) => {
                PlanExpression::Coalesce(self.expression_list(parameters, variables, graph_name)?)
            }
        })
    }

    fn build_cast(
        &mut self,
        parameters: &[Expression],
        constructor: impl Fn(Box<PlanExpression<E::StrId>>) -> PlanExpression<E::StrId>,
        variables: &mut Vec<Variable>,
        graph_name: PatternValue<E::StrId>,
        name: &'static str,
    ) -> Result<PlanExpression<E::StrId>, EvaluationError> {
        if parameters.len() == 1 {
            Ok(constructor(Box::new(self.build_for_expression(
                &parameters[0],
                variables,
                graph_name,
            )?)))
        } else {
            Err(EvaluationError::msg(format!(
                "The xsd:{} casting takes only one parameter",
                name
            )))
        }
    }

    fn expression_list(
        &mut self,
        l: &[Expression],
        variables: &mut Vec<Variable>,
        graph_name: PatternValue<E::StrId>,
    ) -> Result<Vec<PlanExpression<E::StrId>>, EvaluationError> {
        l.iter()
            .map(|e| self.build_for_expression(e, variables, graph_name))
            .collect()
    }

    fn pattern_value_from_term_or_variable(
        &mut self,
        term_or_variable: &TermOrVariable,
        variables: &mut Vec<Variable>,
    ) -> Result<PatternValue<E::StrId>, EvaluationError> {
        Ok(match term_or_variable {
            TermOrVariable::Variable(variable) => {
                PatternValue::Variable(variable_key(variables, variable))
            }
            TermOrVariable::Term(Term::BlankNode(bnode)) => {
                PatternValue::Variable(variable_key(
                    variables,
                    &Variable::new_unchecked(bnode.as_str()),
                ))
                //TODO: very bad hack to convert bnode to variable
            }
            TermOrVariable::Term(term) => PatternValue::Constant(self.build_term(term)?),
        })
    }

    fn pattern_value_from_named_node_or_variable(
        &mut self,
        named_node_or_variable: &NamedNodeOrVariable,
        variables: &mut Vec<Variable>,
    ) -> Result<PatternValue<E::StrId>, EvaluationError> {
        Ok(match named_node_or_variable {
            NamedNodeOrVariable::NamedNode(named_node) => {
                PatternValue::Constant(self.build_named_node(named_node)?)
            }
            NamedNodeOrVariable::Variable(variable) => {
                PatternValue::Variable(variable_key(variables, variable))
            }
        })
    }

    fn encode_bindings(
        &mut self,
        table_variables: &[Variable],
        rows: &[Vec<Option<Term>>],
        variables: &mut Vec<Variable>,
    ) -> Result<Vec<EncodedTuple<E::StrId>>, EvaluationError> {
        let bindings_variables_keys = table_variables
            .iter()
            .map(|v| variable_key(variables, v))
            .collect::<Vec<_>>();
        rows.iter()
            .map(move |row| {
                let mut result = EncodedTuple::with_capacity(variables.len());
                for (key, value) in row.iter().enumerate() {
                    if let Some(term) = value {
                        result.set(bindings_variables_keys[key], self.build_term(term)?);
                    }
                }
                Ok(result)
            })
            .collect()
    }

    fn build_for_aggregate(
        &mut self,
        aggregate: &AggregationFunction,
        variables: &mut Vec<Variable>,
        graph_name: PatternValue<E::StrId>,
    ) -> Result<PlanAggregation<E::StrId>, EvaluationError> {
        match aggregate {
            AggregationFunction::Count { expr, distinct } => Ok(PlanAggregation {
                function: PlanAggregationFunction::Count,
                parameter: match expr {
                    Some(expr) => Some(self.build_for_expression(expr, variables, graph_name)?),
                    None => None,
                },
                distinct: *distinct,
            }),
            AggregationFunction::Sum { expr, distinct } => Ok(PlanAggregation {
                function: PlanAggregationFunction::Sum,
                parameter: Some(self.build_for_expression(expr, variables, graph_name)?),
                distinct: *distinct,
            }),
            AggregationFunction::Min { expr, distinct } => Ok(PlanAggregation {
                function: PlanAggregationFunction::Min,
                parameter: Some(self.build_for_expression(expr, variables, graph_name)?),
                distinct: *distinct,
            }),
            AggregationFunction::Max { expr, distinct } => Ok(PlanAggregation {
                function: PlanAggregationFunction::Max,
                parameter: Some(self.build_for_expression(expr, variables, graph_name)?),
                distinct: *distinct,
            }),
            AggregationFunction::Avg { expr, distinct } => Ok(PlanAggregation {
                function: PlanAggregationFunction::Avg,
                parameter: Some(self.build_for_expression(expr, variables, graph_name)?),
                distinct: *distinct,
            }),
            AggregationFunction::Sample { expr, distinct } => Ok(PlanAggregation {
                function: PlanAggregationFunction::Sample,
                parameter: Some(self.build_for_expression(expr, variables, graph_name)?),
                distinct: *distinct,
            }),
            AggregationFunction::GroupConcat {
                expr,
                distinct,
                separator,
            } => Ok(PlanAggregation {
                function: PlanAggregationFunction::GroupConcat {
                    separator: Rc::new(separator.clone().unwrap_or_else(|| " ".to_string())),
                },
                parameter: Some(self.build_for_expression(expr, variables, graph_name)?),
                distinct: *distinct,
            }),
            AggregationFunction::Custom { .. } => Err(EvaluationError::msg(
                "Custom aggregation functions are not supported yet",
            )),
        }
    }

    fn build_for_graph_template(
        &mut self,
        template: &[TriplePattern],
        variables: &mut Vec<Variable>,
    ) -> Result<Vec<TripleTemplate<E::StrId>>, EvaluationError> {
        let mut bnodes = Vec::default();
        template
            .iter()
            .map(|triple| {
                Ok(TripleTemplate {
                    subject: self.template_value_from_term_or_variable(
                        &triple.subject,
                        variables,
                        &mut bnodes,
                    )?,
                    predicate: self
                        .template_value_from_named_node_or_variable(&triple.predicate, variables)?,
                    object: self.template_value_from_term_or_variable(
                        &triple.object,
                        variables,
                        &mut bnodes,
                    )?,
                })
            })
            .collect()
    }

    fn template_value_from_term_or_variable(
        &mut self,
        term_or_variable: &TermOrVariable,
        variables: &mut Vec<Variable>,
        bnodes: &mut Vec<BlankNode>,
    ) -> Result<TripleTemplateValue<E::StrId>, EvaluationError> {
        Ok(match term_or_variable {
            TermOrVariable::Variable(variable) => {
                TripleTemplateValue::Variable(variable_key(variables, variable))
            }
            TermOrVariable::Term(Term::BlankNode(bnode)) => {
                TripleTemplateValue::BlankNode(bnode_key(bnodes, bnode))
            }
            TermOrVariable::Term(term) => TripleTemplateValue::Constant(self.build_term(term)?),
        })
    }

    fn template_value_from_named_node_or_variable(
        &mut self,
        named_node_or_variable: &NamedNodeOrVariable,
        variables: &mut Vec<Variable>,
    ) -> Result<TripleTemplateValue<E::StrId>, EvaluationError> {
        Ok(match named_node_or_variable {
            NamedNodeOrVariable::Variable(variable) => {
                TripleTemplateValue::Variable(variable_key(variables, variable))
            }
            NamedNodeOrVariable::NamedNode(term) => {
                TripleTemplateValue::Constant(self.build_named_node(term)?)
            }
        })
    }

    fn convert_pattern_value_id(
        &self,
        from_value: PatternValue<E::StrId>,
        from: &[Variable],
        to: &mut Vec<Variable>,
    ) -> PatternValue<E::StrId> {
        match from_value {
            PatternValue::Constant(v) => PatternValue::Constant(v),
            PatternValue::Variable(from_id) => {
                PatternValue::Variable(self.convert_variable_id(from_id, from, to))
            }
        }
    }

    fn convert_variable_id(
        &self,
        from_id: usize,
        from: &[Variable],
        to: &mut Vec<Variable>,
    ) -> usize {
        if let Some(to_id) = to.iter().enumerate().find_map(|(to_id, var)| {
            if *var == from[from_id] {
                Some(to_id)
            } else {
                None
            }
        }) {
            to_id
        } else {
            to.push(Variable::new_random());
            to.len() - 1
        }
    }

    fn add_left_join_problematic_variables(
        &self,
        node: &PlanNode<E::StrId>,
        set: &mut BTreeSet<usize>,
    ) {
        match node {
            PlanNode::Init
            | PlanNode::StaticBindings { .. }
            | PlanNode::QuadPatternJoin { .. }
            | PlanNode::PathPatternJoin { .. } => (),
            PlanNode::Filter { child, expression } => {
                expression.add_maybe_bound_variables(set); //TODO: only if it is not already bound
                self.add_left_join_problematic_variables(&*child, set);
            }
            PlanNode::Union { children } => {
                for child in children.iter() {
                    self.add_left_join_problematic_variables(child, set);
                }
            }
            PlanNode::Join { left, right, .. } => {
                self.add_left_join_problematic_variables(&*left, set);
                self.add_left_join_problematic_variables(&*right, set);
            }
            PlanNode::AntiJoin { left, .. } => {
                self.add_left_join_problematic_variables(&*left, set);
            }
            PlanNode::LeftJoin { left, right, .. } => {
                self.add_left_join_problematic_variables(&*left, set);
                right.add_maybe_bound_variables(set);
            }
            PlanNode::Extend {
                child, expression, ..
            } => {
                expression.add_maybe_bound_variables(set); //TODO: only if it is not already bound
                self.add_left_join_problematic_variables(&*child, set);
                self.add_left_join_problematic_variables(&*child, set);
            }
            PlanNode::Service { child, .. }
            | PlanNode::Sort { child, .. }
            | PlanNode::HashDeduplicate { child }
            | PlanNode::Skip { child, .. }
            | PlanNode::Limit { child, .. } => {
                self.add_left_join_problematic_variables(&*child, set)
            }
            PlanNode::Project { mapping, child } => {
                let mut child_bound = BTreeSet::new();
                self.add_left_join_problematic_variables(&*child, &mut child_bound);
                for (child_i, output_i) in mapping.iter() {
                    if child_bound.contains(child_i) {
                        set.insert(*output_i);
                    }
                }
            }
            PlanNode::Aggregate {
                key_mapping,
                aggregates,
                ..
            } => {
                set.extend(key_mapping.iter().map(|(_, o)| o));
                //TODO: This is too harsh
                for (_, var) in aggregates.iter() {
                    set.insert(*var);
                }
            }
        }
    }

    fn build_named_node(
        &mut self,
        node: &NamedNode,
    ) -> Result<EncodedTerm<E::StrId>, EvaluationError> {
        self.encoder.encode_named_node(node.as_ref())
    }

    fn build_literal(
        &mut self,
        literal: &Literal,
    ) -> Result<EncodedTerm<E::StrId>, EvaluationError> {
        self.encoder.encode_literal(literal.as_ref())
    }

    fn build_term(&mut self, term: &Term) -> Result<EncodedTerm<E::StrId>, EvaluationError> {
        self.encoder.encode_term(term.as_ref())
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

fn bnode_key(blank_nodes: &mut Vec<BlankNode>, blank_node: &BlankNode) -> usize {
    match slice_key(blank_nodes, blank_node) {
        Some(key) => key,
        None => {
            blank_nodes.push(blank_node.clone());
            blank_nodes.len() - 1
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

fn sort_bgp(p: &[TriplePattern]) -> Vec<&TriplePattern> {
    let mut assigned_variables = HashSet::default();
    let mut assigned_blank_nodes = HashSet::default();
    let mut new_p: Vec<_> = p.iter().collect();

    for i in 0..new_p.len() {
        (&mut new_p[i..]).sort_by(|p1, p2| {
            count_pattern_binds(p2, &assigned_variables, &assigned_blank_nodes).cmp(
                &count_pattern_binds(p1, &assigned_variables, &assigned_blank_nodes),
            )
        });
        add_pattern_variables(new_p[i], &mut assigned_variables, &mut assigned_blank_nodes);
    }

    new_p
}

fn count_pattern_binds(
    pattern: &TriplePattern,
    assigned_variables: &HashSet<&Variable>,
    assigned_blank_nodes: &HashSet<&BlankNode>,
) -> u8 {
    let mut count = 12;
    if let TermOrVariable::Variable(v) = &pattern.subject {
        if !assigned_variables.contains(v) {
            count -= 4;
        }
    } else if let TermOrVariable::Term(Term::BlankNode(bnode)) = &pattern.subject {
        if !assigned_blank_nodes.contains(bnode) {
            count -= 4;
        }
    } else {
        count -= 1;
    }
    if let NamedNodeOrVariable::Variable(v) = &pattern.predicate {
        if !assigned_variables.contains(v) {
            count -= 4;
        }
    } else {
        count -= 1;
    }
    if let TermOrVariable::Variable(v) = &pattern.object {
        if !assigned_variables.contains(v) {
            count -= 4;
        }
    } else if let TermOrVariable::Term(Term::BlankNode(bnode)) = &pattern.object {
        if !assigned_blank_nodes.contains(bnode) {
            count -= 4;
        }
    } else {
        count -= 1;
    }
    count
}

fn add_pattern_variables<'a>(
    pattern: &'a TriplePattern,
    variables: &mut HashSet<&'a Variable>,
    blank_nodes: &mut HashSet<&'a BlankNode>,
) {
    if let TermOrVariable::Variable(v) = &pattern.subject {
        variables.insert(v);
    } else if let TermOrVariable::Term(Term::BlankNode(bnode)) = &pattern.subject {
        blank_nodes.insert(bnode);
    }
    if let NamedNodeOrVariable::Variable(v) = &pattern.predicate {
        variables.insert(v);
    }
    if let TermOrVariable::Variable(v) = &pattern.object {
        variables.insert(v);
    } else if let TermOrVariable::Term(Term::BlankNode(bnode)) = &pattern.object {
        blank_nodes.insert(bnode);
    }
}
