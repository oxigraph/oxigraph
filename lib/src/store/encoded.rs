use crate::model::*;
use crate::store::numeric_encoder::*;
use crate::Result;
use std::fmt;
use std::iter::empty;
use std::iter::once;
use std::iter::FromIterator;
use std::iter::Iterator;
use std::sync::Arc;

/// Defines the Store traits that is used to have efficient binary storage

pub trait EncodedQuadsStore: StringStore + Sized + 'static {
    type QuadsIterator: Iterator<Item = Result<EncodedQuad>> + 'static;
    type QuadsForSubjectIterator: Iterator<Item = Result<EncodedQuad>> + 'static;
    type QuadsForSubjectPredicateIterator: Iterator<Item = Result<EncodedQuad>> + 'static;
    type QuadsForSubjectPredicateObjectIterator: Iterator<Item = Result<EncodedQuad>> + 'static;
    type QuadsForSubjectObjectIterator: Iterator<Item = Result<EncodedQuad>> + 'static;
    type QuadsForPredicateIterator: Iterator<Item = Result<EncodedQuad>> + 'static;
    type QuadsForPredicateObjectIterator: Iterator<Item = Result<EncodedQuad>> + 'static;
    type QuadsForObjectIterator: Iterator<Item = Result<EncodedQuad>> + 'static;
    type QuadsForGraphIterator: Iterator<Item = Result<EncodedQuad>> + 'static;
    type QuadsForSubjectGraphIterator: Iterator<Item = Result<EncodedQuad>> + 'static;
    type QuadsForSubjectPredicateGraphIterator: Iterator<Item = Result<EncodedQuad>> + 'static;
    type QuadsForSubjectObjectGraphIterator: Iterator<Item = Result<EncodedQuad>> + 'static;
    type QuadsForPredicateGraphIterator: Iterator<Item = Result<EncodedQuad>> + 'static;
    type QuadsForPredicateObjectGraphIterator: Iterator<Item = Result<EncodedQuad>> + 'static;
    type QuadsForObjectGraphIterator: Iterator<Item = Result<EncodedQuad>> + 'static;

    fn encoder(&self) -> Encoder<&Self> {
        Encoder::new(&self)
    }

    fn quads(&self) -> Result<Self::QuadsIterator>;
    fn quads_for_subject(&self, subject: EncodedTerm) -> Result<Self::QuadsForSubjectIterator>;
    fn quads_for_subject_predicate(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
    ) -> Result<Self::QuadsForSubjectPredicateIterator>;
    fn quads_for_subject_predicate_object(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<Self::QuadsForSubjectPredicateObjectIterator>;
    fn quads_for_subject_object(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<Self::QuadsForSubjectObjectIterator>;
    fn quads_for_predicate(
        &self,
        predicate: EncodedTerm,
    ) -> Result<Self::QuadsForPredicateIterator>;
    fn quads_for_predicate_object(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<Self::QuadsForPredicateObjectIterator>;
    fn quads_for_object(&self, object: EncodedTerm) -> Result<Self::QuadsForObjectIterator>;
    fn quads_for_graph(&self, graph_name: EncodedTerm) -> Result<Self::QuadsForGraphIterator>;
    fn quads_for_subject_graph(
        &self,
        subject: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<Self::QuadsForSubjectGraphIterator>;
    fn quads_for_subject_predicate_graph(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<Self::QuadsForSubjectPredicateGraphIterator>;
    fn quads_for_subject_object_graph(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<Self::QuadsForSubjectObjectGraphIterator>;
    fn quads_for_predicate_graph(
        &self,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<Self::QuadsForPredicateGraphIterator>;
    fn quads_for_predicate_object_graph(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<Self::QuadsForPredicateObjectGraphIterator>;
    fn quads_for_object_graph(
        &self,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<Self::QuadsForObjectGraphIterator>;
    fn contains(&self, quad: &EncodedQuad) -> Result<bool>;
    fn insert(&self, quad: &EncodedQuad) -> Result<()>;
    fn remove(&self, quad: &EncodedQuad) -> Result<()>;
    fn quads_for_pattern(
        &self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad>>> {
        match subject {
            Some(subject) => match predicate {
                Some(predicate) => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => {
                            let quad = EncodedQuad::new(subject, predicate, object, graph_name);
                            match self.contains(&quad) {
                                Ok(true) => Box::new(once(Ok(quad))),
                                Ok(false) => Box::new(empty()),
                                Err(error) => Box::new(once(Err(error))),
                            }
                        }
                        None => wrap_error(
                            self.quads_for_subject_predicate_object(subject, predicate, object),
                        ),
                    },
                    None => match graph_name {
                        Some(graph_name) => wrap_error(
                            self.quads_for_subject_predicate_graph(subject, predicate, graph_name),
                        ),
                        None => wrap_error(self.quads_for_subject_predicate(subject, predicate)),
                    },
                },
                None => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => wrap_error(
                            self.quads_for_subject_object_graph(subject, object, graph_name),
                        ),
                        None => wrap_error(self.quads_for_subject_object(subject, object)),
                    },
                    None => match graph_name {
                        Some(graph_name) => {
                            wrap_error(self.quads_for_subject_graph(subject, graph_name))
                        }
                        None => wrap_error(self.quads_for_subject(subject)),
                    },
                },
            },
            None => match predicate {
                Some(predicate) => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => wrap_error(
                            self.quads_for_predicate_object_graph(predicate, object, graph_name),
                        ),
                        None => wrap_error(self.quads_for_predicate_object(predicate, object)),
                    },
                    None => match graph_name {
                        Some(graph_name) => {
                            wrap_error(self.quads_for_predicate_graph(predicate, graph_name))
                        }
                        None => wrap_error(self.quads_for_predicate(predicate)),
                    },
                },
                None => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => {
                            wrap_error(self.quads_for_object_graph(object, graph_name))
                        }
                        None => wrap_error(self.quads_for_object(object)),
                    },
                    None => match graph_name {
                        Some(graph_name) => wrap_error(self.quads_for_graph(graph_name)),
                        None => wrap_error(self.quads()),
                    },
                },
            },
        }
    }
}

fn wrap_error<E: 'static, I: Iterator<Item = Result<E>> + 'static>(
    iter: Result<I>,
) -> Box<dyn Iterator<Item = Result<E>>> {
    match iter {
        Ok(iter) => Box::new(iter),
        Err(error) => Box::new(once(Err(error))),
    }
}

pub struct StoreDataset<S: EncodedQuadsStore> {
    store: Arc<S>,
}

impl<S: EncodedQuadsStore> StoreDataset<S> {
    pub fn new_from_store(store: S) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    pub(crate) fn encoded(&self) -> Arc<S> {
        self.store.clone()
    }
}

impl<S: EncodedQuadsStore> Dataset for StoreDataset<S> {
    type NamedGraph = StoreNamedGraph<S>;
    type DefaultGraph = StoreDefaultGraph<S>;
    type UnionGraph = StoreUnionGraph<S>;
    type QuadsIterator = QuadsIterator<S::QuadsIterator, S>;
    type QuadsForSubjectIterator = QuadsIterator<S::QuadsForSubjectIterator, S>;
    type QuadsForSubjectPredicateIterator = QuadsIterator<S::QuadsForSubjectPredicateIterator, S>;
    type QuadsForSubjectPredicateObjectIterator =
        QuadsIterator<S::QuadsForSubjectPredicateObjectIterator, S>;
    type QuadsForSubjectObjectIterator = QuadsIterator<S::QuadsForSubjectObjectIterator, S>;
    type QuadsForPredicateIterator = QuadsIterator<S::QuadsForPredicateIterator, S>;
    type QuadsForPredicateObjectIterator = QuadsIterator<S::QuadsForPredicateObjectIterator, S>;
    type QuadsForObjectIterator = QuadsIterator<S::QuadsForObjectIterator, S>;

    fn named_graph(&self, name: &NamedOrBlankNode) -> Result<StoreNamedGraph<S>> {
        Ok(StoreNamedGraph {
            store: self.store.clone(),
            name: name.clone(),
            encoded_name: self.store.encoder().encode_named_or_blank_node(name)?,
        })
    }

    fn default_graph(&self) -> StoreDefaultGraph<S> {
        StoreDefaultGraph {
            store: self.store.clone(),
        }
    }

    fn union_graph(&self) -> StoreUnionGraph<S> {
        StoreUnionGraph {
            store: self.store.clone(),
        }
    }

    fn quads(&self) -> Result<QuadsIterator<S::QuadsIterator, S>> {
        Ok(QuadsIterator {
            iter: self.store.quads()?,
            store: self.store.clone(),
        })
    }

    fn quads_for_subject(
        &self,
        subject: &NamedOrBlankNode,
    ) -> Result<QuadsIterator<S::QuadsForSubjectIterator, S>> {
        let encoder = self.store.encoder();
        Ok(QuadsIterator {
            iter: self
                .store
                .quads_for_subject(encoder.encode_named_or_blank_node(subject)?)?,
            store: self.store.clone(),
        })
    }

    fn quads_for_subject_predicate(
        &self,
        subject: &NamedOrBlankNode,
        predicate: &NamedNode,
    ) -> Result<QuadsIterator<S::QuadsForSubjectPredicateIterator, S>> {
        let encoder = self.store.encoder();
        Ok(QuadsIterator {
            iter: self.store.quads_for_subject_predicate(
                encoder.encode_named_or_blank_node(subject)?,
                encoder.encode_named_node(predicate)?,
            )?,
            store: self.store.clone(),
        })
    }

    fn quads_for_subject_predicate_object(
        &self,
        subject: &NamedOrBlankNode,
        predicate: &NamedNode,
        object: &Term,
    ) -> Result<QuadsIterator<S::QuadsForSubjectPredicateObjectIterator, S>> {
        let encoder = self.store.encoder();
        Ok(QuadsIterator {
            iter: self.store.quads_for_subject_predicate_object(
                encoder.encode_named_or_blank_node(subject)?,
                encoder.encode_named_node(predicate)?,
                encoder.encode_term(object)?,
            )?,
            store: self.store.clone(),
        })
    }

    fn quads_for_subject_object(
        &self,
        subject: &NamedOrBlankNode,
        object: &Term,
    ) -> Result<QuadsIterator<S::QuadsForSubjectObjectIterator, S>> {
        let encoder = self.store.encoder();
        Ok(QuadsIterator {
            iter: self.store.quads_for_subject_object(
                encoder.encode_named_or_blank_node(subject)?,
                encoder.encode_term(object)?,
            )?,
            store: self.store.clone(),
        })
    }

    fn quads_for_predicate(
        &self,
        predicate: &NamedNode,
    ) -> Result<QuadsIterator<S::QuadsForPredicateIterator, S>> {
        let encoder = self.store.encoder();
        Ok(QuadsIterator {
            iter: self
                .store
                .quads_for_predicate(encoder.encode_named_node(predicate)?)?,
            store: self.store.clone(),
        })
    }

    fn quads_for_predicate_object(
        &self,
        predicate: &NamedNode,
        object: &Term,
    ) -> Result<QuadsIterator<S::QuadsForPredicateObjectIterator, S>> {
        let encoder = self.store.encoder();
        Ok(QuadsIterator {
            iter: self.store.quads_for_predicate_object(
                encoder.encode_named_node(predicate)?,
                encoder.encode_term(object)?,
            )?,
            store: self.store.clone(),
        })
    }

    fn quads_for_object(
        &self,
        object: &Term,
    ) -> Result<QuadsIterator<S::QuadsForObjectIterator, S>> {
        let encoder = self.store.encoder();
        Ok(QuadsIterator {
            iter: self.store.quads_for_object(encoder.encode_term(object)?)?,
            store: self.store.clone(),
        })
    }

    fn contains(&self, quad: &Quad) -> Result<bool> {
        self.store
            .contains(&self.store.encoder().encode_quad(quad)?)
    }

    fn insert(&self, quad: &Quad) -> Result<()> {
        self.store.insert(&self.store.encoder().encode_quad(quad)?)
    }

    fn remove(&self, quad: &Quad) -> Result<()> {
        self.store.remove(&self.store.encoder().encode_quad(quad)?)
    }

    fn len(&self) -> Result<usize> {
        Ok(self.store.quads()?.count())
    }

    fn is_empty(&self) -> Result<bool> {
        Ok(self.store.quads()?.any(|_| true))
    }
}

impl<S: EncodedQuadsStore> fmt::Display for StoreDataset<S> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        for quad in self.iter().map_err(|_| fmt::Error)? {
            writeln!(fmt, "{}", quad.map_err(|_| fmt::Error)?)?;
        }
        Ok(())
    }
}

