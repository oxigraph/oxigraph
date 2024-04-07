use crate::model::vocab::{rdf, xsd};
use crate::model::{BlankNode, LiteralRef, NamedNodeRef, Term, Triple};
use crate::sparql::algebra::{Query, QueryDataset};
use crate::sparql::dataset::DatasetView;
use crate::sparql::error::EvaluationError;
use crate::sparql::model::*;
use crate::sparql::service::ServiceHandler;
use crate::sparql::CustomFunctionRegistry;
use crate::storage::numeric_encoder::*;
use crate::storage::small_string::SmallString;
use digest::Digest;
use json_event_parser::{JsonEvent, ToWriteJsonWriter};
use md5::Md5;
use oxilangtag::LanguageTag;
use oxiri::Iri;
use oxrdf::{TermRef, Variable};
use oxsdatatypes::*;
use rand::random;
use regex::{Regex, RegexBuilder};
use sha1::Sha1;
use sha2::{Sha256, Sha384, Sha512};
use spargebra::algebra::{AggregateFunction, Function, PropertyPathExpression};
use spargebra::term::{
    GroundSubject, GroundTerm, GroundTermPattern, GroundTriple, NamedNodePattern, TermPattern,
    TriplePattern,
};
use sparopt::algebra::{
    AggregateExpression, Expression, GraphPattern, JoinAlgorithm, LeftJoinAlgorithm,
    MinusAlgorithm, OrderExpression,
};
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::iter::{empty, once};
use std::rc::Rc;
use std::sync::Arc;
use std::{fmt, io, str};

const REGEX_SIZE_LIMIT: usize = 1_000_000;

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct EncodedTuple {
    inner: Vec<Option<EncodedTerm>>,
}

impl EncodedTuple {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Vec::with_capacity(capacity),
        }
    }

    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    pub fn contains(&self, index: usize) -> bool {
        self.inner.get(index).map_or(false, Option::is_some)
    }

    pub fn get(&self, index: usize) -> Option<&EncodedTerm> {
        self.inner.get(index).unwrap_or(&None).as_ref()
    }

    pub fn iter(&self) -> impl Iterator<Item = Option<EncodedTerm>> + '_ {
        self.inner.iter().cloned()
    }

    pub fn set(&mut self, index: usize, value: EncodedTerm) {
        if self.inner.len() <= index {
            self.inner.resize(index + 1, None);
        }
        self.inner[index] = Some(value);
    }

    pub fn combine_with(&self, other: &Self) -> Option<Self> {
        if self.inner.len() < other.inner.len() {
            let mut result = other.inner.clone();
            for (key, self_value) in self.inner.iter().enumerate() {
                if let Some(self_value) = self_value {
                    match &other.inner[key] {
                        Some(other_value) => {
                            if self_value != other_value {
                                return None;
                            }
                        }
                        None => result[key] = Some(self_value.clone()),
                    }
                }
            }
            Some(Self { inner: result })
        } else {
            let mut result = self.inner.clone();
            for (key, other_value) in other.inner.iter().enumerate() {
                if let Some(other_value) = other_value {
                    match &self.inner[key] {
                        Some(self_value) => {
                            if self_value != other_value {
                                return None;
                            }
                        }
                        None => result[key] = Some(other_value.clone()),
                    }
                }
            }
            Some(Self { inner: result })
        }
    }
}

impl IntoIterator for EncodedTuple {
    type Item = Option<EncodedTerm>;
    type IntoIter = std::vec::IntoIter<Option<EncodedTerm>>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

type EncodedTuplesIterator = Box<dyn Iterator<Item = Result<EncodedTuple, EvaluationError>>>;

#[derive(Clone)]
pub struct SimpleEvaluator {
    dataset: Rc<DatasetView>,
    base_iri: Option<Rc<Iri<String>>>,
    now: DateTime,
    service_handler: Arc<dyn ServiceHandler<Error = EvaluationError>>,
    custom_functions: Arc<CustomFunctionRegistry>,
    run_stats: bool,
}

impl SimpleEvaluator {
    pub fn new(
        dataset: Rc<DatasetView>,
        base_iri: Option<Rc<Iri<String>>>,
        service_handler: Arc<dyn ServiceHandler<Error = EvaluationError>>,
        custom_functions: Arc<CustomFunctionRegistry>,
        run_stats: bool,
    ) -> Self {
        Self {
            dataset,
            base_iri,
            now: DateTime::now(),
            service_handler,
            custom_functions,
            run_stats,
        }
    }

    pub fn evaluate_select(&self, pattern: &GraphPattern) -> (QueryResults, Rc<EvalNodeWithStats>) {
        let mut variables = Vec::new();
        let (eval, stats) = self.graph_pattern_evaluator(pattern, &mut variables);
        let from = EncodedTuple::with_capacity(variables.len());
        (
            QueryResults::Solutions(decode_bindings(
                Rc::clone(&self.dataset),
                eval(from),
                Arc::from(variables),
            )),
            stats,
        )
    }

    pub fn evaluate_ask(
        &self,
        pattern: &GraphPattern,
    ) -> (Result<QueryResults, EvaluationError>, Rc<EvalNodeWithStats>) {
        let mut variables = Vec::new();
        let (eval, stats) = self.graph_pattern_evaluator(pattern, &mut variables);
        let from = EncodedTuple::with_capacity(variables.len());
        (
            match eval(from).next() {
                Some(Ok(_)) => Ok(QueryResults::Boolean(true)),
                Some(Err(error)) => Err(error),
                None => Ok(QueryResults::Boolean(false)),
            },
            stats,
        )
    }

    pub fn evaluate_construct(
        &self,
        pattern: &GraphPattern,
        template: &[TriplePattern],
    ) -> (QueryResults, Rc<EvalNodeWithStats>) {
        let mut variables = Vec::new();
        let (eval, stats) = self.graph_pattern_evaluator(pattern, &mut variables);
        let mut bnodes = Vec::new();
        let template = template
            .iter()
            .map(|t| TripleTemplate {
                subject: self.template_value_from_term_or_variable(
                    &t.subject,
                    &mut variables,
                    &mut bnodes,
                ),
                predicate: self
                    .template_value_from_named_node_or_variable(&t.predicate, &mut variables),
                object: self.template_value_from_term_or_variable(
                    &t.object,
                    &mut variables,
                    &mut bnodes,
                ),
            })
            .collect();
        let from = EncodedTuple::with_capacity(variables.len());
        (
            QueryResults::Graph(QueryTripleIter {
                iter: Box::new(ConstructIterator {
                    eval: self.clone(),
                    iter: eval(from),
                    template,
                    buffered_results: Vec::default(),
                    bnodes: Vec::default(),
                }),
            }),
            stats,
        )
    }

    pub fn evaluate_describe(
        &self,
        pattern: &GraphPattern,
    ) -> (QueryResults, Rc<EvalNodeWithStats>) {
        let mut variables = Vec::new();
        let (eval, stats) = self.graph_pattern_evaluator(pattern, &mut variables);
        let from = EncodedTuple::with_capacity(variables.len());
        (
            QueryResults::Graph(QueryTripleIter {
                iter: Box::new(DescribeIterator {
                    eval: self.clone(),
                    iter: eval(from),
                    quads: Box::new(empty()),
                }),
            }),
            stats,
        )
    }

    pub fn graph_pattern_evaluator(
        &self,
        pattern: &GraphPattern,
        encoded_variables: &mut Vec<Variable>,
    ) -> (
        Rc<dyn Fn(EncodedTuple) -> EncodedTuplesIterator>,
        Rc<EvalNodeWithStats>,
    ) {
        let mut stat_children = Vec::new();
        let mut evaluator =
            self.build_graph_pattern_evaluator(pattern, encoded_variables, &mut stat_children);
        let stats = Rc::new(EvalNodeWithStats {
            label: eval_node_label(pattern),
            children: stat_children,
            exec_count: Cell::new(0),
            exec_duration: Cell::new(self.run_stats.then(DayTimeDuration::default)),
        });
        if self.run_stats {
            let stats = Rc::clone(&stats);
            evaluator = Rc::new(move |tuple| {
                let start = Timer::now();
                let inner = evaluator(tuple);
                stats.exec_duration.set(
                    stats
                        .exec_duration
                        .get()
                        .and_then(|stat| stat.checked_add(start.elapsed()?)),
                );
                Box::new(StatsIterator {
                    inner,
                    stats: Rc::clone(&stats),
                })
            })
        }
        (evaluator, stats)
    }

