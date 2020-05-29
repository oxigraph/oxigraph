use crate::model::xsd::*;
use crate::model::BlankNode;
use crate::model::Triple;
use crate::sparql::algebra::GraphPattern;
use crate::sparql::model::*;
use crate::sparql::plan::*;
use crate::sparql::ServiceHandler;
use crate::store::numeric_encoder::*;
use crate::store::ReadableEncodedStore;
use crate::Error;
use crate::Result;
use digest::Digest;
use md5::Md5;
use oxilangtag::LanguageTag;
use oxiri::Iri;
use rand::random;
use regex::{Regex, RegexBuilder};
use rio_api::model as rio;
use sha1::Sha1;
use sha2::{Sha256, Sha384, Sha512};
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::convert::{TryFrom, TryInto};
use std::hash::Hash;
use std::iter::Iterator;
use std::iter::{empty, once};
use std::str;
use std::sync::Mutex;

const REGEX_SIZE_LIMIT: usize = 1_000_000;

type EncodedTuplesIterator<'a> = Box<dyn Iterator<Item = Result<EncodedTuple>> + 'a>;

pub(crate) struct SimpleEvaluator<S: ReadableEncodedStore> {
    dataset: DatasetView<S>,
    base_iri: Option<Iri<String>>,
    bnodes_map: Mutex<BTreeMap<StrHash, u128>>,
    now: DateTime,
    service_handler: Box<dyn ServiceHandler>,
}

impl<'a, S: ReadableEncodedStore + 'a> SimpleEvaluator<S> {
    pub fn new(
        dataset: DatasetView<S>,
        base_iri: Option<Iri<String>>,
        service_handler: Box<dyn ServiceHandler>,
    ) -> Self {
        Self {
            dataset,
            bnodes_map: Mutex::new(BTreeMap::default()),
            base_iri,
            now: DateTime::now().unwrap(),
            service_handler,
        }
    }

