use crate::model::{GraphNameRef, NamedOrBlankNodeRef, QuadRef, TermRef};
pub use crate::storage::error::StorageError;
use crate::storage::numeric_encoder::{
    insert_term, Decoder, EncodedQuad, EncodedTerm, StrHash, StrHashHasher, StrLookup,
};
use crate::storage::CorruptionError;
use dashmap::iter::Iter;
use dashmap::mapref::entry::Entry;
use dashmap::{DashMap, DashSet};
use oxrdf::Quad;
use rustc_hash::FxHasher;
use std::borrow::Borrow;
use std::error::Error;
use std::hash::{BuildHasherDefault, Hash, Hasher};
use std::mem::transmute;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, RwLock, Weak};

/// In-memory storage working with MVCC
///
/// Each quad and graph name is annotated by a version range, allowing to read old versions while updates are applied.
/// To simplify the implementation a single write transaction is currently allowed. This restriction should be lifted in the future.
#[derive(Clone)]
pub struct MemoryStorage {
    content: Arc<Content>,
    id2str: Arc<DashMap<StrHash, String, BuildHasherDefault<StrHashHasher>>>,
    version_counter: Arc<AtomicUsize>,
    transaction_counter: Arc<Mutex<usize>>,
}

struct Content {
    quad_set: DashSet<Arc<QuadListNode>, BuildHasherDefault<FxHasher>>,
    last_quad: RwLock<Option<Weak<QuadListNode>>>,
    last_quad_by_subject:
        DashMap<EncodedTerm, (Weak<QuadListNode>, u64), BuildHasherDefault<FxHasher>>,
    last_quad_by_predicate:
        DashMap<EncodedTerm, (Weak<QuadListNode>, u64), BuildHasherDefault<FxHasher>>,
    last_quad_by_object:
        DashMap<EncodedTerm, (Weak<QuadListNode>, u64), BuildHasherDefault<FxHasher>>,
    last_quad_by_graph_name:
        DashMap<EncodedTerm, (Weak<QuadListNode>, u64), BuildHasherDefault<FxHasher>>,
    graphs: DashMap<EncodedTerm, VersionRange>,
}

impl MemoryStorage {
    pub fn new() -> Self {
        Self {
            content: Arc::new(Content {
                quad_set: DashSet::default(),
                last_quad: RwLock::new(None),
                last_quad_by_subject: DashMap::default(),
                last_quad_by_predicate: DashMap::default(),
                last_quad_by_object: DashMap::default(),
                last_quad_by_graph_name: DashMap::default(),
                graphs: DashMap::default(),
            }),
            id2str: Arc::new(DashMap::default()),
            version_counter: Arc::new(AtomicUsize::new(0)),
            #[allow(clippy::mutex_atomic)]
            transaction_counter: Arc::new(Mutex::new(usize::MAX >> 1)),
        }
    }

    pub fn snapshot(&self) -> MemoryStorageReader {
        MemoryStorageReader {
            storage: self.clone(),
            snapshot_id: self.version_counter.load(Ordering::Acquire),
        }
    }

    #[allow(clippy::unwrap_in_result)]
    pub fn transaction<T, E: Error + 'static + From<StorageError>>(
        &self,
        f: impl for<'a> Fn(MemoryStorageWriter<'a>) -> Result<T, E>,
    ) -> Result<T, E> {
        let mut transaction_mutex = self.transaction_counter.lock().unwrap();
        *transaction_mutex += 1;
        let transaction_id = *transaction_mutex;
        let snapshot_id = self.version_counter.load(Ordering::Acquire);
        let mut operations = Vec::new();
        let result = f(MemoryStorageWriter {
            storage: self,
            log: &mut operations,
            transaction_id,
        });
        if result.is_ok() {
            let new_version_id = snapshot_id + 1;
            for operation in operations {
                match operation {
                    LogEntry::QuadNode(node) => {
                        node.range
                            .lock()
                            .unwrap()
                            .upgrade_transaction(transaction_id, new_version_id);
                    }
                    LogEntry::Graph(graph_name) => {
                        if let Some(mut entry) = self.content.graphs.get_mut(&graph_name) {
                            entry
                                .value_mut()
                                .upgrade_transaction(transaction_id, new_version_id)
                        }
                    }
                }
            }
            self.version_counter
                .store(new_version_id, Ordering::Release);
        } else {
            for operation in operations {
                match operation {
                    LogEntry::QuadNode(node) => {
                        node.range
                            .lock()
                            .unwrap()
                            .rollback_transaction(transaction_id);
                    }
                    LogEntry::Graph(graph_name) => {
                        if let Some(mut entry) = self.content.graphs.get_mut(&graph_name) {
                            entry.value_mut().rollback_transaction(transaction_id)
                        }
                    }
                }
            }
        }
        // TODO: garbage collection
        result
    }

    pub fn bulk_loader(&self) -> MemoryStorageBulkLoader {
        MemoryStorageBulkLoader {
            storage: self.clone(),
            hooks: Vec::new(),
        }
    }
}

