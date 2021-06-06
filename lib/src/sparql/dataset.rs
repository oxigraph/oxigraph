use crate::model::TermRef;
use crate::sparql::algebra::QueryDataset;
use crate::sparql::EvaluationError;
use crate::storage::numeric_encoder::{EncodedQuad, EncodedTerm, StrHash, StrLookup};
use crate::storage::Storage;
use std::cell::RefCell;
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
                .map(|graphs| graphs.iter().map(|g| g.as_ref().into()).collect::<Vec<_>>()),
            named: dataset
                .available_named_graphs()
                .map(|graphs| graphs.iter().map(|g| g.as_ref().into()).collect::<Vec<_>>()),
        };
        Ok(Self {
            storage,
            extra: RefCell::new(HashMap::default()),
            dataset,
        })
    }

    fn store_encoded_quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> impl Iterator<Item = Result<EncodedQuad, EvaluationError>> + 'static {
        self.storage
            .quads_for_pattern(subject, predicate, object, graph_name)
            .map(|t| t.map_err(|e| e.into()))
    }

    #[allow(clippy::needless_collect)]
    pub fn encoded_quads_for_pattern(
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
                    Box::new(self.store_encoded_quads_for_pattern(subject, predicate, object, None))
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
                        Ok(quad) => quad.graph_name != EncodedTerm::DefaultGraph,
                    }),
            )
        }
    }

    pub fn encode_term<'a>(&self, term: impl Into<TermRef<'a>>) -> EncodedTerm {
        let term = term.into();
        let encoded = term.into();
        self.insert_term_values(term, &encoded);
        encoded
    }

    fn insert_term_values(&self, term: TermRef<'_>, encoded: &EncodedTerm) {
        match (term, encoded) {
            (TermRef::NamedNode(node), EncodedTerm::NamedNode { iri_id }) => {
                self.insert_str(iri_id, node.as_str());
            }
            (TermRef::BlankNode(node), EncodedTerm::BigBlankNode { id_id }) => {
                self.insert_str(id_id, node.as_str());
            }
            (TermRef::Literal(literal), EncodedTerm::BigStringLiteral { value_id }) => {
                self.insert_str(value_id, literal.value());
            }
            (
                TermRef::Literal(literal),
                EncodedTerm::SmallBigLangStringLiteral { language_id, .. },
            ) => {
                if let Some(language) = literal.language() {
                    self.insert_str(language_id, language)
                }
            }
            (
                TermRef::Literal(literal),
                EncodedTerm::BigSmallLangStringLiteral { value_id, .. },
            ) => {
                self.insert_str(value_id, literal.value());
            }
            (
                TermRef::Literal(literal),
                EncodedTerm::BigBigLangStringLiteral {
                    value_id,
                    language_id,
                },
            ) => {
                self.insert_str(value_id, literal.value());
                if let Some(language) = literal.language() {
                    self.insert_str(language_id, language)
                }
            }
            (TermRef::Literal(literal), EncodedTerm::SmallTypedLiteral { datatype_id, .. }) => {
                self.insert_str(datatype_id, literal.datatype().as_str());
            }
            (
                TermRef::Literal(literal),
                EncodedTerm::BigTypedLiteral {
                    value_id,
                    datatype_id,
                },
            ) => {
                self.insert_str(value_id, literal.value());
                self.insert_str(datatype_id, literal.datatype().as_str());
            }
            (TermRef::Triple(triple), EncodedTerm::Triple(encoded)) => {
                self.insert_term_values(triple.subject.as_ref().into(), &encoded.subject);
                self.insert_term_values(triple.predicate.as_ref().into(), &encoded.predicate);
                self.insert_term_values(triple.object.as_ref(), &encoded.object);
            }
            _ => (),
        }
    }

    pub fn insert_str(&self, key: &StrHash, value: &str) {
        if matches!(self.storage.contains_str(key), Ok(true)) {
            return;
        }
        self.extra
            .borrow_mut()
            .entry(*key)
            .or_insert_with(|| value.to_owned());
    }
}

impl StrLookup for DatasetView {
    type Error = EvaluationError;

    fn get_str(&self, key: &StrHash) -> Result<Option<String>, EvaluationError> {
        Ok(if let Some(value) = self.extra.borrow().get(key) {
            Some(value.clone())
        } else {
            self.storage.get_str(key)?
        })
    }

    fn contains_str(&self, key: &StrHash) -> Result<bool, EvaluationError> {
        Ok(self.extra.borrow().contains_key(key) || self.storage.contains_str(key)?)
    }
}

struct EncodedDatasetSpec {
    default: Option<Vec<EncodedTerm>>,
    named: Option<Vec<EncodedTerm>>,
}