    pub fn evaluate_select_plan<'b>(
        &'b self,
        plan: &'b PlanNode,
        variables: &[Variable],
    ) -> Result<QueryResult<'b>>
    where
        'a: 'b,
    {
        let iter = self.eval_plan(plan, EncodedTuple::with_capacity(variables.len()));
        Ok(QueryResult::Bindings(
            self.decode_bindings(iter, variables.to_vec()),
        ))
    }

    pub fn evaluate_ask_plan<'b>(&'b self, plan: &'b PlanNode) -> Result<QueryResult<'b>>
    where
        'a: 'b,
    {
        match self
            .eval_plan(
                plan,
                EncodedTuple::with_capacity(plan.maybe_bound_variables().len()),
            )
            .next()
        {
            Some(Ok(_)) => Ok(QueryResult::Boolean(true)),
            Some(Err(error)) => Err(error),
            None => Ok(QueryResult::Boolean(false)),
        }
    }

    pub fn evaluate_construct_plan<'b>(
        &'b self,
        plan: &'b PlanNode,
        construct: &'b [TripleTemplate],
    ) -> Result<QueryResult<'b>>
    where
        'a: 'b,
    {
        Ok(QueryResult::Graph(Box::new(ConstructIterator {
            eval: self,
            iter: self.eval_plan(
                plan,
                EncodedTuple::with_capacity(plan.maybe_bound_variables().len()),
            ),
            template: construct,
            buffered_results: Vec::default(),
            bnodes: Vec::default(),
        })))
    }

    pub fn evaluate_describe_plan<'b>(&'b self, plan: &'b PlanNode) -> Result<QueryResult<'b>>
    where
        'a: 'b,
    {
        Ok(QueryResult::Graph(Box::new(DescribeIterator {
            eval: self,
            iter: self.eval_plan(
                plan,
                EncodedTuple::with_capacity(plan.maybe_bound_variables().len()),
            ),
            quads: Box::new(empty()),
        })))
    }

    fn eval_plan<'b>(&'b self, node: &'b PlanNode, from: EncodedTuple) -> EncodedTuplesIterator<'b>
    where
        'a: 'b,
    {
        match node {
            PlanNode::Init => Box::new(once(Ok(from))),
            PlanNode::StaticBindings { tuples } => Box::new(tuples.iter().cloned().map(Ok)),
            PlanNode::Service {
                variables,
                silent,
                service_name,
                graph_pattern,
                ..
            } => match self.evaluate_service(service_name, graph_pattern, variables, &from) {
                Ok(result) => Box::new(result.flat_map(move |binding| {
                    binding
                        .map(|binding| binding.combine_with(&from))
                        .transpose()
                })),
                Err(e) => {
                    if *silent {
                        Box::new(once(Ok(from)))
                    } else {
                        Box::new(once(Err(e)))
                    }
                }
            },
            PlanNode::QuadPatternJoin {
                child,
                subject,
                predicate,
                object,
                graph_name,
            } => Box::new(self.eval_plan(&*child, from).flat_map_ok(move |tuple| {
                let mut iter = self.dataset.quads_for_pattern(
                    get_pattern_value(subject, &tuple),
                    get_pattern_value(predicate, &tuple),
                    get_pattern_value(object, &tuple),
                    get_pattern_value(graph_name, &tuple),
                );
                if subject.is_var() && subject == predicate {
                    iter = Box::new(iter.filter(|quad| match quad {
                        Err(_) => true,
                        Ok(quad) => quad.subject == quad.predicate,
                    }))
                }
                if subject.is_var() && subject == object {
                    iter = Box::new(iter.filter(|quad| match quad {
                        Err(_) => true,
                        Ok(quad) => quad.subject == quad.object,
                    }))
                }
                if predicate.is_var() && predicate == object {
                    iter = Box::new(iter.filter(|quad| match quad {
                        Err(_) => true,
                        Ok(quad) => quad.predicate == quad.object,
                    }))
                }
                if graph_name.is_var() {
                    if graph_name == subject {
                        iter = Box::new(iter.filter(|quad| match quad {
                            Err(_) => true,
                            Ok(quad) => quad.graph_name == quad.subject,
                        }))
                    }
                    if graph_name == predicate {
                        iter = Box::new(iter.filter(|quad| match quad {
                            Err(_) => true,
                            Ok(quad) => quad.graph_name == quad.predicate,
                        }))
                    }
                    if graph_name == object {
                        iter = Box::new(iter.filter(|quad| match quad {
                            Err(_) => true,
                            Ok(quad) => quad.graph_name == quad.object,
                        }))
                    }
                }
                let iter: EncodedTuplesIterator<'_> = Box::new(iter.map(move |quad| {
                    let quad = quad?;
                    let mut new_tuple = tuple.clone();
                    put_pattern_value(subject, quad.subject, &mut new_tuple);
                    put_pattern_value(predicate, quad.predicate, &mut new_tuple);
                    put_pattern_value(object, quad.object, &mut new_tuple);
                    put_pattern_value(graph_name, quad.graph_name, &mut new_tuple);
                    Ok(new_tuple)
                }));
                iter
            })),
            PlanNode::PathPatternJoin {
                child,
                subject,
                path,
                object,
                graph_name,
            } => Box::new(self.eval_plan(&*child, from).flat_map_ok(move |tuple| {
                let input_subject = get_pattern_value(subject, &tuple);
                let input_object = get_pattern_value(object, &tuple);
                let input_graph_name =
                    if let Some(graph_name) = get_pattern_value(graph_name, &tuple) {
                        graph_name
                    } else {
                        let result: EncodedTuplesIterator<'_> = Box::new(once(Err(Error::msg(
                            "Unknown graph name is not allowed when evaluating property path",
                        ))));
                        return result;
                    };
                match (input_subject, input_object) {
                    (Some(input_subject), Some(input_object)) => Box::new(
                        self.eval_path_from(path, input_subject, input_graph_name)
                            .filter_map(move |o| match o {
                                Ok(o) => {
                                    if o == input_object {
                                        Some(Ok(tuple.clone()))
                                    } else {
                                        None
                                    }
                                }
                                Err(error) => Some(Err(error)),
                            }),
                    ),
                    (Some(input_subject), None) => Box::new(
                        self.eval_path_from(path, input_subject, input_graph_name)
                            .map(move |o| {
                                let mut new_tuple = tuple.clone();
                                put_pattern_value(object, o?, &mut new_tuple);
                                Ok(new_tuple)
                            }),
                    ),
                    (None, Some(input_object)) => Box::new(
                        self.eval_path_to(path, input_object, input_graph_name)
                            .map(move |s| {
                                let mut new_tuple = tuple.clone();
                                put_pattern_value(subject, s?, &mut new_tuple);
                                Ok(new_tuple)
                            }),
                    ),
                    (None, None) => {
                        Box::new(self.eval_open_path(path, input_graph_name).map(move |so| {
                            let mut new_tuple = tuple.clone();
                            so.map(move |(s, o)| {
                                put_pattern_value(subject, s, &mut new_tuple);
                                put_pattern_value(object, o, &mut new_tuple);
                                new_tuple
                            })
                        }))
                    }
                }
            })),
            PlanNode::Join { left, right } => {
                //TODO: very dumb implementation
                let mut errors = Vec::default();
                let left_values = self
                    .eval_plan(&*left, from.clone())
                    .filter_map(|result| match result {
                        Ok(result) => Some(result),
                        Err(error) => {
                            errors.push(Err(error));
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                Box::new(JoinIterator {
                    left: left_values,
                    right_iter: self.eval_plan(&*right, from),
                    buffered_results: errors,
                })
            }
            PlanNode::AntiJoin { left, right } => {
                //TODO: dumb implementation
                let right: Vec<_> = self
                    .eval_plan(&*right, from.clone())
                    .filter_map(|result| result.ok())
                    .collect();
                Box::new(AntiJoinIterator {
                    left_iter: self.eval_plan(&*left, from),
                    right,
                })
            }
            PlanNode::LeftJoin {
                left,
                right,
                possible_problem_vars,
            } => {
                if possible_problem_vars.is_empty() {
                    Box::new(LeftJoinIterator {
                        eval: self,
                        right_plan: &*right,
                        left_iter: self.eval_plan(&*left, from),
                        current_right: Box::new(empty()),
                    })
                } else {
                    Box::new(BadLeftJoinIterator {
                        eval: self,
                        right_plan: &*right,
                        left_iter: self.eval_plan(&*left, from),
                        current_left: None,
                        current_right: Box::new(empty()),
                        problem_vars: possible_problem_vars.as_slice(),
                    })
                }
            }
            PlanNode::Filter { child, expression } => {
                let eval = self;
                Box::new(self.eval_plan(&*child, from).filter(move |tuple| {
                    match tuple {
                        Ok(tuple) => eval
                            .eval_expression(expression, tuple)
                            .and_then(|term| eval.to_bool(term))
                            .unwrap_or(false),
                        Err(_) => true,
                    }
                }))
            }
            PlanNode::Union { children } => Box::new(UnionIterator {
                eval: self,
                plans: children,
                input: from,
                current_iterator: Box::new(empty()),
                current_plan: 0,
            }),
            PlanNode::Extend {
                child,
                position,
                expression,
            } => {
                let eval = self;
                Box::new(self.eval_plan(&*child, from).map(move |tuple| {
                    let mut tuple = tuple?;
                    if let Some(value) = eval.eval_expression(expression, &tuple) {
                        tuple.set(*position, value)
                    }
                    Ok(tuple)
                }))
            }
            PlanNode::Sort { child, by } => {
                let mut errors = Vec::default();
                let mut values = self
                    .eval_plan(&*child, from)
                    .filter_map(|result| match result {
                        Ok(result) => Some(result),
                        Err(error) => {
                            errors.push(Err(error));
                            None
                        }
                    })
                    .collect::<Vec<_>>();
                values.sort_unstable_by(|a, b| {
                    for comp in by {
                        match comp {
                            Comparator::Asc(expression) => {
                                match self.cmp_according_to_expression(a, b, expression) {
                                    Ordering::Greater => return Ordering::Greater,
                                    Ordering::Less => return Ordering::Less,
                                    Ordering::Equal => (),
                                }
                            }
                            Comparator::Desc(expression) => {
                                match self.cmp_according_to_expression(a, b, expression) {
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
            }
            PlanNode::HashDeduplicate { child } => {
                Box::new(hash_deduplicate(self.eval_plan(&*child, from)))
            }
            PlanNode::Skip { child, count } => Box::new(self.eval_plan(&*child, from).skip(*count)),
            PlanNode::Limit { child, count } => {
                Box::new(self.eval_plan(&*child, from).take(*count))
            }
            PlanNode::Project { child, mapping } => {
                //TODO: use from somewhere?
                Box::new(
                    self.eval_plan(&*child, EncodedTuple::with_capacity(mapping.len()))
                        .map(move |tuple| {
                            let tuple = tuple?;
                            let mut output_tuple = EncodedTuple::with_capacity(from.capacity());
                            for (input_key, output_key) in mapping.iter() {
                                if let Some(value) = tuple.get(*input_key) {
                                    output_tuple.set(*output_key, value)
                                }
                            }
                            Ok(output_tuple)
                        }),
                )
            }
            PlanNode::Aggregate {
                child,
                key_mapping,
                aggregates,
            } => {
                let tuple_size = from.capacity(); //TODO: not nice
                let mut errors = Vec::default();
                let mut accumulators_for_group =
                    HashMap::<Vec<Option<EncodedTerm>>, Vec<Box<dyn Accumulator>>>::default();
                self.eval_plan(child, from)
                    .filter_map(|result| match result {
                        Ok(result) => Some(result),
                        Err(error) => {
                            errors.push(error);
                            None
                        }
                    })
                    .for_each(|tuple| {
                        //TODO avoid copy for key?
                        let key = key_mapping.iter().map(|(v, _)| tuple.get(*v)).collect();

                        let key_accumulators =
                            accumulators_for_group.entry(key).or_insert_with(|| {
                                aggregates
                                    .iter()
                                    .map(|(aggregate, _)| {
                                        self.accumulator_for_aggregate(
                                            &aggregate.function,
                                            aggregate.distinct,
                                        )
                                    })
                                    .collect::<Vec<_>>()
                            });
                        for (i, accumulator) in key_accumulators.iter_mut().enumerate() {
                            let (aggregate, _) = &aggregates[i];
                            accumulator.add(
                                aggregate
                                    .parameter
                                    .as_ref()
                                    .and_then(|parameter| self.eval_expression(parameter, &tuple)),
                            );
                        }
                    });
                if accumulators_for_group.is_empty() {
                    // There is always at least one group
                    accumulators_for_group.insert(vec![None; key_mapping.len()], Vec::default());
                }
                Box::new(
                    errors
                        .into_iter()
                        .map(Err)
                        .chain(accumulators_for_group.into_iter().map(
                            move |(key, accumulators)| {
                                let mut result = EncodedTuple::with_capacity(tuple_size);
                                for (from_position, to_position) in key_mapping.iter() {
                                    if let Some(value) = key[*from_position] {
                                        result.set(*to_position, value);
                                    }
                                }
                                for (i, accumulator) in accumulators.into_iter().enumerate() {
                                    if let Some(value) = accumulator.state() {
                                        result.set(aggregates[i].1, value);
                                    }
                                }
                                Ok(result)
                            },
                        )),
                )
            }
        }
    }

    fn evaluate_service<'b>(
        &'b self,
        service_name: &PatternValue,
        graph_pattern: &'b GraphPattern,
        variables: &'b [Variable],
        from: &EncodedTuple,
    ) -> Result<EncodedTuplesIterator<'b>> {
        let service_name = self.dataset.decode_named_node(
            get_pattern_value(service_name, from)
                .ok_or_else(|| Error::msg("The SERVICE name is not bound"))?,
        )?;
        Ok(self.encode_bindings(
            variables,
            self.service_handler.handle(&service_name, graph_pattern)?,
        ))
    }

    fn accumulator_for_aggregate<'b>(
        &'b self,
        function: &'b PlanAggregationFunction,
        distinct: bool,
    ) -> Box<dyn Accumulator + 'b> {
        match function {
            PlanAggregationFunction::Count => {
                if distinct {
                    Box::new(DistinctAccumulator::new(CountAccumulator::default()))
                } else {
                    Box::new(CountAccumulator::default())
                }
            }
            PlanAggregationFunction::Sum => {
                if distinct {
                    Box::new(DistinctAccumulator::new(SumAccumulator::default()))
                } else {
                    Box::new(SumAccumulator::default())
                }
            }
            PlanAggregationFunction::Min => Box::new(MinAccumulator::new(self)), // DISTINCT does not make sense with min
            PlanAggregationFunction::Max => Box::new(MaxAccumulator::new(self)), // DISTINCT does not make sense with max
            PlanAggregationFunction::Avg => {
                if distinct {
                    Box::new(DistinctAccumulator::new(AvgAccumulator::default()))
                } else {
                    Box::new(AvgAccumulator::default())
                }
            }
            PlanAggregationFunction::Sample => Box::new(SampleAccumulator::default()), // DISTINCT does not make sense with sample
            PlanAggregationFunction::GroupConcat { separator } => {
                if distinct {
                    Box::new(DistinctAccumulator::new(GroupConcatAccumulator::new(
                        self, separator,
                    )))
                } else {
                    Box::new(GroupConcatAccumulator::new(self, separator))
                }
            }
        }
    }

    fn eval_path_from<'b>(
        &'b self,
        path: &'b PlanPropertyPath,
        start: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Box<dyn Iterator<Item = Result<EncodedTerm>> + 'b>
    where
        'a: 'b,
    {
        match path {
            PlanPropertyPath::PredicatePath(p) => Box::new(
                self.dataset
                    .quads_for_pattern(Some(start), Some(*p), None, Some(graph_name))
                    .map(|t| Ok(t?.object)),
            ),
            PlanPropertyPath::InversePath(p) => self.eval_path_to(p, start, graph_name),
            PlanPropertyPath::SequencePath(a, b) => Box::new(
                self.eval_path_from(a, start, graph_name)
                    .flat_map_ok(move |middle| self.eval_path_from(b, middle, graph_name)),
            ),
            PlanPropertyPath::AlternativePath(a, b) => Box::new(
                self.eval_path_from(a, start, graph_name)
                    .chain(self.eval_path_from(b, start, graph_name)),
            ),
            PlanPropertyPath::ZeroOrMorePath(p) => {
                Box::new(transitive_closure(Some(Ok(start)), move |e| {
                    self.eval_path_from(p, e, graph_name)
                }))
            }
            PlanPropertyPath::OneOrMorePath(p) => Box::new(transitive_closure(
                self.eval_path_from(p, start, graph_name),
                move |e| self.eval_path_from(p, e, graph_name),
            )),
            PlanPropertyPath::ZeroOrOnePath(p) => Box::new(hash_deduplicate(
                once(Ok(start)).chain(self.eval_path_from(p, start, graph_name)),
            )),
            PlanPropertyPath::NegatedPropertySet(ps) => Box::new(
                self.dataset
                    .quads_for_pattern(Some(start), None, None, Some(graph_name))
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
            ),
        }
    }

    fn eval_path_to<'b>(
        &'b self,
        path: &'b PlanPropertyPath,
        end: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Box<dyn Iterator<Item = Result<EncodedTerm>> + 'b>
    where
        'a: 'b,
    {
        match path {
            PlanPropertyPath::PredicatePath(p) => Box::new(
                self.dataset
                    .quads_for_pattern(None, Some(*p), Some(end), Some(graph_name))
                    .map(|t| Ok(t?.subject)),
            ),
            PlanPropertyPath::InversePath(p) => self.eval_path_from(p, end, graph_name),
            PlanPropertyPath::SequencePath(a, b) => Box::new(
                self.eval_path_to(b, end, graph_name)
                    .flat_map_ok(move |middle| self.eval_path_to(a, middle, graph_name)),
            ),
            PlanPropertyPath::AlternativePath(a, b) => Box::new(
                self.eval_path_to(a, end, graph_name)
                    .chain(self.eval_path_to(b, end, graph_name)),
            ),
            PlanPropertyPath::ZeroOrMorePath(p) => {
                Box::new(transitive_closure(Some(Ok(end)), move |e| {
                    self.eval_path_to(p, e, graph_name)
                }))
            }
            PlanPropertyPath::OneOrMorePath(p) => Box::new(transitive_closure(
                self.eval_path_to(p, end, graph_name),
                move |e| self.eval_path_to(p, e, graph_name),
            )),
            PlanPropertyPath::ZeroOrOnePath(p) => Box::new(hash_deduplicate(
                once(Ok(end)).chain(self.eval_path_to(p, end, graph_name)),
            )),
            PlanPropertyPath::NegatedPropertySet(ps) => Box::new(
                self.dataset
                    .quads_for_pattern(None, None, Some(end), Some(graph_name))
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
            ),
        }
    }

    fn eval_open_path<'b>(
        &'b self,
        path: &'b PlanPropertyPath,
        graph_name: EncodedTerm,
    ) -> Box<dyn Iterator<Item = Result<(EncodedTerm, EncodedTerm)>> + 'b>
    where
        'a: 'b,
    {
        match path {
            PlanPropertyPath::PredicatePath(p) => Box::new(
                self.dataset
                    .quads_for_pattern(None, Some(*p), None, Some(graph_name))
                    .map(|t| t.map(|t| (t.subject, t.object))),
            ),
            PlanPropertyPath::InversePath(p) => Box::new(
                self.eval_open_path(p, graph_name)
                    .map(|t| t.map(|(s, o)| (o, s))),
            ),
            PlanPropertyPath::SequencePath(a, b) => Box::new(
                self.eval_open_path(a, graph_name)
                    .flat_map_ok(move |(start, middle)| {
                        self.eval_path_from(b, middle, graph_name)
                            .map(move |end| Ok((start, end?)))
                    }),
            ),
            PlanPropertyPath::AlternativePath(a, b) => Box::new(
                self.eval_open_path(a, graph_name)
                    .chain(self.eval_open_path(b, graph_name)),
            ),
            PlanPropertyPath::ZeroOrMorePath(p) => Box::new(transitive_closure(
                self.get_subject_or_object_identity_pairs(graph_name), //TODO: avoid to inject everything
                move |(start, middle)| {
                    self.eval_path_from(p, middle, graph_name)
                        .map(move |end| Ok((start, end?)))
                },
            )),
            PlanPropertyPath::OneOrMorePath(p) => Box::new(transitive_closure(
                self.eval_open_path(p, graph_name),
                move |(start, middle)| {
                    self.eval_path_from(p, middle, graph_name)
                        .map(move |end| Ok((start, end?)))
                },
            )),
            PlanPropertyPath::ZeroOrOnePath(p) => Box::new(hash_deduplicate(
                self.get_subject_or_object_identity_pairs(graph_name)
                    .chain(self.eval_open_path(p, graph_name)),
            )),
            PlanPropertyPath::NegatedPropertySet(ps) => Box::new(
                self.dataset
                    .quads_for_pattern(None, None, None, Some(graph_name))
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
            ),
        }
    }

    fn get_subject_or_object_identity_pairs<'b>(
        &'b self,
        graph_name: EncodedTerm,
    ) -> impl Iterator<Item = Result<(EncodedTerm, EncodedTerm)>> + 'b {
        self.dataset
            .quads_for_pattern(None, None, None, Some(graph_name))
            .flat_map_ok(|t| once(Ok(t.subject)).chain(once(Ok(t.object))))
            .map(|e| e.map(|e| (e, e)))
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    fn eval_expression<'b>(
        &'b self,
        expression: &PlanExpression,
        tuple: &EncodedTuple,
    ) -> Option<EncodedTerm> {
        match expression {
            PlanExpression::Constant(t) => Some(*t),
            PlanExpression::Variable(v) => tuple.get(*v),
            PlanExpression::Exists(node) => {
                Some(self.eval_plan(node, tuple.clone()).next().is_some().into())
            }
            PlanExpression::Or(a, b) => {
                match self.eval_expression(a, tuple).and_then(|v| self.to_bool(v)) {
                    Some(true) => Some(true.into()),
                    Some(false) => self.eval_expression(b, tuple),
                    None => {
                        if Some(true)
                            == self.eval_expression(b, tuple).and_then(|v| self.to_bool(v))
                        {
                            Some(true.into())
                        } else {
                            None
                        }
                    }
                }
            }
            PlanExpression::And(a, b) => match self
                .eval_expression(a, tuple)
                .and_then(|v| self.to_bool(v))
            {
                Some(true) => self.eval_expression(b, tuple),
                Some(false) => Some(false.into()),
                None => {
                    if Some(false) == self.eval_expression(b, tuple).and_then(|v| self.to_bool(v)) {
                        Some(false.into())
                    } else {
                        None
                    }
                }
            },
            PlanExpression::Equal(a, b) => {
                let a = self.eval_expression(a, tuple)?;
                let b = self.eval_expression(b, tuple)?;
                self.equals(a, b).map(|v| v.into())
            }
            PlanExpression::NotEqual(a, b) => {
                let a = self.eval_expression(a, tuple)?;
                let b = self.eval_expression(b, tuple)?;
                self.equals(a, b).map(|v| (!v).into())
            }
            PlanExpression::Greater(a, b) => Some(
                (self.partial_cmp_literals(
                    self.eval_expression(a, tuple)?,
                    self.eval_expression(b, tuple)?,
                )? == Ordering::Greater)
                    .into(),
            ),
            PlanExpression::GreaterOrEq(a, b) => Some(
                match self.partial_cmp_literals(
                    self.eval_expression(a, tuple)?,
                    self.eval_expression(b, tuple)?,
                )? {
                    Ordering::Greater | Ordering::Equal => true,
                    Ordering::Less => false,
                }
                .into(),
            ),
            PlanExpression::Lower(a, b) => Some(
                (self.partial_cmp_literals(
                    self.eval_expression(a, tuple)?,
                    self.eval_expression(b, tuple)?,
                )? == Ordering::Less)
                    .into(),
            ),
            PlanExpression::LowerOrEq(a, b) => Some(
                match self.partial_cmp_literals(
                    self.eval_expression(a, tuple)?,
                    self.eval_expression(b, tuple)?,
                )? {
                    Ordering::Less | Ordering::Equal => true,
                    Ordering::Greater => false,
                }
                .into(),
            ),
            PlanExpression::In(e, l) => {
                let needed = self.eval_expression(e, tuple)?;
                let mut error = false;
                for possible in l {
                    if let Some(possible) = self.eval_expression(possible, tuple) {
                        if Some(true) == self.equals(needed, possible) {
                            return Some(true.into());
                        }
                    } else {
                        error = true;
                    }
                }
                if error {
                    None
                } else {
                    Some(false.into())
                }
            }
            PlanExpression::Add(a, b) => match self.parse_numeric_operands(a, b, tuple)? {
                NumericBinaryOperands::Float(v1, v2) => Some((v1 + v2).into()),
                NumericBinaryOperands::Double(v1, v2) => Some((v1 + v2).into()),
                NumericBinaryOperands::Integer(v1, v2) => Some(v1.checked_add(v2)?.into()),
                NumericBinaryOperands::Decimal(v1, v2) => Some(v1.checked_add(v2)?.into()),
                NumericBinaryOperands::Duration(v1, v2) => Some(v1.checked_add(v2)?.into()),
                NumericBinaryOperands::DateTimeDuration(v1, v2) => {
                    Some(v1.checked_add_duration(v2)?.into())
                }
                NumericBinaryOperands::DateDuration(v1, v2) => {
                    Some(v1.checked_add_duration(v2)?.into())
                }
                NumericBinaryOperands::TimeDuration(v1, v2) => {
                    Some(v1.checked_add_duration(v2)?.into())
                }
                _ => None,
            },
            PlanExpression::Sub(a, b) => Some(match self.parse_numeric_operands(a, b, tuple)? {
                NumericBinaryOperands::Float(v1, v2) => (v1 - v2).into(),
                NumericBinaryOperands::Double(v1, v2) => (v1 - v2).into(),
                NumericBinaryOperands::Integer(v1, v2) => v1.checked_sub(v2)?.into(),
                NumericBinaryOperands::Decimal(v1, v2) => v1.checked_sub(v2)?.into(),
                NumericBinaryOperands::Duration(v1, v2) => v1.checked_sub(v2)?.into(),
                NumericBinaryOperands::DateTime(v1, v2) => v1.checked_sub(v2)?.into(),
                NumericBinaryOperands::Date(v1, v2) => v1.checked_sub(v2)?.into(),
                NumericBinaryOperands::Time(v1, v2) => v1.checked_sub(v2)?.into(),
                NumericBinaryOperands::DateTimeDuration(v1, v2) => {
                    v1.checked_sub_duration(v2)?.into()
                }
                NumericBinaryOperands::DateDuration(v1, v2) => v1.checked_sub_duration(v2)?.into(),
                NumericBinaryOperands::TimeDuration(v1, v2) => v1.checked_sub_duration(v2)?.into(),
            }),
            PlanExpression::Mul(a, b) => match self.parse_numeric_operands(a, b, tuple)? {
                NumericBinaryOperands::Float(v1, v2) => Some((v1 * v2).into()),
                NumericBinaryOperands::Double(v1, v2) => Some((v1 * v2).into()),
                NumericBinaryOperands::Integer(v1, v2) => Some(v1.checked_mul(v2)?.into()),
                NumericBinaryOperands::Decimal(v1, v2) => Some(v1.checked_mul(v2)?.into()),
                _ => None,
            },
            PlanExpression::Div(a, b) => match self.parse_numeric_operands(a, b, tuple)? {
                NumericBinaryOperands::Float(v1, v2) => Some((v1 / v2).into()),
                NumericBinaryOperands::Double(v1, v2) => Some((v1 / v2).into()),
                NumericBinaryOperands::Integer(v1, v2) => {
                    Some(Decimal::from(v1).checked_div(v2)?.into())
                }
                NumericBinaryOperands::Decimal(v1, v2) => Some(v1.checked_div(v2)?.into()),
                _ => None,
            },
            PlanExpression::UnaryPlus(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::FloatLiteral(value) => Some(value.into()),
                EncodedTerm::DoubleLiteral(value) => Some(value.into()),
                EncodedTerm::IntegerLiteral(value) => Some(value.into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.into()),
                EncodedTerm::DurationLiteral(value) => Some(value.into()),
                _ => None,
            },
            PlanExpression::UnaryMinus(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::FloatLiteral(value) => Some((-value).into()),
                EncodedTerm::DoubleLiteral(value) => Some((-value).into()),
                EncodedTerm::IntegerLiteral(value) => Some((-value).into()),
                EncodedTerm::DecimalLiteral(value) => Some((-value).into()),
                EncodedTerm::DurationLiteral(value) => Some((-value).into()),
                _ => None,
            },
            PlanExpression::UnaryNot(e) => self
                .to_bool(self.eval_expression(e, tuple)?)
                .map(|v| (!v).into()),
            PlanExpression::Str(e) => Some(EncodedTerm::StringLiteral {
                value_id: self.to_string_id(self.eval_expression(e, tuple)?)?,
            }),
            PlanExpression::Lang(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::LangStringLiteral { language_id, .. } => {
                    Some(EncodedTerm::StringLiteral {
                        value_id: language_id,
                    })
                }
                e if e.is_literal() => Some(ENCODED_EMPTY_STRING_LITERAL),
                _ => None,
            },
            PlanExpression::LangMatches(language_tag, language_range) => {
                let mut language_tag =
                    self.to_simple_string(self.eval_expression(language_tag, tuple)?)?;
                language_tag.make_ascii_lowercase();
                let mut language_range =
                    self.to_simple_string(self.eval_expression(language_range, tuple)?)?;
                language_range.make_ascii_lowercase();
                Some(
                    if &*language_range == "*" {
                        !language_tag.is_empty()
                    } else {
                        !ZipLongest::new(language_range.split('-'), language_tag.split('-')).any(
                            |parts| match parts {
                                (Some(range_subtag), Some(language_subtag)) => {
                                    range_subtag != language_subtag
                                }
                                (Some(_), None) => true,
                                (None, _) => false,
                            },
                        )
                    }
                    .into(),
                )
            }
            PlanExpression::Datatype(e) => self.eval_expression(e, tuple)?.datatype(),
            PlanExpression::Bound(v) => Some(tuple.contains(*v).into()),
            PlanExpression::IRI(e) => {
                let iri_id = match self.eval_expression(e, tuple)? {
                    EncodedTerm::NamedNode { iri_id } => Some(iri_id),
                    EncodedTerm::StringLiteral { value_id } => Some(value_id),
                    _ => None,
                }?;
                let iri = self.dataset.get_str(iri_id).ok()??;
                if let Some(base_iri) = &self.base_iri {
                    self.build_named_node(&base_iri.resolve(&iri).ok()?.into_inner())
                } else {
                    Iri::parse(iri).ok()?;
                    Some(EncodedTerm::NamedNode { iri_id })
                }
            }
            PlanExpression::BNode(id) => match id {
                Some(id) => {
                    if let EncodedTerm::StringLiteral { value_id } =
                        self.eval_expression(id, tuple)?
                    {
                        Some(EncodedTerm::BlankNode {
                            id: *self
                                .bnodes_map
                                .lock()
                                .ok()?
                                .entry(value_id)
                                .or_insert_with(random::<u128>),
                        })
                    } else {
                        None
                    }
                }
                None => Some(EncodedTerm::BlankNode {
                    id: random::<u128>(),
                }),
            },
            PlanExpression::Rand => Some(random::<f64>().into()),
            PlanExpression::Abs(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::IntegerLiteral(value) => Some(value.checked_abs()?.into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.abs().into()),
                EncodedTerm::FloatLiteral(value) => Some(value.abs().into()),
                EncodedTerm::DoubleLiteral(value) => Some(value.abs().into()),
                _ => None,
            },
            PlanExpression::Ceil(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::IntegerLiteral(value) => Some(value.into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.ceil().into()),
                EncodedTerm::FloatLiteral(value) => Some(value.ceil().into()),
                EncodedTerm::DoubleLiteral(value) => Some(value.ceil().into()),
                _ => None,
            },
            PlanExpression::Floor(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::IntegerLiteral(value) => Some(value.into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.floor().into()),
                EncodedTerm::FloatLiteral(value) => Some(value.floor().into()),
                EncodedTerm::DoubleLiteral(value) => Some(value.floor().into()),
                _ => None,
            },
            PlanExpression::Round(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::IntegerLiteral(value) => Some(value.into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.round().into()),
                EncodedTerm::FloatLiteral(value) => Some(value.round().into()),
                EncodedTerm::DoubleLiteral(value) => Some(value.round().into()),
                _ => None,
            },
            PlanExpression::Concat(l) => {
                let mut result = String::default();
                let mut language = None;
                for e in l {
                    let (value, e_language) =
                        self.to_string_and_language(self.eval_expression(e, tuple)?)?;
                    if let Some(lang) = language {
                        if lang != e_language {
                            language = Some(None)
                        }
                    } else {
                        language = Some(e_language)
                    }
                    result += &value
                }
                self.build_plain_literal(&result, language.and_then(|v| v))
            }
            PlanExpression::SubStr(source, starting_loc, length) => {
                let (source, language) =
                    self.to_string_and_language(self.eval_expression(source, tuple)?)?;

                let starting_location: usize = if let EncodedTerm::IntegerLiteral(v) =
                    self.eval_expression(starting_loc, tuple)?
                {
                    v.try_into().ok()?
                } else {
                    return None;
                };
                let length: Option<usize> = if let Some(length) = length {
                    if let EncodedTerm::IntegerLiteral(v) = self.eval_expression(length, tuple)? {
                        Some(v.try_into().ok()?)
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
                let result = if let Some((start_position, _)) = start_iter.peek().cloned() {
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
                self.build_plain_literal(result, language)
            }
            PlanExpression::StrLen(arg) => Some(
                (self
                    .to_string(self.eval_expression(arg, tuple)?)?
                    .chars()
                    .count() as i64)
                    .into(),
            ),
            PlanExpression::Replace(arg, pattern, replacement, flags) => {
                let regex = self.compile_pattern(
                    self.eval_expression(pattern, tuple)?,
                    if let Some(flags) = flags {
                        Some(self.eval_expression(flags, tuple)?)
                    } else {
                        None
                    },
                )?;
                let (text, language) =
                    self.to_string_and_language(self.eval_expression(arg, tuple)?)?;
                let replacement =
                    self.to_simple_string(self.eval_expression(replacement, tuple)?)?;
                self.build_plain_literal(&regex.replace_all(&text, replacement.as_str()), language)
            }
            PlanExpression::UCase(e) => {
                let (value, language) =
                    self.to_string_and_language(self.eval_expression(e, tuple)?)?;
                self.build_plain_literal(&value.to_uppercase(), language)
            }
            PlanExpression::LCase(e) => {
                let (value, language) =
                    self.to_string_and_language(self.eval_expression(e, tuple)?)?;
                self.build_plain_literal(&value.to_lowercase(), language)
            }
            PlanExpression::StrStarts(arg1, arg2) => {
                let (arg1, arg2, _) = self.to_argument_compatible_strings(
                    self.eval_expression(arg1, tuple)?,
                    self.eval_expression(arg2, tuple)?,
                )?;
                Some((&arg1).starts_with(arg2.as_str()).into())
            }
            PlanExpression::EncodeForURI(ltrl) => {
                let ltlr = self.to_string(self.eval_expression(ltrl, tuple)?)?;
                let mut result = Vec::with_capacity(ltlr.len());
                for c in ltlr.bytes() {
                    match c {
                        b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                            result.push(c)
                        }
                        _ => {
                            result.push(b'%');
                            let hight = c / 16;
                            let low = c % 16;
                            result.push(if hight < 10 {
                                b'0' + hight
                            } else {
                                b'A' + (hight - 10)
                            });
                            result.push(if low < 10 {
                                b'0' + low
                            } else {
                                b'A' + (low - 10)
                            });
                        }
                    }
                }
                self.build_string_literal(str::from_utf8(&result).ok()?)
            }
            PlanExpression::StrEnds(arg1, arg2) => {
                let (arg1, arg2, _) = self.to_argument_compatible_strings(
                    self.eval_expression(arg1, tuple)?,
                    self.eval_expression(arg2, tuple)?,
                )?;
                Some((&arg1).ends_with(arg2.as_str()).into())
            }
            PlanExpression::Contains(arg1, arg2) => {
                let (arg1, arg2, _) = self.to_argument_compatible_strings(
                    self.eval_expression(arg1, tuple)?,
                    self.eval_expression(arg2, tuple)?,
                )?;
                Some((&arg1).contains(arg2.as_str()).into())
            }
            PlanExpression::StrBefore(arg1, arg2) => {
                let (arg1, arg2, language) = self.to_argument_compatible_strings(
                    self.eval_expression(arg1, tuple)?,
                    self.eval_expression(arg2, tuple)?,
                )?;
                if let Some(position) = (&arg1).find(arg2.as_str()) {
                    self.build_plain_literal(&arg1[..position], language)
                } else {
                    Some(ENCODED_EMPTY_STRING_LITERAL)
                }
            }
            PlanExpression::StrAfter(arg1, arg2) => {
                let (arg1, arg2, language) = self.to_argument_compatible_strings(
                    self.eval_expression(arg1, tuple)?,
                    self.eval_expression(arg2, tuple)?,
                )?;
                if let Some(position) = (&arg1).find(arg2.as_str()) {
                    self.build_plain_literal(&arg1[position + arg2.len()..], language)
                } else {
                    Some(ENCODED_EMPTY_STRING_LITERAL)
                }
            }
            PlanExpression::Year(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::DateLiteral(date) => Some(date.year().into()),
                EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.year().into()),
                _ => None,
            },
            PlanExpression::Month(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::DateLiteral(date) => Some(date.year().into()),
                EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.month().into()),
                _ => None,
            },
            PlanExpression::Day(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::DateLiteral(date) => Some(date.year().into()),
                EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.day().into()),
                _ => None,
            },
            PlanExpression::Hours(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::TimeLiteral(time) => Some(time.hour().into()),
                EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.hour().into()),
                _ => None,
            },
            PlanExpression::Minutes(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::TimeLiteral(time) => Some(time.minute().into()),
                EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.minute().into()),
                _ => None,
            },
            PlanExpression::Seconds(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::TimeLiteral(time) => Some(time.second().into()),
                EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.second().into()),
                _ => None,
            },
            PlanExpression::Timezone(e) => Some(
                match self.eval_expression(e, tuple)? {
                    EncodedTerm::DateLiteral(date) => date.timezone(),
                    EncodedTerm::TimeLiteral(time) => time.timezone(),
                    EncodedTerm::DateTimeLiteral(date_time) => date_time.timezone(),
                    _ => None,
                }?
                .into(),
            ),
            PlanExpression::Tz(e) => {
                let timezone_offset = match self.eval_expression(e, tuple)? {
                    EncodedTerm::DateLiteral(date) => date.timezone_offset(),
                    EncodedTerm::TimeLiteral(time) => time.timezone_offset(),
                    EncodedTerm::DateTimeLiteral(date_time) => date_time.timezone_offset(),
                    _ => return None,
                };
                Some(match timezone_offset {
                    Some(timezone_offset) => {
                        self.build_string_literal(&timezone_offset.to_string())?
                    }
                    None => ENCODED_EMPTY_STRING_LITERAL,
                })
            }
            PlanExpression::Now => Some(self.now.into()),
            PlanExpression::UUID => {
                let mut buffer = String::with_capacity(44);
                buffer.push_str("urn:uuid:");
                generate_uuid(&mut buffer);
                self.build_named_node(&buffer)
            }
            PlanExpression::StrUUID => {
                let mut buffer = String::with_capacity(36);
                generate_uuid(&mut buffer);
                self.build_string_literal(&buffer)
            }
            PlanExpression::MD5(arg) => self.hash::<Md5>(arg, tuple),
            PlanExpression::SHA1(arg) => self.hash::<Sha1>(arg, tuple),
            PlanExpression::SHA256(arg) => self.hash::<Sha256>(arg, tuple),
            PlanExpression::SHA384(arg) => self.hash::<Sha384>(arg, tuple),
            PlanExpression::SHA512(arg) => self.hash::<Sha512>(arg, tuple),
            PlanExpression::Coalesce(l) => {
                for e in l {
                    if let Some(result) = self.eval_expression(e, tuple) {
                        return Some(result);
                    }
                }
                None
            }
            PlanExpression::If(a, b, c) => {
                if self.to_bool(self.eval_expression(a, tuple)?)? {
                    self.eval_expression(b, tuple)
                } else {
                    self.eval_expression(c, tuple)
                }
            }
            PlanExpression::StrLang(lexical_form, lang_tag) => {
                Some(EncodedTerm::LangStringLiteral {
                    value_id: self
                        .to_simple_string_id(self.eval_expression(lexical_form, tuple)?)?,
                    language_id: self.build_language_id(self.eval_expression(lang_tag, tuple)?)?,
                })
            }
            PlanExpression::StrDT(lexical_form, datatype) => {
                let value = self.to_simple_string(self.eval_expression(lexical_form, tuple)?)?;
                let datatype = if let EncodedTerm::NamedNode { iri_id } =
                    self.eval_expression(datatype, tuple)?
                {
                    self.dataset.get_str(iri_id).ok()?
                } else {
                    None
                }?;
                self.dataset
                    .encoder()
                    .encode_rio_literal(rio::Literal::Typed {
                        value: &value,
                        datatype: rio::NamedNode { iri: &datatype },
                    })
                    .ok()
            }
            PlanExpression::SameTerm(a, b) => {
                Some((self.eval_expression(a, tuple)? == self.eval_expression(b, tuple)?).into())
            }
            PlanExpression::IsIRI(e) => {
                Some(self.eval_expression(e, tuple)?.is_named_node().into())
            }
            PlanExpression::IsBlank(e) => {
                Some(self.eval_expression(e, tuple)?.is_blank_node().into())
            }
            PlanExpression::IsLiteral(e) => {
                Some(self.eval_expression(e, tuple)?.is_literal().into())
            }
            PlanExpression::IsNumeric(e) => Some(
                match self.eval_expression(e, tuple)? {
                    EncodedTerm::FloatLiteral(_)
                    | EncodedTerm::DoubleLiteral(_)
                    | EncodedTerm::IntegerLiteral(_)
                    | EncodedTerm::DecimalLiteral(_) => true,
                    _ => false,
                }
                .into(),
            ),
            PlanExpression::Regex(text, pattern, flags) => {
                let regex = self.compile_pattern(
                    self.eval_expression(pattern, tuple)?,
                    if let Some(flags) = flags {
                        Some(self.eval_expression(flags, tuple)?)
                    } else {
                        None
                    },
                )?;
                let text = self.to_string(self.eval_expression(text, tuple)?)?;
                Some(regex.is_match(&text).into())
            }
            PlanExpression::BooleanCast(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::BooleanLiteral(value) => Some(value.into()),
                EncodedTerm::StringLiteral { value_id } => {
                    parse_boolean_str(&*self.dataset.get_str(value_id).ok()??)
                }
                _ => None,
            },
            PlanExpression::DoubleCast(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::FloatLiteral(value) => Some(f64::from(value).into()),
                EncodedTerm::DoubleLiteral(value) => Some(value.into()),
                EncodedTerm::IntegerLiteral(value) => Some((value as f64).into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.to_f64().into()),
                EncodedTerm::BooleanLiteral(value) => {
                    Some(if value { 1_f64 } else { 0_f64 }.into())
                }
                EncodedTerm::StringLiteral { value_id } => {
                    parse_double_str(&*self.dataset.get_str(value_id).ok()??)
                }
                _ => None,
            },
            PlanExpression::FloatCast(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::FloatLiteral(value) => Some(value.into()),
                EncodedTerm::DoubleLiteral(value) => Some((value as f32).into()),
                EncodedTerm::IntegerLiteral(value) => Some((value as f32).into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.to_f32().into()),
                EncodedTerm::BooleanLiteral(value) => {
                    Some(if value { 1_f32 } else { 0_f32 }.into())
                }
                EncodedTerm::StringLiteral { value_id } => {
                    parse_float_str(&*self.dataset.get_str(value_id).ok()??)
                }
                _ => None,
            },
            PlanExpression::IntegerCast(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::FloatLiteral(value) => Some((value as i64).into()),
                EncodedTerm::DoubleLiteral(value) => Some((value as i64).into()),
                EncodedTerm::IntegerLiteral(value) => Some(value.into()),
                EncodedTerm::DecimalLiteral(value) => Some(i64::try_from(value).ok()?.into()),
                EncodedTerm::BooleanLiteral(value) => Some(if value { 1 } else { 0 }.into()),
                EncodedTerm::StringLiteral { value_id } => {
                    parse_integer_str(&*self.dataset.get_str(value_id).ok()??)
                }
                _ => None,
            },
            PlanExpression::DecimalCast(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::FloatLiteral(value) => Some(Decimal::from_f32(value).into()),
                EncodedTerm::DoubleLiteral(value) => Some(Decimal::from_f64(value).into()),
                EncodedTerm::IntegerLiteral(value) => Some(Decimal::from(value).into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.into()),
                EncodedTerm::BooleanLiteral(value) => {
                    Some(Decimal::from(if value { 1 } else { 0 }).into())
                }
                EncodedTerm::StringLiteral { value_id } => {
                    parse_decimal_str(&*self.dataset.get_str(value_id).ok()??)
                }
                _ => None,
            },
            PlanExpression::DateCast(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::DateLiteral(value) => Some(value.into()),
                EncodedTerm::DateTimeLiteral(value) => Some(Date::try_from(value).ok()?.into()),
                EncodedTerm::StringLiteral { value_id } => {
                    parse_date_str(&*self.dataset.get_str(value_id).ok()??)
                }
                _ => None,
            },
            PlanExpression::TimeCast(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::TimeLiteral(value) => Some(value.into()),
                EncodedTerm::DateTimeLiteral(value) => Some(Time::try_from(value).ok()?.into()),
                EncodedTerm::StringLiteral { value_id } => {
                    parse_time_str(&*self.dataset.get_str(value_id).ok()??)
                }
                _ => None,
            },
            PlanExpression::DateTimeCast(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::DateTimeLiteral(value) => Some(value.into()),
                EncodedTerm::DateLiteral(value) => Some(DateTime::try_from(value).ok()?.into()),
                EncodedTerm::StringLiteral { value_id } => {
                    parse_date_time_str(&*self.dataset.get_str(value_id).ok()??)
                }
                _ => None,
            },
            PlanExpression::DurationCast(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::DurationLiteral(value) => Some(value.into()),
                EncodedTerm::StringLiteral { value_id } => {
                    parse_duration_str(&*self.dataset.get_str(value_id).ok()??)
                }
                _ => None,
            },
            PlanExpression::StringCast(e) => Some(EncodedTerm::StringLiteral {
                value_id: self.to_string_id(self.eval_expression(e, tuple)?)?,
            }),
        }
    }

    fn to_bool(&self, term: EncodedTerm) -> Option<bool> {
        match term {
            EncodedTerm::BooleanLiteral(value) => Some(value),
            EncodedTerm::StringLiteral { .. } => Some(term != ENCODED_EMPTY_STRING_LITERAL),
            EncodedTerm::FloatLiteral(value) => Some(value != 0_f32),
            EncodedTerm::DoubleLiteral(value) => Some(value != 0_f64),
            EncodedTerm::IntegerLiteral(value) => Some(value != 0),
            EncodedTerm::DecimalLiteral(value) => Some(value != Decimal::default()),
            _ => None,
        }
    }

    fn to_string_id(&self, term: EncodedTerm) -> Option<StrHash> {
        match term {
            EncodedTerm::DefaultGraph => None,
            EncodedTerm::NamedNode { iri_id } => Some(iri_id),
            EncodedTerm::BlankNode { .. } => None,
            EncodedTerm::StringLiteral { value_id }
            | EncodedTerm::LangStringLiteral { value_id, .. }
            | EncodedTerm::TypedLiteral { value_id, .. } => Some(value_id),
            EncodedTerm::BooleanLiteral(value) => {
                self.build_string_id(if value { "true" } else { "false" })
            }
            EncodedTerm::FloatLiteral(value) => self.build_string_id(&value.to_string()),
            EncodedTerm::DoubleLiteral(value) => self.build_string_id(&value.to_string()),
            EncodedTerm::IntegerLiteral(value) => self.build_string_id(&value.to_string()),
            EncodedTerm::DecimalLiteral(value) => self.build_string_id(&value.to_string()),
            EncodedTerm::DateLiteral(value) => self.build_string_id(&value.to_string()),
            EncodedTerm::TimeLiteral(value) => self.build_string_id(&value.to_string()),
            EncodedTerm::DateTimeLiteral(value) => self.build_string_id(&value.to_string()),
            EncodedTerm::DurationLiteral(value) => self.build_string_id(&value.to_string()),
        }
    }

    fn to_simple_string(&self, term: EncodedTerm) -> Option<String> {
        if let EncodedTerm::StringLiteral { value_id } = term {
            self.dataset.get_str(value_id).ok()?
        } else {
            None
        }
    }

    fn to_simple_string_id(&self, term: EncodedTerm) -> Option<StrHash> {
        if let EncodedTerm::StringLiteral { value_id } = term {
            Some(value_id)
        } else {
            None
        }
    }

    fn to_string(&self, term: EncodedTerm) -> Option<String> {
        match term {
            EncodedTerm::StringLiteral { value_id }
            | EncodedTerm::LangStringLiteral { value_id, .. } => {
                self.dataset.get_str(value_id).ok()?
            }
            _ => None,
        }
    }

    fn to_string_and_language(&self, term: EncodedTerm) -> Option<(String, Option<StrHash>)> {
        match term {
            EncodedTerm::StringLiteral { value_id } => {
                Some((self.dataset.get_str(value_id).ok()??, None))
            }
            EncodedTerm::LangStringLiteral {
                value_id,
                language_id,
            } => Some((self.dataset.get_str(value_id).ok()??, Some(language_id))),
            _ => None,
        }
    }

    fn build_named_node(&self, iri: &str) -> Option<EncodedTerm> {
        Some(EncodedTerm::NamedNode {
            iri_id: self.build_string_id(iri)?,
        })
    }

    fn build_string_literal(&self, value: &str) -> Option<EncodedTerm> {
        Some(EncodedTerm::StringLiteral {
            value_id: self.build_string_id(value)?,
        })
    }

    fn build_lang_string_literal(&self, value: &str, language_id: StrHash) -> Option<EncodedTerm> {
        Some(EncodedTerm::LangStringLiteral {
            value_id: self.build_string_id(value)?,
            language_id,
        })
    }

    fn build_plain_literal(&self, value: &str, language: Option<StrHash>) -> Option<EncodedTerm> {
        if let Some(language_id) = language {
            self.build_lang_string_literal(value, language_id)
        } else {
            self.build_string_literal(value)
        }
    }

    fn build_string_id(&self, value: &str) -> Option<StrHash> {
        let value_id = StrHash::new(value);
        self.dataset.encoder().insert_str(value_id, value).ok()?;
        Some(value_id)
    }

    fn build_language_id(&self, value: EncodedTerm) -> Option<StrHash> {
        let mut language = self.to_simple_string(value)?;
        language.make_ascii_lowercase();
        self.build_string_id(LanguageTag::parse(language).ok()?.as_str())
    }

    fn to_argument_compatible_strings(
        &self,
        arg1: EncodedTerm,
        arg2: EncodedTerm,
    ) -> Option<(String, String, Option<StrHash>)> {
        let (value1, language1) = self.to_string_and_language(arg1)?;
        let (value2, language2) = self.to_string_and_language(arg2)?;
        if language2.is_none() || language1 == language2 {
            Some((value1, value2, language1))
        } else {
            None
        }
    }

    fn compile_pattern(&self, pattern: EncodedTerm, flags: Option<EncodedTerm>) -> Option<Regex> {
        // TODO Avoid to compile the regex each time
        let pattern = self.to_simple_string(pattern)?;
        let mut regex_builder = RegexBuilder::new(&pattern);
        regex_builder.size_limit(REGEX_SIZE_LIMIT);
        if let Some(flags) = flags {
            let flags = self.to_simple_string(flags)?;
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
                    'q' => (), //TODO: implement
                    _ => (),
                }
            }
        }
        regex_builder.build().ok()
    }

    fn parse_numeric_operands<'b>(
        &'b self,
        e1: &PlanExpression,
        e2: &PlanExpression,
        tuple: &EncodedTuple,
    ) -> Option<NumericBinaryOperands> {
        NumericBinaryOperands::new(
            self.eval_expression(e1, tuple)?,
            self.eval_expression(e2, tuple)?,
        )
    }

    fn decode_bindings<'b>(
        &'b self,
        iter: EncodedTuplesIterator<'b>,
        variables: Vec<Variable>,
    ) -> QuerySolutionsIterator<'b>
    where
        'a: 'b,
    {
        let eval = self;
        let tuple_size = variables.len();
        QuerySolutionsIterator::new(
            variables,
            Box::new(iter.map(move |values| {
                let mut result = vec![None; tuple_size];
                for (i, value) in values?.iter().enumerate() {
                    if let Some(term) = value {
                        result[i] = Some(eval.dataset.decode_term(term)?)
                    }
                }
                Ok(result)
            })),
        )
    }

    // this is used to encode results from a BindingIterator into an EncodedTuplesIterator. This happens when SERVICE clauses are evaluated
    fn encode_bindings<'b>(
        &'b self,
        variables: &'b [Variable],
        iter: QuerySolutionsIterator<'b>,
    ) -> EncodedTuplesIterator<'b>
    where
        'a: 'b,
    {
        Box::new(iter.map(move |solution| {
            let mut encoder = self.dataset.encoder();
            let mut encoded_terms = EncodedTuple::with_capacity(variables.len());
            for (variable, term) in solution?.iter() {
                put_variable_value(
                    variable,
                    variables,
                    encoder.encode_term(term)?,
                    &mut encoded_terms,
                )
            }
            Ok(encoded_terms)
        }))
    }

    #[allow(
        clippy::float_cmp,
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss
    )]
    fn equals(&self, a: EncodedTerm, b: EncodedTerm) -> Option<bool> {
        match a {
            EncodedTerm::DefaultGraph
            | EncodedTerm::NamedNode { .. }
            | EncodedTerm::BlankNode { .. }
            | EncodedTerm::LangStringLiteral { .. } => Some(a == b),
            EncodedTerm::StringLiteral { value_id: a } => match b {
                EncodedTerm::StringLiteral { value_id: b } => Some(a == b),
                EncodedTerm::TypedLiteral { .. } => None,
                _ => Some(false),
            },
            EncodedTerm::BooleanLiteral(a) => match b {
                EncodedTerm::BooleanLiteral(b) => Some(a == b),
                EncodedTerm::TypedLiteral { .. } => None,
                _ => Some(false),
            },
            EncodedTerm::FloatLiteral(a) => match b {
                EncodedTerm::FloatLiteral(b) => Some(a == b),
                EncodedTerm::DoubleLiteral(b) => Some(f64::from(a) == b),
                EncodedTerm::IntegerLiteral(b) => Some(a == b as f32),
                EncodedTerm::DecimalLiteral(b) => Some(a == b.to_f32()),
                EncodedTerm::TypedLiteral { .. } => None,
                _ => Some(false),
            },
            EncodedTerm::DoubleLiteral(a) => match b {
                EncodedTerm::FloatLiteral(b) => Some(a == f64::from(b)),
                EncodedTerm::DoubleLiteral(b) => Some(a == b),
                EncodedTerm::IntegerLiteral(b) => Some(a == (b as f64)),
                EncodedTerm::DecimalLiteral(b) => Some(a == b.to_f64()),
                EncodedTerm::TypedLiteral { .. } => None,
                _ => Some(false),
            },
            EncodedTerm::IntegerLiteral(a) => match b {
                EncodedTerm::FloatLiteral(b) => Some((a as f32) == b),
                EncodedTerm::DoubleLiteral(b) => Some((a as f64) == b),
                EncodedTerm::IntegerLiteral(b) => Some(a == b),
                EncodedTerm::DecimalLiteral(b) => Some(Decimal::from(a) == b),
                EncodedTerm::TypedLiteral { .. } => None,
                _ => Some(false),
            },
            EncodedTerm::DecimalLiteral(a) => match b {
                EncodedTerm::FloatLiteral(b) => Some(a.to_f32() == b),
                EncodedTerm::DoubleLiteral(b) => Some(a.to_f64() == b),
                EncodedTerm::IntegerLiteral(b) => Some(a == Decimal::from(b)),
                EncodedTerm::DecimalLiteral(b) => Some(a == b),
                EncodedTerm::TypedLiteral { .. } => None,
                _ => Some(false),
            },
            EncodedTerm::TypedLiteral { .. } => match b {
                EncodedTerm::TypedLiteral { .. } if a == b => Some(true),
                EncodedTerm::NamedNode { .. }
                | EncodedTerm::BlankNode { .. }
                | EncodedTerm::LangStringLiteral { .. } => Some(false),
                _ => None,
            },
            EncodedTerm::DateLiteral(a) => match b {
                EncodedTerm::DateLiteral(b) => Some(a == b),
                EncodedTerm::TypedLiteral { .. } => None,
                _ => Some(false),
            },
            EncodedTerm::TimeLiteral(a) => match b {
                EncodedTerm::TimeLiteral(b) => Some(a == b),
                EncodedTerm::TypedLiteral { .. } => None,
                _ => Some(false),
            },
            EncodedTerm::DateTimeLiteral(a) => match b {
                EncodedTerm::DateTimeLiteral(b) => Some(a == b),
                EncodedTerm::TypedLiteral { .. } => None,
                _ => Some(false),
            },
            EncodedTerm::DurationLiteral(a) => match b {
                EncodedTerm::DurationLiteral(b) => Some(a == b),
                EncodedTerm::TypedLiteral { .. } => None,
                _ => Some(false),
            },
        }
    }

    fn cmp_according_to_expression<'b>(
        &'b self,
        tuple_a: &EncodedTuple,
        tuple_b: &EncodedTuple,
        expression: &PlanExpression,
    ) -> Ordering {
        self.cmp_terms(
            self.eval_expression(expression, tuple_a),
            self.eval_expression(expression, tuple_b),
        )
    }

    fn cmp_terms(&self, a: Option<EncodedTerm>, b: Option<EncodedTerm>) -> Ordering {
        match (a, b) {
            (Some(a), Some(b)) => match a {
                EncodedTerm::BlankNode { id: a } => {
                    if let EncodedTerm::BlankNode { id: b } = b {
                        a.cmp(&b)
                    } else {
                        Ordering::Less
                    }
                }
                EncodedTerm::NamedNode { iri_id: a } => match b {
                    EncodedTerm::NamedNode { iri_id: b } => {
                        self.compare_str_ids(a, b).unwrap_or(Ordering::Equal)
                    }
                    EncodedTerm::BlankNode { .. } => Ordering::Greater,
                    _ => Ordering::Less,
                },
                a => match b {
                    EncodedTerm::NamedNode { .. } | EncodedTerm::BlankNode { .. } => {
                        Ordering::Greater
                    }
                    b => self.partial_cmp_literals(a, b).unwrap_or(Ordering::Equal),
                },
            },
            (Some(_), None) => Ordering::Greater,
            (None, Some(_)) => Ordering::Less,
            (None, None) => Ordering::Equal,
        }
    }

    #[allow(clippy::cast_precision_loss)]
    fn partial_cmp_literals(&self, a: EncodedTerm, b: EncodedTerm) -> Option<Ordering> {
        match a {
            EncodedTerm::StringLiteral { value_id: a } => {
                if let EncodedTerm::StringLiteral { value_id: b } = b {
                    self.compare_str_ids(a, b)
                } else {
                    None
                }
            }
            EncodedTerm::FloatLiteral(a) => match b {
                EncodedTerm::FloatLiteral(ref b) => a.partial_cmp(b),
                EncodedTerm::DoubleLiteral(ref b) => f64::from(a).partial_cmp(b),
                EncodedTerm::IntegerLiteral(b) => a.partial_cmp(&(b as f32)),
                EncodedTerm::DecimalLiteral(b) => a.partial_cmp(&b.to_f32()),
                _ => None,
            },
            EncodedTerm::DoubleLiteral(a) => match b {
                EncodedTerm::FloatLiteral(b) => a.partial_cmp(&b.into()),
                EncodedTerm::DoubleLiteral(ref b) => a.partial_cmp(b),
                EncodedTerm::IntegerLiteral(b) => a.partial_cmp(&(b as f64)),
                EncodedTerm::DecimalLiteral(b) => a.partial_cmp(&b.to_f64()),
                _ => None,
            },
            EncodedTerm::IntegerLiteral(a) => match b {
                EncodedTerm::FloatLiteral(ref b) => (a as f32).partial_cmp(b),
                EncodedTerm::DoubleLiteral(ref b) => (a as f64).partial_cmp(b),
                EncodedTerm::IntegerLiteral(ref b) => a.partial_cmp(b),
                EncodedTerm::DecimalLiteral(b) => Decimal::from(a).partial_cmp(&b),
                _ => None,
            },
            EncodedTerm::DecimalLiteral(a) => match b {
                EncodedTerm::FloatLiteral(ref b) => a.to_f32().partial_cmp(b),
                EncodedTerm::DoubleLiteral(ref b) => a.to_f64().partial_cmp(b),
                EncodedTerm::IntegerLiteral(b) => a.partial_cmp(&Decimal::from(b)),
                EncodedTerm::DecimalLiteral(ref b) => a.partial_cmp(b),
                _ => None,
            },
            EncodedTerm::DateLiteral(a) => {
                if let EncodedTerm::DateLiteral(ref b) = b {
                    a.partial_cmp(b)
                } else {
                    None
                }
            }
            EncodedTerm::TimeLiteral(a) => {
                if let EncodedTerm::TimeLiteral(ref b) = b {
                    a.partial_cmp(b)
                } else {
                    None
                }
            }
            EncodedTerm::DateTimeLiteral(a) => {
                if let EncodedTerm::DateTimeLiteral(ref b) = b {
                    a.partial_cmp(b)
                } else {
                    None
                }
            }
            EncodedTerm::DurationLiteral(a) => {
                if let EncodedTerm::DurationLiteral(ref b) = b {
                    a.partial_cmp(b)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn compare_str_ids(&self, a: StrHash, b: StrHash) -> Option<Ordering> {
        Some(
            self.dataset
                .get_str(a)
                .ok()??
                .cmp(&self.dataset.get_str(b).ok()??),
        )
    }

    fn hash<'b, H: Digest>(
        &'b self,
        arg: &PlanExpression,
        tuple: &EncodedTuple,
    ) -> Option<EncodedTerm> {
        let input = self.to_simple_string(self.eval_expression(arg, tuple)?)?;
        let hash = hex::encode(H::new().chain(input.as_str()).result());
        self.build_string_literal(&hash)
    }
}

enum NumericBinaryOperands {
    Float(f32, f32),
    Double(f64, f64),
    Integer(i64, i64),
    Decimal(Decimal, Decimal),
    Duration(Duration, Duration),
    DateTime(DateTime, DateTime),
    Time(Time, Time),
    Date(Date, Date),
    DateTimeDuration(DateTime, Duration),
    TimeDuration(Time, Duration),
    DateDuration(Date, Duration),
}

impl NumericBinaryOperands {
    #[allow(clippy::cast_precision_loss)]
    fn new(a: EncodedTerm, b: EncodedTerm) -> Option<Self> {
        match (a, b) {
            (EncodedTerm::FloatLiteral(v1), EncodedTerm::FloatLiteral(v2)) => {
                Some(NumericBinaryOperands::Float(v1, v2))
            }
            (EncodedTerm::FloatLiteral(v1), EncodedTerm::DoubleLiteral(v2)) => {
                Some(NumericBinaryOperands::Double(v1.into(), v2))
            }
            (EncodedTerm::FloatLiteral(v1), EncodedTerm::IntegerLiteral(v2)) => {
                Some(NumericBinaryOperands::Float(v1, v2 as f32))
            }
            (EncodedTerm::FloatLiteral(v1), EncodedTerm::DecimalLiteral(v2)) => {
                Some(NumericBinaryOperands::Float(v1, v2.to_f32()))
            }
            (EncodedTerm::DoubleLiteral(v1), EncodedTerm::FloatLiteral(v2)) => {
                Some(NumericBinaryOperands::Double(v1, v2.into()))
            }
            (EncodedTerm::DoubleLiteral(v1), EncodedTerm::DoubleLiteral(v2)) => {
                Some(NumericBinaryOperands::Double(v1, v2))
            }
            (EncodedTerm::DoubleLiteral(v1), EncodedTerm::IntegerLiteral(v2)) => {
                Some(NumericBinaryOperands::Double(v1, v2 as f64))
            }
            (EncodedTerm::DoubleLiteral(v1), EncodedTerm::DecimalLiteral(v2)) => {
                Some(NumericBinaryOperands::Double(v1, v2.to_f64()))
            }
            (EncodedTerm::IntegerLiteral(v1), EncodedTerm::FloatLiteral(v2)) => {
                Some(NumericBinaryOperands::Float(v1 as f32, v2))
            }
            (EncodedTerm::IntegerLiteral(v1), EncodedTerm::DoubleLiteral(v2)) => {
                Some(NumericBinaryOperands::Double(v1 as f64, v2))
            }
            (EncodedTerm::IntegerLiteral(v1), EncodedTerm::IntegerLiteral(v2)) => {
                Some(NumericBinaryOperands::Integer(v1, v2))
            }
            (EncodedTerm::IntegerLiteral(v1), EncodedTerm::DecimalLiteral(v2)) => {
                Some(NumericBinaryOperands::Decimal(Decimal::from(v1), v2))
            }
            (EncodedTerm::DecimalLiteral(v1), EncodedTerm::FloatLiteral(v2)) => {
                Some(NumericBinaryOperands::Float(v1.to_f32(), v2))
            }
            (EncodedTerm::DecimalLiteral(v1), EncodedTerm::DoubleLiteral(v2)) => {
                Some(NumericBinaryOperands::Double(v1.to_f64(), v2))
            }
            (EncodedTerm::DecimalLiteral(v1), EncodedTerm::IntegerLiteral(v2)) => {
                Some(NumericBinaryOperands::Decimal(v1, Decimal::from(v2)))
            }
            (EncodedTerm::DecimalLiteral(v1), EncodedTerm::DecimalLiteral(v2)) => {
                Some(NumericBinaryOperands::Decimal(v1, v2))
            }
            (EncodedTerm::DurationLiteral(v1), EncodedTerm::DurationLiteral(v2)) => {
                Some(NumericBinaryOperands::Duration(v1, v2))
            }
            (EncodedTerm::DateTimeLiteral(v1), EncodedTerm::DateTimeLiteral(v2)) => {
                Some(NumericBinaryOperands::DateTime(v1, v2))
            }
            (EncodedTerm::DateLiteral(v1), EncodedTerm::DateLiteral(v2)) => {
                Some(NumericBinaryOperands::Date(v1, v2))
            }
            (EncodedTerm::TimeLiteral(v1), EncodedTerm::TimeLiteral(v2)) => {
                Some(NumericBinaryOperands::Time(v1, v2))
            }
            (EncodedTerm::DateTimeLiteral(v1), EncodedTerm::DurationLiteral(v2)) => {
                Some(NumericBinaryOperands::DateTimeDuration(v1, v2))
            }
            (EncodedTerm::DateLiteral(v1), EncodedTerm::DurationLiteral(v2)) => {
                Some(NumericBinaryOperands::DateDuration(v1, v2))
            }
            (EncodedTerm::TimeLiteral(v1), EncodedTerm::DurationLiteral(v2)) => {
                Some(NumericBinaryOperands::TimeDuration(v1, v2))
            }
            _ => None,
        }
    }
}

fn get_pattern_value(selector: &PatternValue, tuple: &EncodedTuple) -> Option<EncodedTerm> {
    match selector {
        PatternValue::Constant(term) => Some(*term),
        PatternValue::Variable(v) => tuple.get(*v),
    }
}

fn put_pattern_value(selector: &PatternValue, value: EncodedTerm, tuple: &mut EncodedTuple) {
    match selector {
        PatternValue::Constant(_) => (),
        PatternValue::Variable(v) => tuple.set(*v, value),
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

fn unbind_variables(binding: &mut EncodedTuple, variables: &[usize]) {
    for var in variables {
        binding.unset(*var)
    }
}

fn combine_tuples(mut a: EncodedTuple, b: &EncodedTuple, vars: &[usize]) -> Option<EncodedTuple> {
    for var in vars {
        if let Some(b_value) = b.get(*var) {
            if let Some(a_value) = a.get(*var) {
                if a_value != b_value {
                    return None;
                }
            } else {
                a.set(*var, b_value);
            }
        }
    }
    Some(a)
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

struct JoinIterator<'a> {
    left: Vec<EncodedTuple>,
    right_iter: EncodedTuplesIterator<'a>,
    buffered_results: Vec<Result<EncodedTuple>>,
}

impl<'a> Iterator for JoinIterator<'a> {
    type Item = Result<EncodedTuple>;

    fn next(&mut self) -> Option<Result<EncodedTuple>> {
        loop {
            if let Some(result) = self.buffered_results.pop() {
                return Some(result);
            }
            let right_tuple = match self.right_iter.next()? {
                Ok(right_tuple) => right_tuple,
                Err(error) => return Some(Err(error)),
            };
            for left_tuple in &self.left {
                if let Some(result_tuple) = left_tuple.combine_with(&right_tuple) {
                    self.buffered_results.push(Ok(result_tuple))
                }
            }
        }
    }
}

struct AntiJoinIterator<'a> {
    left_iter: EncodedTuplesIterator<'a>,
    right: Vec<EncodedTuple>,
}

impl<'a> Iterator for AntiJoinIterator<'a> {
    type Item = Result<EncodedTuple>;

    fn next(&mut self) -> Option<Result<EncodedTuple>> {
        loop {
            match self.left_iter.next()? {
                Ok(left_tuple) => {
                    let exists_compatible_right = self.right.iter().any(|right_tuple| {
                        are_compatible_and_not_disjointed(&left_tuple, right_tuple)
                    });
                    if !exists_compatible_right {
                        return Some(Ok(left_tuple));
                    }
                }
                Err(error) => return Some(Err(error)),
            }
        }
    }
}

struct LeftJoinIterator<'a, S: ReadableEncodedStore> {
    eval: &'a SimpleEvaluator<S>,
    right_plan: &'a PlanNode,
    left_iter: EncodedTuplesIterator<'a>,
    current_right: EncodedTuplesIterator<'a>,
}

impl<'a, S: ReadableEncodedStore> Iterator for LeftJoinIterator<'a, S> {
    type Item = Result<EncodedTuple>;

    fn next(&mut self) -> Option<Result<EncodedTuple>> {
        if let Some(tuple) = self.current_right.next() {
            return Some(tuple);
        }
        match self.left_iter.next()? {
            Ok(left_tuple) => {
                self.current_right = self.eval.eval_plan(self.right_plan, left_tuple.clone());
                if let Some(right_tuple) = self.current_right.next() {
                    Some(right_tuple)
                } else {
                    Some(Ok(left_tuple))
                }
            }
            Err(error) => Some(Err(error)),
        }
    }
}

struct BadLeftJoinIterator<'a, S: ReadableEncodedStore> {
    eval: &'a SimpleEvaluator<S>,
    right_plan: &'a PlanNode,
    left_iter: EncodedTuplesIterator<'a>,
    current_left: Option<EncodedTuple>,
    current_right: EncodedTuplesIterator<'a>,
    problem_vars: &'a [usize],
}

impl<'a, S: ReadableEncodedStore> Iterator for BadLeftJoinIterator<'a, S> {
    type Item = Result<EncodedTuple>;

    fn next(&mut self) -> Option<Result<EncodedTuple>> {
        while let Some(right_tuple) = self.current_right.next() {
            match right_tuple {
                Ok(right_tuple) => {
                    if let Some(combined) = combine_tuples(
                        right_tuple,
                        self.current_left.as_ref().unwrap(),
                        self.problem_vars,
                    ) {
                        return Some(Ok(combined));
                    }
                }
                Err(error) => return Some(Err(error)),
            }
        }
        match self.left_iter.next()? {
            Ok(left_tuple) => {
                let mut filtered_left = left_tuple.clone();
                unbind_variables(&mut filtered_left, self.problem_vars);
                self.current_right = self.eval.eval_plan(self.right_plan, filtered_left);
                while let Some(right_tuple) = self.current_right.next() {
                    match right_tuple {
                        Ok(right_tuple) => {
                            if let Some(combined) =
                                combine_tuples(right_tuple, &left_tuple, self.problem_vars)
                            {
                                self.current_left = Some(left_tuple);
                                return Some(Ok(combined));
                            }
                        }
                        Err(error) => return Some(Err(error)),
                    }
                }
                Some(Ok(left_tuple))
            }
            Err(error) => Some(Err(error)),
        }
    }
}

struct UnionIterator<'a, S: ReadableEncodedStore> {
    eval: &'a SimpleEvaluator<S>,
    plans: &'a [PlanNode],
    input: EncodedTuple,
    current_iterator: EncodedTuplesIterator<'a>,
    current_plan: usize,
}