#[derive(Clone)]
pub struct MemoryStorageReader {
    storage: MemoryStorage,
    snapshot_id: usize,
}

impl MemoryStorageReader {
    pub fn len(&self) -> usize {
        self.storage
            .content
            .quad_set
            .iter()
            .filter(|e| self.is_node_in_range(e))
            .count()
    }

    pub fn is_empty(&self) -> bool {
        !self
            .storage
            .content
            .quad_set
            .iter()
            .any(|e| self.is_node_in_range(&e))
    }

    pub fn contains(&self, quad: &EncodedQuad) -> bool {
        self.storage
            .content
            .quad_set
            .get(quad)
            .is_some_and(|node| self.is_node_in_range(&node))
    }

    pub fn quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> QuadIterator {
        fn get_start_and_count(
            map: &DashMap<EncodedTerm, (Weak<QuadListNode>, u64), BuildHasherDefault<FxHasher>>,
            term: Option<&EncodedTerm>,
        ) -> (Option<Weak<QuadListNode>>, u64) {
            let Some(term) = term else {
                return (None, u64::MAX);
            };
            map.view(term, |_, (node, count)| (Some(Weak::clone(node)), *count))
                .unwrap_or_default()
        }

        let (subject_start, subject_count) =
            get_start_and_count(&self.storage.content.last_quad_by_subject, subject);
        let (predicate_start, predicate_count) =
            get_start_and_count(&self.storage.content.last_quad_by_predicate, predicate);
        let (object_start, object_count) =
            get_start_and_count(&self.storage.content.last_quad_by_object, object);
        let (graph_name_start, graph_name_count) =
            get_start_and_count(&self.storage.content.last_quad_by_graph_name, graph_name);

        let (start, kind) = if subject.is_some()
            && subject_count <= predicate_count
            && subject_count <= object_count
            && subject_count <= graph_name_count
        {
            (subject_start, QuadIteratorKind::Subject)
        } else if predicate.is_some()
            && predicate_count <= object_count
            && predicate_count <= graph_name_count
        {
            (predicate_start, QuadIteratorKind::Predicate)
        } else if object.is_some() && object_count <= graph_name_count {
            (object_start, QuadIteratorKind::Object)
        } else if graph_name.is_some() {
            (graph_name_start, QuadIteratorKind::GraphName)
        } else {
            (
                self.storage.content.last_quad.read().unwrap().clone(),
                QuadIteratorKind::All,
            )
        };
        QuadIterator {
            reader: self.clone(),
            current: start,
            kind,
            expect_subject: if kind == QuadIteratorKind::Subject {
                None
            } else {
                subject.cloned()
            },
            expect_predicate: if kind == QuadIteratorKind::Predicate {
                None
            } else {
                predicate.cloned()
            },
            expect_object: if kind == QuadIteratorKind::Object {
                None
            } else {
                object.cloned()
            },
            expect_graph_name: if kind == QuadIteratorKind::GraphName {
                None
            } else {
                graph_name.cloned()
            },
        }
    }

    #[allow(unsafe_code)]
    pub fn named_graphs(&self) -> MemoryDecodingGraphIterator {
        MemoryDecodingGraphIterator {
            reader: self.clone(),
            // SAFETY: this is fine, the owning struct also owns the iterated data structure
            iter: unsafe {
                transmute::<Iter<'_, _, _>, Iter<'static, _, _>>(self.storage.content.graphs.iter())
            },
        }
    }

    pub fn contains_named_graph(&self, graph_name: &EncodedTerm) -> bool {
        self.storage
            .content
            .graphs
            .get(graph_name)
            .is_some_and(|range| self.is_in_range(&range))
    }

    pub fn contains_str(&self, key: &StrHash) -> bool {
        self.storage.id2str.contains_key(key)
    }