    fn build_graph_pattern_evaluator(
        &self,
        pattern: &GraphPattern,
        encoded_variables: &mut Vec<Variable>,
        stat_children: &mut Vec<Rc<EvalNodeWithStats>>,
    ) -> Rc<dyn Fn(EncodedTuple) -> EncodedTuplesIterator> {
        match pattern {
            GraphPattern::Values {
                variables,
                bindings,
            } => {
                let encoding = variables
                    .iter()
                    .map(|v| encode_variable(encoded_variables, v))
                    .collect::<Vec<_>>();
                let encoded_tuples = bindings
                    .iter()
                    .map(|row| {
                        let mut result = EncodedTuple::with_capacity(variables.len());
                        for (key, value) in row.iter().enumerate() {
                            if let Some(term) = value {
                                result.set(
                                    encoding[key],
                                    match term {
                                        GroundTerm::NamedNode(node) => self.encode_term(node),
                                        GroundTerm::Literal(literal) => self.encode_term(literal),
                                        GroundTerm::Triple(triple) => self.encode_triple(triple),
                                    },
                                );
                            }
                        }
                        result
                    })
                    .collect::<Vec<_>>();
                Rc::new(move |from| {
                    Box::new(
                        encoded_tuples
                            .iter()
                            .filter_map(move |t| Some(Ok(t.combine_with(&from)?)))
                            .collect::<Vec<_>>()
                            .into_iter(),
                    )
                })
            }
            GraphPattern::Service {
                name,
                inner,
                silent,
            } => {
                #[allow(clippy::shadow_same)]
                let silent = *silent;
                let service_name =
                    TupleSelector::from_named_node_pattern(name, encoded_variables, &self.dataset);
                self.build_graph_pattern_evaluator(inner, encoded_variables, &mut Vec::new()); // We call recursively to fill "encoded_variables"
                let graph_pattern = spargebra::algebra::GraphPattern::from(inner.as_ref());
                let variables = Rc::from(encoded_variables.as_slice());
                let eval = self.clone();
                Rc::new(move |from| {
                    match eval.evaluate_service(
                        &service_name,
                        &graph_pattern,
                        Rc::clone(&variables),
                        &from,
                    ) {
                        Ok(result) => Box::new(result.filter_map(move |binding| {
                            binding
                                .map(|binding| binding.combine_with(&from))
                                .transpose()
                        })),
                        Err(e) => {
                            if silent {
                                Box::new(once(Ok(from)))
                            } else {
                                Box::new(once(Err(e)))
                            }
                        }
                    }
                })
            }
            GraphPattern::QuadPattern {
                subject,
                predicate,
                object,
                graph_name,
            } => {
                let subject = TupleSelector::from_ground_term_pattern(
                    subject,
                    encoded_variables,
                    &self.dataset,
                );
                let predicate = TupleSelector::from_named_node_pattern(
                    predicate,
                    encoded_variables,
                    &self.dataset,
                );
                let object = TupleSelector::from_ground_term_pattern(
                    object,
                    encoded_variables,
                    &self.dataset,
                );
                let graph_name = TupleSelector::from_graph_name_pattern(
                    graph_name,
                    encoded_variables,
                    &self.dataset,
                );
                let dataset = Rc::clone(&self.dataset);
                Rc::new(move |from| {
                    let iter = dataset.encoded_quads_for_pattern(
                        subject.get_pattern_value(&from).as_ref(),
                        predicate.get_pattern_value(&from).as_ref(),
                        object.get_pattern_value(&from).as_ref(),
                        graph_name.get_pattern_value(&from).as_ref(),
                    );
                    let subject = subject.clone();
                    let predicate = predicate.clone();
                    let object = object.clone();
                    let graph_name = graph_name.clone();
                    Box::new(iter.filter_map(move |quad| match quad {
                        Ok(quad) => {
                            let mut new_tuple = from.clone();
                            put_pattern_value(&subject, quad.subject, &mut new_tuple)?;
                            put_pattern_value(&predicate, quad.predicate, &mut new_tuple)?;
                            put_pattern_value(&object, quad.object, &mut new_tuple)?;
                            put_pattern_value(&graph_name, quad.graph_name, &mut new_tuple)?;
                            Some(Ok(new_tuple))
                        }
                        Err(error) => Some(Err(error)),
                    }))
                })
            }
            GraphPattern::Path {
                subject,
                path,
                object,
                graph_name,
            } => {
                let subject = TupleSelector::from_ground_term_pattern(
                    subject,
                    encoded_variables,
                    &self.dataset,
                );
                let path = self.encode_property_path(path);

                let object = TupleSelector::from_ground_term_pattern(
                    object,
                    encoded_variables,
                    &self.dataset,
                );
                let graph_name = TupleSelector::from_graph_name_pattern(
                    graph_name,
                    encoded_variables,
                    &self.dataset,
                );
                let dataset = Rc::clone(&self.dataset);
                Rc::new(move |from| {
                    let input_subject = subject.get_pattern_value(&from);
                    let input_object = object.get_pattern_value(&from);
                    let input_graph_name = graph_name.get_pattern_value(&from);
                    let path_eval = PathEvaluator {
                        dataset: Rc::clone(&dataset),
                    };
                    match (input_subject, input_object, input_graph_name) {
                        (Some(input_subject), Some(input_object), Some(input_graph_name)) => {
                            match path_eval.eval_closed_in_graph(
                                &path,
                                &input_subject,
                                &input_object,
                                &input_graph_name,
                            ) {
                                Ok(true) => Box::new(once(Ok(from))),
                                Ok(false) => Box::new(empty()),
                                Err(e) => Box::new(once(Err(e))),
                            }
                        }
                        (Some(input_subject), None, Some(input_graph_name)) => {
                            let object = object.clone();
                            Box::new(
                                path_eval
                                    .eval_from_in_graph(&path, &input_subject, &input_graph_name)
                                    .filter_map(move |o| match o {
                                        Ok(o) => {
                                            let mut new_tuple = from.clone();
                                            put_pattern_value(&object, o, &mut new_tuple)?;
                                            Some(Ok(new_tuple))
                                        }
                                        Err(error) => Some(Err(error)),
                                    }),
                            )
                        }
                        (None, Some(input_object), Some(input_graph_name)) => {
                            let subject = subject.clone();
                            Box::new(
                                path_eval
                                    .eval_to_in_graph(&path, &input_object, &input_graph_name)
                                    .filter_map(move |s| match s {
                                        Ok(s) => {
                                            let mut new_tuple = from.clone();
                                            put_pattern_value(&subject, s, &mut new_tuple)?;
                                            Some(Ok(new_tuple))
                                        }
                                        Err(error) => Some(Err(error)),
                                    }),
                            )
                        }
                        (None, None, Some(input_graph_name)) => {
                            let subject = subject.clone();
                            let object = object.clone();
                            Box::new(
                                path_eval
                                    .eval_open_in_graph(&path, &input_graph_name)
                                    .filter_map(move |so| match so {
                                        Ok((s, o)) => {
                                            let mut new_tuple = from.clone();
                                            put_pattern_value(&subject, s, &mut new_tuple)?;
                                            put_pattern_value(&object, o, &mut new_tuple)?;
                                            Some(Ok(new_tuple))
                                        }
                                        Err(error) => Some(Err(error)),
                                    }),
                            )
                        }
                        (Some(input_subject), Some(input_object), None) => {
                            let graph_name = graph_name.clone();
                            Box::new(
                                path_eval
                                    .eval_closed_in_unknown_graph(
                                        &path,
                                        &input_subject,
                                        &input_object,
                                    )
                                    .filter_map(move |r| match r {
                                        Ok(g) => {
                                            let mut new_tuple = from.clone();
                                            put_pattern_value(&graph_name, g, &mut new_tuple)?;
                                            Some(Ok(new_tuple))
                                        }
                                        Err(error) => Some(Err(error)),
                                    }),
                            )
                        }
                        (Some(input_subject), None, None) => {
                            let object = object.clone();
                            let graph_name = graph_name.clone();
                            Box::new(
                                path_eval
                                    .eval_from_in_unknown_graph(&path, &input_subject)
                                    .filter_map(move |r| match r {
                                        Ok((o, g)) => {
                                            let mut new_tuple = from.clone();
                                            put_pattern_value(&object, o, &mut new_tuple)?;
                                            put_pattern_value(&graph_name, g, &mut new_tuple)?;
                                            Some(Ok(new_tuple))
                                        }
                                        Err(error) => Some(Err(error)),
                                    }),
                            )
                        }
                        (None, Some(input_object), None) => {
                            let subject = subject.clone();
                            let graph_name = graph_name.clone();
                            Box::new(
                                path_eval
                                    .eval_to_in_unknown_graph(&path, &input_object)
                                    .filter_map(move |r| match r {
                                        Ok((s, g)) => {
                                            let mut new_tuple = from.clone();
                                            put_pattern_value(&subject, s, &mut new_tuple)?;
                                            put_pattern_value(&graph_name, g, &mut new_tuple)?;

                                            Some(Ok(new_tuple))
                                        }
                                        Err(error) => Some(Err(error)),
                                    }),
                            )
                        }
                        (None, None, None) => {
                            let subject = subject.clone();
                            let object = object.clone();
                            let graph_name = graph_name.clone();
                            Box::new(path_eval.eval_open_in_unknown_graph(&path).filter_map(
                                move |r| match r {
                                    Ok((s, o, g)) => {
                                        let mut new_tuple = from.clone();
                                        put_pattern_value(&subject, s, &mut new_tuple)?;
                                        put_pattern_value(&object, o, &mut new_tuple)?;
                                        put_pattern_value(&graph_name, g, &mut new_tuple)?;
                                        Some(Ok(new_tuple))
                                    }
                                    Err(error) => Some(Err(error)),
                                },
                            ))
                        }
                    }
                })
            }
            GraphPattern::Join {
                left,
                right,
                algorithm,
            } => {
                let (left, left_stats) = self.graph_pattern_evaluator(left, encoded_variables);
                stat_children.push(left_stats);
                let (right, right_stats) = self.graph_pattern_evaluator(right, encoded_variables);
                stat_children.push(right_stats);

                match algorithm {
                    JoinAlgorithm::HashBuildLeftProbeRight { keys } => {
                        let build = left;
                        let probe = right;
                        if keys.is_empty() {
                            // Cartesian product
                            Rc::new(move |from| {
                                let mut errors = Vec::default();
                                let build_values = build(from.clone())
                                    .filter_map(|result| match result {
                                        Ok(result) => Some(result),
                                        Err(error) => {
                                            errors.push(Err(error));
                                            None
                                        }
                                    })
                                    .collect::<Vec<_>>();
                                Box::new(CartesianProductJoinIterator {
                                    probe_iter: probe(from),
                                    built: build_values,
                                    buffered_results: errors,
                                })
                            })
                        } else {
                            // Real hash join
                            let keys = keys
                                .iter()
                                .map(|v| encode_variable(encoded_variables, v))
                                .collect::<Vec<_>>();
                            Rc::new(move |from| {
                                let mut errors = Vec::default();
                                let mut built_values = EncodedTupleSet::new(keys.clone());
                                built_values.extend(build(from.clone()).filter_map(|result| {
                                    match result {
                                        Ok(result) => Some(result),
                                        Err(error) => {
                                            errors.push(Err(error));
                                            None
                                        }
                                    }
                                }));
                                Box::new(HashJoinIterator {
                                    probe_iter: probe(from),
                                    built: built_values,
                                    buffered_results: errors,
                                })
                            })
                        }
                    }
                }
            }
            GraphPattern::Lateral { left, right } => {
                let (left, left_stats) = self.graph_pattern_evaluator(left, encoded_variables);
                stat_children.push(left_stats);

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
                        let (right, right_stats) =
                            self.graph_pattern_evaluator(&right, encoded_variables);
                        stat_children.push(right_stats);
                        return Rc::new(move |from| {
                            Box::new(ForLoopLeftJoinIterator {
                                right_evaluator: Rc::clone(&right),
                                left_iter: left(from),
                                current_right: Box::new(empty()),
                            })
                        });
                    }
                }
                let (right, right_stats) = self.graph_pattern_evaluator(right, encoded_variables);
                stat_children.push(right_stats);
                Rc::new(move |from| {
                    let right = Rc::clone(&right);
                    Box::new(left(from).flat_map(move |t| match t {
                        Ok(t) => right(t),
                        Err(e) => Box::new(once(Err(e))),
                    }))
                })
            }
            GraphPattern::Minus {
                left,
                right,
                algorithm,
            } => {
                let (left, left_stats) = self.graph_pattern_evaluator(left, encoded_variables);
                stat_children.push(left_stats);
                let (right, right_stats) = self.graph_pattern_evaluator(right, encoded_variables);
                stat_children.push(right_stats);

                match algorithm {
                    MinusAlgorithm::HashBuildRightProbeLeft { keys } => {
                        if keys.is_empty() {
                            Rc::new(move |from| {
                                let right: Vec<_> =
                                    right(from.clone()).filter_map(Result::ok).collect();
                                Box::new(left(from).filter(move |left_tuple| {
                                    if let Ok(left_tuple) = left_tuple {
                                        !right.iter().any(|right_tuple| {
                                            are_compatible_and_not_disjointed(
                                                left_tuple,
                                                right_tuple,
                                            )
                                        })
                                    } else {
                                        true
                                    }
                                }))
                            })
                        } else {
                            let keys = keys
                                .iter()
                                .map(|v| encode_variable(encoded_variables, v))
                                .collect::<Vec<_>>();
                            Rc::new(move |from| {
                                let mut right_values = EncodedTupleSet::new(keys.clone());
                                right_values.extend(right(from.clone()).filter_map(Result::ok));
                                Box::new(left(from).filter(move |left_tuple| {
                                    if let Ok(left_tuple) = left_tuple {
                                        !right_values.get(left_tuple).iter().any(|right_tuple| {
                                            are_compatible_and_not_disjointed(
                                                left_tuple,
                                                right_tuple,
                                            )
                                        })
                                    } else {
                                        true
                                    }
                                }))
                            })
                        }
                    }
                }
            }
            GraphPattern::LeftJoin {
                left,
                right,
                expression,
                algorithm,
            } => {
                let (left, left_stats) = self.graph_pattern_evaluator(left, encoded_variables);
                stat_children.push(left_stats);
                let (right, right_stats) = self.graph_pattern_evaluator(right, encoded_variables);
                stat_children.push(right_stats);
                let expression =
                    self.expression_evaluator(expression, encoded_variables, stat_children);

                match algorithm {
                    LeftJoinAlgorithm::HashBuildRightProbeLeft { keys } => {
                        // Real hash join
                        let keys = keys
                            .iter()
                            .map(|v| encode_variable(encoded_variables, v))
                            .collect::<Vec<_>>();
                        Rc::new(move |from| {
                            let mut errors = Vec::default();
                            let mut right_values = EncodedTupleSet::new(keys.clone());
                            right_values.extend(right(from.clone()).filter_map(
                                |result| match result {
                                    Ok(result) => Some(result),
                                    Err(error) => {
                                        errors.push(Err(error));
                                        None
                                    }
                                },
                            ));
                            Box::new(HashLeftJoinIterator {
                                left_iter: left(from),
                                right: right_values,
                                buffered_results: errors,
                                expression: Rc::clone(&expression),
                            })
                        })
                    }
                }
            }
            GraphPattern::Filter { inner, expression } => {
                let (child, child_stats) = self.graph_pattern_evaluator(inner, encoded_variables);
                stat_children.push(child_stats);
                let expression =
                    self.expression_evaluator(expression, encoded_variables, stat_children);

                Rc::new(move |from| {
                    let expression = Rc::clone(&expression);
                    Box::new(child(from).filter(move |tuple| {
                        match tuple {
                            Ok(tuple) => expression(tuple)
                                .and_then(|term| to_bool(&term))
                                .unwrap_or(false),
                            Err(_) => true,
                        }
                    }))
                })
            }
            GraphPattern::Union { inner } => {
                let children = inner
                    .iter()
                    .map(|child| {
                        let (child, child_stats) =
                            self.graph_pattern_evaluator(child, encoded_variables);
                        stat_children.push(child_stats);
                        child
                    })
                    .collect::<Vec<_>>();

                Rc::new(move |from| {
                    Box::new(UnionIterator {
                        plans: children.clone(),
                        input: from,
                        current_iterator: Box::new(empty()),
                        current_plan: 0,
                    })
                })
            }
            GraphPattern::Extend {
                inner,
                variable,
                expression,
            } => {
                let (child, child_stats) = self.graph_pattern_evaluator(inner, encoded_variables);
                stat_children.push(child_stats);

                let position = encode_variable(encoded_variables, variable);
                let expression =
                    self.expression_evaluator(expression, encoded_variables, stat_children);
                Rc::new(move |from| {
                    let expression = Rc::clone(&expression);
                    Box::new(child(from).map(move |tuple| {
                        let mut tuple = tuple?;
                        if let Some(value) = expression(&tuple) {
                            tuple.set(position, value);
                        }
                        Ok(tuple)
                    }))
                })
            }
            GraphPattern::OrderBy { inner, expression } => {
                let (child, child_stats) = self.graph_pattern_evaluator(inner, encoded_variables);
                stat_children.push(child_stats);
                let by = expression
                    .iter()
                    .map(|comp| match comp {
                        OrderExpression::Asc(expression) => ComparatorFunction::Asc(
                            self.expression_evaluator(expression, encoded_variables, stat_children),
                        ),
                        OrderExpression::Desc(expression) => ComparatorFunction::Desc(
                            self.expression_evaluator(expression, encoded_variables, stat_children),
                        ),
                    })
                    .collect::<Vec<_>>();
                let dataset = Rc::clone(&self.dataset);
                Rc::new(move |from| {
                    let mut errors = Vec::default();
                    let mut values = child(from)
                        .filter_map(|result| match result {
                            Ok(result) => Some(result),
                            Err(error) => {
                                errors.push(Err(error));
                                None
                            }
                        })
                        .collect::<Vec<_>>();
                    values.sort_unstable_by(|a, b| {
                        for comp in &by {
                            match comp {
                                ComparatorFunction::Asc(expression) => {
                                    match cmp_terms(
                                        &dataset,
                                        expression(a).as_ref(),
                                        expression(b).as_ref(),
                                    ) {
                                        Ordering::Greater => return Ordering::Greater,
                                        Ordering::Less => return Ordering::Less,
                                        Ordering::Equal => (),
                                    }
                                }
                                ComparatorFunction::Desc(expression) => {
                                    match cmp_terms(
                                        &dataset,
                                        expression(a).as_ref(),
                                        expression(b).as_ref(),
                                    ) {
                                        Ordering::Greater => return Ordering::Less,
                                        Ordering::Less => return Ordering::Greater,
                                        Ordering::Equal => (),
                                    }
                                }
                            }
                        }
                        Ordering::Equal
                    });
                    Box::new(errors.into_iter().chain(values.into_iter().map(Ok)))
                })
            }
            GraphPattern::Distinct { inner } => {
                let (child, child_stats) = self.graph_pattern_evaluator(inner, encoded_variables);
                stat_children.push(child_stats);
                Rc::new(move |from| Box::new(hash_deduplicate(child(from))))
            }
            GraphPattern::Reduced { inner } => {
                let (child, child_stats) = self.graph_pattern_evaluator(inner, encoded_variables);
                stat_children.push(child_stats);
                Rc::new(move |from| {
                    Box::new(ConsecutiveDeduplication {
                        inner: child(from),
                        current: None,
                    })
                })
            }
            GraphPattern::Slice {
                inner,
                start,
                length,
            } => {
                let (mut child, child_stats) =
                    self.graph_pattern_evaluator(inner, encoded_variables);
                stat_children.push(child_stats);
                #[allow(clippy::shadow_same)]
                let start = *start;
                if start > 0 {
                    child = Rc::new(move |from| Box::new(child(from).skip(start)));
                }
                if let Some(length) = *length {
                    child = Rc::new(move |from| Box::new(child(from).take(length)));
                }
                child
            }
            GraphPattern::Project { inner, variables } => {
                let mut inner_encoded_variables = variables.clone();
                let (child, child_stats) =
                    self.graph_pattern_evaluator(inner, &mut inner_encoded_variables);
                stat_children.push(child_stats);
                let mapping = variables
                    .iter()
                    .enumerate()
                    .map(|(new_variable, variable)| {
                        (new_variable, encode_variable(encoded_variables, variable))
                    })
                    .collect::<Rc<[(usize, usize)]>>();
                Rc::new(move |from| {
                    let mapping = Rc::clone(&mapping);
                    let mut input_tuple = EncodedTuple::with_capacity(mapping.len());
                    for (input_key, output_key) in &*mapping {
                        if let Some(value) = from.get(*output_key) {
                            input_tuple.set(*input_key, value.clone());
                        }
                    }
                    Box::new(child(input_tuple).filter_map(move |tuple| {
                        match tuple {
                            Ok(tuple) => {
                                let mut output_tuple = from.clone();
                                for (input_key, output_key) in &*mapping {
                                    if let Some(value) = tuple.get(*input_key) {
                                        if let Some(existing_value) = output_tuple.get(*output_key)
                                        {
                                            if existing_value != value {
                                                return None; // Conflict
                                            }
                                        } else {
                                            output_tuple.set(*output_key, value.clone());
                                        }
                                    }
                                }
                                Some(Ok(output_tuple))
                            }
                            Err(e) => Some(Err(e)),
                        }
                    }))
                })
            }
            GraphPattern::Group {
                inner,
                aggregates,
                variables,
            } => {
                let (child, child_stats) = self.graph_pattern_evaluator(inner, encoded_variables);
                stat_children.push(child_stats);
                let key_variables = variables
                    .iter()
                    .map(|k| encode_variable(encoded_variables, k))
                    .collect::<Rc<[_]>>();
                let aggregate_input_expressions = aggregates
                    .iter()
                    .map(|(_, expression)| match expression {
                        AggregateExpression::CountSolutions { .. } => None,
                        AggregateExpression::FunctionCall { expr, .. } => {
                            Some(self.expression_evaluator(expr, encoded_variables, stat_children))
                        }
                    })
                    .collect::<Vec<_>>();
                let accumulator_builders = aggregates
                    .iter()
                    .map(|(_, aggregate)| Self::accumulator_builder(&self.dataset, aggregate))
                    .collect::<Vec<_>>();
                let accumulator_variables = aggregates
                    .iter()
                    .map(|(variable, _)| encode_variable(encoded_variables, variable))
                    .collect::<Vec<_>>();
                Rc::new(move |from| {
                    let tuple_size = from.capacity();
                    let key_variables = Rc::clone(&key_variables);
                    let mut errors = Vec::default();
                    let mut accumulators_for_group =
                        HashMap::<Vec<Option<EncodedTerm>>, Vec<Box<dyn Accumulator>>>::default();
                    if key_variables.is_empty() {
                        // There is always a single group if there is no GROUP BY
                        accumulators_for_group.insert(
                            Vec::new(),
                            accumulator_builders.iter().map(|c| c()).collect::<Vec<_>>(),
                        );
                    }
                    child(from)
                        .filter_map(|result| match result {
                            Ok(result) => Some(result),
                            Err(error) => {
                                errors.push(error);
                                None
                            }
                        })
                        .for_each(|tuple| {
                            // TODO avoid copy for key?
                            let key = key_variables
                                .iter()
                                .map(|v| tuple.get(*v).cloned())
                                .collect();

                            let key_accumulators =
                                accumulators_for_group.entry(key).or_insert_with(|| {
                                    accumulator_builders.iter().map(|c| c()).collect::<Vec<_>>()
                                });
                            for (accumulator, input_expression) in key_accumulators
                                .iter_mut()
                                .zip(&aggregate_input_expressions)
                            {
                                accumulator.add(
                                    input_expression
                                        .as_ref()
                                        .and_then(|parameter| parameter(&tuple)),
                                );
                            }
                        });
                    let accumulator_variables = accumulator_variables.clone();
                    Box::new(
                        errors
                            .into_iter()
                            .map(Err)
                            .chain(accumulators_for_group.into_iter().map(
                                move |(key, accumulators)| {
                                    let mut result = EncodedTuple::with_capacity(tuple_size);
                                    for (variable, value) in key_variables.iter().zip(key) {
                                        if let Some(value) = value {
                                            result.set(*variable, value);
                                        }
                                    }
                                    for (accumulator, variable) in
                                        accumulators.into_iter().zip(&accumulator_variables)
                                    {
                                        if let Some(value) = accumulator.state() {
                                            result.set(*variable, value);
                                        }
                                    }
                                    Ok(result)
                                },
                            )),
                    )
                })
            }
        }
    }

    fn evaluate_service(
        &self,
        service_name: &TupleSelector,
        graph_pattern: &spargebra::algebra::GraphPattern,
        variables: Rc<[Variable]>,
        from: &EncodedTuple,
    ) -> Result<EncodedTuplesIterator, EvaluationError> {
        let service_name = service_name
            .get_pattern_value(from)
            .ok_or(EvaluationError::UnboundService)?;
        if let QueryResults::Solutions(iter) = self.service_handler.handle(
            self.dataset.decode_named_node(&service_name)?,
            Query {
                inner: spargebra::Query::Select {
                    dataset: None,
                    pattern: graph_pattern.clone(),
                    #[allow(clippy::useless_asref)]
                    base_iri: self.base_iri.as_ref().map(|iri| iri.as_ref().clone()),
                },
                dataset: QueryDataset::new(),
                parsing_duration: None,
            },
        )? {
            Ok(encode_bindings(Rc::clone(&self.dataset), variables, iter))
        } else {
            Err(EvaluationError::ServiceDoesNotReturnSolutions)
        }
    }

    #[allow(clippy::redundant_closure)] // False positive in 1.60
    fn accumulator_builder(
        dataset: &Rc<DatasetView>,
        expression: &AggregateExpression,
    ) -> Box<dyn Fn() -> Box<dyn Accumulator>> {
        let mut accumulator: Box<dyn Fn() -> Box<dyn Accumulator>> = match expression {
            AggregateExpression::CountSolutions { .. } => {
                Box::new(|| Box::<CountAccumulator>::default())
            }
            AggregateExpression::FunctionCall { name, .. } => match name {
                AggregateFunction::Count => Box::new(|| Box::<CountAccumulator>::default()),
                AggregateFunction::Sum => Box::new(|| Box::<SumAccumulator>::default()),
                AggregateFunction::Min => {
                    let dataset = Rc::clone(dataset);
                    Box::new(move || Box::new(MinAccumulator::new(Rc::clone(&dataset))))
                }
                AggregateFunction::Max => {
                    let dataset = Rc::clone(dataset);
                    Box::new(move || Box::new(MaxAccumulator::new(Rc::clone(&dataset))))
                }
                AggregateFunction::Avg => Box::new(|| Box::<AvgAccumulator>::default()),
                AggregateFunction::Sample => Box::new(|| Box::<SampleAccumulator>::default()),
                AggregateFunction::GroupConcat { separator } => {
                    let dataset = Rc::clone(dataset);
                    let separator = Rc::from(separator.as_deref().unwrap_or(" "));
                    Box::new(move || {
                        Box::new(GroupConcatAccumulator::new(
                            Rc::clone(&dataset),
                            Rc::clone(&separator),
                        ))
                    })
                }
                AggregateFunction::Custom(_) => Box::new(|| Box::new(FailingAccumulator)),
            },
        };
        if matches!(
            expression,
            AggregateExpression::CountSolutions { distinct: true }
                | AggregateExpression::FunctionCall { distinct: true, .. }
        ) {
            accumulator = Box::new(move || Box::new(Deduplicate::new(accumulator())));
        }
        accumulator
    }

    fn expression_evaluator(
        &self,
        expression: &Expression,
        encoded_variables: &mut Vec<Variable>,
        stat_children: &mut Vec<Rc<EvalNodeWithStats>>,
    ) -> Rc<dyn Fn(&EncodedTuple) -> Option<EncodedTerm>> {
        match expression {
            Expression::NamedNode(t) => {
                let t = self.encode_term(t);
                Rc::new(move |_| Some(t.clone()))
            }
            Expression::Literal(t) => {
                let t = self.encode_term(t);
                Rc::new(move |_| Some(t.clone()))
            }
            Expression::Variable(v) => {
                let v = encode_variable(encoded_variables, v);
                Rc::new(move |tuple| tuple.get(v).cloned())
            }
            Expression::Bound(v) => {
                let v = encode_variable(encoded_variables, v);
                Rc::new(move |tuple| Some(tuple.contains(v).into()))
            }
            Expression::Exists(plan) => {
                let (eval, stats) = self.graph_pattern_evaluator(plan, encoded_variables);
                stat_children.push(stats);
                Rc::new(move |tuple| Some(eval(tuple.clone()).next().is_some().into()))
            }
            Expression::Or(inner) => {
                let children = inner
                    .iter()
                    .map(|i| self.expression_evaluator(i, encoded_variables, stat_children))
                    .collect::<Rc<[_]>>();
                Rc::new(move |tuple| {
                    let mut error = false;
                    for child in &*children {
                        match child(tuple).and_then(|v| to_bool(&v)) {
                            Some(true) => return Some(true.into()),
                            Some(false) => continue,
                            None => error = true,
                        }
                    }
                    if error {
                        None
                    } else {
                        Some(false.into())
                    }
                })
            }
            Expression::And(inner) => {
                let children = inner
                    .iter()
                    .map(|i| self.expression_evaluator(i, encoded_variables, stat_children))
                    .collect::<Rc<[_]>>();
                Rc::new(move |tuple| {
                    let mut error = false;
                    for child in &*children {
                        match child(tuple).and_then(|v| to_bool(&v)) {
                            Some(true) => continue,
                            Some(false) => return Some(false.into()),
                            None => error = true,
                        }
                    }
                    if error {
                        None
                    } else {
                        Some(true.into())
                    }
                })
            }
            Expression::Equal(a, b) => {
                let a = self.expression_evaluator(a, encoded_variables, stat_children);
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                Rc::new(move |tuple| equals(&a(tuple)?, &b(tuple)?).map(Into::into))
            }
            Expression::SameTerm(a, b) => {
                let a = self.expression_evaluator(a, encoded_variables, stat_children);
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                Rc::new(move |tuple| Some((a(tuple)? == b(tuple)?).into()))
            }
            Expression::Greater(a, b) => {
                let a = self.expression_evaluator(a, encoded_variables, stat_children);
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                let dataset = Rc::clone(&self.dataset);
                Rc::new(move |tuple| {
                    Some(
                        (partial_cmp(&dataset, &a(tuple)?, &b(tuple)?)? == Ordering::Greater)
                            .into(),
                    )
                })
            }
            Expression::GreaterOrEqual(a, b) => {
                let a = self.expression_evaluator(a, encoded_variables, stat_children);
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                let dataset = Rc::clone(&self.dataset);
                Rc::new(move |tuple| {
                    Some(
                        match partial_cmp(&dataset, &a(tuple)?, &b(tuple)?)? {
                            Ordering::Greater | Ordering::Equal => true,
                            Ordering::Less => false,
                        }
                        .into(),
                    )
                })
            }
            Expression::Less(a, b) => {
                let a = self.expression_evaluator(a, encoded_variables, stat_children);
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                let dataset = Rc::clone(&self.dataset);
                Rc::new(move |tuple| {
                    Some((partial_cmp(&dataset, &a(tuple)?, &b(tuple)?)? == Ordering::Less).into())
                })
            }
            Expression::LessOrEqual(a, b) => {
                let a = self.expression_evaluator(a, encoded_variables, stat_children);
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                let dataset = Rc::clone(&self.dataset);
                Rc::new(move |tuple| {
                    Some(
                        match partial_cmp(&dataset, &a(tuple)?, &b(tuple)?)? {
                            Ordering::Less | Ordering::Equal => true,
                            Ordering::Greater => false,
                        }
                        .into(),
                    )
                })
            }
            Expression::Add(a, b) => {
                let a = self.expression_evaluator(a, encoded_variables, stat_children);
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                Rc::new(
                    move |tuple| match NumericBinaryOperands::new(a(tuple)?, b(tuple)?)? {
                        NumericBinaryOperands::Float(v1, v2) => Some((v1 + v2).into()),
                        NumericBinaryOperands::Double(v1, v2) => Some((v1 + v2).into()),
                        NumericBinaryOperands::Integer(v1, v2) => Some(v1.checked_add(v2)?.into()),
                        NumericBinaryOperands::Decimal(v1, v2) => Some(v1.checked_add(v2)?.into()),
                        NumericBinaryOperands::Duration(v1, v2) => Some(v1.checked_add(v2)?.into()),
                        NumericBinaryOperands::YearMonthDuration(v1, v2) => {
                            Some(v1.checked_add(v2)?.into())
                        }
                        NumericBinaryOperands::DayTimeDuration(v1, v2) => {
                            Some(v1.checked_add(v2)?.into())
                        }
                        NumericBinaryOperands::DateTimeDuration(v1, v2) => {
                            Some(v1.checked_add_duration(v2)?.into())
                        }
                        NumericBinaryOperands::DateTimeYearMonthDuration(v1, v2) => {
                            Some(v1.checked_add_year_month_duration(v2)?.into())
                        }
                        NumericBinaryOperands::DateTimeDayTimeDuration(v1, v2) => {
                            Some(v1.checked_add_day_time_duration(v2)?.into())
                        }
                        NumericBinaryOperands::DateDuration(v1, v2) => {
                            Some(v1.checked_add_duration(v2)?.into())
                        }
                        NumericBinaryOperands::DateYearMonthDuration(v1, v2) => {
                            Some(v1.checked_add_year_month_duration(v2)?.into())
                        }
                        NumericBinaryOperands::DateDayTimeDuration(v1, v2) => {
                            Some(v1.checked_add_day_time_duration(v2)?.into())
                        }
                        NumericBinaryOperands::TimeDuration(v1, v2) => {
                            Some(v1.checked_add_duration(v2)?.into())
                        }
                        NumericBinaryOperands::TimeDayTimeDuration(v1, v2) => {
                            Some(v1.checked_add_day_time_duration(v2)?.into())
                        }
                        NumericBinaryOperands::DateTime(_, _)
                        | NumericBinaryOperands::Time(_, _)
                        | NumericBinaryOperands::Date(_, _) => None,
                    },
                )
            }
            Expression::Subtract(a, b) => {
                let a = self.expression_evaluator(a, encoded_variables, stat_children);
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                Rc::new(move |tuple| {
                    Some(match NumericBinaryOperands::new(a(tuple)?, b(tuple)?)? {
                        NumericBinaryOperands::Float(v1, v2) => (v1 - v2).into(),
                        NumericBinaryOperands::Double(v1, v2) => (v1 - v2).into(),
                        NumericBinaryOperands::Integer(v1, v2) => v1.checked_sub(v2)?.into(),
                        NumericBinaryOperands::Decimal(v1, v2) => v1.checked_sub(v2)?.into(),
                        NumericBinaryOperands::DateTime(v1, v2) => v1.checked_sub(v2)?.into(),
                        NumericBinaryOperands::Date(v1, v2) => v1.checked_sub(v2)?.into(),
                        NumericBinaryOperands::Time(v1, v2) => v1.checked_sub(v2)?.into(),
                        NumericBinaryOperands::Duration(v1, v2) => v1.checked_sub(v2)?.into(),
                        NumericBinaryOperands::YearMonthDuration(v1, v2) => {
                            v1.checked_sub(v2)?.into()
                        }
                        NumericBinaryOperands::DayTimeDuration(v1, v2) => {
                            v1.checked_sub(v2)?.into()
                        }
                        NumericBinaryOperands::DateTimeDuration(v1, v2) => {
                            v1.checked_sub_duration(v2)?.into()
                        }
                        NumericBinaryOperands::DateTimeYearMonthDuration(v1, v2) => {
                            v1.checked_sub_year_month_duration(v2)?.into()
                        }
                        NumericBinaryOperands::DateTimeDayTimeDuration(v1, v2) => {
                            v1.checked_sub_day_time_duration(v2)?.into()
                        }
                        NumericBinaryOperands::DateDuration(v1, v2) => {
                            v1.checked_sub_duration(v2)?.into()
                        }
                        NumericBinaryOperands::DateYearMonthDuration(v1, v2) => {
                            v1.checked_sub_year_month_duration(v2)?.into()
                        }
                        NumericBinaryOperands::DateDayTimeDuration(v1, v2) => {
                            v1.checked_sub_day_time_duration(v2)?.into()
                        }
                        NumericBinaryOperands::TimeDuration(v1, v2) => {
                            v1.checked_sub_duration(v2)?.into()
                        }
                        NumericBinaryOperands::TimeDayTimeDuration(v1, v2) => {
                            v1.checked_sub_day_time_duration(v2)?.into()
                        }
                    })
                })
            }
            Expression::Multiply(a, b) => {
                let a = self.expression_evaluator(a, encoded_variables, stat_children);
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                Rc::new(
                    move |tuple| match NumericBinaryOperands::new(a(tuple)?, b(tuple)?)? {
                        NumericBinaryOperands::Float(v1, v2) => Some((v1 * v2).into()),
                        NumericBinaryOperands::Double(v1, v2) => Some((v1 * v2).into()),
                        NumericBinaryOperands::Integer(v1, v2) => Some(v1.checked_mul(v2)?.into()),
                        NumericBinaryOperands::Decimal(v1, v2) => Some(v1.checked_mul(v2)?.into()),
                        _ => None,
                    },
                )
            }
            Expression::Divide(a, b) => {
                let a = self.expression_evaluator(a, encoded_variables, stat_children);
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                Rc::new(
                    move |tuple| match NumericBinaryOperands::new(a(tuple)?, b(tuple)?)? {
                        NumericBinaryOperands::Float(v1, v2) => Some((v1 / v2).into()),
                        NumericBinaryOperands::Double(v1, v2) => Some((v1 / v2).into()),
                        NumericBinaryOperands::Integer(v1, v2) => {
                            Some(Decimal::from(v1).checked_div(v2)?.into())
                        }
                        NumericBinaryOperands::Decimal(v1, v2) => Some(v1.checked_div(v2)?.into()),
                        _ => None,
                    },
                )
            }
            Expression::UnaryPlus(e) => {
                let e = self.expression_evaluator(e, encoded_variables, stat_children);
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::FloatLiteral(value) => Some(value.into()),
                    EncodedTerm::DoubleLiteral(value) => Some(value.into()),
                    EncodedTerm::IntegerLiteral(value) => Some(value.into()),
                    EncodedTerm::DecimalLiteral(value) => Some(value.into()),
                    EncodedTerm::DurationLiteral(value) => Some(value.into()),
                    EncodedTerm::YearMonthDurationLiteral(value) => Some(value.into()),
                    EncodedTerm::DayTimeDurationLiteral(value) => Some(value.into()),
                    _ => None,
                })
            }
            Expression::UnaryMinus(e) => {
                let e = self.expression_evaluator(e, encoded_variables, stat_children);
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::FloatLiteral(value) => Some((-value).into()),
                    EncodedTerm::DoubleLiteral(value) => Some((-value).into()),
                    EncodedTerm::IntegerLiteral(value) => Some(value.checked_neg()?.into()),
                    EncodedTerm::DecimalLiteral(value) => Some(value.checked_neg()?.into()),
                    EncodedTerm::DurationLiteral(value) => Some(value.checked_neg()?.into()),
                    EncodedTerm::YearMonthDurationLiteral(value) => {
                        Some(value.checked_neg()?.into())
                    }
                    EncodedTerm::DayTimeDurationLiteral(value) => Some(value.checked_neg()?.into()),
                    _ => None,
                })
            }
            Expression::Not(e) => {
                let e = self.expression_evaluator(e, encoded_variables, stat_children);
                Rc::new(move |tuple| to_bool(&e(tuple)?).map(|v| (!v).into()))
            }
            Expression::Coalesce(l) => {
                let l: Vec<_> = l
                    .iter()
                    .map(|e| self.expression_evaluator(e, encoded_variables, stat_children))
                    .collect();
                Rc::new(move |tuple| {
                    for e in &l {
                        if let Some(result) = e(tuple) {
                            return Some(result);
                        }
                    }
                    None
                })
            }
            Expression::If(a, b, c) => {
                let a = self.expression_evaluator(a, encoded_variables, stat_children);
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                let c = self.expression_evaluator(c, encoded_variables, stat_children);
                Rc::new(move |tuple| {
                    if to_bool(&a(tuple)?)? {
                        b(tuple)
                    } else {
                        c(tuple)
                    }
                })
            }
            Expression::FunctionCall(function, parameters) => {
                match function {
                    Function::Str => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |tuple| {
                            Some(build_string_literal_from_id(to_string_id(
                                &dataset,
                                &e(tuple)?,
                            )?))
                        })
                    }
                    Function::Lang => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |tuple| match e(tuple)? {
                            EncodedTerm::SmallSmallLangStringLiteral { language, .. }
                            | EncodedTerm::BigSmallLangStringLiteral { language, .. } => {
                                Some(build_string_literal_from_id(language.into()))
                            }
                            EncodedTerm::SmallBigLangStringLiteral { language_id, .. }
                            | EncodedTerm::BigBigLangStringLiteral { language_id, .. } => {
                                Some(build_string_literal_from_id(language_id.into()))
                            }
                            e if e.is_literal() => Some(build_string_literal(&dataset, "")),
                            _ => None,
                        })
                    }
                    Function::LangMatches => {
                        let language_tag = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let language_range = self.expression_evaluator(
                            &parameters[1],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |tuple| {
                            let mut language_tag =
                                to_simple_string(&dataset, &language_tag(tuple)?)?;
                            language_tag.make_ascii_lowercase();
                            let mut language_range =
                                to_simple_string(&dataset, &language_range(tuple)?)?;
                            language_range.make_ascii_lowercase();
                            Some(
                                if &*language_range == "*" {
                                    !language_tag.is_empty()
                                } else {
                                    !ZipLongest::new(
                                        language_range.split('-'),
                                        language_tag.split('-'),
                                    )
                                    .any(|parts| match parts {
                                        (Some(range_subtag), Some(language_subtag)) => {
                                            range_subtag != language_subtag
                                        }
                                        (Some(_), None) => true,
                                        (None, _) => false,
                                    })
                                }
                                .into(),
                            )
                        })
                    }
                    Function::Datatype => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |tuple| datatype(&dataset, &e(tuple)?))
                    }
                    Function::Iri => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        let base_iri = self.base_iri.clone();
                        Rc::new(move |tuple| {
                            let e = e(tuple)?;
                            if e.is_named_node() {
                                Some(e)
                            } else {
                                let iri = to_simple_string(&dataset, &e)?;
                                Some(build_named_node(
                                    &dataset,
                                    &if let Some(base_iri) = &base_iri {
                                        base_iri.resolve(&iri)
                                    } else {
                                        Iri::parse(iri)
                                    }
                                    .ok()?
                                    .into_inner(),
                                ))
                            }
                        })
                    }
                    Function::BNode => match parameters.first() {
                        Some(id) => {
                            let id =
                                self.expression_evaluator(id, encoded_variables, stat_children);
                            let dataset = Rc::clone(&self.dataset);
                            Rc::new(move |tuple| {
                                Some(
                                    dataset.encode_term(
                                        BlankNode::new(to_simple_string(&dataset, &id(tuple)?)?)
                                            .ok()?
                                            .as_ref(),
                                    ),
                                )
                            })
                        }
                        None => Rc::new(|_| {
                            Some(EncodedTerm::NumericalBlankNode {
                                id: random::<u128>(),
                            })
                        }),
                    },
                    Function::Rand => Rc::new(|_| Some(random::<f64>().into())),
                    Function::Abs => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| match e(tuple)? {
                            EncodedTerm::IntegerLiteral(value) => Some(value.checked_abs()?.into()),
                            EncodedTerm::DecimalLiteral(value) => Some(value.checked_abs()?.into()),
                            EncodedTerm::FloatLiteral(value) => Some(value.abs().into()),
                            EncodedTerm::DoubleLiteral(value) => Some(value.abs().into()),
                            _ => None,
                        })
                    }
                    Function::Ceil => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| match e(tuple)? {
                            EncodedTerm::IntegerLiteral(value) => Some(value.into()),
                            EncodedTerm::DecimalLiteral(value) => {
                                Some(value.checked_ceil()?.into())
                            }
                            EncodedTerm::FloatLiteral(value) => Some(value.ceil().into()),
                            EncodedTerm::DoubleLiteral(value) => Some(value.ceil().into()),
                            _ => None,
                        })
                    }
                    Function::Floor => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| match e(tuple)? {
                            EncodedTerm::IntegerLiteral(value) => Some(value.into()),
                            EncodedTerm::DecimalLiteral(value) => {
                                Some(value.checked_floor()?.into())
                            }
                            EncodedTerm::FloatLiteral(value) => Some(value.floor().into()),
                            EncodedTerm::DoubleLiteral(value) => Some(value.floor().into()),
                            _ => None,
                        })
                    }
                    Function::Round => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| match e(tuple)? {
                            EncodedTerm::IntegerLiteral(value) => Some(value.into()),
                            EncodedTerm::DecimalLiteral(value) => {
                                Some(value.checked_round()?.into())
                            }
                            EncodedTerm::FloatLiteral(value) => Some(value.round().into()),
                            EncodedTerm::DoubleLiteral(value) => Some(value.round().into()),
                            _ => None,
                        })
                    }
                    Function::Concat => {
                        let l: Vec<_> = parameters
                            .iter()
                            .map(|e| self.expression_evaluator(e, encoded_variables, stat_children))
                            .collect();
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |tuple| {
                            let mut result = String::default();
                            let mut language = None;
                            for e in &l {
                                let (value, e_language) =
                                    to_string_and_language(&dataset, &e(tuple)?)?;
                                if let Some(lang) = language {
                                    if lang != e_language {
                                        language = Some(None)
                                    }
                                } else {
                                    language = Some(e_language)
                                }
                                result += &value
                            }
                            Some(build_plain_literal(
                                &dataset,
                                &result,
                                language.and_then(|v| v),
                            ))
                        })
                    }
                    Function::SubStr => {
                        let source = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let starting_loc = self.expression_evaluator(
                            &parameters[1],
                            encoded_variables,
                            stat_children,
                        );
                        let length = parameters.get(2).map(|l| {
                            self.expression_evaluator(l, encoded_variables, stat_children)
                        });
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |tuple| {
                            let (source, language) =
                                to_string_and_language(&dataset, &source(tuple)?)?;

                            let starting_location: usize =
                                if let EncodedTerm::IntegerLiteral(v) = starting_loc(tuple)? {
                                    i64::from(v).try_into().ok()?
                                } else {
                                    return None;
                                };
                            let length: Option<usize> = if let Some(length) = &length {
                                if let EncodedTerm::IntegerLiteral(v) = length(tuple)? {
                                    Some(i64::from(v).try_into().ok()?)
                                } else {
                                    return None;
                                }
                            } else {
                                None
                            };

                            // We want to slice on char indices, not byte indices
                            let mut start_iter = source
                                .char_indices()
                                .skip(starting_location.checked_sub(1)?)
                                .peekable();
                            let result =
                                if let Some((start_position, _)) = start_iter.peek().copied() {
                                    if let Some(length) = length {
                                        let mut end_iter = start_iter.skip(length).peekable();
                                        if let Some((end_position, _)) = end_iter.peek() {
                                            &source[start_position..*end_position]
                                        } else {
                                            &source[start_position..]
                                        }
                                    } else {
                                        &source[start_position..]
                                    }
                                } else {
                                    ""
                                };
                            Some(build_plain_literal(&dataset, result, language))
                        })
                    }
                    Function::StrLen => {
                        let arg = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |tuple| {
                            Some(
                                i64::try_from(to_string(&dataset, &arg(tuple)?)?.chars().count())
                                    .ok()?
                                    .into(),
                            )
                        })
                    }
                    Function::Replace => {
                        let arg = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let replacement = self.expression_evaluator(
                            &parameters[2],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        if let Some(regex) =
                            compile_static_pattern_if_exists(&parameters[1], parameters.get(3))
                        {
                            Rc::new(move |tuple| {
                                let (text, language) =
                                    to_string_and_language(&dataset, &arg(tuple)?)?;
                                let replacement = to_simple_string(&dataset, &replacement(tuple)?)?;
                                Some(build_plain_literal(
                                    &dataset,
                                    &regex.replace_all(&text, replacement.as_str()),
                                    language,
                                ))
                            })
                        } else {
                            let pattern = self.expression_evaluator(
                                &parameters[1],
                                encoded_variables,
                                stat_children,
                            );
                            let flags = parameters.get(3).map(|flags| {
                                self.expression_evaluator(flags, encoded_variables, stat_children)
                            });
                            Rc::new(move |tuple| {
                                let pattern = to_simple_string(&dataset, &pattern(tuple)?)?;
                                let options = if let Some(flags) = &flags {
                                    Some(to_simple_string(&dataset, &flags(tuple)?)?)
                                } else {
                                    None
                                };
                                let regex = compile_pattern(&pattern, options.as_deref())?;
                                let (text, language) =
                                    to_string_and_language(&dataset, &arg(tuple)?)?;
                                let replacement = to_simple_string(&dataset, &replacement(tuple)?)?;
                                Some(build_plain_literal(
                                    &dataset,
                                    &regex.replace_all(&text, replacement.as_str()),
                                    language,
                                ))
                            })
                        }
                    }
                    Function::UCase => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |tuple| {
                            let (value, language) = to_string_and_language(&dataset, &e(tuple)?)?;
                            Some(build_plain_literal(
                                &dataset,
                                &value.to_uppercase(),
                                language,
                            ))
                        })
                    }
                    Function::LCase => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |tuple| {
                            let (value, language) = to_string_and_language(&dataset, &e(tuple)?)?;
                            Some(build_plain_literal(
                                &dataset,
                                &value.to_lowercase(),
                                language,
                            ))
                        })
                    }
                    Function::StrStarts => {
                        let arg1 = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let arg2 = self.expression_evaluator(
                            &parameters[1],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |tuple| {
                            let (arg1, arg2, _) = to_argument_compatible_strings(
                                &dataset,
                                &arg1(tuple)?,
                                &arg2(tuple)?,
                            )?;
                            Some(arg1.starts_with(arg2.as_str()).into())
                        })
                    }
                    Function::EncodeForUri => {
                        let ltrl = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |tuple| {
                            let ltlr = to_string(&dataset, &ltrl(tuple)?)?;
                            let mut result = Vec::with_capacity(ltlr.len());
                            for c in ltlr.bytes() {
                                match c {
                                    b'A'..=b'Z'
                                    | b'a'..=b'z'
                                    | b'0'..=b'9'
                                    | b'-'
                                    | b'_'
                                    | b'.'
                                    | b'~' => result.push(c),
                                    _ => {
                                        result.push(b'%');
                                        let high = c / 16;
                                        let low = c % 16;
                                        result.push(if high < 10 {
                                            b'0' + high
                                        } else {
                                            b'A' + (high - 10)
                                        });
                                        result.push(if low < 10 {
                                            b'0' + low
                                        } else {
                                            b'A' + (low - 10)
                                        });
                                    }
                                }
                            }
                            Some(build_string_literal(
                                &dataset,
                                str::from_utf8(&result).ok()?,
                            ))
                        })
                    }
                    Function::StrEnds => {
                        let arg1 = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let arg2 = self.expression_evaluator(
                            &parameters[1],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |tuple| {
                            let (arg1, arg2, _) = to_argument_compatible_strings(
                                &dataset,
                                &arg1(tuple)?,
                                &arg2(tuple)?,
                            )?;
                            Some(arg1.ends_with(arg2.as_str()).into())
                        })
                    }
                    Function::Contains => {
                        let arg1 = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let arg2 = self.expression_evaluator(
                            &parameters[1],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |tuple| {
                            let (arg1, arg2, _) = to_argument_compatible_strings(
                                &dataset,
                                &arg1(tuple)?,
                                &arg2(tuple)?,
                            )?;
                            Some(arg1.contains(arg2.as_str()).into())
                        })
                    }
                    Function::StrBefore => {
                        let arg1 = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let arg2 = self.expression_evaluator(
                            &parameters[1],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |tuple| {
                            let (arg1, arg2, language) = to_argument_compatible_strings(
                                &dataset,
                                &arg1(tuple)?,
                                &arg2(tuple)?,
                            )?;
                            Some(if let Some(position) = arg1.find(arg2.as_str()) {
                                build_plain_literal(&dataset, &arg1[..position], language)
                            } else {
                                build_string_literal(&dataset, "")
                            })
                        })
                    }
                    Function::StrAfter => {
                        let arg1 = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let arg2 = self.expression_evaluator(
                            &parameters[1],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |tuple| {
                            let (arg1, arg2, language) = to_argument_compatible_strings(
                                &dataset,
                                &arg1(tuple)?,
                                &arg2(tuple)?,
                            )?;
                            Some(if let Some(position) = arg1.find(arg2.as_str()) {
                                build_plain_literal(
                                    &dataset,
                                    &arg1[position + arg2.len()..],
                                    language,
                                )
                            } else {
                                build_string_literal(&dataset, "")
                            })
                        })
                    }
                    Function::Year => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| match e(tuple)? {
                            EncodedTerm::DateTimeLiteral(date_time) => {
                                Some(date_time.year().into())
                            }
                            EncodedTerm::DateLiteral(date) => Some(date.year().into()),
                            EncodedTerm::GYearMonthLiteral(year_month) => {
                                Some(year_month.year().into())
                            }
                            EncodedTerm::GYearLiteral(year) => Some(year.year().into()),
                            _ => None,
                        })
                    }
                    Function::Month => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| match e(tuple)? {
                            EncodedTerm::DateTimeLiteral(date_time) => {
                                Some(date_time.month().into())
                            }
                            EncodedTerm::DateLiteral(date) => Some(date.month().into()),
                            EncodedTerm::GYearMonthLiteral(year_month) => {
                                Some(year_month.month().into())
                            }
                            EncodedTerm::GMonthDayLiteral(month_day) => {
                                Some(month_day.month().into())
                            }
                            EncodedTerm::GMonthLiteral(month) => Some(month.month().into()),
                            _ => None,
                        })
                    }
                    Function::Day => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| match e(tuple)? {
                            EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.day().into()),
                            EncodedTerm::DateLiteral(date) => Some(date.day().into()),
                            EncodedTerm::GMonthDayLiteral(month_day) => {
                                Some(month_day.day().into())
                            }
                            EncodedTerm::GDayLiteral(day) => Some(day.day().into()),
                            _ => None,
                        })
                    }
                    Function::Hours => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| match e(tuple)? {
                            EncodedTerm::DateTimeLiteral(date_time) => {
                                Some(date_time.hour().into())
                            }
                            EncodedTerm::TimeLiteral(time) => Some(time.hour().into()),
                            _ => None,
                        })
                    }
                    Function::Minutes => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| match e(tuple)? {
                            EncodedTerm::DateTimeLiteral(date_time) => {
                                Some(date_time.minute().into())
                            }
                            EncodedTerm::TimeLiteral(time) => Some(time.minute().into()),
                            _ => None,
                        })
                    }
                    Function::Seconds => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| match e(tuple)? {
                            EncodedTerm::DateTimeLiteral(date_time) => {
                                Some(date_time.second().into())
                            }
                            EncodedTerm::TimeLiteral(time) => Some(time.second().into()),
                            _ => None,
                        })
                    }
                    Function::Timezone => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| {
                            Some(
                                match e(tuple)? {
                                    EncodedTerm::DateTimeLiteral(date_time) => date_time.timezone(),
                                    EncodedTerm::TimeLiteral(time) => time.timezone(),
                                    EncodedTerm::DateLiteral(date) => date.timezone(),
                                    EncodedTerm::GYearMonthLiteral(year_month) => {
                                        year_month.timezone()
                                    }
                                    EncodedTerm::GYearLiteral(year) => year.timezone(),
                                    EncodedTerm::GMonthDayLiteral(month_day) => {
                                        month_day.timezone()
                                    }
                                    EncodedTerm::GDayLiteral(day) => day.timezone(),
                                    EncodedTerm::GMonthLiteral(month) => month.timezone(),
                                    _ => None,
                                }?
                                .into(),
                            )
                        })
                    }
                    Function::Tz => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |tuple| {
                            let timezone_offset = match e(tuple)? {
                                EncodedTerm::DateTimeLiteral(date_time) => {
                                    date_time.timezone_offset()
                                }
                                EncodedTerm::TimeLiteral(time) => time.timezone_offset(),
                                EncodedTerm::DateLiteral(date) => date.timezone_offset(),
                                EncodedTerm::GYearMonthLiteral(year_month) => {
                                    year_month.timezone_offset()
                                }
                                EncodedTerm::GYearLiteral(year) => year.timezone_offset(),
                                EncodedTerm::GMonthDayLiteral(month_day) => {
                                    month_day.timezone_offset()
                                }
                                EncodedTerm::GDayLiteral(day) => day.timezone_offset(),
                                EncodedTerm::GMonthLiteral(month) => month.timezone_offset(),
                                _ => return None,
                            };
                            Some(match timezone_offset {
                                Some(timezone_offset) => {
                                    build_string_literal(&dataset, &timezone_offset.to_string())
                                }
                                None => build_string_literal(&dataset, ""),
                            })
                        })
                    }
                    Function::Adjust => {
                        let dt = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let tz = self.expression_evaluator(
                            &parameters[1],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| {
                            let timezone_offset = Some(
                                match tz(tuple)? {
                                    EncodedTerm::DayTimeDurationLiteral(tz) => {
                                        TimezoneOffset::try_from(tz)
                                    }
                                    EncodedTerm::DurationLiteral(tz) => {
                                        TimezoneOffset::try_from(tz)
                                    }
                                    _ => return None,
                                }
                                .ok()?,
                            );
                            Some(match dt(tuple)? {
                                EncodedTerm::DateTimeLiteral(date_time) => {
                                    date_time.adjust(timezone_offset)?.into()
                                }
                                EncodedTerm::TimeLiteral(time) => {
                                    time.adjust(timezone_offset)?.into()
                                }
                                EncodedTerm::DateLiteral(date) => {
                                    date.adjust(timezone_offset)?.into()
                                }
                                EncodedTerm::GYearMonthLiteral(year_month) => {
                                    year_month.adjust(timezone_offset)?.into()
                                }
                                EncodedTerm::GYearLiteral(year) => {
                                    year.adjust(timezone_offset)?.into()
                                }
                                EncodedTerm::GMonthDayLiteral(month_day) => {
                                    month_day.adjust(timezone_offset)?.into()
                                }
                                EncodedTerm::GDayLiteral(day) => {
                                    day.adjust(timezone_offset)?.into()
                                }
                                EncodedTerm::GMonthLiteral(month) => {
                                    month.adjust(timezone_offset)?.into()
                                }
                                _ => return None,
                            })
                        })
                    }
                    Function::Now => {
                        let now = self.now;
                        Rc::new(move |_| Some(now.into()))
                    }
                    Function::Uuid => {
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |_| {
                            let mut buffer = String::with_capacity(44);
                            buffer.push_str("urn:uuid:");
                            generate_uuid(&mut buffer);
                            Some(build_named_node(&dataset, &buffer))
                        })
                    }
                    Function::StrUuid => {
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |_| {
                            let mut buffer = String::with_capacity(36);
                            generate_uuid(&mut buffer);
                            Some(build_string_literal(&dataset, &buffer))
                        })
                    }
                    Function::Md5 => self.hash::<Md5>(parameters, encoded_variables, stat_children),
                    Function::Sha1 => {
                        self.hash::<Sha1>(parameters, encoded_variables, stat_children)
                    }
                    Function::Sha256 => {
                        self.hash::<Sha256>(parameters, encoded_variables, stat_children)
                    }
                    Function::Sha384 => {
                        self.hash::<Sha384>(parameters, encoded_variables, stat_children)
                    }
                    Function::Sha512 => {
                        self.hash::<Sha512>(parameters, encoded_variables, stat_children)
                    }
                    Function::StrLang => {
                        let lexical_form = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let lang_tag = self.expression_evaluator(
                            &parameters[1],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |tuple| {
                            Some(build_lang_string_literal_from_id(
                                to_simple_string_id(&lexical_form(tuple)?)?,
                                build_language_id(&dataset, &lang_tag(tuple)?)?,
                            ))
                        })
                    }
                    Function::StrDt => {
                        let lexical_form = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let datatype = self.expression_evaluator(
                            &parameters[1],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        Rc::new(move |tuple| {
                            let value = to_simple_string(&dataset, &lexical_form(tuple)?)?;
                            let datatype =
                                if let EncodedTerm::NamedNode { iri_id } = datatype(tuple)? {
                                    dataset.get_str(&iri_id).ok()?
                                } else {
                                    None
                                }?;
                            Some(dataset.encode_term(LiteralRef::new_typed_literal(
                                &value,
                                NamedNodeRef::new_unchecked(&datatype),
                            )))
                        })
                    }
                    Function::IsIri => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| Some(e(tuple)?.is_named_node().into()))
                    }
                    Function::IsBlank => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| Some(e(tuple)?.is_blank_node().into()))
                    }
                    Function::IsLiteral => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| Some(e(tuple)?.is_literal().into()))
                    }
                    Function::IsNumeric => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| {
                            Some(
                                matches!(
                                    e(tuple)?,
                                    EncodedTerm::FloatLiteral(_)
                                        | EncodedTerm::DoubleLiteral(_)
                                        | EncodedTerm::IntegerLiteral(_)
                                        | EncodedTerm::DecimalLiteral(_)
                                )
                                .into(),
                            )
                        })
                    }
                    Function::Regex => {
                        let text = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let dataset = Rc::clone(&self.dataset);
                        if let Some(regex) =
                            compile_static_pattern_if_exists(&parameters[1], parameters.get(2))
                        {
                            Rc::new(move |tuple| {
                                let text = to_string(&dataset, &text(tuple)?)?;
                                Some(regex.is_match(&text).into())
                            })
                        } else {
                            let pattern = self.expression_evaluator(
                                &parameters[0],
                                encoded_variables,
                                stat_children,
                            );
                            let flags = parameters.get(2).map(|flags| {
                                self.expression_evaluator(flags, encoded_variables, stat_children)
                            });
                            Rc::new(move |tuple| {
                                let pattern = to_simple_string(&dataset, &pattern(tuple)?)?;
                                let options = if let Some(flags) = &flags {
                                    Some(to_simple_string(&dataset, &flags(tuple)?)?)
                                } else {
                                    None
                                };
                                let regex = compile_pattern(&pattern, options.as_deref())?;
                                let text = to_string(&dataset, &text(tuple)?)?;
                                Some(regex.is_match(&text).into())
                            })
                        }
                    }
                    Function::Triple => {
                        let s = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        let p = self.expression_evaluator(
                            &parameters[1],
                            encoded_variables,
                            stat_children,
                        );
                        let o = self.expression_evaluator(
                            &parameters[2],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| {
                            let s = s(tuple)?;
                            let p = p(tuple)?;
                            let o = o(tuple)?;
                            (!s.is_literal()
                                && !s.is_default_graph()
                                && p.is_named_node()
                                && !o.is_default_graph())
                            .then(|| EncodedTriple::new(s, p, o).into())
                        })
                    }
                    Function::Subject => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| {
                            if let EncodedTerm::Triple(t) = e(tuple)? {
                                Some(t.subject.clone())
                            } else {
                                None
                            }
                        })
                    }
                    Function::Predicate => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| {
                            if let EncodedTerm::Triple(t) = e(tuple)? {
                                Some(t.predicate.clone())
                            } else {
                                None
                            }
                        })
                    }
                    Function::Object => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| {
                            if let EncodedTerm::Triple(t) = e(tuple)? {
                                Some(t.object.clone())
                            } else {
                                None
                            }
                        })
                    }
                    Function::IsTriple => {
                        let e = self.expression_evaluator(
                            &parameters[0],
                            encoded_variables,
                            stat_children,
                        );
                        Rc::new(move |tuple| Some(e(tuple)?.is_triple().into()))
                    }
                    Function::Custom(function_name) => {
                        if let Some(function) = self.custom_functions.get(function_name).cloned() {
                            let args = parameters
                                .iter()
                                .map(|e| {
                                    self.expression_evaluator(e, encoded_variables, stat_children)
                                })
                                .collect::<Vec<_>>();
                            let dataset = Rc::clone(&self.dataset);
                            return Rc::new(move |tuple| {
                                let args = args
                                    .iter()
                                    .map(|f| dataset.decode_term(&f(tuple)?).ok())
                                    .collect::<Option<Vec<_>>>()?;
                                Some(dataset.encode_term(&function(&args)?))
                            });
                        }
                        match function_name.as_ref() {
                            xsd::STRING => {
                                let e = self.expression_evaluator(
                                    &parameters[0],
                                    encoded_variables,
                                    stat_children,
                                );
                                let dataset = Rc::clone(&self.dataset);
                                Rc::new(move |tuple| {
                                    Some(build_string_literal_from_id(to_string_id(
                                        &dataset,
                                        &e(tuple)?,
                                    )?))
                                })
                            }
                            xsd::BOOLEAN => {
                                let e = self.expression_evaluator(
                                    &parameters[0],
                                    encoded_variables,
                                    stat_children,
                                );
                                Rc::new(move |tuple| match e(tuple)? {
                                    EncodedTerm::BooleanLiteral(value) => Some(value.into()),
                                    EncodedTerm::FloatLiteral(value) => {
                                        Some(Boolean::from(value).into())
                                    }
                                    EncodedTerm::DoubleLiteral(value) => {
                                        Some(Boolean::from(value).into())
                                    }
                                    EncodedTerm::IntegerLiteral(value) => {
                                        Some(Boolean::from(value).into())
                                    }
                                    EncodedTerm::DecimalLiteral(value) => {
                                        Some(Boolean::from(value).into())
                                    }
                                    EncodedTerm::SmallStringLiteral(value) => {
                                        parse_boolean_str(&value)
                                    }
                                    _ => None,
                                })
                            }
                            xsd::DOUBLE => {
                                let e = self.expression_evaluator(
                                    &parameters[0],
                                    encoded_variables,
                                    stat_children,
                                );
                                let dataset = Rc::clone(&self.dataset);
                                Rc::new(move |tuple| match e(tuple)? {
                                    EncodedTerm::FloatLiteral(value) => {
                                        Some(Double::from(value).into())
                                    }
                                    EncodedTerm::DoubleLiteral(value) => Some(value.into()),
                                    EncodedTerm::IntegerLiteral(value) => {
                                        Some(Double::from(value).into())
                                    }
                                    EncodedTerm::DecimalLiteral(value) => {
                                        Some(Double::from(value).into())
                                    }
                                    EncodedTerm::BooleanLiteral(value) => {
                                        Some(Double::from(value).into())
                                    }
                                    EncodedTerm::SmallStringLiteral(value) => {
                                        parse_double_str(&value)
                                    }
                                    EncodedTerm::BigStringLiteral { value_id } => {
                                        parse_double_str(&dataset.get_str(&value_id).ok()??)
                                    }
                                    _ => None,
                                })
                            }
                            xsd::FLOAT => {
                                let e = self.expression_evaluator(
                                    &parameters[0],
                                    encoded_variables,
                                    stat_children,
                                );
                                let dataset = Rc::clone(&self.dataset);
                                Rc::new(move |tuple| match e(tuple)? {
                                    EncodedTerm::FloatLiteral(value) => Some(value.into()),
                                    EncodedTerm::DoubleLiteral(value) => {
                                        Some(Float::from(value).into())
                                    }
                                    EncodedTerm::IntegerLiteral(value) => {
                                        Some(Float::from(value).into())
                                    }
                                    EncodedTerm::DecimalLiteral(value) => {
                                        Some(Float::from(value).into())
                                    }
                                    EncodedTerm::BooleanLiteral(value) => {
                                        Some(Float::from(value).into())
                                    }
                                    EncodedTerm::SmallStringLiteral(value) => {
                                        parse_float_str(&value)
                                    }
                                    EncodedTerm::BigStringLiteral { value_id } => {
                                        parse_float_str(&dataset.get_str(&value_id).ok()??)
                                    }
                                    _ => None,
                                })
                            }
                            xsd::INTEGER => {
                                let e = self.expression_evaluator(
                                    &parameters[0],
                                    encoded_variables,
                                    stat_children,
                                );
                                let dataset = Rc::clone(&self.dataset);
                                Rc::new(move |tuple| match e(tuple)? {
                                    EncodedTerm::FloatLiteral(value) => {
                                        Some(Integer::try_from(value).ok()?.into())
                                    }
                                    EncodedTerm::DoubleLiteral(value) => {
                                        Some(Integer::try_from(value).ok()?.into())
                                    }
                                    EncodedTerm::IntegerLiteral(value) => Some(value.into()),
                                    EncodedTerm::DecimalLiteral(value) => {
                                        Some(Integer::try_from(value).ok()?.into())
                                    }
                                    EncodedTerm::BooleanLiteral(value) => {
                                        Some(Integer::from(value).into())
                                    }
                                    EncodedTerm::SmallStringLiteral(value) => {
                                        parse_integer_str(&value)
                                    }
                                    EncodedTerm::BigStringLiteral { value_id } => {
                                        parse_integer_str(&dataset.get_str(&value_id).ok()??)
                                    }
                                    _ => None,
                                })
                            }
                            xsd::DECIMAL => {
                                let e = self.expression_evaluator(
                                    &parameters[0],
                                    encoded_variables,
                                    stat_children,
                                );
                                let dataset = Rc::clone(&self.dataset);
                                Rc::new(move |tuple| match e(tuple)? {
                                    EncodedTerm::FloatLiteral(value) => {
                                        Some(Decimal::try_from(value).ok()?.into())
                                    }
                                    EncodedTerm::DoubleLiteral(value) => {
                                        Some(Decimal::try_from(value).ok()?.into())
                                    }
                                    EncodedTerm::IntegerLiteral(value) => {
                                        Some(Decimal::from(value).into())
                                    }
                                    EncodedTerm::DecimalLiteral(value) => Some(value.into()),
                                    EncodedTerm::BooleanLiteral(value) => {
                                        Some(Decimal::from(value).into())
                                    }
                                    EncodedTerm::SmallStringLiteral(value) => {
                                        parse_decimal_str(&value)
                                    }
                                    EncodedTerm::BigStringLiteral { value_id } => {
                                        parse_decimal_str(&dataset.get_str(&value_id).ok()??)
                                    }
                                    _ => None,
                                })
                            }
                            xsd::DATE => {
                                let e = self.expression_evaluator(
                                    &parameters[0],
                                    encoded_variables,
                                    stat_children,
                                );
                                let dataset = Rc::clone(&self.dataset);
                                Rc::new(move |tuple| match e(tuple)? {
                                    EncodedTerm::DateLiteral(value) => Some(value.into()),
                                    EncodedTerm::DateTimeLiteral(value) => {
                                        Some(Date::try_from(value).ok()?.into())
                                    }
                                    EncodedTerm::SmallStringLiteral(value) => {
                                        parse_date_str(&value)
                                    }
                                    EncodedTerm::BigStringLiteral { value_id } => {
                                        parse_date_str(&dataset.get_str(&value_id).ok()??)
                                    }
                                    _ => None,
                                })
                            }
                            xsd::TIME => {
                                let e = self.expression_evaluator(
                                    &parameters[0],
                                    encoded_variables,
                                    stat_children,
                                );
                                let dataset = Rc::clone(&self.dataset);
                                Rc::new(move |tuple| match e(tuple)? {
                                    EncodedTerm::TimeLiteral(value) => Some(value.into()),
                                    EncodedTerm::DateTimeLiteral(value) => {
                                        Some(Time::from(value).into())
                                    }
                                    EncodedTerm::SmallStringLiteral(value) => {
                                        parse_time_str(&value)
                                    }
                                    EncodedTerm::BigStringLiteral { value_id } => {
                                        parse_time_str(&dataset.get_str(&value_id).ok()??)
                                    }
                                    _ => None,
                                })
                            }
                            xsd::DATE_TIME => {
                                let e = self.expression_evaluator(
                                    &parameters[0],
                                    encoded_variables,
                                    stat_children,
                                );
                                let dataset = Rc::clone(&self.dataset);
                                Rc::new(move |tuple| match e(tuple)? {
                                    EncodedTerm::DateTimeLiteral(value) => Some(value.into()),
                                    EncodedTerm::DateLiteral(value) => {
                                        Some(DateTime::try_from(value).ok()?.into())
                                    }
                                    EncodedTerm::SmallStringLiteral(value) => {
                                        parse_date_time_str(&value)
                                    }
                                    EncodedTerm::BigStringLiteral { value_id } => {
                                        parse_date_time_str(&dataset.get_str(&value_id).ok()??)
                                    }
                                    _ => None,
                                })
                            }
                            xsd::DURATION => {
                                let e = self.expression_evaluator(
                                    &parameters[0],
                                    encoded_variables,
                                    stat_children,
                                );
                                let dataset = Rc::clone(&self.dataset);
                                Rc::new(move |tuple| match e(tuple)? {
                                    EncodedTerm::DurationLiteral(value) => Some(value.into()),
                                    EncodedTerm::YearMonthDurationLiteral(value) => {
                                        Some(Duration::from(value).into())
                                    }
                                    EncodedTerm::DayTimeDurationLiteral(value) => {
                                        Some(Duration::from(value).into())
                                    }
                                    EncodedTerm::SmallStringLiteral(value) => {
                                        parse_duration_str(&value)
                                    }
                                    EncodedTerm::BigStringLiteral { value_id } => {
                                        parse_duration_str(&dataset.get_str(&value_id).ok()??)
                                    }
                                    _ => None,
                                })
                            }
                            xsd::YEAR_MONTH_DURATION => {
                                let e = self.expression_evaluator(
                                    &parameters[0],
                                    encoded_variables,
                                    stat_children,
                                );
                                let dataset = Rc::clone(&self.dataset);
                                Rc::new(move |tuple| match e(tuple)? {
                                    EncodedTerm::DurationLiteral(value) => {
                                        Some(YearMonthDuration::try_from(value).ok()?.into())
                                    }
                                    EncodedTerm::YearMonthDurationLiteral(value) => {
                                        Some(value.into())
                                    }
                                    EncodedTerm::SmallStringLiteral(value) => {
                                        parse_year_month_duration_str(&value)
                                    }
                                    EncodedTerm::BigStringLiteral { value_id } => {
                                        parse_year_month_duration_str(
                                            &dataset.get_str(&value_id).ok()??,
                                        )
                                    }
                                    _ => None,
                                })
                            }
                            xsd::DAY_TIME_DURATION => {
                                let e = self.expression_evaluator(
                                    &parameters[0],
                                    encoded_variables,
                                    stat_children,
                                );
                                let dataset = Rc::clone(&self.dataset);
                                Rc::new(move |tuple| match e(tuple)? {
                                    EncodedTerm::DurationLiteral(value) => {
                                        Some(DayTimeDuration::try_from(value).ok()?.into())
                                    }
                                    EncodedTerm::DayTimeDurationLiteral(value) => {
                                        Some(value.into())
                                    }
                                    EncodedTerm::SmallStringLiteral(value) => {
                                        parse_day_time_duration_str(&value)
                                    }
                                    EncodedTerm::BigStringLiteral { value_id } => {
                                        parse_day_time_duration_str(
                                            &dataset.get_str(&value_id).ok()??,
                                        )
                                    }
                                    _ => None,
                                })
                            }
                            _ => Rc::new(|_| None),
                        }
                    }
                }
            }
        }
    }

    fn hash<H: Digest>(
        &self,
        parameters: &[Expression],
        encoded_variables: &mut Vec<Variable>,
        stat_children: &mut Vec<Rc<EvalNodeWithStats>>,
    ) -> Rc<dyn Fn(&EncodedTuple) -> Option<EncodedTerm>> {
        let arg = self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
        let dataset = Rc::clone(&self.dataset);
        Rc::new(move |tuple| {
            let input = to_simple_string(&dataset, &arg(tuple)?)?;
            let hash = hex::encode(H::new().chain_update(input.as_str()).finalize());
            Some(build_string_literal(&dataset, &hash))
        })
    }

    fn encode_term<'b>(&self, term: impl Into<TermRef<'b>>) -> EncodedTerm {
        self.dataset.encode_term(term)
    }

    fn encode_triple(&self, triple: &GroundTriple) -> EncodedTerm {
        EncodedTriple::new(
            match &triple.subject {
                GroundSubject::NamedNode(node) => self.encode_term(node),
                GroundSubject::Triple(triple) => self.encode_triple(triple),
            },
            self.encode_term(&triple.predicate),
            match &triple.object {
                GroundTerm::NamedNode(node) => self.encode_term(node),
                GroundTerm::Literal(literal) => self.encode_term(literal),
                GroundTerm::Triple(triple) => self.encode_triple(triple),
            },
        )
        .into()
    }

    fn encode_property_path(&self, path: &PropertyPathExpression) -> Rc<PropertyPath> {
        Rc::new(match path {
            PropertyPathExpression::NamedNode(node) => PropertyPath::Path(self.encode_term(node)),
            PropertyPathExpression::Reverse(p) => {
                PropertyPath::Reverse(self.encode_property_path(p))
            }
            PropertyPathExpression::Sequence(a, b) => {
                PropertyPath::Sequence(self.encode_property_path(a), self.encode_property_path(b))
            }
            PropertyPathExpression::Alternative(a, b) => PropertyPath::Alternative(
                self.encode_property_path(a),
                self.encode_property_path(b),
            ),
            PropertyPathExpression::ZeroOrMore(p) => {
                PropertyPath::ZeroOrMore(self.encode_property_path(p))
            }
            PropertyPathExpression::OneOrMore(p) => {
                PropertyPath::OneOrMore(self.encode_property_path(p))
            }
            PropertyPathExpression::ZeroOrOne(p) => {
                PropertyPath::ZeroOrOne(self.encode_property_path(p))
            }
            PropertyPathExpression::NegatedPropertySet(ps) => {
                PropertyPath::NegatedPropertySet(ps.iter().map(|p| self.encode_term(p)).collect())
            }
        })
    }

    fn template_value_from_term_or_variable(
        &self,
        term_or_variable: &TermPattern,
        variables: &mut Vec<Variable>,
        bnodes: &mut Vec<BlankNode>,
    ) -> TripleTemplateValue {
        match term_or_variable {
            TermPattern::Variable(variable) => {
                TripleTemplateValue::Variable(encode_variable(variables, variable))
            }
            TermPattern::NamedNode(node) => TripleTemplateValue::Constant(self.encode_term(node)),
            TermPattern::BlankNode(bnode) => {
                TripleTemplateValue::BlankNode(bnode_key(bnodes, bnode))
            }
            TermPattern::Literal(literal) => {
                TripleTemplateValue::Constant(self.encode_term(literal))
            }
            TermPattern::Triple(triple) => match (
                self.template_value_from_term_or_variable(&triple.subject, variables, bnodes),
                self.template_value_from_named_node_or_variable(&triple.predicate, variables),
                self.template_value_from_term_or_variable(&triple.object, variables, bnodes),
            ) {
                (
                    TripleTemplateValue::Constant(subject),
                    TripleTemplateValue::Constant(predicate),
                    TripleTemplateValue::Constant(object),
                ) => TripleTemplateValue::Constant(
                    EncodedTriple {
                        subject,
                        predicate,
                        object,
                    }
                    .into(),
                ),
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
                TripleTemplateValue::Variable(encode_variable(variables, variable))
            }
            NamedNodePattern::NamedNode(term) => {
                TripleTemplateValue::Constant(self.encode_term(term))
            }
        }
    }
}

