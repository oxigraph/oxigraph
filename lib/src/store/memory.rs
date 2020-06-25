//! In-memory store.

use crate::model::*;
use crate::sparql::{QueryOptions, QueryResult, SimplePreparedQuery};
use crate::store::numeric_encoder::*;
use crate::store::*;
use crate::{DatasetSyntax, GraphSyntax, Result};
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::io::BufRead;
use std::iter::FromIterator;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// In-memory store.
/// It encodes a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) and allows to query and update it using SPARQL.
/// It is cheap to build using the `MemoryStore::new()` method.
///
/// Usage example:
/// ```
/// use oxigraph::model::*;
/// use oxigraph::{MemoryStore, Result};
/// use oxigraph::sparql::{QueryResult, QueryOptions};
///
/// let store = MemoryStore::new();
///
/// // insertion
/// let ex = NamedNode::parse("http://example.com")?;
/// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
/// store.insert(quad.clone());
///
/// // quad filter
/// let results: Vec<Quad> = store.quads_for_pattern(Some(&ex.clone().into()), None, None, None).collect();
/// assert_eq!(vec![quad], results);
///
/// // SPARQL query
/// let prepared_query = store.prepare_query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())?;
/// if let QueryResult::Bindings(mut solutions) = prepared_query.exec()? {
///     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
/// }
/// # Result::Ok(())
/// ```
#[derive(Clone)]
pub struct MemoryStore {
    indexes: Arc<RwLock<MemoryStoreIndexes>>,
}

type TripleMap<T> = HashMap<T, HashMap<T, HashSet<T>>>;
type QuadMap<T> = HashMap<T, TripleMap<T>>;

