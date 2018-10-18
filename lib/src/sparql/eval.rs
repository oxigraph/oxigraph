use chrono::DateTime;
use chrono::NaiveDateTime;
use language_tags::LanguageTag;
use model::BlankNode;
use num_traits::identities::Zero;
use num_traits::FromPrimitive;
use num_traits::One;
use num_traits::ToPrimitive;
use ordered_float::OrderedFloat;
use rust_decimal::Decimal;
use sparql::algebra::*;
use sparql::plan::*;
use std::cmp::Ordering;
use std::collections::HashSet;
use std::iter::once;
use std::iter::Iterator;
use std::str;
use std::str::FromStr;
use std::sync::Arc;
use store::encoded::EncodedQuadsStore;
use store::numeric_encoder::*;
use Result;

type EncodedTuplesIterator<'a> = Box<dyn Iterator<Item = Result<EncodedTuple>> + 'a>;

pub struct SimpleEvaluator<S: EncodedQuadsStore> {
    store: Arc<S>,
}

impl<S: EncodedQuadsStore> Clone for SimpleEvaluator<S> {
    fn clone(&self) -> Self {
        Self {
            store: self.store.clone(),
        }
    }
}

impl<S: EncodedQuadsStore> SimpleEvaluator<S> {
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
    }

    pub fn evaluate_select_plan<'a>(
        &'a self,
        plan: &'a PlanNode,
        variables: &[Variable],
    ) -> Result<QueryResult<'a>> {
        let iter = self.eval_plan(plan, vec![None; variables.len()]);
        Ok(QueryResult::Bindings(
            self.decode_bindings(iter, variables.to_vec()),
        ))
    }

    fn eval_plan<'a>(&self, node: &'a PlanNode, from: EncodedTuple) -> EncodedTuplesIterator<'a> {
        match node {
            PlanNode::Init => Box::new(once(Ok(from))),
            PlanNode::StaticBindings { tuples } => Box::new(tuples.iter().cloned().map(Ok)),
            PlanNode::QuadPatternJoin {
                child,
                subject,
                predicate,
                object,
                graph_name,
            } => {
                let eval = self.clone();
                Box::new(
                    self.eval_plan(&*child, from)
                        .flat_map(move |tuple| match tuple {
                            Ok(tuple) => {
                                let iter: EncodedTuplesIterator = match eval
                                    .store
                                    .quads_for_pattern(
                                        get_pattern_value(&subject, &tuple),
                                        get_pattern_value(&predicate, &tuple),
                                        get_pattern_value(&object, &tuple),
                                        graph_name.and_then(|graph_name| {
                                            get_pattern_value(&graph_name, &tuple)
                                        }),
                                    ) {
                                    Ok(mut iter) => {
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
                                        if let Some(graph_name) = graph_name {
                                            if graph_name.is_var() {
                                                iter = Box::new(iter.filter(|quad| match quad {
                                                    Err(_) => true,
                                                    Ok(quad) => {
                                                        quad.graph_name != ENCODED_DEFAULT_GRAPH
                                                    }
                                                }))
                                            }
                                        } else {
                                            iter = Box::new(iter.filter(|quad| match quad {
                                                Err(_) => true,
                                                Ok(quad) => {
                                                    quad.graph_name == ENCODED_DEFAULT_GRAPH
                                                }
                                            }))
                                        }
                                        Box::new(iter.map(move |quad| {
                                            let quad = quad?;
                                            let mut new_tuple = tuple.clone();
                                            put_pattern_value(
                                                &subject,
                                                quad.subject,
                                                &mut new_tuple,
                                            );
                                            put_pattern_value(
                                                &predicate,
                                                quad.predicate,
                                                &mut new_tuple,
                                            );
                                            put_pattern_value(&object, quad.object, &mut new_tuple);
                                            if let Some(graph_name) = graph_name {
                                                put_pattern_value(
                                                    &graph_name,
                                                    quad.graph_name,
                                                    &mut new_tuple,
                                                );
                                            }
                                            Ok(new_tuple)
                                        }))
                                    }
                                    Err(error) => Box::new(once(Err(error))),
                                };
                                iter
                            }
                            Err(error) => Box::new(once(Err(error))),
                        }),
                )
            }
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
            PlanNode::LeftJoin {
                left,
                right,
                possible_problem_vars,
            } => {
                let problem_vars = bind_variables_in_set(&from, &possible_problem_vars);
                let mut filtered_from = from.clone();
                unbind_variables(&mut filtered_from, &problem_vars);
                let iter = LeftJoinIterator {
                    eval: self.clone(),
                    right_plan: &*right,
                    left_iter: self.eval_plan(&*left, filtered_from),
                    current_right_iter: None,
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
                let eval = self.clone();
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
                eval: self.clone(),
                children_plan: &children,
                input_iter: self.eval_plan(&*entry, from),
                current_iters: Vec::default(),
            }),
            PlanNode::Extend {
                child,
                position,
                expression,
            } => {
                let eval = self.clone();
                Box::new(
                    self.eval_plan(&*child, from)
                        .filter_map(move |tuple| match tuple {
                            Ok(mut tuple) => {
                                put_value(
                                    *position,
                                    eval.eval_expression(&expression, &tuple)?,
                                    &mut tuple,
                                );
                                Some(Ok(tuple))
                            }
                            Err(error) => Some(Err(error)),
                        }),
                )
            }
            PlanNode::HashDeduplicate { child } => {
                let iter = self.eval_plan(&*child, from);
                let mut values = HashSet::with_capacity(iter.size_hint().0);
                let mut errors = Vec::default();
                for result in iter {
                    match result {
                        Ok(result) => {
                            values.insert(result);
                        }
                        Err(error) => errors.push(Err(error)),
                    }
                }
                Box::new(errors.into_iter().chain(values.into_iter().map(Ok)))
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

    fn eval_expression(
        &self,
        expression: &PlanExpression,
        tuple: &[Option<EncodedTerm>],
    ) -> Option<EncodedTerm> {
        match expression {
            PlanExpression::Constant(t) => Some(*t),
            PlanExpression::Variable(v) => get_tuple_value(*v, tuple),
            PlanExpression::Or(a, b) => match self.to_bool(self.eval_expression(a, tuple)?) {
                Some(true) => Some(true.into()),
                Some(false) => self.eval_expression(b, tuple),
                None => match self.to_bool(self.eval_expression(b, tuple)?) {
                    Some(true) => Some(true.into()),
                    _ => None,
                },
            },
            PlanExpression::And(a, b) => match self.to_bool(self.eval_expression(a, tuple)?) {
                Some(true) => self.eval_expression(b, tuple),
                Some(false) => Some(false.into()),
                None => match self.to_bool(self.eval_expression(b, tuple)?) {
                    Some(false) => Some(false.into()),
                    _ => None,
                },
            },
            PlanExpression::Equal(a, b) => {
                let a = self.eval_expression(a, tuple)?;
                let b = self.eval_expression(b, tuple)?;
                Some((a == b || self.partial_cmp_terms(a, b) == Some(Ordering::Equal)).into())
            }
            PlanExpression::NotEqual(a, b) => {
                let a = self.eval_expression(a, tuple)?;
                let b = self.eval_expression(b, tuple)?;
                Some((a != b && self.partial_cmp_terms(a, b) != Some(Ordering::Equal)).into())
            }
            PlanExpression::Greater(a, b) => Some(
                (self.partial_cmp_terms(
                    self.eval_expression(a, tuple)?,
                    self.eval_expression(b, tuple)?,
                )? == Ordering::Greater)
                    .into(),
            ),
            PlanExpression::GreaterOrEq(a, b) => Some(
                match self.partial_cmp_terms(
                    self.eval_expression(a, tuple)?,
                    self.eval_expression(b, tuple)?,
                )? {
                    Ordering::Greater | Ordering::Equal => true,
                    _ => false,
                }.into(),
            ),
            PlanExpression::Lower(a, b) => Some(
                (self.partial_cmp_terms(
                    self.eval_expression(a, tuple)?,
                    self.eval_expression(b, tuple)?,
                )? == Ordering::Less)
                    .into(),
            ),
            PlanExpression::LowerOrEq(a, b) => Some(
                match self.partial_cmp_terms(
                    self.eval_expression(a, tuple)?,
                    self.eval_expression(b, tuple)?,
                )? {
                    Ordering::Less | Ordering::Equal => true,
                    _ => false,
                }.into(),
            ),
            PlanExpression::Add(a, b) => Some(match self.parse_numeric_operands(a, b, tuple)? {
                NumericBinaryOperands::Float(v1, v2) => (v1 + v2).into(),
                NumericBinaryOperands::Double(v1, v2) => (v1 + v2).into(),
                NumericBinaryOperands::Integer(v1, v2) => (v1 + v2).into(),
                NumericBinaryOperands::Decimal(v1, v2) => (v1 + v2).into(),
            }),
            PlanExpression::Sub(a, b) => Some(match self.parse_numeric_operands(a, b, tuple)? {
                NumericBinaryOperands::Float(v1, v2) => (v1 - v2).into(),
                NumericBinaryOperands::Double(v1, v2) => (v1 - v2).into(),
                NumericBinaryOperands::Integer(v1, v2) => (v1 - v2).into(),
                NumericBinaryOperands::Decimal(v1, v2) => (v1 - v2).into(),
            }),
            PlanExpression::Mul(a, b) => Some(match self.parse_numeric_operands(a, b, tuple)? {
                NumericBinaryOperands::Float(v1, v2) => (v1 * v2).into(),
                NumericBinaryOperands::Double(v1, v2) => (v1 * v2).into(),
                NumericBinaryOperands::Integer(v1, v2) => (v1 * v2).into(),
                NumericBinaryOperands::Decimal(v1, v2) => (v1 * v2).into(),
            }),
            PlanExpression::Div(a, b) => Some(match self.parse_numeric_operands(a, b, tuple)? {
                NumericBinaryOperands::Float(v1, v2) => (v1 / v2).into(),
                NumericBinaryOperands::Double(v1, v2) => (v1 / v2).into(),
                NumericBinaryOperands::Integer(v1, v2) => (v1 / v2).into(),
                NumericBinaryOperands::Decimal(v1, v2) => (v1 / v2).into(),
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
            PlanExpression::Str(e) => Some(EncodedTerm::SimpleLiteral {
                value_id: self.to_string_id(self.eval_expression(e, tuple)?)?,
            }),
            PlanExpression::Lang(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::LangStringLiteral { language_id, .. } => {
                    Some(EncodedTerm::SimpleLiteral {
                        value_id: language_id,
                    })
                }
                e if e.is_literal() => Some(ENCODED_EMPTY_SIMPLE_LITERAL),
                _ => None,
            },
            PlanExpression::Datatype(e) => self.eval_expression(e, tuple)?.datatype(),
            PlanExpression::Bound(v) => Some(has_tuple_value(*v, tuple).into()),
            PlanExpression::IRI(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::NamedNode { iri_id } => Some(EncodedTerm::NamedNode { iri_id }),
                EncodedTerm::SimpleLiteral { value_id }
                | EncodedTerm::StringLiteral { value_id } => {
                    Some(EncodedTerm::NamedNode { iri_id: value_id })
                }
                _ => None,
            },
            PlanExpression::BNode(id) => match id {
                Some(id) => unimplemented!(),
                None => Some(BlankNode::default().into()),
            },
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
                }.into(),
            ),
            PlanExpression::LangMatches(language_tag, language_range) => {
                let language_tag =
                    self.to_simple_string(self.eval_expression(language_tag, tuple)?)?;
                let language_range =
                    self.to_simple_string(self.eval_expression(language_range, tuple)?)?;
                Some(
                    if language_range == "*" {
                        !language_tag.is_empty()
                    } else {
                        LanguageTag::from_str(&language_range)
                            .ok()?
                            .matches(&LanguageTag::from_str(&language_tag).ok()?)
                    }.into(),
                )
            }
            PlanExpression::Regex(text, pattern, flags) => unimplemented!(),
            PlanExpression::BooleanCast(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::BooleanLiteral(value) => Some(value.into()),
                EncodedTerm::SimpleLiteral { value_id }
                | EncodedTerm::StringLiteral { value_id } => {
                    match &*self.store.get_bytes(value_id).ok()?? {
                        b"true" | b"1" => Some(true.into()),
                        b"false" | b"0" => Some(false.into()),
                        _ => None,
                    }
                }
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
                EncodedTerm::SimpleLiteral { value_id }
                | EncodedTerm::StringLiteral { value_id } => {
                    Some(EncodedTerm::DoubleLiteral(OrderedFloat(
                        str::from_utf8(&self.store.get_bytes(value_id).ok()??)
                            .ok()?
                            .parse()
                            .ok()?,
                    )))
                }
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
                EncodedTerm::SimpleLiteral { value_id }
                | EncodedTerm::StringLiteral { value_id } => {
                    Some(EncodedTerm::FloatLiteral(OrderedFloat(
                        str::from_utf8(&self.store.get_bytes(value_id).ok()??)
                            .ok()?
                            .parse()
                            .ok()?,
                    )))
                }
                _ => None,
            },
            PlanExpression::IntegerCast(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::FloatLiteral(value) => Some(value.to_i128()?.into()),
                EncodedTerm::DoubleLiteral(value) => Some(value.to_i128()?.into()),
                EncodedTerm::IntegerLiteral(value) => Some(value.to_i128()?.into()),
                EncodedTerm::DecimalLiteral(value) => Some(value.to_i128()?.into()),
                EncodedTerm::BooleanLiteral(value) => Some(if value { 1 } else { 0 }.into()),
                EncodedTerm::SimpleLiteral { value_id }
                | EncodedTerm::StringLiteral { value_id } => Some(EncodedTerm::IntegerLiteral(
                    str::from_utf8(&self.store.get_bytes(value_id).ok()??)
                        .ok()?
                        .parse()
                        .ok()?,
                )),
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
                    }.into(),
                ),
                EncodedTerm::SimpleLiteral { value_id }
                | EncodedTerm::StringLiteral { value_id } => Some(EncodedTerm::DecimalLiteral(
                    str::from_utf8(&self.store.get_bytes(value_id).ok()??)
                        .ok()?
                        .parse()
                        .ok()?,
                )),
                _ => None,
            },
            PlanExpression::DateTimeCast(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::DateTime(value) => Some(value.into()),
                EncodedTerm::NaiveDateTime(value) => Some(value.into()),
                EncodedTerm::SimpleLiteral { value_id }
                | EncodedTerm::StringLiteral { value_id } => {
                    let bytes = self.store.get_bytes(value_id).ok()??;
                    let value = str::from_utf8(&bytes).ok()?;
                    Some(match DateTime::parse_from_rfc3339(&value) {
                        Ok(value) => value.into(),
                        Err(_) => NaiveDateTime::parse_from_str(&value, "%Y-%m-%dT%H:%M:%S")
                            .ok()?
                            .into(),
                    })
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
            EncodedTerm::SimpleLiteral { .. } => Some(term != ENCODED_EMPTY_SIMPLE_LITERAL),
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
            EncodedTerm::NamedNode { iri_id } => Some(iri_id),
            EncodedTerm::SimpleLiteral { value_id }
            | EncodedTerm::StringLiteral { value_id }
            | EncodedTerm::LangStringLiteral { value_id, .. }
            | EncodedTerm::TypedLiteral { value_id, .. } => Some(value_id),
            EncodedTerm::BooleanLiteral(value) => self
                .store
                .insert_bytes(if value { b"true" } else { b"false" })
                .ok(),
            EncodedTerm::FloatLiteral(value) => {
                self.store.insert_bytes(value.to_string().as_bytes()).ok()
            }
            EncodedTerm::DoubleLiteral(value) => {
                self.store.insert_bytes(value.to_string().as_bytes()).ok()
            }
            EncodedTerm::IntegerLiteral(value) => {
                self.store.insert_bytes(value.to_string().as_bytes()).ok()
            }
            EncodedTerm::DecimalLiteral(value) => {
                self.store.insert_bytes(value.to_string().as_bytes()).ok()
            }
            _ => None,
        }
    }

    fn to_simple_string(&self, term: EncodedTerm) -> Option<String> {
        if let EncodedTerm::SimpleLiteral { value_id } = term {
            Some(
                str::from_utf8(&self.store.get_bytes(value_id).ok()??)
                    .ok()?
                    .to_owned(),
            )
        } else {
            None
        }
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

    fn decode_bindings<'a>(
        &self,
        iter: EncodedTuplesIterator<'a>,
        variables: Vec<Variable>,
    ) -> BindingsIterator<'a> {
        let store = self.store.clone();
        BindingsIterator::new(
            variables,
            Box::new(iter.map(move |values| {
                let encoder = store.encoder();
                values?
                    .into_iter()
                    .map(|value| {
                        Ok(match value {
                            Some(term) => Some(encoder.decode_term(term)?),
                            None => None,
                        })
                    }).collect()
            })),
        )
    }

    fn partial_cmp_terms(&self, a: EncodedTerm, b: EncodedTerm) -> Option<Ordering> {
        match a {
            EncodedTerm::SimpleLiteral { value_id: a }
            | EncodedTerm::StringLiteral { value_id: a } => match b {
                EncodedTerm::SimpleLiteral { value_id: b }
                | EncodedTerm::StringLiteral { value_id: b } => a.partial_cmp(&b),
                _ => None,
            },
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
            _ => None,
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
    set.into_iter()
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
        for (key, a_value) in a.into_iter().enumerate() {
            if let Some(a_value) = a_value {
                match b[key] {
                    Some(ref b_value) => if a_value != b_value {
                        return None;
                    },
                    None => result[key] = Some(*a_value),
                }
            }
        }
        Some(result)
    } else {
        let mut result = a.to_owned();
        for (key, b_value) in b.into_iter().enumerate() {
            if let Some(b_value) = b_value {
                match a[key] {
                    Some(ref a_value) => if a_value != b_value {
                        return None;
                    },
                    None => result[key] = Some(*b_value),
                }
            }
        }
        Some(result)
    }
}

struct JoinIterator<'a> {
    left: Vec<EncodedTuple>,
    right_iter: EncodedTuplesIterator<'a>,
    buffered_results: Vec<Result<EncodedTuple>>,
}