    /// Validates that all the storage invariants held in the data
    #[allow(clippy::unwrap_in_result)]
    pub fn validate(&self) -> Result<(), StorageError> {
        // All used named graphs are in graph set
        let expected_quad_len = self.storage.content.quad_set.len() as u64;

        // last quad chain
        let mut next = self.storage.content.last_quad.read().unwrap().clone();
        let mut count_last_quad = 0;
        while let Some(current) = next.take().and_then(|c| c.upgrade()) {
            count_last_quad += 1;
            if !self
                .storage
                .content
                .quad_set
                .get(&current.quad)
                .is_some_and(|e| Arc::ptr_eq(&e, &current))
            {
                return Err(
                    CorruptionError::new("Quad in previous chain but not in quad set").into(),
                );
            }
            self.decode_quad(&current.quad)?;
            if !current.quad.graph_name.is_default_graph()
                && !self
                    .storage
                    .content
                    .graphs
                    .contains_key(&current.quad.graph_name)
            {
                return Err(
                    CorruptionError::new("Quad in named graph that does not exists").into(),
                );
            };
            next.clone_from(&current.previous);
        }
        if count_last_quad != expected_quad_len {
            return Err(CorruptionError::new("Too many quads in quad_set").into());
        }

        // By subject chain
        let mut count_last_by_subject = 0;
        for entry in &self.storage.content.last_quad_by_subject {
            let mut next = Some(Weak::clone(&entry.value().0));
            let mut element_count = 0;
            while let Some(current) = next.take().and_then(|n| n.upgrade()) {
                element_count += 1;
                if current.quad.subject != *entry.key() {
                    return Err(CorruptionError::new("Quad in wrong list").into());
                }
                if !self
                    .storage
                    .content
                    .quad_set
                    .get(&current.quad)
                    .is_some_and(|e| Arc::ptr_eq(&e, &current))
                {
                    return Err(
                        CorruptionError::new("Quad in previous chain but not in quad set").into(),
                    );
                }
                next.clone_from(&current.previous_subject);
            }
            if element_count != entry.value().1 {
                return Err(CorruptionError::new("Too many quads in a chain").into());
            }
            count_last_by_subject += element_count;
        }
        if count_last_by_subject != expected_quad_len {
            return Err(CorruptionError::new("Too many quads in quad_set").into());
        }

        // By predicate chains
        let mut count_last_by_predicate = 0;
        for entry in &self.storage.content.last_quad_by_predicate {
            let mut next = Some(Weak::clone(&entry.value().0));
            let mut element_count = 0;
            while let Some(current) = next.take().and_then(|n| n.upgrade()) {
                element_count += 1;
                if current.quad.predicate != *entry.key() {
                    return Err(CorruptionError::new("Quad in wrong list").into());
                }
                if !self
                    .storage
                    .content
                    .quad_set
                    .get(&current.quad)
                    .is_some_and(|e| Arc::ptr_eq(&e, &current))
                {
                    return Err(
                        CorruptionError::new("Quad in previous chain but not in quad set").into(),
                    );
                }
                next.clone_from(&current.previous_predicate);
            }
            if element_count != entry.value().1 {
                return Err(CorruptionError::new("Too many quads in a chain").into());
            }
            count_last_by_predicate += element_count;
        }
        if count_last_by_predicate != expected_quad_len {
            return Err(CorruptionError::new("Too many quads in quad_set").into());
        }

        // By object chains
        let mut count_last_by_object = 0;
        for entry in &self.storage.content.last_quad_by_object {
            let mut next = Some(Weak::clone(&entry.value().0));
            let mut element_count = 0;
            while let Some(current) = next.take().and_then(|n| n.upgrade()) {
                element_count += 1;
                if current.quad.object != *entry.key() {
                    return Err(CorruptionError::new("Quad in wrong list").into());
                }
                if !self
                    .storage
                    .content
                    .quad_set
                    .get(&current.quad)
                    .is_some_and(|e| Arc::ptr_eq(&e, &current))
                {
                    return Err(
                        CorruptionError::new("Quad in previous chain but not in quad set").into(),
                    );
                }
                next.clone_from(&current.previous_object);
            }
            if element_count != entry.value().1 {
                return Err(CorruptionError::new("Too many quads in a chain").into());
            }
            count_last_by_object += element_count;
        }
        if count_last_by_object != expected_quad_len {
            return Err(CorruptionError::new("Too many quads in quad_set").into());
        }

        // By graph_name chains
        let mut count_last_by_graph_name = 0;
        for entry in &self.storage.content.last_quad_by_graph_name {
            let mut next = Some(Weak::clone(&entry.value().0));
            let mut element_count = 0;
            while let Some(current) = next.take().and_then(|n| n.upgrade()) {
                element_count += 1;
                if current.quad.graph_name != *entry.key() {
                    return Err(CorruptionError::new("Quad in wrong list").into());
                }
                if !self
                    .storage
                    .content
                    .quad_set
                    .get(&current.quad)
                    .is_some_and(|e| Arc::ptr_eq(&e, &current))
                {
                    return Err(
                        CorruptionError::new("Quad in previous chain but not in quad set").into(),
                    );
                }
                next.clone_from(&current.previous_graph_name);
            }
            if element_count != entry.value().1 {
                return Err(CorruptionError::new("Too many quads in a chain").into());
            }
            count_last_by_graph_name += element_count;
        }
        if count_last_by_graph_name != expected_quad_len {
            return Err(CorruptionError::new("Too many quads in quad_set").into());
        }

        Ok(())
    }