impl<S: EncodedQuadsStore + Default> Default for StoreDataset<S> {
    fn default() -> Self {
        Self::new_from_store(S::default())
    }
}

impl<S: EncodedQuadsStore + Default> FromIterator<Quad> for StoreDataset<S> {
    fn from_iter<I: IntoIterator<Item = Quad>>(iter: I) -> Self {
        let dataset = Self::default();
        for quad in iter {
            dataset.insert(&quad).unwrap();
        }
        dataset
    }
}

impl<'a, S: EncodedQuadsStore + Default> FromIterator<&'a Quad> for StoreDataset<S> {
    fn from_iter<I: IntoIterator<Item = &'a Quad>>(iter: I) -> Self {
        let dataset = Self::default();
        for quad in iter {
            dataset.insert(quad).unwrap();
        }
        dataset
    }
}

pub struct StoreNamedGraph<S: EncodedQuadsStore> {
    store: Arc<S>,
    name: NamedOrBlankNode,
    encoded_name: EncodedTerm,
}

impl<S: EncodedQuadsStore> Graph for StoreNamedGraph<S> {
    type TriplesIterator = TriplesIterator<S::QuadsForGraphIterator, S>;
    type TriplesForSubjectIterator = TriplesIterator<S::QuadsForSubjectGraphIterator, S>;
    type ObjectsForSubjectPredicateIterator =
        ObjectsIterator<S::QuadsForSubjectPredicateGraphIterator, S>;
    type PredicatesForSubjectObjectIterator =
        PredicatesIterator<S::QuadsForSubjectObjectGraphIterator, S>;
    type TriplesForPredicateIterator = TriplesIterator<S::QuadsForPredicateGraphIterator, S>;
    type SubjectsForPredicateObjectIterator =
        SubjectsIterator<S::QuadsForPredicateObjectGraphIterator, S>;
    type TriplesForObjectIterator = TriplesIterator<S::QuadsForObjectGraphIterator, S>;

