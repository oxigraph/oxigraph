use crate::model::BlankNode;
use crate::model::Triple;
use crate::sparql::model::*;
use crate::sparql::plan::*;
use crate::store::numeric_encoder::*;
use crate::store::numeric_encoder::{MemoryStringStore, ENCODED_EMPTY_STRING_LITERAL};
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
use std::collections::BTreeMap;
use std::collections::HashSet;
use std::convert::TryInto;
use std::fmt::Write;
use std::hash::Hash;
use std::iter::Iterator;
use std::iter::{empty, once};
use std::ops::Deref;
use std::str;
use std::sync::Mutex;
use std::u64;
use uuid::Uuid;

const REGEX_SIZE_LIMIT: usize = 1_000_000;

type EncodedTuplesIterator<'a> = Box<dyn Iterator<Item = Result<EncodedTuple>> + 'a>;

pub struct SimpleEvaluator<S: StoreConnection> {
    dataset: DatasetView<S>,
    bnodes_map: Mutex<BTreeMap<u64, Uuid>>,
    base_iri: Option<Iri<String>>,
    now: DateTime<FixedOffset>,
}

impl<'a, S: StoreConnection + 'a> SimpleEvaluator<S> {
    pub fn new(dataset: S, base_iri: Option<Iri<String>>) -> Self {
        Self {
            dataset: DatasetView::new(dataset),
            bnodes_map: Mutex::new(BTreeMap::default()),
            base_iri,
            now: Utc::now().with_timezone(&FixedOffset::east(0)),
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
        let iter = self.eval_plan(plan, vec![None; variables.len()]);
        Ok(QueryResult::Bindings(
            self.decode_bindings(iter, variables.to_vec()),
        ))
    }

    pub fn evaluate_ask_plan<'b>(&'b self, plan: &'b PlanNode) -> Result<QueryResult<'b>>
    where
        'a: 'b,
    {
        match self.eval_plan(plan, vec![]).next() {
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
            iter: self.eval_plan(plan, vec![]),
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
            iter: self.eval_plan(plan, vec![]),
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
            PlanNode::QuadPatternJoin {
                child,
                subject,
                predicate,
                object,
                graph_name,
            } => Box::new(self.eval_plan(&*child, from).flat_map_ok(move |tuple| {
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
            } => Box::new(self.eval_plan(&*child, from).flat_map_ok(move |tuple| {
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
                let left_iter = self.eval_plan(&*left, from.clone());
                let mut left_values = Vec::with_capacity(left_iter.size_hint().0);
                let mut errors = Vec::default();
                for result in left_iter {
                    match result {
                        Ok(result) => {
                            left_values.push(result);
                        }
                        Err(error) => errors.push(Err(error)),
                    }
                }
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
                let problem_vars = bind_variables_in_set(&from, &possible_problem_vars);
                let mut filtered_from = from.clone();
                unbind_variables(&mut filtered_from, &problem_vars);
                let iter = LeftJoinIterator {
                    eval: self,
                    right_plan: &*right,
                    left_iter: self.eval_plan(&*left, filtered_from),
                    current_right: Box::new(empty()),
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
                Box::new(self.eval_plan(&*child, from).filter(move |tuple| {
                    match tuple {
                        Ok(tuple) => eval
                            .eval_expression(&expression, tuple)
                            .and_then(|term| eval.to_bool(term))
                            .unwrap_or(false),
                        Err(_) => true,
                    }
                }))
            }
            PlanNode::Union { entry, children } => Box::new(UnionIterator {
                eval: self,
                children_plan: &children,
                input_iter: self.eval_plan(&*entry, from),
                current_input: Vec::default(),
                current_iterator: Box::new(empty()),
                current_child: children.len(),
            }),
            PlanNode::Extend {
                child,
                position,
                expression,
            } => {
                let eval = self;
                Box::new(self.eval_plan(&*child, from).map(move |tuple| {
                    let mut tuple = tuple?;
                    if let Some(value) = eval.eval_expression(&expression, &tuple) {
                        put_value(*position, value, &mut tuple)
                    }
                    Ok(tuple)
                }))
            }
            PlanNode::Sort { child, by } => {
                let iter = self.eval_plan(&*child, from);
                let mut values = Vec::with_capacity(iter.size_hint().0);
                let mut errors = Vec::default();
                for result in iter {
                    match result {
                        Ok(result) => {
                            values.push(result);
                        }
                        Err(error) => errors.push(Err(error)),
                    }
                }
                values.sort_unstable_by(|a, b| {
                    for comp in by {
                        match comp {
                            Comparator::Asc(expression) => {
                                match self.cmp_according_to_expression(a, b, &expression) {
                                    Ordering::Greater => return Ordering::Greater,
                                    Ordering::Less => return Ordering::Less,
                                    Ordering::Equal => (),
                                }
                            }
                            Comparator::Desc(expression) => {
                                match self.cmp_according_to_expression(a, b, &expression) {
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
                Box::new(self.eval_plan(&*child, from).map(move |tuple| {
                    let tuple = tuple?;
                    let mut new_tuple = Vec::with_capacity(mapping.len());
                    for key in mapping {
                        new_tuple.push(tuple[*key]);
                    }
                    Ok(new_tuple)
                }))
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

    fn eval_expression(
        &self,
        expression: &PlanExpression,
        tuple: &[Option<EncodedTerm>],
    ) -> Option<EncodedTerm> {
        match expression {
            PlanExpression::Constant(t) => Some(*t),
            PlanExpression::Variable(v) => get_tuple_value(*v, tuple),
            PlanExpression::Exists(node) => {
                Some(self.eval_plan(node, tuple.to_vec()).next().is_some().into())
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
            PlanExpression::Add(a, b) => Some(match self.parse_numeric_operands(a, b, tuple)? {
                NumericBinaryOperands::Float(v1, v2) => (v1 + v2).into(),
                NumericBinaryOperands::Double(v1, v2) => (v1 + v2).into(),
                NumericBinaryOperands::Integer(v1, v2) => v1.checked_add(v2)?.into(),
                NumericBinaryOperands::Decimal(v1, v2) => v1.checked_add(v2)?.into(),
            }),
            PlanExpression::Sub(a, b) => Some(match self.parse_numeric_operands(a, b, tuple)? {
                NumericBinaryOperands::Float(v1, v2) => (v1 - v2).into(),
                NumericBinaryOperands::Double(v1, v2) => (v1 - v2).into(),
                NumericBinaryOperands::Integer(v1, v2) => v1.checked_sub(v2)?.into(),
                NumericBinaryOperands::Decimal(v1, v2) => v1.checked_sub(v2)?.into(),
            }),
            PlanExpression::Mul(a, b) => Some(match self.parse_numeric_operands(a, b, tuple)? {
                NumericBinaryOperands::Float(v1, v2) => (v1 * v2).into(),
                NumericBinaryOperands::Double(v1, v2) => (v1 * v2).into(),
                NumericBinaryOperands::Integer(v1, v2) => v1.checked_mul(v2)?.into(),
                NumericBinaryOperands::Decimal(v1, v2) => v1.checked_mul(v2)?.into(),
            }),
            PlanExpression::Div(a, b) => Some(match self.parse_numeric_operands(a, b, tuple)? {
                NumericBinaryOperands::Float(v1, v2) => (v1 / v2).into(),
                NumericBinaryOperands::Double(v1, v2) => (v1 / v2).into(),
                NumericBinaryOperands::Integer(v1, v2) => v1.checked_div(v2)?.into(),
                NumericBinaryOperands::Decimal(v1, v2) => v1.checked_div(v2)?.into(),
            }),
            PlanExpression::UnaryPlus(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::FloatLiteral(value) => Some((*value).into()),
                EncodedTerm::DoubleLiteral(value) => Some((*value).into()),
                EncodedTerm::IntegerLiteral(value) => Some((value).into()),
                EncodedTerm::DecimalLiteral(value) => Some((value).into()),
                _ => None,
            },
            PlanExpression::UnaryMinus(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::FloatLiteral(value) => Some((-*value).into()),
                EncodedTerm::DoubleLiteral(value) => Some((-*value).into()),
                EncodedTerm::IntegerLiteral(value) => Some((-value).into()),
                EncodedTerm::DecimalLiteral(value) => Some((-value).into()),
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
                let language_tag =
                    self.to_simple_string(self.eval_expression(language_tag, tuple)?)?;
                let language_range =
                    self.to_simple_string(self.eval_expression(language_range, tuple)?)?;
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
            PlanExpression::Datatype(e) => self.eval_expression(e, tuple)?.datatype(),
            PlanExpression::Bound(v) => Some(has_tuple_value(*v, tuple).into()),
            PlanExpression::IRI(e) => {
                let iri_id = match self.eval_expression(e, tuple)? {
                    EncodedTerm::NamedNode { iri_id } => Some(iri_id),
                    EncodedTerm::StringLiteral { value_id } => Some(value_id),
                    _ => None,
                }?;
                let iri = self.dataset.get_str(iri_id).ok()??;
                Some(if let Some(base_iri) = &self.base_iri {
                    EncodedTerm::NamedNode {
                        iri_id: self
                            .dataset
                            .insert_str(&base_iri.resolve(&iri).ok()?.into_inner())
                            .ok()?,
                    }
                } else {
                    Iri::parse(iri).ok()?;
                    EncodedTerm::NamedNode { iri_id }
                })
            }
            PlanExpression::BNode(id) => match id {
                Some(id) => {
                    if let EncodedTerm::StringLiteral { value_id } =
                        self.eval_expression(id, tuple)?
                    {
                        Some(EncodedTerm::BlankNode(
                            *self
                                .bnodes_map
                                .lock()
                                .ok()?
                                .entry(value_id)
                                .or_insert_with(Uuid::new_v4),
                        ))
                    } else {
                        None
                    }
                }
                None => Some(EncodedTerm::BlankNode(Uuid::new_v4())),
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
                    .count() as i128)
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
                self.build_plain_literal(&regex.replace_all(&text, &replacement as &str), language)
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
                Some((&arg1).starts_with(&arg2 as &str).into())
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
                Some(EncodedTerm::StringLiteral {
                    value_id: self
                        .dataset
                        .insert_str(str::from_utf8(&result).ok()?)
                        .ok()?,
                })
            }
            PlanExpression::StrEnds(arg1, arg2) => {
                let (arg1, arg2, _) = self.to_argument_compatible_strings(
                    self.eval_expression(arg1, tuple)?,
                    self.eval_expression(arg2, tuple)?,
                )?;
                Some((&arg1).ends_with(&arg2 as &str).into())
            }
            PlanExpression::Contains(arg1, arg2) => {
                let (arg1, arg2, _) = self.to_argument_compatible_strings(
                    self.eval_expression(arg1, tuple)?,
                    self.eval_expression(arg2, tuple)?,
                )?;
                Some((&arg1).contains(&arg2 as &str).into())
            }
            PlanExpression::StrBefore(arg1, arg2) => {
                let (arg1, arg2, language) = self.to_argument_compatible_strings(
                    self.eval_expression(arg1, tuple)?,
                    self.eval_expression(arg2, tuple)?,
                )?;
                if let Some(position) = (&arg1).find(&arg2 as &str) {
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
                if let Some(position) = (&arg1).find(&arg2 as &str) {
                    self.build_plain_literal(&arg1[position + arg2.len()..], language)
                } else {
                    Some(ENCODED_EMPTY_STRING_LITERAL)
                }
            }
            PlanExpression::Year(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::DateLiteral(date) => Some(date.year().into()),
                EncodedTerm::NaiveDateLiteral(date) => Some(date.year().into()),
                EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.year().into()),
                EncodedTerm::NaiveDateTimeLiteral(date_time) => Some(date_time.year().into()),
                _ => None,
            },
            PlanExpression::Month(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::DateLiteral(date) => Some(date.year().into()),
                EncodedTerm::NaiveDateLiteral(date) => Some(date.month().into()),
                EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.month().into()),
                EncodedTerm::NaiveDateTimeLiteral(date_time) => Some(date_time.month().into()),
                _ => None,
            },
            PlanExpression::Day(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::DateLiteral(date) => Some(date.year().into()),
                EncodedTerm::NaiveDateLiteral(date) => Some(date.day().into()),
                EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.day().into()),
                EncodedTerm::NaiveDateTimeLiteral(date_time) => Some(date_time.day().into()),
                _ => None,
            },
            PlanExpression::Hours(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::NaiveTimeLiteral(time) => Some(time.hour().into()),
                EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.hour().into()),
                EncodedTerm::NaiveDateTimeLiteral(date_time) => Some(date_time.hour().into()),
                _ => None,
            },
            PlanExpression::Minutes(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::NaiveTimeLiteral(time) => Some(time.minute().into()),
                EncodedTerm::DateTimeLiteral(date_time) => Some(date_time.minute().into()),
                EncodedTerm::NaiveDateTimeLiteral(date_time) => Some(date_time.minute().into()),
                _ => None,
            },
            PlanExpression::Seconds(e) => match self.eval_expression(e, tuple)? {
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
                let timezone = match self.eval_expression(e, tuple)? {
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
                    value_id: self.dataset.insert_str(&result).ok()?,
                    datatype_id: self
                        .dataset
                        .insert_str("http://www.w3.org/2001/XMLSchema#dayTimeDuration")
                        .ok()?,
                })
            }
            PlanExpression::Tz(e) => {
                let timezone = match self.eval_expression(e, tuple)? {
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
                            self.dataset.insert_str("Z").ok()?
                        } else {
                            self.dataset.insert_str(&timezone.to_string()).ok()?
                        },
                    }
                } else {
                    ENCODED_EMPTY_STRING_LITERAL
                })
            }
            PlanExpression::Now => Some(self.now.into()),
            PlanExpression::UUID => Some(EncodedTerm::NamedNode {
                iri_id: self
                    .dataset
                    .insert_str(
                        Uuid::new_v4()
                            .to_urn()
                            .encode_lower(&mut Uuid::encode_buffer()),
                    )
                    .ok()?,
            }),
            PlanExpression::StrUUID => Some(EncodedTerm::StringLiteral {
                value_id: self
                    .dataset
                    .insert_str(
                        Uuid::new_v4()
                            .to_hyphenated()
                            .encode_lower(&mut Uuid::encode_buffer()),
                    )
                    .ok()?,
            }),
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
                    language_id: self
                        .to_simple_string_id(self.eval_expression(lang_tag, tuple)?)?,
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
                EncodedTerm::StringLiteral { value_id } => self
                    .dataset
                    .encoder()
                    .encode_boolean_str(&*self.dataset.get_str(value_id).ok()??),
                _ => None,
            },
            PlanExpression::DoubleCast(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::FloatLiteral(value) => Some(value.to_f64()?.into()),
                EncodedTerm::DoubleLiteral(value) => Some(value.to_f64()?.into()),
                EncodedTerm::IntegerLiteral(value) => Some(value.to_f64()?.into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.to_f64()?.into()),
                EncodedTerm::BooleanLiteral(value) => {
                    Some(if value { 1. as f64 } else { 0. }.into())
                }
                EncodedTerm::StringLiteral { value_id } => self
                    .dataset
                    .encoder()
                    .encode_double_str(&*self.dataset.get_str(value_id).ok()??),
                _ => None,
            },
            PlanExpression::FloatCast(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::FloatLiteral(value) => Some(value.to_f32()?.into()),
                EncodedTerm::DoubleLiteral(value) => Some(value.to_f32()?.into()),
                EncodedTerm::IntegerLiteral(value) => Some(value.to_f32()?.into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.to_f32()?.into()),
                EncodedTerm::BooleanLiteral(value) => {
                    Some(if value { 1. as f32 } else { 0. }.into())
                }
                EncodedTerm::StringLiteral { value_id } => self
                    .dataset
                    .encoder()
                    .encode_float_str(&*self.dataset.get_str(value_id).ok()??),
                _ => None,
            },
            PlanExpression::IntegerCast(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::FloatLiteral(value) => Some(value.to_i128()?.into()),
                EncodedTerm::DoubleLiteral(value) => Some(value.to_i128()?.into()),
                EncodedTerm::IntegerLiteral(value) => Some(value.to_i128()?.into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.to_i128()?.into()),
                EncodedTerm::BooleanLiteral(value) => Some(if value { 1 } else { 0 }.into()),
                EncodedTerm::StringLiteral { value_id } => self
                    .dataset
                    .encoder()
                    .encode_integer_str(&*self.dataset.get_str(value_id).ok()??),
                _ => None,
            },
            PlanExpression::DecimalCast(e) => match self.eval_expression(e, tuple)? {
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
                EncodedTerm::StringLiteral { value_id } => self
                    .dataset
                    .encoder()
                    .encode_decimal_str(&*self.dataset.get_str(value_id).ok()??),
                _ => None,
            },
            PlanExpression::DateCast(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::DateLiteral(value) => Some(value.into()),
                EncodedTerm::NaiveDateLiteral(value) => Some(value.into()),
                EncodedTerm::DateTimeLiteral(value) => Some(value.date().into()),
                EncodedTerm::NaiveDateTimeLiteral(value) => Some(value.date().into()),
                EncodedTerm::StringLiteral { value_id } => self
                    .dataset
                    .encoder()
                    .encode_date_str(&*self.dataset.get_str(value_id).ok()??),
                _ => None,
            },
            PlanExpression::TimeCast(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::NaiveTimeLiteral(value) => Some(value.into()),
                EncodedTerm::DateTimeLiteral(value) => Some(value.time().into()),
                EncodedTerm::NaiveDateTimeLiteral(value) => Some(value.time().into()),
                EncodedTerm::StringLiteral { value_id } => self
                    .dataset
                    .encoder()
                    .encode_time_str(&*self.dataset.get_str(value_id).ok()??),
                _ => None,
            },
            PlanExpression::DateTimeCast(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::DateTimeLiteral(value) => Some(value.into()),
                EncodedTerm::NaiveDateTimeLiteral(value) => Some(value.into()),
                EncodedTerm::StringLiteral { value_id } => self
                    .dataset
                    .encoder()
                    .encode_date_time_str(&*self.dataset.get_str(value_id).ok()??),
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
            EncodedTerm::FloatLiteral(value) => Some(!value.is_zero()),
            EncodedTerm::DoubleLiteral(value) => Some(!value.is_zero()),
            EncodedTerm::IntegerLiteral(value) => Some(!value.is_zero()),
            EncodedTerm::DecimalLiteral(value) => Some(!value.is_zero()),
            _ => None,
        }
    }

    fn to_string_id(&self, term: EncodedTerm) -> Option<u64> {
        match term {
            EncodedTerm::DefaultGraph => None,
            EncodedTerm::NamedNode { iri_id } => Some(iri_id),
            EncodedTerm::BlankNode(_) => None,
            EncodedTerm::StringLiteral { value_id }
            | EncodedTerm::LangStringLiteral { value_id, .. }
            | EncodedTerm::TypedLiteral { value_id, .. } => Some(value_id),
            EncodedTerm::BooleanLiteral(value) => self
                .dataset
                .insert_str(if value { "true" } else { "false" })
                .ok(),
            EncodedTerm::FloatLiteral(value) => self.dataset.insert_str(&value.to_string()).ok(),
            EncodedTerm::DoubleLiteral(value) => self.dataset.insert_str(&value.to_string()).ok(),
            EncodedTerm::IntegerLiteral(value) => self.dataset.insert_str(&value.to_string()).ok(),
            EncodedTerm::DecimalLiteral(value) => self.dataset.insert_str(&value.to_string()).ok(),
            EncodedTerm::DateLiteral(value) => self.dataset.insert_str(&value.to_string()).ok(),
            EncodedTerm::NaiveDateLiteral(value) => {
                self.dataset.insert_str(&value.to_string()).ok()
            }
            EncodedTerm::NaiveTimeLiteral(value) => {
                self.dataset.insert_str(&value.to_string()).ok()
            }
            EncodedTerm::DateTimeLiteral(value) => self.dataset.insert_str(&value.to_string()).ok(),
            EncodedTerm::NaiveDateTimeLiteral(value) => {
                self.dataset.insert_str(&value.to_string()).ok()
            }
        }
    }

    fn to_simple_string(
        &self,
        term: EncodedTerm,
    ) -> Option<<DatasetView<S> as StringStore>::StringType> {
        if let EncodedTerm::StringLiteral { value_id } = term {
            self.dataset.get_str(value_id).ok()?
        } else {
            None
        }
    }

    fn to_simple_string_id(&self, term: EncodedTerm) -> Option<u64> {
        if let EncodedTerm::StringLiteral { value_id } = term {
            Some(value_id)
        } else {
            None
        }
    }

    fn to_string(&self, term: EncodedTerm) -> Option<<DatasetView<S> as StringStore>::StringType> {
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
    ) -> Option<(<DatasetView<S> as StringStore>::StringType, Option<u64>)> {
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

    fn build_plain_literal(&self, value: &str, language: Option<u64>) -> Option<EncodedTerm> {
        Some(if let Some(language_id) = language {
            EncodedTerm::LangStringLiteral {
                value_id: self.dataset.insert_str(value).ok()?,
                language_id,
            }
        } else {
            EncodedTerm::StringLiteral {
                value_id: self.dataset.insert_str(value).ok()?,
            }
        })
    }

    fn to_argument_compatible_strings(
        &self,
        arg1: EncodedTerm,
        arg2: EncodedTerm,
    ) -> Option<(
        <DatasetView<S> as StringStore>::StringType,
        <DatasetView<S> as StringStore>::StringType,
        Option<u64>,
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

    fn parse_numeric_operands(
        &self,
        e1: &PlanExpression,
        e2: &PlanExpression,
        tuple: &[Option<EncodedTerm>],
    ) -> Option<NumericBinaryOperands> {
        match (
            self.eval_expression(&e1, tuple)?,
            self.eval_expression(&e2, tuple)?,
        ) {
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

    fn decode_bindings<'b>(
        &'b self,
        iter: EncodedTuplesIterator<'b>,
        variables: Vec<Variable>,
    ) -> BindingsIterator<'b>
    where
        'a: 'b,
    {
        let eval = self;
        BindingsIterator::new(
            variables,
            Box::new(iter.map(move |values| {
                let encoder = eval.dataset.encoder();
                values?
                    .into_iter()
                    .map(|value| {
                        Ok(match value {
                            Some(term) => Some(encoder.decode_term(term)?),
                            None => None,
                        })
                    })
                    .collect()
            })),
        )
    }

    #[allow(clippy::float_cmp)]
    fn equals(&self, a: EncodedTerm, b: EncodedTerm) -> Option<bool> {
        match a {
            EncodedTerm::DefaultGraph
            | EncodedTerm::NamedNode { .. }
            | EncodedTerm::BlankNode(_)
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

    fn cmp_according_to_expression(
        &self,
        tuple_a: &[Option<EncodedTerm>],
        tuple_b: &[Option<EncodedTerm>],
        expression: &PlanExpression,
    ) -> Ordering {
        match (
            self.eval_expression(expression, tuple_a),
            self.eval_expression(expression, tuple_b),
        ) {
            (Some(a), Some(b)) => match a {
                EncodedTerm::BlankNode(a) => {
                    if let EncodedTerm::BlankNode(b) = b {
                        a.cmp(&b)
                    } else {
                        Ordering::Less
                    }
                }
                EncodedTerm::NamedNode { iri_id: a } => match b {
                    EncodedTerm::NamedNode { iri_id: b } => {
                        self.compare_str_ids(a, b).unwrap_or(Ordering::Equal)
                    }
                    EncodedTerm::BlankNode(_) => Ordering::Greater,
                    _ => Ordering::Less,
                },
                a => match b {
                    EncodedTerm::NamedNode { .. } | EncodedTerm::BlankNode(_) => Ordering::Greater,
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

    fn compare_str_ids(&self, a: u64, b: u64) -> Option<Ordering> {
        Some(
            self.dataset
                .get_str(a)
                .ok()??
                .cmp(&self.dataset.get_str(b).ok()??),
        )
    }

    fn hash<H: Digest>(
        &self,
        arg: &PlanExpression,
        tuple: &[Option<EncodedTerm>],
    ) -> Option<EncodedTerm> {
        let input = self.to_simple_string(self.eval_expression(arg, tuple)?)?;
        let hash = hex::encode(H::new().chain(&input as &str).result());
        Some(EncodedTerm::StringLiteral {
            value_id: self.dataset.insert_str(&hash).ok()?,
        })
    }
}

struct DatasetView<S: StoreConnection> {
    store: S,
    extra: MemoryStringStore,
}

impl<S: StoreConnection> DatasetView<S> {
    fn new(store: S) -> Self {
        Self {
            store,
            extra: MemoryStringStore::default(),
        }
    }

    fn quads_for_pattern<'a>(
        &'a self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad>> + 'a> {
        self.store
            .quads_for_pattern(subject, predicate, object, graph_name)
    }

    fn encoder(&self) -> Encoder<&Self> {
        Encoder::new(&self)
    }
}

impl<S: StoreConnection> StringStore for DatasetView<S> {
    type StringType = StringOrStoreString<S::StringType>;

    fn get_str(&self, id: u64) -> Result<Option<StringOrStoreString<S::StringType>>> {
        Ok(if let Some(value) = self.store.get_str(id)? {
            Some(StringOrStoreString::Store(value))
        } else if let Some(value) = self.extra.get_str(u64::MAX - id)? {
            Some(StringOrStoreString::String(value))
        } else {
            None
        })
    }

    fn get_str_id(&self, value: &str) -> Result<Option<u64>> {
        Ok(if let Some(id) = self.store.get_str_id(value)? {
            Some(id)
        } else {
            self.extra.get_str_id(value)?.map(|id| u64::MAX - id)
        })
    }

    fn insert_str(&self, value: &str) -> Result<u64> {
        Ok(if let Some(id) = self.store.get_str_id(value)? {
            id
        } else {
            u64::MAX - self.extra.insert_str(value)?
        })
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
}

impl<'a, S: StoreConnection> Iterator for LeftJoinIterator<'a, S> {
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
    children_plan: &'a [PlanNode],
    input_iter: EncodedTuplesIterator<'a>,
    current_input: EncodedTuple,
    current_iterator: EncodedTuplesIterator<'a>,
    current_child: usize,
}

impl<'a, S: StoreConnection> Iterator for UnionIterator<'a, S> {
    type Item = Result<EncodedTuple>;

    fn next(&mut self) -> Option<Result<EncodedTuple>> {
        loop {
            if let Some(tuple) = self.current_iterator.next() {
                return Some(tuple);
            }
            if self.current_child == self.children_plan.len() {
                match self.input_iter.next()? {
                    Ok(input_tuple) => {
                        self.current_input = input_tuple;
                        self.current_child = 0;
                    }
                    Err(error) => return Some(Err(error)),
                }
            }
            self.current_iterator = self.eval.eval_plan(
                &self.children_plan[self.current_child],
                self.current_input.clone(),
            );
            self.current_child += 1;
        }
    }
}

struct ConstructIterator<'a, S: StoreConnection> {
    eval: &'a SimpleEvaluator<S>,
    iter: EncodedTuplesIterator<'a>,
    template: &'a [TripleTemplate],
    buffered_results: Vec<Result<Triple>>,
    bnodes: Vec<BlankNode>,
}

impl<'a, S: StoreConnection> Iterator for ConstructIterator<'a, S> {
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
                let encoder = self.eval.dataset.encoder();
                for template in self.template {
                    if let (Some(subject), Some(predicate), Some(object)) = (
                        get_triple_template_value(&template.subject, &tuple, &mut self.bnodes),
                        get_triple_template_value(&template.predicate, &tuple, &mut self.bnodes),
                        get_triple_template_value(&template.object, &tuple, &mut self.bnodes),
                    ) {
                        self.buffered_results
                            .push(decode_triple(&encoder, subject, predicate, object));
                    } else {
                        self.buffered_results.clear(); //No match, we do not output any triple for this row
                        break;
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

fn decode_triple<S: StringStore>(
    encoder: &Encoder<S>,
    subject: EncodedTerm,
    predicate: EncodedTerm,
    object: EncodedTerm,
) -> Result<Triple> {
    Ok(Triple::new(
        encoder.decode_named_or_blank_node(subject)?,
        encoder.decode_named_node(predicate)?,
        encoder.decode_term(object)?,
    ))
}

struct DescribeIterator<'a, S: StoreConnection + 'a> {
    eval: &'a SimpleEvaluator<S>,
    iter: EncodedTuplesIterator<'a>,
    quads: Box<dyn Iterator<Item = Result<EncodedQuad>> + 'a>,
}

impl<'a, S: StoreConnection> Iterator for DescribeIterator<'a, S> {
    type Item = Result<Triple>;

    fn next(&mut self) -> Option<Result<Triple>> {
        loop {
            if let Some(quad) = self.quads.next() {
                return Some(match quad {
                    Ok(quad) => self
                        .eval
                        .dataset
                        .encoder()
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
