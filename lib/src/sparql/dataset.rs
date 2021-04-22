use crate::sparql::algebra::QueryDataset;
use crate::sparql::EvaluationError;
use crate::storage::numeric_encoder::{
    EncodedQuad, EncodedTerm, ReadEncoder, StrContainer, StrHash, StrLookup,
};
use crate::storage::Storage;
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::iter::empty;

pub(crate) struct DatasetView {
    storage: Storage,
    extra: RefCell<HashMap<StrHash, String>>,
    dataset: EncodedDatasetSpec,
}

impl DatasetView {
    pub fn new(storage: Storage, dataset: &QueryDataset) -> Result<Self, EvaluationError> {
        let dataset = EncodedDatasetSpec {
            default: dataset
                .default_graph_graphs()
                .map(|graphs| {
                    graphs
                        .iter()
                        .flat_map(|g| storage.get_encoded_graph_name(g.as_ref()).transpose())
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?,
            named: dataset
                .available_named_graphs()
                .map(|graphs| {
                    graphs
                        .iter()
                        .flat_map(|g| {
                            storage
                                .get_encoded_named_or_blank_node(g.as_ref())
                                .transpose()
                        })
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose()?,
        };
        Ok(Self {
            storage,
            extra: RefCell::new(HashMap::default()),
            dataset,
        })
    }

    fn store_encoded_quads_for_pattern(
        &self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> impl Iterator<Item = Result<EncodedQuad, EvaluationError>> + 'static {
        self.storage
            .quads_for_pattern(subject, predicate, object, graph_name)
            .map(|t| t.map_err(|e| e.into()))
    }

    #[allow(clippy::needless_collect)]
    pub fn encoded_quads_for_pattern(
        &self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
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
                                Some(default_graph_graphs[0]),
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
                                    Some(*graph_name),
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
                    Box::new(self.store_encoded_quads_for_pattern(subject, predicate, object, None))
                }
            } else if self
                .dataset
                .named
                .as_ref()
                .map_or(true, |d| d.contains(&graph_name))
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
                        Some(*graph_name),
                    )
                })
                .collect::<Vec<_>>();
            Box::new(iters.into_iter().flatten())
        } else {
            Box::new(
                self.store_encoded_quads_for_pattern(subject, predicate, object, None)
                    .filter(|quad| match quad {
                        Err(_) => true,
                        Ok(quad) => quad.graph_name != EncodedTerm::DefaultGraph,
                    }),
            )
        }
    }
}

impl StrLookup for DatasetView {
    type Error = EvaluationError;

    fn get_str(&self, key: StrHash) -> Result<Option<String>, EvaluationError> {
        Ok(if let Some(value) = self.extra.borrow().get(&key) {
            Some(value.clone())
        } else {
            self.storage.get_str(key)?
        })
    }

    fn contains_str(&self, key: StrHash) -> Result<bool, EvaluationError> {
        Ok(self.extra.borrow().contains_key(&key) || self.storage.contains_str(key)?)
    }
}

impl StrContainer for DatasetView {
    fn insert_str(&self, key: StrHash, value: &str) -> Result<bool, EvaluationError> {
        if self.storage.contains_str(key)? {
            Ok(false)
        } else {
            match self.extra.borrow_mut().entry(key) {
                Entry::Occupied(_) => Ok(false),
                Entry::Vacant(entry) => {
                    entry.insert(value.to_owned());
                    Ok(true)
                }
            }
        }
    }
}

struct EncodedDatasetSpec {
    default: Option<Vec<EncodedTerm>>,
    named: Option<Vec<EncodedTerm>>,
}