    fn triples(&self) -> Result<TriplesIterator<S::QuadsForGraphIterator, S>> {
        Ok(TriplesIterator {
            iter: self.store.quads_for_graph(self.encoded_name)?,
            store: self.store.clone(),
        })
    }

    fn triples_for_subject(
        &self,
        subject: &NamedOrBlankNode,
    ) -> Result<TriplesIterator<S::QuadsForSubjectGraphIterator, S>> {
        let encoder = self.store.encoder();
        Ok(TriplesIterator {
            iter: self.store.quads_for_subject_graph(
                encoder.encode_named_or_blank_node(subject)?,
                self.encoded_name,
            )?,
            store: self.store.clone(),
        })
    }
    fn objects_for_subject_predicate(
        &self,
        subject: &NamedOrBlankNode,
        predicate: &NamedNode,
    ) -> Result<ObjectsIterator<S::QuadsForSubjectPredicateGraphIterator, S>> {
        let encoder = self.store.encoder();
        Ok(ObjectsIterator {
            iter: self.store.quads_for_subject_predicate_graph(
                encoder.encode_named_or_blank_node(subject)?,
                encoder.encode_named_node(predicate)?,
                self.encoded_name,
            )?,
            store: self.store.clone(),
        })
    }
    fn predicates_for_subject_object(
        &self,
        subject: &NamedOrBlankNode,
        object: &Term,
    ) -> Result<PredicatesIterator<S::QuadsForSubjectObjectGraphIterator, S>> {
        let encoder = self.store.encoder();
        Ok(PredicatesIterator {
            iter: self.store.quads_for_subject_object_graph(
                encoder.encode_named_or_blank_node(subject)?,
                encoder.encode_term(object)?,
                self.encoded_name,
            )?,
            store: self.store.clone(),
        })
    }
    fn triples_for_predicate(
        &self,
        predicate: &NamedNode,
    ) -> Result<TriplesIterator<S::QuadsForPredicateGraphIterator, S>> {
        let encoder = self.store.encoder();
        Ok(TriplesIterator {
            iter: self.store.quads_for_predicate_graph(
                encoder.encode_named_node(predicate)?,
                self.encoded_name,
            )?,
            store: self.store.clone(),
        })
    }
    fn subjects_for_predicate_object(
        &self,
        predicate: &NamedNode,
        object: &Term,
    ) -> Result<SubjectsIterator<S::QuadsForPredicateObjectGraphIterator, S>> {
        let encoder = self.store.encoder();
        Ok(SubjectsIterator {
            iter: self.store.quads_for_predicate_object_graph(
                encoder.encode_named_node(predicate)?,
                encoder.encode_term(object)?,
                self.encoded_name,
            )?,
            store: self.store.clone(),
        })
    }
    fn triples_for_object(
        &self,
        object: &Term,
    ) -> Result<TriplesIterator<S::QuadsForObjectGraphIterator, S>> {
        let encoder = self.store.encoder();
        Ok(TriplesIterator {
            iter: self
                .store
                .quads_for_object_graph(encoder.encode_term(object)?, self.encoded_name)?,
            store: self.store.clone(),
        })
    }

