#[cfg(feature = "rdf-star")]
use crate::dataset::{ExpressionSubject, ExpressionTriple};
use crate::dataset::{ExpressionTerm, InternalQuad, QueryableDataset};
use crate::error::QueryEvaluationError;
use crate::model::{QuerySolutionIter, QueryTripleIter};
use crate::service::ServiceHandlerRegistry;
use crate::CustomFunctionRegistry;
use json_event_parser::{JsonEvent, ToWriteJsonWriter};
use md5::{Digest, Md5};
use oxiri::Iri;
use oxrdf::vocab::{rdf, xsd};
#[cfg(feature = "sparql-12")]
use oxrdf::BaseDirection;
use oxrdf::{BlankNode, Literal, NamedNode, Term, Triple, Variable};
#[cfg(feature = "sep-0002")]
use oxsdatatypes::{Date, Duration, Time, TimezoneOffset, YearMonthDuration};
use oxsdatatypes::{DateTime, DayTimeDuration, Decimal, Double, Float, Integer};
use rand::random;
use regex::{Regex, RegexBuilder};
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet, FxHasher};
use sha1::Sha1;
use sha2::{Sha256, Sha384, Sha512};
use spargebra::algebra::{AggregateFunction, Function, PropertyPathExpression};
#[cfg(feature = "rdf-star")]
use spargebra::term::GroundTriple;
use spargebra::term::{
    GroundTerm, GroundTermPattern, NamedNodePattern, TermPattern, TriplePattern,
};
use sparopt::algebra::{
    AggregateExpression, Expression, GraphPattern, JoinAlgorithm, LeftJoinAlgorithm,
    MinusAlgorithm, OrderExpression,
};
use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::iter::{empty, once, Peekable};
use std::rc::Rc;
use std::sync::Arc;
use std::{fmt, io};
// TODO: make expression raise error when relevant (storage I/O)

const REGEX_SIZE_LIMIT: usize = 1_000_000;

/// Wrapper on top of [`QueryableDataset`]
struct EvalDataset<D: QueryableDataset> {
    dataset: Rc<D>,
}

impl<D: QueryableDataset> EvalDataset<D> {
    fn internal_quads_for_pattern(
        &self,
        subject: Option<&D::InternalTerm>,
        predicate: Option<&D::InternalTerm>,
        object: Option<&D::InternalTerm>,
        graph_name: Option<Option<&D::InternalTerm>>,
    ) -> impl Iterator<Item = Result<InternalQuad<D>, QueryEvaluationError>> + 'static {
        self.dataset
            .internal_quads_for_pattern(subject, predicate, object, graph_name)
            .map(|r| r.map_err(|e| QueryEvaluationError::Dataset(Box::new(e))))
    }

    fn internal_named_graphs(
        &self,
    ) -> impl Iterator<Item = Result<D::InternalTerm, QueryEvaluationError>> {
        self.dataset
            .internal_named_graphs()
            .map(|r| r.map_err(|e| QueryEvaluationError::Dataset(Box::new(e))))
    }

    fn contains_internal_graph_name(
        &self,
        graph_name: &D::InternalTerm,
    ) -> Result<bool, QueryEvaluationError> {
        self.dataset
            .contains_internal_graph_name(graph_name)
            .map_err(|e| QueryEvaluationError::Dataset(Box::new(e)))
    }

    fn internalize_term(&self, term: Term) -> Result<D::InternalTerm, QueryEvaluationError> {
        self.dataset
            .internalize_term(term)
            .map_err(|e| QueryEvaluationError::Dataset(Box::new(e)))
    }

    fn externalize_term(&self, term: D::InternalTerm) -> Result<Term, QueryEvaluationError> {
        self.dataset
            .externalize_term(term)
            .map_err(|e| QueryEvaluationError::Dataset(Box::new(e)))
    }

    fn externalize_expression_term(
        &self,
        term: D::InternalTerm,
    ) -> Result<ExpressionTerm, QueryEvaluationError> {
        self.dataset
            .externalize_expression_term(term)
            .map_err(|e| QueryEvaluationError::Dataset(Box::new(e)))
    }

    fn internalize_expression_term(
        &self,
        term: ExpressionTerm,
    ) -> Result<D::InternalTerm, QueryEvaluationError> {
        self.dataset
            .internalize_expression_term(term)
            .map_err(|e| QueryEvaluationError::Dataset(Box::new(e)))
    }

    fn internal_term_effective_boolean_value(
        &self,
        term: D::InternalTerm,
    ) -> Result<Option<bool>, QueryEvaluationError> {
        self.dataset
            .internal_term_effective_boolean_value(term)
            .map_err(|e| QueryEvaluationError::Dataset(Box::new(e)))
    }
}

impl<D: QueryableDataset> Clone for EvalDataset<D> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            dataset: Rc::clone(&self.dataset),
        }
    }
}

pub struct InternalTuple<D: QueryableDataset> {
    inner: Vec<Option<D::InternalTerm>>,
}

