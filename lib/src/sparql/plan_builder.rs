use crate::model::Term as OxTerm;
use crate::sparql::dataset::DatasetView;
use crate::sparql::error::EvaluationError;
use crate::sparql::eval::compile_pattern;
use crate::sparql::plan::*;
use crate::storage::numeric_encoder::{EncodedTerm, EncodedTriple};
use oxrdf::vocab::xsd;
use oxrdf::{BlankNode, Term, TermRef, Triple};
use regex::Regex;
use spargebra::term::{GroundSubject, GroundTriple, TermPattern, TriplePattern};
use sparopt::algebra::*;
use sparopt::Optimizer;
use std::collections::HashMap;
use std::rc::Rc;

pub struct PlanBuilder<'a> {
    dataset: &'a DatasetView,
    custom_functions: &'a HashMap<NamedNode, Rc<dyn Fn(&[OxTerm]) -> Option<OxTerm>>>,
}

impl<'a> PlanBuilder<'a> {
    pub fn build(
        dataset: &'a DatasetView,
        pattern: &spargebra::algebra::GraphPattern,
        is_cardinality_meaningful: bool,
        custom_functions: &'a HashMap<NamedNode, Rc<dyn Fn(&[OxTerm]) -> Option<OxTerm>>>,
        without_optimizations: bool,
    ) -> Result<(PlanNode, Vec<Variable>), EvaluationError> {
        let mut pattern = GraphPattern::from(pattern);
        if !without_optimizations {
            pattern = Optimizer::optimize_graph_pattern(pattern);
        }
        let mut variables = Vec::default();
        let plan = PlanBuilder {
            dataset,
            custom_functions,
        }
        .build_for_graph_pattern(&pattern, &mut variables)?;
        let plan = if !without_optimizations && !is_cardinality_meaningful {
            // let's reduce downstream task.
            // TODO: avoid if already REDUCED or DISTINCT
            PlanNode::Reduced {
                child: Rc::new(plan),
            }
        } else {
            plan
        };
        Ok((plan, variables))
    }

    pub fn build_graph_template(
        dataset: &'a DatasetView,
        template: &[TriplePattern],
        mut variables: Vec<Variable>,
        custom_functions: &'a HashMap<NamedNode, Rc<dyn Fn(&[OxTerm]) -> Option<OxTerm>>>,
    ) -> Vec<TripleTemplate> {
        PlanBuilder {
            dataset,
            custom_functions,
        }
        .build_for_graph_template(template, &mut variables)
    }