    fn contains(&self, triple: &Triple) -> Result<bool> {
        self.store.contains(
            &self
                .store
                .encoder()
                .encode_triple_in_graph(triple, self.encoded_name)?,
        )
    }

    fn insert(&self, triple: &Triple) -> Result<()> {
        self.store.insert(
            &self
                .store
                .encoder()
                .encode_triple_in_graph(triple, self.encoded_name)?,
        )
    }

    fn remove(&self, triple: &Triple) -> Result<()> {
        self.store.remove(
            &self
                .store
                .encoder()
                .encode_triple_in_graph(triple, self.encoded_name)?,
        )
    }

    fn len(&self) -> Result<usize> {
        Ok(self.store.quads_for_graph(self.encoded_name)?.count())
    }

    fn is_empty(&self) -> Result<bool> {
        Ok(self.store.quads_for_graph(self.encoded_name)?.any(|_| true))
    }
}

impl<S: EncodedQuadsStore> NamedGraph for StoreNamedGraph<S> {
    fn name(&self) -> &NamedOrBlankNode {
        &self.name
    }
}

impl<S: EncodedQuadsStore> fmt::Display for StoreNamedGraph<S> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        for triple in self.iter().map_err(|_| fmt::Error)? {
            writeln!(fmt, "{}", triple.map_err(|_| fmt::Error)?)?;
        }
        Ok(())
    }
}

