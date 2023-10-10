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
use std::rc::Rc;

/// Boundry between the query evaluator and the storage layer.
pub trait DatasetView: Clone {
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

/// Blanket implementation for references to DatasetView.
impl<T: DatasetView> DatasetView for &T {
    // Implementation based on
    //
    // Gjengset, Jon. 2022. “Ergonomic Trait Implementations.” In Rust
    // for Rustaceans, 40–40. San Francisco, CA: No Starch Press.
    //
    // Smith, Mark. 2023. “Rust Trait Implementations and References.”
    // Judy2k’s Blog (blog). February 22,
    // 2023. https://www.judy.co.uk/blog/rust-traits-and-references/.

    fn encoded_quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad, EvaluationError>>> {
        return (**self).encoded_quads_for_pattern(subject, predicate, object, graph_name);
    }

    fn encode_term<'a>(&self, term: impl Into<TermRef<'a>>) -> EncodedTerm {
        return (**self).encode_term(term);
    }

    fn insert_str(&self, key: &StrHash, value: &str) {
        return (**self).insert_str(key, value);
    }
}

/// Blanket implementation for Rc<T> references to DatasetView.
impl<T: DatasetView> DatasetView for Rc<T> {
    // Implementation based on
    //
    // Gjengset, Jon. 2022. “Ergonomic Trait Implementations.” In Rust
    // for Rustaceans, 40–40. San Francisco, CA: No Starch Press.
    //
    // Smith, Mark. 2023. “Rust Trait Implementations and References.”
    // Judy2k’s Blog (blog). February 22,
    // 2023. https://www.judy.co.uk/blog/rust-traits-and-references/.

    fn encoded_quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad, EvaluationError>>> {
        return (**self).encoded_quads_for_pattern(subject, predicate, object, graph_name);
    }

    fn encode_term<'a>(&self, term: impl Into<TermRef<'a>>) -> EncodedTerm {
        return (**self).encode_term(term);
    }

    fn insert_str(&self, key: &StrHash, value: &str) {
        return (**self).insert_str(key, value);
    }
}

/// Boundry over a Header-Dictionary-Triplies (HDT) storage layer.
// #[derive(Clone)]
// pub struct HDTDatasetView {
// }

// https://w3c.github.io/rdf-tests/sparql/sparql11/csv-tsv-res/data.ttl
// impl DatasetView for HDTDatasetView {

//     fn encoded_quads_for_pattern(
//         &self,
//         subject: Option<&EncodedTerm>,
//         predicate: Option<&EncodedTerm>,
//         object: Option<&EncodedTerm>,
//         graph_name: Option<&EncodedTerm>,
//     ) -> Box<dyn Iterator<Item = Result<EncodedQuad, EvaluationError>>> {

//     }

//     fn encode_term<'a>(&self, term: impl Into<TermRef<'a>>) -> EncodedTerm {

//     }

//     fn insert_str(&self, key: &StrHash, value: &str) {

//     }
// }

// impl StrLookup for HDTDatasetView {
//     fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {

//     }

//     fn contains_str(&self, key: &StrHash) -> Result<bool, StorageError> {

//     }
// }

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
        // println!(
        //     "dataset: encoded_quads_for_pattern {:#?} {:#?} {:#?} {:#?}",
        //     subject, predicate, object, graph_name
        // );

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
        // println!("dataset: insert_str {:#?} {:#?}", &key, &value);

        if let Entry::Vacant(e) = self.extra.borrow_mut().entry(*key) {
            if !matches!(self.reader.contains_str(key), Ok(true)) {
                e.insert(value.to_owned());
            }
        }
    }

    fn encode_term<'a>(&self, term: impl Into<TermRef<'a>>) -> EncodedTerm {
        let term = term.into();
        let encoded = term.into();

        // println!(
        //     "dataset: encode_term term {:#?} encoded {:#?}",
        //     &term, &encoded
        // );

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
        // println!("dataset: get_str {:#?}", &key);

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
