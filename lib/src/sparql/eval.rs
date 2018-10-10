use num_traits::identities::Zero;
use num_traits::FromPrimitive;
use num_traits::ToPrimitive;
use rust_decimal::Decimal;
use sparql::algebra::*;
use sparql::plan::*;
use std::collections::HashSet;
use std::iter::once;
use std::iter::Iterator;
use std::sync::Arc;
use store::encoded::EncodedQuadsStore;
use store::numeric_encoder::*;
use Result;

type EncodedTuplesIterator = Box<dyn Iterator<Item = Result<EncodedTuple>>>;

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

    pub fn evaluate(&self, query: &Query) -> Result<QueryResult> {
        match query {
            Query::Select { algebra, dataset } => {
                let (plan, variables) = PlanBuilder::build(&*self.store, algebra)?;
                let iter = self.eval_plan(plan, vec![None; variables.len()]);
                Ok(QueryResult::Bindings(self.decode_bindings(iter, variables)))
            }
            _ => unimplemented!(),
        }
    }

    fn eval_plan(&self, node: PlanNode, from: EncodedTuple) -> EncodedTuplesIterator {
        match node {
            PlanNode::Init => Box::new(once(Ok(from))),
            PlanNode::StaticBindings { tuples } => Box::new(tuples.into_iter().map(Ok)),
            PlanNode::QuadPatternJoin {
                child,
                subject,
                predicate,
                object,
                graph_name,
            } => {
                let eval = self.clone();
                Box::new(
                    self.eval_plan(*child, from)
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
            PlanNode::Filter { child, expression } => {
                let eval = self.clone();
                Box::new(self.eval_plan(*child, from).filter(move |tuple| {
                    match tuple {
                        Ok(tuple) => eval
                            .eval_expression(&expression, tuple)
                            .and_then(|term| eval.to_bool(term))
                            .unwrap_or(false),
                        Err(_) => true,
                    }
                }))
            }
            PlanNode::Union { entry, children } => {
                //TODO: avoid clones
                let eval = self.clone();
                Box::new(self.eval_plan(*entry, from).flat_map(move |tuple| {
                    let eval = eval.clone();
                    let iter: EncodedTuplesIterator = match tuple {
                        Ok(tuple) => Box::new(
                            children
                                .clone()
                                .into_iter()
                                .flat_map(move |child| eval.eval_plan(child, tuple.clone())),
                        ),
                        Err(error) => Box::new(once(Err(error))),
                    };
                    iter
                }))
            }
            PlanNode::Extend {
                child,
                position,
                expression,
            } => {
                let eval = self.clone();
                Box::new(
                    self.eval_plan(*child, from)
                        .filter_map(move |tuple| match tuple {
                            Ok(mut tuple) => {
                                put_value(
                                    position,
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
                let iter = self.eval_plan(*child, from);
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
            PlanNode::Skip { child, count } => Box::new(self.eval_plan(*child, from).skip(count)),
            PlanNode::Limit { child, count } => Box::new(self.eval_plan(*child, from).take(count)),
            PlanNode::Project { child, mapping } => {
                Box::new(self.eval_plan(*child, from).map(move |tuple| {
                    let tuple = tuple?;
                    let mut new_tuple = Vec::with_capacity(mapping.len());
                    for key in &mapping {
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
            PlanExpression::Variable(v) => if *v < tuple.len() {
                tuple[*v]
            } else {
                None
            },
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
                Some((self.eval_expression(a, tuple)? == self.eval_expression(b, tuple)?).into())
            }
            PlanExpression::NotEqual(a, b) => {
                Some((self.eval_expression(a, tuple)? != self.eval_expression(b, tuple)?).into())
            }
            PlanExpression::Greater(a, b) => {
                Some((self.eval_expression(a, tuple)? > self.eval_expression(b, tuple)?).into())
            }
            PlanExpression::GreaterOrEq(a, b) => {
                Some((self.eval_expression(a, tuple)? >= self.eval_expression(b, tuple)?).into())
            }
            PlanExpression::Lower(a, b) => {
                Some((self.eval_expression(a, tuple)? < self.eval_expression(b, tuple)?).into())
            }
            PlanExpression::LowerOrEq(a, b) => {
                Some((self.eval_expression(a, tuple)? <= self.eval_expression(b, tuple)?).into())
            }
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
                _ => None,
            },
            PlanExpression::Datatype(e) => self.eval_expression(e, tuple)?.datatype(),
            PlanExpression::Bound(v) => Some((*v >= tuple.len() && tuple[*v].is_some()).into()),
            PlanExpression::IRI(e) => match self.eval_expression(e, tuple)? {
                EncodedTerm::NamedNode { iri_id } => Some(EncodedTerm::NamedNode { iri_id }),
                EncodedTerm::SimpleLiteral { value_id }
                | EncodedTerm::StringLiteral { value_id } => {
                    Some(EncodedTerm::NamedNode { iri_id: value_id })
                }
                _ => None,
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
            PlanExpression::BooleanCast(e) => {
                Some(self.to_bool(self.eval_expression(e, tuple)?)?.into())
            }
            PlanExpression::StringCast(e) => Some(EncodedTerm::StringLiteral {
                value_id: self.to_string_id(self.eval_expression(e, tuple)?)?,
            }),
            e => unimplemented!(),
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

    fn decode_bindings(
        &self,
        iter: EncodedTuplesIterator,
        variables: Vec<Variable>,
    ) -> BindingsIterator {
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
}

enum NumericBinaryOperands {
    Float(f32, f32),
    Double(f64, f64),
    Integer(i128, i128),
    Decimal(Decimal, Decimal),
}

fn get_pattern_value(
    selector: &PatternValue,
    tuple: &[Option<EncodedTerm>],
) -> Option<EncodedTerm> {
    match selector {
        PatternValue::Constant(term) => Some(*term),
        PatternValue::Variable(v) => if *v < tuple.len() {
            tuple[*v]
        } else {
            None
        },
    }
}

fn put_pattern_value(selector: &PatternValue, value: EncodedTerm, tuple: &mut EncodedTuple) {
    match selector {
        PatternValue::Constant(_) => (),
        PatternValue::Variable(v) => put_value(*v, value, tuple),
    }
}

fn put_value(position: usize, value: EncodedTerm, tuple: &mut EncodedTuple) {
    if tuple.len() > position {
        tuple[position] = Some(value)
    } else {
        if tuple.len() < position {
            tuple.resize(position, None);
        }
        tuple.push(Some(value))
    }
}