#[derive(Default)]
struct MemoryStoreIndexes {
    spog: QuadMap<EncodedTerm>,
    posg: QuadMap<EncodedTerm>,
    ospg: QuadMap<EncodedTerm>,
    gspo: QuadMap<EncodedTerm>,
    gpos: QuadMap<EncodedTerm>,
    gosp: QuadMap<EncodedTerm>,
    id2str: HashMap<StrHash, String>,
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStore {
    /// Constructs a new `MemoryStore`
    pub fn new() -> Self {
        let mut new = Self {
            indexes: Arc::new(RwLock::default()),
        };
        new.set_first_strings().unwrap();
        new
    }

    /// Prepares a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) and returns an object that could be used to execute it.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::{MemoryStore, Result};
    /// use oxigraph::sparql::{QueryOptions, QueryResult};
    ///
    /// let store = MemoryStore::new();
    ///
    /// // insertions
    /// let ex = NamedNode::parse("http://example.com")?;
    /// store.insert(Quad::new(ex.clone(), ex.clone(), ex.clone(), None));
    ///
    /// // SPARQL query
    /// let prepared_query = store.prepare_query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())?;
    /// if let QueryResult::Bindings(mut solutions) = prepared_query.exec()? {
    ///     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
    /// }
    /// # Result::Ok(())
    /// ```
    pub fn prepare_query(
        &self,
        query: &str,
        options: QueryOptions<'_>,
    ) -> Result<MemoryPreparedQuery> {
        Ok(MemoryPreparedQuery(SimplePreparedQuery::new(
            self.clone(),
            query,
            options,
        )?))
    }

    /// This is similar to `prepare_query`, but useful if a SPARQL query has already been parsed, which is the case when building `ServiceHandler`s for federated queries with `SERVICE` clauses. For examples, look in the tests.
    pub fn prepare_query_from_pattern(
        &self,
        graph_pattern: &GraphPattern,
        options: QueryOptions<'_>,
    ) -> Result<MemoryPreparedQuery> {
        Ok(MemoryPreparedQuery(SimplePreparedQuery::new_from_pattern(
            self.clone(),
            graph_pattern,
            options,
        )?))
    }

    /// Retrieves quads with a filter on each quad component
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::{MemoryStore, Result};
    ///
    /// let store = MemoryStore::new();
    ///
    /// // insertion
    /// let ex = NamedNode::parse("http://example.com")?;
    /// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
    /// store.insert(quad.clone());
    ///
    /// // quad filter
    /// let results: Vec<Quad> = store.quads_for_pattern(None, None, None, None).collect();
    /// assert_eq!(vec![quad], results);
    /// # Result::Ok(())
    /// ```
    #[allow(clippy::option_option)]
    pub fn quads_for_pattern(
        &self,
        subject: Option<&NamedOrBlankNode>,
        predicate: Option<&NamedNode>,
        object: Option<&Term>,
        graph_name: Option<Option<&NamedOrBlankNode>>,
    ) -> impl Iterator<Item = Quad> {
        let subject = subject.map(|s| s.into());
        let predicate = predicate.map(|p| p.into());
        let object = object.map(|o| o.into());
        let graph_name = graph_name.map(|g| g.map_or(ENCODED_DEFAULT_GRAPH, |g| g.into()));
        let this = self.clone();
        self.encoded_quads_for_pattern_inner(subject, predicate, object, graph_name)
            .into_iter()
            .map(
                move |quad| this.decode_quad(&quad).unwrap(), // Could not fail
            )
    }

    /// Checks if this store contains a given quad
    pub fn contains(&self, quad: &Quad) -> bool {
        let quad = quad.into();
        self.contains_encoded(&quad)
    }

    /// Executes a transaction.
    ///
    /// The transaction is executed if the given closure returns `Ok`.
    /// Nothing is done if the clusre returns `Err`.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::{MemoryStore, Result};
    ///
    /// let store = MemoryStore::new();
    ///
    /// let ex = NamedNode::parse("http://example.com")?;
    /// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
    ///
    /// // transaction
    /// store.transaction(|transaction| {
    ///     transaction.insert(quad.clone());
    ///     Ok(())
    /// });
    ///
    /// // quad filter
    /// assert!(store.contains(&quad));
    /// # Result::Ok(())
    /// ```
    pub fn transaction<'a>(
        &'a self,
        f: impl FnOnce(&mut MemoryTransaction<'a>) -> Result<()>,
    ) -> Result<()> {
        let mut transaction = MemoryTransaction {
            store: self,
            ops: Vec::new(),
            strings: Vec::new(),
        };
        f(&mut transaction)?;
        transaction.commit()
    }

    /// Loads a graph file (i.e. triples) into the store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::{MemoryStore, Result, GraphSyntax};
    ///
    /// let store = MemoryStore::new();
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> .";
    /// store.load_graph(file.as_ref(), GraphSyntax::NTriples, None, None);
    ///
    /// // quad filter
    /// let results: Vec<Quad> = store.quads_for_pattern(None, None, None, None).collect();
    /// let ex = NamedNode::parse("http://example.com")?;
    /// assert_eq!(vec![Quad::new(ex.clone(), ex.clone(), ex.clone(), None)], results);
    /// # Result::Ok(())
    /// ```
    pub fn load_graph(
        &self,
        reader: impl BufRead,
        syntax: GraphSyntax,
        to_graph_name: Option<&NamedOrBlankNode>,
        base_iri: Option<&str>,
    ) -> Result<()> {
        let mut store = self;
        load_graph(&mut store, reader, syntax, to_graph_name, base_iri)
    }

    /// Loads a dataset file (i.e. quads) into the store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::{MemoryStore, Result, DatasetSyntax};
    ///
    /// let store = MemoryStore::new();
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> <http://example.com> .";
    /// store.load_dataset(file.as_ref(), DatasetSyntax::NQuads, None);
    ///
    /// // quad filter
    /// let results: Vec<Quad> = store.quads_for_pattern(None, None, None, None).collect();
    /// let ex = NamedNode::parse("http://example.com")?;
    /// assert_eq!(vec![Quad::new(ex.clone(), ex.clone(), ex.clone(), Some(ex.into()))], results);
    /// # Result::Ok(())
    /// ```
    pub fn load_dataset(
        &self,
        reader: impl BufRead,
        syntax: DatasetSyntax,
        base_iri: Option<&str>,
    ) -> Result<()> {
        let mut store = self;
        load_dataset(&mut store, reader, syntax, base_iri)
    }

    /// Adds a quad to this store.
    #[allow(clippy::needless_pass_by_value)]
    pub fn insert(&self, quad: Quad) {
        let mut store = self;
        let quad = store.encode_quad(&quad).unwrap(); // Could never fail
        store.insert_encoded(&quad).unwrap(); // Could never fail
    }

    /// Removes a quad from this store.
    pub fn remove(&self, quad: &Quad) {
        let mut store = self;
        let quad = quad.into();
        store.remove_encoded(&quad).unwrap(); // Could never fail
    }

    /// Returns if the current dataset is [isomorphic](https://www.w3.org/TR/rdf11-concepts/#dfn-dataset-isomorphism) with another one.
    ///
    /// Warning: This implementation worst-case complexity is in O(n!)
    pub fn is_isomorphic(&self, other: &Self) -> bool {
        are_datasets_isomorphic(self, other)
    }

    fn indexes(&self) -> RwLockReadGuard<'_, MemoryStoreIndexes> {
        self.indexes
            .read()
            .expect("the Memory store mutex has been poisoned because of a panic")
    }

