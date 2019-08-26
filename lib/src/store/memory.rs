use crate::store::numeric_encoder::*;
use crate::store::*;
use crate::{Repository, Result};
use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::iter::empty;
use std::iter::once;
use std::sync::RwLock;
use std::sync::RwLockReadGuard;
use std::sync::RwLockWriteGuard;

/// Memory based implementation of the `Repository` trait.
/// They are cheap to build using the `MemoryRepository::default()` method.
///
/// Usage example:
/// ```
/// use rudf::model::*;
/// use rudf::{Repository, RepositoryConnection, MemoryRepository, Result};
/// use crate::rudf::sparql::PreparedQuery;
/// use rudf::sparql::QueryResult;
///
/// let repository = MemoryRepository::default();
/// let connection = repository.connection().unwrap();
///
/// // insertion
/// let ex = NamedNode::parse("http://example.com").unwrap();
/// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
/// connection.insert(&quad);
///
/// // quad filter
/// let results: Result<Vec<Quad>> = connection.quads_for_pattern(None, None, None, None).collect();
/// assert_eq!(vec![quad], results.unwrap());
///
/// // SPARQL query
/// let prepared_query = connection.prepare_query("SELECT ?s WHERE { ?s ?p ?o }", None).unwrap();
/// let results = prepared_query.exec().unwrap();
/// if let QueryResult::Bindings(results) = results {
///     assert_eq!(results.into_values_iter().next().unwrap().unwrap()[0], Some(ex.into()));
/// }
/// ```
#[derive(Default)]
pub struct MemoryRepository {
    inner: MemoryStore,
}

pub type MemoryRepositoryConnection<'a> = StoreRepositoryConnection<&'a MemoryStore>;

#[derive(Default)]
pub struct MemoryStore {
    string_store: MemoryStringStore,
    graph_indexes: RwLock<BTreeMap<EncodedTerm, MemoryGraphIndexes>>,
}

#[derive(Default)]
struct MemoryGraphIndexes {
    spo: BTreeMap<EncodedTerm, BTreeMap<EncodedTerm, BTreeSet<EncodedTerm>>>,
    pos: BTreeMap<EncodedTerm, BTreeMap<EncodedTerm, BTreeSet<EncodedTerm>>>,
    osp: BTreeMap<EncodedTerm, BTreeMap<EncodedTerm, BTreeSet<EncodedTerm>>>,
}

impl<'a> Repository for &'a MemoryRepository {
    type Connection = MemoryRepositoryConnection<'a>;

    fn connection(self) -> Result<StoreRepositoryConnection<&'a MemoryStore>> {
        Ok(self.inner.connection()?.into())
    }
}

impl<'a> Store for &'a MemoryStore {
    type Connection = &'a MemoryStore;

    fn connection(self) -> Result<Self::Connection> {
        Ok(self)
    }
}

impl StringStore for MemoryStore {
    type StringType = String;

    fn insert_str(&self, value: &str) -> Result<u64> {
        self.string_store.insert_str(value)
    }

    fn get_str(&self, id: u64) -> Result<String> {
        self.string_store.get_str(id)
    }
}

impl<'a> StoreConnection for &'a MemoryStore {
    fn contains(&self, quad: &EncodedQuad) -> Result<bool> {
        Ok(self
            .graph_indexes()?
            .get(&quad.graph_name)
            .map_or(false, |graph| {
                graph.spo.get(&quad.subject).map_or(false, |po| {
                    po.get(&quad.predicate)
                        .map_or(false, |o| o.contains(&quad.object))
                })
            }))
    }

    fn insert(&self, quad: &EncodedQuad) -> Result<()> {
        let mut graph_indexes = self.graph_indexes_mut()?;
        let graph = graph_indexes
            .entry(quad.graph_name)
            .or_insert_with(MemoryGraphIndexes::default);
        graph
            .spo
            .entry(quad.subject)
            .or_default()
            .entry(quad.predicate)
            .or_default()
            .insert(quad.object);
        graph
            .pos
            .entry(quad.predicate)
            .or_default()
            .entry(quad.object)
            .or_default()
            .insert(quad.subject);
        graph
            .osp
            .entry(quad.object)
            .or_default()
            .entry(quad.subject)
            .or_default()
            .insert(quad.predicate);
        Ok(())
    }