fn to_bool(term: &EncodedTerm) -> Option<bool> {
    match term {
        EncodedTerm::BooleanLiteral(value) => Some((*value).into()),
        EncodedTerm::SmallStringLiteral(value) => Some(!value.is_empty()),
        EncodedTerm::BigStringLiteral { .. } => {
            Some(false) // A big literal can't be empty
        }
        EncodedTerm::FloatLiteral(value) => Some(Boolean::from(*value).into()),
        EncodedTerm::DoubleLiteral(value) => Some(Boolean::from(*value).into()),
        EncodedTerm::IntegerLiteral(value) => Some(Boolean::from(*value).into()),
        EncodedTerm::DecimalLiteral(value) => Some(Boolean::from(*value).into()),
        _ => None,
    }
}

fn to_string_id(dataset: &DatasetView, term: &EncodedTerm) -> Option<SmallStringOrId> {
    match term {
        EncodedTerm::NamedNode { iri_id } => Some(
            if let Ok(value) = SmallString::try_from(dataset.get_str(iri_id).ok()??.as_str()) {
                value.into()
            } else {
                SmallStringOrId::Big(*iri_id)
            },
        ),
        EncodedTerm::DefaultGraph
        | EncodedTerm::NumericalBlankNode { .. }
        | EncodedTerm::SmallBlankNode { .. }
        | EncodedTerm::BigBlankNode { .. }
        | EncodedTerm::Triple(_) => None,
        EncodedTerm::SmallStringLiteral(value)
        | EncodedTerm::SmallSmallLangStringLiteral { value, .. }
        | EncodedTerm::SmallBigLangStringLiteral { value, .. }
        | EncodedTerm::SmallTypedLiteral { value, .. } => Some((*value).into()),
        EncodedTerm::BigStringLiteral { value_id }
        | EncodedTerm::BigSmallLangStringLiteral { value_id, .. }
        | EncodedTerm::BigBigLangStringLiteral { value_id, .. }
        | EncodedTerm::BigTypedLiteral { value_id, .. } => Some((*value_id).into()),
        EncodedTerm::BooleanLiteral(value) => Some(build_string_id(
            dataset,
            if bool::from(*value) { "true" } else { "false" },
        )),
        EncodedTerm::FloatLiteral(value) => Some(build_string_id(dataset, &value.to_string())),
        EncodedTerm::DoubleLiteral(value) => Some(build_string_id(dataset, &value.to_string())),
        EncodedTerm::IntegerLiteral(value) => Some(build_string_id(dataset, &value.to_string())),
        EncodedTerm::DecimalLiteral(value) => Some(build_string_id(dataset, &value.to_string())),
        EncodedTerm::DateTimeLiteral(value) => Some(build_string_id(dataset, &value.to_string())),
        EncodedTerm::TimeLiteral(value) => Some(build_string_id(dataset, &value.to_string())),
        EncodedTerm::DateLiteral(value) => Some(build_string_id(dataset, &value.to_string())),
        EncodedTerm::GYearMonthLiteral(value) => Some(build_string_id(dataset, &value.to_string())),
        EncodedTerm::GYearLiteral(value) => Some(build_string_id(dataset, &value.to_string())),
        EncodedTerm::GMonthDayLiteral(value) => Some(build_string_id(dataset, &value.to_string())),
        EncodedTerm::GDayLiteral(value) => Some(build_string_id(dataset, &value.to_string())),
        EncodedTerm::GMonthLiteral(value) => Some(build_string_id(dataset, &value.to_string())),
        EncodedTerm::DurationLiteral(value) => Some(build_string_id(dataset, &value.to_string())),
        EncodedTerm::YearMonthDurationLiteral(value) => {
            Some(build_string_id(dataset, &value.to_string()))
        }
        EncodedTerm::DayTimeDurationLiteral(value) => {
            Some(build_string_id(dataset, &value.to_string()))
        }
    }
}

