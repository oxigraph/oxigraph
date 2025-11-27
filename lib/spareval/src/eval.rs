#[cfg(feature = "sparql-12")]
use crate::dataset::ExpressionTriple;
use crate::dataset::{ExpressionTerm, InternalQuad, QueryableDataset};
use crate::error::QueryEvaluationError;
use crate::expression::{
    CustomFunctionRegistry, ExpressionEvaluator, ExpressionEvaluatorContext, NumericBinaryOperands,
    build_expression_evaluator, partial_cmp_literals, try_build_internal_expression_evaluator,
};
use crate::model::{QuerySolutionIter, QueryTripleIter};
use crate::service::ServiceHandlerRegistry;
use crate::{
    AggregateFunctionAccumulator, CustomAggregateFunctionRegistry, QueryDatasetSpecification,
};
use json_event_parser::{JsonEvent, WriterJsonSerializer};
use oxiri::Iri;
#[cfg(feature = "sparql-12")]
use oxrdf::{BaseDirection, NamedOrBlankNode};
use oxrdf::{BlankNode, GraphName, Literal, NamedNode, Term, Triple, Variable};
use oxsdatatypes::{DateTime, DayTimeDuration, Decimal, Double, Float, Integer};
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet, FxHasher};
use spargebra::algebra::{AggregateFunction, PropertyPathExpression};
#[cfg(feature = "sparql-12")]
use spargebra::term::GroundTriple;
use spargebra::term::{
    GroundTerm, GroundTermPattern, NamedNodePattern, TermPattern, TriplePattern,
};
use sparopt::algebra::{
    AggregateExpression, Expression, GraphPattern, JoinAlgorithm, LeftJoinAlgorithm,
    MinusAlgorithm, OrderExpression,
};
use std::cell::Cell;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::iter::{Peekable, empty, once};
use std::marker::PhantomData;
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, atomic};
use std::{fmt, io};
// TODO: make expression raise error when relevant (storage I/O)

type InternalTupleEvaluator<'a, T> =
    Rc<dyn Fn(InternalTuple<T>) -> InternalTuplesIterator<'a, T> + 'a>;

/// Wrapper on top of [`QueryableDataset`]
struct EvalDataset<'a, D: QueryableDataset<'a>> {
    dataset: Rc<D>,
    specification: EncodedDatasetSpec<D::InternalTerm>,
    cancellation_token: CancellationToken,
    _lifetime: PhantomData<&'a ()>,
}

impl<'a, D: QueryableDataset<'a>> EvalDataset<'a, D> {
    fn new(
        dataset: D,
        specification: QueryDatasetSpecification,
        cancellation_token: CancellationToken,
    ) -> Result<Self, QueryEvaluationError> {
        let specification = EncodedDatasetSpec {
            default: specification
                .default
                .map(|graph_names| {
                    graph_names
                        .into_iter()
                        .map(|graph_name| {
                            Ok(match graph_name {
                                GraphName::NamedNode(n) => {
                                    Some(dataset.internalize_term(n.into())?)
                                }
                                GraphName::BlankNode(n) => {
                                    Some(dataset.internalize_term(n.into())?)
                                }
                                GraphName::DefaultGraph => None,
                            })
                        })
                        .collect()
                })
                .transpose()
                .map_err(|e: D::Error| QueryEvaluationError::Dataset(Box::new(e)))?,
            named: specification
                .named
                .map(|graph_names| {
                    graph_names
                        .into_iter()
                        .map(|graph_name| dataset.internalize_term(graph_name.into()))
                        .collect()
                })
                .transpose()
                .map_err(|e| QueryEvaluationError::Dataset(Box::new(e)))?,
        };
        Ok(Self {
            dataset: Rc::new(dataset),
            specification,
            cancellation_token,
            _lifetime: PhantomData,
        })
    }

    fn underlying_internal_quads_for_pattern(
        &self,
        subject: Option<&D::InternalTerm>,
        predicate: Option<&D::InternalTerm>,
        object: Option<&D::InternalTerm>,
        graph_name: Option<Option<&D::InternalTerm>>,
    ) -> impl Iterator<Item = Result<InternalQuad<D::InternalTerm>, QueryEvaluationError>> + use<'a, D>
    {
        let cancellation_token = self.cancellation_token.clone();
        self.dataset
            .internal_quads_for_pattern(subject, predicate, object, graph_name)
            .map(move |r| {
                cancellation_token.ensure_alive()?;
                r.map_err(|e| QueryEvaluationError::Dataset(Box::new(e)))
            })
    }

    fn internal_quads_for_pattern(
        &self,
        subject: Option<&D::InternalTerm>,
        predicate: Option<&D::InternalTerm>,
        object: Option<&D::InternalTerm>,
        graph_name: Option<Option<&D::InternalTerm>>,
    ) -> Box<dyn Iterator<Item = Result<InternalQuad<D::InternalTerm>, QueryEvaluationError>> + 'a>
    {
        if let Some(graph_name) = graph_name {
            // A graph (named or default), has been specified, we only query it
            if let Some(graph_name) = graph_name {
                // We query a specific named graph of data (possibly including the global default graph)
                if self
                    .specification
                    .named
                    .as_ref()
                    .is_none_or(|d| d.contains(graph_name))
                {
                    // It is in the set of allowed named graphs (if this set exists), we query it
                    Box::new(self.underlying_internal_quads_for_pattern(
                        subject,
                        predicate,
                        object,
                        Some(Some(graph_name)),
                    ))
                } else {
                    Box::new(empty())
                }
            } else if let Some(default_graph_graphs) = &self.specification.default {
                // The default graph is queried, and it is set to something and not the union of all graphs
                if default_graph_graphs.len() == 1 {
                    // There is a single graph in the default graph, we return it directly
                    Box::new(
                        self.underlying_internal_quads_for_pattern(
                            subject,
                            predicate,
                            object,
                            Some(default_graph_graphs[0].as_ref()),
                        )
                        .map(|quad| {
                            let mut quad = quad?;
                            quad.graph_name = None;
                            Ok(quad)
                        }),
                    )
                } else {
                    let iters = default_graph_graphs
                        .iter()
                        .map(|graph_name| {
                            self.underlying_internal_quads_for_pattern(
                                subject,
                                predicate,
                                object,
                                Some(graph_name.as_ref()),
                            )
                        })
                        .collect::<Vec<_>>();
                    Box::new(iters.into_iter().flatten().map(|quad| {
                        let mut quad = quad?;
                        quad.graph_name = None;
                        Ok(quad)
                    }))
                }
            } else {
                // The default graph has not been set, it is the union of all graphs, we query all graphs
                Box::new(
                    self.underlying_internal_quads_for_pattern(subject, predicate, object, None)
                        .map(|quad| {
                            let mut quad = quad?;
                            quad.graph_name = None;
                            Ok(quad)
                        }),
                )
            }
        } else if let Some(named_graphs) = &self.specification.named {
            // The list of possible named graphs has been set, we only query these named graphs
            let iters = named_graphs
                .iter()
                .map(|graph_name| {
                    self.underlying_internal_quads_for_pattern(
                        subject,
                        predicate,
                        object,
                        Some(Some(graph_name)),
                    )
                })
                .collect::<Vec<_>>();
            Box::new(iters.into_iter().flatten())
        } else {
            // We query all named graphs because the list of named graphs has not been set
            Box::new(
                self.underlying_internal_quads_for_pattern(subject, predicate, object, None)
                    .filter(|q| !q.as_ref().is_ok_and(|q| q.graph_name.is_none())),
            )
        }
    }

    fn internal_named_graphs(
        &self,
    ) -> Box<dyn Iterator<Item = Result<D::InternalTerm, QueryEvaluationError>> + 'a> {
        if let Some(named_graphs) = &self.specification.named {
            Box::new(
                named_graphs
                    .iter()
                    .cloned()
                    .map(Ok)
                    .collect::<Vec<_>>()
                    .into_iter(),
            )
        } else {
            let cancellation_token = self.cancellation_token.clone();
            Box::new(self.dataset.internal_named_graphs().map(move |r| {
                cancellation_token.ensure_alive()?;
                r.map_err(|e| QueryEvaluationError::Dataset(Box::new(e)))
            }))
        }
    }

    fn contains_internal_graph_name(
        &self,
        graph_name: &D::InternalTerm,
    ) -> Result<bool, QueryEvaluationError> {
        if let Some(named_graphs) = &self.specification.named {
            Ok(named_graphs.contains(graph_name))
        } else {
            self.dataset
                .contains_internal_graph_name(graph_name)
                .map_err(|e| QueryEvaluationError::Dataset(Box::new(e)))
        }
    }

    fn internalize_term(&self, term: Term) -> Result<D::InternalTerm, QueryEvaluationError> {
        self.cancellation_token.ensure_alive()?;
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

impl<'a, D: QueryableDataset<'a>> Clone for EvalDataset<'a, D> {
    #[inline]
    fn clone(&self) -> Self {
        Self {
            dataset: Rc::clone(&self.dataset),
            specification: self.specification.clone(),
            cancellation_token: self.cancellation_token.clone(),
            _lifetime: self._lifetime,
        }
    }
}