impl<'a, S: ReadableEncodedStore> Iterator for UnionIterator<'a, S> {
    type Item = Result<EncodedTuple>;

    fn next(&mut self) -> Option<Result<EncodedTuple>> {
        loop {
            if let Some(tuple) = self.current_iterator.next() {
                return Some(tuple);
            }
            if self.current_plan >= self.plans.len() {
                return None;
            }
            self.current_iterator = self
                .eval
                .eval_plan(&self.plans[self.current_plan], self.input.clone());
            self.current_plan += 1;
        }
    }
}

struct ConstructIterator<'a, S: ReadableEncodedStore> {
    eval: &'a SimpleEvaluator<S>,
    iter: EncodedTuplesIterator<'a>,
    template: &'a [TripleTemplate],
    buffered_results: Vec<Result<Triple>>,
    bnodes: Vec<BlankNode>,
}

impl<'a, S: ReadableEncodedStore + 'a> Iterator for ConstructIterator<'a, S> {
    type Item = Result<Triple>;

    fn next(&mut self) -> Option<Result<Triple>> {
        loop {
            if let Some(result) = self.buffered_results.pop() {
                return Some(result);
            }
            {
                let tuple = match self.iter.next()? {
                    Ok(tuple) => tuple,
                    Err(error) => return Some(Err(error)),
                };
                for template in self.template {
                    if let (Some(subject), Some(predicate), Some(object)) = (
                        get_triple_template_value(&template.subject, &tuple, &mut self.bnodes),
                        get_triple_template_value(&template.predicate, &tuple, &mut self.bnodes),
                        get_triple_template_value(&template.object, &tuple, &mut self.bnodes),
                    ) {
                        self.buffered_results.push(decode_triple(
                            &self.eval.dataset,
                            subject,
                            predicate,
                            object,
                        ));
                    }
                }
                self.bnodes.clear(); //We do not reuse old bnodes
            }
        }
    }
}