fn to_simple_string(dataset: &DatasetView, term: &EncodedTerm) -> Option<String> {
    match term {
        EncodedTerm::SmallStringLiteral(value) => Some((*value).into()),
        EncodedTerm::BigStringLiteral { value_id } => dataset.get_str(value_id).ok()?,
        _ => None,
    }
}

fn to_simple_string_id(term: &EncodedTerm) -> Option<SmallStringOrId> {
    match term {
        EncodedTerm::SmallStringLiteral(value) => Some((*value).into()),
        EncodedTerm::BigStringLiteral { value_id } => Some((*value_id).into()),
        _ => None,
    }
}

fn to_string(dataset: &DatasetView, term: &EncodedTerm) -> Option<String> {
    match term {
        EncodedTerm::SmallStringLiteral(value)
        | EncodedTerm::SmallSmallLangStringLiteral { value, .. }
        | EncodedTerm::SmallBigLangStringLiteral { value, .. } => Some((*value).into()),
        EncodedTerm::BigStringLiteral { value_id }
        | EncodedTerm::BigSmallLangStringLiteral { value_id, .. }
        | EncodedTerm::BigBigLangStringLiteral { value_id, .. } => {
            dataset.get_str(value_id).ok()?
        }
        _ => None,
    }
}

fn to_string_and_language(
    dataset: &DatasetView,
    term: &EncodedTerm,
) -> Option<(String, Option<SmallStringOrId>)> {
    match term {
        EncodedTerm::SmallStringLiteral(value) => Some(((*value).into(), None)),
        EncodedTerm::BigStringLiteral { value_id } => {
            Some((dataset.get_str(value_id).ok()??, None))
        }
        EncodedTerm::SmallSmallLangStringLiteral { value, language } => {
            Some(((*value).into(), Some((*language).into())))
        }
        EncodedTerm::SmallBigLangStringLiteral { value, language_id } => {
            Some(((*value).into(), Some((*language_id).into())))
        }
        EncodedTerm::BigSmallLangStringLiteral { value_id, language } => {
            Some((dataset.get_str(value_id).ok()??, Some((*language).into())))
        }
        EncodedTerm::BigBigLangStringLiteral {
            value_id,
            language_id,
        } => Some((
            dataset.get_str(value_id).ok()??,
            Some((*language_id).into()),
        )),
        _ => None,
    }
}

fn build_named_node(dataset: &DatasetView, iri: &str) -> EncodedTerm {
    dataset.encode_term(NamedNodeRef::new_unchecked(iri))
}