#[derive(Clone)]
struct EncodedDatasetSpec<T> {
    default: Option<Vec<Option<T>>>,
    named: Option<Vec<T>>,
}

pub struct InternalTuple<T> {
    inner: Vec<Option<T>>,
}

impl<T> InternalTuple<T> {
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

    pub fn get(&self, index: usize) -> Option<&T> {
        self.inner.get(index).unwrap_or(&None).as_ref()
    }
}

impl<T: Clone> InternalTuple<T> {
    pub fn iter(&self) -> impl Iterator<Item = Option<T>> + '_ {
        self.inner.iter().cloned()
    }

    pub fn set(&mut self, index: usize, value: T) {
        if self.inner.len() <= index {
            self.inner.resize(index + 1, None);
        }
        self.inner[index] = Some(value);
    }
}

impl<T: Clone + Eq> InternalTuple<T> {
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

impl<T: Clone> Clone for InternalTuple<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<T: Eq> PartialEq for InternalTuple<T> {
    #[inline]
    fn eq(&self, other: &InternalTuple<T>) -> bool {
        self.inner == other.inner
    }
}

impl<T: Eq> Eq for InternalTuple<T> {}

impl<T: Hash> Hash for InternalTuple<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.inner.hash(state)
    }
}

impl<T> IntoIterator for InternalTuple<T> {
    type Item = Option<T>;
    type IntoIter = std::vec::IntoIter<Option<T>>;

    fn into_iter(self) -> Self::IntoIter {
        self.inner.into_iter()
    }
}

type InternalTuplesIterator<'a, T> =
    Box<dyn Iterator<Item = Result<InternalTuple<T>, QueryEvaluationError>> + 'a>;

pub struct SimpleEvaluator<'a, D: QueryableDataset<'a>> {
    dataset: EvalDataset<'a, D>,
    base_iri: Option<Arc<Iri<String>>>,
    now: DateTime,
    service_handler: Rc<ServiceHandlerRegistry>,
    custom_functions: Rc<CustomFunctionRegistry>,
    custom_aggregate_functions: Rc<CustomAggregateFunctionRegistry>,
    run_stats: bool,
}