    fn indexes_mut(&self) -> RwLockWriteGuard<'_, MemoryStoreIndexes> {
        self.indexes
            .write()
            .expect("the Memory store mutex has been poisoned because of a panic")
    }

    fn contains_encoded(&self, quad: &EncodedQuad) -> bool {
        self.indexes().spog.get(&quad.subject).map_or(false, |pog| {
            pog.get(&quad.predicate).map_or(false, |og| {
                og.get(&quad.object)
                    .map_or(false, |g| g.contains(&quad.graph_name))
            })
        })
    }

    fn encoded_quads_for_pattern_inner(
        &self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> Vec<EncodedQuad> {
        match subject {
            Some(subject) => match predicate {
                Some(predicate) => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => {
                            let quad = EncodedQuad::new(subject, predicate, object, graph_name);
                            if self.contains_encoded(&quad) {
                                vec![quad]
                            } else {
                                vec![]
                            }
                        }
                        None => self
                            .encoded_quads_for_subject_predicate_object(subject, predicate, object),
                    },
                    None => match graph_name {
                        Some(graph_name) => self.encoded_quads_for_subject_predicate_graph(
                            subject, predicate, graph_name,
                        ),
                        None => self.encoded_quads_for_subject_predicate(subject, predicate),
                    },
                },
                None => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => {
                            self.encoded_quads_for_subject_object_graph(subject, object, graph_name)
                        }
                        None => self.encoded_quads_for_subject_object(subject, object),
                    },
                    None => match graph_name {
                        Some(graph_name) => {
                            self.encoded_quads_for_subject_graph(subject, graph_name)
                        }
                        None => self.encoded_quads_for_subject(subject),
                    },
                },
            },
            None => match predicate {
                Some(predicate) => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => self.encoded_quads_for_predicate_object_graph(
                            predicate, object, graph_name,
                        ),
                        None => self.encoded_quads_for_predicate_object(predicate, object),
                    },
                    None => match graph_name {
                        Some(graph_name) => {
                            self.encoded_quads_for_predicate_graph(predicate, graph_name)
                        }
                        None => self.encoded_quads_for_predicate(predicate),
                    },
                },
                None => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => self.encoded_quads_for_object_graph(object, graph_name),
                        None => self.encoded_quads_for_object(object),
                    },
                    None => match graph_name {
                        Some(graph_name) => self.encoded_quads_for_graph(graph_name),
                        None => self.encoded_quads(),
                    },
                },
            },
        }
    }

    fn encoded_quads(&self) -> Vec<EncodedQuad> {
        quad_map_flatten(&self.indexes().gspo)
            .map(|(g, s, p, o)| EncodedQuad::new(s, p, o, g))
            .collect()
    }

    fn encoded_quads_for_subject(&self, subject: EncodedTerm) -> Vec<EncodedQuad> {
        option_triple_map_flatten(self.indexes().spog.get(&subject))
            .map(|(p, o, g)| EncodedQuad::new(subject, p, o, g))
            .collect()
    }

    fn encoded_quads_for_subject_predicate(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        option_pair_map_flatten(
            self.indexes()
                .spog
                .get(&subject)
                .and_then(|pog| pog.get(&predicate)),
        )
        .map(|(o, g)| EncodedQuad::new(subject, predicate, o, g))
        .collect()
    }

    fn encoded_quads_for_subject_predicate_object(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        option_set_flatten(
            self.indexes()
                .spog
                .get(&subject)
                .and_then(|pog| pog.get(&predicate))
                .and_then(|og| og.get(&object)),
        )
        .map(|g| EncodedQuad::new(subject, predicate, object, g))
        .collect()
    }

    fn encoded_quads_for_subject_object(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        option_pair_map_flatten(
            self.indexes()
                .ospg
                .get(&object)
                .and_then(|spg| spg.get(&subject)),
        )
        .map(|(p, g)| EncodedQuad::new(subject, p, object, g))
        .collect()
    }

    fn encoded_quads_for_predicate(&self, predicate: EncodedTerm) -> Vec<EncodedQuad> {
        option_triple_map_flatten(self.indexes().posg.get(&predicate))
            .map(|(o, s, g)| EncodedQuad::new(s, predicate, o, g))
            .collect()
    }

    fn encoded_quads_for_predicate_object(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        option_pair_map_flatten(
            self.indexes()
                .posg
                .get(&predicate)
                .and_then(|osg| osg.get(&object)),
        )
        .map(|(s, g)| EncodedQuad::new(s, predicate, object, g))
        .collect()
    }

    fn encoded_quads_for_object(&self, object: EncodedTerm) -> Vec<EncodedQuad> {
        option_triple_map_flatten(self.indexes().ospg.get(&object))
            .map(|(s, p, g)| EncodedQuad::new(s, p, object, g))
            .collect()
    }

    fn encoded_quads_for_graph(&self, graph_name: EncodedTerm) -> Vec<EncodedQuad> {
        option_triple_map_flatten(self.indexes().gspo.get(&graph_name))
            .map(|(s, p, o)| EncodedQuad::new(s, p, o, graph_name))
            .collect()
    }

    fn encoded_quads_for_subject_graph(
        &self,
        subject: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        option_pair_map_flatten(
            self.indexes()
                .gspo
                .get(&graph_name)
                .and_then(|spo| spo.get(&subject)),
        )
        .map(|(p, o)| EncodedQuad::new(subject, p, o, graph_name))
        .collect()
    }

    fn encoded_quads_for_subject_predicate_graph(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        option_set_flatten(
            self.indexes()
                .gspo
                .get(&graph_name)
                .and_then(|spo| spo.get(&subject))
                .and_then(|po| po.get(&predicate)),
        )
        .map(|o| EncodedQuad::new(subject, predicate, o, graph_name))
        .collect()
    }

    fn encoded_quads_for_subject_object_graph(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        option_set_flatten(
            self.indexes()
                .gosp
                .get(&graph_name)
                .and_then(|osp| osp.get(&object))
                .and_then(|sp| sp.get(&subject)),
        )
        .map(|p| EncodedQuad::new(subject, p, object, graph_name))
        .collect()
    }

    fn encoded_quads_for_predicate_graph(
        &self,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        option_pair_map_flatten(
            self.indexes()
                .gpos
                .get(&graph_name)
                .and_then(|pos| pos.get(&predicate)),
        )
        .map(|(o, s)| EncodedQuad::new(s, predicate, o, graph_name))
        .collect()
    }

    fn encoded_quads_for_predicate_object_graph(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        option_set_flatten(
            self.indexes()
                .gpos
                .get(&graph_name)
                .and_then(|pos| pos.get(&predicate))
                .and_then(|os| os.get(&object)),
        )
        .map(|s| EncodedQuad::new(s, predicate, object, graph_name))
        .collect()
    }

    fn encoded_quads_for_object_graph(
        &self,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        option_pair_map_flatten(
            self.indexes()
                .gosp
                .get(&graph_name)
                .and_then(|osp| osp.get(&object)),
        )
        .map(|(s, p)| EncodedQuad::new(s, p, object, graph_name))
        .collect()
    }
}