    fn is_in_range(&self, range: &VersionRange) -> bool {
        range.contains(self.snapshot_id)
    }

    fn is_node_in_range(&self, node: &QuadListNode) -> bool {
        let range = node.range.lock().unwrap();
        self.is_in_range(&range)
    }
}

impl StrLookup for MemoryStorageReader {
    fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
        Ok(self.storage.id2str.view(key, |_, v| v.clone()))
    }
}

pub struct MemoryStorageWriter<'a> {
    storage: &'a MemoryStorage,
    log: &'a mut Vec<LogEntry>,
    transaction_id: usize,
}

impl MemoryStorageWriter<'_> {
    pub fn reader(&self) -> MemoryStorageReader {
        MemoryStorageReader {
            storage: self.storage.clone(),
            snapshot_id: self.transaction_id,
        }
    }

    pub fn insert(&mut self, quad: QuadRef<'_>) -> bool {
        let encoded: EncodedQuad = quad.into();
        if let Some(node) = self
            .storage
            .content
            .quad_set
            .get(&encoded)
            .map(|node| Arc::clone(&node))
        {
            let added = node.range.lock().unwrap().add(self.transaction_id);
            if added {
                self.log.push(LogEntry::QuadNode(node));
                if !quad.graph_name.is_default_graph()
                    && self
                        .storage
                        .content
                        .graphs
                        .get_mut(&encoded.graph_name)
                        .unwrap()
                        .add(self.transaction_id)
                {
                    self.log.push(LogEntry::Graph(encoded.graph_name.clone()));
                }
            }
            added
        } else {
            let node = Arc::new(QuadListNode {
                quad: encoded.clone(),
                range: Mutex::new(VersionRange::Start(self.transaction_id)),
                previous: self.storage.content.last_quad.read().unwrap().clone(),
                previous_subject: self
                    .storage
                    .content
                    .last_quad_by_subject
                    .view(&encoded.subject, |_, (node, _)| Weak::clone(node)),
                previous_predicate: self
                    .storage
                    .content
                    .last_quad_by_predicate
                    .view(&encoded.predicate, |_, (node, _)| Weak::clone(node)),
                previous_object: self
                    .storage
                    .content
                    .last_quad_by_object
                    .view(&encoded.object, |_, (node, _)| Weak::clone(node)),
                previous_graph_name: self
                    .storage
                    .content
                    .last_quad_by_graph_name
                    .view(&encoded.graph_name, |_, (node, _)| Weak::clone(node)),
            });
            self.storage.content.quad_set.insert(Arc::clone(&node));
            *self.storage.content.last_quad.write().unwrap() = Some(Arc::downgrade(&node));
            self.storage
                .content
                .last_quad_by_subject
                .entry(encoded.subject.clone())
                .and_modify(|(e, count)| {
                    *e = Arc::downgrade(&node);
                    *count += 1;
                })
                .or_insert_with(|| (Arc::downgrade(&node), 1));
            self.storage
                .content
                .last_quad_by_predicate
                .entry(encoded.predicate.clone())
                .and_modify(|(e, count)| {
                    *e = Arc::downgrade(&node);
                    *count += 1;
                })
                .or_insert_with(|| (Arc::downgrade(&node), 1));
            self.storage
                .content
                .last_quad_by_object
                .entry(encoded.object.clone())
                .and_modify(|(e, count)| {
                    *e = Arc::downgrade(&node);
                    *count += 1;
                })
                .or_insert_with(|| (Arc::downgrade(&node), 1));
            self.storage
                .content
                .last_quad_by_graph_name
                .entry(encoded.graph_name.clone())
                .and_modify(|(e, count)| {
                    *e = Arc::downgrade(&node);
                    *count += 1;
                })
                .or_insert_with(|| (Arc::downgrade(&node), 1));

            self.insert_term(quad.subject.into(), &encoded.subject);
            self.insert_term(quad.predicate.into(), &encoded.predicate);
            self.insert_term(quad.object, &encoded.object);

            match quad.graph_name {
                GraphNameRef::NamedNode(graph_name) => {
                    self.insert_encoded_named_graph(graph_name.into(), encoded.graph_name.clone());
                }
                GraphNameRef::BlankNode(graph_name) => {
                    self.insert_encoded_named_graph(graph_name.into(), encoded.graph_name.clone());
                }
                GraphNameRef::DefaultGraph => (),
            }
            self.log.push(LogEntry::QuadNode(node));
            true
        }
    }

    pub fn insert_named_graph(&mut self, graph_name: NamedOrBlankNodeRef<'_>) -> bool {
        self.insert_encoded_named_graph(graph_name, graph_name.into())
    }

    fn insert_encoded_named_graph(
        &mut self,
        graph_name: NamedOrBlankNodeRef<'_>,
        encoded_graph_name: EncodedTerm,
    ) -> bool {
        let added = match self
            .storage
            .content
            .graphs
            .entry(encoded_graph_name.clone())
        {
            Entry::Occupied(mut entry) => entry.get_mut().add(self.transaction_id),
            Entry::Vacant(entry) => {
                entry.insert(VersionRange::Start(self.transaction_id));
                self.insert_term(graph_name.into(), &encoded_graph_name);
                true
            }
        };
        if added {
            self.log.push(LogEntry::Graph(encoded_graph_name));
        }
        added
    }

    fn insert_term(&self, term: TermRef<'_>, encoded: &EncodedTerm) {
        insert_term(term, encoded, &mut |key, value| {
            self.insert_str(key, value);
            Ok(())
        })
        .unwrap()
    }

    fn insert_str(&self, key: &StrHash, value: &str) {
        let inserted = self
            .storage
            .id2str
            .entry(*key)
            .or_insert_with(|| value.into());
        debug_assert_eq!(*inserted, value, "Hash conflict for two strings");
    }

    pub fn remove(&mut self, quad: QuadRef<'_>) -> bool {
        self.remove_encoded(&quad.into())
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> bool {
        let Some(node) = self
            .storage
            .content
            .quad_set
            .get(quad)
            .map(|node| Arc::clone(&node))
        else {
            return false;
        };
        let removed = node.range.lock().unwrap().remove(self.transaction_id);
        if removed {
            self.log.push(LogEntry::QuadNode(node));
        }
        removed
    }

    pub fn clear_graph(&mut self, graph_name: GraphNameRef<'_>) {
        self.clear_encoded_graph(&graph_name.into())
    }

    fn clear_encoded_graph(&mut self, graph_name: &EncodedTerm) {
        let mut next = self
            .storage
            .content
            .last_quad_by_graph_name
            .view(graph_name, |_, (node, _)| Weak::clone(node));
        while let Some(current) = next.take().and_then(|c| c.upgrade()) {
            if current.range.lock().unwrap().remove(self.transaction_id) {
                self.log.push(LogEntry::QuadNode(Arc::clone(&current)));
            }
            next.clone_from(&current.previous_graph_name);
        }
    }

    pub fn clear_all_named_graphs(&mut self) {
        for graph_name in self.reader().named_graphs() {
            self.clear_encoded_graph(&graph_name)
        }
    }

    pub fn clear_all_graphs(&mut self) {
        self.storage.content.quad_set.iter().for_each(|node| {
            if node.range.lock().unwrap().remove(self.transaction_id) {
                self.log.push(LogEntry::QuadNode(Arc::clone(&node)));
            }
        });
    }

    pub fn remove_named_graph(&mut self, graph_name: NamedOrBlankNodeRef<'_>) -> bool {
        self.remove_encoded_named_graph(&graph_name.into())
    }

    fn remove_encoded_named_graph(&mut self, graph_name: &EncodedTerm) -> bool {
        self.clear_encoded_graph(graph_name);
        let removed = self
            .storage
            .content
            .graphs
            .get_mut(graph_name)
            .is_some_and(|mut entry| entry.value_mut().remove(self.transaction_id));
        if removed {
            self.log.push(LogEntry::Graph(graph_name.clone()));
        }
        removed
    }

    pub fn remove_all_named_graphs(&mut self) {
        self.clear_all_named_graphs();
        self.do_remove_graphs();
    }

    fn do_remove_graphs(&mut self) {
        self.storage
            .content
            .graphs
            .iter_mut()
            .for_each(|mut entry| {
                if entry.value_mut().remove(self.transaction_id) {
                    self.log.push(LogEntry::Graph(entry.key().clone()));
                }
            });
    }

    pub fn clear(&mut self) {
        self.clear_all_graphs();
        self.do_remove_graphs();
    }
}