pub struct StoreDefaultGraph<S: EncodedQuadsStore> {
    store: Arc<S>,
}

impl<S: EncodedQuadsStore> Graph for StoreDefaultGraph<S> {
    type TriplesIterator = TriplesIterator<S::QuadsForGraphIterator, S>;
    type TriplesForSubjectIterator = TriplesIterator<S::QuadsForSubjectGraphIterator, S>;
    type ObjectsForSubjectPredicateIterator =
        ObjectsIterator<S::QuadsForSubjectPredicateGraphIterator, S>;
    type PredicatesForSubjectObjectIterator =
        PredicatesIterator<S::QuadsForSubjectObjectGraphIterator, S>;
    type TriplesForPredicateIterator = TriplesIterator<S::QuadsForPredicateGraphIterator, S>;
    type SubjectsForPredicateObjectIterator =
        SubjectsIterator<S::QuadsForPredicateObjectGraphIterator, S>;
    type TriplesForObjectIterator = TriplesIterator<S::QuadsForObjectGraphIterator, S>;

    fn triples(&self) -> Result<TriplesIterator<S::QuadsForGraphIterator, S>> {
        Ok(TriplesIterator {
            iter: self.store.quads_for_graph(ENCODED_DEFAULT_GRAPH)?,
            store: self.store.clone(),
        })
    }

