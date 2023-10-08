use crate::model::TermRef;
use crate::sparql::algebra::QueryDataset;
use crate::sparql::EvaluationError;
use crate::storage::numeric_encoder::{
    insert_term, EncodedQuad, EncodedTerm, StrHash, StrHashHasher, StrLookup,
};
use crate::storage::{StorageError, StorageReader};
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::hash::BuildHasherDefault;
use std::iter::empty;

/// Boundry between the query evaluator and the storage layer.
pub trait DatasetView: StrLookup + Clone {
    // Polymorphism can be used to enable multiple storage layer
    // options. A static dispatch approach to polymorphism is used to
    // maximally preserve compile-time optimization
    // opportunities. This is in contrast to use of a dynamic dispatch
    // approach that would be necessary with the use of trait
    // objects. See
    // https://doc.rust-lang.org/stable/book/ch17-02-trait-objects.html#trait-objects-perform-dynamic-dispatch
    // and
    // https://doc.rust-lang.org/stable/book/ch10-01-syntax.html#performance-of-code-using-generics
    // for details on the performance consequences of trait objects
    // versus trait bounds on generics. With static dispatch switching
    // storage layers at run-time requires more managment of the
    // variations in code.

    // References using `&dataset` syntax fail to compile with Rust
    // 1.73.0 with the error that the DatasetView trait is not
    // implemented for `Rc<T>`. A work-around is to use `&(*dataset)`
    // syntax to explicitly deference the Rc. See
    // https://doc.rust-lang.org/stable/book/ch15-02-deref.html#implicit-deref-coercions-with-functions-and-methods
    // fro details on automatic deref coercions.

    fn encoded_quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad, EvaluationError>>>;
    fn encode_term<'a>(&self, term: impl Into<TermRef<'a>>) -> EncodedTerm;
    fn insert_str(&self, key: &StrHash, value: &str);
}

/// Boundry over a Key-Value Store storage layer.
#[derive(Clone)]
pub struct KVDatasetView {
    reader: StorageReader,
    extra: RefCell<HashMap<StrHash, String, BuildHasherDefault<StrHashHasher>>>,
    dataset: EncodedDatasetSpec,
}

impl KVDatasetView {
    pub fn new(reader: StorageReader, dataset: &QueryDataset) -> Self {
        let dataset = EncodedDatasetSpec {
            default: dataset
                .default_graph_graphs()
                .map(|graphs| graphs.iter().map(|g| g.as_ref().into()).collect::<Vec<_>>()),
            named: dataset
                .available_named_graphs()
                .map(|graphs| graphs.iter().map(|g| g.as_ref().into()).collect::<Vec<_>>()),
        };
        Self {
            reader,
            extra: RefCell::new(HashMap::default()),
            dataset,
        }
    }

    fn store_encoded_quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> impl Iterator<Item = Result<EncodedQuad, EvaluationError>> + 'static {
        self.reader
            .quads_for_pattern(subject, predicate, object, graph_name)
            .map(|t| t.map_err(Into::into))
    }
}

impl DatasetView for KVDatasetView {
    #[allow(clippy::needless_collect)]
    fn encoded_quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad, EvaluationError>>> {
        if let Some(graph_name) = graph_name {
            if graph_name.is_default_graph() {
                if let Some(default_graph_graphs) = &self.dataset.default {
                    if default_graph_graphs.len() == 1 {
                        // Single graph optimization
                        Box::new(
                            self.store_encoded_quads_for_pattern(
                                subject,
                                predicate,
                                object,
                                Some(&default_graph_graphs[0]),
                            )
                            .map(|quad| {
                                let quad = quad?;
                                Ok(EncodedQuad::new(
                                    quad.subject,
                                    quad.predicate,
                                    quad.object,
                                    EncodedTerm::DefaultGraph,
                                ))
                            }),
                        )
                    } else {
                        let iters = default_graph_graphs
                            .iter()
                            .map(|graph_name| {
                                self.store_encoded_quads_for_pattern(
                                    subject,
                                    predicate,
                                    object,
                                    Some(graph_name),
                                )
                            })
                            .collect::<Vec<_>>();
                        Box::new(iters.into_iter().flatten().map(|quad| {
                            let quad = quad?;
                            Ok(EncodedQuad::new(
                                quad.subject,
                                quad.predicate,
                                quad.object,
                                EncodedTerm::DefaultGraph,
                            ))
                        }))
                    }
                } else {
                    Box::new(
                        self.store_encoded_quads_for_pattern(subject, predicate, object, None)
                            .map(|quad| {
                                let quad = quad?;
                                Ok(EncodedQuad::new(
                                    quad.subject,
                                    quad.predicate,
                                    quad.object,
                                    EncodedTerm::DefaultGraph,
                                ))
                            }),
                    )
                }
            } else if self
                .dataset
                .named
                .as_ref()
                .map_or(true, |d| d.contains(graph_name))
            {
                Box::new(self.store_encoded_quads_for_pattern(
                    subject,
                    predicate,
                    object,
                    Some(graph_name),
                ))
            } else {
                Box::new(empty())
            }
        } else if let Some(named_graphs) = &self.dataset.named {
            let iters = named_graphs
                .iter()
                .map(|graph_name| {
                    self.store_encoded_quads_for_pattern(
                        subject,
                        predicate,
                        object,
                        Some(graph_name),
                    )
                })
                .collect::<Vec<_>>();
            Box::new(iters.into_iter().flatten())
        } else {
            Box::new(
                self.store_encoded_quads_for_pattern(subject, predicate, object, None)
                    .filter(|quad| match quad {
                        Err(_) => true,
                        Ok(quad) => !quad.graph_name.is_default_graph(),
                    }),
            )
        }
    }

    fn insert_str(&self, key: &StrHash, value: &str) {
        if let Entry::Vacant(e) = self.extra.borrow_mut().entry(*key) {
            if !matches!(self.reader.contains_str(key), Ok(true)) {
                e.insert(value.to_owned());
            }
        }
    }

    fn encode_term<'a>(&self, term: impl Into<TermRef<'a>>) -> EncodedTerm {
        let term = term.into();
        let encoded = term.into();
        insert_term(term, &encoded, &mut |key, value| {
            self.insert_str(key, value);
            Ok(())
        })
        .unwrap();
        encoded
    }
}

impl StrLookup for KVDatasetView {
    fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
        Ok(if let Some(value) = self.extra.borrow().get(key) {
            Some(value.clone())
        } else {
            self.reader.get_str(key)?
        })
    }
}

#[derive(Clone)]
struct EncodedDatasetSpec {
    default: Option<Vec<EncodedTerm>>,
    named: Option<Vec<EncodedTerm>>,
}