impl<'a> Iterator for JoinIterator<'a> {
    type Item = Result<EncodedTuple>;

    fn next(&mut self) -> Option<Result<EncodedTuple>> {
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
        self.next()
    }
}

struct LeftJoinIterator<'a, S: EncodedQuadsStore> {
    eval: SimpleEvaluator<S>,
    right_plan: &'a PlanNode,
    left_iter: EncodedTuplesIterator<'a>,
    current_right_iter: Option<EncodedTuplesIterator<'a>>,
}

impl<'a, S: EncodedQuadsStore> Iterator for LeftJoinIterator<'a, S> {
    type Item = Result<EncodedTuple>;

    fn next(&mut self) -> Option<Result<EncodedTuple>> {
        if let Some(ref mut right_iter) = self.current_right_iter {
            if let Some(tuple) = right_iter.next() {
                return Some(tuple);
            }
        }
        match self.left_iter.next()? {
            Ok(left_tuple) => {
                let mut right_iter = self.eval.eval_plan(self.right_plan, left_tuple.clone());
                match right_iter.next() {
                    Some(right_tuple) => {
                        self.current_right_iter = Some(right_iter);
                        Some(right_tuple)
                    }
                    None => Some(Ok(left_tuple)),
                }
            }
            Err(error) => Some(Err(error)),
        }
    }
}

