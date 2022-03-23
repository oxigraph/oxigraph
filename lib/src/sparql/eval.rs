use crate::model::vocab::{rdf, xsd};
use crate::model::{BlankNode, LiteralRef, NamedNodeRef};
use crate::model::{NamedNode, Term, Triple};
use crate::sparql::algebra::{Query, QueryDataset};
use crate::sparql::dataset::DatasetView;
use crate::sparql::error::EvaluationError;
use crate::sparql::model::*;
use crate::sparql::plan::*;
use crate::sparql::service::ServiceHandler;
use crate::storage::numeric_encoder::*;
use crate::storage::small_string::SmallString;
use crate::xsd::*;
use digest::Digest;
use md5::Md5;
use oxilangtag::LanguageTag;
use oxiri::Iri;
use oxrdf::Variable;
use rand::random;
use regex::{Regex, RegexBuilder};
use sha1::Sha1;
use sha2::{Sha256, Sha384, Sha512};
use spargebra::algebra::GraphPattern;
use std::cmp::Ordering;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::iter::Iterator;
use std::iter::{empty, once};
use std::rc::Rc;
use std::str;

const REGEX_SIZE_LIMIT: usize = 1_000_000;

type EncodedTuplesIterator = Box<dyn Iterator<Item = Result<EncodedTuple, EvaluationError>>>;

#[derive(Clone)]
pub struct SimpleEvaluator {
    dataset: Rc<DatasetView>,
    base_iri: Option<Rc<Iri<String>>>,
    now: DateTime,
    service_handler: Rc<dyn ServiceHandler<Error = EvaluationError>>,
    custom_functions: Rc<HashMap<NamedNode, Rc<dyn Fn(&[Term]) -> Option<Term>>>>,
}

impl SimpleEvaluator {
    pub fn new(
        dataset: Rc<DatasetView>,
        base_iri: Option<Rc<Iri<String>>>,
        service_handler: Rc<dyn ServiceHandler<Error = EvaluationError>>,
        custom_functions: Rc<HashMap<NamedNode, Rc<dyn Fn(&[Term]) -> Option<Term>>>>,
    ) -> Self {
        Self {
            dataset,
            base_iri,
            now: DateTime::now().unwrap(),
            service_handler,
            custom_functions,
        }
    }

    pub fn evaluate_select_plan(
        &self,
        plan: &PlanNode,
        variables: Rc<Vec<Variable>>,
    ) -> QueryResults {
        let iter = self.plan_evaluator(plan)(EncodedTuple::with_capacity(variables.len()));
        QueryResults::Solutions(decode_bindings(self.dataset.clone(), iter, variables))
    }

    pub fn evaluate_ask_plan(&self, plan: &PlanNode) -> Result<QueryResults, EvaluationError> {
        let from = EncodedTuple::with_capacity(plan.used_variables().len());
        match self.plan_evaluator(plan)(from).next() {
            Some(Ok(_)) => Ok(QueryResults::Boolean(true)),
            Some(Err(error)) => Err(error),
            None => Ok(QueryResults::Boolean(false)),
        }
    }

    pub fn evaluate_construct_plan(
        &self,
        plan: &PlanNode,
        template: Vec<TripleTemplate>,
    ) -> QueryResults {
        let from = EncodedTuple::with_capacity(plan.used_variables().len());
        QueryResults::Graph(QueryTripleIter {
            iter: Box::new(ConstructIterator {
                eval: self.clone(),
                iter: self.plan_evaluator(plan)(from),
                template,
                buffered_results: Vec::default(),
                bnodes: Vec::default(),
            }),
        })
    }

    pub fn evaluate_describe_plan(&self, plan: &PlanNode) -> QueryResults {
        let from = EncodedTuple::with_capacity(plan.used_variables().len());
        QueryResults::Graph(QueryTripleIter {
            iter: Box::new(DescribeIterator {
                eval: self.clone(),
                iter: self.plan_evaluator(plan)(from),
                quads: Box::new(empty()),
            }),
        })
    }