    fn remove(&self, quad: &EncodedQuad) -> Result<()> {
        let mut graph_indexes = self.graph_indexes_mut()?;
        let mut empty_graph = false;
        if let Some(graph) = graph_indexes.get_mut(&quad.graph_name) {
            {
                let mut empty_pos = false;
                if let Some(pos) = graph.spo.get_mut(&quad.subject) {
                    let mut empty_os = false;
                    if let Some(os) = pos.get_mut(&quad.predicate) {
                        os.remove(&quad.object);
                        empty_os = os.is_empty();
                    }
                    if empty_os {
                        pos.remove(&quad.predicate);
                    }
                    empty_pos = pos.is_empty();
                }
                if empty_pos {
                    graph.spo.remove(&quad.subject);
                }
            }

            {
                let mut empty_oss = false;
                if let Some(oss) = graph.pos.get_mut(&quad.predicate) {
                    let mut empty_ss = false;
                    if let Some(ss) = oss.get_mut(&quad.object) {
                        ss.remove(&quad.subject);
                        empty_ss = ss.is_empty();
                    }
                    if empty_ss {
                        oss.remove(&quad.object);
                    }
                    empty_oss = oss.is_empty();
                }
                if empty_oss {
                    graph.pos.remove(&quad.predicate);
                }
            }

            {
                let mut empty_sps = false;
                if let Some(sps) = graph.osp.get_mut(&quad.object) {
                    let mut empty_ps = false;
                    if let Some(ps) = sps.get_mut(&quad.subject) {
                        ps.remove(&quad.predicate);
                        empty_ps = ps.is_empty();
                    }
                    if empty_ps {
                        sps.remove(&quad.subject);
                    }
                    empty_sps = sps.is_empty();
                }
                if empty_sps {
                    graph.osp.remove(&quad.object);
                }
            }

            empty_graph = graph.spo.is_empty();
        }
        if empty_graph {
            graph_indexes.remove(&quad.graph_name);
        }
        Ok(())
    }

    fn quads_for_pattern<'b>(
        &'b self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad>> + 'b> {
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

impl MemoryStore {
    fn graph_indexes(
        &self,
    ) -> Result<RwLockReadGuard<'_, BTreeMap<EncodedTerm, MemoryGraphIndexes>>> {
        Ok(self.graph_indexes.read().map_err(MutexPoisonError::from)?)
    }

    fn graph_indexes_mut(
        &self,
    ) -> Result<RwLockWriteGuard<'_, BTreeMap<EncodedTerm, MemoryGraphIndexes>>> {
        Ok(self.graph_indexes.write().map_err(MutexPoisonError::from)?)
    }

