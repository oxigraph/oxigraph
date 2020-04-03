use crate::store::numeric_encoder::*;
use crate::store::*;
use crate::{Repository, Result};
use std::collections::{HashMap, HashSet};
use std::hash::Hash;
use std::iter::{empty, once};
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Memory based implementation of the `Repository` trait.
/// It is cheap to build using the `MemoryRepository::default()` method.
///
/// Usage example:
/// ```
/// use oxigraph::model::*;
/// use oxigraph::{Repository, RepositoryConnection, MemoryRepository, Result};
/// use crate::oxigraph::sparql::PreparedQuery;
/// use oxigraph::sparql::{QueryResult, QueryOptions};
///
/// let repository = MemoryRepository::default();
/// let mut connection = repository.connection()?;
///
/// // insertion
/// let ex = NamedNode::parse("http://example.com")?;
/// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
/// connection.insert(&quad)?;
///
/// // quad filter
/// let results: Result<Vec<Quad>> = connection.quads_for_pattern(None, None, None, None).collect();
/// assert_eq!(vec![quad], results?);
///
/// // SPARQL query
/// let prepared_query = connection.prepare_query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())?;
/// let results = prepared_query.exec()?;
/// if let QueryResult::Bindings(results) = results {
///     assert_eq!(results.into_values_iter().next().unwrap()?[0], Some(ex.into()));
/// }
/// # Result::Ok(())
/// ```
#[derive(Default)]
pub struct MemoryRepository {
    inner: MemoryStore,
}

pub type MemoryRepositoryConnection<'a> = StoreRepositoryConnection<&'a MemoryStore>;
type TripleMap<T> = HashMap<T, HashMap<T, HashSet<T>>>;
type QuadMap<T> = HashMap<T, TripleMap<T>>;

pub struct MemoryStore {
    indexes: RwLock<MemoryStoreIndexes>,
}

#[derive(Default)]
struct MemoryStoreIndexes {
    spog: QuadMap<EncodedTerm>,
    posg: QuadMap<EncodedTerm>,
    ospg: QuadMap<EncodedTerm>,
    gspo: QuadMap<EncodedTerm>,
    gpos: QuadMap<EncodedTerm>,
    gosp: QuadMap<EncodedTerm>,
    id2str: HashMap<u128, String>,
}

impl Default for MemoryStore {
    fn default() -> Self {
        let new = Self {
            indexes: RwLock::default(),
        };

        let mut transaction = (&new).connection().unwrap().transaction();
        transaction.set_first_strings().unwrap();
        transaction.commit().unwrap();

        new
    }
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

impl<'a> StrLookup for &'a MemoryStore {
    fn get_str(&self, id: u128) -> Result<Option<String>> {
        //TODO: avoid copy by adding a lifetime limit to get_str
        Ok(self.indexes()?.id2str.get(&id).cloned())
    }
}

impl<'a> StrContainer for &'a MemoryStore {
    fn insert_str(&mut self, key: u128, value: &str) -> Result<()> {
        self.indexes_mut()?
            .id2str
            .entry(key)
            .or_insert_with(|| value.to_owned());
        Ok(())
    }
}

impl<'a> StoreConnection for &'a MemoryStore {
    type Transaction = MemoryTransaction<'a>;
    type AutoTransaction = &'a MemoryStore;

    fn transaction(&self) -> MemoryTransaction<'a> {
        MemoryTransaction {
            store: self,
            ops: Vec::default(),
            strings: Vec::default(),
        }
    }