impl StrLookup for MemoryStore {
    fn get_str(&self, id: StrHash) -> Result<Option<String>> {
        //TODO: avoid copy by adding a lifetime limit to get_str
        self.indexes().get_str(id)
    }
}

impl StrLookup for MemoryStoreIndexes {
    fn get_str(&self, id: StrHash) -> Result<Option<String>> {
        //TODO: avoid copy by adding a lifetime limit to get_str
        Ok(self.id2str.get(&id).cloned())
    }
}

impl StrContainer for MemoryStore {
    fn insert_str(&mut self, key: StrHash, value: &str) -> Result<()> {
        self.indexes_mut().insert_str(key, value)
    }
}

impl<'a> StrContainer for &'a MemoryStore {
    fn insert_str(&mut self, key: StrHash, value: &str) -> Result<()> {
        self.indexes_mut().insert_str(key, value)
    }
}

impl StrContainer for MemoryStoreIndexes {
    fn insert_str(&mut self, key: StrHash, value: &str) -> Result<()> {
        self.id2str.entry(key).or_insert_with(|| value.to_owned());
        Ok(())
    }
}

impl<'a> ReadableEncodedStore for MemoryStore {
    fn encoded_quads_for_pattern<'b>(
        &'b self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad>> + 'b> {
        Box::new(
            self.encoded_quads_for_pattern_inner(subject, predicate, object, graph_name)
                .into_iter()
                .map(Ok),
        )
    }
}