pub struct QuadIterator {
    reader: MemoryStorageReader,
    current: Option<Weak<QuadListNode>>,
    kind: QuadIteratorKind,
    expect_subject: Option<EncodedTerm>,
    expect_predicate: Option<EncodedTerm>,
    expect_object: Option<EncodedTerm>,
    expect_graph_name: Option<EncodedTerm>,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum QuadIteratorKind {
    All,
    Subject,
    Predicate,
    Object,
    GraphName,
}

impl Iterator for QuadIterator {
    type Item = EncodedQuad;

    fn next(&mut self) -> Option<EncodedQuad> {
        loop {
            let current = self.current.take()?.upgrade()?;
            self.current = match self.kind {
                QuadIteratorKind::All => current.previous.clone(),
                QuadIteratorKind::Subject => current.previous_subject.clone(),
                QuadIteratorKind::Predicate => current.previous_predicate.clone(),
                QuadIteratorKind::Object => current.previous_object.clone(),
                QuadIteratorKind::GraphName => current.previous_graph_name.clone(),
            };
            if !self.reader.is_node_in_range(&current) {
                continue;
            }
            if let Some(expect_subject) = &self.expect_subject {
                if current.quad.subject != *expect_subject {
                    continue;
                }
            }
            if let Some(expect_predicate) = &self.expect_predicate {
                if current.quad.predicate != *expect_predicate {
                    continue;
                }
            }
            if let Some(expect_object) = &self.expect_object {
                if current.quad.object != *expect_object {
                    continue;
                }
            }
            if let Some(expect_graph_name) = &self.expect_graph_name {
                if current.quad.graph_name != *expect_graph_name {
                    continue;
                }
            }
            return Some(current.quad.clone());
        }
    }
}

pub struct MemoryDecodingGraphIterator {
    reader: MemoryStorageReader, // Needed to make sure the underlying map is not GCed
    iter: Iter<'static, EncodedTerm, VersionRange>,
}

impl Iterator for MemoryDecodingGraphIterator {
    type Item = EncodedTerm;