    fn auto_transaction(&self) -> &'a MemoryStore {
        self
    }

    fn contains(&self, quad: &EncodedQuad) -> Result<bool> {
        Ok(self
            .indexes()?
            .spog
            .get(&quad.subject)
            .map_or(false, |pog| {
                pog.get(&quad.predicate).map_or(false, |og| {
                    og.get(&quad.object)
                        .map_or(false, |g| g.contains(&quad.graph_name))
                })
            }))
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

impl<'a> StoreTransaction for &'a MemoryStore {
    fn insert(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.indexes_mut()?.insert_quad(quad);
        Ok(())
    }

    fn remove(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.indexes_mut()?.remove_quad(quad);
        Ok(())
    }

    fn commit(self) -> Result<()> {
        Ok(())
    }
}

impl MemoryStore {
    fn indexes(&self) -> Result<RwLockReadGuard<'_, MemoryStoreIndexes>> {
        Ok(self.indexes.read()?)
    }

    fn indexes_mut(&self) -> Result<RwLockWriteGuard<'_, MemoryStoreIndexes>> {
        Ok(self.indexes.write()?)
    }

    fn quads<'a>(&'a self) -> Result<impl Iterator<Item = Result<EncodedQuad>> + 'a> {
        Ok(quad_map_flatten(&self.indexes()?.gspo)
            .map(|(g, s, p, o)| Ok(EncodedQuad::new(s, p, o, g)))
            .collect::<Vec<_>>()
            .into_iter())
    }

    fn quads_for_subject(
        &self,
        subject: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>>> {
        Ok(
            option_triple_map_flatten(self.indexes()?.spog.get(&subject))
                .map(|(p, o, g)| Ok(EncodedQuad::new(subject, p, o, g)))
                .collect::<Vec<_>>()
                .into_iter(),
        )
    }

    fn quads_for_subject_predicate(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>>> {
        Ok(option_pair_map_flatten(
            self.indexes()?
                .spog
                .get(&subject)
                .and_then(|pog| pog.get(&predicate)),
        )
        .map(|(o, g)| Ok(EncodedQuad::new(subject, predicate, o, g)))
        .collect::<Vec<_>>()
        .into_iter())
    }

    fn quads_for_subject_predicate_object(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>>> {
        Ok(option_set_flatten(
            self.indexes()?
                .spog
                .get(&subject)
                .and_then(|pog| pog.get(&predicate))
                .and_then(|og| og.get(&object)),
        )
        .map(|g| Ok(EncodedQuad::new(subject, predicate, object, g)))
        .collect::<Vec<_>>()
        .into_iter())
    }

    fn quads_for_subject_object(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>>> {
        Ok(option_pair_map_flatten(
            self.indexes()?
                .ospg
                .get(&object)
                .and_then(|spg| spg.get(&subject)),
        )
        .map(|(p, g)| Ok(EncodedQuad::new(subject, p, object, g)))
        .collect::<Vec<_>>()
        .into_iter())
    }

    fn quads_for_predicate(
        &self,
        predicate: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>>> {
        Ok(
            option_triple_map_flatten(self.indexes()?.posg.get(&predicate))
                .map(|(o, s, g)| Ok(EncodedQuad::new(s, predicate, o, g)))
                .collect::<Vec<_>>()
                .into_iter(),
        )
    }

    fn quads_for_predicate_object(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>>> {
        Ok(option_pair_map_flatten(
            self.indexes()?
                .posg
                .get(&predicate)
                .and_then(|osg| osg.get(&object)),
        )
        .map(|(s, g)| Ok(EncodedQuad::new(s, predicate, object, g)))
        .collect::<Vec<_>>()
        .into_iter())
    }

    fn quads_for_object(
        &self,
        object: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>>> {
        Ok(option_triple_map_flatten(self.indexes()?.ospg.get(&object))
            .map(|(s, p, g)| Ok(EncodedQuad::new(s, p, object, g)))
            .collect::<Vec<_>>()
            .into_iter())
    }

    fn quads_for_graph(
        &self,
        graph_name: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>>> {
        Ok(
            option_triple_map_flatten(self.indexes()?.gspo.get(&graph_name))
                .map(|(s, p, o)| Ok(EncodedQuad::new(s, p, o, graph_name)))
                .collect::<Vec<_>>()
                .into_iter(),
        )
    }

    fn quads_for_subject_graph(
        &self,
        subject: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>>> {
        Ok(option_pair_map_flatten(
            self.indexes()?
                .gspo
                .get(&graph_name)
                .and_then(|spo| spo.get(&subject)),
        )
        .map(|(p, o)| Ok(EncodedQuad::new(subject, p, o, graph_name)))
        .collect::<Vec<_>>()
        .into_iter())
    }

    fn quads_for_subject_predicate_graph(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>>> {
        Ok(option_set_flatten(
            self.indexes()?
                .gspo
                .get(&graph_name)
                .and_then(|spo| spo.get(&subject))
                .and_then(|po| po.get(&predicate)),
        )
        .map(|o| Ok(EncodedQuad::new(subject, predicate, o, graph_name)))
        .collect::<Vec<_>>()
        .into_iter())
    }

    fn quads_for_subject_object_graph(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>>> {
        Ok(option_set_flatten(
            self.indexes()?
                .gosp
                .get(&graph_name)
                .and_then(|osp| osp.get(&object))
                .and_then(|sp| sp.get(&subject)),
        )
        .map(|p| Ok(EncodedQuad::new(subject, p, object, graph_name)))
        .collect::<Vec<_>>()
        .into_iter())
    }

    fn quads_for_predicate_graph(
        &self,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>>> {
        Ok(option_pair_map_flatten(
            self.indexes()?
                .gpos
                .get(&graph_name)
                .and_then(|pos| pos.get(&predicate)),
        )
        .map(|(o, s)| Ok(EncodedQuad::new(s, predicate, o, graph_name)))
        .collect::<Vec<_>>()
        .into_iter())
    }

    fn quads_for_predicate_object_graph(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>>> {
        Ok(option_set_flatten(
            self.indexes()?
                .gpos
                .get(&graph_name)
                .and_then(|pos| pos.get(&predicate))
                .and_then(|os| os.get(&object)),
        )
        .map(|s| Ok(EncodedQuad::new(s, predicate, object, graph_name)))
        .collect::<Vec<_>>()
        .into_iter())
    }

    fn quads_for_object_graph(
        &self,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<impl Iterator<Item = Result<EncodedQuad>>> {
        Ok(option_pair_map_flatten(
            self.indexes()?
                .gosp
                .get(&graph_name)
                .and_then(|osp| osp.get(&object)),
        )
        .map(|(s, p)| Ok(EncodedQuad::new(s, p, object, graph_name)))
        .collect::<Vec<_>>()
        .into_iter())
    }
}

impl MemoryStoreIndexes {
    fn insert_quad(&mut self, quad: &EncodedQuad) {
        insert_into_quad_map(
            &mut self.gosp,
            quad.graph_name,
            quad.object,
            quad.subject,
            quad.predicate,
        );
        insert_into_quad_map(
            &mut self.gpos,
            quad.graph_name,
            quad.predicate,
            quad.object,
            quad.subject,
        );
        insert_into_quad_map(
            &mut self.gspo,
            quad.graph_name,
            quad.subject,
            quad.predicate,
            quad.object,
        );
        insert_into_quad_map(
            &mut self.ospg,
            quad.object,
            quad.subject,
            quad.predicate,
            quad.graph_name,
        );
        insert_into_quad_map(
            &mut self.posg,
            quad.predicate,
            quad.object,
            quad.subject,
            quad.graph_name,
        );
        insert_into_quad_map(
            &mut self.spog,
            quad.subject,
            quad.predicate,
            quad.object,
            quad.graph_name,
        );
    }

    fn remove_quad(&mut self, quad: &EncodedQuad) {
        remove_from_quad_map(
            &mut self.gosp,
            &quad.graph_name,
            &quad.object,
            &quad.subject,
            &quad.predicate,
        );
        remove_from_quad_map(
            &mut self.gpos,
            &quad.graph_name,
            &quad.predicate,
            &quad.object,
            &quad.subject,
        );
        remove_from_quad_map(
            &mut self.gspo,
            &quad.graph_name,
            &quad.subject,
            &quad.predicate,
            &quad.object,
        );
        remove_from_quad_map(
            &mut self.ospg,
            &quad.object,
            &quad.subject,
            &quad.predicate,
            &quad.graph_name,
        );
        remove_from_quad_map(
            &mut self.posg,
            &quad.predicate,
            &quad.object,
            &quad.subject,
            &quad.graph_name,
        );
        remove_from_quad_map(
            &mut self.spog,
            &quad.subject,
            &quad.predicate,
            &quad.object,
            &quad.graph_name,
        );
    }
}

fn wrap_error<'a, E: 'static, I: Iterator<Item = Result<E>> + 'a>(
    iter: Result<I>,
) -> Box<dyn Iterator<Item = Result<E>> + 'a> {
    match iter {
        Ok(iter) => Box::new(iter),
        Err(error) => Box::new(once(Err(error))),
    }
}

fn insert_into_quad_map<T: Eq + Hash>(map: &mut QuadMap<T>, e1: T, e2: T, e3: T, e4: T) {
    map.entry(e1)
        .or_default()
        .entry(e2)
        .or_default()
        .entry(e3)
        .or_default()
        .insert(e4);
}

fn remove_from_quad_map<T: Eq + Hash>(map1: &mut QuadMap<T>, e1: &T, e2: &T, e3: &T, e4: &T) {
    let mut map2empty = false;
    if let Some(map2) = map1.get_mut(e1) {
        let mut map3empty = false;
        if let Some(map3) = map2.get_mut(e2) {
            let mut set4empty = false;
            if let Some(set4) = map3.get_mut(e3) {
                set4.remove(e4);
                set4empty = set4.is_empty();
            }
            if set4empty {
                map3.remove(e3);
            }
            map3empty = map3.is_empty();
        }
        if map3empty {
            map2.remove(e2);
        }
        map2empty = map2.is_empty();
    }
    if map2empty {
        map1.remove(e1);
    }
}

fn option_set_flatten<'a, T: Clone>(i: Option<&'a HashSet<T>>) -> impl Iterator<Item = T> + 'a {
    i.into_iter().flat_map(|s| s.iter().cloned())
}

fn option_pair_map_flatten<'a, T: Copy>(
    i: Option<&'a HashMap<T, HashSet<T>>>,
) -> impl Iterator<Item = (T, T)> + 'a {
    i.into_iter().flat_map(|kv| {
        kv.iter().flat_map(|(k, vs)| {
            let k = *k;
            vs.iter().map(move |v| (k, *v))
        })
    })
}

fn option_triple_map_flatten<'a, T: Copy>(
    i: Option<&'a TripleMap<T>>,
) -> impl Iterator<Item = (T, T, T)> + 'a {
    i.into_iter().flat_map(|spo| {
        spo.iter().flat_map(|(s, po)| {
            let s = *s;
            po.iter().flat_map(move |(p, os)| {
                let p = *p;
                os.iter().map(move |o| (s, p, *o))
            })
        })
    })
}

fn quad_map_flatten<'a, T: Copy>(gspo: &'a QuadMap<T>) -> impl Iterator<Item = (T, T, T, T)> + 'a {
    gspo.iter().flat_map(|(g, spo)| {
        let g = *g;
        spo.iter().flat_map(move |(s, po)| {
            let s = *s;
            po.iter().flat_map(move |(p, os)| {
                let p = *p;
                os.iter().map(move |o| (g, s, p, *o))
            })
        })
    })
}

pub struct MemoryTransaction<'a> {
    store: &'a MemoryStore,
    ops: Vec<TransactionOp>,
    strings: Vec<(u128, String)>,
}

enum TransactionOp {
    Insert(EncodedQuad),
    Delete(EncodedQuad),
}

impl StrContainer for MemoryTransaction<'_> {
    fn insert_str(&mut self, key: u128, value: &str) -> Result<()> {
        self.strings.push((key, value.to_owned()));
        Ok(())
    }
}

impl StoreTransaction for MemoryTransaction<'_> {
    fn insert(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.ops.push(TransactionOp::Insert(*quad));
        Ok(())
    }

    fn remove(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.ops.push(TransactionOp::Delete(*quad));
        Ok(())
    }

    fn commit(self) -> Result<()> {
        let mut indexes = self.store.indexes_mut()?;
        indexes.id2str.extend(self.strings);
        for op in self.ops {
            match op {
                TransactionOp::Insert(quad) => indexes.insert_quad(&quad),
                TransactionOp::Delete(quad) => indexes.remove_quad(&quad),
            }
        }
        Ok(())
    }
}