fn get_triple_template_value(
    selector: &TripleTemplateValue,
    tuple: &EncodedTuple,
    bnodes: &mut Vec<BlankNode>,
) -> Option<EncodedTerm> {
    match selector {
        TripleTemplateValue::Constant(term) => Some(*term),
        TripleTemplateValue::Variable(v) => tuple.get(*v),
        TripleTemplateValue::BlankNode(id) => {
            if *id >= bnodes.len() {
                bnodes.resize_with(*id, BlankNode::default)
            }
            Some((&bnodes[*id]).into())
        }
    }
}

fn decode_triple(
    decoder: &impl Decoder,
    subject: EncodedTerm,
    predicate: EncodedTerm,
    object: EncodedTerm,
) -> Result<Triple> {
    Ok(Triple::new(
        decoder.decode_named_or_blank_node(subject)?,
        decoder.decode_named_node(predicate)?,
        decoder.decode_term(object)?,
    ))
}

struct DescribeIterator<'a, S: ReadableEncodedStore> {
    eval: &'a SimpleEvaluator<S>,
    iter: EncodedTuplesIterator<'a>,
    quads: Box<dyn Iterator<Item = Result<EncodedQuad>> + 'a>,
}

impl<'a, S: ReadableEncodedStore + 'a> Iterator for DescribeIterator<'a, S> {
    type Item = Result<Triple>;

    fn next(&mut self) -> Option<Result<Triple>> {
        loop {
            if let Some(quad) = self.quads.next() {
                return Some(match quad {
                    Ok(quad) => self
                        .eval
                        .dataset
                        .decode_quad(&quad)
                        .map(|q| q.into_triple()),
                    Err(error) => Err(error),
                });
            }
            let tuple = match self.iter.next()? {
                Ok(tuple) => tuple,
                Err(error) => return Some(Err(error)),
            };
            for subject in tuple.iter() {
                if let Some(subject) = subject {
                    self.quads =
                        self.eval
                            .dataset
                            .quads_for_pattern(Some(subject), None, None, None);
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

    fn next(&mut self) -> Option<(Option<T1>, Option<T2>)> {
        match (self.a.next(), self.b.next()) {
            (None, None) => None,
            r => Some(r),
        }
    }
}

fn transitive_closure<'a, T: 'a + Copy + Eq + Hash, NI: Iterator<Item = Result<T>> + 'a>(
    start: impl IntoIterator<Item = Result<T>>,
    next: impl Fn(T) -> NI,
) -> impl Iterator<Item = Result<T>> + 'a {
    //TODO: optimize
    let mut all = HashSet::<T>::default();
    let mut errors = Vec::default();
    let mut current = start
        .into_iter()
        .filter_map(|e| match e {
            Ok(e) => {
                all.insert(e);
                Some(e)
            }
            Err(error) => {
                errors.push(error);
                None
            }
        })
        .collect::<Vec<_>>();

    while !current.is_empty() {
        current = current
            .into_iter()
            .flat_map(|e| next(e))
            .filter_map(|e| match e {
                Ok(e) => {
                    if all.contains(&e) {
                        None
                    } else {
                        all.insert(e);
                        Some(e)
                    }
                }
                Err(error) => {
                    errors.push(error);
                    None
                }
            })
            .collect();
    }
    errors.into_iter().map(Err).chain(all.into_iter().map(Ok))
}

fn hash_deduplicate<T: Eq + Hash + Clone>(
    iter: impl Iterator<Item = Result<T>>,
) -> impl Iterator<Item = Result<T>> {
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

trait ResultIterator<T>: Iterator<Item = Result<T>> + Sized {
    fn flat_map_ok<O, F: FnMut(T) -> U, U: IntoIterator<Item = Result<O>>>(
        self,
        f: F,
    ) -> FlatMapOk<T, O, Self, F, U>;
}

impl<T, I: Iterator<Item = Result<T>> + Sized> ResultIterator<T> for I {
    fn flat_map_ok<O, F: FnMut(T) -> U, U: IntoIterator<Item = Result<O>>>(
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
    I: Iterator<Item = Result<T>>,
    F: FnMut(T) -> U,
    U: IntoIterator<Item = Result<O>>,
> {
    inner: I,
    f: F,
    current: Option<U::IntoIter>,
}

impl<T, O, I: Iterator<Item = Result<T>>, F: FnMut(T) -> U, U: IntoIterator<Item = Result<O>>>
    Iterator for FlatMapOk<T, O, I, F, U>
{
    type Item = Result<O>;

    fn next(&mut self) -> Option<Result<O>> {
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

#[derive(Default, Debug)]
struct DistinctAccumulator<T: Accumulator> {
    seen: HashSet<Option<EncodedTerm>>,
    inner: T,
}

impl<T: Accumulator> DistinctAccumulator<T> {
    fn new(inner: T) -> Self {
        Self {
            seen: HashSet::default(),
            inner,
        }
    }
}

impl<T: Accumulator> Accumulator for DistinctAccumulator<T> {
    fn add(&mut self, element: Option<EncodedTerm>) {
        if self.seen.insert(element) {
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

#[derive(Debug)]
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
        if let Some(sum) = self.sum {
            if let Some(operands) = element.and_then(|e| NumericBinaryOperands::new(sum, e)) {
                //TODO: unify with addition?
                self.sum = match operands {
                    NumericBinaryOperands::Float(v1, v2) => Some((v1 + v2).into()),
                    NumericBinaryOperands::Double(v1, v2) => Some((v1 + v2).into()),
                    NumericBinaryOperands::Integer(v1, v2) => v1.checked_add(v2).map(|v| v.into()),
                    NumericBinaryOperands::Decimal(v1, v2) => v1.checked_add(v2).map(|v| v.into()),
                    NumericBinaryOperands::Duration(v1, v2) => v1.checked_add(v2).map(|v| v.into()),
                    _ => None,
                };
            } else {
                self.sum = None;
            }
        }
    }

    fn state(&self) -> Option<EncodedTerm> {
        self.sum
    }
}

#[derive(Debug, Default)]
struct AvgAccumulator {
    sum: SumAccumulator,
    count: CountAccumulator,
}

impl Accumulator for AvgAccumulator {
    fn add(&mut self, element: Option<EncodedTerm>) {
        self.sum.add(element);
        self.count.add(element);
    }

    fn state(&self) -> Option<EncodedTerm> {
        let sum = self.sum.state()?;
        let count = self.count.state()?;
        if count == EncodedTerm::from(0) {
            Some(0.into())
        } else {
            //TODO: deduplicate?
            //TODO: duration?
            match NumericBinaryOperands::new(sum, count)? {
                NumericBinaryOperands::Float(v1, v2) => Some((v1 / v2).into()),
                NumericBinaryOperands::Double(v1, v2) => Some((v1 / v2).into()),
                NumericBinaryOperands::Integer(v1, v2) => {
                    Decimal::from(v1).checked_div(v2).map(|v| v.into())
                }
                NumericBinaryOperands::Decimal(v1, v2) => v1.checked_div(v2).map(|v| v.into()),
                _ => None,
            }
        }
    }
}

#[allow(clippy::option_option)]
struct MinAccumulator<'a, S: ReadableEncodedStore> {
    eval: &'a SimpleEvaluator<S>,
    min: Option<Option<EncodedTerm>>,
}

impl<'a, S: ReadableEncodedStore + 'a> MinAccumulator<'a, S> {
    fn new(eval: &'a SimpleEvaluator<S>) -> Self {
        Self { eval, min: None }
    }
}

impl<'a, S: ReadableEncodedStore + 'a> Accumulator for MinAccumulator<'a, S> {
    fn add(&mut self, element: Option<EncodedTerm>) {
        if let Some(min) = self.min {
            if self.eval.cmp_terms(element, min) == Ordering::Less {
                self.min = Some(element)
            }
        } else {
            self.min = Some(element)
        }
    }

    fn state(&self) -> Option<EncodedTerm> {
        self.min.and_then(|v| v)
    }
}

#[allow(clippy::option_option)]
struct MaxAccumulator<'a, S: ReadableEncodedStore> {
    eval: &'a SimpleEvaluator<S>,
    max: Option<Option<EncodedTerm>>,
}

impl<'a, S: ReadableEncodedStore + 'a> MaxAccumulator<'a, S> {
    fn new(eval: &'a SimpleEvaluator<S>) -> Self {
        Self { eval, max: None }
    }
}

impl<'a, S: ReadableEncodedStore + 'a> Accumulator for MaxAccumulator<'a, S> {
    fn add(&mut self, element: Option<EncodedTerm>) {
        if let Some(max) = self.max {
            if self.eval.cmp_terms(element, max) == Ordering::Greater {
                self.max = Some(element)
            }
        } else {
            self.max = Some(element)
        }
    }

    fn state(&self) -> Option<EncodedTerm> {
        self.max.and_then(|v| v)
    }
}

#[derive(Default, Debug)]
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
        self.value
    }
}