impl<'a, D: QueryableDataset<'a>> SimpleEvaluator<'a, D> {
    pub fn new(
        dataset: D,
        base_iri: Option<Arc<Iri<String>>>,
        service_handler: Rc<ServiceHandlerRegistry>,
        custom_functions: Rc<CustomFunctionRegistry>,
        custom_aggregate_functions: Rc<CustomAggregateFunctionRegistry>,
        cancellation_token: CancellationToken,
        dataset_spec: QueryDatasetSpecification,
        run_stats: bool,
    ) -> Result<Self, QueryEvaluationError> {
        Ok(Self {
            dataset: EvalDataset::new(dataset, dataset_spec, cancellation_token)?,
            base_iri,
            now: DateTime::now(),
            service_handler,
            custom_functions,
            custom_aggregate_functions,
            run_stats,
        })
    }

    pub fn evaluate_select(
        &self,
        pattern: &GraphPattern,
        substitutions: impl IntoIterator<Item = (Variable, Term)>,
    ) -> (
        Result<QuerySolutionIter<'a>, QueryEvaluationError>,
        Rc<EvalNodeWithStats>,
    ) {
        let mut variables = Vec::new();
        let (eval, stats) = self.graph_pattern_evaluator(pattern, &mut variables);
        let eval = match eval {
            Ok(e) => e,
            Err(e) => return (Err(e), stats),
        };
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
        let eval = match eval {
            Ok(e) => e,
            Err(e) => return (Err(e), stats),
        };
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
        Result<QueryTripleIter<'a>, QueryEvaluationError>,
        Rc<EvalNodeWithStats>,
    ) {
        let mut variables = Vec::new();
        let (eval, stats) = self.graph_pattern_evaluator(pattern, &mut variables);
        let eval = match eval {
            Ok(e) => e,
            Err(e) => return (Err(e), stats),
        };
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
        Result<QueryTripleIter<'a>, QueryEvaluationError>,
        Rc<EvalNodeWithStats>,
    ) {
        let mut variables = Vec::new();
        let (eval, stats) = self.graph_pattern_evaluator(pattern, &mut variables);
        let eval = match eval {
            Ok(e) => e,
            Err(e) => return (Err(e), stats),
        };
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
        Result<InternalTupleEvaluator<'a, D::InternalTerm>, QueryEvaluationError>,
        Rc<EvalNodeWithStats>,
    ) {
        let mut stat_children = Vec::new();
        let evaluator =
            self.build_graph_pattern_evaluator(pattern, encoded_variables, &mut stat_children);
        let stats = Rc::new(EvalNodeWithStats {
            label: eval_node_label(pattern),
            children: stat_children,
            exec_count: Cell::new(0),
            exec_duration: Cell::new(self.run_stats.then(DayTimeDuration::default)),
        });
        let mut evaluator = match evaluator {
            Ok(e) => e,
            Err(e) => return (Err(e), stats),
        };
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
        (Ok(evaluator), stats)
    }

    fn build_graph_pattern_evaluator(
        &self,
        pattern: &GraphPattern,
        encoded_variables: &mut Vec<Variable>,
        stat_children: &mut Vec<Rc<EvalNodeWithStats>>,
    ) -> Result<InternalTupleEvaluator<'a, D::InternalTerm>, QueryEvaluationError> {
        Ok(match pattern {
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
                                        #[cfg(feature = "sparql-12")]
                                        GroundTerm::Triple(triple) => self.encode_triple(triple),
                                    }?,
                                );
                            }
                        }
                        Ok(result)
                    })
                    .collect::<Result<Vec<_>, QueryEvaluationError>>()?;
                Rc::new(move |from| {
                    Box::new(
                        encoded_tuples
                            .iter()
                            .filter_map(move |t| t.combine_with(&from))
                            .map(Ok)
                            .collect::<Vec<_>>()
                            .into_iter(),
                    )
                })
            }
            GraphPattern::QuadPattern {
                subject,
                predicate,
                object,
                graph_name,
            } => {
                let subject_selector = TupleSelector::from_ground_term_pattern(
                    subject,
                    encoded_variables,
                    &self.dataset,
                )?;
                let predicate_selector = TupleSelector::from_named_node_pattern(
                    predicate,
                    encoded_variables,
                    &self.dataset,
                )?;
                let object_selector = TupleSelector::from_ground_term_pattern(
                    object,
                    encoded_variables,
                    &self.dataset,
                )?;
                let graph_name_selector = if let Some(graph_name) = graph_name.as_ref() {
                    Some(TupleSelector::from_named_node_pattern(
                        graph_name,
                        encoded_variables,
                        &self.dataset,
                    )?)
                } else {
                    None
                };
                let dataset = self.dataset.clone();
                Rc::new(move |from| {
                    let input_subject = match subject_selector.get_pattern_value(
                        &from,
                        #[cfg(feature = "sparql-12")]
                        &dataset,
                    ) {
                        Ok(value) => value,
                        Err(e) => return Box::new(once(Err(e))),
                    };
                    let input_predicate = match predicate_selector.get_pattern_value(
                        &from,
                        #[cfg(feature = "sparql-12")]
                        &dataset,
                    ) {
                        Ok(value) => value,
                        Err(e) => return Box::new(once(Err(e))),
                    };
                    let input_object = match object_selector.get_pattern_value(
                        &from,
                        #[cfg(feature = "sparql-12")]
                        &dataset,
                    ) {
                        Ok(value) => value,
                        Err(e) => return Box::new(once(Err(e))),
                    };
                    let input_graph_name = if let Some(graph_name_selector) = &graph_name_selector {
                        match graph_name_selector.get_pattern_value(
                            &from,
                            #[cfg(feature = "sparql-12")]
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
                    #[cfg(feature = "sparql-12")]
                    let dataset = dataset.clone();
                    Box::new(
                        iter.map(move |quad| {
                            let quad = quad?;
                            let mut new_tuple = from.clone();
                            if !put_pattern_value::<D>(
                                &subject_selector,
                                quad.subject,
                                &mut new_tuple,
                                #[cfg(feature = "sparql-12")]
                                &dataset,
                            )? {
                                return Ok(None);
                            }
                            if !put_pattern_value::<D>(
                                &predicate_selector,
                                quad.predicate,
                                &mut new_tuple,
                                #[cfg(feature = "sparql-12")]
                                &dataset,
                            )? {
                                return Ok(None);
                            }
                            if !put_pattern_value::<D>(
                                &object_selector,
                                quad.object,
                                &mut new_tuple,
                                #[cfg(feature = "sparql-12")]
                                &dataset,
                            )? {
                                return Ok(None);
                            }
                            if let Some(graph_name_selector) = &graph_name_selector {
                                let Some(quad_graph_name) = quad.graph_name else {
                                    return Err(QueryEvaluationError::UnexpectedDefaultGraph);
                                };
                                if !put_pattern_value::<D>(
                                    graph_name_selector,
                                    quad_graph_name,
                                    &mut new_tuple,
                                    #[cfg(feature = "sparql-12")]
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
                let subject_selector = TupleSelector::from_ground_term_pattern(
                    subject,
                    encoded_variables,
                    &self.dataset,
                )?;
                let path = self.encode_property_path(path)?;
                let object_selector = TupleSelector::from_ground_term_pattern(
                    object,
                    encoded_variables,
                    &self.dataset,
                )?;
                let graph_name_selector = if let Some(graph_name) = graph_name.as_ref() {
                    Some(TupleSelector::from_named_node_pattern(
                        graph_name,
                        encoded_variables,
                        &self.dataset,
                    )?)
                } else {
                    None
                };
                let dataset = self.dataset.clone();
                Rc::new(move |from| {
                    let input_subject = match subject_selector.get_pattern_value(
                        &from,
                        #[cfg(feature = "sparql-12")]
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
                        #[cfg(feature = "sparql-12")]
                        &dataset,
                    ) {
                        Ok(value) => value,
                        Err(e) => return Box::new(once(Err(e))),
                    };
                    let input_graph_name = if let Some(graph_name_selector) = &graph_name_selector {
                        match graph_name_selector.get_pattern_value(
                            &from,
                            #[cfg(feature = "sparql-12")]
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
                            #[cfg(feature = "sparql-12")]
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
                                        if !put_pattern_value::<D>(
                                            &object_selector,
                                            o,
                                            &mut new_tuple,
                                            #[cfg(feature = "sparql-12")]
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
                            #[cfg(feature = "sparql-12")]
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
                                        if !put_pattern_value::<D>(
                                            &subject_selector,
                                            s,
                                            &mut new_tuple,
                                            #[cfg(feature = "sparql-12")]
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
                            #[cfg(feature = "sparql-12")]
                            let dataset = dataset.clone();
                            Box::new(
                                path_eval
                                    .eval_open_in_graph(&path, input_graph_name.as_ref())
                                    .map(move |t| {
                                        let (s, o) = t?;
                                        let mut new_tuple = from.clone();
                                        if !put_pattern_value::<D>(
                                            &subject_selector,
                                            s,
                                            &mut new_tuple,
                                            #[cfg(feature = "sparql-12")]
                                            &dataset,
                                        )? {
                                            return Ok(None);
                                        }
                                        if !put_pattern_value::<D>(
                                            &object_selector,
                                            o,
                                            &mut new_tuple,
                                            #[cfg(feature = "sparql-12")]
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
                            #[cfg(feature = "sparql-12")]
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
                                            if !put_pattern_value::<D>(
                                                graph_name_selector,
                                                g,
                                                &mut new_tuple,
                                                #[cfg(feature = "sparql-12")]
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
                            #[cfg(feature = "sparql-12")]
                            let dataset = dataset.clone();
                            Box::new(
                                path_eval
                                    .eval_from_in_unknown_graph(&path, &input_subject)
                                    .map(move |t| {
                                        let (o, g) = t?;
                                        let mut new_tuple = from.clone();
                                        if !put_pattern_value::<D>(
                                            &object_selector,
                                            o,
                                            &mut new_tuple,
                                            #[cfg(feature = "sparql-12")]
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
                                            if !put_pattern_value::<D>(
                                                graph_name_selector,
                                                g,
                                                &mut new_tuple,
                                                #[cfg(feature = "sparql-12")]
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
                            #[cfg(feature = "sparql-12")]
                            let dataset = dataset.clone();
                            Box::new(
                                path_eval
                                    .eval_to_in_unknown_graph(&path, &input_object)
                                    .map(move |t| {
                                        let (s, g) = t?;
                                        let mut new_tuple = from.clone();
                                        if !put_pattern_value::<D>(
                                            &subject_selector,
                                            s,
                                            &mut new_tuple,
                                            #[cfg(feature = "sparql-12")]
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
                                            if !put_pattern_value::<D>(
                                                graph_name_selector,
                                                g,
                                                &mut new_tuple,
                                                #[cfg(feature = "sparql-12")]
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
                            #[cfg(feature = "sparql-12")]
                            let dataset = dataset.clone();
                            Box::new(
                                path_eval
                                    .eval_open_in_unknown_graph(&path)
                                    .map(move |t| {
                                        let (s, o, g) = t?;
                                        let mut new_tuple = from.clone();
                                        if !put_pattern_value::<D>(
                                            &subject_selector,
                                            s,
                                            &mut new_tuple,
                                            #[cfg(feature = "sparql-12")]
                                            &dataset,
                                        )? {
                                            return Ok(None);
                                        }
                                        if !put_pattern_value::<D>(
                                            &object_selector,
                                            o,
                                            &mut new_tuple,
                                            #[cfg(feature = "sparql-12")]
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
                                            if !put_pattern_value::<D>(
                                                graph_name_selector,
                                                g,
                                                &mut new_tuple,
                                                #[cfg(feature = "sparql-12")]
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
                let graph_name_selector = TupleSelector::from_named_node_pattern(
                    graph_name,
                    encoded_variables,
                    &self.dataset,
                )?;
                let dataset = self.dataset.clone();
                Rc::new(move |from| {
                    let input_graph_name = match graph_name_selector.get_pattern_value(
                        &from,
                        #[cfg(feature = "sparql-12")]
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
                        #[cfg(feature = "sparql-12")]
                        let dataset = dataset.clone();
                        Box::new(
                            dataset
                                .internal_named_graphs()
                                .map(move |graph_name| {
                                    let graph_name = graph_name?;
                                    let mut new_tuple = from.clone();
                                    if !put_pattern_value::<D>(
                                        &graph_name_selector,
                                        graph_name,
                                        &mut new_tuple,
                                        #[cfg(feature = "sparql-12")]
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
                let left = left?;
                let right = right?;

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
                let left = left?;

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
                        let right = right?;
                        return Ok(Rc::new(move |from| {
                            Box::new(ForLoopLeftJoinIterator {
                                right_evaluator: Rc::clone(&right),
                                left_iter: left(from),
                                current_right: Box::new(empty()),
                                left_tuple_to_yield: None,
                            })
                        }));
                    }
                }
                let (right, right_stats) = self.graph_pattern_evaluator(right, encoded_variables);
                stat_children.push(right_stats);
                let right = right?;
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
                let left = left?;
                let right = right?;

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
                let left = left?;
                let right = right?;
                let expression = self.effective_boolean_value_expression_evaluator(
                    expression,
                    encoded_variables,
                    stat_children,
                )?;

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
                let child = child?;
                let expression = self.effective_boolean_value_expression_evaluator(
                    expression,
                    encoded_variables,
                    stat_children,
                )?;
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
                    .collect::<Result<Vec<_>, _>>()?;

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
                let child = child?;

                let position = encode_variable(encoded_variables, variable);
                if let Some(expression) = self.internal_expression_evaluator(
                    expression,
                    encoded_variables,
                    stat_children,
                )? {
                    return Ok(Rc::new(move |from| {
                        let expression = Rc::clone(&expression);
                        Box::new(child(from).map(move |tuple| {
                            let mut tuple = tuple?;
                            if let Some(value) = expression(&tuple) {
                                tuple.set(position, value);
                            }
                            Ok(tuple)
                        }))
                    }));
                }

                let expression =
                    self.expression_evaluator(expression, encoded_variables, stat_children)?;
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
                let child = child?;
                let by = expression
                    .iter()
                    .map(|comp| {
                        Ok(match comp {
                            OrderExpression::Asc(expression) => {
                                ComparatorFunction::Asc(self.expression_evaluator(
                                    expression,
                                    encoded_variables,
                                    stat_children,
                                )?)
                            }
                            OrderExpression::Desc(expression) => {
                                ComparatorFunction::Desc(self.expression_evaluator(
                                    expression,
                                    encoded_variables,
                                    stat_children,
                                )?)
                            }
                        })
                    })
                    .collect::<Result<Vec<_>, QueryEvaluationError>>()?;
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
                let child = child?;
                Rc::new(move |from| Box::new(hash_deduplicate(child(from))))
            }
            GraphPattern::Reduced { inner } => {
                let (child, child_stats) = self.graph_pattern_evaluator(inner, encoded_variables);
                stat_children.push(child_stats);
                let child = child?;
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
                let (child, child_stats) = self.graph_pattern_evaluator(inner, encoded_variables);
                stat_children.push(child_stats);
                let mut child = child?;
                #[expect(clippy::shadow_same)]
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
                let child = child?;
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
                let child = child?;
                let key_variables = variables
                    .iter()
                    .map(|k| encode_variable(encoded_variables, k))
                    .collect::<Rc<[_]>>();
                let accumulator_builders = aggregates
                    .iter()
                    .map(|(_, aggregate)| {
                        self.accumulator_builder(aggregate, encoded_variables, stat_children)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
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
                        Vec<AccumulatorWrapper<'_, D::InternalTerm>>,
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
                                accumulator.accumulate(&tuple);
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
                #[expect(clippy::shadow_same)]
                let silent = *silent;
                let service_name =
                    TupleSelector::from_named_node_pattern(name, encoded_variables, &self.dataset)?;
                self.build_graph_pattern_evaluator(inner, encoded_variables, &mut Vec::new())?; // We call recursively to fill "encoded_variables"
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
        })
    }

    fn evaluate_service(
        &self,
        service_name: &TupleSelector<D::InternalTerm>,
        graph_pattern: &spargebra::algebra::GraphPattern,
        variables: Rc<[Variable]>,
        from: &InternalTuple<D::InternalTerm>,
    ) -> Result<InternalTuplesIterator<'a, D::InternalTerm>, QueryEvaluationError> {
        let service_name = service_name
            .get_pattern_value(
                from,
                #[cfg(feature = "sparql-12")]
                &self.dataset,
            )?
            .ok_or(QueryEvaluationError::UnboundService)?;
        let service_name = match self.dataset.externalize_term(service_name)? {
            Term::NamedNode(service_name) => service_name,
            term => return Err(QueryEvaluationError::InvalidServiceName(term)),
        };
        let iter =
            self.service_handler
                .handle(&service_name, graph_pattern, self.base_iri.as_deref())?;
        Ok(encode_bindings(self.dataset.clone(), variables, iter))
    }

    fn accumulator_builder(
        &self,
        expression: &AggregateExpression,
        encoded_variables: &mut Vec<Variable>,
        stat_children: &mut Vec<Rc<EvalNodeWithStats>>,
    ) -> Result<Box<dyn Fn() -> AccumulatorWrapper<'a, D::InternalTerm> + 'a>, QueryEvaluationError>
    {
        Ok(match expression {
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
                        self.internal_expression_evaluator(expr, encoded_variables, stat_children)?
                    {
                        return Ok(if *distinct {
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
                        });
                    }
                    let evaluator =
                        self.expression_evaluator(expr, encoded_variables, stat_children)?;
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
                        self.expression_evaluator(expr, encoded_variables, stat_children)?;
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
                        self.expression_evaluator(expr, encoded_variables, stat_children)?;
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
                        self.expression_evaluator(expr, encoded_variables, stat_children)?;
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
                        self.expression_evaluator(expr, encoded_variables, stat_children)?;
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
                        self.expression_evaluator(expr, encoded_variables, stat_children)?;
                    Box::new(move || AccumulatorWrapper::Sample {
                        evaluator: Rc::clone(&evaluator),
                        value: None,
                    })
                }
                AggregateFunction::GroupConcat { separator } => {
                    let separator = Rc::from(separator.as_deref().unwrap_or(" "));
                    let evaluator =
                        self.expression_evaluator(expr, encoded_variables, stat_children)?;
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
                AggregateFunction::Custom(function_name) => {
                    let Some(function) = self.custom_aggregate_functions.get(function_name) else {
                        return Err(QueryEvaluationError::UnsupportedCustomFunction(
                            function_name.clone(),
                        ));
                    };
                    let evaluator =
                        self.expression_evaluator(expr, encoded_variables, stat_children)?;
                    let function = Arc::clone(function);
                    if *distinct {
                        Box::new(move || AccumulatorWrapper::DistinctExpression {
                            evaluator: Rc::clone(&evaluator),
                            seen: FxHashSet::default(),
                            accumulator: Some(Box::new(CustomAccumulator(function()))),
                        })
                    } else {
                        Box::new(move || AccumulatorWrapper::Expression {
                            evaluator: Rc::clone(&evaluator),
                            accumulator: Some(Box::new(CustomAccumulator(function()))),
                        })
                    }
                }
            },
        })
    }

    /// Evaluates an expression and returns an internal term
    ///
    /// Returns None if building such expression would mean to convert back to an internal term at the end.
    #[expect(clippy::type_complexity)]
    fn internal_expression_evaluator(
        &self,
        expression: &Expression,
        encoded_variables: &mut Vec<Variable>,
        stat_children: &mut Vec<Rc<EvalNodeWithStats>>,
    ) -> Result<
        Option<Rc<dyn Fn(&InternalTuple<D::InternalTerm>) -> Option<D::InternalTerm> + 'a>>,
        QueryEvaluationError,
    > {
        Ok(try_build_internal_expression_evaluator(
            expression,
            &mut ExpressionContext {
                evaluator: self,
                encoded_variables,
                stat_children,
            },
        )?)
    }

    /// Evaluate an expression and return its effective boolean value
    pub(crate) fn effective_boolean_value_expression_evaluator(
        &self,
        expression: &Expression,
        encoded_variables: &mut Vec<Variable>,
        stat_children: &mut Vec<Rc<EvalNodeWithStats>>,
    ) -> Result<ExpressionEvaluator<'a, InternalTuple<D::InternalTerm>, bool>, QueryEvaluationError>
    {
        // TODO: avoid dyn?
        if let Some(eval) =
            self.internal_expression_evaluator(expression, encoded_variables, stat_children)?
        {
            let dataset = self.dataset.clone();
            return Ok(Rc::new(move |tuple| {
                dataset
                    .internal_term_effective_boolean_value(eval(tuple)?)
                    .ok()?
            }));
        }
        let eval = self.expression_evaluator(expression, encoded_variables, stat_children)?;
        Ok(Rc::new(move |tuple| eval(tuple)?.effective_boolean_value()))
    }

    /// Evaluate an expression and return an explicit ExpressionTerm
    pub(crate) fn expression_evaluator(
        &self,
        expression: &Expression,
        encoded_variables: &mut Vec<Variable>,
        stat_children: &mut Vec<Rc<EvalNodeWithStats>>,
    ) -> Result<
        ExpressionEvaluator<'a, InternalTuple<D::InternalTerm>, ExpressionTerm>,
        QueryEvaluationError,
    > {
        Ok(build_expression_evaluator(
            expression,
            &mut ExpressionContext {
                evaluator: self,
                encoded_variables,
                stat_children,
            },
        )?)
    }

    fn encode_term(&self, term: impl Into<Term>) -> Result<D::InternalTerm, QueryEvaluationError> {
        self.dataset.internalize_term(term.into())
    }

    #[cfg(feature = "sparql-12")]
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
    ) -> Result<Rc<PropertyPath<D::InternalTerm>>, QueryEvaluationError> {
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

impl<'a, D: QueryableDataset<'a>> Clone for SimpleEvaluator<'a, D> {
    fn clone(&self) -> Self {
        Self {
            dataset: self.dataset.clone(),
            base_iri: self.base_iri.clone(),
            now: self.now,
            service_handler: Rc::clone(&self.service_handler),
            custom_functions: Rc::clone(&self.custom_functions),
            custom_aggregate_functions: Rc::clone(&self.custom_aggregate_functions),
            run_stats: self.run_stats,
        }
    }
}

struct ExpressionContext<'a, E> {
    evaluator: &'a E,
    encoded_variables: &'a mut Vec<Variable>,
    stat_children: &'a mut Vec<Rc<EvalNodeWithStats>>,
}

impl<'a, 'b, D: QueryableDataset<'a>> ExpressionEvaluatorContext<'a>
    for ExpressionContext<'b, SimpleEvaluator<'a, D>>
{
    type Tuple = InternalTuple<D::InternalTerm>;
    type Term = D::InternalTerm;
    type Error = QueryEvaluationError;

    fn build_variable_lookup(
        &mut self,
        variable: &Variable,
    ) -> impl Fn(&InternalTuple<D::InternalTerm>) -> Option<D::InternalTerm> + 'a {
        let variable = encode_variable(self.encoded_variables, variable);
        move |tuple| tuple.get(variable).cloned()
    }

    fn build_is_variable_bound(
        &mut self,
        variable: &Variable,
    ) -> impl Fn(&InternalTuple<D::InternalTerm>) -> bool + 'a {
        let variable = encode_variable(self.encoded_variables, variable);
        move |tuple| tuple.contains(variable)
    }

    fn build_exists(
        &mut self,
        plan: &GraphPattern,
    ) -> Result<impl Fn(&InternalTuple<D::InternalTerm>) -> bool + 'a, QueryEvaluationError> {
        let (eval, stats) = self
            .evaluator
            .graph_pattern_evaluator(plan, self.encoded_variables);
        self.stat_children.push(stats);
        let eval = eval?;
        Ok(move |tuple: &InternalTuple<D::InternalTerm>| eval(tuple.clone()).next().is_some())
    }

    fn internalize_named_node(
        &mut self,
        term: &NamedNode,
    ) -> Result<Self::Term, QueryEvaluationError> {
        self.evaluator.encode_term(term.clone())
    }

    fn internalize_literal(&mut self, term: &Literal) -> Result<Self::Term, QueryEvaluationError> {
        self.evaluator.encode_term(term.clone())
    }

    fn build_internalize_expression_term(
        &mut self,
    ) -> impl Fn(ExpressionTerm) -> Option<Self::Term> + 'a {
        let dataset = self.evaluator.dataset.clone();
        move |t| dataset.internalize_expression_term(t).ok()
    }

    fn build_externalize_expression_term(
        &mut self,
    ) -> impl Fn(Self::Term) -> Option<ExpressionTerm> + 'a {
        let dataset = self.evaluator.dataset.clone();
        move |t| dataset.externalize_expression_term(t).ok()
    }

    fn now(&mut self) -> DateTime {
        self.evaluator.now
    }

    fn base_iri(&mut self) -> Option<Arc<Iri<String>>> {
        self.evaluator.base_iri.as_ref().map(Arc::clone)
    }

    fn custom_functions(&mut self) -> &CustomFunctionRegistry {
        &self.evaluator.custom_functions
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
            direction,
        } => Some((value, Some((language, Some(direction))))),
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
    if let Some((language, direction)) = language {
        if let Some(direction) = direction {
            ExpressionTerm::DirLangStringLiteral {
                value,
                language,
                direction,
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

fn decode_bindings<'a, D: QueryableDataset<'a>>(
    dataset: EvalDataset<'a, D>,
    iter: InternalTuplesIterator<'a, D::InternalTerm>,
    variables: Arc<[Variable]>,
) -> QuerySolutionIter<'a> {
    let tuple_size = variables.len();
    QuerySolutionIter::from_tuples(
        variables,
        Box::new(iter.map(move |values| {
            let mut result = vec![None; tuple_size];
            for (i, value) in values?.iter().enumerate() {
                if let Some(term) = value {
                    result[i] = Some(dataset.externalize_term(term)?)
                }
            }
            Ok(result)
        })),
    )
}

// this is used to encode results from a BindingIterator into an InternalTuplesIterator. This happens when SERVICE clauses are evaluated
fn encode_bindings<'a, D: QueryableDataset<'a>>(
    dataset: EvalDataset<'a, D>,
    variables: Rc<[Variable]>,
    iter: QuerySolutionIter<'a>,
) -> InternalTuplesIterator<'a, D::InternalTerm> {
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

fn encode_initial_bindings<'a, D: QueryableDataset<'a>>(
    dataset: &EvalDataset<'a, D>,
    variables: &[Variable],
    values: impl IntoIterator<Item = (Variable, Term)>,
) -> Result<InternalTuple<D::InternalTerm>, QueryEvaluationError> {
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

fn put_variable_value<T: Clone>(
    selector: &Variable,
    variables: &[Variable],
    value: T,
    tuple: &mut InternalTuple<T>,
) -> bool {
    for (i, v) in variables.iter().enumerate() {
        if selector == v {
            tuple.set(i, value);
            return true;
        }
    }
    false
}

enum AccumulatorWrapper<'a, T> {
    CountTuple {
        count: u64,
    },
    CountDistinctTuple {
        seen: FxHashSet<InternalTuple<T>>,
        count: u64,
    },
    CountInternal {
        evaluator: Rc<dyn Fn(&InternalTuple<T>) -> Option<T> + 'a>,
        count: u64,
    },
    CountDistinctInternal {
        seen: FxHashSet<T>,
        evaluator: Rc<dyn Fn(&InternalTuple<T>) -> Option<T> + 'a>,
        count: u64,
    },
    Sample {
        // TODO: add internal variant
        evaluator: Rc<dyn Fn(&InternalTuple<T>) -> Option<ExpressionTerm> + 'a>,
        value: Option<ExpressionTerm>,
    },
    Expression {
        evaluator: Rc<dyn Fn(&InternalTuple<T>) -> Option<ExpressionTerm> + 'a>,
        accumulator: Option<Box<dyn Accumulator>>,
    },
    DistinctExpression {
        seen: FxHashSet<ExpressionTerm>,
        evaluator: Rc<dyn Fn(&InternalTuple<T>) -> Option<ExpressionTerm> + 'a>,
        accumulator: Option<Box<dyn Accumulator>>,
    },
}

impl<T: Clone + Eq + Hash> AccumulatorWrapper<'_, T> {
    fn accumulate(&mut self, tuple: &InternalTuple<T>) {
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
                accumulator.accumulate(value);
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
                    accumulator.accumulate(value);
                }
            }
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
        }
    }
}

trait Accumulator {
    fn accumulate(&mut self, element: ExpressionTerm);

    fn finish(&mut self) -> Option<ExpressionTerm>;
}

#[derive(Default, Debug)]
struct CountAccumulator {
    count: i64,
}

impl Accumulator for CountAccumulator {
    fn accumulate(&mut self, _element: ExpressionTerm) {
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
    fn accumulate(&mut self, element: ExpressionTerm) {
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
    fn accumulate(&mut self, element: ExpressionTerm) {
        self.sum.accumulate(element);
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
#[expect(clippy::option_option)]
struct MinAccumulator {
    min: Option<Option<ExpressionTerm>>,
}

impl Accumulator for MinAccumulator {
    fn accumulate(&mut self, element: ExpressionTerm) {
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
#[expect(clippy::option_option)]
struct MaxAccumulator {
    max: Option<Option<ExpressionTerm>>,
}

impl Accumulator for MaxAccumulator {
    fn accumulate(&mut self, element: ExpressionTerm) {
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

#[expect(clippy::option_option)]
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
    fn accumulate(&mut self, element: ExpressionTerm) {
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

struct CustomAccumulator(Box<dyn AggregateFunctionAccumulator + Send + Sync>);

impl Accumulator for CustomAccumulator {
    fn accumulate(&mut self, element: ExpressionTerm) {
        self.0.accumulate(element.into())
    }

    fn finish(&mut self) -> Option<ExpressionTerm> {
        Some(self.0.finish()?.into())
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
                #[cfg(feature = "sparql-12")]
                ExpressionTerm::Triple(a) => match b {
                    ExpressionTerm::Triple(b) => cmp_triples(a, b),
                    _ => Ordering::Greater,
                },
                _ => match b {
                    ExpressionTerm::NamedNode(_) | ExpressionTerm::BlankNode(_) => {
                        Ordering::Greater
                    }
                    #[cfg(feature = "sparql-12")]
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

#[cfg(feature = "sparql-12")]
fn cmp_triples(a: &ExpressionTriple, b: &ExpressionTriple) -> Ordering {
    match match &a.subject {
        NamedOrBlankNode::BlankNode(a) => match &b.subject {
            NamedOrBlankNode::BlankNode(b) => a.as_str().cmp(b.as_str()),
            NamedOrBlankNode::NamedNode(_) => Ordering::Less,
        },
        NamedOrBlankNode::NamedNode(a) => match &b.subject {
            NamedOrBlankNode::BlankNode(_) => Ordering::Greater,
            NamedOrBlankNode::NamedNode(b) => a.as_str().cmp(b.as_str()),
        },
    } {
        Ordering::Equal => match a.predicate.as_str().cmp(b.predicate.as_str()) {
            Ordering::Equal => cmp_terms(Some(&a.object), Some(&b.object)),
            o => o,
        },
        o => o,
    }
}

enum TupleSelector<T> {
    Constant(T),
    Variable(usize),
    #[cfg(feature = "sparql-12")]
    TriplePattern(Rc<TripleTupleSelector<T>>),
}

impl<T> TupleSelector<T> {
    fn from_ground_term_pattern<'a>(
        term_pattern: &GroundTermPattern,
        variables: &mut Vec<Variable>,
        dataset: &EvalDataset<'a, impl QueryableDataset<'a, InternalTerm = T>>,
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
            #[cfg(feature = "sparql-12")]
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

    fn from_named_node_pattern<'a>(
        named_node_pattern: &NamedNodePattern,
        variables: &mut Vec<Variable>,
        dataset: &EvalDataset<'a, impl QueryableDataset<'a, InternalTerm = T>>,
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
}

impl<T: Clone> TupleSelector<T> {
    #[cfg_attr(
        not(feature = "sparql-12"),
        expect(
            unused_lifetimes,
            clippy::unnecessary_wraps,
            clippy::extra_unused_lifetimes
        )
    )]
    fn get_pattern_value<'a>(
        &self,
        tuple: &InternalTuple<T>,
        #[cfg(feature = "sparql-12")] dataset: &EvalDataset<
            'a,
            impl QueryableDataset<'a, InternalTerm = T>,
        >,
    ) -> Result<Option<T>, QueryEvaluationError> {
        Ok(match self {
            Self::Constant(c) => Some(c.clone()),
            Self::Variable(v) => tuple.get(*v).cloned(),
            #[cfg(feature = "sparql-12")]
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

impl<T: Clone> Clone for TupleSelector<T> {
    fn clone(&self) -> Self {
        match self {
            Self::Constant(c) => Self::Constant(c.clone()),
            Self::Variable(v) => Self::Variable(*v),
            #[cfg(feature = "sparql-12")]
            Self::TriplePattern(t) => Self::TriplePattern(Rc::clone(t)),
        }
    }
}

#[cfg(feature = "sparql-12")]
struct TripleTupleSelector<T> {
    subject: TupleSelector<T>,
    predicate: TupleSelector<T>,
    object: TupleSelector<T>,
}

#[cfg_attr(not(feature = "sparql-12"), expect(clippy::unnecessary_wraps))]
fn put_pattern_value<'a, D: QueryableDataset<'a>>(
    selector: &TupleSelector<D::InternalTerm>,
    value: D::InternalTerm,
    tuple: &mut InternalTuple<D::InternalTerm>,
    #[cfg(feature = "sparql-12")] dataset: &EvalDataset<'a, D>,
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
        #[cfg(feature = "sparql-12")]
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

pub fn are_compatible_and_not_disjointed<T: Clone + Eq>(
    a: &InternalTuple<T>,
    b: &InternalTuple<T>,
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

pub enum PropertyPath<T> {
    Path(T),
    Reverse(Rc<Self>),
    Sequence(Rc<Self>, Rc<Self>),
    Alternative(Rc<Self>, Rc<Self>),
    ZeroOrMore(Rc<Self>),
    OneOrMore(Rc<Self>),
    ZeroOrOne(Rc<Self>),
    NegatedPropertySet(Rc<[T]>),
}

struct PathEvaluator<'a, D: QueryableDataset<'a>> {
    dataset: EvalDataset<'a, D>,
}

impl<'a, D: QueryableDataset<'a>> PathEvaluator<'a, D> {
    fn eval_closed_in_graph(
        &self,
        path: &PropertyPath<D::InternalTerm>,
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
                        if ps.contains(&t.predicate) {
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
        path: &PropertyPath<D::InternalTerm>,
        start: &D::InternalTerm,
        end: &D::InternalTerm,
    ) -> Box<dyn Iterator<Item = Result<Option<D::InternalTerm>, QueryEvaluationError>> + 'a> {
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
                                if ps.contains(&t.predicate) {
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
        path: &PropertyPath<D::InternalTerm>,
        start: &D::InternalTerm,
        graph_name: Option<&D::InternalTerm>,
    ) -> Box<dyn Iterator<Item = Result<D::InternalTerm, QueryEvaluationError>> + 'a> {
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
                                if ps.contains(&t.predicate) {
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
        path: &PropertyPath<D::InternalTerm>,
        start: &D::InternalTerm,
    ) -> Box<
        dyn Iterator<
                Item = Result<(D::InternalTerm, Option<D::InternalTerm>), QueryEvaluationError>,
            > + 'a,
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
                                if ps.contains(&t.predicate) {
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
        path: &PropertyPath<D::InternalTerm>,
        end: &D::InternalTerm,
        graph_name: Option<&D::InternalTerm>,
    ) -> Box<dyn Iterator<Item = Result<D::InternalTerm, QueryEvaluationError>> + 'a> {
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
                                if ps.contains(&t.predicate) {
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
        path: &PropertyPath<D::InternalTerm>,
        end: &D::InternalTerm,
    ) -> Box<
        dyn Iterator<
                Item = Result<(D::InternalTerm, Option<D::InternalTerm>), QueryEvaluationError>,
            > + 'a,
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
                                if ps.contains(&t.predicate) {
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
        path: &PropertyPath<D::InternalTerm>,
        graph_name: Option<&D::InternalTerm>,
    ) -> Box<
        dyn Iterator<Item = Result<(D::InternalTerm, D::InternalTerm), QueryEvaluationError>> + 'a,
    > {
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
                                if ps.contains(&t.predicate) {
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
        path: &PropertyPath<D::InternalTerm>,
    ) -> Box<
        dyn Iterator<
                Item = Result<
                    (D::InternalTerm, D::InternalTerm, Option<D::InternalTerm>),
                    QueryEvaluationError,
                >,
            > + 'a,
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
                                if ps.contains(&t.predicate) {
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
    + use<'a, D> {
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
    > + use<'a, D> {
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
        T: 'a,
        I: Iterator<Item = Result<T, QueryEvaluationError>> + 'a,
    >(
        &self,
        term: &D::InternalTerm,
        graph_name: Option<&D::InternalTerm>,
        f: impl FnOnce() -> I,
    ) -> Box<dyn Iterator<Item = Result<T, QueryEvaluationError>> + 'a> {
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
        T: 'a,
        I: IntoIterator<Item = Result<T, QueryEvaluationError>> + 'a,
    >(
        &self,
        term: &D::InternalTerm,
        f: impl FnMut(Option<D::InternalTerm>) -> I + 'a,
    ) -> Box<dyn Iterator<Item = Result<T, QueryEvaluationError>> + 'a> {
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
    ) -> impl Iterator<Item = Result<Option<D::InternalTerm>, QueryEvaluationError>> + use<'a, D>
    {
        self.dataset
            .internal_quads_for_pattern(Some(term), None, None, None)
            .chain(
                self.dataset
                    .internal_quads_for_pattern(None, None, Some(term), None),
            )
            .map(|q| Ok(q?.graph_name))
    }
}

impl<'a, D: QueryableDataset<'a>> Clone for PathEvaluator<'a, D> {
    fn clone(&self) -> Self {
        Self {
            dataset: self.dataset.clone(),
        }
    }
}

struct CartesianProductJoinIterator<'a, T> {
    probe_iter: Peekable<InternalTuplesIterator<'a, T>>,
    built: Vec<InternalTuple<T>>,
    buffered_results: Vec<Result<InternalTuple<T>, QueryEvaluationError>>,
}

impl<T: Clone + Eq> Iterator for CartesianProductJoinIterator<'_, T> {
    type Item = Result<InternalTuple<T>, QueryEvaluationError>;

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

struct HashJoinIterator<'a, T> {
    probe_iter: Peekable<InternalTuplesIterator<'a, T>>,
    built: InternalTupleSet<T>,
    buffered_results: Vec<Result<InternalTuple<T>, QueryEvaluationError>>,
}

impl<T: Clone + Eq + Hash> Iterator for HashJoinIterator<'_, T> {
    type Item = Result<InternalTuple<T>, QueryEvaluationError>;

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

struct HashLeftJoinIterator<'a, T> {
    left_iter: InternalTuplesIterator<'a, T>,
    right: InternalTupleSet<T>,
    buffered_results: Vec<Result<InternalTuple<T>, QueryEvaluationError>>,
    expression: Rc<dyn Fn(&InternalTuple<T>) -> Option<bool> + 'a>,
}

impl<T: Clone + Eq + Hash> Iterator for HashLeftJoinIterator<'_, T> {
    type Item = Result<InternalTuple<T>, QueryEvaluationError>;

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
struct ForLoopLeftJoinIterator<'a, T> {
    right_evaluator: InternalTupleEvaluator<'a, T>,
    left_iter: InternalTuplesIterator<'a, T>,
    current_right: InternalTuplesIterator<'a, T>,
    left_tuple_to_yield: Option<InternalTuple<T>>,
}

#[cfg(feature = "sep-0006")]
impl<T: Clone> Iterator for ForLoopLeftJoinIterator<'_, T> {
    type Item = Result<InternalTuple<T>, QueryEvaluationError>;

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

struct UnionIterator<'a, T> {
    plans: Vec<Rc<dyn Fn(InternalTuple<T>) -> InternalTuplesIterator<'a, T> + 'a>>,
    input: InternalTuple<T>,
    current_iterator: InternalTuplesIterator<'a, T>,
    current_plan: usize,
}

impl<T: Clone> Iterator for UnionIterator<'_, T> {
    type Item = Result<InternalTuple<T>, QueryEvaluationError>;

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

struct ConsecutiveDeduplication<'a, T> {
    inner: InternalTuplesIterator<'a, T>,
    current: Option<InternalTuple<T>>,
}

impl<T: Eq> Iterator for ConsecutiveDeduplication<'_, T> {
    type Item = Result<InternalTuple<T>, QueryEvaluationError>;

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

struct ConstructIterator<'a, D: QueryableDataset<'a>> {
    eval: SimpleEvaluator<'a, D>,
    iter: InternalTuplesIterator<'a, D::InternalTerm>,
    template: Vec<TripleTemplate>,
    buffered_results: Vec<Result<Triple, QueryEvaluationError>>,
    already_emitted_results: FxHashSet<Triple>,
    bnodes: Vec<BlankNode>,
}

impl<'a, D: QueryableDataset<'a>> Iterator for ConstructIterator<'a, D> {
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
                        #[cfg(feature = "sparql-12")]
                        let new_triple = triple.subject.is_blank_node()
                            || triple.object.is_blank_node()
                            || triple.object.is_triple()
                            || self.already_emitted_results.insert(triple.clone());
                        #[cfg(not(feature = "sparql-12"))]
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
    #[cfg(feature = "sparql-12")]
    Triple(Box<TripleTemplate>),
}

impl TripleTemplateValue {
    #[cfg_attr(not(feature = "sparql-12"), expect(clippy::unnecessary_wraps))]
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
            #[cfg(feature = "sparql-12")]
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

fn get_triple_template_value<'a, D: QueryableDataset<'a>>(
    selector: &TripleTemplateValue,
    tuple: &InternalTuple<D::InternalTerm>,
    bnodes: &mut Vec<BlankNode>,
    dataset: &EvalDataset<'a, D>,
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
        #[cfg(feature = "sparql-12")]
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

struct DescribeIterator<'a, D: QueryableDataset<'a>> {
    eval: SimpleEvaluator<'a, D>,
    tuples_to_describe: InternalTuplesIterator<'a, D::InternalTerm>,
    nodes_described: FxHashSet<D::InternalTerm>,
    nodes_to_describe: Vec<D::InternalTerm>,
    quads:
        Box<dyn Iterator<Item = Result<InternalQuad<D::InternalTerm>, QueryEvaluationError>> + 'a>,
}

impl<'a, D: QueryableDataset<'a>> Iterator for DescribeIterator<'a, D> {
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
                self.quads = self.eval.dataset.internal_quads_for_pattern(
                    Some(&node_to_describe),
                    None,
                    None,
                    Some(None),
                );
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

enum ComparatorFunction<'a, T> {
    Asc(Rc<dyn Fn(&InternalTuple<T>) -> Option<ExpressionTerm> + 'a>),
    Desc(Rc<dyn Fn(&InternalTuple<T>) -> Option<ExpressionTerm> + 'a>),
}

struct InternalTupleSet<T> {
    key: Vec<usize>,
    map: FxHashMap<u64, Vec<InternalTuple<T>>>,
    len: usize,
}

impl<T> InternalTupleSet<T> {
    fn new(key: Vec<usize>) -> Self {
        Self {
            key,
            map: FxHashMap::default(),
            len: 0,
        }
    }

    fn len(&self) -> usize {
        self.len
    }

    fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl<T: Hash> InternalTupleSet<T> {
    fn insert(&mut self, tuple: InternalTuple<T>) {
        self.map
            .entry(self.tuple_key(&tuple))
            .or_default()
            .push(tuple);
        self.len += 1;
    }

    fn get(&self, tuple: &InternalTuple<T>) -> &[InternalTuple<T>] {
        self.map.get(&self.tuple_key(tuple)).map_or(&[], |v| v)
    }

    fn tuple_key(&self, tuple: &InternalTuple<T>) -> u64 {
        let mut hasher = FxHasher::default();
        for v in &self.key {
            if let Some(val) = tuple.get(*v) {
                val.hash(&mut hasher);
            }
        }
        hasher.finish()
    }
}

impl<T: Hash> Extend<InternalTuple<T>> for InternalTupleSet<T> {
    fn extend<I: IntoIterator<Item = InternalTuple<T>>>(&mut self, iter: I) {
        let iter = iter.into_iter();
        self.map.reserve(iter.size_hint().0);
        for tuple in iter {
            self.insert(tuple);
        }
    }
}

struct StatsIterator<'a, T> {
    inner: InternalTuplesIterator<'a, T>,
    stats: Rc<EvalNodeWithStats>,
}

impl<T> Iterator for StatsIterator<'_, T> {
    type Item = Result<InternalTuple<T>, QueryEvaluationError>;

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
    pub(crate) fn empty() -> Self {
        Self {
            label: String::new(),
            children: Vec::new(),
            exec_count: Cell::new(0),
            exec_duration: Cell::new(None),
        }
    }

    pub fn json_node(
        &self,
        serializer: &mut WriterJsonSerializer<impl io::Write>,
        with_stats: bool,
    ) -> io::Result<()> {
        serializer.serialize_event(JsonEvent::StartObject)?;
        serializer.serialize_event(JsonEvent::ObjectKey("name".into()))?;
        serializer.serialize_event(JsonEvent::String((&self.label).into()))?;
        if with_stats {
            serializer.serialize_event(JsonEvent::ObjectKey("number of results".into()))?;
            serializer
                .serialize_event(JsonEvent::Number(self.exec_count.get().to_string().into()))?;
            if let Some(duration) = self.exec_duration.get() {
                serializer.serialize_event(JsonEvent::ObjectKey("duration in seconds".into()))?;
                serializer
                    .serialize_event(JsonEvent::Number(duration.as_seconds().to_string().into()))?;
            }
        }
        serializer.serialize_event(JsonEvent::ObjectKey("children".into()))?;
        serializer.serialize_event(JsonEvent::StartArray)?;
        for child in &self.children {
            child.json_node(serializer, with_stats)?;
        }
        serializer.serialize_event(JsonEvent::EndArray)?;
        serializer.serialize_event(JsonEvent::EndObject)
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
        GraphPattern::Values {
            variables,
            bindings,
        } => {
            format!(
                "StaticBindings(({}), ({}))",
                format_list(variables),
                format_list(bindings.iter().map(|b| {
                    format!(
                        "({})",
                        format_list(b.iter().map(|t| {
                            t.as_ref()
                                .map_or_else(|| "UNDEF".into(), GroundTerm::to_string)
                        }))
                    )
                }))
            )
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

/// A token that can be used to mark something as canceled.
///
/// To cancel run [`CancellationToken::cancel`] and to check if the token is canceled run [`CancellationToken::is_cancelled`].
#[derive(Clone, Default)]
pub struct CancellationToken {
    value: Arc<AtomicBool>,
}

impl CancellationToken {
    #[inline]
    pub fn new() -> Self {
        Self {
            value: Arc::new(AtomicBool::new(false)),
        }
    }

    #[inline]
    pub fn cancel(&self) {
        self.value.store(true, atomic::Ordering::Relaxed);
    }

    #[inline]
    pub fn is_cancelled(&self) -> bool {
        self.value.load(atomic::Ordering::Relaxed)
    }

    fn ensure_alive(&self) -> Result<(), QueryEvaluationError> {
        if self.is_cancelled() {
            Err(QueryEvaluationError::Cancelled)
        } else {
            Ok(())
        }
    }
}