    fn next(&mut self) -> Option<EncodedTerm> {
        loop {
            let entry = self.iter.next()?;
            if self.reader.is_in_range(entry.value()) {
                return Some(entry.key().clone());
            }
        }
    }
}

#[must_use]
pub struct MemoryStorageBulkLoader {
    storage: MemoryStorage,
    hooks: Vec<Box<dyn Fn(u64)>>,
}

impl MemoryStorageBulkLoader {
    pub fn on_progress(mut self, callback: impl Fn(u64) + 'static) -> Self {
        self.hooks.push(Box::new(callback));
        self
    }

    #[allow(clippy::unwrap_in_result)]
    pub fn load<EI, EO: From<StorageError> + From<EI>>(
        &self,
        quads: impl IntoIterator<Item = Result<Quad, EI>>,
    ) -> Result<(), EO> {
        // We lock content here to make sure there is not a transaction committing at the same time
        let _transaction_lock = self.storage.transaction_counter.lock().unwrap();
        let mut done_counter = 0;
        let version_id = self.storage.version_counter.load(Ordering::Acquire) + 1;
        let mut log = Vec::new();
        for quad in quads {
            MemoryStorageWriter {
                storage: &self.storage,
                log: &mut log,
                transaction_id: version_id,
            }
            .insert(quad?.as_ref());
            log.clear();
            done_counter += 1;
            if done_counter % 1_000_000 == 0 {
                for hook in &self.hooks {
                    hook(done_counter);
                }
            }
        }
        self.storage
            .version_counter
            .store(version_id, Ordering::Release);
        Ok(())
    }
}

enum LogEntry {
    QuadNode(Arc<QuadListNode>),
    Graph(EncodedTerm),
}

struct QuadListNode {
    quad: EncodedQuad,
    range: Mutex<VersionRange>,
    previous: Option<Weak<Self>>,
    previous_subject: Option<Weak<Self>>,
    previous_predicate: Option<Weak<Self>>,
    previous_object: Option<Weak<Self>>,
    previous_graph_name: Option<Weak<Self>>,
}

impl PartialEq for QuadListNode {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.quad == other.quad
    }
}

impl Eq for QuadListNode {}

impl Hash for QuadListNode {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.quad.hash(state)
    }
}

impl Borrow<EncodedQuad> for Arc<QuadListNode> {
    fn borrow(&self) -> &EncodedQuad {
        &self.quad
    }
}

// TODO: reduce the size to 128bits
#[derive(Default, Eq, PartialEq, Clone)]
enum VersionRange {
    #[default]
    Empty,
    Start(usize),
    StartEnd(usize, usize),
    Bigger(Box<[usize]>),
}

impl VersionRange {
    fn contains(&self, version: usize) -> bool {
        match self {
            VersionRange::Empty => false,
            VersionRange::Start(start) => *start <= version,
            VersionRange::StartEnd(start, end) => *start <= version && version < *end,
            VersionRange::Bigger(range) => {
                for start_end in range.chunks(2) {
                    match start_end {
                        [start, end] => {
                            if *start <= version && version < *end {
                                return true;
                            }
                        }
                        [start] => {
                            if *start <= version {
                                return true;
                            }
                        }
                        _ => (),
                    }
                }
                false
            }
        }
    }

