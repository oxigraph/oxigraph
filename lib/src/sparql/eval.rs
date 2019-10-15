use crate::model::BlankNode;
use crate::model::Triple;
use crate::sparql::model::*;
use crate::sparql::QueryOptions;
use crate::sparql::plan::*;
use crate::store::numeric_encoder::*;
use crate::store::StoreConnection;
use crate::Result;
use chrono::prelude::*;
use digest::Digest;
use failure::format_err;
use md5::Md5;
use num_traits::identities::Zero;
use num_traits::FromPrimitive;
use num_traits::One;
use num_traits::ToPrimitive;
use rand::random;
use regex::{Regex, RegexBuilder};
use rio_api::iri::Iri;
use rio_api::model as rio;
use rust_decimal::{Decimal, RoundingStrategy};
use sha1::Sha1;
use sha2::{Sha256, Sha384, Sha512};
use std::cmp::min;
use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::convert::TryInto;
use std::fmt::Write;
use std::hash::Hash;
use std::iter::Iterator;
use std::iter::{empty, once};
use std::ops::Deref;
use std::str;
use std::sync::Mutex;
use uuid::Uuid;

const REGEX_SIZE_LIMIT: usize = 1_000_000;

type EncodedTuplesIterator<'a> = Box<dyn Iterator<Item = Result<EncodedTuple>> + 'a>;

pub struct SimpleEvaluator<S: StoreConnection> {
    dataset: DatasetView<S>,
    bnodes_map: Mutex<BTreeMap<u128, u128>>,
    base_iri: Option<Iri<String>>,
    now: DateTime<FixedOffset>,
}

impl<'a, S: StoreConnection + 'a> SimpleEvaluator<S> {
    pub fn new(dataset: DatasetView<S>, base_iri: Option<Iri<String>>) -> Self {
        Self {
            dataset,
            bnodes_map: Mutex::new(BTreeMap::default()),
            base_iri,
            now: Utc::now().with_timezone(&FixedOffset::east(0)),
        }
    }

