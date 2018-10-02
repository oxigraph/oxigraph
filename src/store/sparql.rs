use errors::*;
use sparql::algebra::*;
use std::iter::once;
use std::iter::Iterator;
use std::sync::Arc;
use store::numeric_encoder::EncodedTerm;
use store::store::EncodedQuadsStore;

type EncodedBinding = Vec<Option<EncodedTerm>>;

struct EncodedBindingsIterator {
    variables: Vec<Variable>,
    iter: Box<dyn Iterator<Item = Result<EncodedBinding>>>,
}

impl EncodedBindingsIterator {
    fn take(self, n: usize) -> Self {
        EncodedBindingsIterator {
            variables: self.variables,
            iter: Box::new(self.iter.take(n)),
        }
    }

    fn skip(self, n: usize) -> Self {
        EncodedBindingsIterator {
            variables: self.variables,
            iter: Box::new(self.iter.skip(n)),
        }
    }

    fn project(self, on_variables: Vec<Variable>) -> Self {
        let EncodedBindingsIterator { variables, iter } = self;
        let projection: Vec<(usize, usize)> = on_variables
            .iter()
            .enumerate()
            .flat_map(|(new_pos, v)| slice_key(&variables, v).map(|old_pos| (old_pos, new_pos)))
            .collect();
        let new_len = on_variables.len();
        EncodedBindingsIterator {
            variables: on_variables,
            iter: Box::new(iter.map(move |binding| {
                let binding = binding?;
                let mut new_binding = Vec::with_capacity(new_len);
                new_binding.resize(new_len, None);
                for (old_pos, new_pos) in &projection {
                    new_binding[*new_pos] = binding[*old_pos];
                }
                Ok(new_binding)
            })),
        }
    }
}