fn encode_named_node(dataset: &DatasetView, node: NamedNodeRef<'_>) -> EncodedTerm {
    dataset.encode_term(node)
}

fn build_string_literal(dataset: &DatasetView, value: &str) -> EncodedTerm {
    build_string_literal_from_id(build_string_id(dataset, value))
}

fn build_string_literal_from_id(id: SmallStringOrId) -> EncodedTerm {
    match id {
        SmallStringOrId::Small(value) => EncodedTerm::SmallStringLiteral(value),
        SmallStringOrId::Big(value_id) => EncodedTerm::BigStringLiteral { value_id },
    }
}

fn build_lang_string_literal(
    dataset: &DatasetView,
    value: &str,
    language_id: SmallStringOrId,
) -> EncodedTerm {
    build_lang_string_literal_from_id(build_string_id(dataset, value), language_id)
}

fn build_lang_string_literal_from_id(
    value_id: SmallStringOrId,
    language_id: SmallStringOrId,
) -> EncodedTerm {
    match (value_id, language_id) {
        (SmallStringOrId::Small(value), SmallStringOrId::Small(language)) => {
            EncodedTerm::SmallSmallLangStringLiteral { value, language }
        }
        (SmallStringOrId::Small(value), SmallStringOrId::Big(language_id)) => {
            EncodedTerm::SmallBigLangStringLiteral { value, language_id }
        }
        (SmallStringOrId::Big(value_id), SmallStringOrId::Small(language)) => {
            EncodedTerm::BigSmallLangStringLiteral { value_id, language }
        }
        (SmallStringOrId::Big(value_id), SmallStringOrId::Big(language_id)) => {
            EncodedTerm::BigBigLangStringLiteral {
                value_id,
                language_id,
            }
        }
    }
}

fn build_plain_literal(
    dataset: &DatasetView,
    value: &str,
    language: Option<SmallStringOrId>,
) -> EncodedTerm {
    if let Some(language_id) = language {
        build_lang_string_literal(dataset, value, language_id)
    } else {
        build_string_literal(dataset, value)
    }
}

fn build_string_id(dataset: &DatasetView, value: &str) -> SmallStringOrId {
    if let Ok(value) = SmallString::try_from(value) {
        value.into()
    } else {
        let id = StrHash::new(value);
        dataset.insert_str(&id, value);
        SmallStringOrId::Big(id)
    }
}

fn build_language_id(dataset: &DatasetView, value: &EncodedTerm) -> Option<SmallStringOrId> {
    let mut language = to_simple_string(dataset, value)?;
    language.make_ascii_lowercase();
    Some(build_string_id(
        dataset,
        LanguageTag::parse(language).ok()?.as_str(),
    ))
}

