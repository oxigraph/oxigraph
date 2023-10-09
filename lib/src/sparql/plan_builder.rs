use crate::model::Term as OxTerm;
use crate::sparql::dataset::DatasetView;
use crate::sparql::error::EvaluationError;
use crate::sparql::eval::compile_pattern;
use crate::sparql::plan::*;
use crate::storage::numeric_encoder::{EncodedTerm, EncodedTriple};
use oxrdf::vocab::xsd;
use oxrdf::TermRef;
use rand::random;
use regex::Regex;
use spargebra::algebra::*;
use spargebra::term::*;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::mem::swap;
use std::rc::Rc;

pub struct PlanBuilder<'a> {
    dataset: &'a DatasetView,
    custom_functions: &'a HashMap<NamedNode, Rc<dyn Fn(&[OxTerm]) -> Option<OxTerm>>>,
    with_optimizations: bool,
}

impl<'a> PlanBuilder<'a> {
    pub fn build(
        dataset: &'a DatasetView,
        pattern: &GraphPattern,
        is_cardinality_meaningful: bool,
        custom_functions: &'a HashMap<NamedNode, Rc<dyn Fn(&[OxTerm]) -> Option<OxTerm>>>,
        without_optimizations: bool,
    ) -> Result<(PlanNode, Vec<Variable>), EvaluationError> {
        let mut variables = Vec::default();
        let plan = PlanBuilder {
            dataset,
            custom_functions,
            with_optimizations: !without_optimizations,
        }
        .build_for_graph_pattern(
            pattern,
            &mut variables,
            &PatternValue::Constant(PlanTerm {
                encoded: EncodedTerm::DefaultGraph,
                plain: PatternValueConstant::DefaultGraph,
            }),
        )?;
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
        without_optimizations: bool,
    ) -> Vec<TripleTemplate> {
        PlanBuilder {
            dataset,
            custom_functions,
            with_optimizations: !without_optimizations,
        }
        .build_for_graph_template(template, &mut variables)
    }