impl<D: QueryableDataset> InternalTuple<D> {
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            inner: Vec::with_capacity(capacity),
        }
    }

    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    pub fn contains(&self, index: usize) -> bool {
        self.inner.get(index).is_some_and(Option::is_some)
    }

    pub fn get(&self, index: usize) -> Option<&D::InternalTerm> {
        self.inner.get(index).unwrap_or(&None).as_ref()
    }

    pub fn iter(&self) -> impl Iterator<Item = Option<D::InternalTerm>> + '_ {
        self.inner.iter().cloned()
    }

    pub fn set(&mut self, index: usize, value: D::InternalTerm) {
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

impl<D: QueryableDataset> Clone for InternalTuple<D> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<D: QueryableDataset> PartialEq for InternalTuple<D> {
    #[inline]
    fn eq(&self, other: &InternalTuple<D>) -> bool {
        self.inner == other.inner
    }
}

impl<D: QueryableDataset> Eq for InternalTuple<D> {}

impl<D: QueryableDataset> Hash for InternalTuple<D> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

impl<D: QueryableDataset> IntoIterator for InternalTuple<D> {
    type Item = Option<D::InternalTerm>;
    type IntoIter = std::vec::IntoIter<Option<D::InternalTerm>>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

type InternalTuplesIterator<D> =
    Box<dyn Iterator<Item = Result<InternalTuple<D>, QueryEvaluationError>>>;

pub struct SimpleEvaluator<D: QueryableDataset> {
    dataset: EvalDataset<D>,
    base_iri: Option<Rc<Iri<String>>>,
    now: DateTime,
    service_handler: Rc<ServiceHandlerRegistry>,
    custom_functions: Rc<CustomFunctionRegistry>,
    run_stats: bool,
}

impl<D: QueryableDataset> SimpleEvaluator<D> {
    pub fn new(
        dataset: D,
        base_iri: Option<Rc<Iri<String>>>,
        service_handler: Rc<ServiceHandlerRegistry>,
        custom_functions: Rc<CustomFunctionRegistry>,
        run_stats: bool,
    ) -> Self {
        Self {
            dataset: EvalDataset {
                dataset: Rc::new(dataset),
            },
            base_iri,
            now: DateTime::now(),
            service_handler,
            custom_functions,
            run_stats,
        }
    }

    pub fn evaluate_select(
        &self,
        pattern: &GraphPattern,
        substitutions: impl IntoIterator<Item = (Variable, Term)>,
    ) -> (
        Result<QuerySolutionIter, QueryEvaluationError>,
        Rc<EvalNodeWithStats>,
    ) {
        let mut variables = Vec::new();
        let (eval, stats) = self.graph_pattern_evaluator(pattern, &mut variables);
        let from = match encode_initial_bindings(&self.dataset, &variables, substitutions) {
            Ok(from) => from,
            Err(e) => return (Err(e), stats),
        };
        (
            Ok(decode_bindings(
                self.dataset.clone(),
                eval(from),
                Arc::from(variables),
            )),
            stats,
        )
    }

    pub fn evaluate_ask(
        &self,
        pattern: &GraphPattern,
        substitutions: impl IntoIterator<Item = (Variable, Term)>,
    ) -> (Result<bool, QueryEvaluationError>, Rc<EvalNodeWithStats>) {
        let mut variables = Vec::new();
        let (eval, stats) = self.graph_pattern_evaluator(pattern, &mut variables);
        let from = match encode_initial_bindings(&self.dataset, &variables, substitutions) {
            Ok(from) => from,
            Err(e) => return (Err(e), stats),
        };
        // We apply the same table as the or operation:
        // we return true if we get any valid tuple, an error if we get an error and false otherwise
        let mut error = None;
        for solution in eval(from) {
            if let Err(e) = solution {
                // We keep the first error
                error.get_or_insert(e);
            } else {
                // We have found a valid tuple
                return (Ok(true), stats);
            }
        }
        (
            if let Some(e) = error {
                Err(e)
            } else {
                Ok(false)
            },
            stats,
        )
    }

    pub fn evaluate_construct(
        &self,
        pattern: &GraphPattern,
        template: &[TriplePattern],
        substitutions: impl IntoIterator<Item = (Variable, Term)>,
    ) -> (
        Result<QueryTripleIter, QueryEvaluationError>,
        Rc<EvalNodeWithStats>,
    ) {
        let mut variables = Vec::new();
        let (eval, stats) = self.graph_pattern_evaluator(pattern, &mut variables);
        let mut bnodes = Vec::new();
        let template = template
            .iter()
            .filter_map(|t| {
                Some(TripleTemplate {
                    subject: TripleTemplateValue::from_term_or_variable(
                        &t.subject,
                        &mut variables,
                        &mut bnodes,
                    )?,
                    predicate: TripleTemplateValue::from_named_node_or_variable(
                        &t.predicate,
                        &mut variables,
                    ),
                    object: TripleTemplateValue::from_term_or_variable(
                        &t.object,
                        &mut variables,
                        &mut bnodes,
                    )?,
                })
            })
            .collect();
        let from = match encode_initial_bindings(&self.dataset, &variables, substitutions) {
            Ok(from) => from,
            Err(e) => return (Err(e), stats),
        };
        (
            Ok(QueryTripleIter::new(ConstructIterator {
                eval: self.clone(),
                iter: eval(from),
                template,
                buffered_results: Vec::default(),
                already_emitted_results: FxHashSet::default(),
                bnodes: Vec::default(),
            })),
            stats,
        )
    }

    pub fn evaluate_describe(
        &self,
        pattern: &GraphPattern,
        substitutions: impl IntoIterator<Item = (Variable, Term)>,
    ) -> (
        Result<QueryTripleIter, QueryEvaluationError>,
        Rc<EvalNodeWithStats>,
    ) {
        let mut variables = Vec::new();
        let (eval, stats) = self.graph_pattern_evaluator(pattern, &mut variables);
        let from = match encode_initial_bindings(&self.dataset, &variables, substitutions) {
            Ok(from) => from,
            Err(e) => return (Err(e), stats),
        };
        (
            Ok(QueryTripleIter::new(DescribeIterator {
                eval: self.clone(),
                tuples_to_describe: eval(from),
                nodes_described: FxHashSet::default(),
                nodes_to_describe: Vec::default(),
                quads: Box::new(empty()),
            })),
            stats,
        )
    }

    pub fn graph_pattern_evaluator(
        &self,
        pattern: &GraphPattern,
        encoded_variables: &mut Vec<Variable>,
    ) -> (
        Rc<dyn Fn(InternalTuple<D>) -> InternalTuplesIterator<D>>,
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
                let duration = start.elapsed();
                stats.exec_duration.set(
                    stats
                        .exec_duration
                        .get()
                        .and_then(|d| d.checked_add(duration?)),
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
    ) -> Rc<dyn Fn(InternalTuple<D>) -> InternalTuplesIterator<D>> {
        match pattern {
            GraphPattern::Values {
                variables,
                bindings,
            } => {
                let encoding = variables
                    .iter()
                    .map(|v| encode_variable(encoded_variables, v))
                    .collect::<Vec<_>>();
                match bindings
                    .iter()
                    .map(|row| {
                        let mut result = InternalTuple::with_capacity(variables.len());
                        for (key, value) in row.iter().enumerate() {
                            if let Some(term) = value {
                                result.set(
                                    encoding[key],
                                    match term {
                                        GroundTerm::NamedNode(node) => {
                                            self.encode_term(node.clone())
                                        }
                                        GroundTerm::Literal(literal) => {
                                            self.encode_term(literal.clone())
                                        }
                                        #[cfg(feature = "rdf-star")]
                                        GroundTerm::Triple(triple) => self.encode_triple(triple),
                                    }?,
                                );
                            }
                        }
                        Ok(result)
                    })
                    .collect::<Result<Vec<_>, _>>()
                {
                    Ok(encoded_tuples) => Rc::new(move |from| {
                        Box::new(
                            encoded_tuples
                                .iter()
                                .filter_map(move |t| t.combine_with(&from))
                                .map(Ok)
                                .collect::<Vec<_>>()
                                .into_iter(),
                        )
                    }),
                    Err(e) => error_evaluator(e),
                }
            }
            GraphPattern::QuadPattern {
                subject,
                predicate,
                object,
                graph_name,
            } => {
                let subject_selector = match TupleSelector::from_ground_term_pattern(
                    subject,
                    encoded_variables,
                    &self.dataset,
                ) {
                    Ok(selector) => selector,
                    Err(e) => return error_evaluator(e),
                };
                let predicate_selector = match TupleSelector::from_named_node_pattern(
                    predicate,
                    encoded_variables,
                    &self.dataset,
                ) {
                    Ok(selector) => selector,
                    Err(e) => return error_evaluator(e),
                };
                let object_selector = match TupleSelector::from_ground_term_pattern(
                    object,
                    encoded_variables,
                    &self.dataset,
                ) {
                    Ok(selector) => selector,
                    Err(e) => return error_evaluator(e),
                };
                let graph_name_selector = if let Some(graph_name) = graph_name.as_ref() {
                    match TupleSelector::from_named_node_pattern(
                        graph_name,
                        encoded_variables,
                        &self.dataset,
                    ) {
                        Ok(selector) => Some(selector),
                        Err(e) => return error_evaluator(e),
                    }
                } else {
                    None
                };
                let dataset = self.dataset.clone();
                Rc::new(move |from| {
                    let input_subject = match subject_selector.get_pattern_value(
                        &from,
                        #[cfg(feature = "rdf-star")]
                        &dataset,
                    ) {
                        Ok(value) => value,
                        Err(e) => return Box::new(once(Err(e))),
                    };
                    let input_predicate = match predicate_selector.get_pattern_value(
                        &from,
                        #[cfg(feature = "rdf-star")]
                        &dataset,
                    ) {
                        Ok(value) => value,
                        Err(e) => return Box::new(once(Err(e))),
                    };
                    let input_object = match object_selector.get_pattern_value(
                        &from,
                        #[cfg(feature = "rdf-star")]
                        &dataset,
                    ) {
                        Ok(value) => value,
                        Err(e) => return Box::new(once(Err(e))),
                    };
                    let input_graph_name = if let Some(graph_name_selector) = &graph_name_selector {
                        match graph_name_selector.get_pattern_value(
                            &from,
                            #[cfg(feature = "rdf-star")]
                            &dataset,
                        ) {
                            Ok(value) => value,
                            Err(e) => return Box::new(once(Err(e))),
                        }
                        .map(Some)
                    } else {
                        Some(None) // default graph
                    };
                    let iter = dataset.internal_quads_for_pattern(
                        input_subject.as_ref(),
                        input_predicate.as_ref(),
                        input_object.as_ref(),
                        input_graph_name.as_ref().map(|g| g.as_ref()),
                    );
                    let subject_selector = subject_selector.clone();
                    let predicate_selector = predicate_selector.clone();
                    let object_selector = object_selector.clone();
                    let graph_name_selector = graph_name_selector.clone();
                    #[cfg(feature = "rdf-star")]
                    let dataset = dataset.clone();
                    Box::new(
                        iter.map(move |quad| {
                            let quad = quad?;
                            let mut new_tuple = from.clone();
                            if !put_pattern_value(
                                &subject_selector,
                                quad.subject,
                                &mut new_tuple,
                                #[cfg(feature = "rdf-star")]
                                &dataset,
                            )? {
                                return Ok(None);
                            }
                            if !put_pattern_value(
                                &predicate_selector,
                                quad.predicate,
                                &mut new_tuple,
                                #[cfg(feature = "rdf-star")]
                                &dataset,
                            )? {
                                return Ok(None);
                            }
                            if !put_pattern_value(
                                &object_selector,
                                quad.object,
                                &mut new_tuple,
                                #[cfg(feature = "rdf-star")]
                                &dataset,
                            )? {
                                return Ok(None);
                            }
                            if let Some(graph_name_selector) = &graph_name_selector {
                                let Some(quad_graph_name) = quad.graph_name else {
                                    return Err(QueryEvaluationError::UnexpectedDefaultGraph);
                                };
                                if !put_pattern_value(
                                    graph_name_selector,
                                    quad_graph_name,
                                    &mut new_tuple,
                                    #[cfg(feature = "rdf-star")]
                                    &dataset,
                                )? {
                                    return Ok(None);
                                }
                            }
                            Ok(Some(new_tuple))
                        })
                        .filter_map(Result::transpose),
                    )
                })
            }
            GraphPattern::Path {
                subject,
                path,
                object,
                graph_name,
            } => {
                let subject_selector = match TupleSelector::from_ground_term_pattern(
                    subject,
                    encoded_variables,
                    &self.dataset,
                ) {
                    Ok(selector) => selector,
                    Err(e) => return error_evaluator(e),
                };
                let path = match self.encode_property_path(path) {
                    Ok(path) => path,
                    Err(e) => return error_evaluator(e),
                };
                let object_selector = match TupleSelector::from_ground_term_pattern(
                    object,
                    encoded_variables,
                    &self.dataset,
                ) {
                    Ok(selector) => selector,
                    Err(e) => return error_evaluator(e),
                };
                let graph_name_selector = if let Some(graph_name) = graph_name.as_ref() {
                    match TupleSelector::from_named_node_pattern(
                        graph_name,
                        encoded_variables,
                        &self.dataset,
                    ) {
                        Ok(selector) => Some(selector),
                        Err(e) => return error_evaluator(e),
                    }
                } else {
                    None
                };
                let dataset = self.dataset.clone();
                Rc::new(move |from| {
                    let input_subject = match subject_selector.get_pattern_value(
                        &from,
                        #[cfg(feature = "rdf-star")]
                        &dataset,
                    ) {
                        Ok(value) => value,
                        Err(e) => return Box::new(once(Err(e))),
                    };
                    let path_eval = PathEvaluator {
                        dataset: dataset.clone(),
                    };
                    let input_object = match object_selector.get_pattern_value(
                        &from,
                        #[cfg(feature = "rdf-star")]
                        &dataset,
                    ) {
                        Ok(value) => value,
                        Err(e) => return Box::new(once(Err(e))),
                    };
                    let input_graph_name = if let Some(graph_name_selector) = &graph_name_selector {
                        match graph_name_selector.get_pattern_value(
                            &from,
                            #[cfg(feature = "rdf-star")]
                            &dataset,
                        ) {
                            Ok(value) => value,
                            Err(e) => return Box::new(once(Err(e))),
                        }
                        .map(Some)
                    } else {
                        Some(None) // default graph
                    };
                    match (input_subject, input_object, input_graph_name) {
                        (Some(input_subject), Some(input_object), Some(input_graph_name)) => {
                            match path_eval.eval_closed_in_graph(
                                &path,
                                &input_subject,
                                &input_object,
                                input_graph_name.as_ref(),
                            ) {
                                Ok(true) => Box::new(once(Ok(from))),
                                Ok(false) => Box::new(empty()),
                                Err(e) => Box::new(once(Err(e))),
                            }
                        }
                        (Some(input_subject), None, Some(input_graph_name)) => {
                            let object_selector = object_selector.clone();
                            #[cfg(feature = "rdf-star")]
                            let dataset = dataset.clone();
                            Box::new(
                                path_eval
                                    .eval_from_in_graph(
                                        &path,
                                        &input_subject,
                                        input_graph_name.as_ref(),
                                    )
                                    .map(move |o| {
                                        let o = o?;
                                        let mut new_tuple = from.clone();
                                        if !put_pattern_value(
                                            &object_selector,
                                            o,
                                            &mut new_tuple,
                                            #[cfg(feature = "rdf-star")]
                                            &dataset,
                                        )? {
                                            return Ok(None);
                                        }
                                        Ok(Some(new_tuple))
                                    })
                                    .filter_map(Result::transpose),
                            )
                        }
                        (None, Some(input_object), Some(input_graph_name)) => {
                            let subject_selector = subject_selector.clone();
                            #[cfg(feature = "rdf-star")]
                            let dataset = dataset.clone();
                            Box::new(
                                path_eval
                                    .eval_to_in_graph(
                                        &path,
                                        &input_object,
                                        input_graph_name.as_ref(),
                                    )
                                    .map(move |s| {
                                        let s = s?;
                                        let mut new_tuple = from.clone();
                                        if !put_pattern_value(
                                            &subject_selector,
                                            s,
                                            &mut new_tuple,
                                            #[cfg(feature = "rdf-star")]
                                            &dataset,
                                        )? {
                                            return Ok(None);
                                        }
                                        Ok(Some(new_tuple))
                                    })
                                    .filter_map(Result::transpose),
                            )
                        }
                        (None, None, Some(input_graph_name)) => {
                            let subject_selector = subject_selector.clone();
                            let object_selector = object_selector.clone();
                            #[cfg(feature = "rdf-star")]
                            let dataset = dataset.clone();
                            Box::new(
                                path_eval
                                    .eval_open_in_graph(&path, input_graph_name.as_ref())
                                    .map(move |t| {
                                        let (s, o) = t?;
                                        let mut new_tuple = from.clone();
                                        if !put_pattern_value(
                                            &subject_selector,
                                            s,
                                            &mut new_tuple,
                                            #[cfg(feature = "rdf-star")]
                                            &dataset,
                                        )? {
                                            return Ok(None);
                                        }
                                        if !put_pattern_value(
                                            &object_selector,
                                            o,
                                            &mut new_tuple,
                                            #[cfg(feature = "rdf-star")]
                                            &dataset,
                                        )? {
                                            return Ok(None);
                                        }
                                        Ok(Some(new_tuple))
                                    })
                                    .filter_map(Result::transpose),
                            )
                        }
                        (Some(input_subject), Some(input_object), None) => {
                            let graph_name_selector = graph_name_selector.clone();
                            #[cfg(feature = "rdf-star")]
                            let dataset = dataset.clone();
                            Box::new(
                                path_eval
                                    .eval_closed_in_unknown_graph(
                                        &path,
                                        &input_subject,
                                        &input_object,
                                    )
                                    .map(move |g| {
                                        let g = g?;
                                        let mut new_tuple = from.clone();
                                        if let Some(graph_name_selector) = &graph_name_selector {
                                            let Some(g) = g else {
                                                return Err(
                                                    QueryEvaluationError::UnexpectedDefaultGraph,
                                                );
                                            };
                                            if !put_pattern_value(
                                                graph_name_selector,
                                                g,
                                                &mut new_tuple,
                                                #[cfg(feature = "rdf-star")]
                                                &dataset,
                                            )? {
                                                return Ok(None);
                                            }
                                        }
                                        Ok(Some(new_tuple))
                                    })
                                    .filter_map(Result::transpose),
                            )
                        }
                        (Some(input_subject), None, None) => {
                            let object_selector = object_selector.clone();
                            let graph_name_selector = graph_name_selector.clone();
                            #[cfg(feature = "rdf-star")]
                            let dataset = dataset.clone();
                            Box::new(
                                path_eval
                                    .eval_from_in_unknown_graph(&path, &input_subject)
                                    .map(move |t| {
                                        let (o, g) = t?;
                                        let mut new_tuple = from.clone();
                                        if !put_pattern_value(
                                            &object_selector,
                                            o,
                                            &mut new_tuple,
                                            #[cfg(feature = "rdf-star")]
                                            &dataset,
                                        )? {
                                            return Ok(None);
                                        }
                                        if let Some(graph_name_selector) = &graph_name_selector {
                                            let Some(g) = g else {
                                                return Err(
                                                    QueryEvaluationError::UnexpectedDefaultGraph,
                                                );
                                            };
                                            if !put_pattern_value(
                                                graph_name_selector,
                                                g,
                                                &mut new_tuple,
                                                #[cfg(feature = "rdf-star")]
                                                &dataset,
                                            )? {
                                                return Ok(None);
                                            }
                                        }
                                        Ok(Some(new_tuple))
                                    })
                                    .filter_map(Result::transpose),
                            )
                        }
                        (None, Some(input_object), None) => {
                            let subject_selector = subject_selector.clone();
                            let graph_name_selector = graph_name_selector.clone();
                            #[cfg(feature = "rdf-star")]
                            let dataset = dataset.clone();
                            Box::new(
                                path_eval
                                    .eval_to_in_unknown_graph(&path, &input_object)
                                    .map(move |t| {
                                        let (s, g) = t?;
                                        let mut new_tuple = from.clone();
                                        if !put_pattern_value(
                                            &subject_selector,
                                            s,
                                            &mut new_tuple,
                                            #[cfg(feature = "rdf-star")]
                                            &dataset,
                                        )? {
                                            return Ok(None);
                                        }
                                        if let Some(graph_name_selector) = &graph_name_selector {
                                            let Some(g) = g else {
                                                return Err(
                                                    QueryEvaluationError::UnexpectedDefaultGraph,
                                                );
                                            };
                                            if !put_pattern_value(
                                                graph_name_selector,
                                                g,
                                                &mut new_tuple,
                                                #[cfg(feature = "rdf-star")]
                                                &dataset,
                                            )? {
                                                return Ok(None);
                                            }
                                        }
                                        Ok(Some(new_tuple))
                                    })
                                    .filter_map(Result::transpose),
                            )
                        }
                        (None, None, None) => {
                            let subject_selector = subject_selector.clone();
                            let object_selector = object_selector.clone();
                            let graph_name_selector = graph_name_selector.clone();
                            #[cfg(feature = "rdf-star")]
                            let dataset = dataset.clone();
                            Box::new(
                                path_eval
                                    .eval_open_in_unknown_graph(&path)
                                    .map(move |t| {
                                        let (s, o, g) = t?;
                                        let mut new_tuple = from.clone();
                                        if !put_pattern_value(
                                            &subject_selector,
                                            s,
                                            &mut new_tuple,
                                            #[cfg(feature = "rdf-star")]
                                            &dataset,
                                        )? {
                                            return Ok(None);
                                        }
                                        if !put_pattern_value(
                                            &object_selector,
                                            o,
                                            &mut new_tuple,
                                            #[cfg(feature = "rdf-star")]
                                            &dataset,
                                        )? {
                                            return Ok(None);
                                        }
                                        if let Some(graph_name_selector) = &graph_name_selector {
                                            let Some(g) = g else {
                                                return Err(
                                                    QueryEvaluationError::UnexpectedDefaultGraph,
                                                );
                                            };
                                            if !put_pattern_value(
                                                graph_name_selector,
                                                g,
                                                &mut new_tuple,
                                                #[cfg(feature = "rdf-star")]
                                                &dataset,
                                            )? {
                                                return Ok(None);
                                            }
                                        }
                                        Ok(Some(new_tuple))
                                    })
                                    .filter_map(Result::transpose),
                            )
                        }
                    }
                })
            }
            GraphPattern::Graph { graph_name } => {
                let graph_name_selector = match TupleSelector::from_named_node_pattern(
                    graph_name,
                    encoded_variables,
                    &self.dataset,
                ) {
                    Ok(selector) => selector,
                    Err(e) => return error_evaluator(e),
                };
                let dataset = self.dataset.clone();
                Rc::new(move |from| {
                    let input_graph_name = match graph_name_selector.get_pattern_value(
                        &from,
                        #[cfg(feature = "rdf-star")]
                        &dataset,
                    ) {
                        Ok(value) => value,
                        Err(e) => return Box::new(once(Err(e))),
                    };
                    if let Some(input_graph_name) = input_graph_name {
                        match dataset.contains_internal_graph_name(&input_graph_name) {
                            Ok(true) => Box::new(once(Ok(from))),
                            Ok(false) => Box::new(empty()),
                            Err(e) => Box::new(once(Err(e))),
                        }
                    } else {
                        let graph_name_selector = graph_name_selector.clone();
                        #[cfg(feature = "rdf-star")]
                        let dataset = dataset.clone();
                        Box::new(
                            dataset
                                .internal_named_graphs()
                                .map(move |graph_name| {
                                    let graph_name = graph_name?;
                                    let mut new_tuple = from.clone();
                                    if !put_pattern_value(
                                        &graph_name_selector,
                                        graph_name,
                                        &mut new_tuple,
                                        #[cfg(feature = "rdf-star")]
                                        &dataset,
                                    )? {
                                        return Ok(None);
                                    }
                                    Ok(Some(new_tuple))
                                })
                                .filter_map(Result::transpose),
                        )
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
                                let built_values = build(from.clone())
                                    .filter_map(|result| match result {
                                        Ok(result) => Some(result),
                                        Err(error) => {
                                            errors.push(Err(error));
                                            None
                                        }
                                    })
                                    .collect::<Vec<_>>();
                                if built_values.is_empty() && errors.is_empty() {
                                    // We don't bother to execute the other side
                                    return Box::new(empty());
                                }
                                let mut probe_iter = probe(from).peekable();
                                if probe_iter.peek().is_none() {
                                    // We know it's empty and can discard errors
                                    return Box::new(empty());
                                }
                                Box::new(CartesianProductJoinIterator {
                                    probe_iter,
                                    built: built_values,
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
                                let mut built_values = InternalTupleSet::new(keys.clone());
                                built_values.extend(build(from.clone()).filter_map(|result| {
                                    match result {
                                        Ok(result) => Some(result),
                                        Err(error) => {
                                            errors.push(Err(error));
                                            None
                                        }
                                    }
                                }));
                                if built_values.is_empty() && errors.is_empty() {
                                    // We don't bother to execute the other side
                                    return Box::new(empty());
                                }
                                let mut probe_iter = probe(from).peekable();
                                if probe_iter.peek().is_none() {
                                    // We know it's empty and can discard errors
                                    return Box::new(empty());
                                }
                                Box::new(HashJoinIterator {
                                    probe_iter,
                                    built: built_values,
                                    buffered_results: errors,
                                })
                            })
                        }
                    }
                }
            }
            #[cfg(feature = "sep-0006")]
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
                                left_tuple_to_yield: None,
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
                                if right.is_empty() {
                                    return left(from);
                                }
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
                                let mut right_values = InternalTupleSet::new(keys.clone());
                                right_values.extend(right(from.clone()).filter_map(Result::ok));
                                if right_values.is_empty() {
                                    return left(from);
                                }
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
                let expression = self.effective_boolean_value_expression_evaluator(
                    expression,
                    encoded_variables,
                    stat_children,
                );

                match algorithm {
                    LeftJoinAlgorithm::HashBuildRightProbeLeft { keys } => {
                        // Real hash join
                        let keys = keys
                            .iter()
                            .map(|v| encode_variable(encoded_variables, v))
                            .collect::<Vec<_>>();
                        Rc::new(move |from| {
                            let mut errors = Vec::default();
                            let mut right_values = InternalTupleSet::new(keys.clone());
                            right_values.extend(right(from.clone()).filter_map(
                                |result| match result {
                                    Ok(result) => Some(result),
                                    Err(error) => {
                                        errors.push(Err(error));
                                        None
                                    }
                                },
                            ));
                            if right_values.is_empty() && errors.is_empty() {
                                return left(from);
                            }
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
                let expression = self.effective_boolean_value_expression_evaluator(
                    expression,
                    encoded_variables,
                    stat_children,
                );
                Rc::new(move |from| {
                    let expression = Rc::clone(&expression);
                    Box::new(child(from).filter(move |tuple| match tuple {
                        Ok(tuple) => expression(tuple).unwrap_or(false),
                        Err(_) => true,
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
                if let Some(expression) =
                    self.internal_expression_evaluator(expression, encoded_variables, stat_children)
                {
                    return Rc::new(move |from| {
                        let expression = Rc::clone(&expression);
                        Box::new(child(from).map(move |tuple| {
                            let mut tuple = tuple?;
                            if let Some(value) = expression(&tuple) {
                                tuple.set(position, value);
                            }
                            Ok(tuple)
                        }))
                    });
                }

                let expression =
                    self.expression_evaluator(expression, encoded_variables, stat_children);
                let dataset = self.dataset.clone();
                Rc::new(move |from| {
                    let expression = Rc::clone(&expression);
                    let dataset = dataset.clone();
                    Box::new(child(from).map(move |tuple| {
                        let mut tuple = tuple?;
                        if let Some(value) = expression(&tuple) {
                            tuple.set(position, dataset.internalize_expression_term(value)?);
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
                                    match cmp_terms(expression(a).as_ref(), expression(b).as_ref())
                                    {
                                        Ordering::Greater => return Ordering::Greater,
                                        Ordering::Less => return Ordering::Less,
                                        Ordering::Equal => (),
                                    }
                                }
                                ComparatorFunction::Desc(expression) => {
                                    match cmp_terms(expression(a).as_ref(), expression(b).as_ref())
                                    {
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
                    let mut input_tuple = InternalTuple::with_capacity(mapping.len());
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
                let accumulator_builders = aggregates
                    .iter()
                    .map(|(_, aggregate)| {
                        self.accumulator_builder(aggregate, encoded_variables, stat_children)
                    })
                    .collect::<Vec<_>>();
                let accumulator_variables = aggregates
                    .iter()
                    .map(|(variable, _)| encode_variable(encoded_variables, variable))
                    .collect::<Vec<_>>();
                let dataset = self.dataset.clone();
                Rc::new(move |from| {
                    let tuple_size = from.capacity();
                    let key_variables = Rc::clone(&key_variables);
                    let mut errors = Vec::default();
                    let mut accumulators_for_group = FxHashMap::<
                        Vec<Option<D::InternalTerm>>,
                        Vec<AccumulatorWrapper<D>>,
                    >::default();
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
                            for accumulator in key_accumulators {
                                accumulator.add(&tuple);
                            }
                        });
                    let accumulator_variables = accumulator_variables.clone();
                    let dataset = dataset.clone();
                    Box::new(
                        errors
                            .into_iter()
                            .map(Err)
                            .chain(accumulators_for_group.into_iter().map(
                                move |(key, accumulators)| {
                                    let mut result = InternalTuple::with_capacity(tuple_size);
                                    for (variable, value) in key_variables.iter().zip(key) {
                                        if let Some(value) = value {
                                            result.set(*variable, value);
                                        }
                                    }
                                    for (accumulator, variable) in
                                        accumulators.into_iter().zip(&accumulator_variables)
                                    {
                                        if let Some(value) = accumulator.finish() {
                                            result.set(
                                                *variable,
                                                dataset.internalize_expression_term(value)?,
                                            );
                                        }
                                    }
                                    Ok(result)
                                },
                            )),
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
                let service_name = match TupleSelector::from_named_node_pattern(
                    name,
                    encoded_variables,
                    &self.dataset,
                ) {
                    Ok(service_name) => service_name,
                    Err(e) => return error_evaluator(e),
                };
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
        }
    }

    fn evaluate_service(
        &self,
        service_name: &TupleSelector<D>,
        graph_pattern: &spargebra::algebra::GraphPattern,
        variables: Rc<[Variable]>,
        from: &InternalTuple<D>,
    ) -> Result<InternalTuplesIterator<D>, QueryEvaluationError> {
        let service_name = service_name
            .get_pattern_value(
                from,
                #[cfg(feature = "rdf-star")]
                &self.dataset,
            )?
            .ok_or(QueryEvaluationError::UnboundService)?;
        let service_name = match self.dataset.externalize_term(service_name)? {
            Term::NamedNode(service_name) => service_name,
            term => return Err(QueryEvaluationError::InvalidServiceName(term)),
        };
        let iter = self.service_handler.handle(
            service_name,
            graph_pattern.clone(),
            self.base_iri.as_ref().map(ToString::to_string),
        )?;
        Ok(encode_bindings(self.dataset.clone(), variables, iter))
    }

    fn accumulator_builder(
        &self,
        expression: &AggregateExpression,
        encoded_variables: &mut Vec<Variable>,
        stat_children: &mut Vec<Rc<EvalNodeWithStats>>,
    ) -> Box<dyn Fn() -> AccumulatorWrapper<D>> {
        match expression {
            AggregateExpression::CountSolutions { distinct } => {
                if *distinct {
                    Box::new(move || AccumulatorWrapper::CountDistinctTuple {
                        count: 0,
                        seen: FxHashSet::default(),
                    })
                } else {
                    Box::new(move || AccumulatorWrapper::CountTuple { count: 0 })
                }
            }
            AggregateExpression::FunctionCall {
                name,
                distinct,
                expr,
            } => match name {
                AggregateFunction::Count => {
                    if let Some(evaluator) =
                        self.internal_expression_evaluator(expr, encoded_variables, stat_children)
                    {
                        return if *distinct {
                            Box::new(move || AccumulatorWrapper::CountDistinctInternal {
                                evaluator: Rc::clone(&evaluator),
                                seen: FxHashSet::default(),
                                count: 0,
                            })
                        } else {
                            Box::new(move || AccumulatorWrapper::CountInternal {
                                evaluator: Rc::clone(&evaluator),
                                count: 0,
                            })
                        };
                    }
                    let evaluator =
                        self.expression_evaluator(expr, encoded_variables, stat_children);
                    if *distinct {
                        Box::new(move || AccumulatorWrapper::DistinctExpression {
                            evaluator: Rc::clone(&evaluator),
                            seen: FxHashSet::default(),
                            accumulator: Some(Box::new(CountAccumulator::default())),
                        })
                    } else {
                        Box::new(move || AccumulatorWrapper::Expression {
                            evaluator: Rc::clone(&evaluator),
                            accumulator: Some(Box::new(CountAccumulator::default())),
                        })
                    }
                }
                AggregateFunction::Sum => {
                    let evaluator =
                        self.expression_evaluator(expr, encoded_variables, stat_children);
                    if *distinct {
                        Box::new(move || AccumulatorWrapper::DistinctExpression {
                            evaluator: Rc::clone(&evaluator),
                            seen: FxHashSet::default(),
                            accumulator: Some(Box::new(SumAccumulator::default())),
                        })
                    } else {
                        Box::new(move || AccumulatorWrapper::Expression {
                            evaluator: Rc::clone(&evaluator),
                            accumulator: Some(Box::new(SumAccumulator::default())),
                        })
                    }
                }
                AggregateFunction::Min => {
                    let evaluator =
                        self.expression_evaluator(expr, encoded_variables, stat_children);
                    if *distinct {
                        Box::new(move || AccumulatorWrapper::DistinctExpression {
                            evaluator: Rc::clone(&evaluator),
                            seen: FxHashSet::default(),
                            accumulator: Some(Box::new(MinAccumulator::default())),
                        })
                    } else {
                        Box::new(move || AccumulatorWrapper::Expression {
                            evaluator: Rc::clone(&evaluator),
                            accumulator: Some(Box::new(MinAccumulator::default())),
                        })
                    }
                }
                AggregateFunction::Max => {
                    let evaluator =
                        self.expression_evaluator(expr, encoded_variables, stat_children);
                    if *distinct {
                        Box::new(move || AccumulatorWrapper::DistinctExpression {
                            evaluator: Rc::clone(&evaluator),
                            seen: FxHashSet::default(),
                            accumulator: Some(Box::new(MaxAccumulator::default())),
                        })
                    } else {
                        Box::new(move || AccumulatorWrapper::Expression {
                            evaluator: Rc::clone(&evaluator),
                            accumulator: Some(Box::new(MaxAccumulator::default())),
                        })
                    }
                }
                AggregateFunction::Avg => {
                    let evaluator =
                        self.expression_evaluator(expr, encoded_variables, stat_children);
                    if *distinct {
                        Box::new(move || AccumulatorWrapper::DistinctExpression {
                            evaluator: Rc::clone(&evaluator),
                            seen: FxHashSet::default(),
                            accumulator: Some(Box::new(AvgAccumulator::default())),
                        })
                    } else {
                        Box::new(move || AccumulatorWrapper::Expression {
                            evaluator: Rc::clone(&evaluator),
                            accumulator: Some(Box::new(AvgAccumulator::default())),
                        })
                    }
                }
                AggregateFunction::Sample => {
                    let evaluator =
                        self.expression_evaluator(expr, encoded_variables, stat_children);
                    Box::new(move || AccumulatorWrapper::Sample {
                        evaluator: Rc::clone(&evaluator),
                        value: None,
                    })
                }
                AggregateFunction::GroupConcat { separator } => {
                    let separator = Rc::from(separator.as_deref().unwrap_or(" "));
                    let evaluator =
                        self.expression_evaluator(expr, encoded_variables, stat_children);
                    if *distinct {
                        Box::new(move || AccumulatorWrapper::DistinctExpression {
                            evaluator: Rc::clone(&evaluator),
                            seen: FxHashSet::default(),
                            accumulator: Some(Box::new(GroupConcatAccumulator::new(Rc::clone(
                                &separator,
                            )))),
                        })
                    } else {
                        Box::new(move || AccumulatorWrapper::Expression {
                            evaluator: Rc::clone(&evaluator),
                            accumulator: Some(Box::new(GroupConcatAccumulator::new(Rc::clone(
                                &separator,
                            )))),
                        })
                    }
                }
                AggregateFunction::Custom(_) => Box::new(move || AccumulatorWrapper::Failing),
            },
        }
    }

    /// Evaluates an expression and returns an internal term
    ///
    /// Returns None if building such expression would mean to convert back to an internal term at the end.
    fn internal_expression_evaluator(
        &self,
        expression: &Expression,
        encoded_variables: &mut Vec<Variable>,
        stat_children: &mut Vec<Rc<EvalNodeWithStats>>,
    ) -> Option<Rc<dyn Fn(&InternalTuple<D>) -> Option<D::InternalTerm>>> {
        Some(match expression {
            Expression::NamedNode(t) => {
                let t = self.encode_term(t.clone()).ok();
                Rc::new(move |_| t.clone())
            }
            Expression::Literal(t) => {
                let t = self.encode_term(t.clone()).ok();
                Rc::new(move |_| t.clone())
            }
            Expression::Variable(v) => {
                let v = encode_variable(encoded_variables, v);
                Rc::new(move |tuple| tuple.get(v).cloned())
            }
            Expression::Coalesce(l) => {
                let l = l
                    .iter()
                    .map(|e| {
                        self.internal_expression_evaluator(e, encoded_variables, stat_children)
                    })
                    .collect::<Option<Vec<_>>>()?;
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
                let a = self.effective_boolean_value_expression_evaluator(
                    a,
                    encoded_variables,
                    stat_children,
                );
                let b = self.internal_expression_evaluator(b, encoded_variables, stat_children)?;
                let c = self.internal_expression_evaluator(c, encoded_variables, stat_children)?;
                Rc::new(move |tuple| if a(tuple)? { b(tuple) } else { c(tuple) })
            }
            Expression::Or(_)
            | Expression::And(_)
            | Expression::Equal(_, _)
            | Expression::SameTerm(_, _)
            | Expression::Greater(_, _)
            | Expression::GreaterOrEqual(_, _)
            | Expression::Less(_, _)
            | Expression::LessOrEqual(_, _)
            | Expression::Add(_, _)
            | Expression::Subtract(_, _)
            | Expression::Multiply(_, _)
            | Expression::Divide(_, _)
            | Expression::UnaryPlus(_)
            | Expression::UnaryMinus(_)
            | Expression::Not(_)
            | Expression::Exists(_)
            | Expression::Bound(_)
            | Expression::FunctionCall(_, _) => return None, // TODO: we can do some functions
        })
    }

    /// Evaluate an expression and return its effective boolean value
    fn effective_boolean_value_expression_evaluator(
        &self,
        expression: &Expression,
        encoded_variables: &mut Vec<Variable>,
        stat_children: &mut Vec<Rc<EvalNodeWithStats>>,
    ) -> Rc<dyn Fn(&InternalTuple<D>) -> Option<bool>> {
        // TODO: avoid dyn?
        if let Some(eval) =
            self.internal_expression_evaluator(expression, encoded_variables, stat_children)
        {
            let dataset = self.dataset.clone();
            return Rc::new(move |tuple| {
                dataset
                    .internal_term_effective_boolean_value(eval(tuple)?)
                    .ok()?
            });
        }
        let eval = self.expression_evaluator(expression, encoded_variables, stat_children);
        Rc::new(move |tuple| eval(tuple)?.effective_boolean_value())
    }

    /// Evaluate an expression and return an explicit ExpressionTerm
    fn expression_evaluator(
        &self,
        expression: &Expression,
        encoded_variables: &mut Vec<Variable>,
        stat_children: &mut Vec<Rc<EvalNodeWithStats>>,
    ) -> Rc<dyn Fn(&InternalTuple<D>) -> Option<ExpressionTerm>> {
        match expression {
            Expression::NamedNode(t) => {
                let t = ExpressionTerm::from(Term::from(t.clone()));
                Rc::new(move |_| Some(t.clone()))
            }
            Expression::Literal(t) => {
                let t = ExpressionTerm::from(Term::from(t.clone()));
                Rc::new(move |_| Some(t.clone()))
            }
            Expression::Variable(v) => {
                let v = encode_variable(encoded_variables, v);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    tuple
                        .get(v)
                        .cloned()
                        .and_then(|t| dataset.externalize_expression_term(t).ok())
                })
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
                    .map(|i| {
                        self.effective_boolean_value_expression_evaluator(
                            i,
                            encoded_variables,
                            stat_children,
                        )
                    })
                    .collect::<Rc<[_]>>();
                Rc::new(move |tuple| {
                    let mut error = false;
                    for child in &*children {
                        match child(tuple) {
                            Some(true) => return Some(true.into()),
                            Some(false) => (),
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
                    .map(|i| {
                        self.effective_boolean_value_expression_evaluator(
                            i,
                            encoded_variables,
                            stat_children,
                        )
                    })
                    .collect::<Rc<[_]>>();
                Rc::new(move |tuple| {
                    let mut error = false;
                    for child in &*children {
                        match child(tuple) {
                            Some(true) => (),
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
                if let (Some(a), Some(b)) = (
                    self.internal_expression_evaluator(a, encoded_variables, stat_children),
                    self.internal_expression_evaluator(b, encoded_variables, stat_children),
                ) {
                    return Rc::new(move |tuple| Some((a(tuple)? == b(tuple)?).into()));
                };
                let a = self.expression_evaluator(a, encoded_variables, stat_children);
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                // TODO: if one is an internal term, this might be wrong in case we got checks like SameTerm(01, 1+0)
                Rc::new(move |tuple| Some((a(tuple)? == b(tuple)?).into()))
            }
            Expression::Greater(a, b) => {
                let a = self.expression_evaluator(a, encoded_variables, stat_children);
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                Rc::new(move |tuple| {
                    Some((partial_cmp(&a(tuple)?, &b(tuple)?)? == Ordering::Greater).into())
                })
            }
            Expression::GreaterOrEqual(a, b) => {
                let a = self.expression_evaluator(a, encoded_variables, stat_children);
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                Rc::new(move |tuple| {
                    Some(
                        match partial_cmp(&a(tuple)?, &b(tuple)?)? {
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
                Rc::new(move |tuple| {
                    Some((partial_cmp(&a(tuple)?, &b(tuple)?)? == Ordering::Less).into())
                })
            }
            Expression::LessOrEqual(a, b) => {
                let a = self.expression_evaluator(a, encoded_variables, stat_children);
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                Rc::new(move |tuple| {
                    Some(
                        match partial_cmp(&a(tuple)?, &b(tuple)?)? {
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
                Rc::new(move |tuple| {
                    Some(match NumericBinaryOperands::new(a(tuple)?, b(tuple)?)? {
                        NumericBinaryOperands::Float(v1, v2) => {
                            ExpressionTerm::FloatLiteral(v1 + v2)
                        }
                        NumericBinaryOperands::Double(v1, v2) => {
                            ExpressionTerm::DoubleLiteral(v1 + v2)
                        }
                        NumericBinaryOperands::Integer(v1, v2) => {
                            ExpressionTerm::IntegerLiteral(v1.checked_add(v2)?)
                        }
                        NumericBinaryOperands::Decimal(v1, v2) => {
                            ExpressionTerm::DecimalLiteral(v1.checked_add(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::Duration(v1, v2) => {
                            ExpressionTerm::DurationLiteral(v1.checked_add(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::YearMonthDuration(v1, v2) => {
                            ExpressionTerm::YearMonthDurationLiteral(v1.checked_add(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DayTimeDuration(v1, v2) => {
                            ExpressionTerm::DayTimeDurationLiteral(v1.checked_add(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateTimeDuration(v1, v2) => {
                            ExpressionTerm::DateTimeLiteral(v1.checked_add_duration(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateTimeYearMonthDuration(v1, v2) => {
                            ExpressionTerm::DateTimeLiteral(v1.checked_add_year_month_duration(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateTimeDayTimeDuration(v1, v2) => {
                            ExpressionTerm::DateTimeLiteral(v1.checked_add_day_time_duration(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateDuration(v1, v2) => {
                            ExpressionTerm::DateLiteral(v1.checked_add_duration(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateYearMonthDuration(v1, v2) => {
                            ExpressionTerm::DateLiteral(v1.checked_add_year_month_duration(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateDayTimeDuration(v1, v2) => {
                            ExpressionTerm::DateLiteral(v1.checked_add_day_time_duration(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::TimeDuration(v1, v2) => {
                            ExpressionTerm::TimeLiteral(v1.checked_add_duration(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::TimeDayTimeDuration(v1, v2) => {
                            ExpressionTerm::TimeLiteral(v1.checked_add_day_time_duration(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateTime(_, _)
                        | NumericBinaryOperands::Time(_, _)
                        | NumericBinaryOperands::Date(_, _) => return None,
                    })
                })
            }
            Expression::Subtract(a, b) => {
                let a = self.expression_evaluator(a, encoded_variables, stat_children);
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                Rc::new(move |tuple| {
                    Some(match NumericBinaryOperands::new(a(tuple)?, b(tuple)?)? {
                        NumericBinaryOperands::Float(v1, v2) => {
                            ExpressionTerm::FloatLiteral(v1 - v2)
                        }
                        NumericBinaryOperands::Double(v1, v2) => {
                            ExpressionTerm::DoubleLiteral(v1 - v2)
                        }
                        NumericBinaryOperands::Integer(v1, v2) => {
                            ExpressionTerm::IntegerLiteral(v1.checked_sub(v2)?)
                        }
                        NumericBinaryOperands::Decimal(v1, v2) => {
                            ExpressionTerm::DecimalLiteral(v1.checked_sub(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateTime(v1, v2) => {
                            ExpressionTerm::DayTimeDurationLiteral(v1.checked_sub(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::Date(v1, v2) => {
                            ExpressionTerm::DayTimeDurationLiteral(v1.checked_sub(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::Time(v1, v2) => {
                            ExpressionTerm::DayTimeDurationLiteral(v1.checked_sub(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::Duration(v1, v2) => {
                            ExpressionTerm::DurationLiteral(v1.checked_sub(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::YearMonthDuration(v1, v2) => {
                            ExpressionTerm::YearMonthDurationLiteral(v1.checked_sub(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DayTimeDuration(v1, v2) => {
                            ExpressionTerm::DayTimeDurationLiteral(v1.checked_sub(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateTimeDuration(v1, v2) => {
                            ExpressionTerm::DateTimeLiteral(v1.checked_sub_duration(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateTimeYearMonthDuration(v1, v2) => {
                            ExpressionTerm::DateTimeLiteral(v1.checked_sub_year_month_duration(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateTimeDayTimeDuration(v1, v2) => {
                            ExpressionTerm::DateTimeLiteral(v1.checked_sub_day_time_duration(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateDuration(v1, v2) => {
                            ExpressionTerm::DateLiteral(v1.checked_sub_duration(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateYearMonthDuration(v1, v2) => {
                            ExpressionTerm::DateLiteral(v1.checked_sub_year_month_duration(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::DateDayTimeDuration(v1, v2) => {
                            ExpressionTerm::DateLiteral(v1.checked_sub_day_time_duration(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::TimeDuration(v1, v2) => {
                            ExpressionTerm::TimeLiteral(v1.checked_sub_duration(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        NumericBinaryOperands::TimeDayTimeDuration(v1, v2) => {
                            ExpressionTerm::TimeLiteral(v1.checked_sub_day_time_duration(v2)?)
                        }
                    })
                })
            }
            Expression::Multiply(a, b) => {
                let a = self.expression_evaluator(a, encoded_variables, stat_children);
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                Rc::new(move |tuple| {
                    Some(match NumericBinaryOperands::new(a(tuple)?, b(tuple)?)? {
                        NumericBinaryOperands::Float(v1, v2) => {
                            ExpressionTerm::FloatLiteral(v1 * v2)
                        }
                        NumericBinaryOperands::Double(v1, v2) => {
                            ExpressionTerm::DoubleLiteral(v1 * v2)
                        }
                        NumericBinaryOperands::Integer(v1, v2) => {
                            ExpressionTerm::IntegerLiteral(v1.checked_mul(v2)?)
                        }
                        NumericBinaryOperands::Decimal(v1, v2) => {
                            ExpressionTerm::DecimalLiteral(v1.checked_mul(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        _ => return None,
                    })
                })
            }
            Expression::Divide(a, b) => {
                let a = self.expression_evaluator(a, encoded_variables, stat_children);
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                Rc::new(move |tuple| {
                    Some(match NumericBinaryOperands::new(a(tuple)?, b(tuple)?)? {
                        NumericBinaryOperands::Float(v1, v2) => {
                            ExpressionTerm::FloatLiteral(v1 / v2)
                        }
                        NumericBinaryOperands::Double(v1, v2) => {
                            ExpressionTerm::DoubleLiteral(v1 / v2)
                        }
                        NumericBinaryOperands::Integer(v1, v2) => {
                            ExpressionTerm::DecimalLiteral(Decimal::from(v1).checked_div(v2)?)
                        }
                        NumericBinaryOperands::Decimal(v1, v2) => {
                            ExpressionTerm::DecimalLiteral(v1.checked_div(v2)?)
                        }
                        #[cfg(feature = "sep-0002")]
                        _ => return None,
                    })
                })
            }
            Expression::UnaryPlus(e) => {
                let e = self.expression_evaluator(e, encoded_variables, stat_children);
                Rc::new(move |tuple| {
                    Some(match e(tuple)? {
                        ExpressionTerm::FloatLiteral(value) => ExpressionTerm::FloatLiteral(value),
                        ExpressionTerm::DoubleLiteral(value) => {
                            ExpressionTerm::DoubleLiteral(value)
                        }
                        ExpressionTerm::IntegerLiteral(value) => {
                            ExpressionTerm::IntegerLiteral(value)
                        }
                        ExpressionTerm::DecimalLiteral(value) => {
                            ExpressionTerm::DecimalLiteral(value)
                        }
                        #[cfg(feature = "sep-0002")]
                        ExpressionTerm::DurationLiteral(value) => {
                            ExpressionTerm::DurationLiteral(value)
                        }
                        #[cfg(feature = "sep-0002")]
                        ExpressionTerm::YearMonthDurationLiteral(value) => {
                            ExpressionTerm::YearMonthDurationLiteral(value)
                        }
                        #[cfg(feature = "sep-0002")]
                        ExpressionTerm::DayTimeDurationLiteral(value) => {
                            ExpressionTerm::DayTimeDurationLiteral(value)
                        }
                        _ => return None,
                    })
                })
            }
            Expression::UnaryMinus(e) => {
                let e = self.expression_evaluator(e, encoded_variables, stat_children);
                Rc::new(move |tuple| {
                    Some(match e(tuple)? {
                        ExpressionTerm::FloatLiteral(value) => ExpressionTerm::FloatLiteral(-value),
                        ExpressionTerm::DoubleLiteral(value) => {
                            ExpressionTerm::DoubleLiteral(-value)
                        }
                        ExpressionTerm::IntegerLiteral(value) => {
                            ExpressionTerm::IntegerLiteral(value.checked_neg()?)
                        }
                        ExpressionTerm::DecimalLiteral(value) => {
                            ExpressionTerm::DecimalLiteral(value.checked_neg()?)
                        }
                        #[cfg(feature = "sep-0002")]
                        ExpressionTerm::DurationLiteral(value) => {
                            ExpressionTerm::DurationLiteral(value.checked_neg()?)
                        }
                        #[cfg(feature = "sep-0002")]
                        ExpressionTerm::YearMonthDurationLiteral(value) => {
                            ExpressionTerm::YearMonthDurationLiteral(value.checked_neg()?)
                        }
                        #[cfg(feature = "sep-0002")]
                        ExpressionTerm::DayTimeDurationLiteral(value) => {
                            ExpressionTerm::DayTimeDurationLiteral(value.checked_neg()?)
                        }
                        _ => return None,
                    })
                })
            }
            Expression::Not(e) => {
                let e = self.effective_boolean_value_expression_evaluator(
                    e,
                    encoded_variables,
                    stat_children,
                );
                Rc::new(move |tuple| Some((!e(tuple)?).into()))
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
                let a = self.effective_boolean_value_expression_evaluator(
                    a,
                    encoded_variables,
                    stat_children,
                );
                let b = self.expression_evaluator(b, encoded_variables, stat_children);
                let c = self.expression_evaluator(c, encoded_variables, stat_children);
                Rc::new(move |tuple| if a(tuple)? { b(tuple) } else { c(tuple) })
            }
            Expression::FunctionCall(function, parameters) => match function {
                Function::Str => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        Some(ExpressionTerm::StringLiteral(match e(tuple)?.into() {
                            Term::NamedNode(term) => term.into_string(),
                            Term::BlankNode(_) => return None,
                            Term::Literal(term) => term.destruct().0,
                            #[cfg(feature = "rdf-star")]
                            Term::Triple(_) => return None,
                        }))
                    })
                }
                Function::Lang => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        Some(ExpressionTerm::StringLiteral(match e(tuple)? {
                            ExpressionTerm::LangStringLiteral { language, .. } => language,
                            #[cfg(feature = "sparql-12")]
                            ExpressionTerm::DirLangStringLiteral { language, .. } => language,
                            ExpressionTerm::NamedNode(_) | ExpressionTerm::BlankNode(_) => {
                                return None
                            }
                            #[cfg(feature = "rdf-star")]
                            ExpressionTerm::Triple(_) => return None,
                            _ => String::new(),
                        }))
                    })
                }
                Function::LangMatches => {
                    let language_tag =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    let language_range =
                        self.expression_evaluator(&parameters[1], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        let ExpressionTerm::StringLiteral(mut language_tag) = language_tag(tuple)?
                        else {
                            return None;
                        };
                        language_tag.make_ascii_lowercase();
                        let ExpressionTerm::StringLiteral(mut language_range) =
                            language_range(tuple)?
                        else {
                            return None;
                        };
                        language_range.make_ascii_lowercase();
                        Some(
                            if &*language_range == "*" {
                                !language_tag.is_empty()
                            } else {
                                !ZipLongest::new(language_range.split('-'), language_tag.split('-'))
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
                #[cfg(feature = "sparql-12")]
                Function::LangDir => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        Some(ExpressionTerm::StringLiteral(match e(tuple)? {
                            ExpressionTerm::DirLangStringLiteral { base_direction, .. } => {
                                match base_direction {
                                    BaseDirection::Ltr => "ltr".into(),
                                    BaseDirection::Rtl => "rtl".into(),
                                }
                            }
                            ExpressionTerm::NamedNode(_) | ExpressionTerm::BlankNode(_) => {
                                return None
                            }
                            #[cfg(feature = "rdf-star")]
                            ExpressionTerm::Triple(_) => return None,
                            _ => String::new(),
                        }))
                    })
                }
                Function::Datatype => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        Some(ExpressionTerm::NamedNode(match e(tuple)? {
                            ExpressionTerm::StringLiteral(_) => xsd::STRING.into(),
                            ExpressionTerm::LangStringLiteral { .. } => rdf::LANG_STRING.into(),
                            #[cfg(feature = "sparql-12")]
                            ExpressionTerm::DirLangStringLiteral { .. } => {
                                rdf::DIR_LANG_STRING.into()
                            }
                            ExpressionTerm::BooleanLiteral(_) => xsd::BOOLEAN.into(),
                            ExpressionTerm::IntegerLiteral(_) => xsd::INTEGER.into(),
                            ExpressionTerm::DecimalLiteral(_) => xsd::DECIMAL.into(),
                            ExpressionTerm::FloatLiteral(_) => xsd::FLOAT.into(),
                            ExpressionTerm::DoubleLiteral(_) => xsd::DOUBLE.into(),
                            ExpressionTerm::DateTimeLiteral(_) => xsd::DATE_TIME.into(),
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::DateLiteral(_) => xsd::DATE.into(),
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::TimeLiteral(_) => xsd::TIME.into(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GYearLiteral(_) => xsd::G_YEAR.into(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GYearMonthLiteral(_) => xsd::G_YEAR_MONTH.into(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GMonthLiteral(_) => xsd::G_MONTH.into(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GMonthDayLiteral(_) => xsd::G_MONTH_DAY.into(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GDayLiteral(_) => xsd::G_DAY.into(),
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::DurationLiteral(_) => xsd::DURATION.into(),
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::YearMonthDurationLiteral(_) => {
                                xsd::YEAR_MONTH_DURATION.into()
                            }
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::DayTimeDurationLiteral(_) => {
                                xsd::DAY_TIME_DURATION.into()
                            }
                            ExpressionTerm::OtherTypedLiteral { datatype, .. } => datatype,
                            ExpressionTerm::NamedNode(_) | ExpressionTerm::BlankNode(_) => {
                                return None
                            }
                            #[cfg(feature = "rdf-star")]
                            ExpressionTerm::Triple(_) => return None,
                        }))
                    })
                }
                Function::Iri => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    let base_iri = self.base_iri.clone();
                    Rc::new(move |tuple| {
                        Some(ExpressionTerm::NamedNode(match e(tuple)? {
                            ExpressionTerm::NamedNode(iri) => iri,
                            ExpressionTerm::StringLiteral(iri) => if let Some(base_iri) = &base_iri
                            {
                                base_iri.resolve(&iri)
                            } else {
                                Iri::parse(iri)
                            }
                            .ok()?
                            .into(),
                            _ => return None,
                        }))
                    })
                }
                Function::BNode => match parameters.first() {
                    Some(id) => {
                        let id = self.expression_evaluator(id, encoded_variables, stat_children);
                        Rc::new(move |tuple| {
                            let ExpressionTerm::StringLiteral(id) = id(tuple)? else {
                                return None;
                            };
                            Some(ExpressionTerm::BlankNode(BlankNode::new(id).ok()?))
                        })
                    }
                    None => Rc::new(|_| Some(ExpressionTerm::BlankNode(BlankNode::default()))),
                },
                Function::Rand => {
                    Rc::new(|_| Some(ExpressionTerm::DoubleLiteral(random::<f64>().into())))
                }
                Function::Abs => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| match e(tuple)? {
                        ExpressionTerm::IntegerLiteral(value) => {
                            Some(ExpressionTerm::IntegerLiteral(value.checked_abs()?))
                        }
                        ExpressionTerm::DecimalLiteral(value) => {
                            Some(ExpressionTerm::DecimalLiteral(value.checked_abs()?))
                        }
                        ExpressionTerm::FloatLiteral(value) => {
                            Some(ExpressionTerm::FloatLiteral(value.abs()))
                        }
                        ExpressionTerm::DoubleLiteral(value) => {
                            Some(ExpressionTerm::DoubleLiteral(value.abs()))
                        }
                        _ => None,
                    })
                }
                Function::Ceil => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| match e(tuple)? {
                        ExpressionTerm::IntegerLiteral(value) => {
                            Some(ExpressionTerm::IntegerLiteral(value))
                        }
                        ExpressionTerm::DecimalLiteral(value) => {
                            Some(ExpressionTerm::DecimalLiteral(value.checked_ceil()?))
                        }
                        ExpressionTerm::FloatLiteral(value) => {
                            Some(ExpressionTerm::FloatLiteral(value.ceil()))
                        }
                        ExpressionTerm::DoubleLiteral(value) => {
                            Some(ExpressionTerm::DoubleLiteral(value.ceil()))
                        }
                        _ => None,
                    })
                }
                Function::Floor => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| match e(tuple)? {
                        ExpressionTerm::IntegerLiteral(value) => {
                            Some(ExpressionTerm::IntegerLiteral(value))
                        }
                        ExpressionTerm::DecimalLiteral(value) => {
                            Some(ExpressionTerm::DecimalLiteral(value.checked_floor()?))
                        }
                        ExpressionTerm::FloatLiteral(value) => {
                            Some(ExpressionTerm::FloatLiteral(value.floor()))
                        }
                        ExpressionTerm::DoubleLiteral(value) => {
                            Some(ExpressionTerm::DoubleLiteral(value.floor()))
                        }
                        _ => None,
                    })
                }
                Function::Round => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| match e(tuple)? {
                        ExpressionTerm::IntegerLiteral(value) => {
                            Some(ExpressionTerm::IntegerLiteral(value))
                        }
                        ExpressionTerm::DecimalLiteral(value) => {
                            Some(ExpressionTerm::DecimalLiteral(value.checked_round()?))
                        }
                        ExpressionTerm::FloatLiteral(value) => {
                            Some(ExpressionTerm::FloatLiteral(value.round()))
                        }
                        ExpressionTerm::DoubleLiteral(value) => {
                            Some(ExpressionTerm::DoubleLiteral(value.round()))
                        }
                        _ => None,
                    })
                }
                Function::Concat => {
                    let l: Vec<_> = parameters
                        .iter()
                        .map(|e| self.expression_evaluator(e, encoded_variables, stat_children))
                        .collect();
                    Rc::new(move |tuple| {
                        let mut result = String::default();
                        let mut language = None;
                        for e in &l {
                            let (value, e_language) = to_string_and_language(e(tuple)?)?;
                            if let Some(lang) = &language {
                                if *lang != e_language {
                                    language = Some(None)
                                }
                            } else {
                                language = Some(e_language)
                            }
                            result += &value
                        }
                        Some(build_plain_literal(result, language.flatten()))
                    })
                }
                Function::SubStr => {
                    let source =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    let starting_loc =
                        self.expression_evaluator(&parameters[1], encoded_variables, stat_children);
                    let length = parameters
                        .get(2)
                        .map(|l| self.expression_evaluator(l, encoded_variables, stat_children));
                    Rc::new(move |tuple| {
                        let (source, language) = to_string_and_language(source(tuple)?)?;

                        let starting_location: usize =
                            if let ExpressionTerm::IntegerLiteral(v) = starting_loc(tuple)? {
                                usize::try_from(i64::from(v)).ok()?
                            } else {
                                return None;
                            };
                        let length = if let Some(length) = &length {
                            if let ExpressionTerm::IntegerLiteral(v) = length(tuple)? {
                                Some(usize::try_from(i64::from(v)).ok()?)
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
                        let result = if let Some((start_position, _)) = start_iter.peek().copied() {
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
                        Some(build_plain_literal(result.into(), language))
                    })
                }
                Function::StrLen => {
                    let arg =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        let (string, _) = to_string_and_language(arg(tuple)?)?;
                        Some(ExpressionTerm::IntegerLiteral(
                            i64::try_from(string.chars().count()).ok()?.into(),
                        ))
                    })
                }
                Function::Replace => {
                    let arg =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    let replacement =
                        self.expression_evaluator(&parameters[2], encoded_variables, stat_children);
                    if let Some(regex) =
                        compile_static_pattern_if_exists(&parameters[1], parameters.get(3))
                    {
                        Rc::new(move |tuple| {
                            let (text, language) = to_string_and_language(arg(tuple)?)?;
                            let ExpressionTerm::StringLiteral(replacement) = replacement(tuple)?
                            else {
                                return None;
                            };
                            Some(build_plain_literal(
                                match regex.replace_all(&text, &replacement) {
                                    Cow::Owned(replaced) => replaced,
                                    Cow::Borrowed(_) => text,
                                },
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
                            let ExpressionTerm::StringLiteral(pattern) = pattern(tuple)? else {
                                return None;
                            };
                            let options = if let Some(flags) = &flags {
                                let ExpressionTerm::StringLiteral(options) = flags(tuple)? else {
                                    return None;
                                };
                                Some(options)
                            } else {
                                None
                            };
                            let regex = compile_pattern(&pattern, options.as_deref())?;
                            let (text, language) = to_string_and_language(arg(tuple)?)?;
                            let ExpressionTerm::StringLiteral(replacement) = replacement(tuple)?
                            else {
                                return None;
                            };
                            Some(build_plain_literal(
                                match regex.replace_all(&text, &replacement) {
                                    Cow::Owned(replaced) => replaced,
                                    Cow::Borrowed(_) => text,
                                },
                                language,
                            ))
                        })
                    }
                }
                Function::UCase => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        let (value, language) = to_string_and_language(e(tuple)?)?;
                        Some(build_plain_literal(value.to_uppercase(), language))
                    })
                }
                Function::LCase => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        let (value, language) = to_string_and_language(e(tuple)?)?;
                        Some(build_plain_literal(value.to_lowercase(), language))
                    })
                }
                Function::StrStarts => {
                    let arg1 =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    let arg2 =
                        self.expression_evaluator(&parameters[1], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        let (arg1, arg2, _) =
                            to_argument_compatible_strings(arg1(tuple)?, arg2(tuple)?)?;
                        Some(arg1.starts_with(arg2.as_str()).into())
                    })
                }
                Function::EncodeForUri => {
                    let ltrl =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        let (ltlr, _) = to_string_and_language(ltrl(tuple)?)?;
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
                        Some(ExpressionTerm::StringLiteral(
                            String::from_utf8(result).ok()?,
                        ))
                    })
                }
                Function::StrEnds => {
                    let arg1 =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    let arg2 =
                        self.expression_evaluator(&parameters[1], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        let (arg1, arg2, _) =
                            to_argument_compatible_strings(arg1(tuple)?, arg2(tuple)?)?;
                        Some(arg1.ends_with(arg2.as_str()).into())
                    })
                }
                Function::Contains => {
                    let arg1 =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    let arg2 =
                        self.expression_evaluator(&parameters[1], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        let (arg1, arg2, _) =
                            to_argument_compatible_strings(arg1(tuple)?, arg2(tuple)?)?;
                        Some(arg1.contains(arg2.as_str()).into())
                    })
                }
                Function::StrBefore => {
                    let arg1 =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    let arg2 =
                        self.expression_evaluator(&parameters[1], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        let (arg1, arg2, language) =
                            to_argument_compatible_strings(arg1(tuple)?, arg2(tuple)?)?;
                        Some(if let Some(position) = arg1.find(arg2.as_str()) {
                            build_plain_literal(arg1[..position].into(), language)
                        } else {
                            ExpressionTerm::StringLiteral(String::new())
                        })
                    })
                }
                Function::StrAfter => {
                    let arg1 =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    let arg2 =
                        self.expression_evaluator(&parameters[1], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        let (arg1, arg2, language) =
                            to_argument_compatible_strings(arg1(tuple)?, arg2(tuple)?)?;
                        Some(if let Some(position) = arg1.find(arg2.as_str()) {
                            build_plain_literal(arg1[position + arg2.len()..].into(), language)
                        } else {
                            ExpressionTerm::StringLiteral(String::new())
                        })
                    })
                }
                Function::Year => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        Some(ExpressionTerm::IntegerLiteral(
                            match e(tuple)? {
                                ExpressionTerm::DateTimeLiteral(date_time) => date_time.year(),
                                #[cfg(feature = "sep-0002")]
                                ExpressionTerm::DateLiteral(date) => date.year(),
                                #[cfg(feature = "calendar-ext")]
                                ExpressionTerm::GYearMonthLiteral(year_month) => year_month.year(),
                                #[cfg(feature = "calendar-ext")]
                                ExpressionTerm::GYearLiteral(year) => year.year(),
                                _ => return None,
                            }
                            .into(),
                        ))
                    })
                }
                Function::Month => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        Some(ExpressionTerm::IntegerLiteral(
                            match e(tuple)? {
                                ExpressionTerm::DateTimeLiteral(date_time) => date_time.month(),
                                #[cfg(feature = "sep-0002")]
                                ExpressionTerm::DateLiteral(date) => date.month(),
                                #[cfg(feature = "calendar-ext")]
                                ExpressionTerm::GYearMonthLiteral(year_month) => year_month.month(),
                                #[cfg(feature = "calendar-ext")]
                                ExpressionTerm::GMonthDayLiteral(month_day) => month_day.month(),
                                #[cfg(feature = "calendar-ext")]
                                ExpressionTerm::GMonthLiteral(month) => month.month(),
                                _ => return None,
                            }
                            .into(),
                        ))
                    })
                }
                Function::Day => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        Some(ExpressionTerm::IntegerLiteral(
                            match e(tuple)? {
                                ExpressionTerm::DateTimeLiteral(date_time) => date_time.day(),
                                #[cfg(feature = "sep-0002")]
                                ExpressionTerm::DateLiteral(date) => date.day(),
                                #[cfg(feature = "calendar-ext")]
                                ExpressionTerm::GMonthDayLiteral(month_day) => month_day.day(),
                                #[cfg(feature = "calendar-ext")]
                                ExpressionTerm::GDayLiteral(day) => day.day(),
                                _ => return None,
                            }
                            .into(),
                        ))
                    })
                }
                Function::Hours => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        Some(ExpressionTerm::IntegerLiteral(
                            match e(tuple)? {
                                ExpressionTerm::DateTimeLiteral(date_time) => date_time.hour(),
                                #[cfg(feature = "sep-0002")]
                                ExpressionTerm::TimeLiteral(time) => time.hour(),
                                _ => return None,
                            }
                            .into(),
                        ))
                    })
                }
                Function::Minutes => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        Some(ExpressionTerm::IntegerLiteral(
                            match e(tuple)? {
                                ExpressionTerm::DateTimeLiteral(date_time) => date_time.minute(),
                                #[cfg(feature = "sep-0002")]
                                ExpressionTerm::TimeLiteral(time) => time.minute(),
                                _ => return None,
                            }
                            .into(),
                        ))
                    })
                }
                Function::Seconds => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        Some(ExpressionTerm::DecimalLiteral(match e(tuple)? {
                            ExpressionTerm::DateTimeLiteral(date_time) => date_time.second(),
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::TimeLiteral(time) => time.second(),
                            _ => return None,
                        }))
                    })
                }
                Function::Timezone => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        let result = match e(tuple)? {
                            ExpressionTerm::DateTimeLiteral(date_time) => date_time.timezone(),
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::TimeLiteral(time) => time.timezone(),
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::DateLiteral(date) => date.timezone(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GYearMonthLiteral(year_month) => year_month.timezone(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GYearLiteral(year) => year.timezone(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GMonthDayLiteral(month_day) => month_day.timezone(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GDayLiteral(day) => day.timezone(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GMonthLiteral(month) => month.timezone(),
                            _ => None,
                        }?;
                        #[cfg(feature = "sep-0002")]
                        {
                            Some(ExpressionTerm::DayTimeDurationLiteral(result))
                        }
                        #[cfg(not(feature = "sep-0002"))]
                        {
                            Some(ExpressionTerm::OtherTypedLiteral {
                                value: result.to_string(),
                                datatype: xsd::DAY_TIME_DURATION.into(),
                            })
                        }
                    })
                }
                Function::Tz => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        let timezone_offset = match e(tuple)? {
                            ExpressionTerm::DateTimeLiteral(date_time) => {
                                date_time.timezone_offset()
                            }
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::TimeLiteral(time) => time.timezone_offset(),
                            #[cfg(feature = "sep-0002")]
                            ExpressionTerm::DateLiteral(date) => date.timezone_offset(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GYearMonthLiteral(year_month) => {
                                year_month.timezone_offset()
                            }
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GYearLiteral(year) => year.timezone_offset(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GMonthDayLiteral(month_day) => {
                                month_day.timezone_offset()
                            }
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GDayLiteral(day) => day.timezone_offset(),
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GMonthLiteral(month) => month.timezone_offset(),
                            _ => return None,
                        };
                        Some(ExpressionTerm::StringLiteral(
                            timezone_offset.map_or_else(String::new, |o| o.to_string()),
                        ))
                    })
                }
                #[cfg(feature = "sep-0002")]
                Function::Adjust => {
                    let dt =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    let tz =
                        self.expression_evaluator(&parameters[1], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        let timezone_offset = Some(
                            match tz(tuple)? {
                                ExpressionTerm::DayTimeDurationLiteral(tz) => {
                                    TimezoneOffset::try_from(tz)
                                }
                                ExpressionTerm::DurationLiteral(tz) => TimezoneOffset::try_from(tz),
                                _ => return None,
                            }
                            .ok()?,
                        );
                        Some(match dt(tuple)? {
                            ExpressionTerm::DateTimeLiteral(date_time) => {
                                ExpressionTerm::DateTimeLiteral(date_time.adjust(timezone_offset)?)
                            }
                            ExpressionTerm::TimeLiteral(time) => {
                                ExpressionTerm::TimeLiteral(time.adjust(timezone_offset)?)
                            }
                            ExpressionTerm::DateLiteral(date) => {
                                ExpressionTerm::DateLiteral(date.adjust(timezone_offset)?)
                            }
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GYearMonthLiteral(year_month) => {
                                ExpressionTerm::GYearMonthLiteral(
                                    year_month.adjust(timezone_offset)?,
                                )
                            }
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GYearLiteral(year) => {
                                ExpressionTerm::GYearLiteral(year.adjust(timezone_offset)?)
                            }
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GMonthDayLiteral(month_day) => {
                                ExpressionTerm::GMonthDayLiteral(month_day.adjust(timezone_offset)?)
                            }
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GDayLiteral(day) => {
                                ExpressionTerm::GDayLiteral(day.adjust(timezone_offset)?)
                            }
                            #[cfg(feature = "calendar-ext")]
                            ExpressionTerm::GMonthLiteral(month) => {
                                ExpressionTerm::GMonthLiteral(month.adjust(timezone_offset)?)
                            }
                            _ => return None,
                        })
                    })
                }
                Function::Now => {
                    let now = self.now;
                    Rc::new(move |_| Some(ExpressionTerm::DateTimeLiteral(now)))
                }
                Function::Uuid => Rc::new(move |_| {
                    let mut buffer = String::with_capacity(44);
                    buffer.push_str("urn:uuid:");
                    generate_uuid(&mut buffer);
                    Some(ExpressionTerm::NamedNode(NamedNode::new_unchecked(buffer)))
                }),
                Function::StrUuid => Rc::new(move |_| {
                    let mut buffer = String::with_capacity(36);
                    generate_uuid(&mut buffer);
                    Some(ExpressionTerm::StringLiteral(buffer))
                }),
                Function::Md5 => self.hash::<Md5>(parameters, encoded_variables, stat_children),
                Function::Sha1 => self.hash::<Sha1>(parameters, encoded_variables, stat_children),
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
                    let lexical_form =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    let lang_tag =
                        self.expression_evaluator(&parameters[1], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        let ExpressionTerm::StringLiteral(value) = lexical_form(tuple)? else {
                            return None;
                        };
                        let ExpressionTerm::StringLiteral(language) = lang_tag(tuple)? else {
                            return None;
                        };
                        Some(
                            Term::from(Literal::new_language_tagged_literal(value, language).ok()?)
                                .into(),
                        )
                    })
                }
                #[cfg(feature = "sparql-12")]
                Function::StrLangDir => {
                    let lexical_form =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    let lang_tag =
                        self.expression_evaluator(&parameters[1], encoded_variables, stat_children);
                    let base_direction =
                        self.expression_evaluator(&parameters[2], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        let ExpressionTerm::StringLiteral(value) = lexical_form(tuple)? else {
                            return None;
                        };
                        let ExpressionTerm::StringLiteral(language) = lang_tag(tuple)? else {
                            return None;
                        };
                        let ExpressionTerm::StringLiteral(base_direction) = base_direction(tuple)?
                        else {
                            return None;
                        };
                        let base_direction = match base_direction.as_str() {
                            "ltr" => BaseDirection::Ltr,
                            "rtl" => BaseDirection::Rtl,
                            _ => return None,
                        };
                        Some(
                            Term::from(
                                Literal::new_directional_language_tagged_literal(
                                    value,
                                    language,
                                    base_direction,
                                )
                                .ok()?,
                            )
                            .into(),
                        )
                    })
                }
                Function::StrDt => {
                    let lexical_form =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    let datatype =
                        self.expression_evaluator(&parameters[1], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        let ExpressionTerm::StringLiteral(value) = lexical_form(tuple)? else {
                            return None;
                        };
                        let ExpressionTerm::NamedNode(datatype) = datatype(tuple)? else {
                            return None;
                        };
                        Some(Term::from(Literal::new_typed_literal(value, datatype)).into())
                    })
                }

                Function::IsIri => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        Some(matches!(e(tuple)?, ExpressionTerm::NamedNode(_)).into())
                    })
                }
                Function::IsBlank => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        Some(matches!(e(tuple)?, ExpressionTerm::BlankNode(_)).into())
                    })
                }
                Function::IsLiteral => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        Some(
                            match e(tuple)? {
                                ExpressionTerm::NamedNode(_) | ExpressionTerm::BlankNode(_) => {
                                    false
                                }
                                #[cfg(feature = "rdf-star")]
                                ExpressionTerm::Triple(_) => false,
                                _ => true,
                            }
                            .into(),
                        )
                    })
                }
                Function::IsNumeric => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        Some(
                            matches!(
                                e(tuple)?,
                                ExpressionTerm::IntegerLiteral(_)
                                    | ExpressionTerm::DecimalLiteral(_)
                                    | ExpressionTerm::FloatLiteral(_)
                                    | ExpressionTerm::DoubleLiteral(_)
                            )
                            .into(),
                        )
                    })
                }
                #[cfg(feature = "sparql-12")]
                Function::HasLang => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        Some(
                            matches!(
                                e(tuple)?,
                                ExpressionTerm::LangStringLiteral { .. }
                                    | ExpressionTerm::DirLangStringLiteral { .. }
                            )
                            .into(),
                        )
                    })
                }
                #[cfg(feature = "sparql-12")]
                Function::HasLangDir => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        Some(
                            matches!(e(tuple)?, ExpressionTerm::DirLangStringLiteral { .. }).into(),
                        )
                    })
                }
                Function::Regex => {
                    let text =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    if let Some(regex) =
                        compile_static_pattern_if_exists(&parameters[1], parameters.get(2))
                    {
                        Rc::new(move |tuple| {
                            let (text, _) = to_string_and_language(text(tuple)?)?;
                            Some(regex.is_match(&text).into())
                        })
                    } else {
                        let pattern = self.expression_evaluator(
                            &parameters[1],
                            encoded_variables,
                            stat_children,
                        );
                        let flags = parameters.get(2).map(|flags| {
                            self.expression_evaluator(flags, encoded_variables, stat_children)
                        });
                        Rc::new(move |tuple| {
                            let ExpressionTerm::StringLiteral(pattern) = pattern(tuple)? else {
                                return None;
                            };
                            let options = if let Some(flags) = &flags {
                                let ExpressionTerm::StringLiteral(options) = flags(tuple)? else {
                                    return None;
                                };
                                Some(options)
                            } else {
                                None
                            };
                            let regex = compile_pattern(&pattern, options.as_deref())?;
                            let (text, _) = to_string_and_language(text(tuple)?)?;
                            Some(regex.is_match(&text).into())
                        })
                    }
                }
                #[cfg(feature = "rdf-star")]
                Function::Triple => {
                    let s =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    let p =
                        self.expression_evaluator(&parameters[1], encoded_variables, stat_children);
                    let o =
                        self.expression_evaluator(&parameters[2], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        Some(ExpressionTriple::new(s(tuple)?, p(tuple)?, o(tuple)?)?.into())
                    })
                }
                #[cfg(feature = "rdf-star")]
                Function::Subject => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        if let ExpressionTerm::Triple(t) = e(tuple)? {
                            Some(t.subject.into())
                        } else {
                            None
                        }
                    })
                }
                #[cfg(feature = "rdf-star")]
                Function::Predicate => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        if let ExpressionTerm::Triple(t) = e(tuple)? {
                            Some(t.predicate.into())
                        } else {
                            None
                        }
                    })
                }
                #[cfg(feature = "rdf-star")]
                Function::Object => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        if let ExpressionTerm::Triple(t) = e(tuple)? {
                            Some(t.object)
                        } else {
                            None
                        }
                    })
                }
                #[cfg(feature = "rdf-star")]
                Function::IsTriple => {
                    let e =
                        self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
                    Rc::new(move |tuple| {
                        Some(matches!(e(tuple)?, ExpressionTerm::Triple(_)).into())
                    })
                }
                Function::Custom(function_name) => {
                    if let Some(function) = self.custom_functions.get(function_name).cloned() {
                        let args = parameters
                            .iter()
                            .map(|e| self.expression_evaluator(e, encoded_variables, stat_children))
                            .collect::<Vec<_>>();
                        return Rc::new(move |tuple| {
                            let args = args
                                .iter()
                                .map(|f| Some(f(tuple)?.into()))
                                .collect::<Option<Vec<Term>>>()?;
                            Some(function(&args)?.into())
                        });
                    }
                    match function_name.as_ref() {
                        xsd::STRING => {
                            let e = self.expression_evaluator(
                                &parameters[0],
                                encoded_variables,
                                stat_children,
                            );
                            Rc::new(move |tuple| {
                                Some(ExpressionTerm::StringLiteral(match e(tuple)?.into() {
                                    Term::NamedNode(term) => term.into_string(),
                                    Term::BlankNode(_) => return None,
                                    Term::Literal(term) => term.destruct().0,
                                    #[cfg(feature = "rdf-star")]
                                    Term::Triple(_) => return None,
                                }))
                            })
                        }
                        xsd::BOOLEAN => {
                            let e = self.expression_evaluator(
                                &parameters[0],
                                encoded_variables,
                                stat_children,
                            );
                            Rc::new(move |tuple| {
                                Some(ExpressionTerm::BooleanLiteral(match e(tuple)? {
                                    ExpressionTerm::BooleanLiteral(value) => value,
                                    ExpressionTerm::FloatLiteral(value) => value.into(),
                                    ExpressionTerm::DoubleLiteral(value) => value.into(),
                                    ExpressionTerm::IntegerLiteral(value) => value.into(),
                                    ExpressionTerm::DecimalLiteral(value) => value.into(),
                                    ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                                    _ => return None,
                                }))
                            })
                        }
                        xsd::DOUBLE => {
                            let e = self.expression_evaluator(
                                &parameters[0],
                                encoded_variables,
                                stat_children,
                            );
                            Rc::new(move |tuple| {
                                Some(ExpressionTerm::DoubleLiteral(match e(tuple)? {
                                    ExpressionTerm::FloatLiteral(value) => value.into(),
                                    ExpressionTerm::DoubleLiteral(value) => value,
                                    ExpressionTerm::IntegerLiteral(value) => value.into(),
                                    ExpressionTerm::DecimalLiteral(value) => value.into(),
                                    ExpressionTerm::BooleanLiteral(value) => value.into(),
                                    ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                                    _ => return None,
                                }))
                            })
                        }
                        xsd::FLOAT => {
                            let e = self.expression_evaluator(
                                &parameters[0],
                                encoded_variables,
                                stat_children,
                            );
                            Rc::new(move |tuple| {
                                Some(ExpressionTerm::FloatLiteral(match e(tuple)? {
                                    ExpressionTerm::FloatLiteral(value) => value,
                                    ExpressionTerm::DoubleLiteral(value) => value.into(),
                                    ExpressionTerm::IntegerLiteral(value) => value.into(),
                                    ExpressionTerm::DecimalLiteral(value) => value.into(),
                                    ExpressionTerm::BooleanLiteral(value) => value.into(),
                                    ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                                    _ => return None,
                                }))
                            })
                        }
                        xsd::INTEGER => {
                            let e = self.expression_evaluator(
                                &parameters[0],
                                encoded_variables,
                                stat_children,
                            );
                            Rc::new(move |tuple| {
                                Some(ExpressionTerm::IntegerLiteral(match e(tuple)? {
                                    ExpressionTerm::FloatLiteral(value) => value.try_into().ok()?,
                                    ExpressionTerm::DoubleLiteral(value) => {
                                        value.try_into().ok()?
                                    }
                                    ExpressionTerm::IntegerLiteral(value) => value,
                                    ExpressionTerm::DecimalLiteral(value) => {
                                        value.try_into().ok()?
                                    }
                                    ExpressionTerm::BooleanLiteral(value) => value.into(),
                                    ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                                    _ => return None,
                                }))
                            })
                        }
                        xsd::DECIMAL => {
                            let e = self.expression_evaluator(
                                &parameters[0],
                                encoded_variables,
                                stat_children,
                            );
                            Rc::new(move |tuple| {
                                Some(ExpressionTerm::DecimalLiteral(match e(tuple)? {
                                    ExpressionTerm::FloatLiteral(value) => value.try_into().ok()?,
                                    ExpressionTerm::DoubleLiteral(value) => {
                                        value.try_into().ok()?
                                    }
                                    ExpressionTerm::IntegerLiteral(value) => value.into(),
                                    ExpressionTerm::DecimalLiteral(value) => value,
                                    ExpressionTerm::BooleanLiteral(value) => value.into(),
                                    ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                                    _ => return None,
                                }))
                            })
                        }
                        #[cfg(feature = "sep-0002")]
                        xsd::DATE => {
                            let e = self.expression_evaluator(
                                &parameters[0],
                                encoded_variables,
                                stat_children,
                            );
                            Rc::new(move |tuple| {
                                Some(ExpressionTerm::DateLiteral(match e(tuple)? {
                                    ExpressionTerm::DateLiteral(value) => value,
                                    ExpressionTerm::DateTimeLiteral(value) => {
                                        value.try_into().ok()?
                                    }
                                    ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                                    _ => return None,
                                }))
                            })
                        }
                        #[cfg(feature = "sep-0002")]
                        xsd::TIME => {
                            let e = self.expression_evaluator(
                                &parameters[0],
                                encoded_variables,
                                stat_children,
                            );
                            Rc::new(move |tuple| {
                                Some(ExpressionTerm::TimeLiteral(match e(tuple)? {
                                    ExpressionTerm::TimeLiteral(value) => value,
                                    ExpressionTerm::DateTimeLiteral(value) => value.into(),
                                    ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                                    _ => return None,
                                }))
                            })
                        }
                        xsd::DATE_TIME => {
                            let e = self.expression_evaluator(
                                &parameters[0],
                                encoded_variables,
                                stat_children,
                            );
                            Rc::new(move |tuple| {
                                Some(ExpressionTerm::DateTimeLiteral(match e(tuple)? {
                                    ExpressionTerm::DateTimeLiteral(value) => value,
                                    #[cfg(feature = "sep-0002")]
                                    ExpressionTerm::DateLiteral(value) => value.try_into().ok()?,
                                    ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                                    _ => return None,
                                }))
                            })
                        }
                        #[cfg(feature = "sep-0002")]
                        xsd::DURATION => {
                            let e = self.expression_evaluator(
                                &parameters[0],
                                encoded_variables,
                                stat_children,
                            );
                            Rc::new(move |tuple| {
                                Some(ExpressionTerm::DurationLiteral(match e(tuple)? {
                                    ExpressionTerm::DurationLiteral(value) => value,
                                    ExpressionTerm::YearMonthDurationLiteral(value) => value.into(),
                                    ExpressionTerm::DayTimeDurationLiteral(value) => value.into(),
                                    ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                                    _ => return None,
                                }))
                            })
                        }
                        #[cfg(feature = "sep-0002")]
                        xsd::YEAR_MONTH_DURATION => {
                            let e = self.expression_evaluator(
                                &parameters[0],
                                encoded_variables,
                                stat_children,
                            );
                            Rc::new(move |tuple| {
                                Some(ExpressionTerm::YearMonthDurationLiteral(match e(tuple)? {
                                    ExpressionTerm::DurationLiteral(value) => {
                                        value.try_into().ok()?
                                    }
                                    ExpressionTerm::YearMonthDurationLiteral(value) => value,
                                    ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                                    _ => return None,
                                }))
                            })
                        }
                        #[cfg(feature = "sep-0002")]
                        xsd::DAY_TIME_DURATION => {
                            let e = self.expression_evaluator(
                                &parameters[0],
                                encoded_variables,
                                stat_children,
                            );
                            Rc::new(move |tuple| {
                                Some(ExpressionTerm::DayTimeDurationLiteral(match e(tuple)? {
                                    ExpressionTerm::DurationLiteral(value) => {
                                        value.try_into().ok()?
                                    }
                                    ExpressionTerm::DayTimeDurationLiteral(value) => value,
                                    ExpressionTerm::StringLiteral(value) => value.parse().ok()?,
                                    _ => return None,
                                }))
                            })
                        }
                        // TODO: gYear...
                        _ => Rc::new(|_| None),
                    }
                }
            },
        }
    }

    fn hash<H: Digest>(
        &self,
        parameters: &[Expression],
        encoded_variables: &mut Vec<Variable>,
        stat_children: &mut Vec<Rc<EvalNodeWithStats>>,
    ) -> Rc<dyn Fn(&InternalTuple<D>) -> Option<ExpressionTerm>> {
        let arg = self.expression_evaluator(&parameters[0], encoded_variables, stat_children);
        Rc::new(move |tuple| {
            let ExpressionTerm::StringLiteral(input) = arg(tuple)? else {
                return None;
            };
            let hash = hex::encode(H::new().chain_update(input.as_str()).finalize());
            Some(ExpressionTerm::StringLiteral(hash))
        })
    }

    fn encode_term(&self, term: impl Into<Term>) -> Result<D::InternalTerm, QueryEvaluationError> {
        self.dataset.internalize_term(term.into())
    }

    #[cfg(feature = "rdf-star")]
    fn encode_triple(
        &self,
        triple: &GroundTriple,
    ) -> Result<D::InternalTerm, QueryEvaluationError> {
        self.dataset.internalize_expression_term(
            ExpressionTriple::from(Triple::from(triple.clone())).into(),
        )
    }

    fn encode_property_path(
        &self,
        path: &PropertyPathExpression,
    ) -> Result<Rc<PropertyPath<D>>, QueryEvaluationError> {
        Ok(Rc::new(match path {
            PropertyPathExpression::NamedNode(node) => {
                PropertyPath::Path(self.encode_term(node.clone())?)
            }
            PropertyPathExpression::Reverse(p) => {
                PropertyPath::Reverse(self.encode_property_path(p)?)
            }
            PropertyPathExpression::Sequence(a, b) => {
                PropertyPath::Sequence(self.encode_property_path(a)?, self.encode_property_path(b)?)
            }
            PropertyPathExpression::Alternative(a, b) => PropertyPath::Alternative(
                self.encode_property_path(a)?,
                self.encode_property_path(b)?,
            ),
            PropertyPathExpression::ZeroOrMore(p) => {
                PropertyPath::ZeroOrMore(self.encode_property_path(p)?)
            }
            PropertyPathExpression::OneOrMore(p) => {
                PropertyPath::OneOrMore(self.encode_property_path(p)?)
            }
            PropertyPathExpression::ZeroOrOne(p) => {
                PropertyPath::ZeroOrOne(self.encode_property_path(p)?)
            }
            PropertyPathExpression::NegatedPropertySet(ps) => PropertyPath::NegatedPropertySet(
                ps.iter()
                    .map(|p| self.encode_term(p.clone()))
                    .collect::<Result<Rc<[_]>, _>>()?,
            ),
        }))
    }
}

impl<D: QueryableDataset> Clone for SimpleEvaluator<D> {
    fn clone(&self) -> Self {
        Self {
            dataset: self.dataset.clone(),
            base_iri: self.base_iri.clone(),
            now: self.now,
            service_handler: Rc::clone(&self.service_handler),
            custom_functions: Rc::clone(&self.custom_functions),
            run_stats: self.run_stats,
        }
    }
}

#[cfg(feature = "sparql-12")]
type LanguageWithMaybeBaseDirection = (String, Option<BaseDirection>);
#[cfg(not(feature = "sparql-12"))]
type LanguageWithMaybeBaseDirection = String;

#[cfg(feature = "sparql-12")]
fn to_string_and_language(
    term: ExpressionTerm,
) -> Option<(String, Option<LanguageWithMaybeBaseDirection>)> {
    match term {
        ExpressionTerm::StringLiteral(value) => Some((value, None)),
        ExpressionTerm::LangStringLiteral { value, language } => {
            Some((value, Some((language, None))))
        }
        ExpressionTerm::DirLangStringLiteral {
            value,
            language,
            base_direction,
        } => Some((value, Some((language, Some(base_direction))))),
        _ => None,
    }
}

#[cfg(not(feature = "sparql-12"))]
fn to_string_and_language(
    term: ExpressionTerm,
) -> Option<(String, Option<LanguageWithMaybeBaseDirection>)> {
    match term {
        ExpressionTerm::StringLiteral(value) => Some((value, None)),
        ExpressionTerm::LangStringLiteral { value, language } => Some((value, Some(language))),
        _ => None,
    }
}

#[cfg(feature = "sparql-12")]
fn build_plain_literal(
    value: String,
    language: Option<LanguageWithMaybeBaseDirection>,
) -> ExpressionTerm {
    if let Some((language, base_direction)) = language {
        if let Some(base_direction) = base_direction {
            ExpressionTerm::DirLangStringLiteral {
                value,
                language,
                base_direction,
            }
        } else {
            ExpressionTerm::LangStringLiteral { value, language }
        }
    } else {
        ExpressionTerm::StringLiteral(value)
    }
}

#[cfg(not(feature = "sparql-12"))]
fn build_plain_literal(
    value: String,
    language: Option<LanguageWithMaybeBaseDirection>,
) -> ExpressionTerm {
    if let Some(language) = language {
        ExpressionTerm::LangStringLiteral { value, language }
    } else {
        ExpressionTerm::StringLiteral(value)
    }
}

fn to_argument_compatible_strings(
    arg1: ExpressionTerm,
    arg2: ExpressionTerm,
) -> Option<(String, String, Option<LanguageWithMaybeBaseDirection>)> {
    let (value1, language1) = to_string_and_language(arg1)?;
    let (value2, language2) = to_string_and_language(arg2)?;
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

fn compile_pattern(pattern: &str, flags: Option<&str>) -> Option<Regex> {
    let mut pattern = Cow::Borrowed(pattern);
    let flags = flags.unwrap_or_default();
    if flags.contains('q') {
        pattern = regex::escape(&pattern).into();
    }
    let mut regex_builder = RegexBuilder::new(&pattern);
    regex_builder.size_limit(REGEX_SIZE_LIMIT);
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
            'q' => (),        // Already supported
            _ => return None, // invalid option
        }
    }
    regex_builder.build().ok()
}

fn decode_bindings<D: QueryableDataset>(
    dataset: EvalDataset<D>,
    iter: InternalTuplesIterator<D>,
    variables: Arc<[Variable]>,
) -> QuerySolutionIter {
    let tuple_size = variables.len();
    QuerySolutionIter::new(
        Arc::clone(&variables),
        Box::new(iter.map(move |values| {
            let mut result = vec![None; tuple_size];
            for (i, value) in values?.iter().enumerate() {
                if let Some(term) = value {
                    result[i] = Some(dataset.externalize_term(term)?)
                }
            }
            Ok((Arc::clone(&variables), result).into())
        })),
    )
}

// this is used to encode results from a BindingIterator into an InternalTuplesIterator. This happens when SERVICE clauses are evaluated
fn encode_bindings<D: QueryableDataset>(
    dataset: EvalDataset<D>,
    variables: Rc<[Variable]>,
    iter: QuerySolutionIter,
) -> InternalTuplesIterator<D> {
    Box::new(iter.map(move |solution| {
        let mut encoded_terms = InternalTuple::with_capacity(variables.len());
        for (variable, term) in &solution? {
            put_variable_value(
                variable,
                &variables,
                dataset.internalize_term(term.clone())?,
                &mut encoded_terms,
            );
        }
        Ok(encoded_terms)
    }))
}

fn encode_initial_bindings<D: QueryableDataset>(
    dataset: &EvalDataset<D>,
    variables: &[Variable],
    values: impl IntoIterator<Item = (Variable, Term)>,
) -> Result<InternalTuple<D>, QueryEvaluationError> {
    let mut encoded_terms = InternalTuple::with_capacity(variables.len());
    for (variable, term) in values {
        if !put_variable_value(
            &variable,
            variables,
            dataset.internalize_term(term)?,
            &mut encoded_terms,
        ) {
            return Err(QueryEvaluationError::NotExistingSubstitutedVariable(
                variable,
            ));
        }
    }
    Ok(encoded_terms)
}

fn put_variable_value<D: QueryableDataset>(
    selector: &Variable,
    variables: &[Variable],
    value: D::InternalTerm,
    tuple: &mut InternalTuple<D>,
) -> bool {
    for (i, v) in variables.iter().enumerate() {
        if selector == v {
            tuple.set(i, value);
            return true;
        }
    }
    false
}

enum AccumulatorWrapper<D: QueryableDataset> {
    CountTuple {
        count: u64,
    },
    CountDistinctTuple {
        seen: FxHashSet<InternalTuple<D>>,
        count: u64,
    },
    CountInternal {
        evaluator: Rc<dyn Fn(&InternalTuple<D>) -> Option<D::InternalTerm>>,
        count: u64,
    },
    CountDistinctInternal {
        seen: FxHashSet<D::InternalTerm>,
        evaluator: Rc<dyn Fn(&InternalTuple<D>) -> Option<D::InternalTerm>>,
        count: u64,
    },
    Sample {
        // TODO: add internal variant
        evaluator: Rc<dyn Fn(&InternalTuple<D>) -> Option<ExpressionTerm>>,
        value: Option<ExpressionTerm>,
    },
    Expression {
        evaluator: Rc<dyn Fn(&InternalTuple<D>) -> Option<ExpressionTerm>>,
        accumulator: Option<Box<dyn Accumulator>>,
    },
    DistinctExpression {
        seen: FxHashSet<ExpressionTerm>,
        evaluator: Rc<dyn Fn(&InternalTuple<D>) -> Option<ExpressionTerm>>,
        accumulator: Option<Box<dyn Accumulator>>,
    },
    Failing,
}

impl<D: QueryableDataset> AccumulatorWrapper<D> {
    fn add(&mut self, tuple: &InternalTuple<D>) {
        match self {
            Self::CountTuple { count } => {
                *count += 1;
            }
            Self::CountDistinctTuple { seen, count } => {
                if seen.insert(tuple.clone()) {
                    *count += 1;
                }
            }
            Self::CountInternal { evaluator, count } => {
                if evaluator(tuple).is_some() {
                    *count += 1;
                };
            }
            Self::CountDistinctInternal {
                seen,
                evaluator,
                count,
            } => {
                let Some(value) = evaluator(tuple) else {
                    return;
                };
                if seen.insert(value) {
                    *count += 1;
                }
            }
            Self::Sample { evaluator, value } => {
                if value.is_some() {
                    return; // We already got a value
                }
                *value = evaluator(tuple);
            }
            Self::Expression {
                evaluator,
                accumulator,
            } => {
                if accumulator.is_none() {
                    return; // Already failed
                }
                let Some(value) = evaluator(tuple) else {
                    *accumulator = None;
                    return;
                };
                let Some(accumulator) = accumulator else {
                    return;
                };
                accumulator.add(value);
            }
            Self::DistinctExpression {
                seen,
                evaluator,
                accumulator,
            } => {
                if accumulator.is_none() {
                    return; // Already failed
                }
                let Some(value) = evaluator(tuple) else {
                    *accumulator = None;
                    return;
                };
                let Some(accumulator) = accumulator else {
                    return;
                };
                if seen.insert(value.clone()) {
                    accumulator.add(value);
                }
            }
            Self::Failing => (),
        }
    }

    fn finish(self) -> Option<ExpressionTerm> {
        match self {
            Self::CountTuple { count, .. }
            | Self::CountDistinctTuple { count, .. }
            | Self::CountInternal { count, .. }
            | Self::CountDistinctInternal { count, .. } => Some(ExpressionTerm::IntegerLiteral(
                i64::try_from(count).ok()?.into(),
            )),
            Self::Sample { value, .. } => value,
            Self::Expression { accumulator, .. } | Self::DistinctExpression { accumulator, .. } => {
                accumulator?.finish()
            }
            Self::Failing => None,
        }
    }
}

trait Accumulator {
    fn add(&mut self, element: ExpressionTerm);

    fn finish(&mut self) -> Option<ExpressionTerm>;
}

#[derive(Default, Debug)]
struct CountAccumulator {
    count: i64,
}

impl Accumulator for CountAccumulator {
    fn add(&mut self, _element: ExpressionTerm) {
        self.count += 1;
    }

    fn finish(&mut self) -> Option<ExpressionTerm> {
        Some(ExpressionTerm::IntegerLiteral(self.count.into()))
    }
}

struct SumAccumulator {
    sum: Option<ExpressionTerm>,
}

impl Default for SumAccumulator {
    fn default() -> Self {
        Self {
            sum: Some(ExpressionTerm::IntegerLiteral(Integer::default())),
        }
    }
}

impl Accumulator for SumAccumulator {
    fn add(&mut self, element: ExpressionTerm) {
        let Some(sum) = &self.sum else {
            return;
        };
        self.sum = if let Some(operands) = NumericBinaryOperands::new(sum.clone(), element) {
            // TODO: unify with addition?
            match operands {
                NumericBinaryOperands::Float(v1, v2) => Some(ExpressionTerm::FloatLiteral(v1 + v2)),
                NumericBinaryOperands::Double(v1, v2) => {
                    Some(ExpressionTerm::DoubleLiteral(v1 + v2))
                }
                NumericBinaryOperands::Integer(v1, v2) => {
                    v1.checked_add(v2).map(ExpressionTerm::IntegerLiteral)
                }
                NumericBinaryOperands::Decimal(v1, v2) => {
                    v1.checked_add(v2).map(ExpressionTerm::DecimalLiteral)
                }
                #[cfg(feature = "sep-0002")]
                _ => None,
            }
        } else {
            None
        };
    }

    fn finish(&mut self) -> Option<ExpressionTerm> {
        self.sum.take()
    }
}

#[derive(Default)]
struct AvgAccumulator {
    sum: SumAccumulator,
    count: i64,
}

impl Accumulator for AvgAccumulator {
    fn add(&mut self, element: ExpressionTerm) {
        self.sum.add(element);
        self.count += 1;
    }

    fn finish(&mut self) -> Option<ExpressionTerm> {
        let sum = self.sum.finish()?;
        if self.count == 0 {
            return Some(ExpressionTerm::IntegerLiteral(0.into()));
        }
        // TODO: duration?
        let count = Integer::from(self.count);
        match sum {
            ExpressionTerm::FloatLiteral(sum) => {
                Some(ExpressionTerm::FloatLiteral(sum / Float::from(count)))
            }
            ExpressionTerm::DoubleLiteral(sum) => {
                Some(ExpressionTerm::DoubleLiteral(sum / Double::from(count)))
            }
            ExpressionTerm::IntegerLiteral(sum) => Some(ExpressionTerm::DecimalLiteral(
                Decimal::from(sum).checked_div(count)?,
            )),
            ExpressionTerm::DecimalLiteral(sum) => {
                Some(ExpressionTerm::DecimalLiteral(sum.checked_div(count)?))
            }
            _ => None,
        }
    }
}

#[derive(Default)]
#[allow(clippy::option_option)]
struct MinAccumulator {
    min: Option<Option<ExpressionTerm>>,
}

impl Accumulator for MinAccumulator {
    fn add(&mut self, element: ExpressionTerm) {
        if let Some(min) = &self.min {
            if cmp_terms(Some(&element), min.as_ref()) == Ordering::Less {
                self.min = Some(Some(element));
            }
        } else {
            self.min = Some(Some(element))
        }
    }

    fn finish(&mut self) -> Option<ExpressionTerm> {
        self.min.clone().and_then(|v| v)
    }
}

#[derive(Default)]
#[allow(clippy::option_option)]
struct MaxAccumulator {
    max: Option<Option<ExpressionTerm>>,
}

impl Accumulator for MaxAccumulator {
    fn add(&mut self, element: ExpressionTerm) {
        if let Some(max) = &self.max {
            if cmp_terms(Some(&element), max.as_ref()) == Ordering::Greater {
                self.max = Some(Some(element))
            }
        } else {
            self.max = Some(Some(element))
        }
    }

    fn finish(&mut self) -> Option<ExpressionTerm> {
        self.max.clone().and_then(|v| v)
    }
}

#[allow(clippy::option_option)]
struct GroupConcatAccumulator {
    concat: Option<String>,
    language: Option<Option<LanguageWithMaybeBaseDirection>>,
    separator: Rc<str>,
}

impl GroupConcatAccumulator {
    fn new(separator: Rc<str>) -> Self {
        Self {
            concat: Some(String::new()),
            language: None,
            separator,
        }
    }
}

impl Accumulator for GroupConcatAccumulator {
    fn add(&mut self, element: ExpressionTerm) {
        let Some(concat) = self.concat.as_mut() else {
            return;
        };
        let Some((value, e_language)) = to_string_and_language(element) else {
            self.concat = None;
            return;
        };
        if let Some(lang) = &self.language {
            if *lang != e_language {
                self.language = Some(None)
            }
            concat.push_str(&self.separator);
        } else {
            self.language = Some(e_language)
        }
        concat.push_str(&value);
    }

    fn finish(&mut self) -> Option<ExpressionTerm> {
        self.concat
            .take()
            .map(|result| build_plain_literal(result, self.language.take().flatten()))
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

/// Equality operator (=)
fn equals(a: &ExpressionTerm, b: &ExpressionTerm) -> Option<bool> {
    match a {
        ExpressionTerm::NamedNode(_)
        | ExpressionTerm::BlankNode(_)
        | ExpressionTerm::LangStringLiteral { .. } => Some(a == b),
        #[cfg(feature = "sparql-12")]
        ExpressionTerm::DirLangStringLiteral { .. } => Some(a == b),
        ExpressionTerm::StringLiteral(a) => match b {
            ExpressionTerm::StringLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::OtherTypedLiteral { .. } => match b {
            ExpressionTerm::OtherTypedLiteral { .. } if a == b => Some(true),
            ExpressionTerm::NamedNode(_)
            | ExpressionTerm::BlankNode(_)
            | ExpressionTerm::LangStringLiteral { .. } => Some(false),
            #[cfg(feature = "sparql-12")]
            ExpressionTerm::DirLangStringLiteral { .. } => Some(false),
            #[cfg(feature = "rdf-star")]
            ExpressionTerm::Triple(_) => Some(false),
            _ => None,
        },
        ExpressionTerm::BooleanLiteral(a) => match b {
            ExpressionTerm::BooleanLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::FloatLiteral(a) => match b {
            ExpressionTerm::FloatLiteral(b) => Some(a == b),
            ExpressionTerm::DoubleLiteral(b) => Some(Double::from(*a) == *b),
            ExpressionTerm::IntegerLiteral(b) => Some(*a == (*b).into()),
            ExpressionTerm::DecimalLiteral(b) => Some(*a == (*b).into()),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::DoubleLiteral(a) => match b {
            ExpressionTerm::FloatLiteral(b) => Some(*a == (*b).into()),
            ExpressionTerm::DoubleLiteral(b) => Some(a == b),
            ExpressionTerm::IntegerLiteral(b) => Some(*a == (*b).into()),
            ExpressionTerm::DecimalLiteral(b) => Some(*a == (*b).into()),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::IntegerLiteral(a) => match b {
            ExpressionTerm::FloatLiteral(b) => Some(Float::from(*a) == *b),
            ExpressionTerm::DoubleLiteral(b) => Some(Double::from(*a) == *b),
            ExpressionTerm::IntegerLiteral(b) => Some(a == b),
            ExpressionTerm::DecimalLiteral(b) => Some(Decimal::from(*a) == *b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::DecimalLiteral(a) => match b {
            ExpressionTerm::FloatLiteral(b) => Some(Float::from(*a) == *b),
            ExpressionTerm::DoubleLiteral(b) => Some(Double::from(*a) == *b),
            ExpressionTerm::IntegerLiteral(b) => Some(*a == (*b).into()),
            ExpressionTerm::DecimalLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        ExpressionTerm::DateTimeLiteral(a) => match b {
            ExpressionTerm::DateTimeLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::TimeLiteral(a) => match b {
            ExpressionTerm::TimeLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::DateLiteral(a) => match b {
            ExpressionTerm::DateLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GYearMonthLiteral(a) => match b {
            ExpressionTerm::GYearMonthLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GYearLiteral(a) => match b {
            ExpressionTerm::GYearLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GMonthDayLiteral(a) => match b {
            ExpressionTerm::GMonthDayLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GDayLiteral(a) => match b {
            ExpressionTerm::GDayLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GMonthLiteral(a) => match b {
            ExpressionTerm::GMonthLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::DurationLiteral(a) => match b {
            ExpressionTerm::DurationLiteral(b) => Some(a == b),
            ExpressionTerm::YearMonthDurationLiteral(b) => Some(a == b),
            ExpressionTerm::DayTimeDurationLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::YearMonthDurationLiteral(a) => match b {
            ExpressionTerm::DurationLiteral(b) => Some(a == b),
            ExpressionTerm::YearMonthDurationLiteral(b) => Some(a == b),
            ExpressionTerm::DayTimeDurationLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::DayTimeDurationLiteral(a) => match b {
            ExpressionTerm::DurationLiteral(b) => Some(a == b),
            ExpressionTerm::YearMonthDurationLiteral(b) => Some(a == b),
            ExpressionTerm::DayTimeDurationLiteral(b) => Some(a == b),
            ExpressionTerm::OtherTypedLiteral { .. } => None,
            _ => Some(false),
        },
        #[cfg(feature = "rdf-star")]
        ExpressionTerm::Triple(a) => {
            if let ExpressionTerm::Triple(b) = b {
                triple_equals(a, b)
            } else {
                Some(false)
            }
        }
    }
}

#[cfg(feature = "rdf-star")]
fn triple_equals(a: &ExpressionTriple, b: &ExpressionTriple) -> Option<bool> {
    Some(
        match &a.subject {
            ExpressionSubject::NamedNode(_) | ExpressionSubject::BlankNode(_) => {
                a.subject == b.subject
            }
            ExpressionSubject::Triple(a) => {
                if let ExpressionSubject::Triple(b) = &b.subject {
                    triple_equals(a, b)?
                } else {
                    false
                }
            }
        } && a.predicate == b.predicate
            && equals(&a.object, &b.object)?,
    )
}

/// Comparison for ordering
fn cmp_terms(a: Option<&ExpressionTerm>, b: Option<&ExpressionTerm>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => {
            match a {
                ExpressionTerm::BlankNode(a) => match b {
                    ExpressionTerm::BlankNode(b) => a.as_str().cmp(b.as_str()),
                    _ => Ordering::Less,
                },
                ExpressionTerm::NamedNode(a) => match b {
                    ExpressionTerm::BlankNode(_) => Ordering::Greater,
                    ExpressionTerm::NamedNode(b) => a.as_str().cmp(b.as_str()),
                    _ => Ordering::Less,
                },
                #[cfg(feature = "rdf-star")]
                ExpressionTerm::Triple(a) => match b {
                    ExpressionTerm::Triple(b) => cmp_triples(a, b),
                    _ => Ordering::Greater,
                },
                _ => match b {
                    ExpressionTerm::NamedNode(_) | ExpressionTerm::BlankNode(_) => {
                        Ordering::Greater
                    }
                    #[cfg(feature = "rdf-star")]
                    ExpressionTerm::Triple(_) => Ordering::Less,
                    _ => {
                        if let Some(ord) = partial_cmp_literals(a, b) {
                            ord
                        } else if let (Term::Literal(a), Term::Literal(b)) =
                            (a.clone().into(), b.clone().into())
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
            }
        }
        (Some(_), None) => Ordering::Greater,
        (None, Some(_)) => Ordering::Less,
        (None, None) => Ordering::Equal,
    }
}

#[cfg(feature = "rdf-star")]
fn cmp_triples(a: &ExpressionTriple, b: &ExpressionTriple) -> Ordering {
    match match &a.subject {
        ExpressionSubject::BlankNode(a) => match &b.subject {
            ExpressionSubject::BlankNode(b) => a.as_str().cmp(b.as_str()),
            ExpressionSubject::NamedNode(_) => Ordering::Less,
            #[cfg(feature = "rdf-star")]
            ExpressionSubject::Triple(_) => Ordering::Less,
        },
        ExpressionSubject::NamedNode(a) => match &b.subject {
            ExpressionSubject::BlankNode(_) => Ordering::Greater,
            ExpressionSubject::NamedNode(b) => a.as_str().cmp(b.as_str()),
            #[cfg(feature = "rdf-star")]
            ExpressionSubject::Triple(_) => Ordering::Less,
        },
        ExpressionSubject::Triple(a) => match &b.subject {
            ExpressionSubject::Triple(b) => cmp_triples(a, b),
            _ => Ordering::Greater,
        },
    } {
        Ordering::Equal => match a.predicate.as_str().cmp(b.predicate.as_str()) {
            Ordering::Equal => cmp_terms(Some(&a.object), Some(&b.object)),
            o => o,
        },
        o => o,
    }
}

/// Comparison for <, >, <= and >= operators
fn partial_cmp(a: &ExpressionTerm, b: &ExpressionTerm) -> Option<Ordering> {
    if a == b {
        return Some(Ordering::Equal);
    }
    #[cfg(feature = "rdf-star")]
    if let ExpressionTerm::Triple(a) = a {
        return if let ExpressionTerm::Triple(b) = b {
            partial_cmp_triples(a, b)
        } else {
            None
        };
    }
    partial_cmp_literals(a, b)
}

fn partial_cmp_literals(a: &ExpressionTerm, b: &ExpressionTerm) -> Option<Ordering> {
    match a {
        ExpressionTerm::StringLiteral(a) => {
            if let ExpressionTerm::StringLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        ExpressionTerm::LangStringLiteral {
            value: va,
            language: la,
        } => {
            if let ExpressionTerm::LangStringLiteral {
                value: vb,
                language: lb,
            } = b
            {
                if la == lb {
                    va.partial_cmp(vb)
                } else {
                    None
                }
            } else {
                None
            }
        }
        #[cfg(feature = "sparql-12")]
        ExpressionTerm::DirLangStringLiteral {
            value: va,
            language: la,
            base_direction: da,
        } => {
            if let ExpressionTerm::DirLangStringLiteral {
                value: vb,
                language: lb,
                base_direction: db,
            } = b
            {
                if la == lb && da == db {
                    va.partial_cmp(vb)
                } else {
                    None
                }
            } else {
                None
            }
        }
        ExpressionTerm::FloatLiteral(a) => match b {
            ExpressionTerm::FloatLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::DoubleLiteral(b) => Double::from(*a).partial_cmp(b),
            ExpressionTerm::IntegerLiteral(b) => a.partial_cmp(&Float::from(*b)),
            ExpressionTerm::DecimalLiteral(b) => a.partial_cmp(&(*b).into()),
            _ => None,
        },
        ExpressionTerm::DoubleLiteral(a) => match b {
            ExpressionTerm::FloatLiteral(b) => a.partial_cmp(&(*b).into()),
            ExpressionTerm::DoubleLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::IntegerLiteral(b) => a.partial_cmp(&Double::from(*b)),
            ExpressionTerm::DecimalLiteral(b) => a.partial_cmp(&(*b).into()),
            _ => None,
        },
        ExpressionTerm::IntegerLiteral(a) => match b {
            ExpressionTerm::FloatLiteral(b) => Float::from(*a).partial_cmp(b),
            ExpressionTerm::DoubleLiteral(b) => Double::from(*a).partial_cmp(b),
            ExpressionTerm::IntegerLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::DecimalLiteral(b) => Decimal::from(*a).partial_cmp(b),
            _ => None,
        },
        ExpressionTerm::DecimalLiteral(a) => match b {
            ExpressionTerm::FloatLiteral(b) => Float::from(*a).partial_cmp(b),
            ExpressionTerm::DoubleLiteral(b) => Double::from(*a).partial_cmp(b),
            ExpressionTerm::IntegerLiteral(b) => a.partial_cmp(&Decimal::from(*b)),
            ExpressionTerm::DecimalLiteral(b) => a.partial_cmp(b),
            _ => None,
        },
        ExpressionTerm::DateTimeLiteral(a) => {
            if let ExpressionTerm::DateTimeLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::TimeLiteral(a) => {
            if let ExpressionTerm::TimeLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::DateLiteral(a) => {
            if let ExpressionTerm::DateLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GYearMonthLiteral(a) => {
            if let ExpressionTerm::GYearMonthLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GYearLiteral(a) => {
            if let ExpressionTerm::GYearLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GMonthDayLiteral(a) => {
            if let ExpressionTerm::GMonthDayLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GDayLiteral(a) => {
            if let ExpressionTerm::GDayLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        #[cfg(feature = "calendar-ext")]
        ExpressionTerm::GMonthLiteral(a) => {
            if let ExpressionTerm::GMonthLiteral(b) = b {
                a.partial_cmp(b)
            } else {
                None
            }
        }
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::DurationLiteral(a) => match b {
            ExpressionTerm::DurationLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::YearMonthDurationLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::DayTimeDurationLiteral(b) => a.partial_cmp(b),
            _ => None,
        },
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::YearMonthDurationLiteral(a) => match b {
            ExpressionTerm::DurationLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::YearMonthDurationLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::DayTimeDurationLiteral(b) => a.partial_cmp(b),
            _ => None,
        },
        #[cfg(feature = "sep-0002")]
        ExpressionTerm::DayTimeDurationLiteral(a) => match b {
            ExpressionTerm::DurationLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::YearMonthDurationLiteral(b) => a.partial_cmp(b),
            ExpressionTerm::DayTimeDurationLiteral(b) => a.partial_cmp(b),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(feature = "rdf-star")]
fn partial_cmp_triples(a: &ExpressionTriple, b: &ExpressionTriple) -> Option<Ordering> {
    // We compare subjects
    match (&a.subject, &b.subject) {
        (ExpressionSubject::NamedNode(a), ExpressionSubject::NamedNode(b)) => {
            if a != b {
                return None;
            }
        }
        (ExpressionSubject::BlankNode(a), ExpressionSubject::BlankNode(b)) => {
            if a != b {
                return None;
            }
        }
        (ExpressionSubject::Triple(a), ExpressionSubject::Triple(b)) => {
            match partial_cmp_triples(a, b)? {
                Ordering::Equal => (),
                o => return Some(o),
            }
        }
        _ => return None,
    }
    if a.predicate != b.predicate {
        return None;
    }
    partial_cmp(&a.object, &b.object)
}

enum NumericBinaryOperands {
    Float(Float, Float),
    Double(Double, Double),
    Integer(Integer, Integer),
    Decimal(Decimal, Decimal),
    #[cfg(feature = "sep-0002")]
    Duration(Duration, Duration),
    #[cfg(feature = "sep-0002")]
    YearMonthDuration(YearMonthDuration, YearMonthDuration),
    #[cfg(feature = "sep-0002")]
    DayTimeDuration(DayTimeDuration, DayTimeDuration),
    #[cfg(feature = "sep-0002")]
    DateTime(DateTime, DateTime),
    #[cfg(feature = "sep-0002")]
    Time(Time, Time),
    #[cfg(feature = "sep-0002")]
    Date(Date, Date),
    #[cfg(feature = "sep-0002")]
    DateTimeDuration(DateTime, Duration),
    #[cfg(feature = "sep-0002")]
    DateTimeYearMonthDuration(DateTime, YearMonthDuration),
    #[cfg(feature = "sep-0002")]
    DateTimeDayTimeDuration(DateTime, DayTimeDuration),
    #[cfg(feature = "sep-0002")]
    DateDuration(Date, Duration),
    #[cfg(feature = "sep-0002")]
    DateYearMonthDuration(Date, YearMonthDuration),
    #[cfg(feature = "sep-0002")]
    DateDayTimeDuration(Date, DayTimeDuration),
    #[cfg(feature = "sep-0002")]
    TimeDuration(Time, Duration),
    #[cfg(feature = "sep-0002")]
    TimeDayTimeDuration(Time, DayTimeDuration),
}

impl NumericBinaryOperands {
    fn new(a: ExpressionTerm, b: ExpressionTerm) -> Option<Self> {
        match (a, b) {
            (ExpressionTerm::FloatLiteral(v1), ExpressionTerm::FloatLiteral(v2)) => {
                Some(Self::Float(v1, v2))
            }
            (ExpressionTerm::FloatLiteral(v1), ExpressionTerm::DoubleLiteral(v2)) => {
                Some(Self::Double(v1.into(), v2))
            }
            (ExpressionTerm::FloatLiteral(v1), ExpressionTerm::IntegerLiteral(v2)) => {
                Some(Self::Float(v1, v2.into()))
            }
            (ExpressionTerm::FloatLiteral(v1), ExpressionTerm::DecimalLiteral(v2)) => {
                Some(Self::Float(v1, v2.into()))
            }
            (ExpressionTerm::DoubleLiteral(v1), ExpressionTerm::FloatLiteral(v2)) => {
                Some(Self::Double(v1, v2.into()))
            }
            (ExpressionTerm::DoubleLiteral(v1), ExpressionTerm::DoubleLiteral(v2)) => {
                Some(Self::Double(v1, v2))
            }
            (ExpressionTerm::DoubleLiteral(v1), ExpressionTerm::IntegerLiteral(v2)) => {
                Some(Self::Double(v1, v2.into()))
            }
            (ExpressionTerm::DoubleLiteral(v1), ExpressionTerm::DecimalLiteral(v2)) => {
                Some(Self::Double(v1, v2.into()))
            }
            (ExpressionTerm::IntegerLiteral(v1), ExpressionTerm::FloatLiteral(v2)) => {
                Some(Self::Float(v1.into(), v2))
            }
            (ExpressionTerm::IntegerLiteral(v1), ExpressionTerm::DoubleLiteral(v2)) => {
                Some(Self::Double(v1.into(), v2))
            }
            (ExpressionTerm::IntegerLiteral(v1), ExpressionTerm::IntegerLiteral(v2)) => {
                Some(Self::Integer(v1, v2))
            }
            (ExpressionTerm::IntegerLiteral(v1), ExpressionTerm::DecimalLiteral(v2)) => {
                Some(Self::Decimal(v1.into(), v2))
            }
            (ExpressionTerm::DecimalLiteral(v1), ExpressionTerm::FloatLiteral(v2)) => {
                Some(Self::Float(v1.into(), v2))
            }
            (ExpressionTerm::DecimalLiteral(v1), ExpressionTerm::DoubleLiteral(v2)) => {
                Some(Self::Double(v1.into(), v2))
            }
            (ExpressionTerm::DecimalLiteral(v1), ExpressionTerm::IntegerLiteral(v2)) => {
                Some(Self::Decimal(v1, v2.into()))
            }
            (ExpressionTerm::DecimalLiteral(v1), ExpressionTerm::DecimalLiteral(v2)) => {
                Some(Self::Decimal(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DurationLiteral(v1), ExpressionTerm::DurationLiteral(v2)) => {
                Some(Self::Duration(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DurationLiteral(v1), ExpressionTerm::YearMonthDurationLiteral(v2)) => {
                Some(Self::Duration(v1, v2.into()))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DurationLiteral(v1), ExpressionTerm::DayTimeDurationLiteral(v2)) => {
                Some(Self::Duration(v1, v2.into()))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::YearMonthDurationLiteral(v1), ExpressionTerm::DurationLiteral(v2)) => {
                Some(Self::Duration(v1.into(), v2))
            }
            #[cfg(feature = "sep-0002")]
            (
                ExpressionTerm::YearMonthDurationLiteral(v1),
                ExpressionTerm::YearMonthDurationLiteral(v2),
            ) => Some(Self::YearMonthDuration(v1, v2)),
            #[cfg(feature = "sep-0002")]
            (
                ExpressionTerm::YearMonthDurationLiteral(v1),
                ExpressionTerm::DayTimeDurationLiteral(v2),
            ) => Some(Self::Duration(v1.into(), v2.into())),
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DayTimeDurationLiteral(v1), ExpressionTerm::DurationLiteral(v2)) => {
                Some(Self::Duration(v1.into(), v2))
            }
            #[cfg(feature = "sep-0002")]
            (
                ExpressionTerm::DayTimeDurationLiteral(v1),
                ExpressionTerm::YearMonthDurationLiteral(v2),
            ) => Some(Self::Duration(v1.into(), v2.into())),
            #[cfg(feature = "sep-0002")]
            (
                ExpressionTerm::DayTimeDurationLiteral(v1),
                ExpressionTerm::DayTimeDurationLiteral(v2),
            ) => Some(Self::DayTimeDuration(v1, v2)),
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DateTimeLiteral(v1), ExpressionTerm::DateTimeLiteral(v2)) => {
                Some(Self::DateTime(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DateLiteral(v1), ExpressionTerm::DateLiteral(v2)) => {
                Some(Self::Date(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::TimeLiteral(v1), ExpressionTerm::TimeLiteral(v2)) => {
                Some(Self::Time(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DateTimeLiteral(v1), ExpressionTerm::DurationLiteral(v2)) => {
                Some(Self::DateTimeDuration(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DateTimeLiteral(v1), ExpressionTerm::YearMonthDurationLiteral(v2)) => {
                Some(Self::DateTimeYearMonthDuration(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DateTimeLiteral(v1), ExpressionTerm::DayTimeDurationLiteral(v2)) => {
                Some(Self::DateTimeDayTimeDuration(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DateLiteral(v1), ExpressionTerm::DurationLiteral(v2)) => {
                Some(Self::DateDuration(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DateLiteral(v1), ExpressionTerm::YearMonthDurationLiteral(v2)) => {
                Some(Self::DateYearMonthDuration(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::DateLiteral(v1), ExpressionTerm::DayTimeDurationLiteral(v2)) => {
                Some(Self::DateDayTimeDuration(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::TimeLiteral(v1), ExpressionTerm::DurationLiteral(v2)) => {
                Some(Self::TimeDuration(v1, v2))
            }
            #[cfg(feature = "sep-0002")]
            (ExpressionTerm::TimeLiteral(v1), ExpressionTerm::DayTimeDurationLiteral(v2)) => {
                Some(Self::TimeDayTimeDuration(v1, v2))
            }
            _ => None,
        }
    }
}

enum TupleSelector<D: QueryableDataset> {
    Constant(D::InternalTerm),
    Variable(usize),
    #[cfg(feature = "rdf-star")]
    TriplePattern(Rc<TripleTupleSelector<D>>),
}

impl<D: QueryableDataset> TupleSelector<D> {
    fn from_ground_term_pattern(
        term_pattern: &GroundTermPattern,
        variables: &mut Vec<Variable>,
        dataset: &EvalDataset<D>,
    ) -> Result<Self, QueryEvaluationError> {
        Ok(match term_pattern {
            GroundTermPattern::Variable(variable) => {
                Self::Variable(encode_variable(variables, variable))
            }
            GroundTermPattern::NamedNode(term) => {
                Self::Constant(dataset.internalize_term(term.as_ref().into())?)
            }
            GroundTermPattern::Literal(term) => {
                Self::Constant(dataset.internalize_term(term.as_ref().into())?)
            }
            #[cfg(feature = "rdf-star")]
            GroundTermPattern::Triple(triple) => {
                match (
                    Self::from_ground_term_pattern(&triple.subject, variables, dataset)?,
                    Self::from_named_node_pattern(&triple.predicate, variables, dataset)?,
                    Self::from_ground_term_pattern(&triple.object, variables, dataset)?,
                ) {
                    (
                        Self::Constant(subject),
                        Self::Constant(predicate),
                        Self::Constant(object),
                    ) => Self::Constant(
                        dataset.internalize_expression_term(
                            ExpressionTriple::new(
                                dataset.externalize_expression_term(subject)?,
                                dataset.externalize_expression_term(predicate)?,
                                dataset.externalize_expression_term(object)?,
                            )
                            .ok_or_else(|| QueryEvaluationError::InvalidStorageTripleTerm)?
                            .into(),
                        )?,
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
        })
    }

    fn from_named_node_pattern(
        named_node_pattern: &NamedNodePattern,
        variables: &mut Vec<Variable>,
        dataset: &EvalDataset<D>,
    ) -> Result<Self, QueryEvaluationError> {
        Ok(match named_node_pattern {
            NamedNodePattern::Variable(variable) => {
                Self::Variable(encode_variable(variables, variable))
            }
            NamedNodePattern::NamedNode(term) => {
                Self::Constant(dataset.internalize_term(term.as_ref().into())?)
            }
        })
    }

    #[cfg_attr(not(feature = "rdf-star"), allow(clippy::unnecessary_wraps))]
    fn get_pattern_value(
        &self,
        tuple: &InternalTuple<D>,
        #[cfg(feature = "rdf-star")] dataset: &EvalDataset<D>,
    ) -> Result<Option<D::InternalTerm>, QueryEvaluationError> {
        Ok(match self {
            Self::Constant(c) => Some(c.clone()),
            Self::Variable(v) => tuple.get(*v).cloned(),
            #[cfg(feature = "rdf-star")]
            Self::TriplePattern(triple) => {
                let Some(subject) = triple.subject.get_pattern_value(tuple, dataset)? else {
                    return Ok(None);
                };
                let Some(predicate) = triple.predicate.get_pattern_value(tuple, dataset)? else {
                    return Ok(None);
                };
                let Some(object) = triple.object.get_pattern_value(tuple, dataset)? else {
                    return Ok(None);
                };
                Some(
                    dataset.internalize_expression_term(
                        ExpressionTriple::new(
                            dataset.externalize_expression_term(subject)?,
                            dataset.externalize_expression_term(predicate)?,
                            dataset.externalize_expression_term(object)?,
                        )
                        .ok_or(QueryEvaluationError::InvalidStorageTripleTerm)?
                        .into(),
                    )?,
                )
            }
        })
    }
}

impl<D: QueryableDataset> Clone for TupleSelector<D> {
    fn clone(&self) -> Self {
        match self {
            Self::Constant(c) => Self::Constant(c.clone()),
            Self::Variable(v) => Self::Variable(*v),
            #[cfg(feature = "rdf-star")]
            Self::TriplePattern(t) => Self::TriplePattern(Rc::clone(t)),
        }
    }
}

#[cfg(feature = "rdf-star")]
struct TripleTupleSelector<D: QueryableDataset> {
    subject: TupleSelector<D>,
    predicate: TupleSelector<D>,
    object: TupleSelector<D>,
}

#[cfg_attr(not(feature = "rdf-star"), allow(clippy::unnecessary_wraps))]
fn put_pattern_value<D: QueryableDataset>(
    selector: &TupleSelector<D>,
    value: D::InternalTerm,
    tuple: &mut InternalTuple<D>,
    #[cfg(feature = "rdf-star")] dataset: &EvalDataset<D>,
) -> Result<bool, QueryEvaluationError> {
    Ok(match selector {
        TupleSelector::Constant(c) => *c == value,
        TupleSelector::Variable(v) => {
            if let Some(old) = tuple.get(*v) {
                value == *old
            } else {
                tuple.set(*v, value);
                true
            }
        }
        #[cfg(feature = "rdf-star")]
        TupleSelector::TriplePattern(triple) => {
            let ExpressionTerm::Triple(value) = dataset.externalize_expression_term(value)? else {
                return Ok(false);
            };
            put_pattern_value(
                &triple.subject,
                dataset.internalize_expression_term(value.subject.into())?,
                tuple,
                dataset,
            )? && put_pattern_value(
                &triple.predicate,
                dataset.internalize_expression_term(value.predicate.into())?,
                tuple,
                dataset,
            )? && put_pattern_value(
                &triple.object,
                dataset.internalize_expression_term(value.object)?,
                tuple,
                dataset,
            )?
        }
    })
}

pub fn are_compatible_and_not_disjointed<D: QueryableDataset>(
    a: &InternalTuple<D>,
    b: &InternalTuple<D>,
) -> bool {
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

pub enum PropertyPath<D: QueryableDataset> {
    Path(D::InternalTerm),
    Reverse(Rc<Self>),
    Sequence(Rc<Self>, Rc<Self>),
    Alternative(Rc<Self>, Rc<Self>),
    ZeroOrMore(Rc<Self>),
    OneOrMore(Rc<Self>),
    ZeroOrOne(Rc<Self>),
    NegatedPropertySet(Rc<[D::InternalTerm]>),
}

struct PathEvaluator<D: QueryableDataset> {
    dataset: EvalDataset<D>,
}

impl<D: QueryableDataset> PathEvaluator<D> {
    fn eval_closed_in_graph(
        &self,
        path: &PropertyPath<D>,
        start: &D::InternalTerm,
        end: &D::InternalTerm,
        graph_name: Option<&D::InternalTerm>,
    ) -> Result<bool, QueryEvaluationError> {
        Ok(match path {
            PropertyPath::Path(p) => self
                .dataset
                .internal_quads_for_pattern(Some(start), Some(p), Some(end), Some(graph_name))
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
                .internal_quads_for_pattern(Some(start), None, Some(end), Some(graph_name))
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
        path: &PropertyPath<D>,
        start: &D::InternalTerm,
        end: &D::InternalTerm,
    ) -> Box<dyn Iterator<Item = Result<Option<D::InternalTerm>, QueryEvaluationError>>> {
        match path {
            PropertyPath::Path(p) => Box::new(
                self.dataset
                    .internal_quads_for_pattern(Some(start), Some(p), Some(end), None)
                    .map(|t| Ok(t?.graph_name)),
            ),
            PropertyPath::Reverse(p) => self.eval_closed_in_unknown_graph(p, end, start),
            PropertyPath::Sequence(a, b) => {
                let eval = self.clone();
                let b = Rc::clone(b);
                let end = end.clone();
                Box::new(self.eval_from_in_unknown_graph(a, start).flat_map_ok(
                    move |(middle, graph_name)| {
                        eval.eval_closed_in_graph(&b, &middle, &end, graph_name.as_ref())
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
                        |e| eval.eval_from_in_graph(&p, &e, graph_name.as_ref()),
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
                                    |e| eval.eval_from_in_graph(&p, &e, graph_name.as_ref()),
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
                        eval.eval_closed_in_graph(&p, &start2, &end, graph_name.as_ref())
                            .map(|is_found| is_found.then_some(graph_name))
                            .transpose()
                    })
                }
            }
            PropertyPath::NegatedPropertySet(ps) => {
                let ps = Rc::clone(ps);
                Box::new(
                    self.dataset
                        .internal_quads_for_pattern(Some(start), None, Some(end), None)
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
        path: &PropertyPath<D>,
        start: &D::InternalTerm,
        graph_name: Option<&D::InternalTerm>,
    ) -> Box<dyn Iterator<Item = Result<D::InternalTerm, QueryEvaluationError>>> {
        match path {
            PropertyPath::Path(p) => Box::new(
                self.dataset
                    .internal_quads_for_pattern(Some(start), Some(p), None, Some(graph_name))
                    .map(|t| Ok(t?.object)),
            ),
            PropertyPath::Reverse(p) => self.eval_to_in_graph(p, start, graph_name),
            PropertyPath::Sequence(a, b) => {
                let eval = self.clone();
                let b = Rc::clone(b);
                let graph_name2 = graph_name.cloned();
                Box::new(
                    self.eval_from_in_graph(a, start, graph_name)
                        .flat_map_ok(move |middle| {
                            eval.eval_from_in_graph(&b, &middle, graph_name2.as_ref())
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
                    let graph_name2 = graph_name.cloned();
                    transitive_closure(Some(Ok(start.clone())), move |e| {
                        eval.eval_from_in_graph(&p, &e, graph_name2.as_ref())
                    })
                })
            }
            PropertyPath::OneOrMore(p) => {
                let eval = self.clone();
                let p = Rc::clone(p);
                let graph_name2 = graph_name.cloned();
                Box::new(transitive_closure(
                    self.eval_from_in_graph(&p, start, graph_name),
                    move |e| eval.eval_from_in_graph(&p, &e, graph_name2.as_ref()),
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
                        .internal_quads_for_pattern(Some(start), None, None, Some(graph_name))
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
        path: &PropertyPath<D>,
        start: &D::InternalTerm,
    ) -> Box<
        dyn Iterator<
            Item = Result<(D::InternalTerm, Option<D::InternalTerm>), QueryEvaluationError>,
        >,
    > {
        match path {
            PropertyPath::Path(p) => Box::new(
                self.dataset
                    .internal_quads_for_pattern(Some(start), Some(p), None, None)
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
                        eval.eval_from_in_graph(&b, &middle, graph_name.as_ref())
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
                        eval.eval_from_in_graph(&p, &e, graph_name2.as_ref())
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
                        eval.eval_from_in_graph(&p, &e, graph_name.as_ref())
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
                        graph_name.as_ref(),
                    )))
                    .map(move |e| Ok((e?, graph_name.clone())))
                })
            }
            PropertyPath::NegatedPropertySet(ps) => {
                let ps = Rc::clone(ps);
                Box::new(
                    self.dataset
                        .internal_quads_for_pattern(Some(start), None, None, None)
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
        path: &PropertyPath<D>,
        end: &D::InternalTerm,
        graph_name: Option<&D::InternalTerm>,
    ) -> Box<dyn Iterator<Item = Result<D::InternalTerm, QueryEvaluationError>>> {
        match path {
            PropertyPath::Path(p) => Box::new(
                self.dataset
                    .internal_quads_for_pattern(None, Some(p), Some(end), Some(graph_name))
                    .map(|t| Ok(t?.subject)),
            ),
            PropertyPath::Reverse(p) => self.eval_from_in_graph(p, end, graph_name),
            PropertyPath::Sequence(a, b) => {
                let eval = self.clone();
                let a = Rc::clone(a);
                let graph_name2 = graph_name.cloned();
                Box::new(
                    self.eval_to_in_graph(b, end, graph_name)
                        .flat_map_ok(move |middle| {
                            eval.eval_to_in_graph(&a, &middle, graph_name2.as_ref())
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
                    let graph_name2 = graph_name.cloned();
                    transitive_closure(Some(Ok(end.clone())), move |e| {
                        eval.eval_to_in_graph(&p, &e, graph_name2.as_ref())
                    })
                })
            }
            PropertyPath::OneOrMore(p) => {
                let eval = self.clone();
                let p = Rc::clone(p);
                let graph_name2 = graph_name.cloned();
                Box::new(transitive_closure(
                    self.eval_to_in_graph(&p, end, graph_name),
                    move |e| eval.eval_to_in_graph(&p, &e, graph_name2.as_ref()),
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
                        .internal_quads_for_pattern(None, None, Some(end), Some(graph_name))
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
        path: &PropertyPath<D>,
        end: &D::InternalTerm,
    ) -> Box<
        dyn Iterator<
            Item = Result<(D::InternalTerm, Option<D::InternalTerm>), QueryEvaluationError>,
        >,
    > {
        match path {
            PropertyPath::Path(p) => Box::new(
                self.dataset
                    .internal_quads_for_pattern(None, Some(p), Some(end), None)
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
                        eval.eval_to_in_graph(&a, &middle, graph_name.as_ref())
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
                        eval.eval_to_in_graph(&p, &e, graph_name2.as_ref())
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
                        eval.eval_to_in_graph(&p, &e, graph_name.as_ref())
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
                        graph_name.as_ref(),
                    )))
                    .map(move |e| Ok((e?, graph_name.clone())))
                })
            }
            PropertyPath::NegatedPropertySet(ps) => {
                let ps = Rc::clone(ps);
                Box::new(
                    self.dataset
                        .internal_quads_for_pattern(None, None, Some(end), None)
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
        path: &PropertyPath<D>,
        graph_name: Option<&D::InternalTerm>,
    ) -> Box<dyn Iterator<Item = Result<(D::InternalTerm, D::InternalTerm), QueryEvaluationError>>>
    {
        match path {
            PropertyPath::Path(p) => Box::new(
                self.dataset
                    .internal_quads_for_pattern(None, Some(p), None, Some(graph_name))
                    .map(|t| {
                        let t = t?;
                        Ok((t.subject, t.object))
                    }),
            ),
            PropertyPath::Reverse(p) => Box::new(
                self.eval_open_in_graph(p, graph_name)
                    .map(|t| t.map(|(s, o)| (o, s))),
            ),
            PropertyPath::Sequence(a, b) => {
                let eval = self.clone();
                let b = Rc::clone(b);
                let graph_name2 = graph_name.cloned();
                Box::new(self.eval_open_in_graph(a, graph_name).flat_map_ok(
                    move |(start, middle)| {
                        eval.eval_from_in_graph(&b, &middle, graph_name2.as_ref())
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
                let graph_name2 = graph_name.cloned();
                Box::new(transitive_closure(
                    self.get_subject_or_object_identity_pairs_in_graph(graph_name),
                    move |(start, middle)| {
                        eval.eval_from_in_graph(&p, &middle, graph_name2.as_ref())
                            .map(move |end| Ok((start.clone(), end?)))
                    },
                ))
            }
            PropertyPath::OneOrMore(p) => {
                let eval = self.clone();
                let p = Rc::clone(p);
                let graph_name2 = graph_name.cloned();
                Box::new(transitive_closure(
                    self.eval_open_in_graph(&p, graph_name),
                    move |(start, middle)| {
                        eval.eval_from_in_graph(&p, &middle, graph_name2.as_ref())
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
                        .internal_quads_for_pattern(None, None, None, Some(graph_name))
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
        path: &PropertyPath<D>,
    ) -> Box<
        dyn Iterator<
            Item = Result<
                (D::InternalTerm, D::InternalTerm, Option<D::InternalTerm>),
                QueryEvaluationError,
            >,
        >,
    > {
        match path {
            PropertyPath::Path(p) => Box::new(
                self.dataset
                    .internal_quads_for_pattern(None, Some(p), None, None)
                    .map(|t| {
                        let t = t?;
                        Ok((t.subject, t.object, t.graph_name))
                    }),
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
                        eval.eval_from_in_graph(&b, &middle, graph_name.as_ref())
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
                        eval.eval_from_in_graph(&p, &middle, graph_name.as_ref())
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
                        eval.eval_from_in_graph(&p, &middle, graph_name.as_ref())
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
                        .internal_quads_for_pattern(None, None, None, None)
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
        graph_name: Option<&D::InternalTerm>,
    ) -> impl Iterator<Item = Result<(D::InternalTerm, D::InternalTerm), QueryEvaluationError>>
    {
        self.dataset
            .internal_quads_for_pattern(None, None, None, Some(graph_name))
            .flat_map_ok(|t| {
                [
                    Ok((t.subject.clone(), t.subject)),
                    Ok((t.object.clone(), t.object)),
                ]
            })
    }

    fn get_subject_or_object_identity_pairs_in_dataset(
        &self,
    ) -> impl Iterator<
        Item = Result<
            (D::InternalTerm, D::InternalTerm, Option<D::InternalTerm>),
            QueryEvaluationError,
        >,
    > {
        self.dataset
            .internal_quads_for_pattern(None, None, None, None)
            .flat_map_ok(|t| {
                [
                    Ok((t.subject.clone(), t.subject, t.graph_name.clone())),
                    Ok((t.object.clone(), t.object, t.graph_name)),
                ]
            })
    }

    fn run_if_term_is_a_graph_node<
        T: 'static,
        I: Iterator<Item = Result<T, QueryEvaluationError>> + 'static,
    >(
        &self,
        term: &D::InternalTerm,
        graph_name: Option<&D::InternalTerm>,
        f: impl FnOnce() -> I,
    ) -> Box<dyn Iterator<Item = Result<T, QueryEvaluationError>>> {
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
        term: &D::InternalTerm,
        graph_name: Option<&D::InternalTerm>,
    ) -> Result<bool, QueryEvaluationError> {
        Ok(self
            .dataset
            .internal_quads_for_pattern(Some(term), None, None, Some(graph_name))
            .next()
            .transpose()?
            .is_some()
            || self
                .dataset
                .internal_quads_for_pattern(None, None, Some(term), Some(graph_name))
                .next()
                .transpose()?
                .is_some())
    }

    fn run_if_term_is_a_dataset_node<
        T: 'static,
        I: IntoIterator<Item = Result<T, QueryEvaluationError>> + 'static,
    >(
        &self,
        term: &D::InternalTerm,
        f: impl FnMut(Option<D::InternalTerm>) -> I + 'static,
    ) -> Box<dyn Iterator<Item = Result<T, QueryEvaluationError>>> {
        match self
            .find_graphs_where_the_node_is_in(term)
            .collect::<Result<FxHashSet<_>, _>>()
        {
            Ok(graph_names) => Box::new(graph_names.into_iter().flat_map(f)),
            Err(error) => Box::new(once(Err(error))),
        }
    }

    fn find_graphs_where_the_node_is_in(
        &self,
        term: &D::InternalTerm,
    ) -> impl Iterator<Item = Result<Option<D::InternalTerm>, QueryEvaluationError>> {
        self.dataset
            .internal_quads_for_pattern(Some(term), None, None, None)
            .chain(
                self.dataset
                    .internal_quads_for_pattern(None, None, Some(term), None),
            )
            .map(|q| Ok(q?.graph_name))
    }
}

impl<D: QueryableDataset> Clone for PathEvaluator<D> {
    fn clone(&self) -> Self {
        Self {
            dataset: self.dataset.clone(),
        }
    }
}

struct CartesianProductJoinIterator<D: QueryableDataset> {
    probe_iter: Peekable<InternalTuplesIterator<D>>,
    built: Vec<InternalTuple<D>>,
    buffered_results: Vec<Result<InternalTuple<D>, QueryEvaluationError>>,
}

impl<D: QueryableDataset> Iterator for CartesianProductJoinIterator<D> {
    type Item = Result<InternalTuple<D>, QueryEvaluationError>;

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

struct HashJoinIterator<D: QueryableDataset> {
    probe_iter: Peekable<InternalTuplesIterator<D>>,
    built: InternalTupleSet<D>,
    buffered_results: Vec<Result<InternalTuple<D>, QueryEvaluationError>>,
}

impl<D: QueryableDataset> Iterator for HashJoinIterator<D> {
    type Item = Result<InternalTuple<D>, QueryEvaluationError>;

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

struct HashLeftJoinIterator<D: QueryableDataset> {
    left_iter: InternalTuplesIterator<D>,
    right: InternalTupleSet<D>,
    buffered_results: Vec<Result<InternalTuple<D>, QueryEvaluationError>>,
    expression: Rc<dyn Fn(&InternalTuple<D>) -> Option<bool>>,
}

impl<D: QueryableDataset> Iterator for HashLeftJoinIterator<D> {
    type Item = Result<InternalTuple<D>, QueryEvaluationError>;

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
                    .filter(|tuple| (self.expression)(tuple).unwrap_or(false))
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

#[cfg(feature = "sep-0006")]
struct ForLoopLeftJoinIterator<D: QueryableDataset> {
    right_evaluator: Rc<dyn Fn(InternalTuple<D>) -> InternalTuplesIterator<D>>,
    left_iter: InternalTuplesIterator<D>,
    current_right: InternalTuplesIterator<D>,
    left_tuple_to_yield: Option<InternalTuple<D>>,
}

#[cfg(feature = "sep-0006")]
impl<D: QueryableDataset> Iterator for ForLoopLeftJoinIterator<D> {
    type Item = Result<InternalTuple<D>, QueryEvaluationError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(tuple) = self.current_right.next() {
                if tuple.is_ok() {
                    // No need to yield left, we have a tuple combined with right
                    self.left_tuple_to_yield = None;
                }
                return Some(tuple);
            }
            if let Some(left_tuple) = self.left_tuple_to_yield.take() {
                return Some(Ok(left_tuple));
            }
            let left_tuple = match self.left_iter.next()? {
                Ok(left_tuple) => left_tuple,
                Err(error) => return Some(Err(error)),
            };
            self.current_right = (self.right_evaluator)(left_tuple.clone());
            self.left_tuple_to_yield = Some(left_tuple);
        }
    }
}

struct UnionIterator<D: QueryableDataset> {
    plans: Vec<Rc<dyn Fn(InternalTuple<D>) -> InternalTuplesIterator<D>>>,
    input: InternalTuple<D>,
    current_iterator: InternalTuplesIterator<D>,
    current_plan: usize,
}

impl<D: QueryableDataset> Iterator for UnionIterator<D> {
    type Item = Result<InternalTuple<D>, QueryEvaluationError>;

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

struct ConsecutiveDeduplication<D: QueryableDataset> {
    inner: InternalTuplesIterator<D>,
    current: Option<InternalTuple<D>>,
}

impl<D: QueryableDataset> Iterator for ConsecutiveDeduplication<D> {
    type Item = Result<InternalTuple<D>, QueryEvaluationError>;

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

struct ConstructIterator<D: QueryableDataset> {
    eval: SimpleEvaluator<D>,
    iter: InternalTuplesIterator<D>,
    template: Vec<TripleTemplate>,
    buffered_results: Vec<Result<Triple, QueryEvaluationError>>,
    already_emitted_results: FxHashSet<Triple>,
    bnodes: Vec<BlankNode>,
}

impl<D: QueryableDataset> Iterator for ConstructIterator<D> {
    type Item = Result<Triple, QueryEvaluationError>;

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
                        get_triple_template_value(
                            &template.subject,
                            &tuple,
                            &mut self.bnodes,
                            &self.eval.dataset,
                        )
                        .and_then(|t| t.try_into().ok()),
                        get_triple_template_value(
                            &template.predicate,
                            &tuple,
                            &mut self.bnodes,
                            &self.eval.dataset,
                        )
                        .and_then(|t| t.try_into().ok()),
                        get_triple_template_value(
                            &template.object,
                            &tuple,
                            &mut self.bnodes,
                            &self.eval.dataset,
                        ),
                    ) {
                        let triple = Triple {
                            subject,
                            predicate,
                            object,
                        };
                        // We allocate new blank nodes for each solution,
                        // triples with blank nodes are likely to be new.
                        #[cfg(feature = "rdf-star")]
                        let new_triple = triple.subject.is_blank_node()
                            || triple.subject.is_triple()
                            || triple.object.is_blank_node()
                            || triple.object.is_triple()
                            || self.already_emitted_results.insert(triple.clone());
                        #[cfg(not(feature = "rdf-star"))]
                        let new_triple = triple.subject.is_blank_node()
                            || triple.object.is_blank_node()
                            || self.already_emitted_results.insert(triple.clone());
                        if new_triple {
                            self.buffered_results.push(Ok(triple));
                            if self.already_emitted_results.len() > 1024 * 1024 {
                                // We don't want to have a too big memory impact
                                self.already_emitted_results.clear();
                            }
                        }
                    }
                }
                self.bnodes.clear(); // We do not reuse blank nodes
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
    Constant(Term),
    BlankNode(usize),
    Variable(usize),
    #[cfg(feature = "rdf-star")]
    Triple(Box<TripleTemplate>),
}

impl TripleTemplateValue {
    #[cfg_attr(not(feature = "rdf-star"), allow(clippy::unnecessary_wraps))]
    fn from_term_or_variable(
        term_or_variable: &TermPattern,
        variables: &mut Vec<Variable>,
        bnodes: &mut Vec<BlankNode>,
    ) -> Option<Self> {
        Some(match term_or_variable {
            TermPattern::Variable(variable) => Self::Variable(encode_variable(variables, variable)),
            TermPattern::NamedNode(node) => Self::Constant(node.clone().into()),
            TermPattern::BlankNode(bnode) => Self::BlankNode(bnode_key(bnodes, bnode)),
            TermPattern::Literal(literal) => Self::Constant(literal.clone().into()),
            #[cfg(feature = "rdf-star")]
            TermPattern::Triple(triple) => {
                match (
                    Self::from_term_or_variable(&triple.subject, variables, bnodes)?,
                    Self::from_named_node_or_variable(&triple.predicate, variables),
                    Self::from_term_or_variable(&triple.object, variables, bnodes)?,
                ) {
                    (
                        Self::Constant(subject),
                        Self::Constant(predicate),
                        Self::Constant(object),
                    ) => Self::Constant(
                        Triple {
                            subject: subject.try_into().ok()?,
                            predicate: predicate.try_into().ok()?,
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
                }
            }
        })
    }

    fn from_named_node_or_variable(
        named_node_or_variable: &NamedNodePattern,
        variables: &mut Vec<Variable>,
    ) -> TripleTemplateValue {
        match named_node_or_variable {
            NamedNodePattern::Variable(variable) => {
                Self::Variable(encode_variable(variables, variable))
            }
            NamedNodePattern::NamedNode(term) => Self::Constant(term.clone().into()),
        }
    }
}

fn get_triple_template_value<D: QueryableDataset>(
    selector: &TripleTemplateValue,
    tuple: &InternalTuple<D>,
    bnodes: &mut Vec<BlankNode>,
    dataset: &EvalDataset<D>,
) -> Option<Term> {
    match selector {
        TripleTemplateValue::Constant(term) => Some(term.clone()),
        TripleTemplateValue::Variable(v) => {
            tuple
                .get(*v)
                .and_then(|t| dataset.externalize_term(t.clone()).ok()) // TODO: raise error
        }
        TripleTemplateValue::BlankNode(bnode) => {
            if *bnode >= bnodes.len() {
                bnodes.resize_with(*bnode + 1, BlankNode::default)
            }
            Some(bnodes[*bnode].clone().into())
        }
        #[cfg(feature = "rdf-star")]
        TripleTemplateValue::Triple(triple) => Some(
            Triple {
                subject: get_triple_template_value(&triple.subject, tuple, bnodes, dataset)?
                    .try_into()
                    .ok()?,
                predicate: get_triple_template_value(&triple.predicate, tuple, bnodes, dataset)?
                    .try_into()
                    .ok()?,
                object: get_triple_template_value(&triple.object, tuple, bnodes, dataset)?,
            }
            .into(),
        ),
    }
}

struct DescribeIterator<D: QueryableDataset> {
    eval: SimpleEvaluator<D>,
    tuples_to_describe: InternalTuplesIterator<D>,
    nodes_described: FxHashSet<D::InternalTerm>,
    nodes_to_describe: Vec<D::InternalTerm>,
    quads: Box<dyn Iterator<Item = Result<InternalQuad<D>, QueryEvaluationError>>>,
}

impl<D: QueryableDataset> Iterator for DescribeIterator<D> {
    type Item = Result<Triple, QueryEvaluationError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(quad) = self.quads.next() {
                let quad = match quad {
                    Ok(quad) => quad,
                    Err(error) => return Some(Err(error)),
                };
                // We yield the triple
                let subject = match self.eval.dataset.externalize_term(quad.subject) {
                    Ok(t) => t,
                    Err(e) => return Some(Err(e)),
                };
                let predicate = match self.eval.dataset.externalize_term(quad.predicate) {
                    Ok(t) => t,
                    Err(e) => return Some(Err(e)),
                };
                let object = match self.eval.dataset.externalize_term(quad.object.clone()) {
                    Ok(t) => t,
                    Err(e) => return Some(Err(e)),
                };
                // If there is a blank node object, we need to describe it too
                if object.is_blank_node() && self.nodes_described.insert(quad.object.clone()) {
                    self.nodes_to_describe.push(quad.object);
                }
                return Some(Ok(Triple {
                    subject: subject.try_into().ok()?,
                    predicate: predicate.try_into().ok()?,
                    object,
                }));
            }
            if let Some(node_to_describe) = self.nodes_to_describe.pop() {
                // We have a new node to describe
                self.quads = Box::new(self.eval.dataset.internal_quads_for_pattern(
                    Some(&node_to_describe),
                    None,
                    None,
                    Some(None),
                ));
            } else {
                let tuple = match self.tuples_to_describe.next()? {
                    Ok(tuple) => tuple,
                    Err(error) => return Some(Err(error)),
                };
                for node in tuple.into_iter().flatten() {
                    if self.nodes_described.insert(node.clone()) {
                        self.nodes_to_describe.push(node);
                    }
                }
            }
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

fn transitive_closure<T: Clone + Eq + Hash, E, NI: Iterator<Item = Result<T, E>>>(
    start: impl IntoIterator<Item = Result<T, E>>,
    mut next: impl FnMut(T) -> NI,
) -> impl Iterator<Item = Result<T, E>> {
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
    let mut all = todo.iter().cloned().collect::<FxHashSet<_>>();
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

fn look_in_transitive_closure<T: Clone + Eq + Hash, E, NI: Iterator<Item = Result<T, E>>>(
    start: impl IntoIterator<Item = Result<T, E>>,
    mut next: impl FnMut(T) -> NI,
    target: &T,
) -> Result<bool, E> {
    let mut todo = start.into_iter().collect::<Result<Vec<_>, _>>()?;
    let mut all = todo.iter().cloned().collect::<FxHashSet<_>>();
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

fn hash_deduplicate<T: Eq + Hash + Clone, E>(
    iter: impl Iterator<Item = Result<T, E>>,
) -> impl Iterator<Item = Result<T, E>> {
    let mut already_seen = FxHashSet::with_capacity_and_hasher(iter.size_hint().0, FxBuildHasher);
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

trait ResultIterator<T, E>: Iterator<Item = Result<T, E>> + Sized {
    fn flat_map_ok<O, F: FnMut(T) -> U, U: IntoIterator<Item = Result<O, E>>>(
        self,
        f: F,
    ) -> FlatMapOk<T, E, O, Self, F, U>;
}

impl<T, E, I: Iterator<Item = Result<T, E>> + Sized> ResultIterator<T, E> for I {
    #[inline]
    fn flat_map_ok<O, F: FnMut(T) -> U, U: IntoIterator<Item = Result<O, E>>>(
        self,
        f: F,
    ) -> FlatMapOk<T, E, O, Self, F, U> {
        FlatMapOk {
            inner: self,
            f,
            current: None,
        }
    }
}

struct FlatMapOk<
    T,
    E,
    O,
    I: Iterator<Item = Result<T, E>>,
    F: FnMut(T) -> U,
    U: IntoIterator<Item = Result<O, E>>,
> {
    inner: I,
    f: F,
    current: Option<U::IntoIter>,
}

impl<
        T,
        E,
        O,
        I: Iterator<Item = Result<T, E>>,
        F: FnMut(T) -> U,
        U: IntoIterator<Item = Result<O, E>>,
    > Iterator for FlatMapOk<T, E, O, I, F, U>
{
    type Item = Result<O, E>;

    #[inline]
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

fn error_evaluator<D: QueryableDataset>(
    error: QueryEvaluationError,
) -> Rc<dyn Fn(InternalTuple<D>) -> InternalTuplesIterator<D>> {
    let e = RefCell::new(Some(error));
    Rc::new(move |_| {
        if let Some(e) = e.replace(None) {
            Box::new(once(Err(e)))
        } else {
            Box::new(empty())
        }
    })
}

enum ComparatorFunction<D: QueryableDataset> {
    Asc(Rc<dyn Fn(&InternalTuple<D>) -> Option<ExpressionTerm>>),
    Desc(Rc<dyn Fn(&InternalTuple<D>) -> Option<ExpressionTerm>>),
}

struct InternalTupleSet<D: QueryableDataset> {
    key: Vec<usize>,
    map: FxHashMap<u64, Vec<InternalTuple<D>>>,
    len: usize,
}

impl<D: QueryableDataset> InternalTupleSet<D> {
    fn new(key: Vec<usize>) -> Self {
        Self {
            key,
            map: FxHashMap::default(),
            len: 0,
        }
    }

    fn insert(&mut self, tuple: InternalTuple<D>) {
        self.map
            .entry(self.tuple_key(&tuple))
            .or_default()
            .push(tuple);
        self.len += 1;
    }

    fn get(&self, tuple: &InternalTuple<D>) -> &[InternalTuple<D>] {
        self.map.get(&self.tuple_key(tuple)).map_or(&[], |v| v)
    }

    fn tuple_key(&self, tuple: &InternalTuple<D>) -> u64 {
        let mut hasher = FxHasher::default();
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

    fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<D: QueryableDataset> Extend<InternalTuple<D>> for InternalTupleSet<D> {
    fn extend<T: IntoIterator<Item = InternalTuple<D>>>(&mut self, iter: T) {
        let iter = iter.into_iter();
        self.map.reserve(iter.size_hint().0);
        for tuple in iter {
            self.insert(tuple);
        }
    }
}

struct StatsIterator<D: QueryableDataset> {
    inner: InternalTuplesIterator<D>,
    stats: Rc<EvalNodeWithStats>,
}

impl<D: QueryableDataset> Iterator for StatsIterator<D> {
    type Item = Result<InternalTuple<D>, QueryEvaluationError>;

    fn next(&mut self) -> Option<Self::Item> {
        let start = Timer::now();
        let result = self.inner.next();
        let duration = start.elapsed()?;
        self.stats.exec_duration.set(
            self.stats
                .exec_duration
                .get()
                .and_then(|d| d.checked_add(duration)),
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
        GraphPattern::Graph { graph_name } => format!("Graph({graph_name})"),
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
        #[cfg(feature = "sep-0006")]
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