    pub fn evaluate_select_plan<'b>(
        &'b self,
        plan: &'b PlanNode,
        variables: &[Variable],
        options: &'b QueryOptions<'b>
    ) -> Result<QueryResult<'b>>
    where
        'a: 'b,
    {
        let iter = self.eval_plan(plan, vec![None; variables.len()], &options);
        Ok(QueryResult::Bindings(
            self.decode_bindings(iter, variables.to_vec()),
        ))
    }

    pub fn evaluate_ask_plan<'b>(
        &'b self,
        plan: &'b PlanNode,
        options: &'b QueryOptions<'b>
    ) -> Result<QueryResult<'b>>
    where
        'a: 'b,
    {
        match self.eval_plan(plan, vec![], &options).next() {
            Some(Ok(_)) => Ok(QueryResult::Boolean(true)),
            Some(Err(error)) => Err(error),
            None => Ok(QueryResult::Boolean(false)),
        }
    }

    pub fn evaluate_construct_plan<'b>(
        &'b self,
        plan: &'b PlanNode,
        construct: &'b [TripleTemplate],
        options: &'b QueryOptions<'b>
    ) -> Result<QueryResult<'b>>
    where
        'a: 'b,
    {
        Ok(QueryResult::Graph(Box::new(ConstructIterator {
            eval: self,
            iter: self.eval_plan(plan, vec![], options),
            template: construct,
            buffered_results: Vec::default(),
            bnodes: Vec::default(),
        })))
    }

    pub fn evaluate_describe_plan<'b>(
        &'b self,
        plan: &'b PlanNode,
        options: &'b QueryOptions<'b>
    ) -> Result<QueryResult<'b>>
    where
        'a: 'b,
    {
        Ok(QueryResult::Graph(Box::new(DescribeIterator {
            eval: self,
            iter: self.eval_plan(plan, vec![], options),
            quads: Box::new(empty()),
        })))
    }

    fn eval_plan<'b>(
        &'b self,
        node: &'b PlanNode,
        from: EncodedTuple,
        options: &'b QueryOptions<'b>
    ) -> EncodedTuplesIterator<'b>
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
            } => {
                match &options.service_handler {
                    None => if *silent {
                        return Box::new(vec![].into_iter());
                    } else {
                        return Box::new(once(Err(format_err!(
                            "No handler was supplied to resolve the given service"
                        )))) as EncodedTuplesIterator<'_>;
                    },
                    Some(handler) => {
                        let pattern_option = match get_pattern_value(service_name, &[]) {
                            None => if *silent {
                                        return Box::new(vec![].into_iter());
                                    } else {
                                        return Box::new(once(Err(format_err!(
                                            "The handler supplied was unable to evaluate the given service"
                                        )))) as EncodedTuplesIterator<'_>;
                                    },
                            Some(term) => {
                                let named_node = self.dataset.decode_named_node(term).unwrap();
                                handler.handle(named_node)
                            },
                        };
                            
                        match pattern_option {
                            None => if *silent {
                                        return Box::new(vec![].into_iter());
                                    } else {
                                        return Box::new(once(Err(format_err!(
                                            "The handler supplied was unable to produce any result set on the given service"
                                        )))) as EncodedTuplesIterator<'_>;
                                    }, 
                            Some(pattern_fn) => {
                                let bindings = pattern_fn(graph_pattern.clone()).unwrap();
                                let encoded = self.encode_bindings(variables, bindings);
                                let collected = encoded.collect::<Vec<_>>();
                                Box::new(JoinIterator {
                                    left: vec![from],
                                    right_iter: Box::new(collected.into_iter()),
                                    buffered_results: vec![],
                                })
                           },
                        }
                    }
                }
            },
            PlanNode::QuadPatternJoin {
                child,
                subject,
                predicate,
                object,
                graph_name,
            } => Box::new(self.eval_plan(&*child, from, options).flat_map_ok(move |tuple| {
                let mut iter = self.dataset.quads_for_pattern(
                    get_pattern_value(&subject, &tuple),
                    get_pattern_value(&predicate, &tuple),
                    get_pattern_value(&object, &tuple),
                    get_pattern_value(&graph_name, &tuple),
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
                    iter = Box::new(iter.filter(|quad| match quad {
                        Err(_) => true,
                        Ok(quad) => quad.graph_name != ENCODED_DEFAULT_GRAPH,
                    }));
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
                    put_pattern_value(&subject, quad.subject, &mut new_tuple);
                    put_pattern_value(&predicate, quad.predicate, &mut new_tuple);
                    put_pattern_value(&object, quad.object, &mut new_tuple);
                    put_pattern_value(&graph_name, quad.graph_name, &mut new_tuple);
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
            } => Box::new(self.eval_plan(&*child, from, options).flat_map_ok(move |tuple| {
                let input_subject = get_pattern_value(&subject, &tuple);
                let input_object = get_pattern_value(&object, &tuple);
                let input_graph_name =
                    if let Some(graph_name) = get_pattern_value(&graph_name, &tuple) {
                        graph_name
                    } else {
                        return Box::new(once(Err(format_err!(
                            "Unknown graph name is not allowed when evaluating property path"
                        )))) as EncodedTuplesIterator<'_>;
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
                    )
                        as EncodedTuplesIterator<'_>,
                    (Some(input_subject), None) => Box::new(
                        self.eval_path_from(path, input_subject, input_graph_name)
                            .map(move |o| {
                                let mut new_tuple = tuple.clone();
                                put_pattern_value(&object, o?, &mut new_tuple);
                                Ok(new_tuple)
                            }),
                    ),
                    (None, Some(input_object)) => Box::new(
                        self.eval_path_to(path, input_object, input_graph_name)
                            .map(move |s| {
                                let mut new_tuple = tuple.clone();
                                put_pattern_value(&subject, s?, &mut new_tuple);
                                Ok(new_tuple)
                            }),
                    ),
                    (None, None) => {
                        Box::new(self.eval_open_path(path, input_graph_name).map(move |so| {
                            let mut new_tuple = tuple.clone();
                            so.map(move |(s, o)| {
                                put_pattern_value(&subject, s, &mut new_tuple);
                                put_pattern_value(&object, o, &mut new_tuple);
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
                    .eval_plan(&*left, from.clone(), options)
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
                    right_iter: self.eval_plan(&*right, from, options),
                    buffered_results: errors,
                })
            }
            PlanNode::AntiJoin { left, right } => {
                //TODO: dumb implementation
                let right: Vec<_> = self
                    .eval_plan(&*right, from.clone(), options)
                    .filter_map(|result| result.ok())
                    .collect();
                Box::new(AntiJoinIterator {
                    left_iter: self.eval_plan(&*left, from, options),
                    right,
                })
            }
            PlanNode::LeftJoin {
                left,
                right,
                possible_problem_vars,
            } => {
                let problem_vars = bind_variables_in_set(&from, &possible_problem_vars);
                let mut filtered_from = from.clone();
                unbind_variables(&mut filtered_from, &problem_vars);
                let iter = LeftJoinIterator {
                    eval: self,
                    right_plan: &*right,
                    left_iter: self.eval_plan(&*left, filtered_from, options),
                    current_right: Box::new(empty()),
                    options,
                };
                if problem_vars.is_empty() {
                    Box::new(iter)
                } else {
                    Box::new(BadLeftJoinIterator {
                        input: from,
                        iter,
                        problem_vars,
                    })
                }
            }
            PlanNode::Filter { child, expression } => {
                let eval = self;
                Box::new(self.eval_plan(&*child, from, options).filter(move |tuple| {
                    match tuple {
                        Ok(tuple) => eval
                            .eval_expression(&expression, tuple, options)
                            .and_then(|term| eval.to_bool(term))
                            .unwrap_or(false),
                        Err(_) => true,
                    }
                }))
            }
            PlanNode::Union { children } => Box::new(UnionIterator {
                eval: self,
                plans: &children,
                input: from,
                current_iterator: Box::new(empty()),
                current_plan: 0,
                options,
            }),
            PlanNode::Extend {
                child,
                position,
                expression,
            } => {
                let eval = self;
                Box::new(self.eval_plan(&*child, from, options).map(move |tuple| {
                    let mut tuple = tuple?;
                    if let Some(value) = eval.eval_expression(&expression, &tuple, options) {
                        put_value(*position, value, &mut tuple)
                    }
                    Ok(tuple)
                }))
            }
            PlanNode::Sort { child, by } => {
                let mut errors = Vec::default();
                let mut values = self
                    .eval_plan(&*child, from, options)
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
                                match self.cmp_according_to_expression(a, b, &expression, options) {
                                    Ordering::Greater => return Ordering::Greater,
                                    Ordering::Less => return Ordering::Less,
                                    Ordering::Equal => (),
                                }
                            }
                            Comparator::Desc(expression) => {
                                match self.cmp_according_to_expression(a, b, &expression, options) {
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
                Box::new(hash_deduplicate(self.eval_plan(&*child, from, options)))
            }
            PlanNode::Skip { child, count } => Box::new(self.eval_plan(&*child, from, options).skip(*count)),
            PlanNode::Limit { child, count } => {
                Box::new(self.eval_plan(&*child, from, options).take(*count))
            }
            PlanNode::Project { child, mapping } => {
                //TODO: use from somewhere?
                Box::new(
                    self.eval_plan(&*child, vec![None; mapping.len()], options)
                        .map(move |tuple| {
                            let tuple = tuple?;
                            let mut output_tuple = vec![None; from.len()];
                            for (input_key, output_key) in mapping.iter() {
                                if let Some(value) = tuple[*input_key] {
                                    put_value(*output_key, value, &mut output_tuple)
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
                let tuple_size = from.len(); //TODO: not nice
                let mut errors = Vec::default();
                let mut accumulators_for_group =
                    HashMap::<Vec<Option<EncodedTerm>>, Vec<Box<dyn Accumulator>>>::default();
                self.eval_plan(child, from, options)
                    .filter_map(|result| match result {
                        Ok(result) => Some(result),
                        Err(error) => {
                            errors.push(error);
                            None
                        }
                    })
                    .for_each(|tuple| {
                        //TODO avoid copy for key?
                        let key = (0..key_mapping.len())
                            .map(|v| get_tuple_value(v, &tuple))
                            .collect();

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
                                    .and_then(|parameter| self.eval_expression(&parameter, &tuple, options)),
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
                                let mut result = vec![None; tuple_size];
                                for (from_position, to_position) in key_mapping.iter().enumerate() {
                                    if let Some(value) = key[from_position] {
                                        put_value(*to_position, value, &mut result);
                                    }
                                }
                                for (i, accumulator) in accumulators.into_iter().enumerate() {
                                    if let Some(value) = accumulator.state() {
                                        put_value(aggregates[i].1, value, &mut result);
                                    }
                                }
                                Ok(result)
                            },
                        )),
                )
            }
        }
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
            PlanPropertyPath::InversePath(p) => self.eval_path_to(&p, start, graph_name),
            PlanPropertyPath::SequencePath(a, b) => Box::new(
                self.eval_path_from(&a, start, graph_name)
                    .flat_map_ok(move |middle| self.eval_path_from(&b, middle, graph_name)),
            ),
            PlanPropertyPath::AlternativePath(a, b) => Box::new(
                self.eval_path_from(&a, start, graph_name)
                    .chain(self.eval_path_from(&b, start, graph_name)),
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
                once(Ok(start)).chain(self.eval_path_from(&p, start, graph_name)),
            )),
            PlanPropertyPath::NegatedPropertySet(ps) => Box::new(
                self.dataset
                    .quads_for_pattern(Some(start), None, None, Some(graph_name))
                    .filter(move |t| match t {
                        Ok(t) => !ps.contains(&t.predicate),
                        Err(_) => true,
                    })
                    .map(|t| Ok(t?.object)),
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
            PlanPropertyPath::InversePath(p) => self.eval_path_from(&p, end, graph_name),
            PlanPropertyPath::SequencePath(a, b) => Box::new(
                self.eval_path_to(&b, end, graph_name)
                    .flat_map_ok(move |middle| self.eval_path_to(&a, middle, graph_name)),
            ),
            PlanPropertyPath::AlternativePath(a, b) => Box::new(
                self.eval_path_to(&a, end, graph_name)
                    .chain(self.eval_path_to(&b, end, graph_name)),
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
                once(Ok(end)).chain(self.eval_path_to(&p, end, graph_name)),
            )),
            PlanPropertyPath::NegatedPropertySet(ps) => Box::new(
                self.dataset
                    .quads_for_pattern(None, None, Some(end), Some(graph_name))
                    .filter(move |t| match t {
                        Ok(t) => !ps.contains(&t.predicate),
                        Err(_) => true,
                    })
                    .map(|t| Ok(t?.subject)),
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
                self.eval_open_path(&p, graph_name)
                    .map(|t| t.map(|(s, o)| (o, s))),
            ),
            PlanPropertyPath::SequencePath(a, b) => Box::new(
                self.eval_open_path(&a, graph_name)
                    .flat_map_ok(move |(start, middle)| {
                        self.eval_path_from(&b, middle, graph_name)
                            .map(move |end| Ok((start, end?)))
                    }),
            ),
            PlanPropertyPath::AlternativePath(a, b) => Box::new(
                self.eval_open_path(&a, graph_name)
                    .chain(self.eval_open_path(&b, graph_name)),
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
                    .chain(self.eval_open_path(&p, graph_name)),
            )),
            PlanPropertyPath::NegatedPropertySet(ps) => Box::new(
                self.dataset
                    .quads_for_pattern(None, None, None, Some(graph_name))
                    .filter(move |t| match t {
                        Ok(t) => !ps.contains(&t.predicate),
                        Err(_) => true,
                    })
                    .map(|t| t.map(|t| (t.subject, t.object))),
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

    fn eval_expression<'b>(
        &'b self,
        expression: &PlanExpression,
        tuple: &[Option<EncodedTerm>],
        options: &QueryOptions<'b>
    ) -> Option<EncodedTerm> {
        match expression {
            PlanExpression::Constant(t) => Some(*t),
            PlanExpression::Variable(v) => get_tuple_value(*v, tuple),
            PlanExpression::Exists(node) => {
                Some(self.eval_plan(node, tuple.to_vec(), options).next().is_some().into())
            }
            PlanExpression::Or(a, b) => {
                match self.eval_expression(a, tuple, options).and_then(|v| self.to_bool(v)) {
                    Some(true) => Some(true.into()),
                    Some(false) => self.eval_expression(b, tuple, options),
                    None => {
                        if Some(true)
                            == self.eval_expression(b, tuple, options).and_then(|v| self.to_bool(v))
                        {
                            Some(true.into())
                        } else {
                            None
                        }
                    }
                }
            }
            PlanExpression::And(a, b) => match self
                .eval_expression(a, tuple, options)
                .and_then(|v| self.to_bool(v))
            {
                Some(true) => self.eval_expression(b, tuple, options),
                Some(false) => Some(false.into()),
                None => {
                    if Some(false) == self.eval_expression(b, tuple, options).and_then(|v| self.to_bool(v)) {
                        Some(false.into())
                    } else {
                        None
                    }
                }
            },
            PlanExpression::Equal(a, b) => {
                let a = self.eval_expression(a, tuple, options)?;
                let b = self.eval_expression(b, tuple, options)?;
                self.equals(a, b).map(|v| v.into())
            }
            PlanExpression::NotEqual(a, b) => {
                let a = self.eval_expression(a, tuple, options)?;
                let b = self.eval_expression(b, tuple, options)?;
                self.equals(a, b).map(|v| (!v).into())
            }
            PlanExpression::Greater(a, b) => Some(
                (self.partial_cmp_literals(
                    self.eval_expression(a, tuple, options)?,
                    self.eval_expression(b, tuple, options)?,
                )? == Ordering::Greater)
                    .into(),
            ),
            PlanExpression::GreaterOrEq(a, b) => Some(
                match self.partial_cmp_literals(
                    self.eval_expression(a, tuple, options)?,
                    self.eval_expression(b, tuple, options)?,
                )? {
                    Ordering::Greater | Ordering::Equal => true,
                    Ordering::Less => false,
                }
                .into(),
            ),
            PlanExpression::Lower(a, b) => Some(
                (self.partial_cmp_literals(
                    self.eval_expression(a, tuple, options)?,
                    self.eval_expression(b, tuple, options)?,
                )? == Ordering::Less)
                    .into(),
            ),
            PlanExpression::LowerOrEq(a, b) => Some(
                match self.partial_cmp_literals(
                    self.eval_expression(a, tuple, options)?,
                    self.eval_expression(b, tuple, options)?,
                )? {
                    Ordering::Less | Ordering::Equal => true,
                    Ordering::Greater => false,
                }
                .into(),
            ),
            PlanExpression::In(e, l) => {
                let needed = self.eval_expression(e, tuple, options)?;
                let mut error = false;
                for possible in l {
                    if let Some(possible) = self.eval_expression(possible, tuple, options) {
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
            PlanExpression::Add(a, b) => Some(match self.parse_numeric_operands(a, b, tuple, options)? {
                NumericBinaryOperands::Float(v1, v2) => (v1 + v2).into(),
                NumericBinaryOperands::Double(v1, v2) => (v1 + v2).into(),
                NumericBinaryOperands::Integer(v1, v2) => v1.checked_add(v2)?.into(),
                NumericBinaryOperands::Decimal(v1, v2) => v1.checked_add(v2)?.into(),
            }),
            PlanExpression::Sub(a, b) => Some(match self.parse_numeric_operands(a, b, tuple, options)? {
                NumericBinaryOperands::Float(v1, v2) => (v1 - v2).into(),
                NumericBinaryOperands::Double(v1, v2) => (v1 - v2).into(),
                NumericBinaryOperands::Integer(v1, v2) => v1.checked_sub(v2)?.into(),
                NumericBinaryOperands::Decimal(v1, v2) => v1.checked_sub(v2)?.into(),
            }),
            PlanExpression::Mul(a, b) => Some(match self.parse_numeric_operands(a, b, tuple, options)? {
                NumericBinaryOperands::Float(v1, v2) => (v1 * v2).into(),
                NumericBinaryOperands::Double(v1, v2) => (v1 * v2).into(),
                NumericBinaryOperands::Integer(v1, v2) => v1.checked_mul(v2)?.into(),
                NumericBinaryOperands::Decimal(v1, v2) => v1.checked_mul(v2)?.into(),
            }),
            PlanExpression::Div(a, b) => Some(match self.parse_numeric_operands(a, b, tuple, options)? {
                NumericBinaryOperands::Float(v1, v2) => (v1 / v2).into(),
                NumericBinaryOperands::Double(v1, v2) => (v1 / v2).into(),
                NumericBinaryOperands::Integer(v1, v2) => Decimal::from_i128(v1)?
                    .checked_div(Decimal::from_i128(v2)?)?
                    .into(),
                NumericBinaryOperands::Decimal(v1, v2) => v1.checked_div(v2)?.into(),
            }),
            PlanExpression::UnaryPlus(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::FloatLiteral(value) => Some((*value).into()),
                EncodedTerm::DoubleLiteral(value) => Some((*value).into()),
                EncodedTerm::IntegerLiteral(value) => Some((value).into()),
                EncodedTerm::DecimalLiteral(value) => Some((value).into()),
                _ => None,
            },
            PlanExpression::UnaryMinus(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::FloatLiteral(value) => Some((-*value).into()),
                EncodedTerm::DoubleLiteral(value) => Some((-*value).into()),
                EncodedTerm::IntegerLiteral(value) => Some((-value).into()),
                EncodedTerm::DecimalLiteral(value) => Some((-value).into()),
                _ => None,
            },
            PlanExpression::UnaryNot(e) => self
                .to_bool(self.eval_expression(e, tuple, options)?)
                .map(|v| (!v).into()),
            PlanExpression::Str(e) => Some(EncodedTerm::StringLiteral {
                value_id: self.to_string_id(self.eval_expression(e, tuple, options)?)?,
            }),
            PlanExpression::Lang(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::LangStringLiteral { language_id, .. } => {
                    Some(EncodedTerm::StringLiteral {
                        value_id: language_id,
                    })
                }
                e if e.is_literal() => Some(ENCODED_EMPTY_STRING_LITERAL),
                _ => None,
            },
            PlanExpression::LangMatches(language_tag, language_range) => {
                let language_tag =
                    self.to_simple_string(self.eval_expression(language_tag, tuple, options)?)?;
                let language_range =
                    self.to_simple_string(self.eval_expression(language_range, tuple, options)?)?;
                Some(
                    if &*language_range == "*" {
                        !language_tag.is_empty()
                    } else {
                        !ZipLongest::new(language_range.split('-'), language_tag.split('-')).any(
                            |parts| match parts {
                                (Some(range_subtag), Some(language_subtag)) => {
                                    !range_subtag.eq_ignore_ascii_case(language_subtag)
                                }
                                (Some(_), None) => true,
                                (None, _) => false,
                            },
                        )
                    }
                    .into(),
                )
            }
            PlanExpression::Datatype(e) => self.eval_expression(e, tuple, options)?.datatype(),
            PlanExpression::Bound(v) => Some(has_tuple_value(*v, tuple).into()),
            PlanExpression::IRI(e) => {
                let iri_id = match self.eval_expression(e, tuple, options)? {
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
                        self.eval_expression(id, tuple, options)?
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
            PlanExpression::Abs(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::IntegerLiteral(value) => Some(value.checked_abs()?.into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.abs().into()),
                EncodedTerm::FloatLiteral(value) => Some(value.abs().into()),
                EncodedTerm::DoubleLiteral(value) => Some(value.abs().into()),
                _ => None,
            },
            PlanExpression::Ceil(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::IntegerLiteral(value) => Some(value.into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.ceil().into()),
                EncodedTerm::FloatLiteral(value) => Some(value.ceil().into()),
                EncodedTerm::DoubleLiteral(value) => Some(value.ceil().into()),
                _ => None,
            },
            PlanExpression::Floor(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::IntegerLiteral(value) => Some(value.into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.floor().into()),
                EncodedTerm::FloatLiteral(value) => Some(value.floor().into()),
                EncodedTerm::DoubleLiteral(value) => Some(value.floor().into()),
                _ => None,
            },
            PlanExpression::Round(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::IntegerLiteral(value) => Some(value.into()),
                EncodedTerm::DecimalLiteral(value) => Some(
                    value
                        .round_dp_with_strategy(0, RoundingStrategy::RoundHalfUp)
                        .into(),
                ),
                EncodedTerm::FloatLiteral(value) => Some(value.round().into()),
                EncodedTerm::DoubleLiteral(value) => Some(value.round().into()),
                _ => None,
            },
            PlanExpression::Concat(l) => {
                let mut result = String::default();
                let mut language = None;
                for e in l {
                    let (value, e_language) =
                        self.to_string_and_language(self.eval_expression(e, tuple, options)?)?;
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
                    self.to_string_and_language(self.eval_expression(source, tuple, options)?)?;

                let starting_location: usize = if let EncodedTerm::IntegerLiteral(v) =
                    self.eval_expression(starting_loc, tuple, options)?
                {
                    v.try_into().ok()?
                } else {
                    return None;
                };
                let length: Option<usize> = if let Some(length) = length {
                    if let EncodedTerm::IntegerLiteral(v) = self.eval_expression(length, tuple, options)? {
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
                    .to_string(self.eval_expression(arg, tuple, options)?)?
                    .chars()
                    .count() as i128)
                    .into(),
            ),
            PlanExpression::Replace(arg, pattern, replacement, flags) => {
                let regex = self.compile_pattern(
                    self.eval_expression(pattern, tuple, options)?,
                    if let Some(flags) = flags {
                        Some(self.eval_expression(flags, tuple, options)?)
                    } else {
                        None
                    },
                )?;
                let (text, language) =
                    self.to_string_and_language(self.eval_expression(arg, tuple, options)?)?;
                let replacement =
                    self.to_simple_string(self.eval_expression(replacement, tuple, options)?)?;
                self.build_plain_literal(&regex.replace_all(&text, &replacement as &str), language)
            }
            PlanExpression::UCase(e) => {
                let (value, language) =
                    self.to_string_and_language(self.eval_expression(e, tuple, options)?)?;
                self.build_plain_literal(&value.to_uppercase(), language)
            }
            PlanExpression::LCase(e) => {
                let (value, language) =
                    self.to_string_and_language(self.eval_expression(e, tuple, options)?)?;
                self.build_plain_literal(&value.to_lowercase(), language)
            }
            PlanExpression::StrStarts(arg1, arg2) => {
                let (arg1, arg2, _) = self.to_argument_compatible_strings(
                    self.eval_expression(arg1, tuple, options)?,
                    self.eval_expression(arg2, tuple, options)?,
                )?;
                Some((&arg1).starts_with(&arg2 as &str).into())
            }
            PlanExpression::EncodeForURI(ltrl) => {
                let ltlr = self.to_string(self.eval_expression(ltrl, tuple, options)?)?;
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
                    self.eval_expression(arg1, tuple, options)?,
                    self.eval_expression(arg2, tuple, options)?,
                )?;
                Some((&arg1).ends_with(&arg2 as &str).into())
            }
            PlanExpression::Contains(arg1, arg2) => {
                let (arg1, arg2, _) = self.to_argument_compatible_strings(
                    self.eval_expression(arg1, tuple, options)?,
                    self.eval_expression(arg2, tuple, options)?,
                )?;
                Some((&arg1).contains(&arg2 as &str).into())
            }
            PlanExpression::StrBefore(arg1, arg2) => {
                let (arg1, arg2, language) = self.to_argument_compatible_strings(
                    self.eval_expression(arg1, tuple, options)?,
                    self.eval_expression(arg2, tuple, options)?,
                )?;
                if let Some(position) = (&arg1).find(&arg2 as &str) {
                    self.build_plain_literal(&arg1[..position], language)
                } else {
                    Some(ENCODED_EMPTY_STRING_LITERAL)
                }
            }
            PlanExpression::StrAfter(arg1, arg2) => {
                let (arg1, arg2, language) = self.to_argument_compatible_strings(
                    self.eval_expression(arg1, tuple, options)?,
                    self.eval_expression(arg2, tuple, options)?,
                )?;
                if let Some(position) = (&arg1).find(&arg2 as &str) {
                    self.build_plain_literal(&arg1[position + arg2.len()..], language)
                } else {
                    Some(ENCODED_EMPTY_STRING_LITERAL)
                }
            }
            PlanExpression::Year(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::DateLiteral(date) => Some(date.year().into()),
                EncodedTerm::NaiveDateLiteral(date) => Some(date.year().into()),
                EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.year().into()),
                EncodedTerm::NaiveDateTimeLiteral(date_time) => Some(date_time.year().into()),
                _ => None,
            },
            PlanExpression::Month(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::DateLiteral(date) => Some(date.year().into()),
                EncodedTerm::NaiveDateLiteral(date) => Some(date.month().into()),
                EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.month().into()),
                EncodedTerm::NaiveDateTimeLiteral(date_time) => Some(date_time.month().into()),
                _ => None,
            },
            PlanExpression::Day(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::DateLiteral(date) => Some(date.year().into()),
                EncodedTerm::NaiveDateLiteral(date) => Some(date.day().into()),
                EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.day().into()),
                EncodedTerm::NaiveDateTimeLiteral(date_time) => Some(date_time.day().into()),
                _ => None,
            },
            PlanExpression::Hours(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::NaiveTimeLiteral(time) => Some(time.hour().into()),
                EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.hour().into()),
                EncodedTerm::NaiveDateTimeLiteral(date_time) => Some(date_time.hour().into()),
                _ => None,
            },
            PlanExpression::Minutes(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::NaiveTimeLiteral(time) => Some(time.minute().into()),
                EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.minute().into()),
                EncodedTerm::NaiveDateTimeLiteral(date_time) => Some(date_time.minute().into()),
                _ => None,
            },
            PlanExpression::Seconds(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::NaiveTimeLiteral(time) => Some(
                    (Decimal::new(time.nanosecond().into(), 9) + Decimal::from(time.second()))
                        .into(),
                ),
                EncodedTerm::DateTimeLiteral(date_time) => Some(
                    (Decimal::new(date_time.nanosecond().into(), 9)
                        + Decimal::from(date_time.second()))
                    .into(),
                ),
                EncodedTerm::NaiveDateTimeLiteral(date_time) => Some(
                    (Decimal::new(date_time.nanosecond().into(), 9)
                        + Decimal::from(date_time.second()))
                    .into(),
                ),
                _ => None,
            },
            PlanExpression::Timezone(e) => {
                let timezone = match self.eval_expression(e, tuple, options)? {
                    EncodedTerm::DateLiteral(date) => date.timezone(),
                    EncodedTerm::DateTimeLiteral(date_time) => date_time.timezone(),
                    _ => return None,
                };
                let mut result = String::with_capacity(9);
                let mut shift = timezone.local_minus_utc();
                if shift < 0 {
                    write!(&mut result, "-").ok()?;
                    shift = -shift
                };
                write!(&mut result, "PT").ok()?;

                let hours = shift / 3600;
                if hours > 0 {
                    write!(&mut result, "{}H", hours).ok()?;
                }

                let minutes = (shift / 60) % 60;
                if minutes > 0 {
                    write!(&mut result, "{}M", minutes).ok()?;
                }

                let seconds = shift % 60;
                if seconds > 0 || shift == 0 {
                    write!(&mut result, "{}S", seconds).ok()?;
                }
                Some(EncodedTerm::TypedLiteral {
                    value_id: self.build_string_id(&result)?,
                    datatype_id: self
                        .build_string_id("http://www.w3.org/2001/XMLSchema#dayTimeDuration")?,
                })
            }
            PlanExpression::Tz(e) => {
                let timezone = match self.eval_expression(e, tuple, options)? {
                    EncodedTerm::DateLiteral(date) => Some(date.timezone()),
                    EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.timezone()),
                    EncodedTerm::NaiveDateLiteral(_)
                    | EncodedTerm::NaiveTimeLiteral(_)
                    | EncodedTerm::NaiveDateTimeLiteral(_) => None,
                    _ => return None,
                };
                Some(if let Some(timezone) = timezone {
                    EncodedTerm::StringLiteral {
                        value_id: if timezone.local_minus_utc() == 0 {
                            self.build_string_id("Z")?
                        } else {
                            self.build_string_id(&timezone.to_string())?
                        },
                    }
                } else {
                    ENCODED_EMPTY_STRING_LITERAL
                })
            }
            PlanExpression::Now => Some(self.now.into()),
            PlanExpression::UUID => self.build_named_node(
                Uuid::new_v4()
                    .to_urn()
                    .encode_lower(&mut Uuid::encode_buffer()),
            ),
            PlanExpression::StrUUID => self.build_string_literal(
                Uuid::new_v4()
                    .to_hyphenated()
                    .encode_lower(&mut Uuid::encode_buffer()),
            ),
            PlanExpression::MD5(arg) => self.hash::<Md5>(arg, tuple, options),
            PlanExpression::SHA1(arg) => self.hash::<Sha1>(arg, tuple, options),
            PlanExpression::SHA256(arg) => self.hash::<Sha256>(arg, tuple, options),
            PlanExpression::SHA384(arg) => self.hash::<Sha384>(arg, tuple, options),
            PlanExpression::SHA512(arg) => self.hash::<Sha512>(arg, tuple, options),
            PlanExpression::Coalesce(l) => {
                for e in l {
                    if let Some(result) = self.eval_expression(e, tuple, options) {
                        return Some(result);
                    }
                }
                None
            }
            PlanExpression::If(a, b, c) => {
                if self.to_bool(self.eval_expression(a, tuple, options)?)? {
                    self.eval_expression(b, tuple, options)
                } else {
                    self.eval_expression(c, tuple, options)
                }
            }
            PlanExpression::StrLang(lexical_form, lang_tag) => {
                Some(EncodedTerm::LangStringLiteral {
                    value_id: self
                        .to_simple_string_id(self.eval_expression(lexical_form, tuple, options)?)?,
                    language_id: self
                        .to_simple_string_id(self.eval_expression(lang_tag, tuple, options)?)?,
                })
            }
            PlanExpression::StrDT(lexical_form, datatype) => {
                let value = self.to_simple_string(self.eval_expression(lexical_form, tuple, options)?)?;
                let datatype = if let EncodedTerm::NamedNode { iri_id } =
                    self.eval_expression(datatype, tuple, options)?
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
                Some((self.eval_expression(a, tuple, options)? == self.eval_expression(b, tuple, options)?).into())
            }
            PlanExpression::IsIRI(e) => {
                Some(self.eval_expression(e, tuple, options)?.is_named_node().into())
            }
            PlanExpression::IsBlank(e) => {
                Some(self.eval_expression(e, tuple, options)?.is_blank_node().into())
            }
            PlanExpression::IsLiteral(e) => {
                Some(self.eval_expression(e, tuple, options)?.is_literal().into())
            }
            PlanExpression::IsNumeric(e) => Some(
                match self.eval_expression(e, tuple, options)? {
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
                    self.eval_expression(pattern, tuple, options)?,
                    if let Some(flags) = flags {
                        Some(self.eval_expression(flags, tuple, options)?)
                    } else {
                        None
                    },
                )?;
                let text = self.to_string(self.eval_expression(text, tuple, options)?)?;
                Some(regex.is_match(&text).into())
            }
            PlanExpression::BooleanCast(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::BooleanLiteral(value) => Some(value.into()),
                EncodedTerm::StringLiteral { value_id } => {
                    parse_boolean_str(&*self.dataset.get_str(value_id).ok()??)
                }
                _ => None,
            },
            PlanExpression::DoubleCast(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::FloatLiteral(value) => Some(value.to_f64()?.into()),
                EncodedTerm::DoubleLiteral(value) => Some(value.to_f64()?.into()),
                EncodedTerm::IntegerLiteral(value) => Some(value.to_f64()?.into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.to_f64()?.into()),
                EncodedTerm::BooleanLiteral(value) => {
                    Some(if value { 1. as f64 } else { 0. }.into())
                }
                EncodedTerm::StringLiteral { value_id } => {
                    parse_double_str(&*self.dataset.get_str(value_id).ok()??)
                }
                _ => None,
            },
            PlanExpression::FloatCast(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::FloatLiteral(value) => Some(value.to_f32()?.into()),
                EncodedTerm::DoubleLiteral(value) => Some(value.to_f32()?.into()),
                EncodedTerm::IntegerLiteral(value) => Some(value.to_f32()?.into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.to_f32()?.into()),
                EncodedTerm::BooleanLiteral(value) => {
                    Some(if value { 1. as f32 } else { 0. }.into())
                }
                EncodedTerm::StringLiteral { value_id } => {
                    parse_float_str(&*self.dataset.get_str(value_id).ok()??)
                }
                _ => None,
            },
            PlanExpression::IntegerCast(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::FloatLiteral(value) => Some(value.to_i128()?.into()),
                EncodedTerm::DoubleLiteral(value) => Some(value.to_i128()?.into()),
                EncodedTerm::IntegerLiteral(value) => Some(value.to_i128()?.into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.to_i128()?.into()),
                EncodedTerm::BooleanLiteral(value) => Some(if value { 1 } else { 0 }.into()),
                EncodedTerm::StringLiteral { value_id } => {
                    parse_integer_str(&*self.dataset.get_str(value_id).ok()??)
                }
                _ => None,
            },
            PlanExpression::DecimalCast(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::FloatLiteral(value) => Some(Decimal::from_f32(*value)?.into()),
                EncodedTerm::DoubleLiteral(value) => Some(Decimal::from_f64(*value)?.into()),
                EncodedTerm::IntegerLiteral(value) => Some(Decimal::from_i128(value)?.into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.into()),
                EncodedTerm::BooleanLiteral(value) => Some(
                    if value {
                        Decimal::one()
                    } else {
                        Decimal::zero()
                    }
                    .into(),
                ),
                EncodedTerm::StringLiteral { value_id } => {
                    parse_decimal_str(&*self.dataset.get_str(value_id).ok()??)
                }
                _ => None,
            },
            PlanExpression::DateCast(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::DateLiteral(value) => Some(value.into()),
                EncodedTerm::NaiveDateLiteral(value) => Some(value.into()),
                EncodedTerm::DateTimeLiteral(value) => Some(value.date().into()),
                EncodedTerm::NaiveDateTimeLiteral(value) => Some(value.date().into()),
                EncodedTerm::StringLiteral { value_id } => {
                    parse_date_str(&*self.dataset.get_str(value_id).ok()??)
                }
                _ => None,
            },
            PlanExpression::TimeCast(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::NaiveTimeLiteral(value) => Some(value.into()),
                EncodedTerm::DateTimeLiteral(value) => Some(value.time().into()),
                EncodedTerm::NaiveDateTimeLiteral(value) => Some(value.time().into()),
                EncodedTerm::StringLiteral { value_id } => {
                    parse_time_str(&*self.dataset.get_str(value_id).ok()??)
                }
                _ => None,
            },
            PlanExpression::DateTimeCast(e) => match self.eval_expression(e, tuple, options)? {
                EncodedTerm::DateTimeLiteral(value) => Some(value.into()),
                EncodedTerm::NaiveDateTimeLiteral(value) => Some(value.into()),
                EncodedTerm::StringLiteral { value_id } => {
                    parse_date_time_str(&*self.dataset.get_str(value_id).ok()??)
                }
                _ => None,
            },
            PlanExpression::StringCast(e) => Some(EncodedTerm::StringLiteral {
                value_id: self.to_string_id(self.eval_expression(e, tuple, options)?)?,
            }),
        }
    }

    fn to_bool(&self, term: EncodedTerm) -> Option<bool> {
        match term {
            EncodedTerm::BooleanLiteral(value) => Some(value),
            EncodedTerm::StringLiteral { .. } => Some(term != ENCODED_EMPTY_STRING_LITERAL),
            EncodedTerm::FloatLiteral(value) => Some(!value.is_zero()),
            EncodedTerm::DoubleLiteral(value) => Some(!value.is_zero()),
            EncodedTerm::IntegerLiteral(value) => Some(!value.is_zero()),
            EncodedTerm::DecimalLiteral(value) => Some(!value.is_zero()),
            _ => None,
        }
    }

    fn to_string_id(&self, term: EncodedTerm) -> Option<u128> {
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
            EncodedTerm::NaiveDateLiteral(value) => self.build_string_id(&value.to_string()),
            EncodedTerm::NaiveTimeLiteral(value) => self.build_string_id(&value.to_string()),
            EncodedTerm::DateTimeLiteral(value) => self.build_string_id(&value.to_string()),
            EncodedTerm::NaiveDateTimeLiteral(value) => self.build_string_id(&value.to_string()),
        }
    }

    fn to_simple_string(
        &self,
        term: EncodedTerm,
    ) -> Option<<DatasetView<S> as StrLookup>::StrType> {
        if let EncodedTerm::StringLiteral { value_id } = term {
            self.dataset.get_str(value_id).ok()?
        } else {
            None
        }
    }

    fn to_simple_string_id(&self, term: EncodedTerm) -> Option<u128> {
        if let EncodedTerm::StringLiteral { value_id } = term {
            Some(value_id)
        } else {
            None
        }
    }

    fn to_string(&self, term: EncodedTerm) -> Option<<DatasetView<S> as StrLookup>::StrType> {
        match term {
            EncodedTerm::StringLiteral { value_id }
            | EncodedTerm::LangStringLiteral { value_id, .. } => {
                self.dataset.get_str(value_id).ok()?
            }
            _ => None,
        }
    }

    fn to_string_and_language(
        &self,
        term: EncodedTerm,
    ) -> Option<(<DatasetView<S> as StrLookup>::StrType, Option<u128>)> {
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

    fn build_lang_string_literal(&self, value: &str, language_id: u128) -> Option<EncodedTerm> {
        Some(EncodedTerm::LangStringLiteral {
            value_id: self.build_string_id(value)?,
            language_id,
        })
    }

    fn build_plain_literal(&self, value: &str, language: Option<u128>) -> Option<EncodedTerm> {
        if let Some(language_id) = language {
            self.build_lang_string_literal(value, language_id)
        } else {
            self.build_string_literal(value)
        }
    }

    fn build_string_id(&self, value: &str) -> Option<u128> {
        let value_id = get_str_id(value);
        self.dataset.encoder().insert_str(value_id, value).ok()?;
        Some(value_id)
    }

    fn to_argument_compatible_strings(
        &self,
        arg1: EncodedTerm,
        arg2: EncodedTerm,
    ) -> Option<(
        <DatasetView<S> as StrLookup>::StrType,
        <DatasetView<S> as StrLookup>::StrType,
        Option<u128>,
    )> {
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
        tuple: &[Option<EncodedTerm>],
        options: &QueryOptions<'b>
    ) -> Option<NumericBinaryOperands> {
        NumericBinaryOperands::new(
            self.eval_expression(&e1, tuple, options)?,
            self.eval_expression(&e2, tuple, options)?,
        )
    }

    fn decode_bindings<'b>(
        &'b self,
        iter: EncodedTuplesIterator<'b>,
        variables: Vec<Variable>,
    ) -> BindingsIterator<'b>
    where
        'a: 'b,
    {
        let eval = self;
        let tuple_size = variables.len();
        BindingsIterator::new(
            variables,
            Box::new(iter.map(move |values| {
                let mut result = vec![None; tuple_size];
                for (i, value) in values?.into_iter().enumerate() {
                    if let Some(term) = value {
                        result[i] = Some(eval.dataset.decode_term(term)?)
                    }
                }
                Ok(result)
            })),
        )
    }

    fn encode_bindings<'b>(
        &'b self,
        variables: &'b [Variable],
        iter: BindingsIterator<'b>,
    ) -> EncodedTuplesIterator<'b>
    where
        'a: 'b,
    {
        let mut encoder = self.dataset.encoder();
        let (binding_variables, iter) = BindingsIterator::destruct(iter);
        let mut combined_variables = variables.clone().to_vec();
        for v in binding_variables.clone() {
            if !combined_variables.contains(&v) {
                combined_variables.resize(combined_variables.len() + 1, v);
            }
        }

        println!("binding_variables: {:?}", binding_variables.clone());
        println!("variables: {:?}", variables.clone());
        println!("combined_variables: {:?}", combined_variables.clone());
        println!("\n\n");
        Box::new(iter.map(move |terms| {
            let mut encoded_terms = vec![None; combined_variables.len()];
            for (i, term_option) in terms?.into_iter().enumerate() {
                match term_option {
                    None => (),
                    Some(term) => {
                        if let Ok(encoded) = encoder.encode_term(&term) {
                            let variable = binding_variables[i].clone();
                            put_variable_value(&variable, &combined_variables, encoded, &mut encoded_terms)
                        }
                    }
                }
            }
            Ok(encoded_terms)
        }))
    }


    #[allow(clippy::float_cmp)]
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
                EncodedTerm::DoubleLiteral(b) => Some(a.to_f64()? == *b),
                EncodedTerm::IntegerLiteral(b) => Some(*a == b.to_f32()?),
                EncodedTerm::DecimalLiteral(b) => Some(*a == b.to_f32()?),
                EncodedTerm::TypedLiteral { .. } => None,
                _ => Some(false),
            },
            EncodedTerm::DoubleLiteral(a) => match b {
                EncodedTerm::FloatLiteral(b) => Some(*a == b.to_f64()?),
                EncodedTerm::DoubleLiteral(b) => Some(a == b),
                EncodedTerm::IntegerLiteral(b) => Some(*a == b.to_f64()?),
                EncodedTerm::DecimalLiteral(b) => Some(*a == b.to_f64()?),
                EncodedTerm::TypedLiteral { .. } => None,
                _ => Some(false),
            },
            EncodedTerm::IntegerLiteral(a) => match b {
                EncodedTerm::FloatLiteral(b) => Some(a.to_f32()? == *b),
                EncodedTerm::DoubleLiteral(b) => Some(a.to_f64()? == *b),
                EncodedTerm::IntegerLiteral(b) => Some(a == b),
                EncodedTerm::DecimalLiteral(b) => Some(Decimal::from_i128(a)? == b),
                EncodedTerm::TypedLiteral { .. } => None,
                _ => Some(false),
            },
            EncodedTerm::DecimalLiteral(a) => match b {
                EncodedTerm::FloatLiteral(b) => Some(a.to_f32()? == *b),
                EncodedTerm::DoubleLiteral(b) => Some(a.to_f64()? == *b),
                EncodedTerm::IntegerLiteral(b) => Some(a == Decimal::from_i128(b)?),
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
                EncodedTerm::NaiveDateLiteral(b) => {
                    if a.naive_utc() == b {
                        None
                    } else {
                        Some(false)
                    }
                }
                EncodedTerm::TypedLiteral { .. } => None,
                _ => Some(false),
            },
            EncodedTerm::NaiveDateLiteral(a) => match b {
                EncodedTerm::NaiveDateLiteral(b) => Some(a == b),
                EncodedTerm::DateLiteral(b) => {
                    if a == b.naive_utc() {
                        None
                    } else {
                        Some(false)
                    }
                }
                EncodedTerm::TypedLiteral { .. } => None,
                _ => Some(false),
            },
            EncodedTerm::NaiveTimeLiteral(a) => match b {
                EncodedTerm::NaiveTimeLiteral(b) => Some(a == b),
                EncodedTerm::TypedLiteral { .. } => None,
                _ => Some(false),
            },
            EncodedTerm::DateTimeLiteral(a) => match b {
                EncodedTerm::DateTimeLiteral(b) => Some(a == b),
                EncodedTerm::NaiveDateTimeLiteral(b) => {
                    if a.naive_utc() == b {
                        None
                    } else {
                        Some(false)
                    }
                }
                EncodedTerm::TypedLiteral { .. } => None,
                _ => Some(false),
            },
            EncodedTerm::NaiveDateTimeLiteral(a) => match b {
                EncodedTerm::NaiveDateTimeLiteral(b) => Some(a == b),
                EncodedTerm::DateTimeLiteral(b) => {
                    if a == b.naive_utc() {
                        None
                    } else {
                        Some(false)
                    }
                }
                EncodedTerm::TypedLiteral { .. } => None,
                _ => Some(false),
            },
        }
    }

    fn cmp_according_to_expression<'b>(
        &'b self,
        tuple_a: &[Option<EncodedTerm>],
        tuple_b: &[Option<EncodedTerm>],
        expression: &PlanExpression,
        options: &QueryOptions<'b>
    ) -> Ordering {
        self.cmp_terms(
            self.eval_expression(expression, tuple_a, options),
            self.eval_expression(expression, tuple_b, options),
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
                EncodedTerm::FloatLiteral(b) => (*a).partial_cmp(&*b),
                EncodedTerm::DoubleLiteral(b) => a.to_f64()?.partial_cmp(&*b),
                EncodedTerm::IntegerLiteral(b) => (*a).partial_cmp(&b.to_f32()?),
                EncodedTerm::DecimalLiteral(b) => (*a).partial_cmp(&b.to_f32()?),
                _ => None,
            },
            EncodedTerm::DoubleLiteral(a) => match b {
                EncodedTerm::FloatLiteral(b) => (*a).partial_cmp(&b.to_f64()?),
                EncodedTerm::DoubleLiteral(b) => (*a).partial_cmp(&*b),
                EncodedTerm::IntegerLiteral(b) => (*a).partial_cmp(&b.to_f64()?),
                EncodedTerm::DecimalLiteral(b) => (*a).partial_cmp(&b.to_f64()?),
                _ => None,
            },
            EncodedTerm::IntegerLiteral(a) => match b {
                EncodedTerm::FloatLiteral(b) => a.to_f32()?.partial_cmp(&*b),
                EncodedTerm::DoubleLiteral(b) => a.to_f64()?.partial_cmp(&*b),
                EncodedTerm::IntegerLiteral(b) => a.partial_cmp(&b),
                EncodedTerm::DecimalLiteral(b) => Decimal::from_i128(a)?.partial_cmp(&b),
                _ => None,
            },
            EncodedTerm::DecimalLiteral(a) => match b {
                EncodedTerm::FloatLiteral(b) => a.to_f32()?.partial_cmp(&*b),
                EncodedTerm::DoubleLiteral(b) => a.to_f64()?.partial_cmp(&*b),
                EncodedTerm::IntegerLiteral(b) => a.partial_cmp(&Decimal::from_i128(b)?),
                EncodedTerm::DecimalLiteral(b) => a.partial_cmp(&b),
                _ => None,
            },
            EncodedTerm::DateLiteral(a) => match b {
                EncodedTerm::DateLiteral(ref b) => a.partial_cmp(b),
                EncodedTerm::NaiveDateLiteral(ref b) => a.naive_utc().partial_cmp(b), //TODO: check edges
                _ => None,
            },
            EncodedTerm::NaiveDateLiteral(a) => match b {
                EncodedTerm::NaiveDateLiteral(ref b) => a.partial_cmp(b),
                EncodedTerm::DateLiteral(ref b) => a.partial_cmp(&b.naive_utc()), //TODO: check edges
                _ => None,
            },
            EncodedTerm::NaiveTimeLiteral(a) => {
                if let EncodedTerm::NaiveTimeLiteral(ref b) = b {
                    a.partial_cmp(b)
                } else {
                    None
                }
            }
            EncodedTerm::DateTimeLiteral(a) => match b {
                EncodedTerm::DateTimeLiteral(ref b) => a.partial_cmp(b),
                EncodedTerm::NaiveDateTimeLiteral(ref b) => a.naive_utc().partial_cmp(b), //TODO: check edges
                _ => None,
            },
            EncodedTerm::NaiveDateTimeLiteral(a) => match b {
                EncodedTerm::NaiveDateTimeLiteral(ref b) => a.partial_cmp(b),
                EncodedTerm::DateTimeLiteral(ref b) => a.partial_cmp(&b.naive_utc()), //TODO: check edges
                _ => None,
            },
            _ => None,
        }
    }

    fn compare_str_ids(&self, a: u128, b: u128) -> Option<Ordering> {
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
        tuple: &[Option<EncodedTerm>],
        options: &QueryOptions<'b>
    ) -> Option<EncodedTerm> {
        let input = self.to_simple_string(self.eval_expression(arg, tuple, options)?)?;
        let hash = hex::encode(H::new().chain(&input as &str).result());
        self.build_string_literal(&hash)
    }
}

pub enum StringOrStoreString<S: Deref<Target = str> + ToString + Into<String>> {
    String(String),
    Store(S),
}

impl<S: Deref<Target = str> + ToString + Into<String>> Deref for StringOrStoreString<S> {
    type Target = str;

    fn deref(&self) -> &str {
        match self {
            StringOrStoreString::String(s) => &*s,
            StringOrStoreString::Store(s) => &*s,
        }
    }
}

impl<S: Deref<Target = str> + ToString + Into<String>> ToString for StringOrStoreString<S> {
    fn to_string(&self) -> String {
        match self {
            StringOrStoreString::String(s) => s.to_string(),
            StringOrStoreString::Store(s) => s.to_string(),
        }
    }
}

impl<S: Deref<Target = str> + ToString + Into<String>> From<StringOrStoreString<S>> for String {
    fn from(string: StringOrStoreString<S>) -> Self {
        match string {
            StringOrStoreString::String(s) => s,
            StringOrStoreString::Store(s) => s.into(),
        }
    }
}

enum NumericBinaryOperands {
    Float(f32, f32),
    Double(f64, f64),
    Integer(i128, i128),
    Decimal(Decimal, Decimal),
}

impl NumericBinaryOperands {
    fn new(a: EncodedTerm, b: EncodedTerm) -> Option<Self> {
        match (a, b) {
            (EncodedTerm::FloatLiteral(v1), EncodedTerm::FloatLiteral(v2)) => {
                Some(NumericBinaryOperands::Float(*v1, v2.to_f32()?))
            }
            (EncodedTerm::FloatLiteral(v1), EncodedTerm::DoubleLiteral(v2)) => {
                Some(NumericBinaryOperands::Double(v1.to_f64()?, *v2))
            }
            (EncodedTerm::FloatLiteral(v1), EncodedTerm::IntegerLiteral(v2)) => {
                Some(NumericBinaryOperands::Float(*v1, v2.to_f32()?))
            }
            (EncodedTerm::FloatLiteral(v1), EncodedTerm::DecimalLiteral(v2)) => {
                Some(NumericBinaryOperands::Float(*v1, v2.to_f32()?))
            }
            (EncodedTerm::DoubleLiteral(v1), EncodedTerm::FloatLiteral(v2)) => {
                Some(NumericBinaryOperands::Double(*v1, v2.to_f64()?))
            }
            (EncodedTerm::DoubleLiteral(v1), EncodedTerm::DoubleLiteral(v2)) => {
                Some(NumericBinaryOperands::Double(*v1, *v2))
            }
            (EncodedTerm::DoubleLiteral(v1), EncodedTerm::IntegerLiteral(v2)) => {
                Some(NumericBinaryOperands::Double(*v1, v2.to_f64()?))
            }
            (EncodedTerm::DoubleLiteral(v1), EncodedTerm::DecimalLiteral(v2)) => {
                Some(NumericBinaryOperands::Double(*v1, v2.to_f64()?))
            }
            (EncodedTerm::IntegerLiteral(v1), EncodedTerm::FloatLiteral(v2)) => {
                Some(NumericBinaryOperands::Float(v1.to_f32()?, *v2))
            }
            (EncodedTerm::IntegerLiteral(v1), EncodedTerm::DoubleLiteral(v2)) => {
                Some(NumericBinaryOperands::Double(v1.to_f64()?, *v2))
            }
            (EncodedTerm::IntegerLiteral(v1), EncodedTerm::IntegerLiteral(v2)) => {
                Some(NumericBinaryOperands::Integer(v1, v2))
            }
            (EncodedTerm::IntegerLiteral(v1), EncodedTerm::DecimalLiteral(v2)) => {
                Some(NumericBinaryOperands::Decimal(Decimal::from_i128(v1)?, v2))
            }
            (EncodedTerm::DecimalLiteral(v1), EncodedTerm::FloatLiteral(v2)) => {
                Some(NumericBinaryOperands::Float(v1.to_f32()?, *v2))
            }
            (EncodedTerm::DecimalLiteral(v1), EncodedTerm::DoubleLiteral(v2)) => {
                Some(NumericBinaryOperands::Double(v1.to_f64()?, *v2))
            }
            (EncodedTerm::DecimalLiteral(v1), EncodedTerm::IntegerLiteral(v2)) => {
                Some(NumericBinaryOperands::Decimal(v1, Decimal::from_i128(v2)?))
            }
            (EncodedTerm::DecimalLiteral(v1), EncodedTerm::DecimalLiteral(v2)) => {
                Some(NumericBinaryOperands::Decimal(v1, v2))
            }
            _ => None,
        }
    }
}

fn get_tuple_value(variable: usize, tuple: &[Option<EncodedTerm>]) -> Option<EncodedTerm> {
    if variable < tuple.len() {
        tuple[variable]
    } else {
        None
    }
}

fn has_tuple_value(variable: usize, tuple: &[Option<EncodedTerm>]) -> bool {
    if variable < tuple.len() {
        tuple[variable].is_some()
    } else {
        false
    }
}

fn get_pattern_value(
    selector: &PatternValue,
    tuple: &[Option<EncodedTerm>],
) -> Option<EncodedTerm> {
    match selector {
        PatternValue::Constant(term) => Some(*term),
        PatternValue::Variable(v) => get_tuple_value(*v, tuple),
    }
}

fn put_pattern_value(selector: &PatternValue, value: EncodedTerm, tuple: &mut EncodedTuple) {
    match selector {
        PatternValue::Constant(_) => (),
        PatternValue::Variable(v) => put_value(*v, value, tuple),
    }
}

fn put_variable_value(selector: &Variable, variables: &[Variable], value: EncodedTerm, tuple: &mut EncodedTuple) {
    for (i, v) in variables.iter().enumerate() {
        if selector == v {
            put_value(i, value, tuple);
            break;
        }
    }
}


fn put_value(position: usize, value: EncodedTerm, tuple: &mut EncodedTuple) {
    if position < tuple.len() {
        tuple[position] = Some(value)
    } else {
        if position > tuple.len() {
            tuple.resize(position, None);
        }
        tuple.push(Some(value))
    }
}

fn bind_variables_in_set(binding: &[Option<EncodedTerm>], set: &[usize]) -> Vec<usize> {
    set.iter()
        .cloned()
        .filter(|key| *key < binding.len() && binding[*key].is_some())
        .collect()
}

fn unbind_variables(binding: &mut [Option<EncodedTerm>], variables: &[usize]) {
    for var in variables {
        if *var < binding.len() {
            binding[*var] = None
        }
    }
}

fn combine_tuples(a: &[Option<EncodedTerm>], b: &[Option<EncodedTerm>]) -> Option<EncodedTuple> {
    if a.len() < b.len() {
        let mut result = b.to_owned();
        for (key, a_value) in a.iter().enumerate() {
            if let Some(a_value) = a_value {
                match b[key] {
                    Some(ref b_value) => {
                        if a_value != b_value {
                            return None;
                        }
                    }
                    None => result[key] = Some(*a_value),
                }
            }
        }
        Some(result)
    } else {
        let mut result = a.to_owned();
        for (key, b_value) in b.iter().enumerate() {
            if let Some(b_value) = b_value {
                match a[key] {
                    Some(ref a_value) => {
                        if a_value != b_value {
                            return None;
                        }
                    }
                    None => result[key] = Some(*b_value),
                }
            }
        }
        Some(result)
    }
}

fn are_tuples_compatible_and_not_disjointed(
    a: &[Option<EncodedTerm>],
    b: &[Option<EncodedTerm>],
) -> bool {
    let mut found_intersection = false;
    for i in 0..min(a.len(), b.len()) {
        if let (Some(a_value), Some(b_value)) = (a[i], b[i]) {
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
                if let Some(result_tuple) = combine_tuples(left_tuple, &right_tuple) {
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
                        are_tuples_compatible_and_not_disjointed(&left_tuple, right_tuple)
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

struct LeftJoinIterator<'a, S: StoreConnection + 'a> {
    eval: &'a SimpleEvaluator<S>,
    right_plan: &'a PlanNode,
    left_iter: EncodedTuplesIterator<'a>,
    current_right: EncodedTuplesIterator<'a>,
    options: &'a QueryOptions<'a>,
}

impl<'a, S: StoreConnection> Iterator for LeftJoinIterator<'a, S> {
    type Item = Result<EncodedTuple>;

    fn next(&mut self) -> Option<Result<EncodedTuple>> {
        if let Some(tuple) = self.current_right.next() {
            return Some(tuple);
        }
        match self.left_iter.next()? {
            Ok(left_tuple) => {
                self.current_right = self.eval.eval_plan(self.right_plan, left_tuple.clone(), self.options);
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

struct BadLeftJoinIterator<'a, S: StoreConnection> {
    input: EncodedTuple,
    iter: LeftJoinIterator<'a, S>,
    problem_vars: Vec<usize>,
}

impl<'a, S: StoreConnection> Iterator for BadLeftJoinIterator<'a, S> {
    type Item = Result<EncodedTuple>;

    fn next(&mut self) -> Option<Result<EncodedTuple>> {
        loop {
            match self.iter.next()? {
                Ok(mut tuple) => {
                    let mut conflict = false;
                    for problem_var in &self.problem_vars {
                        if let Some(input_value) = self.input[*problem_var] {
                            if let Some(result_value) = get_tuple_value(*problem_var, &tuple) {
                                if input_value != result_value {
                                    conflict = true;
                                    continue; //Binding conflict
                                }
                            } else {
                                put_value(*problem_var, input_value, &mut tuple);
                            }
                        }
                    }
                    if !conflict {
                        return Some(Ok(tuple));
                    }
                }
                Err(error) => return Some(Err(error)),
            }
        }
    }
}

struct UnionIterator<'a, S: StoreConnection + 'a> {
    eval: &'a SimpleEvaluator<S>,
    plans: &'a [PlanNode],
    input: EncodedTuple,
    current_iterator: EncodedTuplesIterator<'a>,
    current_plan: usize,
    options: &'a QueryOptions<'a>,
}

impl<'a, S: StoreConnection> Iterator for UnionIterator<'a, S> {
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
                .eval_plan(&self.plans[self.current_plan], self.input.clone(), self.options);
            self.current_plan += 1;
        }
    }
}

struct ConstructIterator<'a, S: StoreConnection + 'a> {
    eval: &'a SimpleEvaluator<S>,
    iter: EncodedTuplesIterator<'a>,
    template: &'a [TripleTemplate],
    buffered_results: Vec<Result<Triple>>,
    bnodes: Vec<BlankNode>,
}

impl<'a, S: StoreConnection + 'a> Iterator for ConstructIterator<'a, S> {
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
    tuple: &[Option<EncodedTerm>],
    bnodes: &mut Vec<BlankNode>,
) -> Option<EncodedTerm> {
    match selector {
        TripleTemplateValue::Constant(term) => Some(*term),
        TripleTemplateValue::Variable(v) => get_tuple_value(*v, tuple),
        TripleTemplateValue::BlankNode(id) => {
            if *id >= tuple.len() {
                bnodes.resize_with(*id, BlankNode::default)
            }
            tuple[*id]
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

struct DescribeIterator<'a, S: StoreConnection + 'a> {
    eval: &'a SimpleEvaluator<S>,
    iter: EncodedTuplesIterator<'a>,
    quads: Box<dyn Iterator<Item = Result<EncodedQuad>> + 'a>,
}

impl<'a, S: StoreConnection + 'a> Iterator for DescribeIterator<'a, S> {
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
            for subject in tuple {
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
    count: u64,
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
            match NumericBinaryOperands::new(sum, count)? {
                NumericBinaryOperands::Float(v1, v2) => Some((v1 / v2).into()),
                NumericBinaryOperands::Double(v1, v2) => Some((v1 / v2).into()),
                NumericBinaryOperands::Integer(v1, v2) => Decimal::from_i128(v1)?
                    .checked_div(Decimal::from_i128(v2)?)
                    .map(|v| v.into()),
                NumericBinaryOperands::Decimal(v1, v2) => v1.checked_div(v2).map(|v| v.into()),
            }
        }
    }
}

struct MinAccumulator<'a, S: StoreConnection + 'a> {
    eval: &'a SimpleEvaluator<S>,
    min: Option<Option<EncodedTerm>>,
}

impl<'a, S: StoreConnection + 'a> MinAccumulator<'a, S> {
    fn new(eval: &'a SimpleEvaluator<S>) -> Self {
        Self { eval, min: None }
    }
}

impl<'a, S: StoreConnection + 'a> Accumulator for MinAccumulator<'a, S> {
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

struct MaxAccumulator<'a, S: StoreConnection + 'a> {
    eval: &'a SimpleEvaluator<S>,
    max: Option<Option<EncodedTerm>>,
}

impl<'a, S: StoreConnection + 'a> MaxAccumulator<'a, S> {
    fn new(eval: &'a SimpleEvaluator<S>) -> Self {
        Self { eval, max: None }
    }
}

impl<'a, S: StoreConnection + 'a> Accumulator for MaxAccumulator<'a, S> {
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

struct GroupConcatAccumulator<'a, S: StoreConnection + 'a> {
    eval: &'a SimpleEvaluator<S>,
    concat: Option<String>,
    language: Option<Option<u128>>,
    separator: &'a str,
}

impl<'a, S: StoreConnection + 'a> GroupConcatAccumulator<'a, S> {
    fn new(eval: &'a SimpleEvaluator<S>, separator: &'a str) -> Self {
        Self {
            eval,
            concat: Some("".to_owned()),
            language: None,
            separator,
        }
    }
}

impl<'a, S: StoreConnection + 'a> Accumulator for GroupConcatAccumulator<'a, S> {
    fn add(&mut self, element: Option<EncodedTerm>) {
        if let Some(concat) = self.concat.as_mut() {
            let element = if let Some(element) = element {
                self.eval.to_string_and_language(element)
            } else {
                None
            };
            if let Some((value, e_language)) = element {
                if let Some(lang) = self.language {
                    if lang != e_language {
                        self.language = Some(None)
                    }
                    concat.push_str(self.separator);
                } else {
                    self.language = Some(e_language)
                }
                concat.push_str(&value);
            } else {
                self.concat = None;
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