    pub fn plan_evaluator(
        &self,
        node: &PlanNode,
    ) -> Rc<dyn Fn(EncodedTuple) -> EncodedTuplesIterator> {
        match node {
            PlanNode::StaticBindings { tuples } => {
                let tuples = tuples.clone();
                Rc::new(move |from| {
                    Box::new(
                        tuples
                            .iter()
                            .filter_map(move |t| Some(Ok(t.combine_with(&from)?)))
                            .collect::<Vec<_>>()
                            .into_iter(),
                    )
                })
            }
            PlanNode::Service {
                variables,
                silent,
                service_name,
                graph_pattern,
                ..
            } => {
                let variables = variables.clone();
                let silent = *silent;
                let service_name = service_name.clone();
                let graph_pattern = graph_pattern.clone();
                let eval = self.clone();
                Rc::new(move |from| {
                    match eval.evaluate_service(
                        &service_name,
                        &graph_pattern,
                        variables.clone(),
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
            PlanNode::QuadPattern {
                subject,
                predicate,
                object,
                graph_name,
            } => {
                let subject = subject.clone();
                let predicate = predicate.clone();
                let object = object.clone();
                let graph_name = graph_name.clone();
                let dataset = self.dataset.clone();
                Rc::new(move |from| {
                    let iter = dataset.encoded_quads_for_pattern(
                        get_pattern_value(&subject, &from).as_ref(),
                        get_pattern_value(&predicate, &from).as_ref(),
                        get_pattern_value(&object, &from).as_ref(),
                        get_pattern_value(&graph_name, &from).as_ref(),
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
            PlanNode::PathPattern {
                subject,
                path,
                object,
                graph_name,
            } => {
                let eval = self.clone();
                let subject = subject.clone();
                let path = path.clone();
                let object = object.clone();
                let graph_name = graph_name.clone();
                Rc::new(move |from| {
                    let input_subject = get_pattern_value(&subject, &from);
                    let input_object = get_pattern_value(&object, &from);
                    let input_graph_name =
                        if let Some(graph_name) = get_pattern_value(&graph_name, &from) {
                            graph_name
                        } else {
                            let result: EncodedTuplesIterator =
                            Box::new(once(Err(EvaluationError::msg(
                                "Unknown graph name is not allowed when evaluating property path",
                            ))));
                            return result;
                        };
                    match (input_subject, input_object) {
                        (Some(input_subject), Some(input_object)) => Box::new(
                            eval.eval_path_from(&path, &input_subject, &input_graph_name)
                                .filter_map(move |o| match o {
                                    Ok(o) => {
                                        if o == input_object {
                                            Some(Ok(from.clone()))
                                        } else {
                                            None
                                        }
                                    }
                                    Err(error) => Some(Err(error)),
                                }),
                        ),
                        (Some(input_subject), None) => {
                            let object = object.clone();
                            Box::new(
                                eval.eval_path_from(&path, &input_subject, &input_graph_name)
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
                        (None, Some(input_object)) => {
                            let subject = subject.clone();
                            Box::new(
                                eval.eval_path_to(&path, &input_object, &input_graph_name)
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
                        (None, None) => {
                            let subject = subject.clone();
                            let object = object.clone();
                            Box::new(eval.eval_open_path(&path, &input_graph_name).filter_map(
                                move |so| match so {
                                    Ok((s, o)) => {
                                        let mut new_tuple = from.clone();
                                        put_pattern_value(&subject, s, &mut new_tuple)?;
                                        put_pattern_value(&object, o, &mut new_tuple)?;
                                        Some(Ok(new_tuple))
                                    }
                                    Err(error) => Some(Err(error)),
                                },
                            ))
                        }
                    }
                })
            }
            PlanNode::HashJoin { left, right } => {
                let join_keys: Vec<_> = left
                    .always_bound_variables()
                    .intersection(&right.always_bound_variables())
                    .copied()
                    .collect();
                let left = self.plan_evaluator(left);
                let right = self.plan_evaluator(right);
                if join_keys.is_empty() {
                    // Cartesian product
                    Rc::new(move |from| {
                        let mut errors = Vec::default();
                        let right_values = right(from.clone())
                            .filter_map(|result| match result {
                                Ok(result) => Some(result),
                                Err(error) => {
                                    errors.push(Err(error));
                                    None
                                }
                            })
                            .collect::<Vec<_>>();
                        Box::new(CartesianProductJoinIterator {
                            left_iter: left(from),
                            right: right_values,
                            buffered_results: errors,
                        })
                    })
                } else {
                    // Real hash join
                    Rc::new(move |from| {
                        let mut errors = Vec::default();
                        let mut right_values = EncodedTupleSet::new(join_keys.clone());
                        right_values.extend(right(from.clone()).filter_map(
                            |result| match result {
                                Ok(result) => Some(result),
                                Err(error) => {
                                    errors.push(Err(error));
                                    None
                                }
                            },
                        ));
                        Box::new(HashJoinIterator {
                            left_iter: left(from),
                            right: right_values,
                            buffered_results: errors,
                        })
                    })
                }
            }
            PlanNode::ForLoopJoin { left, right } => {
                let left = self.plan_evaluator(left);
                let right = self.plan_evaluator(right);
                Rc::new(move |from| {
                    let right = right.clone();
                    Box::new(left(from).flat_map(move |t| match t {
                        Ok(t) => right(t),
                        Err(e) => Box::new(once(Err(e))),
                    }))
                })
            }
            PlanNode::AntiJoin { left, right } => {
                let join_keys: Vec<_> = left
                    .always_bound_variables()
                    .intersection(&right.always_bound_variables())
                    .copied()
                    .collect();
                let left = self.plan_evaluator(left);
                let right = self.plan_evaluator(right);
                if join_keys.is_empty() {
                    Rc::new(move |from| {
                        let right: Vec<_> = right(from.clone())
                            .filter_map(std::result::Result::ok)
                            .collect();
                        Box::new(left(from).filter(move |left_tuple| {
                            if let Ok(left_tuple) = left_tuple {
                                !right.iter().any(|right_tuple| {
                                    are_compatible_and_not_disjointed(left_tuple, right_tuple)
                                })
                            } else {
                                true
                            }
                        }))
                    })
                } else {
                    Rc::new(move |from| {
                        let mut right_values = EncodedTupleSet::new(join_keys.clone());
                        right_values
                            .extend(right(from.clone()).filter_map(std::result::Result::ok));
                        Box::new(left(from).filter(move |left_tuple| {
                            if let Ok(left_tuple) = left_tuple {
                                !right_values.get(left_tuple).iter().any(|right_tuple| {
                                    are_compatible_and_not_disjointed(left_tuple, right_tuple)
                                })
                            } else {
                                true
                            }
                        }))
                    })
                }
            }
            PlanNode::LeftJoin {
                left,
                right,
                possible_problem_vars,
            } => {
                let left = self.plan_evaluator(left);
                let right = self.plan_evaluator(right);
                let possible_problem_vars = possible_problem_vars.clone();
                Rc::new(move |from| {
                    if possible_problem_vars.is_empty() {
                        Box::new(LeftJoinIterator {
                            right_evaluator: right.clone(),
                            left_iter: left(from),
                            current_right: Box::new(empty()),
                        })
                    } else {
                        Box::new(BadLeftJoinIterator {
                            right_evaluator: right.clone(),
                            left_iter: left(from),
                            current_left: None,
                            current_right: Box::new(empty()),
                            problem_vars: possible_problem_vars.clone(),
                        })
                    }
                })
            }
            PlanNode::Filter { child, expression } => {
                let child = self.plan_evaluator(child);
                let expression = self.expression_evaluator(expression);
                Rc::new(move |from| {
                    let expression = expression.clone();
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
            PlanNode::Union { children } => {
                let children: Vec<_> = children
                    .iter()
                    .map(|child| self.plan_evaluator(child))
                    .collect();
                Rc::new(move |from| {
                    Box::new(UnionIterator {
                        plans: children.clone(),
                        input: from,
                        current_iterator: Box::new(empty()),
                        current_plan: 0,
                    })
                })
            }
            PlanNode::Extend {
                child,
                position,
                expression,
            } => {
                let child = self.plan_evaluator(child);
                let position = *position;
                let expression = self.expression_evaluator(expression);
                Rc::new(move |from| {
                    let expression = expression.clone();
                    Box::new(child(from).map(move |tuple| {
                        let mut tuple = tuple?;
                        if let Some(value) = expression(&tuple) {
                            tuple.set(position, value);
                        }
                        Ok(tuple)
                    }))
                })
            }
            PlanNode::Sort { child, by } => {
                let child = self.plan_evaluator(child);
                let by: Vec<_> = by
                    .iter()
                    .map(|comp| match comp {
                        Comparator::Asc(expression) => {
                            ComparatorFunction::Asc(self.expression_evaluator(expression))
                        }
                        Comparator::Desc(expression) => {
                            ComparatorFunction::Desc(self.expression_evaluator(expression))
                        }
                    })
                    .collect();
                let dataset = self.dataset.clone();
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
            PlanNode::HashDeduplicate { child } => {
                let child = self.plan_evaluator(child);
                Rc::new(move |from| Box::new(hash_deduplicate(child(from))))
            }
            PlanNode::Reduced { child } => {
                let child = self.plan_evaluator(child);
                Rc::new(move |from| {
                    Box::new(ConsecutiveDeduplication {
                        inner: child(from),
                        current: None,
                    })
                })
            }
            PlanNode::Skip { child, count } => {
                let child = self.plan_evaluator(child);
                let count = *count;
                Rc::new(move |from| Box::new(child(from).skip(count)))
            }
            PlanNode::Limit { child, count } => {
                let child = self.plan_evaluator(child);
                let count = *count;
                Rc::new(move |from| Box::new(child(from).take(count)))
            }
            PlanNode::Project { child, mapping } => {
                let child = self.plan_evaluator(child);
                let mapping = mapping.clone();
                Rc::new(move |from| {
                    let mapping = mapping.clone();
                    // We map forward the "from" values to make sure we join wit them
                    let mut inner_from = EncodedTuple::with_capacity(mapping.len());
                    for (input_key, output_key) in mapping.iter() {
                        if let Some(value) = from.get(*output_key) {
                            inner_from.set(*input_key, value.clone());
                        }
                    }
                    Box::new(child(inner_from).map(move |tuple| {
                        let tuple = tuple?;
                        let mut output_tuple = from.clone();
                        for (input_key, output_key) in mapping.iter() {
                            if let Some(value) = tuple.get(*input_key) {
                                output_tuple.set(*output_key, value.clone());
                            }
                        }
                        Ok(output_tuple)
                    }))
                })
            }
            PlanNode::Aggregate {
                child,
                key_mapping,
                aggregates,
            } => {
                let child = self.plan_evaluator(child);
                let key_mapping = key_mapping.clone();
                let aggregate_input_expressions: Vec<_> = aggregates
                    .iter()
                    .map(|(aggregate, _)| {
                        aggregate
                            .parameter
                            .as_ref()
                            .map(|p| self.expression_evaluator(p))
                    })
                    .collect();
                let accumulator_builders: Vec<_> = aggregates
                    .iter()
                    .map(|(aggregate, _)| {
                        Self::accumulator_builder(
                            &self.dataset,
                            &aggregate.function,
                            aggregate.distinct,
                        )
                    })
                    .collect();
                let accumulator_variables: Vec<_> =
                    aggregates.iter().map(|(_, var)| *var).collect();
                Rc::new(move |from| {
                    let tuple_size = from.capacity(); //TODO: not nice
                    let key_mapping = key_mapping.clone();
                    let mut errors = Vec::default();
                    let mut accumulators_for_group =
                        HashMap::<Vec<Option<EncodedTerm>>, Vec<Box<dyn Accumulator>>>::default();
                    child(from)
                        .filter_map(|result| match result {
                            Ok(result) => Some(result),
                            Err(error) => {
                                errors.push(error);
                                None
                            }
                        })
                        .for_each(|tuple| {
                            //TODO avoid copy for key?
                            let key = key_mapping
                                .iter()
                                .map(|(v, _)| tuple.get(*v).cloned())
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
                    if accumulators_for_group.is_empty() && key_mapping.is_empty() {
                        // There is always a single group if there is no GROUP BY
                        accumulators_for_group.insert(Vec::new(), Vec::new());
                    }
                    let accumulator_variables = accumulator_variables.clone();
                    Box::new(
                        errors
                            .into_iter()
                            .map(Err)
                            .chain(accumulators_for_group.into_iter().map(
                                move |(key, accumulators)| {
                                    let mut result = EncodedTuple::with_capacity(tuple_size);
                                    for (from_position, to_position) in key_mapping.iter() {
                                        if let Some(value) = &key[*from_position] {
                                            result.set(*to_position, value.clone());
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
        service_name: &PatternValue,
        graph_pattern: &GraphPattern,
        variables: Rc<Vec<Variable>>,
        from: &EncodedTuple,
    ) -> Result<EncodedTuplesIterator, EvaluationError> {
        let service_name = get_pattern_value(service_name, from)
            .ok_or_else(|| EvaluationError::msg("The SERVICE name is not bound"))?;
        if let QueryResults::Solutions(iter) = self.service_handler.handle(
            self.dataset.decode_named_node(&service_name)?,
            Query {
                inner: spargebra::Query::Select {
                    dataset: None,
                    pattern: graph_pattern.clone(),
                    base_iri: self.base_iri.as_ref().map(|iri| iri.as_ref().clone()),
                },
                dataset: QueryDataset::new(),
            },
        )? {
            Ok(encode_bindings(self.dataset.clone(), variables, iter))
        } else {
            Err(EvaluationError::msg(
                "The service call has not returned a set of solutions",
            ))
        }
    }

    fn accumulator_builder(
        dataset: &Rc<DatasetView>,
        function: &PlanAggregationFunction,
        distinct: bool,
    ) -> Box<dyn Fn() -> Box<dyn Accumulator>> {
        match function {
            PlanAggregationFunction::Count => {
                if distinct {
                    Box::new(|| Box::new(DistinctAccumulator::new(CountAccumulator::default())))
                } else {
                    Box::new(|| Box::new(CountAccumulator::default()))
                }
            }
            PlanAggregationFunction::Sum => {
                if distinct {
                    Box::new(|| Box::new(DistinctAccumulator::new(SumAccumulator::default())))
                } else {
                    Box::new(|| Box::new(SumAccumulator::default()))
                }
            }
            PlanAggregationFunction::Min => {
                let dataset = dataset.clone();
                Box::new(move || Box::new(MinAccumulator::new(dataset.clone())))
            } // DISTINCT does not make sense with min
            PlanAggregationFunction::Max => {
                let dataset = dataset.clone();
                Box::new(move || Box::new(MaxAccumulator::new(dataset.clone())))
            } // DISTINCT does not make sense with max
            PlanAggregationFunction::Avg => {
                if distinct {
                    Box::new(|| Box::new(DistinctAccumulator::new(AvgAccumulator::default())))
                } else {
                    Box::new(|| Box::new(AvgAccumulator::default()))
                }
            }
            PlanAggregationFunction::Sample => Box::new(|| Box::new(SampleAccumulator::default())), // DISTINCT does not make sense with sample
            PlanAggregationFunction::GroupConcat { separator } => {
                let dataset = dataset.clone();
                let separator = separator.clone();
                if distinct {
                    Box::new(move || {
                        Box::new(DistinctAccumulator::new(GroupConcatAccumulator::new(
                            dataset.clone(),
                            separator.clone(),
                        )))
                    })
                } else {
                    Box::new(move || {
                        Box::new(GroupConcatAccumulator::new(
                            dataset.clone(),
                            separator.clone(),
                        ))
                    })
                }
            }
        }
    }

    fn eval_path_from(
        &self,
        path: &PlanPropertyPath,
        start: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> Box<dyn Iterator<Item = Result<EncodedTerm, EvaluationError>>> {
        match path {
            PlanPropertyPath::Path(p) => Box::new(
                self.dataset
                    .encoded_quads_for_pattern(Some(start), Some(p), None, Some(graph_name))
                    .map(|t| Ok(t?.object)),
            ),
            PlanPropertyPath::Reverse(p) => self.eval_path_to(p, start, graph_name),
            PlanPropertyPath::Sequence(a, b) => {
                let eval = self.clone();
                let b = b.clone();
                let graph_name2 = graph_name.clone();
                Box::new(
                    self.eval_path_from(a, start, graph_name)
                        .flat_map_ok(move |middle| eval.eval_path_from(&b, &middle, &graph_name2)),
                )
            }
            PlanPropertyPath::Alternative(a, b) => Box::new(
                self.eval_path_from(a, start, graph_name)
                    .chain(self.eval_path_from(b, start, graph_name)),
            ),
            PlanPropertyPath::ZeroOrMore(p) => {
                let eval = self.clone();
                let p = p.clone();
                let graph_name2 = graph_name.clone();
                Box::new(transitive_closure(Some(Ok(start.clone())), move |e| {
                    eval.eval_path_from(&p, &e, &graph_name2)
                }))
            }
            PlanPropertyPath::OneOrMore(p) => {
                let eval = self.clone();
                let p = p.clone();
                let graph_name2 = graph_name.clone();
                Box::new(transitive_closure(
                    self.eval_path_from(&p, start, graph_name),
                    move |e| eval.eval_path_from(&p, &e, &graph_name2),
                ))
            }
            PlanPropertyPath::ZeroOrOne(p) => Box::new(hash_deduplicate(
                once(Ok(start.clone())).chain(self.eval_path_from(p, start, graph_name)),
            )),
            PlanPropertyPath::NegatedPropertySet(ps) => {
                let ps = ps.clone();
                Box::new(
                    self.dataset
                        .encoded_quads_for_pattern(Some(start), None, None, Some(graph_name))
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

    fn eval_path_to(
        &self,
        path: &PlanPropertyPath,
        end: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> Box<dyn Iterator<Item = Result<EncodedTerm, EvaluationError>>> {
        match path {
            PlanPropertyPath::Path(p) => Box::new(
                self.dataset
                    .encoded_quads_for_pattern(None, Some(p), Some(end), Some(graph_name))
                    .map(|t| Ok(t?.subject)),
            ),
            PlanPropertyPath::Reverse(p) => self.eval_path_from(p, end, graph_name),
            PlanPropertyPath::Sequence(a, b) => {
                let eval = self.clone();
                let a = a.clone();
                let graph_name2 = graph_name.clone();
                Box::new(
                    self.eval_path_to(b, end, graph_name)
                        .flat_map_ok(move |middle| eval.eval_path_to(&a, &middle, &graph_name2)),
                )
            }
            PlanPropertyPath::Alternative(a, b) => Box::new(
                self.eval_path_to(a, end, graph_name)
                    .chain(self.eval_path_to(b, end, graph_name)),
            ),
            PlanPropertyPath::ZeroOrMore(p) => {
                let eval = self.clone();
                let p = p.clone();
                let graph_name2 = graph_name.clone();
                Box::new(transitive_closure(Some(Ok(end.clone())), move |e| {
                    eval.eval_path_to(&p, &e, &graph_name2)
                }))
            }
            PlanPropertyPath::OneOrMore(p) => {
                let eval = self.clone();
                let p = p.clone();
                let graph_name2 = graph_name.clone();
                Box::new(transitive_closure(
                    self.eval_path_to(&p, end, graph_name),
                    move |e| eval.eval_path_to(&p, &e, &graph_name2),
                ))
            }
            PlanPropertyPath::ZeroOrOne(p) => Box::new(hash_deduplicate(
                once(Ok(end.clone())).chain(self.eval_path_to(p, end, graph_name)),
            )),
            PlanPropertyPath::NegatedPropertySet(ps) => {
                let ps = ps.clone();
                Box::new(
                    self.dataset
                        .encoded_quads_for_pattern(None, None, Some(end), Some(graph_name))
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

    fn eval_open_path(
        &self,
        path: &PlanPropertyPath,
        graph_name: &EncodedTerm,
    ) -> Box<dyn Iterator<Item = Result<(EncodedTerm, EncodedTerm), EvaluationError>>> {
        match path {
            PlanPropertyPath::Path(p) => Box::new(
                self.dataset
                    .encoded_quads_for_pattern(None, Some(p), None, Some(graph_name))
                    .map(|t| t.map(|t| (t.subject, t.object))),
            ),
            PlanPropertyPath::Reverse(p) => Box::new(
                self.eval_open_path(p, graph_name)
                    .map(|t| t.map(|(s, o)| (o, s))),
            ),
            PlanPropertyPath::Sequence(a, b) => {
                let eval = self.clone();
                let b = b.clone();
                let graph_name2 = graph_name.clone();
                Box::new(
                    self.eval_open_path(a, graph_name)
                        .flat_map_ok(move |(start, middle)| {
                            eval.eval_path_from(&b, &middle, &graph_name2)
                                .map(move |end| Ok((start.clone(), end?)))
                        }),
                )
            }
            PlanPropertyPath::Alternative(a, b) => Box::new(
                self.eval_open_path(a, graph_name)
                    .chain(self.eval_open_path(b, graph_name)),
            ),
            PlanPropertyPath::ZeroOrMore(p) => {
                let eval = self.clone();
                let p = p.clone();
                let graph_name2 = graph_name.clone();
                Box::new(transitive_closure(
                    self.get_subject_or_object_identity_pairs(graph_name), //TODO: avoid to inject everything
                    move |(start, middle)| {
                        eval.eval_path_from(&p, &middle, &graph_name2)
                            .map(move |end| Ok((start.clone(), end?)))
                    },
                ))
            }
            PlanPropertyPath::OneOrMore(p) => {
                let eval = self.clone();
                let p = p.clone();
                let graph_name2 = graph_name.clone();
                Box::new(transitive_closure(
                    self.eval_open_path(&p, graph_name),
                    move |(start, middle)| {
                        eval.eval_path_from(&p, &middle, &graph_name2)
                            .map(move |end| Ok((start.clone(), end?)))
                    },
                ))
            }
            PlanPropertyPath::ZeroOrOne(p) => Box::new(hash_deduplicate(
                self.get_subject_or_object_identity_pairs(graph_name)
                    .chain(self.eval_open_path(p, graph_name)),
            )),
            PlanPropertyPath::NegatedPropertySet(ps) => {
                let ps = ps.clone();
                Box::new(
                    self.dataset
                        .encoded_quads_for_pattern(None, None, None, Some(graph_name))
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

    fn get_subject_or_object_identity_pairs(
        &self,
        graph_name: &EncodedTerm,
    ) -> impl Iterator<Item = Result<(EncodedTerm, EncodedTerm), EvaluationError>> {
        self.dataset
            .encoded_quads_for_pattern(None, None, None, Some(graph_name))
            .flat_map_ok(|t| once(Ok(t.subject)).chain(once(Ok(t.object))))
            .map(|e| e.map(|e| (e.clone(), e)))
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
    fn expression_evaluator(
        &self,
        expression: &PlanExpression,
    ) -> Rc<dyn Fn(&EncodedTuple) -> Option<EncodedTerm>> {
        match expression {
            PlanExpression::Constant(t) => {
                let t = t.clone();
                Rc::new(move |_| Some(t.clone()))
            }
            PlanExpression::Variable(v) => {
                let v = *v;
                Rc::new(move |tuple| tuple.get(v).cloned())
            }
            PlanExpression::Exists(plan) => {
                let plan = plan.clone();
                let eval = self.plan_evaluator(&plan);
                Rc::new(move |tuple| Some(eval(tuple.clone()).next().is_some().into()))
            }
            PlanExpression::Or(a, b) => {
                let a = self.expression_evaluator(a);
                let b = self.expression_evaluator(b);
                Rc::new(move |tuple| match a(tuple).and_then(|v| to_bool(&v)) {
                    Some(true) => Some(true.into()),
                    Some(false) => b(tuple),
                    None => {
                        if Some(true) == a(tuple).and_then(|v| to_bool(&v)) {
                            Some(true.into())
                        } else {
                            None
                        }
                    }
                })
            }
            PlanExpression::And(a, b) => {
                let a = self.expression_evaluator(a);
                let b = self.expression_evaluator(b);
                Rc::new(move |tuple| match a(tuple).and_then(|v| to_bool(&v)) {
                    Some(true) => b(tuple),
                    Some(false) => Some(false.into()),
                    None => {
                        if Some(false) == b(tuple).and_then(|v| to_bool(&v)) {
                            Some(false.into())
                        } else {
                            None
                        }
                    }
                })
            }
            PlanExpression::Equal(a, b) => {
                let a = self.expression_evaluator(a);
                let b = self.expression_evaluator(b);
                Rc::new(move |tuple| equals(&a(tuple)?, &b(tuple)?).map(|v| v.into()))
            }
            PlanExpression::Greater(a, b) => {
                let a = self.expression_evaluator(a);
                let b = self.expression_evaluator(b);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    Some(
                        (partial_cmp(&dataset, &a(tuple)?, &b(tuple)?)? == Ordering::Greater)
                            .into(),
                    )
                })
            }
            PlanExpression::GreaterOrEqual(a, b) => {
                let a = self.expression_evaluator(a);
                let b = self.expression_evaluator(b);
                let dataset = self.dataset.clone();
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
            PlanExpression::Less(a, b) => {
                let a = self.expression_evaluator(a);
                let b = self.expression_evaluator(b);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    Some((partial_cmp(&dataset, &a(tuple)?, &b(tuple)?)? == Ordering::Less).into())
                })
            }
            PlanExpression::LessOrEqual(a, b) => {
                let a = self.expression_evaluator(a);
                let b = self.expression_evaluator(b);
                let dataset = self.dataset.clone();
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
            PlanExpression::Add(a, b) => {
                let a = self.expression_evaluator(a);
                let b = self.expression_evaluator(b);
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
                        _ => None,
                    },
                )
            }
            PlanExpression::Subtract(a, b) => {
                let a = self.expression_evaluator(a);
                let b = self.expression_evaluator(b);
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
            PlanExpression::Multiply(a, b) => {
                let a = self.expression_evaluator(a);
                let b = self.expression_evaluator(b);
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
            PlanExpression::Divide(a, b) => {
                let a = self.expression_evaluator(a);
                let b = self.expression_evaluator(b);
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
            PlanExpression::UnaryPlus(e) => {
                let e = self.expression_evaluator(e);
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
            PlanExpression::UnaryMinus(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::FloatLiteral(value) => Some((-value).into()),
                    EncodedTerm::DoubleLiteral(value) => Some((-value).into()),
                    EncodedTerm::IntegerLiteral(value) => Some((-value).into()),
                    EncodedTerm::DecimalLiteral(value) => Some((-value).into()),
                    EncodedTerm::DurationLiteral(value) => Some((-value).into()),
                    EncodedTerm::YearMonthDurationLiteral(value) => Some((-value).into()),
                    EncodedTerm::DayTimeDurationLiteral(value) => Some((-value).into()),
                    _ => None,
                })
            }
            PlanExpression::Not(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| to_bool(&e(tuple)?).map(|v| (!v).into()))
            }
            PlanExpression::Str(e) | PlanExpression::StringCast(e) => {
                let e = self.expression_evaluator(e);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    Some(build_string_literal_from_id(to_string_id(
                        &dataset,
                        &e(tuple)?,
                    )?))
                })
            }
            PlanExpression::Lang(e) => {
                let e = self.expression_evaluator(e);
                let dataset = self.dataset.clone();
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
            PlanExpression::LangMatches(language_tag, language_range) => {
                let language_tag = self.expression_evaluator(language_tag);
                let language_range = self.expression_evaluator(language_range);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    let mut language_tag = to_simple_string(&dataset, &language_tag(tuple)?)?;
                    language_tag.make_ascii_lowercase();
                    let mut language_range = to_simple_string(&dataset, &language_range(tuple)?)?;
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
            PlanExpression::Datatype(e) => {
                let e = self.expression_evaluator(e);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| datatype(&dataset, &e(tuple)?))
            }
            PlanExpression::Bound(v) => {
                let v = *v;
                Rc::new(move |tuple| Some(tuple.contains(v).into()))
            }
            PlanExpression::Iri(e) => {
                let e = self.expression_evaluator(e);
                let dataset = self.dataset.clone();
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
            PlanExpression::BNode(id) => match id {
                Some(id) => {
                    let id = self.expression_evaluator(id);
                    let dataset = self.dataset.clone();
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
            PlanExpression::Rand => Rc::new(|_| Some(random::<f64>().into())),
            PlanExpression::Abs(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::IntegerLiteral(value) => Some(value.checked_abs()?.into()),
                    EncodedTerm::DecimalLiteral(value) => Some(value.abs().into()),
                    EncodedTerm::FloatLiteral(value) => Some(value.abs().into()),
                    EncodedTerm::DoubleLiteral(value) => Some(value.abs().into()),
                    _ => None,
                })
            }
            PlanExpression::Ceil(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::IntegerLiteral(value) => Some(value.into()),
                    EncodedTerm::DecimalLiteral(value) => Some(value.ceil().into()),
                    EncodedTerm::FloatLiteral(value) => Some(value.ceil().into()),
                    EncodedTerm::DoubleLiteral(value) => Some(value.ceil().into()),
                    _ => None,
                })
            }
            PlanExpression::Floor(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::IntegerLiteral(value) => Some(value.into()),
                    EncodedTerm::DecimalLiteral(value) => Some(value.floor().into()),
                    EncodedTerm::FloatLiteral(value) => Some(value.floor().into()),
                    EncodedTerm::DoubleLiteral(value) => Some(value.floor().into()),
                    _ => None,
                })
            }
            PlanExpression::Round(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::IntegerLiteral(value) => Some(value.into()),
                    EncodedTerm::DecimalLiteral(value) => Some(value.round().into()),
                    EncodedTerm::FloatLiteral(value) => Some(value.round().into()),
                    EncodedTerm::DoubleLiteral(value) => Some(value.round().into()),
                    _ => None,
                })
            }
            PlanExpression::Concat(l) => {
                let l: Vec<_> = l.iter().map(|e| self.expression_evaluator(e)).collect();
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    let mut result = String::default();
                    let mut language = None;
                    for e in &l {
                        let (value, e_language) = to_string_and_language(&dataset, &e(tuple)?)?;
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
            PlanExpression::SubStr(source, starting_loc, length) => {
                let source = self.expression_evaluator(source);
                let starting_loc = self.expression_evaluator(starting_loc);
                let length = length.as_ref().map(|l| self.expression_evaluator(l));
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    let (source, language) = to_string_and_language(&dataset, &source(tuple)?)?;

                    let starting_location: usize =
                        if let EncodedTerm::IntegerLiteral(v) = starting_loc(tuple)? {
                            v.try_into().ok()?
                        } else {
                            return None;
                        };
                    let length: Option<usize> = if let Some(length) = &length {
                        if let EncodedTerm::IntegerLiteral(v) = length(tuple)? {
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
                    Some(build_plain_literal(&dataset, result, language))
                })
            }
            PlanExpression::StrLen(arg) => {
                let arg = self.expression_evaluator(arg);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    Some((to_string(&dataset, &arg(tuple)?)?.chars().count() as i64).into())
                })
            }
            PlanExpression::Replace(arg, pattern, replacement, flags) => {
                let arg = self.expression_evaluator(arg);
                let pattern = self.expression_evaluator(pattern);
                let replacement = self.expression_evaluator(replacement);
                let flags = flags.as_ref().map(|flags| self.expression_evaluator(flags));
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    let regex = compile_pattern(
                        &dataset,
                        &pattern(tuple)?,
                        if let Some(flags) = &flags {
                            Some(flags(tuple)?)
                        } else {
                            None
                        },
                    )?;
                    let (text, language) = to_string_and_language(&dataset, &arg(tuple)?)?;
                    let replacement = to_simple_string(&dataset, &replacement(tuple)?)?;
                    Some(build_plain_literal(
                        &dataset,
                        &regex.replace_all(&text, replacement.as_str()),
                        language,
                    ))
                })
            }
            PlanExpression::UCase(e) => {
                let e = self.expression_evaluator(e);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    let (value, language) = to_string_and_language(&dataset, &e(tuple)?)?;
                    Some(build_plain_literal(
                        &dataset,
                        &value.to_uppercase(),
                        language,
                    ))
                })
            }
            PlanExpression::LCase(e) => {
                let e = self.expression_evaluator(e);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    let (value, language) = to_string_and_language(&dataset, &e(tuple)?)?;
                    Some(build_plain_literal(
                        &dataset,
                        &value.to_lowercase(),
                        language,
                    ))
                })
            }
            PlanExpression::StrStarts(arg1, arg2) => {
                let arg1 = self.expression_evaluator(arg1);
                let arg2 = self.expression_evaluator(arg2);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    let (arg1, arg2, _) =
                        to_argument_compatible_strings(&dataset, &arg1(tuple)?, &arg2(tuple)?)?;
                    Some((&arg1).starts_with(arg2.as_str()).into())
                })
            }
            PlanExpression::EncodeForUri(ltrl) => {
                let ltrl = self.expression_evaluator(ltrl);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    let ltlr = to_string(&dataset, &ltrl(tuple)?)?;
                    let mut result = Vec::with_capacity(ltlr.len());
                    for c in ltlr.bytes() {
                        match c {
                            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                                result.push(c)
                            }
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
            PlanExpression::StrEnds(arg1, arg2) => {
                let arg1 = self.expression_evaluator(arg1);
                let arg2 = self.expression_evaluator(arg2);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    let (arg1, arg2, _) =
                        to_argument_compatible_strings(&dataset, &arg1(tuple)?, &arg2(tuple)?)?;
                    Some((&arg1).ends_with(arg2.as_str()).into())
                })
            }
            PlanExpression::Contains(arg1, arg2) => {
                let arg1 = self.expression_evaluator(arg1);
                let arg2 = self.expression_evaluator(arg2);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    let (arg1, arg2, _) =
                        to_argument_compatible_strings(&dataset, &arg1(tuple)?, &arg2(tuple)?)?;
                    Some((&arg1).contains(arg2.as_str()).into())
                })
            }
            PlanExpression::StrBefore(arg1, arg2) => {
                let arg1 = self.expression_evaluator(arg1);
                let arg2 = self.expression_evaluator(arg2);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    let (arg1, arg2, language) =
                        to_argument_compatible_strings(&dataset, &arg1(tuple)?, &arg2(tuple)?)?;
                    Some(if let Some(position) = (&arg1).find(arg2.as_str()) {
                        build_plain_literal(&dataset, &arg1[..position], language)
                    } else {
                        build_string_literal(&dataset, "")
                    })
                })
            }
            PlanExpression::StrAfter(arg1, arg2) => {
                let arg1 = self.expression_evaluator(arg1);
                let arg2 = self.expression_evaluator(arg2);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    let (arg1, arg2, language) =
                        to_argument_compatible_strings(&dataset, &arg1(tuple)?, &arg2(tuple)?)?;
                    Some(if let Some(position) = (&arg1).find(arg2.as_str()) {
                        build_plain_literal(&dataset, &arg1[position + arg2.len()..], language)
                    } else {
                        build_string_literal(&dataset, "")
                    })
                })
            }
            PlanExpression::Year(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.year().into()),
                    EncodedTerm::DateLiteral(date) => Some(date.year().into()),
                    EncodedTerm::GYearMonthLiteral(year_month) => Some(year_month.year().into()),
                    EncodedTerm::GYearLiteral(year) => Some(year.year().into()),
                    _ => None,
                })
            }
            PlanExpression::Month(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.month().into()),
                    EncodedTerm::DateLiteral(date) => Some(date.month().into()),
                    EncodedTerm::GYearMonthLiteral(year_month) => Some(year_month.month().into()),
                    EncodedTerm::GMonthDayLiteral(month_day) => Some(month_day.month().into()),
                    EncodedTerm::GMonthLiteral(month) => Some(month.month().into()),
                    _ => None,
                })
            }
            PlanExpression::Day(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.day().into()),
                    EncodedTerm::DateLiteral(date) => Some(date.day().into()),
                    EncodedTerm::GMonthDayLiteral(month_day) => Some(month_day.day().into()),
                    EncodedTerm::GDayLiteral(day) => Some(day.day().into()),
                    _ => None,
                })
            }
            PlanExpression::Hours(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.hour().into()),
                    EncodedTerm::TimeLiteral(time) => Some(time.hour().into()),
                    _ => None,
                })
            }
            PlanExpression::Minutes(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.minute().into()),
                    EncodedTerm::TimeLiteral(time) => Some(time.minute().into()),
                    _ => None,
                })
            }
            PlanExpression::Seconds(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.second().into()),
                    EncodedTerm::TimeLiteral(time) => Some(time.second().into()),
                    _ => None,
                })
            }
            PlanExpression::Timezone(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| {
                    Some(
                        match e(tuple)? {
                            EncodedTerm::DateTimeLiteral(date_time) => date_time.timezone(),
                            EncodedTerm::TimeLiteral(time) => time.timezone(),
                            EncodedTerm::DateLiteral(date) => date.timezone(),
                            EncodedTerm::GYearMonthLiteral(year_month) => year_month.timezone(),
                            EncodedTerm::GYearLiteral(year) => year.timezone(),
                            EncodedTerm::GMonthDayLiteral(month_day) => month_day.timezone(),
                            EncodedTerm::GDayLiteral(day) => day.timezone(),
                            EncodedTerm::GMonthLiteral(month) => month.timezone(),
                            _ => None,
                        }?
                        .into(),
                    )
                })
            }
            PlanExpression::Tz(e) => {
                let e = self.expression_evaluator(e);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    let timezone_offset = match e(tuple)? {
                        EncodedTerm::DateTimeLiteral(date_time) => date_time.timezone_offset(),
                        EncodedTerm::TimeLiteral(time) => time.timezone_offset(),
                        EncodedTerm::DateLiteral(date) => date.timezone_offset(),
                        EncodedTerm::GYearMonthLiteral(year_month) => year_month.timezone_offset(),
                        EncodedTerm::GYearLiteral(year) => year.timezone_offset(),
                        EncodedTerm::GMonthDayLiteral(month_day) => month_day.timezone_offset(),
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
            PlanExpression::Now => {
                let now = self.now;
                Rc::new(move |_| Some(now.into()))
            }
            PlanExpression::Uuid => {
                let dataset = self.dataset.clone();
                Rc::new(move |_| {
                    let mut buffer = String::with_capacity(44);
                    buffer.push_str("urn:uuid:");
                    generate_uuid(&mut buffer);
                    Some(build_named_node(&dataset, &buffer))
                })
            }
            PlanExpression::StrUuid => {
                let dataset = self.dataset.clone();
                Rc::new(move |_| {
                    let mut buffer = String::with_capacity(36);
                    generate_uuid(&mut buffer);
                    Some(build_string_literal(&dataset, &buffer))
                })
            }
            PlanExpression::Md5(arg) => self.hash::<Md5>(arg),
            PlanExpression::Sha1(arg) => self.hash::<Sha1>(arg),
            PlanExpression::Sha256(arg) => self.hash::<Sha256>(arg),
            PlanExpression::Sha384(arg) => self.hash::<Sha384>(arg),
            PlanExpression::Sha512(arg) => self.hash::<Sha512>(arg),
            PlanExpression::Coalesce(l) => {
                let l: Vec<_> = l.iter().map(|e| self.expression_evaluator(e)).collect();
                Rc::new(move |tuple| {
                    for e in &l {
                        if let Some(result) = e(tuple) {
                            return Some(result);
                        }
                    }
                    None
                })
            }
            PlanExpression::If(a, b, c) => {
                let a = self.expression_evaluator(a);
                let b = self.expression_evaluator(b);
                let c = self.expression_evaluator(c);
                Rc::new(move |tuple| {
                    if to_bool(&a(tuple)?)? {
                        b(tuple)
                    } else {
                        c(tuple)
                    }
                })
            }
            PlanExpression::StrLang(lexical_form, lang_tag) => {
                let lexical_form = self.expression_evaluator(lexical_form);
                let lang_tag = self.expression_evaluator(lang_tag);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    Some(build_lang_string_literal_from_id(
                        to_simple_string_id(&lexical_form(tuple)?)?,
                        build_language_id(&dataset, &lang_tag(tuple)?)?,
                    ))
                })
            }
            PlanExpression::StrDt(lexical_form, datatype) => {
                let lexical_form = self.expression_evaluator(lexical_form);
                let datatype = self.expression_evaluator(datatype);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    let value = to_simple_string(&dataset, &lexical_form(tuple)?)?;
                    let datatype = if let EncodedTerm::NamedNode { iri_id } = datatype(tuple)? {
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
            PlanExpression::SameTerm(a, b) => {
                let a = self.expression_evaluator(a);
                let b = self.expression_evaluator(b);
                Rc::new(move |tuple| Some((a(tuple)? == b(tuple)?).into()))
            }
            PlanExpression::IsIri(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| Some(e(tuple)?.is_named_node().into()))
            }
            PlanExpression::IsBlank(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| Some(e(tuple)?.is_blank_node().into()))
            }
            PlanExpression::IsLiteral(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| Some(e(tuple)?.is_literal().into()))
            }
            PlanExpression::IsNumeric(e) => {
                let e = self.expression_evaluator(e);
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
            PlanExpression::Regex(text, pattern, flags) => {
                let text = self.expression_evaluator(text);
                let pattern = self.expression_evaluator(pattern);
                let flags = flags.as_ref().map(|flags| self.expression_evaluator(flags));
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| {
                    let regex = compile_pattern(
                        &dataset,
                        &pattern(tuple)?,
                        if let Some(flags) = &flags {
                            Some(flags(tuple)?)
                        } else {
                            None
                        },
                    )?;
                    let text = to_string(&dataset, &text(tuple)?)?;
                    Some(regex.is_match(&text).into())
                })
            }
            PlanExpression::Triple(s, p, o) => {
                let s = self.expression_evaluator(s);
                let p = self.expression_evaluator(p);
                let o = self.expression_evaluator(o);
                Rc::new(move |tuple| {
                    let s = s(tuple)?;
                    let p = p(tuple)?;
                    let o = o(tuple)?;
                    if !s.is_literal()
                        && !s.is_default_graph()
                        && p.is_named_node()
                        && !o.is_default_graph()
                    {
                        Some(EncodedTriple::new(s, p, o).into())
                    } else {
                        None
                    }
                })
            }
            PlanExpression::Subject(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| {
                    if let EncodedTerm::Triple(t) = e(tuple)? {
                        Some(t.subject.clone())
                    } else {
                        None
                    }
                })
            }
            PlanExpression::Predicate(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| {
                    if let EncodedTerm::Triple(t) = e(tuple)? {
                        Some(t.predicate.clone())
                    } else {
                        None
                    }
                })
            }
            PlanExpression::Object(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| {
                    if let EncodedTerm::Triple(t) = e(tuple)? {
                        Some(t.object.clone())
                    } else {
                        None
                    }
                })
            }
            PlanExpression::IsTriple(e) => {
                let e = self.expression_evaluator(e);
                Rc::new(move |tuple| Some(e(tuple)?.is_triple().into()))
            }
            PlanExpression::BooleanCast(e) => {
                let e = self.expression_evaluator(e);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::BooleanLiteral(value) => Some(value.into()),
                    EncodedTerm::FloatLiteral(value) => Some(value.to_bool().into()),
                    EncodedTerm::DoubleLiteral(value) => Some(value.to_bool().into()),
                    EncodedTerm::IntegerLiteral(value) => Some((value != 0).into()),
                    EncodedTerm::DecimalLiteral(value) => Some(value.to_bool().into()),
                    EncodedTerm::SmallStringLiteral(value) => parse_boolean_str(&value),
                    EncodedTerm::BigStringLiteral { value_id } => {
                        parse_boolean_str(&*dataset.get_str(&value_id).ok()??)
                    }
                    _ => None,
                })
            }
            PlanExpression::DoubleCast(e) => {
                let e = self.expression_evaluator(e);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::FloatLiteral(value) => Some(f64::from(value).into()),
                    EncodedTerm::DoubleLiteral(value) => Some(value.into()),
                    EncodedTerm::IntegerLiteral(value) => Some((value as f64).into()),
                    EncodedTerm::DecimalLiteral(value) => Some(value.to_double().into()),
                    EncodedTerm::BooleanLiteral(value) => {
                        Some(if value { 1_f64 } else { 0_f64 }.into())
                    }
                    EncodedTerm::SmallStringLiteral(value) => parse_double_str(&value),
                    EncodedTerm::BigStringLiteral { value_id } => {
                        parse_double_str(&*dataset.get_str(&value_id).ok()??)
                    }
                    _ => None,
                })
            }
            PlanExpression::FloatCast(e) => {
                let e = self.expression_evaluator(e);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::FloatLiteral(value) => Some(value.into()),
                    EncodedTerm::DoubleLiteral(value) => Some(value.to_f32().into()),
                    EncodedTerm::IntegerLiteral(value) => Some((value as f32).into()),
                    EncodedTerm::DecimalLiteral(value) => Some(value.to_float().into()),
                    EncodedTerm::BooleanLiteral(value) => {
                        Some(if value { 1_f32 } else { 0_f32 }.into())
                    }
                    EncodedTerm::SmallStringLiteral(value) => parse_float_str(&value),
                    EncodedTerm::BigStringLiteral { value_id } => {
                        parse_float_str(&*dataset.get_str(&value_id).ok()??)
                    }
                    _ => None,
                })
            }
            PlanExpression::IntegerCast(e) => {
                let e = self.expression_evaluator(e);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::FloatLiteral(value) => Some(value.to_i64().into()),
                    EncodedTerm::DoubleLiteral(value) => Some(value.to_i64().into()),
                    EncodedTerm::IntegerLiteral(value) => Some(value.into()),
                    EncodedTerm::DecimalLiteral(value) => Some(i64::try_from(value).ok()?.into()),
                    EncodedTerm::BooleanLiteral(value) => Some(if value { 1 } else { 0 }.into()),
                    EncodedTerm::SmallStringLiteral(value) => parse_integer_str(&value),
                    EncodedTerm::BigStringLiteral { value_id } => {
                        parse_integer_str(&*dataset.get_str(&value_id).ok()??)
                    }
                    _ => None,
                })
            }
            PlanExpression::DecimalCast(e) => {
                let e = self.expression_evaluator(e);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::FloatLiteral(value) => Some(Decimal::from_float(value).into()),
                    EncodedTerm::DoubleLiteral(value) => Some(Decimal::from_double(value).into()),
                    EncodedTerm::IntegerLiteral(value) => Some(Decimal::from(value).into()),
                    EncodedTerm::DecimalLiteral(value) => Some(value.into()),
                    EncodedTerm::BooleanLiteral(value) => {
                        Some(Decimal::from(if value { 1 } else { 0 }).into())
                    }
                    EncodedTerm::SmallStringLiteral(value) => parse_decimal_str(&value),
                    EncodedTerm::BigStringLiteral { value_id } => {
                        parse_decimal_str(&*dataset.get_str(&value_id).ok()??)
                    }
                    _ => None,
                })
            }
            PlanExpression::DateCast(e) => {
                let e = self.expression_evaluator(e);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::DateLiteral(value) => Some(value.into()),
                    EncodedTerm::DateTimeLiteral(value) => Some(Date::try_from(value).ok()?.into()),
                    EncodedTerm::SmallStringLiteral(value) => parse_date_str(&value),
                    EncodedTerm::BigStringLiteral { value_id } => {
                        parse_date_str(&*dataset.get_str(&value_id).ok()??)
                    }
                    _ => None,
                })
            }
            PlanExpression::TimeCast(e) => {
                let e = self.expression_evaluator(e);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::TimeLiteral(value) => Some(value.into()),
                    EncodedTerm::DateTimeLiteral(value) => Some(Time::try_from(value).ok()?.into()),
                    EncodedTerm::SmallStringLiteral(value) => parse_time_str(&value),
                    EncodedTerm::BigStringLiteral { value_id } => {
                        parse_time_str(&*dataset.get_str(&value_id).ok()??)
                    }
                    _ => None,
                })
            }
            PlanExpression::DateTimeCast(e) => {
                let e = self.expression_evaluator(e);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::DateTimeLiteral(value) => Some(value.into()),
                    EncodedTerm::DateLiteral(value) => Some(DateTime::try_from(value).ok()?.into()),
                    EncodedTerm::SmallStringLiteral(value) => parse_date_time_str(&value),
                    EncodedTerm::BigStringLiteral { value_id } => {
                        parse_date_time_str(&*dataset.get_str(&value_id).ok()??)
                    }
                    _ => None,
                })
            }
            PlanExpression::DurationCast(e) => {
                let e = self.expression_evaluator(e);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::DurationLiteral(value) => Some(value.into()),
                    EncodedTerm::YearMonthDurationLiteral(value) => {
                        Some(Duration::from(value).into())
                    }
                    EncodedTerm::DayTimeDurationLiteral(value) => {
                        Some(Duration::from(value).into())
                    }
                    EncodedTerm::SmallStringLiteral(value) => parse_duration_str(&value),
                    EncodedTerm::BigStringLiteral { value_id } => {
                        parse_duration_str(&*dataset.get_str(&value_id).ok()??)
                    }
                    _ => None,
                })
            }
            PlanExpression::YearMonthDurationCast(e) => {
                let e = self.expression_evaluator(e);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::DurationLiteral(value) => {
                        Some(YearMonthDuration::try_from(value).ok()?.into())
                    }
                    EncodedTerm::YearMonthDurationLiteral(value) => Some(value.into()),
                    EncodedTerm::SmallStringLiteral(value) => parse_year_month_duration_str(&value),
                    EncodedTerm::BigStringLiteral { value_id } => {
                        parse_year_month_duration_str(&*dataset.get_str(&value_id).ok()??)
                    }
                    _ => None,
                })
            }
            PlanExpression::DayTimeDurationCast(e) => {
                let e = self.expression_evaluator(e);
                let dataset = self.dataset.clone();
                Rc::new(move |tuple| match e(tuple)? {
                    EncodedTerm::DurationLiteral(value) => {
                        Some(DayTimeDuration::try_from(value).ok()?.into())
                    }
                    EncodedTerm::DayTimeDurationLiteral(value) => Some(value.into()),
                    EncodedTerm::SmallStringLiteral(value) => parse_day_time_duration_str(&value),
                    EncodedTerm::BigStringLiteral { value_id } => {
                        parse_day_time_duration_str(&*dataset.get_str(&value_id).ok()??)
                    }
                    _ => None,
                })
            }
            PlanExpression::CustomFunction(function_name, args) => {
                if let Some(function) = self.custom_functions.get(function_name).cloned() {
                    let args = args
                        .iter()
                        .map(|e| self.expression_evaluator(e))
                        .collect::<Vec<_>>();
                    let dataset = self.dataset.clone();
                    Rc::new(move |tuple| {
                        let args = args
                            .iter()
                            .map(|f| dataset.decode_term(&f(tuple)?).ok())
                            .collect::<Option<Vec<_>>>()?;
                        Some(dataset.encode_term(&function(&args)?))
                    })
                } else {
                    Rc::new(|_| None)
                }
            }
        }
    }

    fn hash<H: Digest>(
        &self,
        arg: &PlanExpression,
    ) -> Rc<dyn Fn(&EncodedTuple) -> Option<EncodedTerm>> {
        let arg = self.expression_evaluator(arg);
        let dataset = self.dataset.clone();
        Rc::new(move |tuple| {
            let input = to_simple_string(&dataset, &arg(tuple)?)?;
            let hash = hex::encode(H::new().chain_update(input.as_str()).finalize());
            Some(build_string_literal(&dataset, &hash))
        })
    }
}