    fn build_for_graph_pattern(
        &self,
        pattern: &GraphPattern,
        variables: &mut Vec<Variable>,
    ) -> Result<PlanNode, EvaluationError> {
        Ok(match pattern {
            GraphPattern::QuadPattern {
                subject,
                predicate,
                object,
                graph_name,
            } => PlanNode::QuadPattern {
                subject: self.pattern_value_from_ground_term_pattern(subject, variables),
                predicate: self.pattern_value_from_named_node_or_variable(predicate, variables),
                object: self.pattern_value_from_ground_term_pattern(object, variables),
                graph_name: graph_name.as_ref().map_or(
                    PatternValue::Constant(PlanTerm {
                        encoded: EncodedTerm::DefaultGraph,
                        plain: PatternValueConstant::DefaultGraph,
                    }),
                    |g| self.pattern_value_from_named_node_or_variable(g, variables),
                ),
            },
            GraphPattern::Path {
                subject,
                path,
                object,
                graph_name,
            } => PlanNode::PathPattern {
                subject: self.pattern_value_from_ground_term_pattern(subject, variables),
                path: Rc::new(self.build_for_path(path)),
                object: self.pattern_value_from_ground_term_pattern(object, variables),
                graph_name: graph_name.as_ref().map_or(
                    PatternValue::Constant(PlanTerm {
                        encoded: EncodedTerm::DefaultGraph,
                        plain: PatternValueConstant::DefaultGraph,
                    }),
                    |g| self.pattern_value_from_named_node_or_variable(g, variables),
                ),
            },
            GraphPattern::Join {
                left,
                right,
                algorithm,
            } => match algorithm {
                JoinAlgorithm::HashBuildLeftProbeRight { keys } => PlanNode::HashJoin {
                    build_child: Rc::new(self.build_for_graph_pattern(left, variables)?),
                    probe_child: Rc::new(self.build_for_graph_pattern(right, variables)?),
                    keys: keys
                        .iter()
                        .map(|v| build_plan_variable(variables, v))
                        .collect(),
                },
            },
            GraphPattern::LeftJoin {
                left,
                right,
                expression,
                algorithm,
            } => match algorithm {
                LeftJoinAlgorithm::HashBuildRightProbeLeft { keys } => PlanNode::HashLeftJoin {
                    left: Rc::new(self.build_for_graph_pattern(left, variables)?),
                    right: Rc::new(self.build_for_graph_pattern(right, variables)?),
                    expression: Box::new(self.build_for_expression(expression, variables)?),
                    keys: keys
                        .iter()
                        .map(|v| build_plan_variable(variables, v))
                        .collect(),
                },
            },
            GraphPattern::Lateral { left, right } => {
                if let GraphPattern::LeftJoin {
                    left: nested_left,
                    right: nested_right,
                    expression,
                    ..
                } = right.as_ref()
                {
                    if nested_left.is_empty_singleton() {
                        // We are in a ForLoopLeftJoin
                        let right =
                            GraphPattern::filter(nested_right.as_ref().clone(), expression.clone());
                        PlanNode::ForLoopLeftJoin {
                            left: Rc::new(self.build_for_graph_pattern(left, variables)?),
                            right: Rc::new(self.build_for_graph_pattern(&right, variables)?),
                        }
                    } else {
                        PlanNode::ForLoopJoin {
                            left: Rc::new(self.build_for_graph_pattern(left, variables)?),
                            right: Rc::new(self.build_for_graph_pattern(right, variables)?),
                        }
                    }
                } else {
                    PlanNode::ForLoopJoin {
                        left: Rc::new(self.build_for_graph_pattern(left, variables)?),
                        right: Rc::new(self.build_for_graph_pattern(right, variables)?),
                    }
                }
            }
            GraphPattern::Filter { expression, inner } => PlanNode::Filter {
                child: Rc::new(self.build_for_graph_pattern(inner, variables)?),
                expression: Box::new(self.build_for_expression(expression, variables)?),
            },
            GraphPattern::Union { inner } => PlanNode::Union {
                children: inner
                    .iter()
                    .map(|p| Ok(Rc::new(self.build_for_graph_pattern(p, variables)?)))
                    .collect::<Result<_, EvaluationError>>()?,
            },
            GraphPattern::Extend {
                inner,
                variable,
                expression,
            } => PlanNode::Extend {
                child: Rc::new(self.build_for_graph_pattern(inner, variables)?),
                variable: build_plan_variable(variables, variable),
                expression: Box::new(self.build_for_expression(expression, variables)?),
            },
            GraphPattern::Minus {
                left,
                right,
                algorithm,
            } => match algorithm {
                MinusAlgorithm::HashBuildRightProbeLeft { keys } => PlanNode::AntiJoin {
                    left: Rc::new(self.build_for_graph_pattern(left, variables)?),
                    right: Rc::new(self.build_for_graph_pattern(right, variables)?),
                    keys: keys
                        .iter()
                        .map(|v| build_plan_variable(variables, v))
                        .collect(),
                },
            },
            GraphPattern::Service {
                name,
                inner,
                silent,
            } => {
                // Child building should be at the begging in order for `variables` to be filled
                let child = self.build_for_graph_pattern(inner, variables)?;
                let service_name = self.pattern_value_from_named_node_or_variable(name, variables);
                PlanNode::Service {
                    service_name,
                    variables: Rc::from(variables.as_slice()),
                    child: Rc::new(child),
                    graph_pattern: Rc::new(inner.as_ref().into()),
                    silent: *silent,
                }
            }
            GraphPattern::Group {
                inner,
                variables: by,
                aggregates,
            } => PlanNode::Aggregate {
                child: Rc::new(self.build_for_graph_pattern(inner, variables)?),
                key_variables: by
                    .iter()
                    .map(|k| build_plan_variable(variables, k))
                    .collect(),
                aggregates: aggregates
                    .iter()
                    .map(|(v, a)| {
                        Ok((
                            self.build_for_aggregate(a, variables)?,
                            build_plan_variable(variables, v),
                        ))
                    })
                    .collect::<Result<_, EvaluationError>>()?,
            },
            GraphPattern::Values {
                variables: table_variables,
                bindings,
            } => {
                let bindings_variables = table_variables
                    .iter()
                    .map(|v| build_plan_variable(variables, v))
                    .collect::<Vec<_>>();
                let encoded_tuples = bindings
                    .iter()
                    .map(|row| {
                        let mut result = EncodedTuple::with_capacity(variables.len());
                        for (key, value) in row.iter().enumerate() {
                            if let Some(term) = value {
                                result.set(
                                    bindings_variables[key].encoded,
                                    match term {
                                        GroundTerm::NamedNode(node) => self.build_term(node),
                                        GroundTerm::Literal(literal) => self.build_term(literal),
                                        GroundTerm::Triple(triple) => self.build_triple(triple),
                                    },
                                );
                            }
                        }
                        result
                    })
                    .collect();
                PlanNode::StaticBindings {
                    encoded_tuples,
                    variables: bindings_variables,
                    plain_bindings: bindings.clone(),
                }
            }
            GraphPattern::OrderBy { inner, expression } => {
                let condition: Result<Vec<_>, EvaluationError> = expression
                    .iter()
                    .map(|comp| match comp {
                        OrderExpression::Asc(e) => {
                            Ok(Comparator::Asc(self.build_for_expression(e, variables)?))
                        }
                        OrderExpression::Desc(e) => {
                            Ok(Comparator::Desc(self.build_for_expression(e, variables)?))
                        }
                    })
                    .collect();
                PlanNode::Sort {
                    child: Rc::new(self.build_for_graph_pattern(inner, variables)?),
                    by: condition?,
                }
            }
            GraphPattern::Project {
                inner,
                variables: projection,
            } => {
                let mut inner_variables = projection.clone();
                PlanNode::Project {
                    child: Rc::new(self.build_for_graph_pattern(inner, &mut inner_variables)?),
                    mapping: projection
                        .iter()
                        .enumerate()
                        .map(|(new_variable, variable)| {
                            (
                                PlanVariable {
                                    encoded: new_variable,
                                    plain: variable.clone(),
                                },
                                build_plan_variable(variables, variable),
                            )
                        })
                        .collect(),
                }
            }
            GraphPattern::Distinct { inner } => PlanNode::HashDeduplicate {
                child: Rc::new(self.build_for_graph_pattern(inner, variables)?),
            },
            GraphPattern::Reduced { inner } => PlanNode::Reduced {
                child: Rc::new(self.build_for_graph_pattern(inner, variables)?),
            },
            GraphPattern::Slice {
                inner,
                start,
                length,
            } => {
                let mut plan = self.build_for_graph_pattern(inner, variables)?;
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

    fn build_for_path(&self, path: &PropertyPathExpression) -> PlanPropertyPath {
        match path {
            PropertyPathExpression::NamedNode(p) => PlanPropertyPath::Path(PlanTerm {
                encoded: self.build_term(p),
                plain: p.clone(),
            }),
            PropertyPathExpression::Reverse(p) => {
                PlanPropertyPath::Reverse(Rc::new(self.build_for_path(p)))
            }
            PropertyPathExpression::Alternative(a, b) => PlanPropertyPath::Alternative(
                Rc::new(self.build_for_path(a)),
                Rc::new(self.build_for_path(b)),
            ),
            PropertyPathExpression::Sequence(a, b) => PlanPropertyPath::Sequence(
                Rc::new(self.build_for_path(a)),
                Rc::new(self.build_for_path(b)),
            ),
            PropertyPathExpression::ZeroOrMore(p) => {
                PlanPropertyPath::ZeroOrMore(Rc::new(self.build_for_path(p)))
            }
            PropertyPathExpression::OneOrMore(p) => {
                PlanPropertyPath::OneOrMore(Rc::new(self.build_for_path(p)))
            }
            PropertyPathExpression::ZeroOrOne(p) => {
                PlanPropertyPath::ZeroOrOne(Rc::new(self.build_for_path(p)))
            }
            PropertyPathExpression::NegatedPropertySet(p) => PlanPropertyPath::NegatedPropertySet(
                p.iter()
                    .map(|p| PlanTerm {
                        encoded: self.build_term(p),
                        plain: p.clone(),
                    })
                    .collect(),
            ),
        }
    }

    fn build_for_expression(
        &self,
        expression: &Expression,
        variables: &mut Vec<Variable>,
    ) -> Result<PlanExpression, EvaluationError> {
        Ok(match expression {
            Expression::NamedNode(node) => PlanExpression::NamedNode(PlanTerm {
                encoded: self.build_term(node),
                plain: node.clone(),
            }),
            Expression::Literal(l) => PlanExpression::Literal(PlanTerm {
                encoded: self.build_term(l),
                plain: l.clone(),
            }),
            Expression::Variable(v) => PlanExpression::Variable(build_plan_variable(variables, v)),
            Expression::Or(inner) => PlanExpression::Or(
                inner
                    .iter()
                    .map(|e| self.build_for_expression(e, variables))
                    .collect::<Result<_, _>>()?,
            ),
            Expression::And(inner) => PlanExpression::And(
                inner
                    .iter()
                    .map(|e| self.build_for_expression(e, variables))
                    .collect::<Result<_, _>>()?,
            ),
            Expression::Equal(a, b) => PlanExpression::Equal(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::SameTerm(a, b) => PlanExpression::SameTerm(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::Greater(a, b) => PlanExpression::Greater(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::GreaterOrEqual(a, b) => PlanExpression::GreaterOrEqual(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::Less(a, b) => PlanExpression::Less(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::LessOrEqual(a, b) => PlanExpression::LessOrEqual(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::Add(a, b) => PlanExpression::Add(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::Subtract(a, b) => PlanExpression::Subtract(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::Multiply(a, b) => PlanExpression::Multiply(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::Divide(a, b) => PlanExpression::Divide(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
            ),
            Expression::UnaryPlus(e) => {
                PlanExpression::UnaryPlus(Box::new(self.build_for_expression(e, variables)?))
            }
            Expression::UnaryMinus(e) => {
                PlanExpression::UnaryMinus(Box::new(self.build_for_expression(e, variables)?))
            }
            Expression::Not(e) => {
                PlanExpression::Not(Box::new(self.build_for_expression(e, variables)?))
            }
            Expression::FunctionCall(function, parameters) => match function {
                Function::Str => PlanExpression::Str(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Lang => PlanExpression::Lang(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::LangMatches => PlanExpression::LangMatches(
                    Box::new(self.build_for_expression(&parameters[0], variables)?),
                    Box::new(self.build_for_expression(&parameters[1], variables)?),
                ),
                Function::Datatype => PlanExpression::Datatype(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Iri => PlanExpression::Iri(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::BNode => PlanExpression::BNode(match parameters.get(0) {
                    Some(e) => Some(Box::new(self.build_for_expression(e, variables)?)),
                    None => None,
                }),
                Function::Rand => PlanExpression::Rand,
                Function::Abs => PlanExpression::Abs(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Ceil => PlanExpression::Ceil(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Floor => PlanExpression::Floor(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Round => PlanExpression::Round(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Concat => {
                    PlanExpression::Concat(self.expression_list(parameters, variables)?)
                }
                Function::SubStr => PlanExpression::SubStr(
                    Box::new(self.build_for_expression(&parameters[0], variables)?),
                    Box::new(self.build_for_expression(&parameters[1], variables)?),
                    match parameters.get(2) {
                        Some(flags) => Some(Box::new(self.build_for_expression(flags, variables)?)),
                        None => None,
                    },
                ),
                Function::StrLen => PlanExpression::StrLen(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Replace => {
                    if let Some(static_regex) =
                        compile_static_pattern_if_exists(&parameters[1], parameters.get(3))
                    {
                        PlanExpression::StaticReplace(
                            Box::new(self.build_for_expression(&parameters[0], variables)?),
                            static_regex,
                            Box::new(self.build_for_expression(&parameters[2], variables)?),
                        )
                    } else {
                        PlanExpression::DynamicReplace(
                            Box::new(self.build_for_expression(&parameters[0], variables)?),
                            Box::new(self.build_for_expression(&parameters[1], variables)?),
                            Box::new(self.build_for_expression(&parameters[2], variables)?),
                            match parameters.get(3) {
                                Some(flags) => {
                                    Some(Box::new(self.build_for_expression(flags, variables)?))
                                }
                                None => None,
                            },
                        )
                    }
                }
                Function::UCase => PlanExpression::UCase(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::LCase => PlanExpression::LCase(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::EncodeForUri => PlanExpression::EncodeForUri(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Contains => PlanExpression::Contains(
                    Box::new(self.build_for_expression(&parameters[0], variables)?),
                    Box::new(self.build_for_expression(&parameters[1], variables)?),
                ),
                Function::StrStarts => PlanExpression::StrStarts(
                    Box::new(self.build_for_expression(&parameters[0], variables)?),
                    Box::new(self.build_for_expression(&parameters[1], variables)?),
                ),
                Function::StrEnds => PlanExpression::StrEnds(
                    Box::new(self.build_for_expression(&parameters[0], variables)?),
                    Box::new(self.build_for_expression(&parameters[1], variables)?),
                ),
                Function::StrBefore => PlanExpression::StrBefore(
                    Box::new(self.build_for_expression(&parameters[0], variables)?),
                    Box::new(self.build_for_expression(&parameters[1], variables)?),
                ),
                Function::StrAfter => PlanExpression::StrAfter(
                    Box::new(self.build_for_expression(&parameters[0], variables)?),
                    Box::new(self.build_for_expression(&parameters[1], variables)?),
                ),
                Function::Year => PlanExpression::Year(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Month => PlanExpression::Month(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Day => PlanExpression::Day(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Hours => PlanExpression::Hours(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Minutes => PlanExpression::Minutes(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Seconds => PlanExpression::Seconds(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Timezone => PlanExpression::Timezone(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Tz => PlanExpression::Tz(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Now => PlanExpression::Now,
                Function::Uuid => PlanExpression::Uuid,
                Function::StrUuid => PlanExpression::StrUuid,
                Function::Md5 => PlanExpression::Md5(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Sha1 => PlanExpression::Sha1(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Sha256 => PlanExpression::Sha256(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Sha384 => PlanExpression::Sha384(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Sha512 => PlanExpression::Sha512(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::StrLang => PlanExpression::StrLang(
                    Box::new(self.build_for_expression(&parameters[0], variables)?),
                    Box::new(self.build_for_expression(&parameters[1], variables)?),
                ),
                Function::StrDt => PlanExpression::StrDt(
                    Box::new(self.build_for_expression(&parameters[0], variables)?),
                    Box::new(self.build_for_expression(&parameters[1], variables)?),
                ),
                Function::IsIri => PlanExpression::IsIri(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::IsBlank => PlanExpression::IsBlank(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::IsLiteral => PlanExpression::IsLiteral(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::IsNumeric => PlanExpression::IsNumeric(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Regex => {
                    if let Some(static_regex) =
                        compile_static_pattern_if_exists(&parameters[1], parameters.get(2))
                    {
                        PlanExpression::StaticRegex(
                            Box::new(self.build_for_expression(&parameters[0], variables)?),
                            static_regex,
                        )
                    } else {
                        PlanExpression::DynamicRegex(
                            Box::new(self.build_for_expression(&parameters[0], variables)?),
                            Box::new(self.build_for_expression(&parameters[1], variables)?),
                            match parameters.get(2) {
                                Some(flags) => {
                                    Some(Box::new(self.build_for_expression(flags, variables)?))
                                }
                                None => None,
                            },
                        )
                    }
                }
                Function::Triple => PlanExpression::Triple(
                    Box::new(self.build_for_expression(&parameters[0], variables)?),
                    Box::new(self.build_for_expression(&parameters[1], variables)?),
                    Box::new(self.build_for_expression(&parameters[2], variables)?),
                ),
                Function::Subject => PlanExpression::Subject(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Predicate => PlanExpression::Predicate(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Object => PlanExpression::Object(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::IsTriple => PlanExpression::IsTriple(Box::new(
                    self.build_for_expression(&parameters[0], variables)?,
                )),
                Function::Adjust => PlanExpression::Adjust(
                    Box::new(self.build_for_expression(&parameters[0], variables)?),
                    Box::new(self.build_for_expression(&parameters[1], variables)?),
                ),
                Function::Custom(name) => {
                    if self.custom_functions.contains_key(name) {
                        PlanExpression::CustomFunction(
                            name.clone(),
                            parameters
                                .iter()
                                .map(|p| self.build_for_expression(p, variables))
                                .collect::<Result<Vec<_>, EvaluationError>>()?,
                        )
                    } else if name.as_ref() == xsd::BOOLEAN {
                        self.build_cast(
                            parameters,
                            PlanExpression::BooleanCast,
                            variables,
                            "boolean",
                        )?
                    } else if name.as_ref() == xsd::DOUBLE {
                        self.build_cast(
                            parameters,
                            PlanExpression::DoubleCast,
                            variables,
                            "double",
                        )?
                    } else if name.as_ref() == xsd::FLOAT {
                        self.build_cast(parameters, PlanExpression::FloatCast, variables, "float")?
                    } else if name.as_ref() == xsd::DECIMAL {
                        self.build_cast(
                            parameters,
                            PlanExpression::DecimalCast,
                            variables,
                            "decimal",
                        )?
                    } else if name.as_ref() == xsd::INTEGER {
                        self.build_cast(
                            parameters,
                            PlanExpression::IntegerCast,
                            variables,
                            "integer",
                        )?
                    } else if name.as_ref() == xsd::DATE {
                        self.build_cast(parameters, PlanExpression::DateCast, variables, "date")?
                    } else if name.as_ref() == xsd::TIME {
                        self.build_cast(parameters, PlanExpression::TimeCast, variables, "time")?
                    } else if name.as_ref() == xsd::DATE_TIME {
                        self.build_cast(
                            parameters,
                            PlanExpression::DateTimeCast,
                            variables,
                            "dateTime",
                        )?
                    } else if name.as_ref() == xsd::DURATION {
                        self.build_cast(
                            parameters,
                            PlanExpression::DurationCast,
                            variables,
                            "duration",
                        )?
                    } else if name.as_ref() == xsd::YEAR_MONTH_DURATION {
                        self.build_cast(
                            parameters,
                            PlanExpression::YearMonthDurationCast,
                            variables,
                            "yearMonthDuration",
                        )?
                    } else if name.as_ref() == xsd::DAY_TIME_DURATION {
                        self.build_cast(
                            parameters,
                            PlanExpression::DayTimeDurationCast,
                            variables,
                            "dayTimeDuration",
                        )?
                    } else if name.as_ref() == xsd::STRING {
                        self.build_cast(
                            parameters,
                            PlanExpression::StringCast,
                            variables,
                            "string",
                        )?
                    } else {
                        return Err(EvaluationError::msg(format!(
                            "Not supported custom function {name}"
                        )));
                    }
                }
            },
            Expression::Bound(v) => PlanExpression::Bound(build_plan_variable(variables, v)),
            Expression::If(a, b, c) => PlanExpression::If(
                Box::new(self.build_for_expression(a, variables)?),
                Box::new(self.build_for_expression(b, variables)?),
                Box::new(self.build_for_expression(c, variables)?),
            ),
            Expression::Exists(n) => {
                let mut variables = variables.clone(); // Do not expose the exists variables outside
                PlanExpression::Exists(Rc::new(self.build_for_graph_pattern(n, &mut variables)?))
            }
            Expression::Coalesce(parameters) => {
                PlanExpression::Coalesce(self.expression_list(parameters, variables)?)
            }
        })
    }

    fn build_cast(
        &self,
        parameters: &[Expression],
        constructor: impl Fn(Box<PlanExpression>) -> PlanExpression,
        variables: &mut Vec<Variable>,
        name: &'static str,
    ) -> Result<PlanExpression, EvaluationError> {
        if parameters.len() == 1 {
            Ok(constructor(Box::new(
                self.build_for_expression(&parameters[0], variables)?,
            )))
        } else {
            Err(EvaluationError::msg(format!(
                "The xsd:{name} casting takes only one parameter"
            )))
        }
    }

    fn expression_list(
        &self,
        l: &[Expression],
        variables: &mut Vec<Variable>,
    ) -> Result<Vec<PlanExpression>, EvaluationError> {
        l.iter()
            .map(|e| self.build_for_expression(e, variables))
            .collect()
    }

    fn pattern_value_from_ground_term_pattern(
        &self,
        term_pattern: &GroundTermPattern,
        variables: &mut Vec<Variable>,
    ) -> PatternValue {
        match term_pattern {
            GroundTermPattern::Variable(variable) => {
                PatternValue::Variable(build_plan_variable(variables, variable))
            }
            GroundTermPattern::NamedNode(node) => PatternValue::Constant(PlanTerm {
                encoded: self.build_term(node),
                plain: PatternValueConstant::NamedNode(node.clone()),
            }),
            GroundTermPattern::Literal(literal) => PatternValue::Constant(PlanTerm {
                encoded: self.build_term(literal),
                plain: PatternValueConstant::Literal(literal.clone()),
            }),
            GroundTermPattern::Triple(triple) => {
                match (
                    self.pattern_value_from_ground_term_pattern(&triple.subject, variables),
                    self.pattern_value_from_named_node_or_variable(&triple.predicate, variables),
                    self.pattern_value_from_ground_term_pattern(&triple.object, variables),
                ) {
                    (
                        PatternValue::Constant(PlanTerm {
                            encoded: encoded_subject,
                            plain: plain_subject,
                        }),
                        PatternValue::Constant(PlanTerm {
                            encoded: encoded_predicate,
                            plain: plain_predicate,
                        }),
                        PatternValue::Constant(PlanTerm {
                            encoded: encoded_object,
                            plain: plain_object,
                        }),
                    ) => PatternValue::Constant(PlanTerm {
                        encoded: EncodedTriple {
                            subject: encoded_subject,
                            predicate: encoded_predicate,
                            object: encoded_object,
                        }
                        .into(),
                        plain: PatternValueConstant::Triple(Box::new(Triple {
                            subject: match plain_subject {
                                PatternValueConstant::NamedNode(s) => s.into(),
                                PatternValueConstant::Triple(s) => s.into(),
                                PatternValueConstant::Literal(_)
                                | PatternValueConstant::DefaultGraph => unreachable!(),
                            },
                            predicate: match plain_predicate {
                                PatternValueConstant::NamedNode(s) => s,
                                PatternValueConstant::Literal(_)
                                | PatternValueConstant::Triple(_)
                                | PatternValueConstant::DefaultGraph => unreachable!(),
                            },
                            object: match plain_object {
                                PatternValueConstant::NamedNode(s) => s.into(),
                                PatternValueConstant::Literal(s) => s.into(),
                                PatternValueConstant::Triple(s) => s.into(),
                                PatternValueConstant::DefaultGraph => unreachable!(),
                            },
                        })),
                    }),
                    (subject, predicate, object) => {
                        PatternValue::TriplePattern(Box::new(TriplePatternValue {
                            subject,
                            predicate,
                            object,
                        }))
                    }
                }
            }
        }
    }

    fn pattern_value_from_named_node_or_variable(
        &self,
        named_node_or_variable: &NamedNodePattern,
        variables: &mut Vec<Variable>,
    ) -> PatternValue {
        match named_node_or_variable {
            NamedNodePattern::NamedNode(named_node) => PatternValue::Constant(PlanTerm {
                encoded: self.build_term(named_node),
                plain: PatternValueConstant::NamedNode(named_node.clone()),
            }),
            NamedNodePattern::Variable(variable) => {
                PatternValue::Variable(build_plan_variable(variables, variable))
            }
        }
    }

    fn build_for_aggregate(
        &self,
        aggregate: &AggregateExpression,
        variables: &mut Vec<Variable>,
    ) -> Result<PlanAggregation, EvaluationError> {
        match aggregate {
            AggregateExpression::Count { expr, distinct } => Ok(PlanAggregation {
                function: PlanAggregationFunction::Count,
                parameter: match expr {
                    Some(expr) => Some(self.build_for_expression(expr, variables)?),
                    None => None,
                },
                distinct: *distinct,
            }),
            AggregateExpression::Sum { expr, distinct } => Ok(PlanAggregation {
                function: PlanAggregationFunction::Sum,
                parameter: Some(self.build_for_expression(expr, variables)?),
                distinct: *distinct,
            }),
            AggregateExpression::Min { expr, distinct } => Ok(PlanAggregation {
                function: PlanAggregationFunction::Min,
                parameter: Some(self.build_for_expression(expr, variables)?),
                distinct: *distinct,
            }),
            AggregateExpression::Max { expr, distinct } => Ok(PlanAggregation {
                function: PlanAggregationFunction::Max,
                parameter: Some(self.build_for_expression(expr, variables)?),
                distinct: *distinct,
            }),
            AggregateExpression::Avg { expr, distinct } => Ok(PlanAggregation {
                function: PlanAggregationFunction::Avg,
                parameter: Some(self.build_for_expression(expr, variables)?),
                distinct: *distinct,
            }),
            AggregateExpression::Sample { expr, distinct } => Ok(PlanAggregation {
                function: PlanAggregationFunction::Sample,
                parameter: Some(self.build_for_expression(expr, variables)?),
                distinct: *distinct,
            }),
            AggregateExpression::GroupConcat {
                expr,
                distinct,
                separator,
            } => Ok(PlanAggregation {
                function: PlanAggregationFunction::GroupConcat {
                    separator: Rc::from(separator.as_deref().unwrap_or(" ")),
                },
                parameter: Some(self.build_for_expression(expr, variables)?),
                distinct: *distinct,
            }),
            AggregateExpression::Custom { .. } => Err(EvaluationError::msg(
                "Custom aggregation functions are not supported yet",
            )),
        }
    }

    fn build_for_graph_template(
        &self,
        template: &[TriplePattern],
        variables: &mut Vec<Variable>,
    ) -> Vec<TripleTemplate> {
        let mut bnodes = Vec::default();
        template
            .iter()
            .map(|triple| TripleTemplate {
                subject: self.template_value_from_term_or_variable(
                    &triple.subject,
                    variables,
                    &mut bnodes,
                ),
                predicate: self
                    .template_value_from_named_node_or_variable(&triple.predicate, variables),
                object: self.template_value_from_term_or_variable(
                    &triple.object,
                    variables,
                    &mut bnodes,
                ),
            })
            .collect()
    }

    fn template_value_from_term_or_variable(
        &self,
        term_or_variable: &TermPattern,
        variables: &mut Vec<Variable>,
        bnodes: &mut Vec<BlankNode>,
    ) -> TripleTemplateValue {
        match term_or_variable {
            TermPattern::Variable(variable) => {
                TripleTemplateValue::Variable(build_plan_variable(variables, variable))
            }
            TermPattern::NamedNode(node) => TripleTemplateValue::Constant(PlanTerm {
                encoded: self.build_term(node),
                plain: node.clone().into(),
            }),
            TermPattern::BlankNode(bnode) => TripleTemplateValue::BlankNode(PlanVariable {
                encoded: bnode_key(bnodes, bnode),
                plain: bnode.clone(),
            }),
            TermPattern::Literal(literal) => TripleTemplateValue::Constant(PlanTerm {
                encoded: self.build_term(literal),
                plain: literal.clone().into(),
            }),
            TermPattern::Triple(triple) => match (
                self.template_value_from_term_or_variable(&triple.subject, variables, bnodes),
                self.template_value_from_named_node_or_variable(&triple.predicate, variables),
                self.template_value_from_term_or_variable(&triple.object, variables, bnodes),
            ) {
                (
                    TripleTemplateValue::Constant(subject),
                    TripleTemplateValue::Constant(predicate),
                    TripleTemplateValue::Constant(object),
                ) => TripleTemplateValue::Constant(PlanTerm {
                    encoded: EncodedTriple {
                        subject: subject.encoded,
                        predicate: predicate.encoded,
                        object: object.encoded,
                    }
                    .into(),
                    plain: Triple {
                        subject: match subject.plain {
                            Term::NamedNode(node) => node.into(),
                            Term::BlankNode(node) => node.into(),
                            Term::Literal(_) => unreachable!(),
                            Term::Triple(node) => node.into(),
                        },
                        predicate: match predicate.plain {
                            Term::NamedNode(node) => node,
                            Term::BlankNode(_) | Term::Literal(_) | Term::Triple(_) => {
                                unreachable!()
                            }
                        },
                        object: object.plain,
                    }
                    .into(),
                }),
                (subject, predicate, object) => {
                    TripleTemplateValue::Triple(Box::new(TripleTemplate {
                        subject,
                        predicate,
                        object,
                    }))
                }
            },
        }
    }

    fn template_value_from_named_node_or_variable(
        &self,
        named_node_or_variable: &NamedNodePattern,
        variables: &mut Vec<Variable>,
    ) -> TripleTemplateValue {
        match named_node_or_variable {
            NamedNodePattern::Variable(variable) => {
                TripleTemplateValue::Variable(build_plan_variable(variables, variable))
            }
            NamedNodePattern::NamedNode(term) => TripleTemplateValue::Constant(PlanTerm {
                encoded: self.build_term(term),
                plain: term.clone().into(),
            }),
        }
    }

    fn build_term<'b>(&self, term: impl Into<TermRef<'b>>) -> EncodedTerm {
        self.dataset.encode_term(term)
    }

    fn build_triple(&self, triple: &GroundTriple) -> EncodedTerm {
        EncodedTriple::new(
            match &triple.subject {
                GroundSubject::NamedNode(node) => self.build_term(node),
                GroundSubject::Triple(triple) => self.build_triple(triple),
            },
            self.build_term(&triple.predicate),
            match &triple.object {
                GroundTerm::NamedNode(node) => self.build_term(node),
                GroundTerm::Literal(literal) => self.build_term(literal),
                GroundTerm::Triple(triple) => self.build_triple(triple),
            },
        )
        .into()
    }
}

fn build_plan_variable(variables: &mut Vec<Variable>, variable: &Variable) -> PlanVariable {
    let encoded = if let Some(key) = slice_key(variables, variable) {
        key
    } else {
        variables.push(variable.clone());
        variables.len() - 1
    };
    PlanVariable {
        plain: variable.clone(),
        encoded,
    }
}

fn bnode_key(blank_nodes: &mut Vec<BlankNode>, blank_node: &BlankNode) -> usize {
    if let Some(key) = slice_key(blank_nodes, blank_node) {
        key
    } else {
        blank_nodes.push(blank_node.clone());
        blank_nodes.len() - 1
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

fn compile_static_pattern_if_exists(
    pattern: &Expression,
    options: Option<&Expression>,
) -> Option<Regex> {
    let static_pattern = if let Expression::Literal(pattern) = pattern {
        (pattern.datatype() == xsd::STRING).then(|| pattern.value())
    } else {
        None
    };
    let static_options = if let Some(options) = options {
        if let Expression::Literal(options) = options {
            (options.datatype() == xsd::STRING).then(|| Some(options.value()))
        } else {
            None
        }
    } else {
        Some(None)
    };
    if let (Some(static_pattern), Some(static_options)) = (static_pattern, static_options) {
        compile_pattern(static_pattern, static_options)
    } else {
        None
    }
}