    fn triples_for_subject(
        &self,
        subject: &NamedOrBlankNode,
    ) -> Result<TriplesIterator<S::QuadsForSubjectGraphIterator, S>> {
        let encoder = self.store.encoder();
        Ok(TriplesIterator {
            iter: self.store.quads_for_subject_graph(
                encoder.encode_named_or_blank_node(subject)?,
                ENCODED_DEFAULT_GRAPH,
            )?,
            store: self.store.clone(),
        })
    }
    fn objects_for_subject_predicate(
        &self,
        subject: &NamedOrBlankNode,
        predicate: &NamedNode,
    ) -> Result<ObjectsIterator<S::QuadsForSubjectPredicateGraphIterator, S>> {
        let encoder = self.store.encoder();
        Ok(ObjectsIterator {
            iter: self.store.quads_for_subject_predicate_graph(
                encoder.encode_named_or_blank_node(subject)?,
                encoder.encode_named_node(predicate)?,
                ENCODED_DEFAULT_GRAPH,
            )?,
            store: self.store.clone(),
        })
    }
    fn predicates_for_subject_object(
        &self,
        subject: &NamedOrBlankNode,
        object: &Term,
    ) -> Result<PredicatesIterator<S::QuadsForSubjectObjectGraphIterator, S>> {
        let encoder = self.store.encoder();
        Ok(PredicatesIterator {
            iter: self.store.quads_for_subject_object_graph(
                encoder.encode_named_or_blank_node(subject)?,
                encoder.encode_term(object)?,
                ENCODED_DEFAULT_GRAPH,
            )?,
            store: self.store.clone(),
        })
    }
    fn triples_for_predicate(
        &self,
        predicate: &NamedNode,
    ) -> Result<TriplesIterator<S::QuadsForPredicateGraphIterator, S>> {
        let encoder = self.store.encoder();
        Ok(TriplesIterator {
            iter: self.store.quads_for_predicate_graph(
                encoder.encode_named_node(predicate)?,
                ENCODED_DEFAULT_GRAPH,
            )?,
            store: self.store.clone(),
        })
    }
    fn subjects_for_predicate_object(
        &self,
        predicate: &NamedNode,
        object: &Term,
    ) -> Result<SubjectsIterator<S::QuadsForPredicateObjectGraphIterator, S>> {
        let encoder = self.store.encoder();
        Ok(SubjectsIterator {
            iter: self.store.quads_for_predicate_object_graph(
                encoder.encode_named_node(predicate)?,
                encoder.encode_term(object)?,
                ENCODED_DEFAULT_GRAPH,
            )?,
            store: self.store.clone(),
        })
    }
    fn triples_for_object(
        &self,
        object: &Term,
    ) -> Result<TriplesIterator<S::QuadsForObjectGraphIterator, S>> {
        let encoder = self.store.encoder();
        Ok(TriplesIterator {
            iter: self
                .store
                .quads_for_object_graph(encoder.encode_term(object)?, ENCODED_DEFAULT_GRAPH)?,
            store: self.store.clone(),
        })
    }

    fn contains(&self, triple: &Triple) -> Result<bool> {
        self.store.contains(
            &self
                .store
                .encoder()
                .encode_triple_in_graph(triple, ENCODED_DEFAULT_GRAPH)?,
        )
    }

    fn insert(&self, triple: &Triple) -> Result<()> {
        self.store.insert(
            &self
                .store
                .encoder()
                .encode_triple_in_graph(triple, ENCODED_DEFAULT_GRAPH)?,
        )
    }

    fn remove(&self, triple: &Triple) -> Result<()> {
        self.store.remove(
            &self
                .store
                .encoder()
                .encode_triple_in_graph(triple, ENCODED_DEFAULT_GRAPH)?,
        )
    }

    fn len(&self) -> Result<usize> {
        Ok(self.store.quads_for_graph(ENCODED_DEFAULT_GRAPH)?.count())
    }

    fn is_empty(&self) -> Result<bool> {
        Ok(self
            .store
            .quads_for_graph(ENCODED_DEFAULT_GRAPH)?
            .any(|_| true))
    }
}

impl<S: EncodedQuadsStore> fmt::Display for StoreDefaultGraph<S> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        for triple in self.iter().map_err(|_| fmt::Error)? {
            writeln!(fmt, "{}", triple.map_err(|_| fmt::Error)?)?;
        }
        Ok(())
    }
}

impl<S: EncodedQuadsStore + Default> Default for StoreDefaultGraph<S> {
    fn default() -> Self {
        StoreDataset::default().default_graph()
    }
}