impl WritableEncodedStore for MemoryStore {
    fn insert_encoded(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.indexes_mut().insert_encoded(quad)
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.indexes_mut().remove_encoded(quad)
    }
}

impl<'a> WritableEncodedStore for &'a MemoryStore {
    fn insert_encoded(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.indexes_mut().insert_encoded(quad)
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.indexes_mut().remove_encoded(quad)
    }
}

impl WritableEncodedStore for MemoryStoreIndexes {
    fn insert_encoded(&mut self, quad: &EncodedQuad) -> Result<()> {
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
        Ok(())
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<()> {
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
        Ok(())
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

/// A prepared [SPARQL query](https://www.w3.org/TR/sparql11-query/) for the `MemoryStore`.
pub struct MemoryPreparedQuery(SimplePreparedQuery<MemoryStore>);

impl MemoryPreparedQuery {
    /// Evaluates the query and returns its results
    pub fn exec(&self) -> Result<QueryResult<'_>> {
        self.0.exec()
    }
}

/// Allows to insert and delete quads during a transaction with the `MemoryStore`.
pub struct MemoryTransaction<'a> {
    store: &'a MemoryStore,
    ops: Vec<TransactionOp>,
    strings: Vec<(StrHash, String)>,
}

enum TransactionOp {
    Insert(EncodedQuad),
    Delete(EncodedQuad),
}

impl<'a> MemoryTransaction<'a> {
    /// Loads a graph file (i.e. triples) into the store during the transaction.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::{MemoryStore, Result, GraphSyntax};
    ///
    /// let store = MemoryStore::new();
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> .";
    /// store.transaction(|transaction| {
    ///     store.load_graph(file.as_ref(), GraphSyntax::NTriples, None, None)
    /// })?;
    ///
    /// // quad filter
    /// let results: Vec<Quad> = store.quads_for_pattern(None, None, None, None).collect();
    /// let ex = NamedNode::parse("http://example.com")?;
    /// assert_eq!(vec![Quad::new(ex.clone(), ex.clone(), ex.clone(), None)], results);
    /// # Result::Ok(())
    /// ```
    pub fn load_graph(
        &mut self,
        reader: impl BufRead,
        syntax: GraphSyntax,
        to_graph_name: Option<&NamedOrBlankNode>,
        base_iri: Option<&str>,
    ) -> Result<()> {
        load_graph(self, reader, syntax, to_graph_name, base_iri)
    }