fn to_argument_compatible_strings(
    dataset: &DatasetView,
    arg1: &EncodedTerm,
    arg2: &EncodedTerm,
) -> Option<(String, String, Option<SmallStringOrId>)> {
    let (value1, language1) = to_string_and_language(dataset, arg1)?;
    let (value2, language2) = to_string_and_language(dataset, arg2)?;
    (language2.is_none() || language1 == language2).then_some((value1, value2, language1))
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

pub(super) fn compile_pattern(pattern: &str, flags: Option<&str>) -> Option<Regex> {
    let mut regex_builder = RegexBuilder::new(pattern);
    regex_builder.size_limit(REGEX_SIZE_LIMIT);
    if let Some(flags) = flags {
        for flag in flags.chars() {
            match flag {
                's' => {
                    regex_builder.dot_matches_new_line(true);
                }
                'm' => {
                    regex_builder.multi_line(true);
                }
                'i' => {
                    regex_builder.case_insensitive(true);
                }
                'x' => {
                    regex_builder.ignore_whitespace(true);
                }
                _ => (), // TODO: implement q
            }
        }
    }
    regex_builder.build().ok()
}

fn decode_bindings(
    dataset: Rc<DatasetView>,
    iter: EncodedTuplesIterator,
    variables: Arc<[Variable]>,
) -> QuerySolutionIter {
    let tuple_size = variables.len();
    QuerySolutionIter::new(
        variables,
        Box::new(iter.map(move |values| {
            let mut result = vec![None; tuple_size];
            for (i, value) in values?.iter().enumerate() {
                if let Some(term) = value {
                    result[i] = Some(dataset.decode_term(&term)?)
                }
            }
            Ok(result)
        })),
    )
}

// this is used to encode results from a BindingIterator into an EncodedTuplesIterator. This happens when SERVICE clauses are evaluated
fn encode_bindings(
    dataset: Rc<DatasetView>,
    variables: Rc<[Variable]>,
    iter: QuerySolutionIter,
) -> EncodedTuplesIterator {
    Box::new(iter.map(move |solution| {
        let mut encoded_terms = EncodedTuple::with_capacity(variables.len());
        for (variable, term) in solution?.iter() {
            put_variable_value(
                variable,
                &variables,
                dataset.encode_term(term),
                &mut encoded_terms,
            )
        }
        Ok(encoded_terms)
    }))
}

fn equals(a: &EncodedTerm, b: &EncodedTerm) -> Option<bool> {
    match a {
        EncodedTerm::DefaultGraph
        | EncodedTerm::NamedNode { .. }
        | EncodedTerm::NumericalBlankNode { .. }
        | EncodedTerm::SmallBlankNode { .. }
        | EncodedTerm::BigBlankNode { .. }
        | EncodedTerm::SmallSmallLangStringLiteral { .. }
        | EncodedTerm::SmallBigLangStringLiteral { .. }
        | EncodedTerm::BigSmallLangStringLiteral { .. }
        | EncodedTerm::BigBigLangStringLiteral { .. } => Some(a == b),
        EncodedTerm::SmallStringLiteral(a) => match b {
            EncodedTerm::SmallStringLiteral(b) => Some(a == b),
            EncodedTerm::SmallTypedLiteral { .. } | EncodedTerm::BigTypedLiteral { .. } => None,
            _ => Some(false),
        },
        EncodedTerm::BigStringLiteral { value_id: a } => match b {
            EncodedTerm::BigStringLiteral { value_id: b } => Some(a == b),
            EncodedTerm::SmallTypedLiteral { .. } | EncodedTerm::BigTypedLiteral { .. } => None,
            _ => Some(false),
        },
        EncodedTerm::SmallTypedLiteral { .. } => match b {
            EncodedTerm::SmallTypedLiteral { .. } if a == b => Some(true),
            EncodedTerm::NamedNode { .. }
            | EncodedTerm::NumericalBlankNode { .. }
            | EncodedTerm::SmallBlankNode { .. }
            | EncodedTerm::BigBlankNode { .. }
            | EncodedTerm::SmallSmallLangStringLiteral { .. }
            | EncodedTerm::SmallBigLangStringLiteral { .. }
            | EncodedTerm::BigSmallLangStringLiteral { .. }
            | EncodedTerm::BigBigLangStringLiteral { .. }
            | EncodedTerm::BigTypedLiteral { .. } => Some(false),
            _ => None,
        },
        EncodedTerm::BigTypedLiteral { .. } => match b {
            EncodedTerm::BigTypedLiteral { .. } if a == b => Some(true),
            EncodedTerm::NamedNode { .. }
            | EncodedTerm::NumericalBlankNode { .. }
            | EncodedTerm::SmallBlankNode { .. }
            | EncodedTerm::BigBlankNode { .. }
            | EncodedTerm::SmallSmallLangStringLiteral { .. }
            | EncodedTerm::SmallBigLangStringLiteral { .. }
            | EncodedTerm::BigSmallLangStringLiteral { .. }
            | EncodedTerm::BigBigLangStringLiteral { .. }
            | EncodedTerm::SmallTypedLiteral { .. } => Some(false),
            _ => None,
        },
        EncodedTerm::BooleanLiteral(a) => match b {
            EncodedTerm::BooleanLiteral(b) => Some(a == b),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::FloatLiteral(a) => match b {
            EncodedTerm::FloatLiteral(b) => Some(a == b),
            EncodedTerm::DoubleLiteral(b) => Some(Double::from(*a) == *b),
            EncodedTerm::IntegerLiteral(b) => Some(*a == (*b).into()),
            EncodedTerm::DecimalLiteral(b) => Some(*a == (*b).into()),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::DoubleLiteral(a) => match b {
            EncodedTerm::FloatLiteral(b) => Some(*a == (*b).into()),
            EncodedTerm::DoubleLiteral(b) => Some(a == b),
            EncodedTerm::IntegerLiteral(b) => Some(*a == (*b).into()),
            EncodedTerm::DecimalLiteral(b) => Some(*a == (*b).into()),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::IntegerLiteral(a) => match b {
            EncodedTerm::FloatLiteral(b) => Some(Float::from(*a) == *b),
            EncodedTerm::DoubleLiteral(b) => Some(Double::from(*a) == *b),
            EncodedTerm::IntegerLiteral(b) => Some(a == b),
            EncodedTerm::DecimalLiteral(b) => Some(Decimal::from(*a) == *b),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::DecimalLiteral(a) => match b {
            EncodedTerm::FloatLiteral(b) => Some(Float::from(*a) == *b),
            EncodedTerm::DoubleLiteral(b) => Some(Double::from(*a) == *b),
            EncodedTerm::IntegerLiteral(b) => Some(*a == (*b).into()),
            EncodedTerm::DecimalLiteral(b) => Some(a == b),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::DateTimeLiteral(a) => match b {
            EncodedTerm::DateTimeLiteral(b) => Some(a == b),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::TimeLiteral(a) => match b {
            EncodedTerm::TimeLiteral(b) => Some(a == b),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::DateLiteral(a) => match b {
            EncodedTerm::DateLiteral(b) => Some(a == b),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::GYearMonthLiteral(a) => match b {
            EncodedTerm::GYearMonthLiteral(b) => Some(a == b),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::GYearLiteral(a) => match b {
            EncodedTerm::GYearLiteral(b) => Some(a == b),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::GMonthDayLiteral(a) => match b {
            EncodedTerm::GMonthDayLiteral(b) => Some(a == b),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::GDayLiteral(a) => match b {
            EncodedTerm::GDayLiteral(b) => Some(a == b),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::GMonthLiteral(a) => match b {
            EncodedTerm::GMonthLiteral(b) => Some(a == b),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::DurationLiteral(a) => match b {
            EncodedTerm::DurationLiteral(b) => Some(a == b),
            EncodedTerm::YearMonthDurationLiteral(b) => Some(a == b),
            EncodedTerm::DayTimeDurationLiteral(b) => Some(a == b),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::YearMonthDurationLiteral(a) => match b {
            EncodedTerm::DurationLiteral(b) => Some(a == b),
            EncodedTerm::YearMonthDurationLiteral(b) => Some(a == b),
            EncodedTerm::DayTimeDurationLiteral(b) => Some(a == b),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::DayTimeDurationLiteral(a) => match b {
            EncodedTerm::DurationLiteral(b) => Some(a == b),
            EncodedTerm::YearMonthDurationLiteral(b) => Some(a == b),
            EncodedTerm::DayTimeDurationLiteral(b) => Some(a == b),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::Triple(a) => {
            if let EncodedTerm::Triple(b) = b {
                Some(
                    equals(&a.subject, &b.subject)?
                        && equals(&a.predicate, &b.predicate)?
                        && equals(&a.object, &b.object)?,
                )
            } else {
                Some(false)
            }
        }
    }
}

fn cmp_terms(dataset: &DatasetView, a: Option<&EncodedTerm>, b: Option<&EncodedTerm>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => match a {
            EncodedTerm::SmallBlankNode(a) => match b {
                EncodedTerm::SmallBlankNode(b) => a.cmp(b),
                EncodedTerm::BigBlankNode { id_id: b } => {
                    compare_str_str_id(dataset, a, b).unwrap_or(Ordering::Equal)
                }
                EncodedTerm::NumericalBlankNode { id: b } => {
                    a.as_str().cmp(BlankNode::new_from_unique_id(*b).as_str())
                }
                _ => Ordering::Less,
            },
            EncodedTerm::BigBlankNode { id_id: a } => match b {
                EncodedTerm::SmallBlankNode(b) => {
                    compare_str_id_str(dataset, a, b).unwrap_or(Ordering::Equal)
                }
                EncodedTerm::BigBlankNode { id_id: b } => {
                    compare_str_ids(dataset, a, b).unwrap_or(Ordering::Equal)
                }
                EncodedTerm::NumericalBlankNode { id: b } => {
                    compare_str_id_str(dataset, a, BlankNode::new_from_unique_id(*b).as_str())
                        .unwrap_or(Ordering::Equal)
                }
                _ => Ordering::Less,
            },
            EncodedTerm::NumericalBlankNode { id: a } => {
                let a = BlankNode::new_from_unique_id(*a);
                match b {
                    EncodedTerm::SmallBlankNode(b) => a.as_str().cmp(b),
                    EncodedTerm::BigBlankNode { id_id: b } => {
                        compare_str_str_id(dataset, a.as_str(), b).unwrap_or(Ordering::Equal)
                    }
                    EncodedTerm::NumericalBlankNode { id: b } => {
                        a.as_str().cmp(BlankNode::new_from_unique_id(*b).as_str())
                    }
                    _ => Ordering::Less,
                }
            }
            EncodedTerm::NamedNode { iri_id: a } => match b {
                EncodedTerm::NamedNode { iri_id: b } => {
                    compare_str_ids(dataset, a, b).unwrap_or(Ordering::Equal)
                }
                _ if b.is_blank_node() => Ordering::Greater,
                _ => Ordering::Less,
            },
            EncodedTerm::Triple(a) => match b {
                EncodedTerm::Triple(b) => {
                    match cmp_terms(dataset, Some(&a.subject), Some(&b.subject)) {
                        Ordering::Equal => {
                            match cmp_terms(dataset, Some(&a.predicate), Some(&b.predicate)) {
                                Ordering::Equal => {
                                    cmp_terms(dataset, Some(&a.object), Some(&b.object))
                                }
                                o => o,
                            }
                        }
                        o => o,
                    }
                }
                _ => Ordering::Greater,
            },
            _ => match b {
                _ if b.is_named_node() || b.is_blank_node() => Ordering::Greater,
                _ if b.is_triple() => Ordering::Less,
                _ => {
                    if let Some(ord) = partial_cmp_literals(dataset, a, b) {
                        ord
                    } else if let (Ok(Term::Literal(a)), Ok(Term::Literal(b))) =
                        (dataset.decode_term(a), dataset.decode_term(b))
                    {
                        (a.value(), a.datatype(), a.language()).cmp(&(
                            b.value(),
                            b.datatype(),
                            b.language(),
                        ))
                    } else {
                        Ordering::Equal // Should never happen
                    }
                }
            },
        },
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

fn partial_cmp(dataset: &DatasetView, a: &EncodedTerm, b: &EncodedTerm) -> Option<Ordering> {
    if a == b {
        Some(Ordering::Equal)
    } else if let EncodedTerm::Triple(a) = a {
        if let EncodedTerm::Triple(b) = b {
            match partial_cmp(dataset, &a.subject, &b.subject) {
                Some(Ordering::Equal) => match partial_cmp(dataset, &a.predicate, &b.predicate) {
                    Some(Ordering::Equal) => partial_cmp(dataset, &a.object, &b.object),
                    o => o,
                },
                o => o,
            }
        } else {
            None
        }
    } else {
        partial_cmp_literals(dataset, a, b)
    }
}

fn partial_cmp_literals(
    dataset: &DatasetView,
    a: &EncodedTerm,
    b: &EncodedTerm,
) -> Option<Ordering> {
    match a {
        EncodedTerm::SmallStringLiteral(a) => match b {
            EncodedTerm::SmallStringLiteral(b) => a.partial_cmp(b),
            EncodedTerm::BigStringLiteral { value_id: b } => compare_str_str_id(dataset, a, b),
            _ => None,
        },
        EncodedTerm::BigStringLiteral { value_id: a } => match b {
            EncodedTerm::SmallStringLiteral(b) => compare_str_id_str(dataset, a, b),
            EncodedTerm::BigStringLiteral { value_id: b } => compare_str_ids(dataset, a, b),
            _ => None,
        },
        EncodedTerm::SmallSmallLangStringLiteral {
            value: a,
            language: la,
        } => match b {
            EncodedTerm::SmallSmallLangStringLiteral {
                value: b,
                language: lb,
            } if la == lb => a.partial_cmp(b),
            EncodedTerm::BigSmallLangStringLiteral {
                value_id: b,
                language: lb,
            } if la == lb => compare_str_str_id(dataset, a, b),
            _ => None,
        },
        EncodedTerm::SmallBigLangStringLiteral {
            value: a,
            language_id: la,
        } => match b {
            EncodedTerm::SmallBigLangStringLiteral {
                value: b,
                language_id: lb,
            } if la == lb => a.partial_cmp(b),
            EncodedTerm::BigBigLangStringLiteral {
                value_id: b,
                language_id: lb,
            } if la == lb => compare_str_str_id(dataset, a, b),
            _ => None,
        },
        EncodedTerm::BigSmallLangStringLiteral {
            value_id: a,
            language: la,
        } => match b {
            EncodedTerm::SmallSmallLangStringLiteral {
                value: b,
                language: lb,
            } if la == lb => compare_str_id_str(dataset, a, b),
            EncodedTerm::BigSmallLangStringLiteral {
                value_id: b,
                language: lb,
            } if la == lb => compare_str_ids(dataset, a, b),
            _ => None,
        },
        EncodedTerm::BigBigLangStringLiteral {
            value_id: a,
            language_id: la,
        } => match b {
            EncodedTerm::SmallBigLangStringLiteral {
                value: b,
                language_id: lb,
            } if la == lb => compare_str_id_str(dataset, a, b),
            EncodedTerm::BigBigLangStringLiteral {
                value_id: b,
                language_id: lb,
            } if la == lb => compare_str_ids(dataset, a, b),
            _ => None,
        },
        EncodedTerm::FloatLiteral(a) => match b {
            EncodedTerm::FloatLiteral(b) => a.partial_cmp(b),
            EncodedTerm::DoubleLiteral(b) => Double::from(*a).partial_cmp(b),
            EncodedTerm::IntegerLiteral(b) => a.partial_cmp(&Float::from(*b)),
            EncodedTerm::DecimalLiteral(b) => a.partial_cmp(&(*b).into()),
            _ => None,
        },
        EncodedTerm::DoubleLiteral(a) => match b {
            EncodedTerm::FloatLiteral(b) => a.partial_cmp(&(*b).into()),
            EncodedTerm::DoubleLiteral(b) => a.partial_cmp(b),
            EncodedTerm::IntegerLiteral(b) => a.partial_cmp(&Double::from(*b)),
            EncodedTerm::DecimalLiteral(b) => a.partial_cmp(&(*b).into()),
            _ => None,
        },
        EncodedTerm::IntegerLiteral(a) => match b {
            EncodedTerm::FloatLiteral(b) => Float::from(*a).partial_cmp(b),
            EncodedTerm::DoubleLiteral(b) => Double::from(*a).partial_cmp(b),
            EncodedTerm::IntegerLiteral(b) => a.partial_cmp(b),
            EncodedTerm::DecimalLiteral(b) => Decimal::from(*a).partial_cmp(b),
            _ => None,
        },
        EncodedTerm::DecimalLiteral(a) => match b {
            EncodedTerm::FloatLiteral(b) => Float::from(*a).partial_cmp(b),
            EncodedTerm::DoubleLiteral(b) => Double::from(*a).partial_cmp(b),
            EncodedTerm::IntegerLiteral(b) => a.partial_cmp(&Decimal::from(*b)),
            EncodedTerm::DecimalLiteral(b) => a.partial_cmp(b),
            _ => None,
        },
        EncodedTerm::DateTimeLiteral(a) => {
            if let EncodedTerm::DateTimeLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        EncodedTerm::TimeLiteral(a) => {
            if let EncodedTerm::TimeLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        EncodedTerm::DateLiteral(a) => {
            if let EncodedTerm::DateLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        EncodedTerm::GYearMonthLiteral(a) => {
            if let EncodedTerm::GYearMonthLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        EncodedTerm::GYearLiteral(a) => {
            if let EncodedTerm::GYearLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        EncodedTerm::GMonthDayLiteral(a) => {
            if let EncodedTerm::GMonthDayLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        EncodedTerm::GDayLiteral(a) => {
            if let EncodedTerm::GDayLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        EncodedTerm::GMonthLiteral(a) => {
            if let EncodedTerm::GMonthLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        EncodedTerm::DurationLiteral(a) => match b {
            EncodedTerm::DurationLiteral(b) => a.partial_cmp(b),
            EncodedTerm::YearMonthDurationLiteral(b) => a.partial_cmp(b),
            EncodedTerm::DayTimeDurationLiteral(b) => a.partial_cmp(b),
            _ => None,
        },
        EncodedTerm::YearMonthDurationLiteral(a) => match b {
            EncodedTerm::DurationLiteral(b) => a.partial_cmp(b),
            EncodedTerm::YearMonthDurationLiteral(b) => a.partial_cmp(b),
            EncodedTerm::DayTimeDurationLiteral(b) => a.partial_cmp(b),
            _ => None,
        },
        EncodedTerm::DayTimeDurationLiteral(a) => match b {
            EncodedTerm::DurationLiteral(b) => a.partial_cmp(b),
            EncodedTerm::YearMonthDurationLiteral(b) => a.partial_cmp(b),
            EncodedTerm::DayTimeDurationLiteral(b) => a.partial_cmp(b),
            _ => None,
        },
        _ => None,
    }
}

fn compare_str_ids(dataset: &DatasetView, a: &StrHash, b: &StrHash) -> Option<Ordering> {
    Some(dataset.get_str(a).ok()??.cmp(&dataset.get_str(b).ok()??))
}

fn compare_str_id_str(dataset: &DatasetView, a: &StrHash, b: &str) -> Option<Ordering> {
    Some(dataset.get_str(a).ok()??.as_str().cmp(b))
}

fn compare_str_str_id(dataset: &DatasetView, a: &str, b: &StrHash) -> Option<Ordering> {
    Some(a.cmp(dataset.get_str(b).ok()??.as_str()))
}

fn datatype(dataset: &DatasetView, value: &EncodedTerm) -> Option<EncodedTerm> {
    // TODO: optimize?
    match value {
        EncodedTerm::NamedNode { .. }
        | EncodedTerm::SmallBlankNode { .. }
        | EncodedTerm::BigBlankNode { .. }
        | EncodedTerm::NumericalBlankNode { .. }
        | EncodedTerm::DefaultGraph
        | EncodedTerm::Triple(_) => None,
        EncodedTerm::SmallStringLiteral(_) | EncodedTerm::BigStringLiteral { .. } => {
            Some(encode_named_node(dataset, xsd::STRING))
        }
        EncodedTerm::SmallSmallLangStringLiteral { .. }
        | EncodedTerm::SmallBigLangStringLiteral { .. }
        | EncodedTerm::BigSmallLangStringLiteral { .. }
        | EncodedTerm::BigBigLangStringLiteral { .. } => {
            Some(encode_named_node(dataset, rdf::LANG_STRING))
        }
        EncodedTerm::SmallTypedLiteral { datatype_id, .. }
        | EncodedTerm::BigTypedLiteral { datatype_id, .. } => Some(EncodedTerm::NamedNode {
            iri_id: *datatype_id,
        }),
        EncodedTerm::BooleanLiteral(..) => Some(encode_named_node(dataset, xsd::BOOLEAN)),
        EncodedTerm::FloatLiteral(..) => Some(encode_named_node(dataset, xsd::FLOAT)),
        EncodedTerm::DoubleLiteral(..) => Some(encode_named_node(dataset, xsd::DOUBLE)),
        EncodedTerm::IntegerLiteral(..) => Some(encode_named_node(dataset, xsd::INTEGER)),
        EncodedTerm::DecimalLiteral(..) => Some(encode_named_node(dataset, xsd::DECIMAL)),
        EncodedTerm::DateTimeLiteral(..) => Some(encode_named_node(dataset, xsd::DATE_TIME)),
        EncodedTerm::TimeLiteral(..) => Some(encode_named_node(dataset, xsd::TIME)),
        EncodedTerm::DateLiteral(..) => Some(encode_named_node(dataset, xsd::DATE)),
        EncodedTerm::GYearMonthLiteral(..) => Some(encode_named_node(dataset, xsd::G_YEAR_MONTH)),
        EncodedTerm::GYearLiteral(..) => Some(encode_named_node(dataset, xsd::G_YEAR)),
        EncodedTerm::GMonthDayLiteral(..) => Some(encode_named_node(dataset, xsd::G_MONTH_DAY)),
        EncodedTerm::GDayLiteral(..) => Some(encode_named_node(dataset, xsd::G_DAY)),
        EncodedTerm::GMonthLiteral(..) => Some(encode_named_node(dataset, xsd::G_MONTH)),
        EncodedTerm::DurationLiteral(..) => Some(encode_named_node(dataset, xsd::DURATION)),
        EncodedTerm::YearMonthDurationLiteral(..) => {
            Some(encode_named_node(dataset, xsd::YEAR_MONTH_DURATION))
        }
        EncodedTerm::DayTimeDurationLiteral(..) => {
            Some(encode_named_node(dataset, xsd::DAY_TIME_DURATION))
        }
    }
}

enum NumericBinaryOperands {
    Float(Float, Float),
    Double(Double, Double),
    Integer(Integer, Integer),
    Decimal(Decimal, Decimal),
    Duration(Duration, Duration),
    YearMonthDuration(YearMonthDuration, YearMonthDuration),
    DayTimeDuration(DayTimeDuration, DayTimeDuration),
    DateTime(DateTime, DateTime),
    Time(Time, Time),
    Date(Date, Date),
    DateTimeDuration(DateTime, Duration),
    DateTimeYearMonthDuration(DateTime, YearMonthDuration),
    DateTimeDayTimeDuration(DateTime, DayTimeDuration),
    DateDuration(Date, Duration),
    DateYearMonthDuration(Date, YearMonthDuration),
    DateDayTimeDuration(Date, DayTimeDuration),
    TimeDuration(Time, Duration),
    TimeDayTimeDuration(Time, DayTimeDuration),
}

impl NumericBinaryOperands {
    fn new(a: EncodedTerm, b: EncodedTerm) -> Option<Self> {
        match (a, b) {
            (EncodedTerm::FloatLiteral(v1), EncodedTerm::FloatLiteral(v2)) => {
                Some(Self::Float(v1, v2))
            }
            (EncodedTerm::FloatLiteral(v1), EncodedTerm::DoubleLiteral(v2)) => {
                Some(Self::Double(v1.into(), v2))
            }
            (EncodedTerm::FloatLiteral(v1), EncodedTerm::IntegerLiteral(v2)) => {
                Some(Self::Float(v1, v2.into()))
            }
            (EncodedTerm::FloatLiteral(v1), EncodedTerm::DecimalLiteral(v2)) => {
                Some(Self::Float(v1, v2.into()))
            }
            (EncodedTerm::DoubleLiteral(v1), EncodedTerm::FloatLiteral(v2)) => {
                Some(Self::Double(v1, v2.into()))
            }
            (EncodedTerm::DoubleLiteral(v1), EncodedTerm::DoubleLiteral(v2)) => {
                Some(Self::Double(v1, v2))
            }
            (EncodedTerm::DoubleLiteral(v1), EncodedTerm::IntegerLiteral(v2)) => {
                Some(Self::Double(v1, v2.into()))
            }
            (EncodedTerm::DoubleLiteral(v1), EncodedTerm::DecimalLiteral(v2)) => {
                Some(Self::Double(v1, v2.into()))
            }
            (EncodedTerm::IntegerLiteral(v1), EncodedTerm::FloatLiteral(v2)) => {
                Some(Self::Float(v1.into(), v2))
            }
            (EncodedTerm::IntegerLiteral(v1), EncodedTerm::DoubleLiteral(v2)) => {
                Some(Self::Double(v1.into(), v2))
            }
            (EncodedTerm::IntegerLiteral(v1), EncodedTerm::IntegerLiteral(v2)) => {
                Some(Self::Integer(v1, v2))
            }
            (EncodedTerm::IntegerLiteral(v1), EncodedTerm::DecimalLiteral(v2)) => {
                Some(Self::Decimal(v1.into(), v2))
            }
            (EncodedTerm::DecimalLiteral(v1), EncodedTerm::FloatLiteral(v2)) => {
                Some(Self::Float(v1.into(), v2))
            }
            (EncodedTerm::DecimalLiteral(v1), EncodedTerm::DoubleLiteral(v2)) => {
                Some(Self::Double(v1.into(), v2))
            }
            (EncodedTerm::DecimalLiteral(v1), EncodedTerm::IntegerLiteral(v2)) => {
                Some(Self::Decimal(v1, v2.into()))
            }
            (EncodedTerm::DecimalLiteral(v1), EncodedTerm::DecimalLiteral(v2)) => {
                Some(Self::Decimal(v1, v2))
            }
            (EncodedTerm::DurationLiteral(v1), EncodedTerm::DurationLiteral(v2)) => {
                Some(Self::Duration(v1, v2))
            }
            (EncodedTerm::DurationLiteral(v1), EncodedTerm::YearMonthDurationLiteral(v2)) => {
                Some(Self::Duration(v1, v2.into()))
            }
            (EncodedTerm::DurationLiteral(v1), EncodedTerm::DayTimeDurationLiteral(v2)) => {
                Some(Self::Duration(v1, v2.into()))
            }
            (EncodedTerm::YearMonthDurationLiteral(v1), EncodedTerm::DurationLiteral(v2)) => {
                Some(Self::Duration(v1.into(), v2))
            }
            (
                EncodedTerm::YearMonthDurationLiteral(v1),
                EncodedTerm::YearMonthDurationLiteral(v2),
            ) => Some(Self::YearMonthDuration(v1, v2)),
            (
                EncodedTerm::YearMonthDurationLiteral(v1),
                EncodedTerm::DayTimeDurationLiteral(v2),
            ) => Some(Self::Duration(v1.into(), v2.into())),
            (EncodedTerm::DayTimeDurationLiteral(v1), EncodedTerm::DurationLiteral(v2)) => {
                Some(Self::Duration(v1.into(), v2))
            }
            (
                EncodedTerm::DayTimeDurationLiteral(v1),
                EncodedTerm::YearMonthDurationLiteral(v2),
            ) => Some(Self::Duration(v1.into(), v2.into())),
            (EncodedTerm::DayTimeDurationLiteral(v1), EncodedTerm::DayTimeDurationLiteral(v2)) => {
                Some(Self::DayTimeDuration(v1, v2))
            }
            (EncodedTerm::DateTimeLiteral(v1), EncodedTerm::DateTimeLiteral(v2)) => {
                Some(Self::DateTime(v1, v2))
            }
            (EncodedTerm::DateLiteral(v1), EncodedTerm::DateLiteral(v2)) => {
                Some(Self::Date(v1, v2))
            }
            (EncodedTerm::TimeLiteral(v1), EncodedTerm::TimeLiteral(v2)) => {
                Some(Self::Time(v1, v2))
            }
            (EncodedTerm::DateTimeLiteral(v1), EncodedTerm::DurationLiteral(v2)) => {
                Some(Self::DateTimeDuration(v1, v2))
            }
            (EncodedTerm::DateTimeLiteral(v1), EncodedTerm::YearMonthDurationLiteral(v2)) => {
                Some(Self::DateTimeYearMonthDuration(v1, v2))
            }
            (EncodedTerm::DateTimeLiteral(v1), EncodedTerm::DayTimeDurationLiteral(v2)) => {
                Some(Self::DateTimeDayTimeDuration(v1, v2))
            }
            (EncodedTerm::DateLiteral(v1), EncodedTerm::DurationLiteral(v2)) => {
                Some(Self::DateDuration(v1, v2))
            }
            (EncodedTerm::DateLiteral(v1), EncodedTerm::YearMonthDurationLiteral(v2)) => {
                Some(Self::DateYearMonthDuration(v1, v2))
            }
            (EncodedTerm::DateLiteral(v1), EncodedTerm::DayTimeDurationLiteral(v2)) => {
                Some(Self::DateDayTimeDuration(v1, v2))
            }
            (EncodedTerm::TimeLiteral(v1), EncodedTerm::DurationLiteral(v2)) => {
                Some(Self::TimeDuration(v1, v2))
            }
            (EncodedTerm::TimeLiteral(v1), EncodedTerm::DayTimeDurationLiteral(v2)) => {
                Some(Self::TimeDayTimeDuration(v1, v2))
            }
            _ => None,
        }
    }
}

#[derive(Clone)]
enum TupleSelector {
    Constant(EncodedTerm),
    Variable(usize),
    TriplePattern(Rc<TripleTupleSelector>),
}

impl TupleSelector {
    fn from_ground_term_pattern(
        term_pattern: &GroundTermPattern,
        variables: &mut Vec<Variable>,
        dataset: &DatasetView,
    ) -> Self {
        match term_pattern {
            GroundTermPattern::Variable(variable) => {
                Self::Variable(encode_variable(variables, variable))
            }
            GroundTermPattern::NamedNode(term) => Self::Constant(dataset.encode_term(term)),
            GroundTermPattern::Literal(term) => Self::Constant(dataset.encode_term(term)),
            GroundTermPattern::Triple(triple) => {
                match (
                    Self::from_ground_term_pattern(&triple.subject, variables, dataset),
                    Self::from_named_node_pattern(&triple.predicate, variables, dataset),
                    Self::from_ground_term_pattern(&triple.object, variables, dataset),
                ) {
                    (
                        Self::Constant(subject),
                        Self::Constant(predicate),
                        Self::Constant(object),
                    ) => Self::Constant(
                        EncodedTriple {
                            subject,
                            predicate,
                            object,
                        }
                        .into(),
                    ),
                    (subject, predicate, object) => {
                        Self::TriplePattern(Rc::new(TripleTupleSelector {
                            subject,
                            predicate,
                            object,
                        }))
                    }
                }
            }
        }
    }

    fn from_named_node_pattern(
        named_node_pattern: &NamedNodePattern,
        variables: &mut Vec<Variable>,
        dataset: &DatasetView,
    ) -> Self {
        match named_node_pattern {
            NamedNodePattern::Variable(variable) => {
                Self::Variable(encode_variable(variables, variable))
            }
            NamedNodePattern::NamedNode(term) => Self::Constant(dataset.encode_term(term)),
        }
    }

    fn from_graph_name_pattern(
        graph_name_pattern: &Option<NamedNodePattern>,
        variables: &mut Vec<Variable>,
        dataset: &DatasetView,
    ) -> Self {
        if let Some(graph_name_pattern) = graph_name_pattern {
            Self::from_named_node_pattern(graph_name_pattern, variables, dataset)
        } else {
            Self::Constant(EncodedTerm::DefaultGraph)
        }
    }

    fn get_pattern_value(&self, tuple: &EncodedTuple) -> Option<EncodedTerm> {
        match self {
            Self::Constant(c) => Some(c.clone()),
            Self::Variable(v) => tuple.get(*v).cloned(),
            Self::TriplePattern(triple) => Some(
                EncodedTriple {
                    subject: triple.subject.get_pattern_value(tuple)?,
                    predicate: triple.predicate.get_pattern_value(tuple)?,
                    object: triple.object.get_pattern_value(tuple)?,
                }
                .into(),
            ),
        }
    }
}

struct TripleTupleSelector {
    subject: TupleSelector,
    predicate: TupleSelector,
    object: TupleSelector,
}

fn put_pattern_value(
    selector: &TupleSelector,
    value: EncodedTerm,
    tuple: &mut EncodedTuple,
) -> Option<()> {
    match selector {
        TupleSelector::Constant(c) => (*c == value).then_some(()),
        TupleSelector::Variable(v) => {
            if let Some(old) = tuple.get(*v) {
                (value == *old).then_some(())
            } else {
                tuple.set(*v, value);
                Some(())
            }
        }
        TupleSelector::TriplePattern(triple) => {
            if let EncodedTerm::Triple(value) = value {
                put_pattern_value(&triple.subject, value.subject.clone(), tuple)?;
                put_pattern_value(&triple.predicate, value.predicate.clone(), tuple)?;
                put_pattern_value(&triple.object, value.object.clone(), tuple)
            } else {
                None
            }
        }
    }
}

fn put_variable_value(
    selector: &Variable,
    variables: &[Variable],
    value: EncodedTerm,
    tuple: &mut EncodedTuple,
) {
    for (i, v) in variables.iter().enumerate() {
        if selector == v {
            tuple.set(i, value);
            break;
        }
    }
}

pub fn are_compatible_and_not_disjointed(a: &EncodedTuple, b: &EncodedTuple) -> bool {
    let mut found_intersection = false;
    for (a_value, b_value) in a.iter().zip(b.iter()) {
        if let (Some(a_value), Some(b_value)) = (a_value, b_value) {
            if a_value != b_value {
                return false;
            }
            found_intersection = true;
        }
    }
    found_intersection
}

pub enum PropertyPath {
    Path(EncodedTerm),
    Reverse(Rc<Self>),
    Sequence(Rc<Self>, Rc<Self>),
    Alternative(Rc<Self>, Rc<Self>),
    ZeroOrMore(Rc<Self>),
    OneOrMore(Rc<Self>),
    ZeroOrOne(Rc<Self>),
    NegatedPropertySet(Rc<[EncodedTerm]>),
}

#[derive(Clone)]
struct PathEvaluator {
    dataset: Rc<DatasetView>,
}

impl PathEvaluator {
    fn eval_closed_in_graph(
        &self,
        path: &PropertyPath,
        start: &EncodedTerm,
        end: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> Result<bool, EvaluationError> {
        Ok(match path {
            PropertyPath::Path(p) => self
                .dataset
                .encoded_quads_for_pattern(Some(start), Some(p), Some(end), Some(graph_name))
                .next()
                .transpose()?
                .is_some(),
            PropertyPath::Reverse(p) => self.eval_closed_in_graph(p, end, start, graph_name)?,
            PropertyPath::Sequence(a, b) => self
                .eval_from_in_graph(a, start, graph_name)
                .find_map(|middle| {
                    middle
                        .and_then(|middle| {
                            Ok(self
                                .eval_closed_in_graph(b, &middle, end, graph_name)?
                                .then_some(()))
                        })
                        .transpose()
                })
                .transpose()?
                .is_some(),
            PropertyPath::Alternative(a, b) => {
                self.eval_closed_in_graph(a, start, end, graph_name)?
                    || self.eval_closed_in_graph(b, start, end, graph_name)?
            }
            PropertyPath::ZeroOrMore(p) => {
                if start == end {
                    self.is_subject_or_object_in_graph(start, graph_name)?
                } else {
                    look_in_transitive_closure(
                        self.eval_from_in_graph(p, start, graph_name),
                        move |e| self.eval_from_in_graph(p, &e, graph_name),
                        end,
                    )?
                }
            }
            PropertyPath::OneOrMore(p) => look_in_transitive_closure(
                self.eval_from_in_graph(p, start, graph_name),
                move |e| self.eval_from_in_graph(p, &e, graph_name),
                end,
            )?,
            PropertyPath::ZeroOrOne(p) => {
                if start == end {
                    self.is_subject_or_object_in_graph(start, graph_name)
                } else {
                    self.eval_closed_in_graph(p, start, end, graph_name)
                }?
            }
            PropertyPath::NegatedPropertySet(ps) => self
                .dataset
                .encoded_quads_for_pattern(Some(start), None, Some(end), Some(graph_name))
                .find_map(move |t| match t {
                    Ok(t) => {
                        if ps.iter().any(|p| *p == t.predicate) {
                            None
                        } else {
                            Some(Ok(()))
                        }
                    }
                    Err(e) => Some(Err(e)),
                })
                .transpose()?
                .is_some(),
        })
    }

    fn eval_closed_in_unknown_graph(
        &self,
        path: &PropertyPath,
        start: &EncodedTerm,
        end: &EncodedTerm,
    ) -> Box<dyn Iterator<Item = Result<EncodedTerm, EvaluationError>>> {
        match path {
            PropertyPath::Path(p) => Box::new(
                self.dataset
                    .encoded_quads_for_pattern(Some(start), Some(p), Some(end), None)
                    .map(|t| Ok(t?.graph_name)),
            ),
            PropertyPath::Reverse(p) => self.eval_closed_in_unknown_graph(p, end, start),
            PropertyPath::Sequence(a, b) => {
                let eval = self.clone();
                let b = Rc::clone(b);
                let end = end.clone();
                Box::new(self.eval_from_in_unknown_graph(a, start).flat_map_ok(
                    move |(middle, graph_name)| {
                        eval.eval_closed_in_graph(&b, &middle, &end, &graph_name)
                            .map(|is_found| is_found.then_some(graph_name))
                            .transpose()
                    },
                ))
            }
            PropertyPath::Alternative(a, b) => Box::new(hash_deduplicate(
                self.eval_closed_in_unknown_graph(a, start, end)
                    .chain(self.eval_closed_in_unknown_graph(b, start, end)),
            )),
            PropertyPath::ZeroOrMore(p) => {
                let eval = self.clone();
                let start2 = start.clone();
                let end = end.clone();
                let p = Rc::clone(p);
                self.run_if_term_is_a_dataset_node(start, move |graph_name| {
                    look_in_transitive_closure(
                        Some(Ok(start2.clone())),
                        |e| eval.eval_from_in_graph(&p, &e, &graph_name),
                        &end,
                    )
                    .map(|is_found| is_found.then_some(graph_name))
                    .transpose()
                })
            }
            PropertyPath::OneOrMore(p) => {
                let eval = self.clone();
                let end = end.clone();
                let p = Rc::clone(p);
                Box::new(
                    self.eval_from_in_unknown_graph(&p, start)
                        .filter_map(move |r| {
                            r.and_then(|(start, graph_name)| {
                                look_in_transitive_closure(
                                    Some(Ok(start)),
                                    |e| eval.eval_from_in_graph(&p, &e, &graph_name),
                                    &end,
                                )
                                .map(|is_found| is_found.then_some(graph_name))
                            })
                            .transpose()
                        }),
                )
            }
            PropertyPath::ZeroOrOne(p) => {
                if start == end {
                    self.run_if_term_is_a_dataset_node(start, |graph_name| Some(Ok(graph_name)))
                } else {
                    let eval = self.clone();
                    let start2 = start.clone();
                    let end = end.clone();
                    let p = Rc::clone(p);
                    self.run_if_term_is_a_dataset_node(start, move |graph_name| {
                        eval.eval_closed_in_graph(&p, &start2, &end, &graph_name)
                            .map(|is_found| is_found.then_some(graph_name))
                            .transpose()
                    })
                }
            }
            PropertyPath::NegatedPropertySet(ps) => {
                let ps = Rc::clone(ps);
                Box::new(
                    self.dataset
                        .encoded_quads_for_pattern(Some(start), None, Some(end), None)
                        .filter_map(move |t| match t {
                            Ok(t) => {
                                if ps.iter().any(|p| *p == t.predicate) {
                                    None
                                } else {
                                    Some(Ok(t.graph_name))
                                }
                            }
                            Err(e) => Some(Err(e)),
                        }),
                )
            }
        }
    }

    fn eval_from_in_graph(
        &self,
        path: &PropertyPath,
        start: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> Box<dyn Iterator<Item = Result<EncodedTerm, EvaluationError>>> {
        match path {
            PropertyPath::Path(p) => Box::new(
                self.dataset
                    .encoded_quads_for_pattern(Some(start), Some(p), None, Some(graph_name))
                    .map(|t| Ok(t?.object)),
            ),
            PropertyPath::Reverse(p) => self.eval_to_in_graph(p, start, graph_name),
            PropertyPath::Sequence(a, b) => {
                let eval = self.clone();
                let b = Rc::clone(b);
                let graph_name2 = graph_name.clone();
                Box::new(
                    self.eval_from_in_graph(a, start, graph_name)
                        .flat_map_ok(move |middle| {
                            eval.eval_from_in_graph(&b, &middle, &graph_name2)
                        }),
                )
            }
            PropertyPath::Alternative(a, b) => Box::new(hash_deduplicate(
                self.eval_from_in_graph(a, start, graph_name)
                    .chain(self.eval_from_in_graph(b, start, graph_name)),
            )),
            PropertyPath::ZeroOrMore(p) => {
                self.run_if_term_is_a_graph_node(start, graph_name, || {
                    let eval = self.clone();
                    let p = Rc::clone(p);
                    let graph_name2 = graph_name.clone();
                    transitive_closure(Some(Ok(start.clone())), move |e| {
                        eval.eval_from_in_graph(&p, &e, &graph_name2)
                    })
                })
            }
            PropertyPath::OneOrMore(p) => {
                let eval = self.clone();
                let p = Rc::clone(p);
                let graph_name2 = graph_name.clone();
                Box::new(transitive_closure(
                    self.eval_from_in_graph(&p, start, graph_name),
                    move |e| eval.eval_from_in_graph(&p, &e, &graph_name2),
                ))
            }
            PropertyPath::ZeroOrOne(p) => {
                self.run_if_term_is_a_graph_node(start, graph_name, || {
                    hash_deduplicate(
                        once(Ok(start.clone()))
                            .chain(self.eval_from_in_graph(p, start, graph_name)),
                    )
                })
            }
            PropertyPath::NegatedPropertySet(ps) => {
                let ps = Rc::clone(ps);
                Box::new(
                    self.dataset
                        .encoded_quads_for_pattern(Some(start), None, None, Some(graph_name))
                        .filter_map(move |t| match t {
                            Ok(t) => {
                                if ps.iter().any(|p| *p == t.predicate) {
                                    None
                                } else {
                                    Some(Ok(t.object))
                                }
                            }
                            Err(e) => Some(Err(e)),
                        }),
                )
            }
        }
    }

    fn eval_from_in_unknown_graph(
        &self,
        path: &PropertyPath,
        start: &EncodedTerm,
    ) -> Box<dyn Iterator<Item = Result<(EncodedTerm, EncodedTerm), EvaluationError>>> {
        match path {
            PropertyPath::Path(p) => Box::new(
                self.dataset
                    .encoded_quads_for_pattern(Some(start), Some(p), None, None)
                    .map(|t| {
                        let t = t?;
                        Ok((t.object, t.graph_name))
                    }),
            ),
            PropertyPath::Reverse(p) => self.eval_to_in_unknown_graph(p, start),
            PropertyPath::Sequence(a, b) => {
                let eval = self.clone();
                let b = Rc::clone(b);
                Box::new(self.eval_from_in_unknown_graph(a, start).flat_map_ok(
                    move |(middle, graph_name)| {
                        eval.eval_from_in_graph(&b, &middle, &graph_name)
                            .map(move |end| Ok((end?, graph_name.clone())))
                    },
                ))
            }
            PropertyPath::Alternative(a, b) => Box::new(hash_deduplicate(
                self.eval_from_in_unknown_graph(a, start)
                    .chain(self.eval_from_in_unknown_graph(b, start)),
            )),
            PropertyPath::ZeroOrMore(p) => {
                let start2 = start.clone();
                let eval = self.clone();
                let p = Rc::clone(p);
                self.run_if_term_is_a_dataset_node(start, move |graph_name| {
                    let eval = eval.clone();
                    let p = Rc::clone(&p);
                    let graph_name2 = graph_name.clone();
                    transitive_closure(Some(Ok(start2.clone())), move |e| {
                        eval.eval_from_in_graph(&p, &e, &graph_name2)
                    })
                    .map(move |e| Ok((e?, graph_name.clone())))
                })
            }
            PropertyPath::OneOrMore(p) => {
                let eval = self.clone();
                let p = Rc::clone(p);
                Box::new(transitive_closure(
                    self.eval_from_in_unknown_graph(&p, start),
                    move |(e, graph_name)| {
                        eval.eval_from_in_graph(&p, &e, &graph_name)
                            .map(move |e| Ok((e?, graph_name.clone())))
                    },
                ))
            }
            PropertyPath::ZeroOrOne(p) => {
                let eval = self.clone();
                let start2 = start.clone();
                let p = Rc::clone(p);
                self.run_if_term_is_a_dataset_node(start, move |graph_name| {
                    hash_deduplicate(once(Ok(start2.clone())).chain(eval.eval_from_in_graph(
                        &p,
                        &start2,
                        &graph_name,
                    )))
                    .map(move |e| Ok((e?, graph_name.clone())))
                })
            }
            PropertyPath::NegatedPropertySet(ps) => {
                let ps = Rc::clone(ps);
                Box::new(
                    self.dataset
                        .encoded_quads_for_pattern(Some(start), None, None, None)
                        .filter_map(move |t| match t {
                            Ok(t) => {
                                if ps.iter().any(|p| *p == t.predicate) {
                                    None
                                } else {
                                    Some(Ok((t.object, t.graph_name)))
                                }
                            }
                            Err(e) => Some(Err(e)),
                        }),
                )
            }
        }
    }

    fn eval_to_in_graph(
        &self,
        path: &PropertyPath,
        end: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> Box<dyn Iterator<Item = Result<EncodedTerm, EvaluationError>>> {
        match path {
            PropertyPath::Path(p) => Box::new(
                self.dataset
                    .encoded_quads_for_pattern(None, Some(p), Some(end), Some(graph_name))
                    .map(|t| Ok(t?.subject)),
            ),
            PropertyPath::Reverse(p) => self.eval_from_in_graph(p, end, graph_name),
            PropertyPath::Sequence(a, b) => {
                let eval = self.clone();
                let a = Rc::clone(a);
                let graph_name2 = graph_name.clone();
                Box::new(
                    self.eval_to_in_graph(b, end, graph_name)
                        .flat_map_ok(move |middle| {
                            eval.eval_to_in_graph(&a, &middle, &graph_name2)
                        }),
                )
            }
            PropertyPath::Alternative(a, b) => Box::new(hash_deduplicate(
                self.eval_to_in_graph(a, end, graph_name)
                    .chain(self.eval_to_in_graph(b, end, graph_name)),
            )),
            PropertyPath::ZeroOrMore(p) => {
                self.run_if_term_is_a_graph_node(end, graph_name, || {
                    let eval = self.clone();
                    let p = Rc::clone(p);
                    let graph_name2 = graph_name.clone();
                    transitive_closure(Some(Ok(end.clone())), move |e| {
                        eval.eval_to_in_graph(&p, &e, &graph_name2)
                    })
                })
            }
            PropertyPath::OneOrMore(p) => {
                let eval = self.clone();
                let p = Rc::clone(p);
                let graph_name2 = graph_name.clone();
                Box::new(transitive_closure(
                    self.eval_to_in_graph(&p, end, graph_name),
                    move |e| eval.eval_to_in_graph(&p, &e, &graph_name2),
                ))
            }
            PropertyPath::ZeroOrOne(p) => self.run_if_term_is_a_graph_node(end, graph_name, || {
                hash_deduplicate(
                    once(Ok(end.clone())).chain(self.eval_to_in_graph(p, end, graph_name)),
                )
            }),
            PropertyPath::NegatedPropertySet(ps) => {
                let ps = Rc::clone(ps);
                Box::new(
                    self.dataset
                        .encoded_quads_for_pattern(None, None, Some(end), Some(graph_name))
                        .filter_map(move |t| match t {
                            Ok(t) => {
                                if ps.iter().any(|p| *p == t.predicate) {
                                    None
                                } else {
                                    Some(Ok(t.subject))
                                }
                            }
                            Err(e) => Some(Err(e)),
                        }),
                )
            }
        }
    }

    fn eval_to_in_unknown_graph(
        &self,
        path: &PropertyPath,
        end: &EncodedTerm,
    ) -> Box<dyn Iterator<Item = Result<(EncodedTerm, EncodedTerm), EvaluationError>>> {
        match path {
            PropertyPath::Path(p) => Box::new(
                self.dataset
                    .encoded_quads_for_pattern(None, Some(p), Some(end), None)
                    .map(|t| {
                        let t = t?;
                        Ok((t.subject, t.graph_name))
                    }),
            ),
            PropertyPath::Reverse(p) => self.eval_from_in_unknown_graph(p, end),
            PropertyPath::Sequence(a, b) => {
                let eval = self.clone();
                let a = Rc::clone(a);
                Box::new(self.eval_to_in_unknown_graph(b, end).flat_map_ok(
                    move |(middle, graph_name)| {
                        eval.eval_from_in_graph(&a, &middle, &graph_name)
                            .map(move |start| Ok((start?, graph_name.clone())))
                    },
                ))
            }
            PropertyPath::Alternative(a, b) => Box::new(hash_deduplicate(
                self.eval_to_in_unknown_graph(a, end)
                    .chain(self.eval_to_in_unknown_graph(b, end)),
            )),
            PropertyPath::ZeroOrMore(p) => {
                let end2 = end.clone();
                let eval = self.clone();
                let p = Rc::clone(p);
                self.run_if_term_is_a_dataset_node(end, move |graph_name| {
                    let eval = eval.clone();
                    let p = Rc::clone(&p);
                    let graph_name2 = graph_name.clone();
                    transitive_closure(Some(Ok(end2.clone())), move |e| {
                        eval.eval_to_in_graph(&p, &e, &graph_name2)
                    })
                    .map(move |e| Ok((e?, graph_name.clone())))
                })
            }
            PropertyPath::OneOrMore(p) => {
                let eval = self.clone();
                let p = Rc::clone(p);
                Box::new(transitive_closure(
                    self.eval_to_in_unknown_graph(&p, end),
                    move |(e, graph_name)| {
                        eval.eval_to_in_graph(&p, &e, &graph_name)
                            .map(move |e| Ok((e?, graph_name.clone())))
                    },
                ))
            }
            PropertyPath::ZeroOrOne(p) => {
                let eval = self.clone();
                let end2 = end.clone();
                let p = Rc::clone(p);
                self.run_if_term_is_a_dataset_node(end, move |graph_name| {
                    hash_deduplicate(once(Ok(end2.clone())).chain(eval.eval_to_in_graph(
                        &p,
                        &end2,
                        &graph_name,
                    )))
                    .map(move |e| Ok((e?, graph_name.clone())))
                })
            }
            PropertyPath::NegatedPropertySet(ps) => {
                let ps = Rc::clone(ps);
                Box::new(
                    self.dataset
                        .encoded_quads_for_pattern(Some(end), None, None, None)
                        .filter_map(move |t| match t {
                            Ok(t) => {
                                if ps.iter().any(|p| *p == t.predicate) {
                                    None
                                } else {
                                    Some(Ok((t.subject, t.graph_name)))
                                }
                            }
                            Err(e) => Some(Err(e)),
                        }),
                )
            }
        }
    }

    fn eval_open_in_graph(
        &self,
        path: &PropertyPath,
        graph_name: &EncodedTerm,
    ) -> Box<dyn Iterator<Item = Result<(EncodedTerm, EncodedTerm), EvaluationError>>> {
        match path {
            PropertyPath::Path(p) => Box::new(
                self.dataset
                    .encoded_quads_for_pattern(None, Some(p), None, Some(graph_name))
                    .map(|t| t.map(|t| (t.subject, t.object))),
            ),
            PropertyPath::Reverse(p) => Box::new(
                self.eval_open_in_graph(p, graph_name)
                    .map(|t| t.map(|(s, o)| (o, s))),
            ),
            PropertyPath::Sequence(a, b) => {
                let eval = self.clone();
                let b = Rc::clone(b);
                let graph_name2 = graph_name.clone();
                Box::new(self.eval_open_in_graph(a, graph_name).flat_map_ok(
                    move |(start, middle)| {
                        eval.eval_from_in_graph(&b, &middle, &graph_name2)
                            .map(move |end| Ok((start.clone(), end?)))
                    },
                ))
            }
            PropertyPath::Alternative(a, b) => Box::new(hash_deduplicate(
                self.eval_open_in_graph(a, graph_name)
                    .chain(self.eval_open_in_graph(b, graph_name)),
            )),
            PropertyPath::ZeroOrMore(p) => {
                let eval = self.clone();
                let p = Rc::clone(p);
                let graph_name2 = graph_name.clone();
                Box::new(transitive_closure(
                    self.get_subject_or_object_identity_pairs_in_graph(graph_name),
                    move |(start, middle)| {
                        eval.eval_from_in_graph(&p, &middle, &graph_name2)
                            .map(move |end| Ok((start.clone(), end?)))
                    },
                ))
            }
            PropertyPath::OneOrMore(p) => {
                let eval = self.clone();
                let p = Rc::clone(p);
                let graph_name2 = graph_name.clone();
                Box::new(transitive_closure(
                    self.eval_open_in_graph(&p, graph_name),
                    move |(start, middle)| {
                        eval.eval_from_in_graph(&p, &middle, &graph_name2)
                            .map(move |end| Ok((start.clone(), end?)))
                    },
                ))
            }
            PropertyPath::ZeroOrOne(p) => Box::new(hash_deduplicate(
                self.get_subject_or_object_identity_pairs_in_graph(graph_name)
                    .chain(self.eval_open_in_graph(p, graph_name)),
            )),
            PropertyPath::NegatedPropertySet(ps) => {
                let ps = Rc::clone(ps);
                Box::new(
                    self.dataset
                        .encoded_quads_for_pattern(None, None, None, Some(graph_name))
                        .filter_map(move |t| match t {
                            Ok(t) => {
                                if ps.iter().any(|p| *p == t.predicate) {
                                    None
                                } else {
                                    Some(Ok((t.subject, t.object)))
                                }
                            }
                            Err(e) => Some(Err(e)),
                        }),
                )
            }
        }
    }

    fn eval_open_in_unknown_graph(
        &self,
        path: &PropertyPath,
    ) -> Box<dyn Iterator<Item = Result<(EncodedTerm, EncodedTerm, EncodedTerm), EvaluationError>>>
    {
        match path {
            PropertyPath::Path(p) => Box::new(
                self.dataset
                    .encoded_quads_for_pattern(None, Some(p), None, None)
                    .map(|t| t.map(|t| (t.subject, t.object, t.graph_name))),
            ),
            PropertyPath::Reverse(p) => Box::new(
                self.eval_open_in_unknown_graph(p)
                    .map(|t| t.map(|(s, o, g)| (o, s, g))),
            ),
            PropertyPath::Sequence(a, b) => {
                let eval = self.clone();
                let b = Rc::clone(b);
                Box::new(self.eval_open_in_unknown_graph(a).flat_map_ok(
                    move |(start, middle, graph_name)| {
                        eval.eval_from_in_graph(&b, &middle, &graph_name)
                            .map(move |end| Ok((start.clone(), end?, graph_name.clone())))
                    },
                ))
            }
            PropertyPath::Alternative(a, b) => Box::new(hash_deduplicate(
                self.eval_open_in_unknown_graph(a)
                    .chain(self.eval_open_in_unknown_graph(b)),
            )),
            PropertyPath::ZeroOrMore(p) => {
                let eval = self.clone();
                let p = Rc::clone(p);
                Box::new(transitive_closure(
                    self.get_subject_or_object_identity_pairs_in_dataset(),
                    move |(start, middle, graph_name)| {
                        eval.eval_from_in_graph(&p, &middle, &graph_name)
                            .map(move |end| Ok((start.clone(), end?, graph_name.clone())))
                    },
                ))
            }
            PropertyPath::OneOrMore(p) => {
                let eval = self.clone();
                let p = Rc::clone(p);
                Box::new(transitive_closure(
                    self.eval_open_in_unknown_graph(&p),
                    move |(start, middle, graph_name)| {
                        eval.eval_from_in_graph(&p, &middle, &graph_name)
                            .map(move |end| Ok((start.clone(), end?, graph_name.clone())))
                    },
                ))
            }
            PropertyPath::ZeroOrOne(p) => Box::new(hash_deduplicate(
                self.get_subject_or_object_identity_pairs_in_dataset()
                    .chain(self.eval_open_in_unknown_graph(p)),
            )),
            PropertyPath::NegatedPropertySet(ps) => {
                let ps = Rc::clone(ps);
                Box::new(
                    self.dataset
                        .encoded_quads_for_pattern(None, None, None, None)
                        .filter_map(move |t| match t {
                            Ok(t) => {
                                if ps.iter().any(|p| *p == t.predicate) {
                                    None
                                } else {
                                    Some(Ok((t.subject, t.object, t.graph_name)))
                                }
                            }
                            Err(e) => Some(Err(e)),
                        }),
                )
            }
        }
    }

    fn get_subject_or_object_identity_pairs_in_graph(
        &self,
        graph_name: &EncodedTerm,
    ) -> impl Iterator<Item = Result<(EncodedTerm, EncodedTerm), EvaluationError>> {
        self.dataset
            .encoded_quads_for_pattern(None, None, None, Some(graph_name))
            .flat_map_ok(|t| {
                [
                    Ok((t.subject.clone(), t.subject)),
                    Ok((t.object.clone(), t.object)),
                ]
            })
    }

    fn get_subject_or_object_identity_pairs_in_dataset(
        &self,
    ) -> impl Iterator<Item = Result<(EncodedTerm, EncodedTerm, EncodedTerm), EvaluationError>>
    {
        self.dataset
            .encoded_quads_for_pattern(None, None, None, None)
            .flat_map_ok(|t| {
                [
                    Ok((t.subject.clone(), t.subject, t.graph_name.clone())),
                    Ok((t.object.clone(), t.object, t.graph_name)),
                ]
            })
    }

    fn run_if_term_is_a_graph_node<
        T: 'static,
        I: Iterator<Item = Result<T, EvaluationError>> + 'static,
    >(
        &self,
        term: &EncodedTerm,
        graph_name: &EncodedTerm,
        f: impl FnOnce() -> I,
    ) -> Box<dyn Iterator<Item = Result<T, EvaluationError>>> {
        match self.is_subject_or_object_in_graph(term, graph_name) {
            Ok(true) => Box::new(f()),
            Ok(false) => {
                Box::new(empty()) // Not in the database
            }
            Err(error) => Box::new(once(Err(error))),
        }
    }

    fn is_subject_or_object_in_graph(
        &self,
        term: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> Result<bool, EvaluationError> {
        Ok(self
            .dataset
            .encoded_quads_for_pattern(Some(term), None, None, Some(graph_name))
            .next()
            .transpose()?
            .is_some()
            || self
                .dataset
                .encoded_quads_for_pattern(None, None, Some(term), Some(graph_name))
                .next()
                .transpose()?
                .is_some())
    }

    fn run_if_term_is_a_dataset_node<
        T: 'static,
        I: IntoIterator<Item = Result<T, EvaluationError>> + 'static,
    >(
        &self,
        term: &EncodedTerm,
        f: impl FnMut(EncodedTerm) -> I + 'static,
    ) -> Box<dyn Iterator<Item = Result<T, EvaluationError>>> {
        match self
            .find_graphs_where_the_node_is_in(term)
            .collect::<Result<HashSet<_>, _>>()
        {
            Ok(graph_names) => Box::new(graph_names.into_iter().flat_map(f)),
            Err(error) => Box::new(once(Err(error))),
        }
    }

    fn find_graphs_where_the_node_is_in(
        &self,
        term: &EncodedTerm,
    ) -> impl Iterator<Item = Result<EncodedTerm, EvaluationError>> {
        self.dataset
            .encoded_quads_for_pattern(Some(term), None, None, None)
            .chain(
                self.dataset
                    .encoded_quads_for_pattern(None, None, Some(term), None),
            )
            .map(|q| Ok(q?.graph_name))
    }
}

struct CartesianProductJoinIterator {
    probe_iter: EncodedTuplesIterator,
    built: Vec<EncodedTuple>,
    buffered_results: Vec<Result<EncodedTuple, EvaluationError>>,
}

impl Iterator for CartesianProductJoinIterator {
    type Item = Result<EncodedTuple, EvaluationError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(result) = self.buffered_results.pop() {
                return Some(result);
            }
            let probe_tuple = match self.probe_iter.next()? {
                Ok(probe_tuple) => probe_tuple,
                Err(error) => return Some(Err(error)),
            };
            for built_tuple in &self.built {
                if let Some(result_tuple) = probe_tuple.combine_with(built_tuple) {
                    self.buffered_results.push(Ok(result_tuple))
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (min, max) = self.probe_iter.size_hint();
        (
            min.saturating_mul(self.built.len()),
            max.map(|v| v.saturating_mul(self.built.len())),
        )
    }
}

struct HashJoinIterator {
    probe_iter: EncodedTuplesIterator,
    built: EncodedTupleSet,
    buffered_results: Vec<Result<EncodedTuple, EvaluationError>>,
}

impl Iterator for HashJoinIterator {
    type Item = Result<EncodedTuple, EvaluationError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(result) = self.buffered_results.pop() {
                return Some(result);
            }
            let probe_tuple = match self.probe_iter.next()? {
                Ok(probe_tuple) => probe_tuple,
                Err(error) => return Some(Err(error)),
            };
            self.buffered_results.extend(
                self.built
                    .get(&probe_tuple)
                    .iter()
                    .filter_map(|built_tuple| probe_tuple.combine_with(built_tuple).map(Ok)),
            )
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            0,
            self.probe_iter
                .size_hint()
                .1
                .map(|v| v.saturating_mul(self.built.len())),
        )
    }
}

struct HashLeftJoinIterator {
    left_iter: EncodedTuplesIterator,
    right: EncodedTupleSet,
    buffered_results: Vec<Result<EncodedTuple, EvaluationError>>,
    expression: Rc<dyn Fn(&EncodedTuple) -> Option<EncodedTerm>>,
}

impl Iterator for HashLeftJoinIterator {
    type Item = Result<EncodedTuple, EvaluationError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(result) = self.buffered_results.pop() {
                return Some(result);
            }
            let left_tuple = match self.left_iter.next()? {
                Ok(left_tuple) => left_tuple,
                Err(error) => return Some(Err(error)),
            };
            self.buffered_results.extend(
                self.right
                    .get(&left_tuple)
                    .iter()
                    .filter_map(|right_tuple| left_tuple.combine_with(right_tuple))
                    .filter(|tuple| {
                        (self.expression)(tuple)
                            .and_then(|term| to_bool(&term))
                            .unwrap_or(false)
                    })
                    .map(Ok),
            );
            if self.buffered_results.is_empty() {
                // We have not manage to join with anything
                return Some(Ok(left_tuple));
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            0,
            self.left_iter
                .size_hint()
                .1
                .map(|v| v.saturating_mul(self.right.len())),
        )
    }
}

struct ForLoopLeftJoinIterator {
    right_evaluator: Rc<dyn Fn(EncodedTuple) -> EncodedTuplesIterator>,
    left_iter: EncodedTuplesIterator,
    current_right: EncodedTuplesIterator,
}

impl Iterator for ForLoopLeftJoinIterator {
    type Item = Result<EncodedTuple, EvaluationError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(tuple) = self.current_right.next() {
            return Some(tuple);
        }
        let left_tuple = match self.left_iter.next()? {
            Ok(left_tuple) => left_tuple,
            Err(error) => return Some(Err(error)),
        };
        self.current_right = (self.right_evaluator)(left_tuple.clone());
        if let Some(right_tuple) = self.current_right.next() {
            Some(right_tuple)
        } else {
            Some(Ok(left_tuple))
        }
    }
}

struct UnionIterator {
    plans: Vec<Rc<dyn Fn(EncodedTuple) -> EncodedTuplesIterator>>,
    input: EncodedTuple,
    current_iterator: EncodedTuplesIterator,
    current_plan: usize,
}

impl Iterator for UnionIterator {
    type Item = Result<EncodedTuple, EvaluationError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(tuple) = self.current_iterator.next() {
                return Some(tuple);
            }
            if self.current_plan >= self.plans.len() {
                return None;
            }
            self.current_iterator = self.plans[self.current_plan](self.input.clone());
            self.current_plan += 1;
        }
    }
}

struct ConsecutiveDeduplication {
    inner: EncodedTuplesIterator,
    current: Option<EncodedTuple>,
}

impl Iterator for ConsecutiveDeduplication {
    type Item = Result<EncodedTuple, EvaluationError>;

    fn next(&mut self) -> Option<Self::Item> {
        // Basic idea. We buffer the previous result and we only emit it when we know the next one or it's the end
        loop {
            if let Some(next) = self.inner.next() {
                match next {
                    Ok(next) => match self.current.take() {
                        Some(current) if current != next => {
                            // We found a relevant value
                            self.current = Some(next);
                            return Some(Ok(current));
                        }
                        _ => {
                            //  We discard the value and move to the next one
                            self.current = Some(next);
                        }
                    },
                    Err(error) => return Some(Err(error)), // We swap but it's fine. It's an error.
                }
            } else {
                return self.current.take().map(Ok);
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (min, max) = self.inner.size_hint();
        ((min != 0).into(), max)
    }
}

struct ConstructIterator {
    eval: SimpleEvaluator,
    iter: EncodedTuplesIterator,
    template: Vec<TripleTemplate>,
    buffered_results: Vec<Result<Triple, EvaluationError>>,
    bnodes: Vec<EncodedTerm>,
}

impl Iterator for ConstructIterator {
    type Item = Result<Triple, EvaluationError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(result) = self.buffered_results.pop() {
                return Some(result);
            }
            {
                let tuple = match self.iter.next()? {
                    Ok(tuple) => tuple,
                    Err(error) => return Some(Err(error)),
                };
                for template in &self.template {
                    if let (Some(subject), Some(predicate), Some(object)) = (
                        get_triple_template_value(&template.subject, &tuple, &mut self.bnodes),
                        get_triple_template_value(&template.predicate, &tuple, &mut self.bnodes),
                        get_triple_template_value(&template.object, &tuple, &mut self.bnodes),
                    ) {
                        self.buffered_results.push(decode_triple(
                            &*self.eval.dataset,
                            &subject,
                            &predicate,
                            &object,
                        ));
                    }
                }
                self.bnodes.clear(); // We do not reuse old bnodes
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (min, max) = self.iter.size_hint();
        (
            min.saturating_mul(self.template.len()),
            max.map(|v| v.saturating_mul(self.template.len())),
        )
    }
}

pub struct TripleTemplate {
    pub subject: TripleTemplateValue,
    pub predicate: TripleTemplateValue,
    pub object: TripleTemplateValue,
}

pub enum TripleTemplateValue {
    Constant(EncodedTerm),
    BlankNode(usize),
    Variable(usize),
    Triple(Box<TripleTemplate>),
}

fn get_triple_template_value<'a>(
    selector: &'a TripleTemplateValue,
    tuple: &'a EncodedTuple,
    bnodes: &'a mut Vec<EncodedTerm>,
) -> Option<EncodedTerm> {
    match selector {
        TripleTemplateValue::Constant(term) => Some(term.clone()),
        TripleTemplateValue::Variable(v) => tuple.get(*v).cloned(),
        TripleTemplateValue::BlankNode(bnode) => {
            if *bnode >= bnodes.len() {
                bnodes.resize_with(*bnode + 1, new_bnode)
            }
            Some(bnodes[*bnode].clone())
        }
        TripleTemplateValue::Triple(triple) => Some(
            EncodedTriple {
                subject: get_triple_template_value(&triple.subject, tuple, bnodes)?,
                predicate: get_triple_template_value(&triple.predicate, tuple, bnodes)?,
                object: get_triple_template_value(&triple.object, tuple, bnodes)?,
            }
            .into(),
        ),
    }
}

fn new_bnode() -> EncodedTerm {
    EncodedTerm::NumericalBlankNode { id: random() }
}

fn decode_triple<D: Decoder>(
    decoder: &D,
    subject: &EncodedTerm,
    predicate: &EncodedTerm,
    object: &EncodedTerm,
) -> Result<Triple, EvaluationError> {
    Ok(Triple::new(
        decoder.decode_subject(subject)?,
        decoder.decode_named_node(predicate)?,
        decoder.decode_term(object)?,
    ))
}

struct DescribeIterator {
    eval: SimpleEvaluator,
    iter: EncodedTuplesIterator,
    quads: Box<dyn Iterator<Item = Result<EncodedQuad, EvaluationError>>>,
}

impl Iterator for DescribeIterator {
    type Item = Result<Triple, EvaluationError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(quad) = self.quads.next() {
                return Some(match quad {
                    Ok(quad) => self
                        .eval
                        .dataset
                        .decode_quad(&quad)
                        .map(Into::into)
                        .map_err(Into::into),
                    Err(error) => Err(error),
                });
            }
            let tuple = match self.iter.next()? {
                Ok(tuple) => tuple,
                Err(error) => return Some(Err(error)),
            };
            let eval = self.eval.clone();
            self.quads = Box::new(tuple.into_iter().flatten().flat_map(move |subject| {
                eval.dataset
                    .encoded_quads_for_pattern(
                        Some(&subject),
                        None,
                        None,
                        Some(&EncodedTerm::DefaultGraph),
                    )
                    .chain(
                        eval.dataset
                            .encoded_quads_for_pattern(Some(&subject), None, None, None),
                    )
            }));
        }
    }
}

struct ZipLongest<T1, T2, I1: Iterator<Item = T1>, I2: Iterator<Item = T2>> {
    a: I1,
    b: I2,
}

impl<T1, T2, I1: Iterator<Item = T1>, I2: Iterator<Item = T2>> ZipLongest<T1, T2, I1, I2> {
    fn new(a: I1, b: I2) -> Self {
        Self { a, b }
    }
}

impl<T1, T2, I1: Iterator<Item = T1>, I2: Iterator<Item = T2>> Iterator
    for ZipLongest<T1, T2, I1, I2>
{
    type Item = (Option<T1>, Option<T2>);

    fn next(&mut self) -> Option<Self::Item> {
        match (self.a.next(), self.b.next()) {
            (None, None) => None,
            r => Some(r),
        }
    }
}

fn transitive_closure<T: Clone + Eq + Hash, NI: Iterator<Item = Result<T, EvaluationError>>>(
    start: impl IntoIterator<Item = Result<T, EvaluationError>>,
    mut next: impl FnMut(T) -> NI,
) -> impl Iterator<Item = Result<T, EvaluationError>> {
    let mut errors = Vec::new();
    let mut todo = start
        .into_iter()
        .filter_map(|e| match e {
            Ok(e) => Some(e),
            Err(e) => {
                errors.push(e);
                None
            }
        })
        .collect::<Vec<_>>();
    let mut all = todo.iter().cloned().collect::<HashSet<_>>();
    while let Some(e) = todo.pop() {
        for e in next(e) {
            match e {
                Ok(e) => {
                    if all.insert(e.clone()) {
                        todo.push(e)
                    }
                }
                Err(e) => errors.push(e),
            }
        }
    }
    errors.into_iter().map(Err).chain(all.into_iter().map(Ok))
}

fn look_in_transitive_closure<
    T: Clone + Eq + Hash,
    NI: Iterator<Item = Result<T, EvaluationError>>,
>(
    start: impl IntoIterator<Item = Result<T, EvaluationError>>,
    mut next: impl FnMut(T) -> NI,
    target: &T,
) -> Result<bool, EvaluationError> {
    let mut todo = start.into_iter().collect::<Result<Vec<_>, _>>()?;
    let mut all = todo.iter().cloned().collect::<HashSet<_>>();
    while let Some(e) = todo.pop() {
        if e == *target {
            return Ok(true);
        }
        for e in next(e) {
            let e = e?;
            if all.insert(e.clone()) {
                todo.push(e);
            }
        }
    }
    Ok(false)
}

fn hash_deduplicate<T: Eq + Hash + Clone>(
    iter: impl Iterator<Item = Result<T, EvaluationError>>,
) -> impl Iterator<Item = Result<T, EvaluationError>> {
    let mut already_seen = HashSet::with_capacity(iter.size_hint().0);
    iter.filter(move |e| {
        if let Ok(e) = e {
            if already_seen.contains(e) {
                false
            } else {
                already_seen.insert(e.clone());
                true
            }
        } else {
            true
        }
    })
}

trait ResultIterator<T>: Iterator<Item = Result<T, EvaluationError>> + Sized {
    fn flat_map_ok<O, F: FnMut(T) -> U, U: IntoIterator<Item = Result<O, EvaluationError>>>(
        self,
        f: F,
    ) -> FlatMapOk<T, O, Self, F, U>;
}

impl<T, I: Iterator<Item = Result<T, EvaluationError>> + Sized> ResultIterator<T> for I {
    fn flat_map_ok<O, F: FnMut(T) -> U, U: IntoIterator<Item = Result<O, EvaluationError>>>(
        self,
        f: F,
    ) -> FlatMapOk<T, O, Self, F, U> {
        FlatMapOk {
            inner: self,
            f,
            current: None,
        }
    }
}

struct FlatMapOk<
    T,
    O,
    I: Iterator<Item = Result<T, EvaluationError>>,
    F: FnMut(T) -> U,
    U: IntoIterator<Item = Result<O, EvaluationError>>,
> {
    inner: I,
    f: F,
    current: Option<U::IntoIter>,
}

impl<
        T,
        O,
        I: Iterator<Item = Result<T, EvaluationError>>,
        F: FnMut(T) -> U,
        U: IntoIterator<Item = Result<O, EvaluationError>>,
    > Iterator for FlatMapOk<T, O, I, F, U>
{
    type Item = Result<O, EvaluationError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(current) = &mut self.current {
                if let Some(next) = current.next() {
                    return Some(next);
                }
            }
            self.current = None;
            match self.inner.next()? {
                Ok(e) => self.current = Some((self.f)(e).into_iter()),
                Err(error) => return Some(Err(error)),
            }
        }
    }
}

trait Accumulator {
    fn add(&mut self, element: Option<EncodedTerm>);

    fn state(&self) -> Option<EncodedTerm>;
}

struct Deduplicate {
    seen: HashSet<Option<EncodedTerm>>,
    inner: Box<dyn Accumulator>,
}

impl Deduplicate {
    fn new(inner: Box<dyn Accumulator>) -> Self {
        Self {
            seen: HashSet::default(),
            inner,
        }
    }
}

impl Accumulator for Deduplicate {
    fn add(&mut self, element: Option<EncodedTerm>) {
        if self.seen.insert(element.clone()) {
            self.inner.add(element)
        }
    }

    fn state(&self) -> Option<EncodedTerm> {
        self.inner.state()
    }
}

#[derive(Default, Debug)]
struct CountAccumulator {
    count: i64,
}

impl Accumulator for CountAccumulator {
    fn add(&mut self, _element: Option<EncodedTerm>) {
        self.count += 1;
    }

    fn state(&self) -> Option<EncodedTerm> {
        Some(self.count.into())
    }
}

struct SumAccumulator {
    sum: Option<EncodedTerm>,
}

impl Default for SumAccumulator {
    fn default() -> Self {
        Self {
            sum: Some(0.into()),
        }
    }
}

impl Accumulator for SumAccumulator {
    fn add(&mut self, element: Option<EncodedTerm>) {
        if let Some(sum) = &self.sum {
            if let Some(operands) = element.and_then(|e| NumericBinaryOperands::new(sum.clone(), e))
            {
                // TODO: unify with addition?
                self.sum = match operands {
                    NumericBinaryOperands::Float(v1, v2) => Some((v1 + v2).into()),
                    NumericBinaryOperands::Double(v1, v2) => Some((v1 + v2).into()),
                    NumericBinaryOperands::Integer(v1, v2) => v1.checked_add(v2).map(Into::into),
                    NumericBinaryOperands::Decimal(v1, v2) => v1.checked_add(v2).map(Into::into),
                    NumericBinaryOperands::Duration(v1, v2) => v1.checked_add(v2).map(Into::into),
                    NumericBinaryOperands::YearMonthDuration(v1, v2) => {
                        v1.checked_add(v2).map(Into::into)
                    }
                    NumericBinaryOperands::DayTimeDuration(v1, v2) => {
                        v1.checked_add(v2).map(Into::into)
                    }
                    _ => None,
                };
            } else {
                self.sum = None;
            }
        }
    }

    fn state(&self) -> Option<EncodedTerm> {
        self.sum.clone()
    }
}

#[derive(Default)]
struct AvgAccumulator {
    sum: SumAccumulator,
    count: i64,
}

impl Accumulator for AvgAccumulator {
    fn add(&mut self, element: Option<EncodedTerm>) {
        self.sum.add(element);
        self.count += 1;
    }

    fn state(&self) -> Option<EncodedTerm> {
        let sum = self.sum.state()?;
        if self.count == 0 {
            Some(0.into())
        } else {
            // TODO: deduplicate?
            // TODO: duration?
            let count = Integer::from(self.count);
            match sum {
                EncodedTerm::FloatLiteral(sum) => Some((sum / Float::from(count)).into()),
                EncodedTerm::DoubleLiteral(sum) => Some((sum / Double::from(count)).into()),
                EncodedTerm::IntegerLiteral(sum) => {
                    Some(Decimal::from(sum).checked_div(count)?.into())
                }
                EncodedTerm::DecimalLiteral(sum) => Some(sum.checked_div(count)?.into()),
                _ => None,
            }
        }
    }
}

#[allow(clippy::option_option)]
struct MinAccumulator {
    dataset: Rc<DatasetView>,
    min: Option<Option<EncodedTerm>>,
}

impl MinAccumulator {
    fn new(dataset: Rc<DatasetView>) -> Self {
        Self { dataset, min: None }
    }
}

impl Accumulator for MinAccumulator {
    fn add(&mut self, element: Option<EncodedTerm>) {
        if let Some(min) = &self.min {
            if cmp_terms(&self.dataset, element.as_ref(), min.as_ref()) == Ordering::Less {
                self.min = Some(element)
            }
        } else {
            self.min = Some(element)
        }
    }

    fn state(&self) -> Option<EncodedTerm> {
        self.min.clone().and_then(|v| v)
    }
}

#[allow(clippy::option_option)]
struct MaxAccumulator {
    dataset: Rc<DatasetView>,
    max: Option<Option<EncodedTerm>>,
}

impl MaxAccumulator {
    fn new(dataset: Rc<DatasetView>) -> Self {
        Self { dataset, max: None }
    }
}

impl Accumulator for MaxAccumulator {
    fn add(&mut self, element: Option<EncodedTerm>) {
        if let Some(max) = &self.max {
            if cmp_terms(&self.dataset, element.as_ref(), max.as_ref()) == Ordering::Greater {
                self.max = Some(element)
            }
        } else {
            self.max = Some(element)
        }
    }

    fn state(&self) -> Option<EncodedTerm> {
        self.max.clone().and_then(|v| v)
    }
}

#[derive(Default)]
struct SampleAccumulator {
    value: Option<EncodedTerm>,
}

impl Accumulator for SampleAccumulator {
    fn add(&mut self, element: Option<EncodedTerm>) {
        if element.is_some() {
            self.value = element
        }
    }

    fn state(&self) -> Option<EncodedTerm> {
        self.value.clone()
    }
}

#[allow(clippy::option_option)]
struct GroupConcatAccumulator {
    dataset: Rc<DatasetView>,
    concat: Option<String>,
    language: Option<Option<SmallStringOrId>>,
    separator: Rc<str>,
}

impl GroupConcatAccumulator {
    fn new(dataset: Rc<DatasetView>, separator: Rc<str>) -> Self {
        Self {
            dataset,
            concat: Some(String::new()),
            language: None,
            separator,
        }
    }
}

impl Accumulator for GroupConcatAccumulator {
    fn add(&mut self, element: Option<EncodedTerm>) {
        if let Some(concat) = self.concat.as_mut() {
            if let Some(element) = element {
                if let Some((value, e_language)) = to_string_and_language(&self.dataset, &element) {
                    if let Some(lang) = self.language {
                        if lang != e_language {
                            self.language = Some(None)
                        }
                        concat.push_str(&self.separator);
                    } else {
                        self.language = Some(e_language)
                    }
                    concat.push_str(&value);
                }
            }
        }
    }

    fn state(&self) -> Option<EncodedTerm> {
        self.concat
            .as_ref()
            .map(|result| build_plain_literal(&self.dataset, result, self.language.and_then(|v| v)))
    }
}

struct FailingAccumulator;

impl Accumulator for FailingAccumulator {
    fn add(&mut self, _: Option<EncodedTerm>) {}

    fn state(&self) -> Option<EncodedTerm> {
        None
    }
}

fn encode_variable(variables: &mut Vec<Variable>, variable: &Variable) -> usize {
    if let Some(key) = slice_key(variables, variable) {
        key
    } else {
        variables.push(variable.clone());
        variables.len() - 1
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

fn generate_uuid(buffer: &mut String) {
    let mut uuid = random::<u128>().to_le_bytes();
    uuid[6] = (uuid[6] & 0x0F) | 0x40;
    uuid[8] = (uuid[8] & 0x3F) | 0x80;

    write_hexa_bytes(&uuid[0..4], buffer);
    buffer.push('-');
    write_hexa_bytes(&uuid[4..6], buffer);
    buffer.push('-');
    write_hexa_bytes(&uuid[6..8], buffer);
    buffer.push('-');
    write_hexa_bytes(&uuid[8..10], buffer);
    buffer.push('-');
    write_hexa_bytes(&uuid[10..16], buffer);
}

fn write_hexa_bytes(bytes: &[u8], buffer: &mut String) {
    for b in bytes {
        let high = b / 16;
        buffer.push(char::from(if high < 10 {
            b'0' + high
        } else {
            b'a' + (high - 10)
        }));
        let low = b % 16;
        buffer.push(char::from(if low < 10 {
            b'0' + low
        } else {
            b'a' + (low - 10)
        }));
    }
}

#[derive(Eq, PartialEq, Clone, Copy)]
enum SmallStringOrId {
    Small(SmallString),
    Big(StrHash),
}

impl From<SmallString> for SmallStringOrId {
    fn from(value: SmallString) -> Self {
        Self::Small(value)
    }
}

impl From<StrHash> for SmallStringOrId {
    fn from(value: StrHash) -> Self {
        Self::Big(value)
    }
}

pub enum ComparatorFunction {
    Asc(Rc<dyn Fn(&EncodedTuple) -> Option<EncodedTerm>>),
    Desc(Rc<dyn Fn(&EncodedTuple) -> Option<EncodedTerm>>),
}

struct EncodedTupleSet {
    key: Vec<usize>,
    map: HashMap<u64, Vec<EncodedTuple>>,
    len: usize,
}

impl EncodedTupleSet {
    fn new(key: Vec<usize>) -> Self {
        Self {
            key,
            map: HashMap::new(),
            len: 0,
        }
    }

    fn insert(&mut self, tuple: EncodedTuple) {
        self.map
            .entry(self.tuple_key(&tuple))
            .or_default()
            .push(tuple);
        self.len += 1;
    }

    fn get(&self, tuple: &EncodedTuple) -> &[EncodedTuple] {
        self.map.get(&self.tuple_key(tuple)).map_or(&[], |v| v)
    }

    fn tuple_key(&self, tuple: &EncodedTuple) -> u64 {
        let mut hasher = DefaultHasher::default();
        for v in &self.key {
            if let Some(val) = tuple.get(*v) {
                val.hash(&mut hasher);
            }
        }
        hasher.finish()
    }

    fn len(&self) -> usize {
        self.len
    }
}

impl Extend<EncodedTuple> for EncodedTupleSet {
    fn extend<T: IntoIterator<Item = EncodedTuple>>(&mut self, iter: T) {
        let iter = iter.into_iter();
        self.map.reserve(iter.size_hint().0);
        for tuple in iter {
            self.insert(tuple);
        }
    }
}

struct StatsIterator {
    inner: EncodedTuplesIterator,
    stats: Rc<EvalNodeWithStats>,
}

impl Iterator for StatsIterator {
    type Item = Result<EncodedTuple, EvaluationError>;

    fn next(&mut self) -> Option<Self::Item> {
        let start = Timer::now();
        let result = self.inner.next();
        self.stats.exec_duration.set(
            self.stats
                .exec_duration
                .get()
                .and_then(|stat| stat.checked_add(start.elapsed()?)),
        );
        if matches!(result, Some(Ok(_))) {
            self.stats.exec_count.set(self.stats.exec_count.get() + 1);
        }
        result
    }
}

pub struct EvalNodeWithStats {
    pub label: String,
    pub children: Vec<Rc<EvalNodeWithStats>>,
    pub exec_count: Cell<usize>,
    pub exec_duration: Cell<Option<DayTimeDuration>>,
}

impl EvalNodeWithStats {
    pub fn json_node(
        &self,
        writer: &mut ToWriteJsonWriter<impl io::Write>,
        with_stats: bool,
    ) -> io::Result<()> {
        writer.write_event(JsonEvent::StartObject)?;
        writer.write_event(JsonEvent::ObjectKey("name".into()))?;
        writer.write_event(JsonEvent::String((&self.label).into()))?;
        if with_stats {
            writer.write_event(JsonEvent::ObjectKey("number of results".into()))?;
            writer.write_event(JsonEvent::Number(self.exec_count.get().to_string().into()))?;
            if let Some(duration) = self.exec_duration.get() {
                writer.write_event(JsonEvent::ObjectKey("duration in seconds".into()))?;
                writer.write_event(JsonEvent::Number(duration.as_seconds().to_string().into()))?;
            }
        }
        writer.write_event(JsonEvent::ObjectKey("children".into()))?;
        writer.write_event(JsonEvent::StartArray)?;
        for child in &self.children {
            child.json_node(writer, with_stats)?;
        }
        writer.write_event(JsonEvent::EndArray)?;
        writer.write_event(JsonEvent::EndObject)
    }
}

impl fmt::Debug for EvalNodeWithStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut obj = f.debug_struct("Node");
        obj.field("name", &self.label);
        if let Some(exec_duration) = self.exec_duration.get() {
            obj.field("number of results", &self.exec_count.get());
            obj.field(
                "duration in seconds",
                &f32::from(Float::from(exec_duration.as_seconds())),
            );
        }
        if !self.children.is_empty() {
            obj.field("children", &self.children);
        }
        obj.finish()
    }
}

fn eval_node_label(node: &GraphPattern) -> String {
    match node {
        GraphPattern::Distinct { .. } => "Distinct(Hash)".to_owned(),
        GraphPattern::Extend {
            expression,
            variable,
            ..
        } => format!(
            "Extend({} -> {variable})",
            spargebra::algebra::Expression::from(expression)
        ),
        GraphPattern::Filter { expression, .. } => format!(
            "Filter({})",
            spargebra::algebra::Expression::from(expression)
        ),
        GraphPattern::Group {
            variables,
            aggregates,
            ..
        } => {
            format!(
                "Aggregate({})",
                format_list(variables.iter().map(ToString::to_string).chain(
                    aggregates.iter().map(|(v, agg)| format!(
                        "{} -> {v}",
                        spargebra::algebra::AggregateExpression::from(agg)
                    ))
                ))
            )
        }
        GraphPattern::Join { algorithm, .. } => match algorithm {
            JoinAlgorithm::HashBuildLeftProbeRight { keys } => format!(
                "LeftJoin(HashBuildLeftProbeRight, keys = {})",
                format_list(keys)
            ),
        },
        GraphPattern::Lateral { right, .. } => {
            if let GraphPattern::LeftJoin {
                left: nested_left,
                expression,
                ..
            } = right.as_ref()
            {
                if nested_left.is_empty_singleton() {
                    // We are in a ForLoopLeftJoin
                    return format!(
                        "ForLoopLeftJoin(expression = {})",
                        spargebra::algebra::Expression::from(expression)
                    );
                }
            }
            "Lateral".to_owned()
        }
        GraphPattern::LeftJoin {
            algorithm,
            expression,
            ..
        } => match algorithm {
            LeftJoinAlgorithm::HashBuildRightProbeLeft { keys } => format!(
                "LeftJoin(HashBuildRightProbeLeft, keys = {}, expression = {})",
                format_list(keys),
                spargebra::algebra::Expression::from(expression)
            ),
        },
        GraphPattern::Minus { algorithm, .. } => match algorithm {
            MinusAlgorithm::HashBuildRightProbeLeft { keys } => format!(
                "AntiJoin(HashBuildRightProbeLeft, keys = {})",
                format_list(keys)
            ),
        },
        GraphPattern::OrderBy { expression, .. } => {
            format!(
                "Sort({})",
                format_list(
                    expression
                        .iter()
                        .map(spargebra::algebra::OrderExpression::from)
                )
            )
        }
        GraphPattern::Path {
            subject,
            path,
            object,
            graph_name,
        } => {
            if let Some(graph_name) = graph_name {
                format!("Path({subject} {path} {object} {graph_name})")
            } else {
                format!("Path({subject} {path} {object})")
            }
        }
        GraphPattern::Project { variables, .. } => {
            format!("Project({})", format_list(variables))
        }
        GraphPattern::QuadPattern {
            subject,
            predicate,
            object,
            graph_name,
        } => {
            if let Some(graph_name) = graph_name {
                format!("QuadPattern({subject} {predicate} {object} {graph_name})")
            } else {
                format!("QuadPattern({subject} {predicate} {object})")
            }
        }
        GraphPattern::Reduced { .. } => "Reduced".to_owned(),
        GraphPattern::Service { name, silent, .. } => {
            if *silent {
                format!("Service({name}, Silent)")
            } else {
                format!("Service({name})")
            }
        }
        GraphPattern::Slice { start, length, .. } => {
            if let Some(length) = length {
                format!("Slice(start = {start}, length = {length})")
            } else {
                format!("Slice(start = {start})")
            }
        }
        GraphPattern::Union { .. } => "Union".to_owned(),
        GraphPattern::Values { variables, .. } => {
            format!("StaticBindings({})", format_list(variables))
        }
    }
}

fn format_list<T: ToString>(values: impl IntoIterator<Item = T>) -> String {
    values
        .into_iter()
        .map(|v| v.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

pub struct Timer {
    start: DateTime,
}

impl Timer {
    pub fn now() -> Self {
        Self {
            start: DateTime::now(),
        }
    }

    pub fn elapsed(&self) -> Option<DayTimeDuration> {
        DateTime::now().checked_sub(self.start)
    }
}

#[cfg(test)]
#[allow(clippy::panic_in_result_fn)]
mod tests {
    use super::*;

    #[test]
    fn uuid() {
        let mut buffer = String::default();
        generate_uuid(&mut buffer);
        assert!(
            Regex::new("^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")
                .unwrap()
                .is_match(&buffer),
            "{buffer} is not a valid UUID"
        );
    }
}