struct BadLeftJoinIterator<'a, S: EncodedQuadsStore> {
    input: EncodedTuple,
    iter: LeftJoinIterator<'a, S>,
    problem_vars: Vec<usize>,
}

impl<'a, S: EncodedQuadsStore> Iterator for BadLeftJoinIterator<'a, S> {
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

struct UnionIterator<'a, S: EncodedQuadsStore> {
    eval: SimpleEvaluator<S>,
    children_plan: &'a Vec<PlanNode>,
    input_iter: EncodedTuplesIterator<'a>,
    current_iters: Vec<EncodedTuplesIterator<'a>>,
}

impl<'a, S: EncodedQuadsStore> Iterator for UnionIterator<'a, S> {
    type Item = Result<EncodedTuple>;

    fn next(&mut self) -> Option<Result<EncodedTuple>> {
        while let Some(mut iter) = self.current_iters.pop() {
            if let Some(tuple) = iter.next() {
                self.current_iters.push(iter);
                return Some(tuple);
            }
        }
        match self.input_iter.next()? {
            Ok(input_tuple) => {
                for plan in self.children_plan {
                    self.current_iters
                        .push(self.eval.eval_plan(plan, input_tuple.clone()));
                }
            }
            Err(error) => return Some(Err(error)),
        }
        self.next()
    }
}