    /// Loads a dataset file (i.e. quads) into the store during the transaction.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::{MemoryStore, Result, DatasetSyntax};
    ///
    /// let store = MemoryStore::new();
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> <http://example.com> .";
    /// store.load_dataset(file.as_ref(), DatasetSyntax::NQuads, None);
    ///
    /// // quad filter
    /// let results: Vec<Quad> = store.quads_for_pattern(None, None, None, None).collect();
    /// let ex = NamedNode::parse("http://example.com")?;
    /// assert_eq!(vec![Quad::new(ex.clone(), ex.clone(), ex.clone(), Some(ex.into()))], results);
    /// # Result::Ok(())
    /// ```
    pub fn load_dataset(
        &mut self,
        reader: impl BufRead,
        syntax: DatasetSyntax,
        base_iri: Option<&str>,
    ) -> Result<()> {
        load_dataset(self, reader, syntax, base_iri)
    }

    /// Adds a quad to this store during the transaction.
    #[allow(clippy::needless_pass_by_value)]
    pub fn insert(&mut self, quad: Quad) {
        let quad = self.encode_quad(&quad).unwrap(); // Could never fail
        self.insert_encoded(&quad).unwrap(); // Could never fail
    }

    /// Removes a quad from this store during the transaction.
    pub fn remove(&mut self, quad: &Quad) {
        let quad = quad.into();
        self.remove_encoded(&quad).unwrap(); // Could never fail
    }

    fn commit(self) -> Result<()> {
        let mut indexes = self.store.indexes_mut();
        indexes.id2str.extend(self.strings);
        for op in self.ops {
            match op {
                TransactionOp::Insert(quad) => indexes.insert_encoded(&quad)?,
                TransactionOp::Delete(quad) => indexes.remove_encoded(&quad)?,
            }
        }
        Ok(())
    }
}

impl StrContainer for MemoryTransaction<'_> {
    fn insert_str(&mut self, key: StrHash, value: &str) -> Result<()> {
        self.strings.push((key, value.to_owned()));
        Ok(())
    }
}

impl WritableEncodedStore for MemoryTransaction<'_> {
    fn insert_encoded(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.ops.push(TransactionOp::Insert(*quad));
        Ok(())
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<()> {
        self.ops.push(TransactionOp::Delete(*quad));
        Ok(())
    }
}

impl FromIterator<Quad> for MemoryStore {
    fn from_iter<I: IntoIterator<Item = Quad>>(iter: I) -> Self {
        let mut store = MemoryStore::new();
        store.extend(iter);
        store
    }
}

impl Extend<Quad> for MemoryStore {
    fn extend<T: IntoIterator<Item = Quad>>(&mut self, iter: T) {
        for quad in iter {
            self.insert(quad);
        }
    }
}

impl fmt::Display for MemoryStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for t in self.quads_for_pattern(None, None, None, None) {
            writeln!(f, "{}", t)?;
        }
        Ok(())
    }
}

// Isomorphism implementation