    fn quads(&self) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        for (graph_name, graph) in self.graph_indexes()?.iter() {
            for (s, pos) in &graph.spo {
                for (p, os) in pos.iter() {
                    for o in os.iter() {
                        result.push(Ok(EncodedQuad::new(*s, *p, *o, *graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_subject(
        &self,
        subject: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        for (graph_name, graph) in self.graph_indexes()?.iter() {
            if let Some(pos) = graph.spo.get(&subject) {
                for (p, os) in pos.iter() {
                    for o in os.iter() {
                        result.push(Ok(EncodedQuad::new(subject, *p, *o, *graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_subject_predicate(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        for (graph_name, graph) in self.graph_indexes()?.iter() {
            if let Some(pos) = graph.spo.get(&subject) {
                if let Some(os) = pos.get(&predicate) {
                    for o in os.iter() {
                        result.push(Ok(EncodedQuad::new(subject, predicate, *o, *graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_subject_predicate_object(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        for (graph_name, graph) in self.graph_indexes()?.iter() {
            if let Some(pos) = graph.spo.get(&subject) {
                if let Some(os) = pos.get(&predicate) {
                    if os.contains(&object) {
                        result.push(Ok(EncodedQuad::new(
                            subject,
                            predicate,
                            object,
                            *graph_name,
                        )))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_subject_object(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        for (graph_name, graph) in self.graph_indexes()?.iter() {
            if let Some(sps) = graph.osp.get(&object) {
                if let Some(ps) = sps.get(&subject) {
                    for p in ps.iter() {
                        result.push(Ok(EncodedQuad::new(subject, *p, object, *graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_predicate(
        &self,
        predicate: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        for (graph_name, graph) in self.graph_indexes()?.iter() {
            if let Some(oss) = graph.pos.get(&predicate) {
                for (o, ss) in oss.iter() {
                    for s in ss.iter() {
                        result.push(Ok(EncodedQuad::new(*s, predicate, *o, *graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_predicate_object(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        for (graph_name, graph) in self.graph_indexes()?.iter() {
            if let Some(oss) = graph.pos.get(&predicate) {
                if let Some(ss) = oss.get(&object) {
                    for s in ss.iter() {
                        result.push(Ok(EncodedQuad::new(*s, predicate, object, *graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_object(
        &self,
        object: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        for (graph_name, graph) in self.graph_indexes()?.iter() {
            if let Some(sps) = graph.osp.get(&object) {
                for (s, ps) in sps.iter() {
                    for p in ps.iter() {
                        result.push(Ok(EncodedQuad::new(*s, *p, object, *graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_graph(
        &self,
        graph_name: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        if let Some(graph) = self.graph_indexes()?.get(&graph_name) {
            for (s, pos) in &graph.spo {
                for (p, os) in pos.iter() {
                    for o in os.iter() {
                        result.push(Ok(EncodedQuad::new(*s, *p, *o, graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_subject_graph(
        &self,
        subject: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        if let Some(graph) = self.graph_indexes()?.get(&graph_name) {
            if let Some(pos) = graph.spo.get(&subject) {
                for (p, os) in pos.iter() {
                    for o in os.iter() {
                        result.push(Ok(EncodedQuad::new(subject, *p, *o, graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_subject_predicate_graph(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        if let Some(graph) = self.graph_indexes()?.get(&graph_name) {
            if let Some(pos) = graph.spo.get(&subject) {
                if let Some(os) = pos.get(&predicate) {
                    for o in os.iter() {
                        result.push(Ok(EncodedQuad::new(subject, predicate, *o, graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_subject_object_graph(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        if let Some(graph) = self.graph_indexes()?.get(&graph_name) {
            if let Some(sps) = graph.osp.get(&object) {
                if let Some(ps) = sps.get(&subject) {
                    for p in ps.iter() {
                        result.push(Ok(EncodedQuad::new(subject, *p, object, graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_predicate_graph(
        &self,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        if let Some(graph) = self.graph_indexes()?.get(&graph_name) {
            if let Some(oss) = graph.pos.get(&predicate) {
                for (o, ss) in oss.iter() {
                    for s in ss.iter() {
                        result.push(Ok(EncodedQuad::new(*s, predicate, *o, graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_predicate_object_graph(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        if let Some(graph) = self.graph_indexes()?.get(&graph_name) {
            if let Some(oss) = graph.pos.get(&predicate) {
                if let Some(ss) = oss.get(&object) {
                    for s in ss.iter() {
                        result.push(Ok(EncodedQuad::new(*s, predicate, object, graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
    }

    fn quads_for_object_graph(
        &self,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<<Vec<Result<EncodedQuad>> as IntoIterator>::IntoIter> {
        let mut result = Vec::default();
        if let Some(graph) = self.graph_indexes()?.get(&graph_name) {
            if let Some(sps) = graph.osp.get(&object) {
                for (s, ps) in sps.iter() {
                    for p in ps.iter() {
                        result.push(Ok(EncodedQuad::new(*s, *p, object, graph_name)))
                    }
                }
            }
        }
        Ok(result.into_iter())
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