impl<S: EncodedQuadsStore + Default> FromIterator<Triple> for StoreDefaultGraph<S> {
    fn from_iter<I: IntoIterator<Item = Triple>>(iter: I) -> Self {
        let graph = Self::default();
        for triple in iter {
            graph.insert(&triple).unwrap();
        }
        graph
    }
}

impl<'a, S: EncodedQuadsStore + Default> FromIterator<&'a Triple> for StoreDefaultGraph<S> {
    fn from_iter<I: IntoIterator<Item = &'a Triple>>(iter: I) -> Self {
        let graph = Self::default();
        for triple in iter {
            graph.insert(triple).unwrap();
        }
        graph
    }
}

pub struct StoreUnionGraph<S: EncodedQuadsStore> {
    store: Arc<S>,
}

impl<S: EncodedQuadsStore> Graph for StoreUnionGraph<S> {
    type TriplesIterator = TriplesIterator<S::QuadsIterator, S>;
    type TriplesForSubjectIterator = TriplesIterator<S::QuadsForSubjectIterator, S>;
    type ObjectsForSubjectPredicateIterator =
        ObjectsIterator<S::QuadsForSubjectPredicateIterator, S>;
    type PredicatesForSubjectObjectIterator =
        PredicatesIterator<S::QuadsForSubjectObjectIterator, S>;
    type TriplesForPredicateIterator = TriplesIterator<S::QuadsForPredicateIterator, S>;
    type SubjectsForPredicateObjectIterator =
        SubjectsIterator<S::QuadsForPredicateObjectIterator, S>;
    type TriplesForObjectIterator = TriplesIterator<S::QuadsForObjectIterator, S>;

    fn triples(&self) -> Result<TriplesIterator<S::QuadsIterator, S>> {
        Ok(TriplesIterator {
            iter: self.store.quads()?,
            store: self.store.clone(),
        })
    }

    fn triples_for_subject(
        &self,
        subject: &NamedOrBlankNode,
    ) -> Result<TriplesIterator<S::QuadsForSubjectIterator, S>> {
        let encoder = self.store.encoder();
        Ok(TriplesIterator {
            iter: self
                .store
                .quads_for_subject(encoder.encode_named_or_blank_node(subject)?)?,
            store: self.store.clone(),
        })
    }
    fn objects_for_subject_predicate(
        &self,
        subject: &NamedOrBlankNode,
        predicate: &NamedNode,
    ) -> Result<ObjectsIterator<S::QuadsForSubjectPredicateIterator, S>> {
        let encoder = self.store.encoder();
        Ok(ObjectsIterator {
            iter: self.store.quads_for_subject_predicate(
                encoder.encode_named_or_blank_node(subject)?,
                encoder.encode_named_node(predicate)?,
            )?,
            store: self.store.clone(),
        })
    }
    fn predicates_for_subject_object(
        &self,
        subject: &NamedOrBlankNode,
        object: &Term,
    ) -> Result<PredicatesIterator<S::QuadsForSubjectObjectIterator, S>> {
        let encoder = self.store.encoder();
        Ok(PredicatesIterator {
            iter: self.store.quads_for_subject_object(
                encoder.encode_named_or_blank_node(subject)?,
                encoder.encode_term(object)?,
            )?,
            store: self.store.clone(),
        })
    }
    fn triples_for_predicate(
        &self,
        predicate: &NamedNode,
    ) -> Result<TriplesIterator<S::QuadsForPredicateIterator, S>> {
        let encoder = self.store.encoder();
        Ok(TriplesIterator {
            iter: self
                .store
                .quads_for_predicate(encoder.encode_named_node(predicate)?)?,
            store: self.store.clone(),
        })
    }
    fn subjects_for_predicate_object(
        &self,
        predicate: &NamedNode,
        object: &Term,
    ) -> Result<SubjectsIterator<S::QuadsForPredicateObjectIterator, S>> {
        let encoder = self.store.encoder();
        Ok(SubjectsIterator {
            iter: self.store.quads_for_predicate_object(
                encoder.encode_named_node(predicate)?,
                encoder.encode_term(object)?,
            )?,
            store: self.store.clone(),
        })
    }

    fn triples_for_object(
        &self,
        object: &Term,
    ) -> Result<TriplesIterator<S::QuadsForObjectIterator, S>> {
        let encoder = self.store.encoder();
        Ok(TriplesIterator {
            iter: self.store.quads_for_object(encoder.encode_term(object)?)?,
            store: self.store.clone(),
        })
    }