fn split_hash_buckets(
    bnodes_by_hash: HashMap<u64, Vec<EncodedTerm>>,
    graph: &MemoryStore,
    distance: usize,
) -> HashMap<u64, Vec<EncodedTerm>> {
    let mut new_bnodes_by_hash = HashMap::default();

    for (hash, bnodes) in bnodes_by_hash {
        if bnodes.len() == 1 {
            new_bnodes_by_hash.insert(hash, bnodes); // Nothing to improve
        } else {
            for bnode in bnodes {
                let mut starts = vec![bnode];
                for _ in 0..distance {
                    let mut new_starts = Vec::default();
                    for s in starts {
                        for q in graph.encoded_quads_for_subject(s) {
                            if q.object.is_named_node() || q.object.is_blank_node() {
                                new_starts.push(q.object)
                            }
                        }
                        for t in graph.encoded_quads_for_object(s) {
                            new_starts.push(t.subject);
                        }
                    }
                    starts = new_starts;
                }

                // We do the hashing
                let mut hasher = DefaultHasher::default();
                hash.hash(&mut hasher); // We start with the previous hash

                // NB: we need to sort the triples to have the same hash
                let mut po_set = BTreeSet::default();
                for start in &starts {
                    for quad in graph.encoded_quads_for_subject(*start) {
                        if !quad.object.is_blank_node() {
                            po_set.insert(encode_term_pair(quad.predicate, quad.object));
                        }
                    }
                }
                for po in &po_set {
                    po.hash(&mut hasher);
                }

                let mut sp_set = BTreeSet::default();
                for start in starts {
                    for quad in graph.encoded_quads_for_object(start) {
                        if !quad.subject.is_blank_node() {
                            sp_set.insert(encode_term_pair(quad.subject, quad.predicate));
                        }
                    }
                }
                for sp in &sp_set {
                    sp.hash(&mut hasher);
                }

                new_bnodes_by_hash
                    .entry(hasher.finish())
                    .or_insert_with(Vec::default)
                    .push(bnode);
            }
        }
    }
    new_bnodes_by_hash
}

fn encode_term_pair(t1: EncodedTerm, t2: EncodedTerm) -> Vec<u8> {
    let mut vec = Vec::with_capacity(2 * WRITTEN_TERM_MAX_SIZE);
    write_term(&mut vec, t1);
    write_term(&mut vec, t2);
    vec
}

fn build_and_check_containment_from_hashes<'a>(
    a_bnodes_by_hash: &mut Vec<(u64, Vec<EncodedTerm>)>,
    b_bnodes_by_hash: &'a HashMap<u64, Vec<EncodedTerm>>,
    a_to_b_mapping: &mut HashMap<EncodedTerm, EncodedTerm>,
    a: &'a HashSet<EncodedQuad>,
    b: &'a HashSet<EncodedQuad>,
    current_a_nodes: &[EncodedTerm],
    current_b_nodes: &mut HashSet<EncodedTerm>,
) -> bool {
    if let Some((a_node, remaining_a_node)) = current_a_nodes.split_last() {
        let b_nodes = current_b_nodes.iter().cloned().collect::<Vec<_>>();
        for b_node in b_nodes {
            current_b_nodes.remove(&b_node);
            a_to_b_mapping.insert(*a_node, b_node);
            if check_is_contained_focused(a_to_b_mapping, *a_node, a, b)
                && build_and_check_containment_from_hashes(
                    a_bnodes_by_hash,
                    b_bnodes_by_hash,
                    a_to_b_mapping,
                    a,
                    b,
                    remaining_a_node,
                    current_b_nodes,
                )
            {
                return true;
            }
            current_b_nodes.insert(b_node);
        }
        a_to_b_mapping.remove(a_node);
        false
    } else {
        let (hash, new_a_nodes) = match a_bnodes_by_hash.pop() {
            Some(v) => v,
            None => return true,
        };

        let mut new_b_nodes = b_bnodes_by_hash
            .get(&hash)
            .map_or(HashSet::default(), |v| v.iter().cloned().collect());
        if new_a_nodes.len() != new_b_nodes.len() {
            return false;
        }

        if new_a_nodes.len() > 10 {
            eprintln!("Too big instance, aborting");
            return true; //TODO: Very very very bad
        }

        if build_and_check_containment_from_hashes(
            a_bnodes_by_hash,
            b_bnodes_by_hash,
            a_to_b_mapping,
            a,
            b,
            &new_a_nodes,
            &mut new_b_nodes,
        ) {
            true
        } else {
            a_bnodes_by_hash.push((hash, new_a_nodes));
            false
        }
    }
}

