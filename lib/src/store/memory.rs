//! In-memory store.

use crate::error::UnwrapInfallible;
use crate::model::*;
use crate::sparql::{QueryOptions, QueryResult, SimplePreparedQuery};
use crate::store::numeric_encoder::*;
use crate::store::*;
use crate::Result;
use crate::{DatasetSyntax, GraphSyntax};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::convert::Infallible;
use std::fmt;
use std::hash::{BuildHasherDefault, Hash, Hasher};
use std::io::BufRead;
use std::iter::FromIterator;
use std::mem::size_of;
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
/// let ex = NamedNode::new("http://example.com")?;
/// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
/// store.insert(quad.clone());
///
/// // quad filter
/// let results: Vec<Quad> = store.quads_for_pattern(Some(&ex.clone().into()), None, None, None).collect();
/// assert_eq!(vec![quad], results);
///
/// // SPARQL query
/// let prepared_query = store.prepare_query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())?;
/// if let QueryResult::Solutions(mut solutions) = prepared_query.exec()? {
///     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
/// }
/// # Result::Ok(())
/// ```
#[derive(Clone)]
pub struct MemoryStore {
    indexes: Arc<RwLock<MemoryStoreIndexes>>,
}

type TrivialHashMap<K, V> = HashMap<K, V, BuildHasherDefault<TrivialHasher>>;
type TrivialHashSet<T> = HashSet<T, BuildHasherDefault<TrivialHasher>>;
type TripleMap<T> = TrivialHashMap<T, TrivialHashMap<T, TrivialHashSet<T>>>;
type QuadMap<T> = TrivialHashMap<T, TripleMap<T>>;

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
        new.set_first_strings().unwrap_infallible();
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
    /// let ex = NamedNode::new("http://example.com")?;
    /// store.insert(Quad::new(ex.clone(), ex.clone(), ex.clone(), None));
    ///
    /// // SPARQL query
    /// let prepared_query = store.prepare_query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())?;
    /// if let QueryResult::Solutions(mut solutions) = prepared_query.exec()? {
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
    /// let ex = NamedNode::new("http://example.com")?;
    /// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
    /// store.insert(quad.clone());
    ///
    /// // quad filter
    /// let results: Vec<Quad> = store.quads_for_pattern(None, None, None, None).collect();
    /// assert_eq!(vec![quad], results);
    /// # Result::Ok(())
    /// ```
    pub fn quads_for_pattern(
        &self,
        subject: Option<&NamedOrBlankNode>,
        predicate: Option<&NamedNode>,
        object: Option<&Term>,
        graph_name: Option<&GraphName>,
    ) -> impl Iterator<Item = Quad> {
        let subject = subject.map(|s| s.into());
        let predicate = predicate.map(|p| p.into());
        let object = object.map(|o| o.into());
        let graph_name = graph_name.map(|g| g.into());
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

    /// Returns the number of quads in the store
    pub fn len(&self) -> usize {
        self.indexes()
            .spog
            .values()
            .map(|v| {
                v.values()
                    .map(|v| v.values().map(|v| v.len()).sum::<usize>())
                    .sum::<usize>()
            })
            .sum()
    }

    /// Returns if the store is empty
    pub fn is_empty(&self) -> bool {
        self.indexes().spog.is_empty()
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
    /// let ex = NamedNode::new("http://example.com")?;
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
    /// store.load_graph(file.as_ref(), GraphSyntax::NTriples, &GraphName::DefaultGraph, None);
    ///
    /// // quad filter
    /// let results: Vec<Quad> = store.quads_for_pattern(None, None, None, None).collect();
    /// let ex = NamedNode::new("http://example.com")?;
    /// assert_eq!(vec![Quad::new(ex.clone(), ex.clone(), ex.clone(), None)], results);
    /// # Result::Ok(())
    /// ```
    pub fn load_graph(
        &self,
        reader: impl BufRead,
        syntax: GraphSyntax,
        to_graph_name: &GraphName,
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
    /// let ex = NamedNode::new("http://example.com")?;
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
        let quad = store.encode_quad(&quad).unwrap_infallible();
        store.insert_encoded(&quad).unwrap_infallible();
    }

    /// Removes a quad from this store.
    pub fn remove(&self, quad: &Quad) {
        let mut store = self;
        let quad = quad.into();
        store.remove_encoded(&quad).unwrap_infallible();
    }

    /// Returns if the current dataset is [isomorphic](https://www.w3.org/TR/rdf11-concepts/#dfn-dataset-isomorphism) with another one.
    ///
    /// It is implemented using the canonicalization approach presented in
    /// [Canonical Forms for Isomorphic and Equivalent RDF Graphs: Algorithms for Leaning and Labelling Blank Nodes, Aidan Hogan, 2017](http://aidanhogan.com/docs/rdf-canonicalisation.pdf)
    ///
    /// Warning: This implementation worst-case complexity is in O(b!) with b the number of blank node node in the input graphs.
    pub fn is_isomorphic(&self, other: &Self) -> bool {
        iso_canonicalize(self) == iso_canonicalize(other)
    }

    #[allow(clippy::expect_used)]
    fn indexes(&self) -> RwLockReadGuard<'_, MemoryStoreIndexes> {
        self.indexes
            .read()
            .expect("the Memory store mutex has been poisoned because of a panic")
    }

    #[allow(clippy::expect_used)]
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
    type Error = Infallible;

    fn get_str(&self, id: StrHash) -> std::result::Result<Option<String>, Infallible> {
        //TODO: avoid copy by adding a lifetime limit to get_str
        self.indexes().get_str(id)
    }
}