    fn contains(&self, triple: &Triple) -> Result<bool> {
        let encoder = self.store.encoder();
        Ok(self
            .store
            .quads_for_subject_predicate_object(
                encoder.encode_named_or_blank_node(triple.subject())?,
                encoder.encode_named_node(triple.predicate())?,
                encoder.encode_term(triple.object())?,
            )?
            .any(|_| true))
    }

    fn insert(&self, _triple: &Triple) -> Result<()> {
        Err(format_err!("Union graph is not writable"))
    }

    fn remove(&self, _triple: &Triple) -> Result<()> {
        Err(format_err!("Union graph is not writable"))
    }

    fn len(&self) -> Result<usize> {
        Ok(self.store.quads()?.count())
    }

    fn is_empty(&self) -> Result<bool> {
        Ok(self.store.quads()?.any(|_| true))
    }
}

impl<S: EncodedQuadsStore> fmt::Display for StoreUnionGraph<S> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        for triple in self.iter().map_err(|_| fmt::Error)? {
            writeln!(fmt, "{}", triple.map_err(|_| fmt::Error)?)?;
        }
        Ok(())
    }
}

pub struct QuadsIterator<I: Iterator<Item = Result<EncodedQuad>>, S: EncodedQuadsStore> {
    iter: I,
    store: Arc<S>,
}

impl<I: Iterator<Item = Result<EncodedQuad>>, S: EncodedQuadsStore> Iterator
    for QuadsIterator<I, S>
{
    type Item = Result<Quad>;

    fn next(&mut self) -> Option<Result<Quad>> {
        self.iter
            .next()
            .map(|k| k.and_then(|quad| self.store.encoder().decode_quad(&quad)))
    }
}

pub struct TriplesIterator<I: Iterator<Item = Result<EncodedQuad>>, S: EncodedQuadsStore> {
    iter: I,
    store: Arc<S>,
}

impl<I: Iterator<Item = Result<EncodedQuad>>, S: EncodedQuadsStore> Iterator
    for TriplesIterator<I, S>
{
    type Item = Result<Triple>;

    fn next(&mut self) -> Option<Result<Triple>> {
        self.iter
            .next()
            .map(|k| k.and_then(|quad| self.store.encoder().decode_triple(&quad)))
    }
}

pub struct SubjectsIterator<I: Iterator<Item = Result<EncodedQuad>>, S: EncodedQuadsStore> {
    iter: I,
    store: Arc<S>,
}

impl<I: Iterator<Item = Result<EncodedQuad>>, S: EncodedQuadsStore> Iterator
    for SubjectsIterator<I, S>
{
    type Item = Result<NamedOrBlankNode>;

    fn next(&mut self) -> Option<Result<NamedOrBlankNode>> {
        self.iter.next().map(|k| {
            k.and_then(|quad| {
                self.store
                    .encoder()
                    .decode_named_or_blank_node(quad.subject)
            })
        })
    }
}

pub struct PredicatesIterator<I: Iterator<Item = Result<EncodedQuad>>, S: EncodedQuadsStore> {
    iter: I,
    store: Arc<S>,
}

impl<I: Iterator<Item = Result<EncodedQuad>>, S: EncodedQuadsStore> Iterator
    for PredicatesIterator<I, S>
{
    type Item = Result<NamedNode>;

    fn next(&mut self) -> Option<Result<NamedNode>> {
        self.iter
            .next()
            .map(|k| k.and_then(|quad| self.store.encoder().decode_named_node(quad.predicate)))
    }
}

pub struct ObjectsIterator<I: Iterator<Item = Result<EncodedQuad>>, S: EncodedQuadsStore> {
    iter: I,
    store: Arc<S>,
}

impl<I: Iterator<Item = Result<EncodedQuad>>, S: EncodedQuadsStore> Iterator
    for ObjectsIterator<I, S>
{
    type Item = Result<Term>;

    fn next(&mut self) -> Option<Result<Term>> {
        self.iter
            .next()
            .map(|k| k.and_then(|quad| self.store.encoder().decode_term(quad.object)))
    }
}