#[allow(clippy::option_option)]
struct GroupConcatAccumulator<'a, S: ReadableEncodedStore> {
    eval: &'a SimpleEvaluator<S>,
    concat: Option<String>,
    language: Option<Option<StrHash>>,
    separator: &'a str,
}

impl<'a, S: ReadableEncodedStore + 'a> GroupConcatAccumulator<'a, S> {
    fn new(eval: &'a SimpleEvaluator<S>, separator: &'a str) -> Self {
        Self {
            eval,
            concat: Some("".to_owned()),
            language: None,
            separator,
        }
    }
}

impl<'a, S: ReadableEncodedStore + 'a> Accumulator for GroupConcatAccumulator<'a, S> {
    fn add(&mut self, element: Option<EncodedTerm>) {
        if let Some(concat) = self.concat.as_mut() {
            if let Some(element) = element {
                if let Some((value, e_language)) = self.eval.to_string_and_language(element) {
                    if let Some(lang) = self.language {
                        if lang != e_language {
                            self.language = Some(None)
                        }
                        concat.push_str(self.separator);
                    } else {
                        self.language = Some(e_language)
                    }
                    concat.push_str(&value);
                }
            }
        }
    }

    fn state(&self) -> Option<EncodedTerm> {
        self.concat.as_ref().and_then(|result| {
            self.eval
                .build_plain_literal(result, self.language.and_then(|v| v))
        })
    }
}

fn generate_uuid(buffer: &mut String) {
    let mut uuid = random::<u128>().to_ne_bytes();
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

#[test]
fn uuid() {
    let mut buffer = String::default();
    generate_uuid(&mut buffer);
    assert!(
        Regex::new("^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")
            .unwrap()
            .is_match(&buffer),
        "{} is not a valid UUID",
        buffer
    );
}