    fn build_for_graph_pattern(
        &self,
        pattern: &GraphPattern,
        variables: &mut Vec<Variable>,
        graph_name: &PatternValue,
    ) -> Result<PlanNode, EvaluationError> {
        Ok(match pattern {
            GraphPattern::Bgp { patterns } => {
                if self.with_optimizations {
                    self.build_for_bgp(sort_bgp(patterns), variables, graph_name)
                } else {
                    self.build_for_bgp(patterns, variables, graph_name)
                }
            }
            GraphPattern::Path {
                subject,
                path,
                object,
            } => PlanNode::PathPattern {
                subject: self.pattern_value_from_term_or_variable(subject, variables),
                path: Rc::new(self.build_for_path(path)),
                object: self.pattern_value_from_term_or_variable(object, variables),
                graph_name: graph_name.clone(),
            },
            GraphPattern::Join { left, right } => self.new_join(
                self.build_for_graph_pattern(left, variables, graph_name)?,
                self.build_for_graph_pattern(right, variables, graph_name)?,
            ),
            GraphPattern::LeftJoin {
                left,
                right,
                expression,
            } => {
                let left = self.build_for_graph_pattern(left, variables, graph_name)?;
                let right = self.build_for_graph_pattern(right, variables, graph_name)?;

                if self.with_optimizations && Self::can_use_for_loop_left_join(&right) {
                    let mut possible_problem_vars = BTreeSet::new();
                    Self::add_left_join_problematic_variables(&right, &mut possible_problem_vars);

                    //We add the extra filter if needed
                    let right = if let Some(expr) = expression {
                        self.push_filter(
                            Rc::new(right),
                            Box::new(self.build_for_expression(expr, variables, graph_name)?),
                        )
                    } else {
                        right
                    };
                    PlanNode::ForLoopLeftJoin {
                        left: Rc::new(left),
                        right: Rc::new(right),
                        possible_problem_vars: possible_problem_vars.into_iter().collect(),
                    }
                } else {
                    PlanNode::HashLeftJoin {
                        left: Rc::new(left),
                        right: Rc::new(right),
                        expression: Box::new(expression.as_ref().map_or(
                            Ok(PlanExpression::Literal(PlanTerm {
                                encoded: true.into(),
                                plain: true.into(),
                            })),
                            |e| self.build_for_expression(e, variables, graph_name),
                        )?),
                    }
                }
            }
            GraphPattern::Lateral { left, right } => PlanNode::ForLoopJoin {
                left: Rc::new(self.build_for_graph_pattern(left, variables, graph_name)?),
                right: Rc::new(self.build_for_graph_pattern(right, variables, graph_name)?),
            },
            GraphPattern::Filter { expr, inner } => self.push_filter(
                Rc::new(self.build_for_graph_pattern(inner, variables, graph_name)?),
                Box::new(self.build_for_expression(expr, variables, graph_name)?),
            ),
            GraphPattern::Union { left, right } => {
                //We flatten the UNION
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
            GraphPattern::Graph { name, inner } => {
                let graph_name = self.pattern_value_from_named_node_or_variable(name, variables);
                self.build_for_graph_pattern(inner, variables, &graph_name)?
            }
            GraphPattern::Extend {
                inner,
                variable,
                expression,
            } => PlanNode::Extend {
                child: Rc::new(self.build_for_graph_pattern(inner, variables, graph_name)?),
                variable: build_plan_variable(variables, variable),
                expression: Box::new(self.build_for_expression(expression, variables, graph_name)?),
            },
            GraphPattern::Minus { left, right } => PlanNode::AntiJoin {
                left: Rc::new(self.build_for_graph_pattern(left, variables, graph_name)?),
                right: Rc::new(self.build_for_graph_pattern(right, variables, graph_name)?),
            },
            GraphPattern::Service {
                name,
                inner,
                silent,
            } => {
                // Child building should be at the begging in order for `variables` to be filled
                let child = self.build_for_graph_pattern(inner, variables, graph_name)?;
                let service_name = self.pattern_value_from_named_node_or_variable(name, variables);
                PlanNode::Service {
                    service_name,
                    variables: Rc::from(variables.as_slice()),
                    child: Rc::new(child),
                    graph_pattern: Rc::new(inner.as_ref().clone()),
                    silent: *silent,
                }
            }
            GraphPattern::Group {
                inner,
                variables: by,
                aggregates,
            } => PlanNode::Aggregate {
                child: Rc::new(self.build_for_graph_pattern(inner, variables, graph_name)?),
                key_variables: by
                    .iter()
                    .map(|k| build_plan_variable(variables, k))
                    .collect(),
                aggregates: aggregates
                    .iter()
                    .map(|(v, a)| {
                        Ok((
                            self.build_for_aggregate(a, variables, graph_name)?,
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
                        OrderExpression::Asc(e) => Ok(Comparator::Asc(
                            self.build_for_expression(e, variables, graph_name)?,
                        )),
                        OrderExpression::Desc(e) => Ok(Comparator::Desc(
                            self.build_for_expression(e, variables, graph_name)?,
                        )),
                    })
                    .collect();
                PlanNode::Sort {
                    child: Rc::new(self.build_for_graph_pattern(inner, variables, graph_name)?),
                    by: condition?,
                }
            }
            GraphPattern::Project {
                inner,
                variables: projection,
            } => {
                let mut inner_variables = projection.clone();
                let inner_graph_name =
                    Self::convert_pattern_value_id(graph_name, &mut inner_variables);
                PlanNode::Project {
                    child: Rc::new(self.build_for_graph_pattern(
                        inner,
                        &mut inner_variables,
                        &inner_graph_name,
                    )?),
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
                child: Rc::new(self.build_for_graph_pattern(inner, variables, graph_name)?),
            },
            GraphPattern::Reduced { inner } => PlanNode::Reduced {
                child: Rc::new(self.build_for_graph_pattern(inner, variables, graph_name)?),
            },
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

    fn build_for_bgp<'b>(
        &self,
        patterns: impl IntoIterator<Item = &'b TriplePattern>,
        variables: &mut Vec<Variable>,
        graph_name: &PatternValue,
    ) -> PlanNode {
        patterns
            .into_iter()
            .map(|triple| PlanNode::QuadPattern {
                subject: self.pattern_value_from_term_or_variable(&triple.subject, variables),
                predicate: self
                    .pattern_value_from_named_node_or_variable(&triple.predicate, variables),
                object: self.pattern_value_from_term_or_variable(&triple.object, variables),
                graph_name: graph_name.clone(),
            })
            .reduce(|a, b| self.new_join(a, b))
            .unwrap_or_else(|| PlanNode::StaticBindings {
                encoded_tuples: vec![EncodedTuple::with_capacity(variables.len())],
                variables: Vec::new(),
                plain_bindings: vec![Vec::new()],
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
        graph_name: &PatternValue,
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
            Expression::Or(a, b) => PlanExpression::Or(vec![
                self.build_for_expression(a, variables, graph_name)?,
                self.build_for_expression(b, variables, graph_name)?,
            ]),
            Expression::And(a, b) => PlanExpression::And(vec![
                self.build_for_expression(a, variables, graph_name)?,
                self.build_for_expression(b, variables, graph_name)?,
            ]),
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
            Expression::In(e, l) => {
                let e = self.build_for_expression(e, variables, graph_name)?;
                if l.is_empty() {
                    // False except on error
                    PlanExpression::If(
                        Box::new(e),
                        Box::new(PlanExpression::Literal(PlanTerm {
                            encoded: false.into(),
                            plain: false.into(),
                        })),
                        Box::new(PlanExpression::Literal(PlanTerm {
                            encoded: false.into(),
                            plain: false.into(),
                        })),
                    )
                } else {
                    PlanExpression::Or(
                        l.iter()
                            .map(|v| {
                                Ok(PlanExpression::Equal(
                                    Box::new(e.clone()),
                                    Box::new(self.build_for_expression(v, variables, graph_name)?),
                                ))
                            })
                            .collect::<Result<_, EvaluationError>>()?,
                    )
                }
            }
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
                Function::Iri => PlanExpression::Iri(Box::new(self.build_for_expression(
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
                Function::Replace => {
                    if let Some(static_regex) =
                        compile_static_pattern_if_exists(&parameters[1], parameters.get(3))
                    {
                        PlanExpression::StaticReplace(
                            Box::new(self.build_for_expression(
                                &parameters[0],
                                variables,
                                graph_name,
                            )?),
                            static_regex,
                            Box::new(self.build_for_expression(
                                &parameters[2],
                                variables,
                                graph_name,
                            )?),
                        )
                    } else {
                        PlanExpression::DynamicReplace(
                            Box::new(self.build_for_expression(
                                &parameters[0],
                                variables,
                                graph_name,
                            )?),
                            Box::new(self.build_for_expression(
                                &parameters[1],
                                variables,
                                graph_name,
                            )?),
                            Box::new(self.build_for_expression(
                                &parameters[2],
                                variables,
                                graph_name,
                            )?),
                            match parameters.get(3) {
                                Some(flags) => Some(Box::new(
                                    self.build_for_expression(flags, variables, graph_name)?,
                                )),
                                None => None,
                            },
                        )
                    }
                }
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
                Function::EncodeForUri => PlanExpression::EncodeForUri(Box::new(
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
                Function::Uuid => PlanExpression::Uuid,
                Function::StrUuid => PlanExpression::StrUuid,
                Function::Md5 => PlanExpression::Md5(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::Sha1 => PlanExpression::Sha1(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::Sha256 => PlanExpression::Sha256(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::Sha384 => PlanExpression::Sha384(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::Sha512 => PlanExpression::Sha512(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::StrLang => PlanExpression::StrLang(
                    Box::new(self.build_for_expression(&parameters[0], variables, graph_name)?),
                    Box::new(self.build_for_expression(&parameters[1], variables, graph_name)?),
                ),
                Function::StrDt => PlanExpression::StrDt(
                    Box::new(self.build_for_expression(&parameters[0], variables, graph_name)?),
                    Box::new(self.build_for_expression(&parameters[1], variables, graph_name)?),
                ),
                Function::IsIri => PlanExpression::IsIri(Box::new(self.build_for_expression(
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
                Function::Regex => {
                    if let Some(static_regex) =
                        compile_static_pattern_if_exists(&parameters[1], parameters.get(2))
                    {
                        PlanExpression::StaticRegex(
                            Box::new(self.build_for_expression(
                                &parameters[0],
                                variables,
                                graph_name,
                            )?),
                            static_regex,
                        )
                    } else {
                        PlanExpression::DynamicRegex(
                            Box::new(self.build_for_expression(
                                &parameters[0],
                                variables,
                                graph_name,
                            )?),
                            Box::new(self.build_for_expression(
                                &parameters[1],
                                variables,
                                graph_name,
                            )?),
                            match parameters.get(2) {
                                Some(flags) => Some(Box::new(
                                    self.build_for_expression(flags, variables, graph_name)?,
                                )),
                                None => None,
                            },
                        )
                    }
                }
                Function::Triple => PlanExpression::Triple(
                    Box::new(self.build_for_expression(&parameters[0], variables, graph_name)?),
                    Box::new(self.build_for_expression(&parameters[1], variables, graph_name)?),
                    Box::new(self.build_for_expression(&parameters[2], variables, graph_name)?),
                ),
                Function::Subject => PlanExpression::Subject(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::Predicate => PlanExpression::Predicate(Box::new(
                    self.build_for_expression(&parameters[0], variables, graph_name)?,
                )),
                Function::Object => PlanExpression::Object(Box::new(self.build_for_expression(
                    &parameters[0],
                    variables,
                    graph_name,
                )?)),
                Function::IsTriple => PlanExpression::IsTriple(Box::new(
                    self.build_for_expression(&parameters[0], variables, graph_name)?,
                )),
                Function::Adjust => PlanExpression::Adjust(
                    Box::new(self.build_for_expression(&parameters[0], variables, graph_name)?),
                    Box::new(self.build_for_expression(&parameters[1], variables, graph_name)?),
                ),
                Function::Custom(name) => {
                    if self.custom_functions.contains_key(name) {
                        PlanExpression::CustomFunction(
                            name.clone(),
                            parameters
                                .iter()
                                .map(|p| self.build_for_expression(p, variables, graph_name))
                                .collect::<Result<Vec<_>, EvaluationError>>()?,
                        )
                    } else if name.as_ref() == xsd::BOOLEAN {
                        self.build_cast(
                            parameters,
                            PlanExpression::BooleanCast,
                            variables,
                            graph_name,
                            "boolean",
                        )?
                    } else if name.as_ref() == xsd::DOUBLE {
                        self.build_cast(
                            parameters,
                            PlanExpression::DoubleCast,
                            variables,
                            graph_name,
                            "double",
                        )?
                    } else if name.as_ref() == xsd::FLOAT {
                        self.build_cast(
                            parameters,
                            PlanExpression::FloatCast,
                            variables,
                            graph_name,
                            "float",
                        )?
                    } else if name.as_ref() == xsd::DECIMAL {
                        self.build_cast(
                            parameters,
                            PlanExpression::DecimalCast,
                            variables,
                            graph_name,
                            "decimal",
                        )?
                    } else if name.as_ref() == xsd::INTEGER {
                        self.build_cast(
                            parameters,
                            PlanExpression::IntegerCast,
                            variables,
                            graph_name,
                            "integer",
                        )?
                    } else if name.as_ref() == xsd::DATE {
                        self.build_cast(
                            parameters,
                            PlanExpression::DateCast,
                            variables,
                            graph_name,
                            "date",
                        )?
                    } else if name.as_ref() == xsd::TIME {
                        self.build_cast(
                            parameters,
                            PlanExpression::TimeCast,
                            variables,
                            graph_name,
                            "time",
                        )?
                    } else if name.as_ref() == xsd::DATE_TIME {
                        self.build_cast(
                            parameters,
                            PlanExpression::DateTimeCast,
                            variables,
                            graph_name,
                            "dateTime",
                        )?
                    } else if name.as_ref() == xsd::DURATION {
                        self.build_cast(
                            parameters,
                            PlanExpression::DurationCast,
                            variables,
                            graph_name,
                            "duration",
                        )?
                    } else if name.as_ref() == xsd::YEAR_MONTH_DURATION {
                        self.build_cast(
                            parameters,
                            PlanExpression::YearMonthDurationCast,
                            variables,
                            graph_name,
                            "yearMonthDuration",
                        )?
                    } else if name.as_ref() == xsd::DAY_TIME_DURATION {
                        self.build_cast(
                            parameters,
                            PlanExpression::DayTimeDurationCast,
                            variables,
                            graph_name,
                            "dayTimeDuration",
                        )?
                    } else if name.as_ref() == xsd::STRING {
                        self.build_cast(
                            parameters,
                            PlanExpression::StringCast,
                            variables,
                            graph_name,
                            "string",
                        )?
                    } else {
                        return Err(EvaluationError::msg(format!(
                            "Not supported custom function {expression}"
                        )));
                    }
                }
            },
            Expression::Bound(v) => PlanExpression::Bound(build_plan_variable(variables, v)),
            Expression::If(a, b, c) => PlanExpression::If(
                Box::new(self.build_for_expression(a, variables, graph_name)?),
                Box::new(self.build_for_expression(b, variables, graph_name)?),
                Box::new(self.build_for_expression(c, variables, graph_name)?),
            ),
            Expression::Exists(n) => {
                let mut variables = variables.clone(); // Do not expose the exists variables outside
                PlanExpression::Exists(Rc::new(self.build_for_graph_pattern(
                    n,
                    &mut variables,
                    graph_name,
                )?))
            }
            Expression::Coalesce(parameters) => {
                PlanExpression::Coalesce(self.expression_list(parameters, variables, graph_name)?)
            }
        })
    }

    fn build_cast(
        &self,
        parameters: &[Expression],
        constructor: impl Fn(Box<PlanExpression>) -> PlanExpression,
        variables: &mut Vec<Variable>,
        graph_name: &PatternValue,
        name: &'static str,
    ) -> Result<PlanExpression, EvaluationError> {
        if parameters.len() == 1 {
            Ok(constructor(Box::new(self.build_for_expression(
                &parameters[0],
                variables,
                graph_name,
            )?)))
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
        graph_name: &PatternValue,
    ) -> Result<Vec<PlanExpression>, EvaluationError> {
        l.iter()
            .map(|e| self.build_for_expression(e, variables, graph_name))
            .collect()
    }

    fn pattern_value_from_term_or_variable(
        &self,
        term_or_variable: &TermPattern,
        variables: &mut Vec<Variable>,
    ) -> PatternValue {
        match term_or_variable {
            TermPattern::Variable(variable) => {
                PatternValue::Variable(build_plan_variable(variables, variable))
            }
            TermPattern::NamedNode(node) => PatternValue::Constant(PlanTerm {
                encoded: self.build_term(node),
                plain: PatternValueConstant::NamedNode(node.clone()),
            }),
            TermPattern::BlankNode(bnode) => {
                PatternValue::Variable(build_plan_variable(
                    variables,
                    &Variable::new_unchecked(bnode.as_str()),
                ))
                //TODO: very bad hack to convert bnode to variable
            }
            TermPattern::Literal(literal) => PatternValue::Constant(PlanTerm {
                encoded: self.build_term(literal),
                plain: PatternValueConstant::Literal(literal.clone()),
            }),
            TermPattern::Triple(triple) => {
                match (
                    self.pattern_value_from_term_or_variable(&triple.subject, variables),
                    self.pattern_value_from_named_node_or_variable(&triple.predicate, variables),
                    self.pattern_value_from_term_or_variable(&triple.object, variables),
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
        graph_name: &PatternValue,
    ) -> Result<PlanAggregation, EvaluationError> {
        match aggregate {
            AggregateExpression::Count { expr, distinct } => Ok(PlanAggregation {
                function: PlanAggregationFunction::Count,
                parameter: match expr {
                    Some(expr) => Some(self.build_for_expression(expr, variables, graph_name)?),
                    None => None,
                },
                distinct: *distinct,
            }),
            AggregateExpression::Sum { expr, distinct } => Ok(PlanAggregation {
                function: PlanAggregationFunction::Sum,
                parameter: Some(self.build_for_expression(expr, variables, graph_name)?),
                distinct: *distinct,
            }),
            AggregateExpression::Min { expr, distinct } => Ok(PlanAggregation {
                function: PlanAggregationFunction::Min,
                parameter: Some(self.build_for_expression(expr, variables, graph_name)?),
                distinct: *distinct,
            }),
            AggregateExpression::Max { expr, distinct } => Ok(PlanAggregation {
                function: PlanAggregationFunction::Max,
                parameter: Some(self.build_for_expression(expr, variables, graph_name)?),
                distinct: *distinct,
            }),
            AggregateExpression::Avg { expr, distinct } => Ok(PlanAggregation {
                function: PlanAggregationFunction::Avg,
                parameter: Some(self.build_for_expression(expr, variables, graph_name)?),
                distinct: *distinct,
            }),
            AggregateExpression::Sample { expr, distinct } => Ok(PlanAggregation {
                function: PlanAggregationFunction::Sample,
                parameter: Some(self.build_for_expression(expr, variables, graph_name)?),
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
                parameter: Some(self.build_for_expression(expr, variables, graph_name)?),
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

    fn convert_pattern_value_id(from_value: &PatternValue, to: &mut Vec<Variable>) -> PatternValue {
        match from_value {
            PatternValue::Constant(c) => PatternValue::Constant(c.clone()),
            PatternValue::Variable(from_id) => {
                PatternValue::Variable(Self::convert_plan_variable(from_id, to))
            }
            PatternValue::TriplePattern(triple) => {
                PatternValue::TriplePattern(Box::new(TriplePatternValue {
                    subject: Self::convert_pattern_value_id(&triple.subject, to),
                    predicate: Self::convert_pattern_value_id(&triple.predicate, to),
                    object: Self::convert_pattern_value_id(&triple.object, to),
                }))
            }
        }
    }

    fn convert_plan_variable(from_variable: &PlanVariable, to: &mut Vec<Variable>) -> PlanVariable {
        let encoded = if let Some(to_id) = to
            .iter()
            .enumerate()
            .find_map(|(to_id, var)| (*var == from_variable.plain).then(|| to_id))
        {
            to_id
        } else {
            to.push(Variable::new_unchecked(format!("{:x}", random::<u128>())));
            to.len() - 1
        };
        PlanVariable {
            encoded,
            plain: from_variable.plain.clone(),
        }
    }

    fn can_use_for_loop_left_join(node: &PlanNode) -> bool {
        // We forbid MINUS, SERVICE and everything that affects cardinality in for loop left joins
        match node {
            PlanNode::StaticBindings { .. }
            | PlanNode::QuadPattern { .. }
            | PlanNode::PathPattern { .. } => true,
            PlanNode::Filter { child, .. }
            | PlanNode::Extend { child, .. }
            | PlanNode::Sort { child, .. }
            | PlanNode::Project { child, .. }
            | PlanNode::Aggregate { child, .. } => Self::can_use_for_loop_left_join(child),
            PlanNode::Union { children } => {
                children.iter().all(|c| Self::can_use_for_loop_left_join(c))
            }
            PlanNode::HashJoin { left, right }
            | PlanNode::ForLoopJoin { left, right }
            | PlanNode::ForLoopLeftJoin { left, right, .. }
            | PlanNode::HashLeftJoin { left, right, .. } => {
                Self::can_use_for_loop_left_join(left) && Self::can_use_for_loop_left_join(right)
            }
            PlanNode::AntiJoin { .. }
            | PlanNode::Service { .. }
            | PlanNode::HashDeduplicate { .. }
            | PlanNode::Reduced { .. }
            | PlanNode::Skip { .. }
            | PlanNode::Limit { .. } => false,
        }
    }

    fn add_left_join_problematic_variables(node: &PlanNode, set: &mut BTreeSet<usize>) {
        match node {
            PlanNode::StaticBindings { .. }
            | PlanNode::QuadPattern { .. }
            | PlanNode::PathPattern { .. } => (),
            PlanNode::Filter { child, expression } => {
                let always_already_bound = child.always_bound_variables();
                expression.lookup_used_variables(&mut |v| {
                    if !always_already_bound.contains(&v) {
                        set.insert(v);
                    }
                });
                Self::add_left_join_problematic_variables(child, set);
            }
            PlanNode::Union { children } => {
                for child in children {
                    Self::add_left_join_problematic_variables(child, set);
                }
            }
            PlanNode::HashJoin { left, right } | PlanNode::ForLoopJoin { left, right } => {
                Self::add_left_join_problematic_variables(left, set);
                Self::add_left_join_problematic_variables(right, set);
            }
            PlanNode::AntiJoin { left, .. } => {
                Self::add_left_join_problematic_variables(left, set);
            }
            PlanNode::ForLoopLeftJoin { left, right, .. } => {
                Self::add_left_join_problematic_variables(left, set);
                right.lookup_used_variables(&mut |v| {
                    set.insert(v);
                });
            }
            PlanNode::HashLeftJoin {
                left,
                right,
                expression,
            } => {
                Self::add_left_join_problematic_variables(left, set);
                right.lookup_used_variables(&mut |v| {
                    set.insert(v);
                });
                let always_already_bound = left.always_bound_variables();
                expression.lookup_used_variables(&mut |v| {
                    if !always_already_bound.contains(&v) {
                        set.insert(v);
                    }
                });
            }
            PlanNode::Extend {
                child, expression, ..
            } => {
                let always_already_bound = child.always_bound_variables();
                expression.lookup_used_variables(&mut |v| {
                    if !always_already_bound.contains(&v) {
                        set.insert(v);
                    }
                });
                Self::add_left_join_problematic_variables(child, set);
                Self::add_left_join_problematic_variables(child, set);
            }
            PlanNode::Sort { child, .. }
            | PlanNode::HashDeduplicate { child }
            | PlanNode::Reduced { child }
            | PlanNode::Project { child, .. } => {
                Self::add_left_join_problematic_variables(child, set);
            }
            PlanNode::Skip { child, .. } | PlanNode::Limit { child, .. } => {
                // Any variable might affect arity
                child.lookup_used_variables(&mut |v| {
                    set.insert(v);
                })
            }
            PlanNode::Service { child, silent, .. } => {
                if *silent {
                    child.lookup_used_variables(&mut |v| {
                        set.insert(v);
                    });
                } else {
                    Self::add_left_join_problematic_variables(child, set)
                }
            }
            PlanNode::Aggregate {
                key_variables,
                aggregates,
                ..
            } => {
                set.extend(key_variables.iter().map(|v| v.encoded));
                //TODO: This is too harsh
                for (_, var) in aggregates.iter() {
                    set.insert(var.encoded);
                }
            }
        }
    }

    fn new_join(&self, mut left: PlanNode, mut right: PlanNode) -> PlanNode {
        // We first use VALUES to filter the following patterns evaluation
        if matches!(right, PlanNode::StaticBindings { .. }) {
            swap(&mut left, &mut right);
        }

        if self.with_optimizations
            && Self::is_fit_for_for_loop_join(&right)
            && Self::has_some_common_variables(&left, &right)
        {
            PlanNode::ForLoopJoin {
                left: Rc::new(left),
                right: Rc::new(right),
            }
        } else {
            // Let's avoid materializing right if left is already materialized
            // TODO: be smarter and reuse already existing materialization
            if matches!(left, PlanNode::StaticBindings { .. }) {
                swap(&mut left, &mut right);
            }
            PlanNode::HashJoin {
                left: Rc::new(left),
                right: Rc::new(right),
            }
        }
    }

    fn has_some_common_variables(left: &PlanNode, right: &PlanNode) -> bool {
        left.always_bound_variables()
            .intersection(&right.always_bound_variables())
            .next()
            .is_some()
    }

    fn is_fit_for_for_loop_join(node: &PlanNode) -> bool {
        //TODO: think more about it
        match node {
            PlanNode::StaticBindings { .. }
            | PlanNode::QuadPattern { .. }
            | PlanNode::PathPattern { .. } => true,
            PlanNode::ForLoopJoin { left, right } | PlanNode::HashJoin { left, right } => {
                Self::is_fit_for_for_loop_join(left) && Self::is_fit_for_for_loop_join(right)
            }
            PlanNode::Filter { child, .. } | PlanNode::Extend { child, .. } => {
                Self::is_fit_for_for_loop_join(child)
            }
            PlanNode::Union { children } => {
                children.iter().all(|c| Self::is_fit_for_for_loop_join(c))
            }
            PlanNode::AntiJoin { .. }
            | PlanNode::HashLeftJoin { .. }
            | PlanNode::ForLoopLeftJoin { .. }
            | PlanNode::Service { .. }
            | PlanNode::Sort { .. }
            | PlanNode::HashDeduplicate { .. }
            | PlanNode::Reduced { .. }
            | PlanNode::Skip { .. }
            | PlanNode::Limit { .. }
            | PlanNode::Project { .. }
            | PlanNode::Aggregate { .. } => false,
        }
    }

    fn push_filter(&self, node: Rc<PlanNode>, filter: Box<PlanExpression>) -> PlanNode {
        if !self.with_optimizations {
            return PlanNode::Filter {
                child: node,
                expression: filter,
            };
        }
        if let PlanExpression::And(filters) = *filter {
            return filters
                .into_iter()
                .fold((*node.as_ref()).clone(), |acc, f| {
                    self.push_filter(Rc::new(acc), Box::new(f))
                });
        }
        let mut filter_variables = BTreeSet::new();
        filter.lookup_used_variables(&mut |v| {
            filter_variables.insert(v);
        });
        match node.as_ref() {
            PlanNode::HashJoin { left, right } => {
                if filter_variables.iter().all(|v| left.is_variable_bound(*v)) {
                    if filter_variables.iter().all(|v| right.is_variable_bound(*v)) {
                        PlanNode::HashJoin {
                            left: Rc::new(self.push_filter(Rc::clone(left), filter.clone())),
                            right: Rc::new(self.push_filter(Rc::clone(right), filter)),
                        }
                    } else {
                        PlanNode::HashJoin {
                            left: Rc::new(self.push_filter(Rc::clone(left), filter)),
                            right: Rc::clone(right),
                        }
                    }
                } else if filter_variables.iter().all(|v| right.is_variable_bound(*v)) {
                    PlanNode::HashJoin {
                        left: Rc::clone(left),
                        right: Rc::new(self.push_filter(Rc::clone(right), filter)),
                    }
                } else {
                    PlanNode::Filter {
                        child: Rc::new(PlanNode::HashJoin {
                            left: Rc::clone(left),
                            right: Rc::clone(right),
                        }),
                        expression: filter,
                    }
                }
            }
            PlanNode::ForLoopJoin { left, right } => {
                if filter_variables.iter().all(|v| left.is_variable_bound(*v)) {
                    PlanNode::ForLoopJoin {
                        left: Rc::new(self.push_filter(Rc::clone(left), filter)),
                        right: Rc::clone(right),
                    }
                } else if filter_variables.iter().all(|v| right.is_variable_bound(*v)) {
                    PlanNode::ForLoopJoin {
                        //TODO: should we do that always?
                        left: Rc::clone(left),
                        right: Rc::new(self.push_filter(Rc::clone(right), filter)),
                    }
                } else {
                    PlanNode::Filter {
                        child: Rc::new(PlanNode::HashJoin {
                            left: Rc::clone(left),
                            right: Rc::clone(right),
                        }),
                        expression: filter,
                    }
                }
            }
            PlanNode::Extend {
                child,
                expression,
                variable,
            } => {
                //TODO: handle the case where the filter generates an expression variable
                if filter_variables.iter().all(|v| child.is_variable_bound(*v)) {
                    PlanNode::Extend {
                        child: Rc::new(self.push_filter(Rc::clone(child), filter)),
                        expression: expression.clone(),
                        variable: variable.clone(),
                    }
                } else {
                    PlanNode::Filter {
                        child: Rc::new(PlanNode::Extend {
                            child: Rc::clone(child),
                            expression: expression.clone(),
                            variable: variable.clone(),
                        }),
                        expression: filter,
                    }
                }
            }
            PlanNode::Filter { child, expression } => {
                if filter_variables.iter().all(|v| child.is_variable_bound(*v)) {
                    PlanNode::Filter {
                        child: Rc::new(self.push_filter(Rc::clone(child), filter)),
                        expression: expression.clone(),
                    }
                } else {
                    PlanNode::Filter {
                        child: Rc::clone(child),
                        expression: Box::new(PlanExpression::And(vec![
                            *expression.clone(),
                            *filter,
                        ])),
                    }
                }
            }
            PlanNode::Union { children } => PlanNode::Union {
                children: children
                    .iter()
                    .map(|c| Rc::new(self.push_filter(Rc::clone(c), filter.clone())))
                    .collect(),
            },
            _ => PlanNode::Filter {
                //TODO: more?
                child: node,
                expression: filter,
            },
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

fn sort_bgp(p: &[TriplePattern]) -> Vec<&TriplePattern> {
    let mut assigned_variables = HashSet::default();
    let mut assigned_blank_nodes = HashSet::default();
    let mut new_p: Vec<_> = p.iter().collect();

    for i in 0..new_p.len() {
        new_p[i..].sort_by(|p1, p2| {
            estimate_pattern_cost(p1, &assigned_variables, &assigned_blank_nodes).cmp(
                &estimate_pattern_cost(p2, &assigned_variables, &assigned_blank_nodes),
            )
        });
        add_pattern_variables(new_p[i], &mut assigned_variables, &mut assigned_blank_nodes);
    }

    new_p
}

fn estimate_pattern_cost(
    pattern: &TriplePattern,
    assigned_variables: &HashSet<&Variable>,
    assigned_blank_nodes: &HashSet<&BlankNode>,
) -> u32 {
    let mut count = 0;
    match &pattern.subject {
        TermPattern::NamedNode(_) | TermPattern::Literal(_) => count += 1,
        TermPattern::BlankNode(bnode) => {
            if !assigned_blank_nodes.contains(bnode) {
                count += 4;
            }
        }
        TermPattern::Variable(v) => {
            if !assigned_variables.contains(v) {
                count += 4;
            }
        }
        TermPattern::Triple(t) => {
            count += estimate_pattern_cost(t, assigned_variables, assigned_blank_nodes)
        }
    }
    if let NamedNodePattern::Variable(v) = &pattern.predicate {
        if !assigned_variables.contains(v) {
            count += 4;
        }
    } else {
        count += 1;
    }
    match &pattern.object {
        TermPattern::NamedNode(_) | TermPattern::Literal(_) => count += 1,
        TermPattern::BlankNode(bnode) => {
            if !assigned_blank_nodes.contains(bnode) {
                count += 4;
            }
        }
        TermPattern::Variable(v) => {
            if !assigned_variables.contains(v) {
                count += 4;
            }
        }
        TermPattern::Triple(t) => {
            count += estimate_pattern_cost(t, assigned_variables, assigned_blank_nodes)
        }
    }
    count
}

fn add_pattern_variables<'a>(
    pattern: &'a TriplePattern,
    variables: &mut HashSet<&'a Variable>,
    blank_nodes: &mut HashSet<&'a BlankNode>,
) {
    match &pattern.subject {
        TermPattern::NamedNode(_) | TermPattern::Literal(_) => (),
        TermPattern::BlankNode(bnode) => {
            blank_nodes.insert(bnode);
        }
        TermPattern::Variable(v) => {
            variables.insert(v);
        }
        TermPattern::Triple(t) => add_pattern_variables(t, variables, blank_nodes),
    }
    if let NamedNodePattern::Variable(v) = &pattern.predicate {
        variables.insert(v);
    }
    match &pattern.object {
        TermPattern::NamedNode(_) | TermPattern::Literal(_) => (),
        TermPattern::BlankNode(bnode) => {
            blank_nodes.insert(bnode);
        }
        TermPattern::Variable(v) => {
            variables.insert(v);
        }
        TermPattern::Triple(t) => add_pattern_variables(t, variables, blank_nodes),
    }
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