fn to_bool(term: &EncodedTerm) -> Option<bool> {
    match term {
        EncodedTerm::BooleanLiteral(value) => Some(*value),
        EncodedTerm::SmallStringLiteral(value) => Some(!value.is_empty()),
        EncodedTerm::BigStringLiteral { .. } => {
            Some(false) // A big literal can't be empty
        }
        EncodedTerm::FloatLiteral(value) => Some(*value != Float::default()),
        EncodedTerm::DoubleLiteral(value) => Some(*value != Double::default()),
        EncodedTerm::IntegerLiteral(value) => Some(*value != 0),
        EncodedTerm::DecimalLiteral(value) => Some(*value != Decimal::default()),
        _ => None,
    }
}

fn to_string_id(dataset: &DatasetView, term: &EncodedTerm) -> Option<SmallStringOrId> {
    match term {
        EncodedTerm::NamedNode { iri_id } => Some((*iri_id).into()),
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
            if *value { "true" } else { "false" },
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
    if language2.is_none() || language1 == language2 {
        Some((value1, value2, language1))
    } else {
        None
    }
}

fn compile_pattern(
    dataset: &DatasetView,
    pattern: &EncodedTerm,
    flags: Option<EncodedTerm>,
) -> Option<Regex> {
    // TODO Avoid to compile the regex each time
    let pattern = to_simple_string(dataset, pattern)?;
    let mut regex_builder = RegexBuilder::new(&pattern);
    regex_builder.size_limit(REGEX_SIZE_LIMIT);
    if let Some(flags) = flags {
        let flags = to_simple_string(dataset, &flags)?;
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
                _ => (), //TODO: implement q
            }
        }
    }
    regex_builder.build().ok()
}