fn check_is_contained_focused<'a>(
    a_to_b_mapping: &mut HashMap<EncodedTerm, EncodedTerm>,
    a_bnode_focus: EncodedTerm,
    a: &'a HashSet<EncodedQuad>,
    b: &'a HashSet<EncodedQuad>,
) -> bool {
    let ts_a = a
        .iter()
        .filter(|t| t.subject == a_bnode_focus)
        .chain(a.iter().filter(|t| t.object == a_bnode_focus));
    //TODO: these filters
    for t_a in ts_a {
        let subject = if t_a.subject.is_blank_node() {
            if let Some(s_a) = a_to_b_mapping.get(&t_a.subject) {
                *s_a
            } else {
                continue; // We skip for now
            }
        } else {
            t_a.subject
        };
        let object = if t_a.object.is_blank_node() {
            if let Some(o_a) = a_to_b_mapping.get(&t_a.object) {
                *o_a
            } else {
                continue; // We skip for now
            }
        } else {
            t_a.object
        };
        if !b.contains(&EncodedQuad::new(
            subject,
            t_a.predicate,
            object,
            t_a.graph_name, //TODO: support blank node graph names
        )) {
            //TODO
            return false;
        }
    }

    true
}

fn graph_blank_nodes(graph: &HashSet<EncodedQuad>) -> Vec<EncodedTerm> {
    let mut blank_nodes: HashSet<EncodedTerm> = HashSet::default();
    for t in graph {
        if t.subject.is_blank_node() {
            blank_nodes.insert(t.subject);
        }
        if t.object.is_blank_node() {
            blank_nodes.insert(t.object);
        }
    }
    blank_nodes.into_iter().collect()
}

fn are_datasets_isomorphic(a: &MemoryStore, b: &MemoryStore) -> bool {
    /* TODO if a.len() != b.len() {
        return false;
    }*/

    // We check containment of everything buts triples with blank nodes
    let mut a_bnodes_triples = HashSet::default();
    for t in a.encoded_quads() {
        if t.subject.is_blank_node() || t.object.is_blank_node() {
            a_bnodes_triples.insert(t);
        } else if !b.contains_encoded(&t) {
            return false; // Triple in a not in b without blank nodes
        }
    }

    let mut b_bnodes_triples = HashSet::default();
    for t in b.encoded_quads() {
        if t.subject.is_blank_node() || t.object.is_blank_node() {
            b_bnodes_triples.insert(t);
        } else if !a.contains_encoded(&t) {
            return false; // Triple in a not in b without blank nodes
        }
    }

    let mut a_bnodes_by_hash = HashMap::default();
    a_bnodes_by_hash.insert(0, graph_blank_nodes(&a_bnodes_triples));
    let mut b_bnodes_by_hash = HashMap::default();
    b_bnodes_by_hash.insert(0, graph_blank_nodes(&b_bnodes_triples));

    for distance in 0..5 {
        let max_size = a_bnodes_by_hash.values().map(Vec::len).max().unwrap_or(0);
        if max_size < 2 {
            break; // We only have small buckets
        }

        a_bnodes_by_hash = split_hash_buckets(a_bnodes_by_hash, a, distance);
        b_bnodes_by_hash = split_hash_buckets(b_bnodes_by_hash, b, distance);

        // Hashes should have the same size
        if a_bnodes_by_hash.len() != b_bnodes_by_hash.len() {
            return false;
        }
    }

    let mut sorted_a_bnodes_by_hash: Vec<_> = a_bnodes_by_hash.into_iter().collect();
    sorted_a_bnodes_by_hash.sort_by(|(_, l1), (_, l2)| l1.len().cmp(&l2.len()));

    build_and_check_containment_from_hashes(
        &mut sorted_a_bnodes_by_hash,
        &b_bnodes_by_hash,
        &mut HashMap::default(),
        &a_bnodes_triples,
        &b_bnodes_triples,
        &[],
        &mut HashSet::default(),
    )
}