    fn add(&mut self, version: usize) -> bool {
        match self {
            VersionRange::Empty => {
                *self = VersionRange::Start(version);
                true
            }
            VersionRange::Start(_) => false,
            VersionRange::StartEnd(start, end) => {
                *self = if version == *end {
                    VersionRange::Start(*start)
                } else {
                    VersionRange::Bigger(Box::new([*start, *end, version]))
                };
                true
            }
            VersionRange::Bigger(vec) => {
                if vec.len() % 2 == 0 {
                    *self = VersionRange::Bigger(if vec.ends_with(&[version]) {
                        pop_boxed_slice(vec)
                    } else {
                        push_boxed_slice(vec, version)
                    });
                    true
                } else {
                    false
                }
            }
        }
    }

    fn remove(&mut self, version: usize) -> bool {
        match self {
            VersionRange::Empty | VersionRange::StartEnd(_, _) => false,
            VersionRange::Start(start) => {
                *self = if *start == version {
                    VersionRange::Empty
                } else {
                    VersionRange::StartEnd(*start, version)
                };
                true
            }
            VersionRange::Bigger(vec) => {
                if vec.len() % 2 == 0 {
                    false
                } else {
                    *self = if vec.ends_with(&[version]) {
                        match vec.as_ref() {
                            [start, end, _] => Self::StartEnd(*start, *end),
                            _ => Self::Bigger(pop_boxed_slice(vec)),
                        }
                    } else {
                        Self::Bigger(push_boxed_slice(vec, version))
                    };
                    true
                }
            }
        }
    }

    fn upgrade_transaction(&mut self, transaction_id: usize, version_id: usize) {
        match self {
            VersionRange::Empty => (),
            VersionRange::Start(start) => {
                if *start == transaction_id {
                    *start = version_id;
                }
            }
            VersionRange::StartEnd(_, end) => {
                if *end == transaction_id {
                    *end = version_id
                }
            }
            VersionRange::Bigger(vec) => {
                if vec.ends_with(&[transaction_id]) {
                    vec[vec.len() - 1] = version_id
                }
            }
        }
    }

    fn rollback_transaction(&mut self, transaction_id: usize) {
        match self {
            VersionRange::Empty => (),
            VersionRange::Start(start) => {
                if *start == transaction_id {
                    *self = VersionRange::Empty;
                }
            }
            VersionRange::StartEnd(start, end) => {
                if *end == transaction_id {
                    *self = VersionRange::Start(*start)
                }
            }
            VersionRange::Bigger(vec) => {
                if vec.ends_with(&[transaction_id]) {
                    *self = match vec.as_ref() {
                        [start, end, _] => Self::StartEnd(*start, *end),
                        _ => Self::Bigger(pop_boxed_slice(vec)),
                    }
                }
            }
        }
    }
}

fn push_boxed_slice<T: Copy>(slice: &[T], element: T) -> Box<[T]> {
    let mut out = Vec::with_capacity(slice.len() + 1);
    out.extend_from_slice(slice);
    out.push(element);
    out.into_boxed_slice()
}

fn pop_boxed_slice<T: Copy>(slice: &[T]) -> Box<[T]> {
    slice[..slice.len() - 1].into()
}

#[cfg(test)]
#[allow(clippy::panic_in_result_fn)]
mod tests {
    use super::*;
    use oxrdf::NamedNodeRef;

    #[test]
    fn test_range() {
        let mut range = VersionRange::default();

        assert!(range.add(1));
        assert!(!range.add(1));
        assert!(range.contains(1));
        assert!(!range.contains(0));
        assert!(range.contains(2));

        assert!(range.remove(1));
        assert!(!range.remove(1));
        assert!(!range.contains(1));

        assert!(range.add(1));
        assert!(range.remove(2));
        assert!(!range.remove(2));
        assert!(range.contains(1));
        assert!(!range.contains(2));

        assert!(range.add(2));
        assert!(range.contains(3));

        assert!(range.remove(2));
        assert!(range.add(4));
        assert!(range.remove(6));
        assert!(!range.contains(3));
        assert!(range.contains(4));
        assert!(!range.contains(6));
    }

    #[test]
    fn test_upgrade() {
        let mut range = VersionRange::default();

        assert!(range.add(1000));
        range.upgrade_transaction(999, 1);
        assert!(!range.contains(1));
        range.upgrade_transaction(1000, 1);
        assert!(range.contains(1));

        assert!(range.remove(1000));
        range.upgrade_transaction(999, 2);
        assert!(range.contains(2));
        range.upgrade_transaction(1000, 2);
        assert!(!range.contains(2));

        assert!(range.add(1000));
        range.upgrade_transaction(999, 3);
        assert!(!range.contains(3));
        range.upgrade_transaction(1000, 3);
        assert!(range.contains(3));
    }