fn decode_bindings(
    dataset: Rc<DatasetView>,
    iter: EncodedTuplesIterator,
    variables: Rc<Vec<Variable>>,
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
    variables: Rc<Vec<Variable>>,
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

#[allow(
    clippy::float_cmp,
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss
)]
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
            EncodedTerm::IntegerLiteral(b) => Some(*a == Float::from_i64(*b)),
            EncodedTerm::DecimalLiteral(b) => Some(*a == b.to_float()),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::DoubleLiteral(a) => match b {
            EncodedTerm::FloatLiteral(b) => Some(*a == Double::from(*b)),
            EncodedTerm::DoubleLiteral(b) => Some(a == b),
            EncodedTerm::IntegerLiteral(b) => Some(*a == Double::from_i64(*b)),
            EncodedTerm::DecimalLiteral(b) => Some(*a == b.to_double()),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::IntegerLiteral(a) => match b {
            EncodedTerm::FloatLiteral(b) => Some(Float::from_i64(*a) == *b),
            EncodedTerm::DoubleLiteral(b) => Some(Double::from_i64(*a) == *b),
            EncodedTerm::IntegerLiteral(b) => Some(a == b),
            EncodedTerm::DecimalLiteral(b) => Some(Decimal::from(*a) == *b),
            _ if b.is_unknown_typed_literal() => None,
            _ => Some(false),
        },
        EncodedTerm::DecimalLiteral(a) => match b {
            EncodedTerm::FloatLiteral(b) => Some(a.to_float() == *b),
            EncodedTerm::DoubleLiteral(b) => Some(a.to_double() == *b),
            EncodedTerm::IntegerLiteral(b) => Some(*a == Decimal::from(*b)),
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
            a => match b {
                _ if b.is_named_node() || b.is_blank_node() => Ordering::Greater,
                _ if b.is_triple() => Ordering::Less,
                b => {
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

#[allow(clippy::cast_precision_loss)]
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
            EncodedTerm::IntegerLiteral(b) => a.partial_cmp(&Float::from_i64(*b)),
            EncodedTerm::DecimalLiteral(b) => a.partial_cmp(&b.to_float()),
            _ => None,
        },
        EncodedTerm::DoubleLiteral(a) => match b {
            EncodedTerm::FloatLiteral(b) => a.partial_cmp(&(*b).into()),
            EncodedTerm::DoubleLiteral(b) => a.partial_cmp(b),
            EncodedTerm::IntegerLiteral(b) => a.partial_cmp(&Double::from_i64(*b)),
            EncodedTerm::DecimalLiteral(b) => a.partial_cmp(&b.to_double()),
            _ => None,
        },
        EncodedTerm::IntegerLiteral(a) => match b {
            EncodedTerm::FloatLiteral(b) => Float::from_i64(*a).partial_cmp(b),
            EncodedTerm::DoubleLiteral(b) => Double::from_i64(*a).partial_cmp(b),
            EncodedTerm::IntegerLiteral(b) => a.partial_cmp(b),
            EncodedTerm::DecimalLiteral(b) => Decimal::from(*a).partial_cmp(b),
            _ => None,
        },
        EncodedTerm::DecimalLiteral(a) => match b {
            EncodedTerm::FloatLiteral(b) => a.to_float().partial_cmp(b),
            EncodedTerm::DoubleLiteral(b) => a.to_double().partial_cmp(b),
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
    //TODO: optimize?
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
    Integer(i64, i64),
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
    #[allow(clippy::cast_precision_loss)]
    fn new(a: EncodedTerm, b: EncodedTerm) -> Option<Self> {
        match (a, b) {
            (EncodedTerm::FloatLiteral(v1), EncodedTerm::FloatLiteral(v2)) => {
                Some(Self::Float(v1, v2))
            }
            (EncodedTerm::FloatLiteral(v1), EncodedTerm::DoubleLiteral(v2)) => {
                Some(Self::Double(v1.into(), v2))
            }
            (EncodedTerm::FloatLiteral(v1), EncodedTerm::IntegerLiteral(v2)) => {
                Some(Self::Float(v1, Float::from_i64(v2)))
            }
            (EncodedTerm::FloatLiteral(v1), EncodedTerm::DecimalLiteral(v2)) => {
                Some(Self::Float(v1, v2.to_float()))
            }
            (EncodedTerm::DoubleLiteral(v1), EncodedTerm::FloatLiteral(v2)) => {
                Some(Self::Double(v1, v2.into()))
            }
            (EncodedTerm::DoubleLiteral(v1), EncodedTerm::DoubleLiteral(v2)) => {
                Some(Self::Double(v1, v2))
            }
            (EncodedTerm::DoubleLiteral(v1), EncodedTerm::IntegerLiteral(v2)) => {
                Some(Self::Double(v1, Double::from_i64(v2)))
            }
            (EncodedTerm::DoubleLiteral(v1), EncodedTerm::DecimalLiteral(v2)) => {
                Some(Self::Double(v1, v2.to_double()))
            }
            (EncodedTerm::IntegerLiteral(v1), EncodedTerm::FloatLiteral(v2)) => {
                Some(Self::Float(Float::from_i64(v1), v2))
            }
            (EncodedTerm::IntegerLiteral(v1), EncodedTerm::DoubleLiteral(v2)) => {
                Some(Self::Double(Double::from_i64(v1), v2))
            }
            (EncodedTerm::IntegerLiteral(v1), EncodedTerm::IntegerLiteral(v2)) => {
                Some(Self::Integer(v1, v2))
            }
            (EncodedTerm::IntegerLiteral(v1), EncodedTerm::DecimalLiteral(v2)) => {
                Some(Self::Decimal(Decimal::from(v1), v2))
            }
            (EncodedTerm::DecimalLiteral(v1), EncodedTerm::FloatLiteral(v2)) => {
                Some(Self::Float(v1.to_float(), v2))
            }
            (EncodedTerm::DecimalLiteral(v1), EncodedTerm::DoubleLiteral(v2)) => {
                Some(Self::Double(v1.to_double(), v2))
            }
            (EncodedTerm::DecimalLiteral(v1), EncodedTerm::IntegerLiteral(v2)) => {
                Some(Self::Decimal(v1, Decimal::from(v2)))
            }
            (EncodedTerm::DecimalLiteral(v1), EncodedTerm::DecimalLiteral(v2)) => {
                Some(Self::Decimal(v1, v2))
            }
            (EncodedTerm::DurationLiteral(v1), EncodedTerm::DurationLiteral(v2)) => {
                Some(Self::Duration(v1, v2))
            }
            (EncodedTerm::DurationLiteral(v1), EncodedTerm::YearMonthDurationLiteral(v2)) => {
                Some(Self::Duration(v1, (v2).into()))
            }
            (EncodedTerm::DurationLiteral(v1), EncodedTerm::DayTimeDurationLiteral(v2)) => {
                Some(Self::Duration(v1, (v2).into()))
            }
            (EncodedTerm::YearMonthDurationLiteral(v1), EncodedTerm::DurationLiteral(v2)) => {
                Some(Self::Duration((v1).into(), v2))
            }
            (
                EncodedTerm::YearMonthDurationLiteral(v1),
                EncodedTerm::YearMonthDurationLiteral(v2),
            ) => Some(Self::YearMonthDuration(v1, v2)),
            (
                EncodedTerm::YearMonthDurationLiteral(v1),
                EncodedTerm::DayTimeDurationLiteral(v2),
            ) => Some(Self::Duration((v1).into(), (v2).into())),
            (EncodedTerm::DayTimeDurationLiteral(v1), EncodedTerm::DurationLiteral(v2)) => {
                Some(Self::Duration((v1).into(), v2))
            }
            (
                EncodedTerm::DayTimeDurationLiteral(v1),
                EncodedTerm::YearMonthDurationLiteral(v2),
            ) => Some(Self::Duration((v1).into(), (v2).into())),
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

fn get_pattern_value<'a>(
    selector: &'a PatternValue,
    tuple: &'a EncodedTuple,
) -> Option<EncodedTerm> {
    match selector {
        PatternValue::Constant(term) => Some(term.clone()),
        PatternValue::Variable(v) => tuple.get(*v).cloned(),
        PatternValue::Triple(triple) => Some(
            EncodedTriple {
                subject: get_pattern_value(&triple.subject, tuple)?,
                predicate: get_pattern_value(&triple.predicate, tuple)?,
                object: get_pattern_value(&triple.object, tuple)?,
            }
            .into(),
        ),
    }
}

fn put_pattern_value(
    selector: &PatternValue,
    value: EncodedTerm,
    tuple: &mut EncodedTuple,
) -> Option<()> {
    match selector {
        PatternValue::Constant(c) => {
            if *c == value {
                Some(())
            } else {
                None
            }
        }
        PatternValue::Variable(v) => {
            if let Some(old) = tuple.get(*v) {
                if value == *old {
                    Some(())
                } else {
                    None
                }
            } else {
                tuple.set(*v, value);
                Some(())
            }
        }
        PatternValue::Triple(triple) => {
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
                a.set(*var, b_value.clone());
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

struct CartesianProductJoinIterator {
    left_iter: EncodedTuplesIterator,
    right: Vec<EncodedTuple>,
    buffered_results: Vec<Result<EncodedTuple, EvaluationError>>,
}

impl Iterator for CartesianProductJoinIterator {
    type Item = Result<EncodedTuple, EvaluationError>;

    fn next(&mut self) -> Option<Result<EncodedTuple, EvaluationError>> {
        loop {
            if let Some(result) = self.buffered_results.pop() {
                return Some(result);
            }
            let left_tuple = match self.left_iter.next()? {
                Ok(left_tuple) => left_tuple,
                Err(error) => return Some(Err(error)),
            };
            for right_tuple in &self.right {
                if let Some(result_tuple) = left_tuple.combine_with(right_tuple) {
                    self.buffered_results.push(Ok(result_tuple))
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (min, max) = self.left_iter.size_hint();
        (min * self.right.len(), max.map(|v| v * self.right.len()))
    }
}

struct HashJoinIterator {
    left_iter: EncodedTuplesIterator,
    right: EncodedTupleSet,
    buffered_results: Vec<Result<EncodedTuple, EvaluationError>>,
}

impl Iterator for HashJoinIterator {
    type Item = Result<EncodedTuple, EvaluationError>;

    fn next(&mut self) -> Option<Result<EncodedTuple, EvaluationError>> {
        loop {
            if let Some(result) = self.buffered_results.pop() {
                return Some(result);
            }
            let left_tuple = match self.left_iter.next()? {
                Ok(left_tuple) => left_tuple,
                Err(error) => return Some(Err(error)),
            };
            for right_tuple in self.right.get(&left_tuple) {
                if let Some(result_tuple) = left_tuple.combine_with(right_tuple) {
                    self.buffered_results.push(Ok(result_tuple))
                }
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (
            0,
            self.left_iter.size_hint().1.map(|v| v * self.right.len()),
        )
    }
}

struct LeftJoinIterator {
    right_evaluator: Rc<dyn Fn(EncodedTuple) -> EncodedTuplesIterator>,
    left_iter: EncodedTuplesIterator,
    current_right: EncodedTuplesIterator,
}

impl Iterator for LeftJoinIterator {
    type Item = Result<EncodedTuple, EvaluationError>;

    fn next(&mut self) -> Option<Result<EncodedTuple, EvaluationError>> {
        if let Some(tuple) = self.current_right.next() {
            return Some(tuple);
        }
        match self.left_iter.next()? {
            Ok(left_tuple) => {
                self.current_right = (self.right_evaluator)(left_tuple.clone());
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

struct BadLeftJoinIterator {
    right_evaluator: Rc<dyn Fn(EncodedTuple) -> EncodedTuplesIterator>,
    left_iter: EncodedTuplesIterator,
    current_left: Option<EncodedTuple>,
    current_right: EncodedTuplesIterator,
    problem_vars: Rc<Vec<usize>>,
}

impl Iterator for BadLeftJoinIterator {
    type Item = Result<EncodedTuple, EvaluationError>;

    fn next(&mut self) -> Option<Result<EncodedTuple, EvaluationError>> {
        for right_tuple in &mut self.current_right {
            match right_tuple {
                Ok(right_tuple) => {
                    if let Some(combined) = combine_tuples(
                        right_tuple,
                        self.current_left.as_ref().unwrap(),
                        &self.problem_vars,
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
                unbind_variables(&mut filtered_left, &self.problem_vars);
                self.current_right = (self.right_evaluator)(filtered_left);
                for right_tuple in &mut self.current_right {
                    match right_tuple {
                        Ok(right_tuple) => {
                            if let Some(combined) =
                                combine_tuples(right_tuple, &left_tuple, &self.problem_vars)
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

struct UnionIterator {
    plans: Vec<Rc<dyn Fn(EncodedTuple) -> EncodedTuplesIterator>>,
    input: EncodedTuple,
    current_iterator: EncodedTuplesIterator,
    current_plan: usize,
}

impl Iterator for UnionIterator {
    type Item = Result<EncodedTuple, EvaluationError>;

    fn next(&mut self) -> Option<Result<EncodedTuple, EvaluationError>> {
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

    fn next(&mut self) -> Option<Result<EncodedTuple, EvaluationError>> {
        // Basic idea. We buffer the previous result and we only emit it when we kow the next one or it's the end
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
        (if min == 0 { 0 } else { 1 }, max)
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

    fn next(&mut self) -> Option<Result<Triple, EvaluationError>> {
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
                self.bnodes.clear(); //We do not reuse old bnodes
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (min, max) = self.iter.size_hint();
        (
            min * self.template.len(),
            max.map(|v| v * self.template.len()),
        )
    }
}

fn get_triple_template_value<'a>(
    selector: &'a TripleTemplateValue,
    tuple: &'a EncodedTuple,
    bnodes: &'a mut Vec<EncodedTerm>,
) -> Option<EncodedTerm> {
    match selector {
        TripleTemplateValue::Constant(term) => Some(term.clone()),
        TripleTemplateValue::Variable(v) => tuple.get(*v).cloned(),
        TripleTemplateValue::BlankNode(id) => {
            if *id >= bnodes.len() {
                bnodes.resize_with(*id + 1, new_bnode)
            }
            Some(bnodes[*id].clone())
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

    fn next(&mut self) -> Option<Result<Triple, EvaluationError>> {
        loop {
            if let Some(quad) = self.quads.next() {
                return Some(match quad {
                    Ok(quad) => self
                        .eval
                        .dataset
                        .decode_quad(&quad)
                        .map(|q| q.into())
                        .map_err(|e| e.into()),
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

    fn next(&mut self) -> Option<(Option<T1>, Option<T2>)> {
        match (self.a.next(), self.b.next()) {
            (None, None) => None,
            r => Some(r),
        }
    }
}

fn transitive_closure<T: Clone + Eq + Hash, NI: Iterator<Item = Result<T, EvaluationError>>>(
    start: impl IntoIterator<Item = Result<T, EvaluationError>>,
    next: impl Fn(T) -> NI,
) -> impl Iterator<Item = Result<T, EvaluationError>> {
    //TODO: optimize
    let mut all = HashSet::<T>::default();
    let mut errors = Vec::default();
    let mut current = start
        .into_iter()
        .filter_map(|e| match e {
            Ok(e) => {
                all.insert(e.clone());
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
            .flat_map(&next)
            .filter_map(|e| match e {
                Ok(e) => {
                    if all.contains(&e) {
                        None
                    } else {
                        all.insert(e.clone());
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

    fn next(&mut self) -> Option<Result<O, EvaluationError>> {
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
        if let Some(sum) = &self.sum {
            if let Some(operands) = element.and_then(|e| NumericBinaryOperands::new(sum.clone(), e))
            {
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
        self.sum.clone()
    }
}

#[derive(Debug, Default)]
struct AvgAccumulator {
    sum: SumAccumulator,
    count: CountAccumulator,
}

impl Accumulator for AvgAccumulator {
    fn add(&mut self, element: Option<EncodedTerm>) {
        self.sum.add(element.clone());
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

#[derive(Debug, Default)]
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
    separator: Rc<String>,
}

impl GroupConcatAccumulator {
    fn new(dataset: Rc<DatasetView>, separator: Rc<String>) -> Self {
        Self {
            dataset,
            concat: Some("".to_owned()),
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