impl StrLookup for MemoryStoreIndexes {
    type Error = Infallible;

    fn get_str(&self, id: StrHash) -> std::result::Result<Option<String>, Infallible> {
        //TODO: avoid copy by adding a lifetime limit to get_str
        Ok(self.id2str.get(&id).cloned())
    }
}

impl StrContainer for MemoryStore {
    type Error = Infallible;

    fn insert_str(&mut self, key: StrHash, value: &str) -> std::result::Result<(), Infallible> {
        self.indexes_mut().insert_str(key, value)
    }
}

impl<'a> StrContainer for &'a MemoryStore {
    type Error = Infallible;

    fn insert_str(&mut self, key: StrHash, value: &str) -> std::result::Result<(), Infallible> {
        self.indexes_mut().insert_str(key, value)
    }
}

impl StrContainer for MemoryStoreIndexes {
    type Error = Infallible;

    fn insert_str(&mut self, key: StrHash, value: &str) -> std::result::Result<(), Infallible> {
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
    type Error = Infallible;

    fn insert_encoded(&mut self, quad: &EncodedQuad) -> std::result::Result<(), Infallible> {
        self.indexes_mut().insert_encoded(quad)
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> std::result::Result<(), Infallible> {
        self.indexes_mut().remove_encoded(quad)
    }
}

impl<'a> WritableEncodedStore for &'a MemoryStore {
    type Error = Infallible;

    fn insert_encoded(&mut self, quad: &EncodedQuad) -> std::result::Result<(), Infallible> {
        self.indexes_mut().insert_encoded(quad)
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> std::result::Result<(), Infallible> {
        self.indexes_mut().remove_encoded(quad)
    }
}

impl WritableEncodedStore for MemoryStoreIndexes {
    type Error = Infallible;

    fn insert_encoded(&mut self, quad: &EncodedQuad) -> std::result::Result<(), Infallible> {
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

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> std::result::Result<(), Infallible> {
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

fn option_set_flatten<'a, T: Clone>(
    i: Option<&'a TrivialHashSet<T>>,
) -> impl Iterator<Item = T> + 'a {
    i.into_iter().flat_map(|s| s.iter().cloned())
}

fn option_pair_map_flatten<'a, T: Copy>(
    i: Option<&'a TrivialHashMap<T, TrivialHashSet<T>>>,
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
    ///     store.load_graph(file.as_ref(), GraphSyntax::NTriples, &GraphName::DefaultGraph, None)
    /// })?;
    ///
    /// // quad filter
    /// let results: Vec<Quad> = store.quads_for_pattern(None, None, None, None).collect();
    /// let ex = NamedNode::new("http://example.com")?;
    /// assert_eq!(vec![Quad::new(ex.clone(), ex.clone(), ex.clone(), None)], results);
    /// # Result::Ok(())
    /// ```
    pub fn load_graph(
        &mut self,
        reader: impl BufRead,
        syntax: GraphSyntax,
        to_graph_name: &GraphName,
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
    /// let ex = NamedNode::new("http://example.com")?;
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
        let quad = self.encode_quad(&quad).unwrap_infallible();
        self.insert_encoded(&quad).unwrap_infallible();
    }

    /// Removes a quad from this store during the transaction.
    pub fn remove(&mut self, quad: &Quad) {
        let quad = quad.into();
        self.remove_encoded(&quad).unwrap_infallible();
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
    type Error = Infallible;

    fn insert_str(&mut self, key: StrHash, value: &str) -> std::result::Result<(), Infallible> {
        self.strings.push((key, value.to_owned()));
        Ok(())
    }
}

impl WritableEncodedStore for MemoryTransaction<'_> {
    type Error = Infallible;

    fn insert_encoded(&mut self, quad: &EncodedQuad) -> std::result::Result<(), Infallible> {
        self.ops.push(TransactionOp::Insert(*quad));
        Ok(())
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> std::result::Result<(), Infallible> {
        self.ops.push(TransactionOp::Delete(*quad));
        Ok(())
    }
}

impl PartialEq for MemoryStore {
    fn eq(&self, other: &Self) -> bool {
        self.indexes().spog == other.indexes().spog
    }
}

impl Eq for MemoryStore {}

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

fn iso_canonicalize(g: &MemoryStore) -> Vec<Vec<u8>> {
    let bnodes = bnodes(g);
    let (hash, partition) = hash_bnodes(g, bnodes.into_iter().map(|bnode| (bnode, 0)).collect());
    distinguish(g, &hash, &partition)
}

fn distinguish(
    g: &MemoryStore,
    hash: &TrivialHashMap<EncodedTerm, u64>,
    partition: &[(u64, Vec<EncodedTerm>)],
) -> Vec<Vec<u8>> {
    let b_prime = partition
        .iter()
        .find_map(|(_, b)| if b.len() > 1 { Some(b) } else { None });
    if let Some(b_prime) = b_prime {
        b_prime
            .iter()
            .map(|b| {
                let mut hash_prime = hash.clone();
                hash_prime.insert(*b, hash_tuple((hash_prime[b], 22)));
                let (hash_prime_prime, partition_prime) = hash_bnodes(g, hash_prime);
                distinguish(g, &hash_prime_prime, &partition_prime)
            })
            .fold(None, |a, b| {
                Some(if let Some(a) = a {
                    if a <= b {
                        a
                    } else {
                        b
                    }
                } else {
                    b
                })
            })
            .unwrap_or_else(Vec::new)
    } else {
        label(g, hash)
    }
}

fn hash_bnodes(
    g: &MemoryStore,
    mut hashes: TrivialHashMap<EncodedTerm, u64>,
) -> (
    TrivialHashMap<EncodedTerm, u64>,
    Vec<(u64, Vec<EncodedTerm>)>,
) {
    let mut to_hash = Vec::new();
    let mut partition: TrivialHashMap<u64, Vec<EncodedTerm>> =
        TrivialHashMap::with_hasher(BuildHasherDefault::<TrivialHasher>::default());
    let mut partition_len = 0;
    loop {
        //TODO: improve termination
        let mut new_hashes =
            TrivialHashMap::with_hasher(BuildHasherDefault::<TrivialHasher>::default());
        for (bnode, old_hash) in &hashes {
            for q in g.encoded_quads_for_subject(*bnode) {
                to_hash.push((
                    hash_term(q.predicate, &hashes),
                    hash_term(q.object, &hashes),
                    hash_term(q.graph_name, &hashes),
                    0,
                ));
            }
            for q in g.encoded_quads_for_object(*bnode) {
                to_hash.push((
                    hash_term(q.subject, &hashes),
                    hash_term(q.predicate, &hashes),
                    hash_term(q.graph_name, &hashes),
                    1,
                ));
            }
            for q in g.encoded_quads_for_graph(*bnode) {
                to_hash.push((
                    hash_term(q.subject, &hashes),
                    hash_term(q.predicate, &hashes),
                    hash_term(q.object, &hashes),
                    2,
                ));
            }
            to_hash.sort();
            let hash = hash_tuple((old_hash, &to_hash));
            to_hash.clear();
            new_hashes.insert(*bnode, hash);
            partition.entry(hash).or_default().push(*bnode);
        }
        if partition.len() == partition_len {
            let mut partition: Vec<_> = partition.into_iter().collect();
            partition.sort_by(|(h1, b1), (h2, b2)| (b1.len(), h1).cmp(&(b2.len(), h2)));
            return (hashes, partition);
        }
        hashes = new_hashes;
        partition_len = partition.len();
        partition.clear();
    }
}

fn bnodes(g: &MemoryStore) -> TrivialHashSet<EncodedTerm> {
    let mut bnodes = TrivialHashSet::with_hasher(BuildHasherDefault::<TrivialHasher>::default());
    for q in g.encoded_quads() {
        if q.subject.is_blank_node() {
            bnodes.insert(q.subject);
        }
        if q.object.is_blank_node() {
            bnodes.insert(q.object);
        }
        if q.graph_name.is_blank_node() {
            bnodes.insert(q.graph_name);
        }
    }
    bnodes
}

fn label(g: &MemoryStore, hashes: &TrivialHashMap<EncodedTerm, u64>) -> Vec<Vec<u8>> {
    //TODO: better representation?
    let mut data: Vec<_> = g
        .encoded_quads()
        .into_iter()
        .map(|q| {
            let mut buffer = Vec::with_capacity(WRITTEN_TERM_MAX_SIZE * 4);
            write_spog_quad(
                &mut buffer,
                &EncodedQuad::new(
                    map_term(q.subject, hashes),
                    map_term(q.predicate, hashes),
                    map_term(q.object, hashes),
                    map_term(q.graph_name, hashes),
                ),
            );
            buffer
        })
        .collect();
    data.sort();
    data
}

fn map_term(term: EncodedTerm, bnodes_hash: &TrivialHashMap<EncodedTerm, u64>) -> EncodedTerm {
    if term.is_blank_node() {
        EncodedTerm::InlineBlankNode {
            id: (*bnodes_hash.get(&term).unwrap()).into(),
        }
    } else {
        term
    }
}

fn hash_term(term: EncodedTerm, bnodes_hash: &TrivialHashMap<EncodedTerm, u64>) -> u64 {
    if term.is_blank_node() {
        *bnodes_hash.get(&term).unwrap()
    } else {
        hash_tuple(term)
    }
}

fn hash_tuple(v: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    v.hash(&mut hasher);
    hasher.finish()
}

#[derive(Default)]
struct TrivialHasher {
    value: u64,
}

#[allow(
    arithmetic_overflow,
    clippy::cast_sign_loss,
    clippy::cast_possible_truncation
)]
impl Hasher for TrivialHasher {
    fn finish(&self) -> u64 {
        self.value
    }

    fn write(&mut self, bytes: &[u8]) {
        for chunk in bytes.chunks(size_of::<u64>()) {
            let mut val = [0; size_of::<u64>()];
            val[0..chunk.len()].copy_from_slice(chunk);
            self.write_u64(u64::from_le_bytes(val));
        }
    }

    fn write_u8(&mut self, i: u8) {
        self.write_u64(i.into());
    }

    fn write_u16(&mut self, i: u16) {
        self.write_u64(i.into());
    }

    fn write_u32(&mut self, i: u32) {
        self.write_u64(i.into());
    }

    fn write_u64(&mut self, i: u64) {
        self.value ^= i;
    }

    fn write_u128(&mut self, i: u128) {
        self.write_u64(i as u64);
        self.write_u64((i >> 64) as u64);
    }

    fn write_usize(&mut self, i: usize) {
        self.write_u64(i as u64);
        self.write_u64((i >> 64) as u64);
    }

    fn write_i8(&mut self, i: i8) {
        self.write_u8(i as u8);
    }

    fn write_i16(&mut self, i: i16) {
        self.write_u16(i as u16);
    }

    fn write_i32(&mut self, i: i32) {
        self.write_u32(i as u32);
    }

    fn write_i64(&mut self, i: i64) {
        self.write_u64(i as u64);
    }

    fn write_i128(&mut self, i: i128) {
        self.write_u128(i as u128);
    }

    fn write_isize(&mut self, i: isize) {
        self.write_usize(i as usize);
    }
}