    #[test]
    fn test_rollback() {
        let mut range = VersionRange::default();

        assert!(range.add(1000));
        range.rollback_transaction(999);
        assert!(range.contains(1000));
        range.rollback_transaction(1000);
        assert!(!range.contains(1));
    }

    #[test]
    fn test_transaction() -> Result<(), StorageError> {
        let example = NamedNodeRef::new_unchecked("http://example.com/1");
        let example2 = NamedNodeRef::new_unchecked("http://example.com/2");
        let encoded_example = EncodedTerm::from(example);
        let encoded_example2 = EncodedTerm::from(example2);
        let default_quad = QuadRef::new(example, example, example, GraphNameRef::DefaultGraph);
        let encoded_default_quad = EncodedQuad::from(default_quad);
        let named_graph_quad = QuadRef::new(example, example, example, example);
        let encoded_named_graph_quad = EncodedQuad::from(named_graph_quad);

        let storage = MemoryStorage::new();

        // We start with a graph
        let snapshot = storage.snapshot();
        storage.transaction(|mut writer| {
            writer.insert_named_graph(example.into());
            Ok::<_, StorageError>(())
        })?;
        assert!(!snapshot.contains_named_graph(&encoded_example));
        assert!(storage.snapshot().contains_named_graph(&encoded_example));
        storage.snapshot().validate()?;

        // We add two quads
        let snapshot = storage.snapshot();
        storage.transaction(|mut writer| {
            writer.insert(default_quad);
            writer.insert(named_graph_quad);
            Ok::<_, StorageError>(())
        })?;
        assert!(!snapshot.contains(&encoded_default_quad));
        assert!(!snapshot.contains(&encoded_named_graph_quad));
        assert!(storage.snapshot().contains(&encoded_default_quad));
        assert!(storage.snapshot().contains(&encoded_named_graph_quad));
        storage.snapshot().validate()?;

        // We remove the quads
        let snapshot = storage.snapshot();
        storage.transaction(|mut writer| {
            writer.remove(default_quad);
            writer.remove_named_graph(example.into());
            Ok::<_, StorageError>(())
        })?;
        assert!(snapshot.contains(&encoded_default_quad));
        assert!(snapshot.contains(&encoded_named_graph_quad));
        assert!(snapshot.contains_named_graph(&encoded_example));
        assert!(!storage.snapshot().contains(&encoded_default_quad));
        assert!(!storage.snapshot().contains(&encoded_named_graph_quad));
        assert!(!storage.snapshot().contains_named_graph(&encoded_example));
        storage.snapshot().validate()?;

        // We add the quads again but rollback
        let snapshot = storage.snapshot();
        assert!(storage
            .transaction(|mut writer| {
                writer.insert(default_quad);
                writer.insert(named_graph_quad);
                writer.insert_named_graph(example2.into());
                Err::<(), _>(StorageError::Other("foo".into()))
            })
            .is_err());
        assert!(!snapshot.contains(&encoded_default_quad));
        assert!(!snapshot.contains(&encoded_named_graph_quad));
        assert!(!snapshot.contains_named_graph(&encoded_example));
        assert!(!snapshot.contains_named_graph(&encoded_example2));
        assert!(!storage.snapshot().contains(&encoded_default_quad));
        assert!(!storage.snapshot().contains(&encoded_named_graph_quad));
        assert!(!storage.snapshot().contains_named_graph(&encoded_example));
        assert!(!storage.snapshot().contains_named_graph(&encoded_example2));
        storage.snapshot().validate()?;

        // We add quads and graph, then clear
        storage.bulk_loader().load::<StorageError, StorageError>([
            Ok(default_quad.into_owned()),
            Ok(named_graph_quad.into_owned()),
        ])?;
        storage.transaction(|mut writer| {
            writer.insert_named_graph(example2.into());
            Ok::<_, StorageError>(())
        })?;
        storage.transaction(|mut writer| {
            writer.clear();
            Ok::<_, StorageError>(())
        })?;
        assert!(!storage.snapshot().contains(&encoded_default_quad));
        assert!(!storage.snapshot().contains(&encoded_named_graph_quad));
        assert!(!storage.snapshot().contains_named_graph(&encoded_example));
        assert!(!storage.snapshot().contains_named_graph(&encoded_example2));
        assert!(storage.snapshot().is_empty());
        storage.snapshot().validate()?;

        Ok(())
    }
}