impl Default for EncodedBindingsIterator {
    fn default() -> Self {
        EncodedBindingsIterator {
            variables: Vec::default(),
            iter: Box::new(once(Ok(Vec::default()))),
        }
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

pub struct SparqlEvaluator<S: EncodedQuadsStore> {
    store: Arc<S>,
}

impl<S: EncodedQuadsStore> SparqlEvaluator<S> {
    pub fn new(store: Arc<S>) -> Self {
        Self { store }
    }

    pub fn evaluate(&self, query: &Query) -> Result<QueryResult> {
        match query {
            Query::SelectQuery { algebra, dataset } => {
                Ok(QueryResult::Bindings(self.decode_bindings(
                    self.eval_list_pattern(algebra, EncodedBindingsIterator::default())?,
                )))
            }
            _ => unimplemented!(),
        }
    }

    fn eval_list_pattern(
        &self,
        pattern: &ListPattern,
        from: EncodedBindingsIterator,
    ) -> Result<EncodedBindingsIterator> {
        match pattern {
            ListPattern::Data(bs) => Ok(self.encode_bindings(bs)),
            ListPattern::ToList(l) => self.eval_multi_set_pattern(l, from),
            ListPattern::OrderBy(l, o) => self.eval_list_pattern(l, from), //TODO
            ListPattern::Project(l, new_variables) => Ok(self
                .eval_list_pattern(l, from)?
                .project(new_variables.to_vec())),
            ListPattern::Distinct(l) => self.eval_list_pattern(l, from), //TODO
            ListPattern::Reduced(l) => self.eval_list_pattern(l, from),
            ListPattern::Slice(l, start, length) => {
                let mut iter = self.eval_list_pattern(l, from)?;
                if *start > 0 {
                    iter = iter.skip(*start);
                }
                if let Some(length) = length {
                    iter = iter.take(*length);
                }
                Ok(iter)
            }
        }
    }

    fn eval_multi_set_pattern(
        &self,
        pattern: &MultiSetPattern,
        from: EncodedBindingsIterator,
    ) -> Result<EncodedBindingsIterator> {
        match pattern {
            MultiSetPattern::BGP(p) => {
                let mut iter = from;
                for pattern in p {
                    iter = match pattern {
                        TripleOrPathPattern::Triple(pattern) => {
                            self.eval_triple_pattern(pattern, iter)
                        }
                        TripleOrPathPattern::Path(pattern) => self.eval_path_pattern(pattern, iter),
                    }?;
                }
                Ok(iter)
            }
            MultiSetPattern::Join(a, b) => {
                self.eval_multi_set_pattern(b, self.eval_multi_set_pattern(a, from)?)
            }
            MultiSetPattern::LeftJoin(a, b, e) => unimplemented!(),
            MultiSetPattern::Filter(e, p) => unimplemented!(),
            MultiSetPattern::Union(a, b) => unimplemented!(),
            MultiSetPattern::Graph(g, p) => unimplemented!(),
            MultiSetPattern::Extend(p, v, e) => unimplemented!(),
            MultiSetPattern::Minus(a, b) => unimplemented!(),
            MultiSetPattern::ToMultiSet(l) => self.eval_list_pattern(l, from),
            MultiSetPattern::Service(n, p, s) => unimplemented!(),
            MultiSetPattern::AggregateJoin(g, a) => unimplemented!(),
        }
    }

    fn eval_triple_pattern(
        &self,
        pattern: &TriplePattern,
        from: EncodedBindingsIterator,
    ) -> Result<EncodedBindingsIterator> {
        let EncodedBindingsIterator {
            mut variables,
            iter: from_iter,
        } = from;
        let subject =
            self.binding_value_lookup_from_term_or_variable(&pattern.subject, &mut variables)?;
        let predicate = self
            .binding_value_lookup_from_named_node_or_variable(&pattern.predicate, &mut variables)?;
        let object =
            self.binding_value_lookup_from_term_or_variable(&pattern.object, &mut variables)?;

        let filter_sp = subject.is_var() && subject == predicate;
        let filter_so = subject.is_var() && subject == object;
        let filter_po = predicate.is_var() && predicate == object;

        let store = self.store.clone();
        let variables_len = variables.len();
        Ok(EncodedBindingsIterator {
            variables,
            iter: Box::new(from_iter.flat_map(move |binding| {
                let result: Box<dyn Iterator<Item = Result<EncodedBinding>>> = match binding {
                    Ok(mut binding) => {
                        match store.quads_for_pattern(
                            subject.get(&binding),
                            predicate.get(&binding),
                            object.get(&binding),
                            None, //TODO
                        ) {
                            Ok(mut iter) => {
                                if filter_sp {
                                    iter = Box::new(iter.filter(|quad| match quad {
                                        Err(_) => true,
                                        Ok(quad) => quad.subject == quad.predicate,
                                    }))
                                }
                                if filter_so {
                                    iter = Box::new(iter.filter(|quad| match quad {
                                        Err(_) => true,
                                        Ok(quad) => quad.subject == quad.object,
                                    }))
                                }
                                if filter_po {
                                    iter = Box::new(iter.filter(|quad| match quad {
                                        Err(_) => true,
                                        Ok(quad) => quad.predicate == quad.object,
                                    }))
                                }
                                Box::new(iter.map(move |quad| {
                                    let quad = quad?;
                                    let mut binding = binding.clone();
                                    binding.resize(variables_len, None);
                                    subject.put(quad.subject, &mut binding);
                                    predicate.put(quad.predicate, &mut binding);
                                    object.put(quad.object, &mut binding);
                                    Ok(binding)
                                }))
                            }
                            Err(error) => Box::new(once(Err(error))),
                        }
                    }
                    Err(error) => Box::new(once(Err(error))),
                };
                result
            })),
        })
    }

    fn eval_path_pattern(
        &self,
        pattern: &PathPattern,
        from: EncodedBindingsIterator,
    ) -> Result<EncodedBindingsIterator> {
        unimplemented!()
    }

    fn binding_value_lookup_from_term_or_variable(
        &self,
        term_or_variable: &TermOrVariable,
        variables: &mut Vec<Variable>,
    ) -> Result<BindingValueLookup> {
        Ok(match term_or_variable {
            TermOrVariable::Term(term) => {
                BindingValueLookup::Constant(self.store.encoder().encode_term(term)?)
            }
            TermOrVariable::Variable(variable) => {
                BindingValueLookup::Variable(match slice_key(variables, variable) {
                    Some(key) => key,
                    None => {
                        variables.push(variable.clone());
                        variables.len() - 1
                    }
                })
            }
        })
    }

    fn binding_value_lookup_from_named_node_or_variable(
        &self,
        named_node_or_variable: &NamedNodeOrVariable,
        variables: &mut Vec<Variable>,
    ) -> Result<BindingValueLookup> {
        Ok(match named_node_or_variable {
            NamedNodeOrVariable::NamedNode(named_node) => {
                BindingValueLookup::Constant(self.store.encoder().encode_named_node(named_node)?)
            }
            NamedNodeOrVariable::Variable(variable) => {
                BindingValueLookup::Variable(match slice_key(variables, variable) {
                    Some(key) => key,
                    None => {
                        variables.push(variable.clone());
                        variables.len() - 1
                    }
                })
            }
        })
    }

    fn encode_bindings(&self, bindings: &StaticBindings) -> EncodedBindingsIterator {
        let encoder = self.store.encoder();
        let encoded_values: Vec<Result<EncodedBinding>> = bindings
            .values_iter()
            .map(move |values| {
                let mut result = Vec::with_capacity(values.len());
                for value in values {
                    result.push(match value {
                        Some(term) => Some(encoder.encode_term(term)?),
                        None => None,
                    });
                }
                Ok(result)
            }).collect();
        EncodedBindingsIterator {
            variables: bindings.variables().to_vec(),
            iter: Box::new(encoded_values.into_iter()),
        }
    }

    fn decode_bindings(&self, iter: EncodedBindingsIterator) -> BindingsIterator {
        let store = self.store.clone();
        let EncodedBindingsIterator { variables, iter } = iter;
        BindingsIterator::new(
            variables,
            Box::new(iter.map(move |values| {
                let values = values?;
                let encoder = store.encoder();
                let mut result = Vec::with_capacity(values.len());
                for value in values {
                    result.push(match value {
                        Some(term) => Some(encoder.decode_term(term)?),
                        None => None,
                    });
                }
                Ok(result)
            })),
        )
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum BindingValueLookup {
    Constant(EncodedTerm),
    Variable(usize),
}

impl BindingValueLookup {
    fn get(&self, binding: &[Option<EncodedTerm>]) -> Option<EncodedTerm> {
        match self {
            BindingValueLookup::Constant(term) => Some(*term),
            BindingValueLookup::Variable(v) => if *v < binding.len() {
                binding[*v]
            } else {
                None
            },
        }
    }

    fn put(&self, value: EncodedTerm, binding: &mut EncodedBinding) {
        match self {
            BindingValueLookup::Constant(_) => (),
            BindingValueLookup::Variable(v) => binding[*v] = Some(value),
        }
    }

    fn is_var(&self) -> bool {
        match self {
            BindingValueLookup::Constant(_) => false,
            BindingValueLookup::Variable(_) => true,
        }
    }
}
