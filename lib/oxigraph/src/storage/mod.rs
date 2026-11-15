use crate::model::{GraphName, NamedOrBlankNode, Quad};
pub use crate::storage::error::{CorruptionError, LoaderError, SerializerError, StorageError};
use crate::storage::memory::{
    MemoryDecodingGraphIterator, MemoryStorage, MemoryStorageBulkLoader, MemoryStorageReader,
    MemoryStorageTransaction, QuadIterator,
};
use crate::storage::numeric_encoder::{
    EncodedQuad, EncodedTerm, EncodedTriple, StrHash, StrLookup,
};
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use crate::storage::rocksdb::{
    RocksDbChainedDecodingQuadIterator, RocksDbDecodingGraphIterator, RocksDbStorage,
    RocksDbStorageBulkLoader, RocksDbStorageOptions, RocksDbStorageReadableTransaction,
    RocksDbStorageReader, RocksDbStorageTransaction,
};
use oxstr::OxString;
use rustc_hash::{FxBuildHasher, FxHashSet};
use spareval::{Delta, IncrementalQueryNotifier};
use std::collections::{HashMap, HashSet, VecDeque};
use std::mem::take;
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use std::path::Path;
use std::sync::{Arc, Mutex, Weak};
#[cfg(not(target_family = "wasm"))]
use std::{io, thread};

#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
mod binary_encoder;
mod error;
mod memory;
pub mod numeric_encoder;
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
mod rocksdb;
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
mod rocksdb_wrapper;
pub mod small_string;

pub const DEFAULT_BULK_LOAD_BATCH_SIZE: usize = 1_000_000;

#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct StorageOptions {
    max_open_files: Option<i32>,
    fd_reserve: Option<u32>,
}

#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
impl StorageOptions {
    pub(crate) fn new(max_open_files: Option<i32>, fd_reserve: Option<u32>) -> Self {
        Self {
            max_open_files,
            fd_reserve,
        }
    }
}

/// Low level storage primitives
#[derive(Clone)]
pub struct Storage {
    kind: StorageKind,
    incremental_state: Arc<Mutex<IncrementalState>>,
    incremental_commit_lock: Arc<Mutex<()>>,
}

#[derive(Clone)]
enum StorageKind {
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    RocksDb(RocksDbStorage),
    Memory(MemoryStorage),
}

#[derive(Default)]
struct IncrementalState {
    subscribers: Vec<Option<IncrementalSubscriber>>,
    free_subscriber_slots: Vec<usize>,
    subscription_index: IncrementalSubscriptionIndex,
}

impl IncrementalState {
    fn for_each_matching_subscriber(
        &mut self,
        quad: &EncodedQuad,
        mut callback: impl FnMut(&mut IncrementalSubscriber),
    ) {
        let subscription_index = &self.subscription_index;
        let subscribers = &mut self.subscribers;
        subscription_index.for_each_candidate(quad, |id| {
            if let Some(subscriber) = subscribers.get_mut(id).and_then(Option::as_mut)
                && subscriber.pattern.matches(quad)
            {
                callback(subscriber);
            }
        });
    }
}

struct IncrementalSubscriber {
    pattern: IncrementalQuadPattern,
    changes: VecDeque<Delta<EncodedQuad>>,
    notifier: Weak<IncrementalQueryNotifier>,
}

#[derive(Clone)]
struct IncrementalQuadPattern {
    subject: Option<EncodedTerm>,
    predicate: Option<EncodedTerm>,
    object: Option<EncodedTerm>,
    graph_name: Option<Option<EncodedTerm>>,
}

impl IncrementalQuadPattern {
    fn matches(&self, quad: &EncodedQuad) -> bool {
        self.subject
            .as_ref()
            .is_none_or(|subject| quad.subject == *subject)
            && self
                .predicate
                .as_ref()
                .is_none_or(|predicate| quad.predicate == *predicate)
            && self
                .object
                .as_ref()
                .is_none_or(|object| quad.object == *object)
            && self
                .graph_name
                .as_ref()
                .is_none_or(|graph_name| match graph_name {
                    Some(graph_name) => quad.graph_name == *graph_name,
                    None => quad.graph_name.is_default_graph(),
                })
    }
}

#[derive(Default)]
struct IncrementalSubscriptionIndex {
    unconstrained: HashSet<usize>,
    by_subject: HashMap<EncodedTerm, HashSet<usize>>,
    by_predicate: HashMap<EncodedTerm, HashSet<usize>>,
    by_object: HashMap<EncodedTerm, HashSet<usize>>,
    by_graph_name: HashMap<EncodedTerm, HashSet<usize>>,
}

impl IncrementalSubscriptionIndex {
    fn insert(&mut self, id: usize, pattern: &IncrementalQuadPattern) {
        let mut constrained = false;
        if let Some(subject) = &pattern.subject {
            self.by_subject
                .entry(subject.clone())
                .or_default()
                .insert(id);
            constrained = true;
        }
        if let Some(predicate) = &pattern.predicate {
            self.by_predicate
                .entry(predicate.clone())
                .or_default()
                .insert(id);
            constrained = true;
        }
        if let Some(object) = &pattern.object {
            self.by_object.entry(object.clone()).or_default().insert(id);
            constrained = true;
        }
        if let Some(graph_name) = &pattern.graph_name {
            self.by_graph_name
                .entry(graph_name.clone().unwrap_or(EncodedTerm::DefaultGraph))
                .or_default()
                .insert(id);
            constrained = true;
        }
        if !constrained {
            self.unconstrained.insert(id);
        }
    }

    fn remove(&mut self, id: usize, pattern: &IncrementalQuadPattern) {
        if let Some(subject) = &pattern.subject {
            remove_subscription_index_entry(&mut self.by_subject, subject, id);
        }
        if let Some(predicate) = &pattern.predicate {
            remove_subscription_index_entry(&mut self.by_predicate, predicate, id);
        }
        if let Some(object) = &pattern.object {
            remove_subscription_index_entry(&mut self.by_object, object, id);
        }
        if let Some(graph_name) = &pattern.graph_name {
            remove_subscription_index_entry(
                &mut self.by_graph_name,
                &graph_name.clone().unwrap_or(EncodedTerm::DefaultGraph),
                id,
            );
        }
        if pattern.subject.is_none()
            && pattern.predicate.is_none()
            && pattern.object.is_none()
            && pattern.graph_name.is_none()
        {
            self.unconstrained.remove(&id);
        }
    }

    fn for_each_candidate(&self, quad: &EncodedQuad, mut callback: impl FnMut(usize)) {
        // Unconstrained subscriptions are never present in the component indexes.
        for &id in &self.unconstrained {
            callback(id);
        }

        let matching_indexes = [
            self.by_subject.get(&quad.subject),
            self.by_predicate.get(&quad.predicate),
            self.by_object.get(&quad.object),
            self.by_graph_name.get(&quad.graph_name),
        ];
        if matching_indexes.iter().filter(|ids| ids.is_some()).count() < 2 {
            for ids in matching_indexes.into_iter().flatten() {
                for &id in ids {
                    callback(id);
                }
            }
            return;
        }

        let mut visited = HashSet::with_capacity(
            matching_indexes
                .iter()
                .flatten()
                .map(|ids| ids.len())
                .max()
                .unwrap_or(0),
        );
        for ids in matching_indexes.into_iter().flatten() {
            for &id in ids {
                if visited.insert(id) {
                    callback(id);
                }
            }
        }
    }
}

fn remove_subscription_index_entry(
    index: &mut HashMap<EncodedTerm, HashSet<usize>>,
    key: &EncodedTerm,
    id: usize,
) {
    if let Some(ids) = index.get_mut(key) {
        ids.remove(&id);
        if ids.is_empty() {
            index.remove(key);
        }
    }
}

impl Storage {
    #[expect(clippy::unnecessary_wraps)]
    pub fn new() -> Result<Self, StorageError> {
        Ok(Self {
            kind: StorageKind::Memory(MemoryStorage::new()),
            incremental_state: Arc::new(Mutex::new(IncrementalState::default())),
            incremental_commit_lock: Arc::new(Mutex::new(())),
        })
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        Ok(Self {
            kind: StorageKind::RocksDb(RocksDbStorage::open(path)?),
            incremental_state: Arc::new(Mutex::new(IncrementalState::default())),
            incremental_commit_lock: Arc::new(Mutex::new(())),
        })
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn open_with_options(path: &Path, options: StorageOptions) -> Result<Self, StorageError> {
        Ok(Self {
            kind: StorageKind::RocksDb(RocksDbStorage::open_with_options(
                path,
                RocksDbStorageOptions {
                    max_open_files: options.max_open_files,
                    fd_reserve: options.fd_reserve,
                },
            )?),
            incremental_state: Arc::new(Mutex::new(IncrementalState::default())),
            incremental_commit_lock: Arc::new(Mutex::new(())),
        })
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn open_read_only(path: &Path) -> Result<Self, StorageError> {
        Ok(Self {
            kind: StorageKind::RocksDb(RocksDbStorage::open_read_only(path)?),
            incremental_state: Arc::new(Mutex::new(IncrementalState::default())),
            incremental_commit_lock: Arc::new(Mutex::new(())),
        })
    }

    pub fn snapshot(&self) -> StorageReader<'static> {
        StorageReader {
            kind: match &self.kind {
                #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
                StorageKind::RocksDb(storage) => StorageReaderKind::RocksDb(storage.snapshot()),
                StorageKind::Memory(storage) => StorageReaderKind::Memory(storage.snapshot()),
            },
            incremental_state: Arc::clone(&self.incremental_state),
            incremental_commit_lock: Arc::clone(&self.incremental_commit_lock),
            can_refresh: true,
        }
    }

    #[cfg_attr(
        not(all(not(target_family = "wasm"), feature = "rocksdb")),
        expect(clippy::unnecessary_wraps)
    )]
    pub fn start_transaction(&self) -> Result<StorageTransaction<'_>, StorageError> {
        Ok(StorageTransaction {
            kind: match &self.kind {
                #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
                StorageKind::RocksDb(storage) => {
                    StorageTransactionKind::RocksDb(storage.start_transaction()?)
                }
                StorageKind::Memory(storage) => {
                    StorageTransactionKind::Memory(storage.start_transaction())
                }
            },
            incremental: IncrementalTransactionState::new(self),
        })
    }

    #[cfg_attr(
        not(all(not(target_family = "wasm"), feature = "rocksdb")),
        expect(clippy::unnecessary_wraps)
    )]
    pub fn start_readable_transaction(
        &self,
    ) -> Result<StorageReadableTransaction<'_>, StorageError> {
        Ok(StorageReadableTransaction {
            kind: match &self.kind {
                #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
                StorageKind::RocksDb(storage) => {
                    StorageReadableTransactionKind::RocksDb(storage.start_readable_transaction()?)
                }
                StorageKind::Memory(storage) => {
                    StorageReadableTransactionKind::Memory(storage.start_transaction())
                }
            },
            incremental: IncrementalTransactionState::new(self),
        })
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn flush(&self) -> Result<(), StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageKind::RocksDb(storage) => storage.flush(),
            StorageKind::Memory(_) => Ok(()),
        }
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn compact(&self) -> Result<(), StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageKind::RocksDb(storage) => storage.compact(),
            StorageKind::Memory(_) => Ok(()),
        }
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn backup(&self, target_directory: &Path) -> Result<(), StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageKind::RocksDb(storage) => storage.backup(target_directory),
            StorageKind::Memory(_) => Err(StorageError::Other(
                "It is not possible to backup an in-memory database".into(),
            )),
        }
    }

    pub fn bulk_loader(&self) -> StorageBulkLoader<'_> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageKind::RocksDb(storage) => StorageBulkLoader {
                kind: StorageBulkLoaderKind::RocksDb(storage.bulk_loader()),
                incremental: IncrementalTransactionState::new(self),
                non_atomic: false,
            },
            StorageKind::Memory(storage) => StorageBulkLoader {
                kind: StorageBulkLoaderKind::Memory(storage.bulk_loader()),
                incremental: IncrementalTransactionState::new(self),
                non_atomic: false,
            },
        }
    }

    fn queue_committed_quad_changes(
        &self,
        changes: Vec<Delta<EncodedQuad>>,
    ) -> HashMap<*const IncrementalQueryNotifier, Weak<IncrementalQueryNotifier>> {
        if changes.is_empty() {
            return HashMap::new();
        }
        let mut state = self.incremental_state.lock().unwrap();
        let mut notifiers = HashMap::new();

        for delta in changes {
            state.for_each_matching_subscriber(delta.value(), |subscriber| {
                subscriber.changes.push_back(delta.clone());

                notifiers
                    .entry(subscriber.notifier.as_ptr())
                    .or_insert_with(|| Weak::clone(&subscriber.notifier));
            });
        }
        notifiers
    }
}

fn notify_incremental_changes(
    notifiers: HashMap<*const IncrementalQueryNotifier, Weak<IncrementalQueryNotifier>>,
) {
    for notifier in notifiers.into_values() {
        if let Some(notifier) = notifier.upgrade() {
            notifier.notify();
        }
    }
}

#[derive(Clone)]
#[must_use]
pub struct StorageReader<'a> {
    kind: StorageReaderKind<'a>,
    incremental_state: Arc<Mutex<IncrementalState>>,
    incremental_commit_lock: Arc<Mutex<()>>,
    can_refresh: bool,
}

#[derive(Clone)]
enum StorageReaderKind<'a> {
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    RocksDb(RocksDbStorageReader<'a>),
    Memory(MemoryStorageReader<'a>),
}

#[cfg_attr(
    not(all(not(target_family = "wasm"), feature = "rocksdb")),
    expect(clippy::unnecessary_wraps)
)]
impl<'a> StorageReader<'a> {
    pub(crate) fn can_refresh(&self) -> bool {
        self.can_refresh
    }

    pub fn len(&self) -> Result<usize, StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.len(),
            StorageReaderKind::Memory(reader) => Ok(reader.len()),
        }
    }

    pub fn is_empty(&self) -> Result<bool, StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.is_empty(),
            StorageReaderKind::Memory(reader) => Ok(reader.is_empty()),
        }
    }

    pub fn contains(&self, quad: &EncodedQuad) -> Result<bool, StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.contains(quad),
            StorageReaderKind::Memory(reader) => Ok(reader.contains(quad)),
        }
    }

    pub fn quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> DecodingQuadIterator<'a> {
        DecodingQuadIterator {
            kind: match &self.kind {
                #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
                StorageReaderKind::RocksDb(reader) => DecodingQuadIteratorKind::RocksDb(
                    reader.quads_for_pattern(subject, predicate, object, graph_name),
                ),
                StorageReaderKind::Memory(reader) => DecodingQuadIteratorKind::Memory(
                    reader.quads_for_pattern(subject, predicate, object, graph_name),
                ),
            },
        }
    }

    pub fn triples_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_names: Option<&[Option<EncodedTerm>]>,
    ) -> Box<dyn Iterator<Item = Result<EncodedTriple, StorageError>> + 'a> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => {
                reader.triples_for_pattern(subject, predicate, object, graph_names)
            }
            StorageReaderKind::Memory(_) => {
                if let Some(graph_names) = graph_names {
                    let iters = graph_names
                        .iter()
                        .map(|graph_name| {
                            self.quads_for_pattern(
                                subject,
                                predicate,
                                object,
                                Some(graph_name.as_ref().unwrap_or(&EncodedTerm::DefaultGraph)),
                            )
                        })
                        .collect::<Vec<_>>();
                    Box::new(hash_deduplicate(
                        iters.into_iter().flatten().map(|quad| Ok(quad?.into())),
                    ))
                } else {
                    Box::new(hash_deduplicate(
                        self.quads_for_pattern(subject, predicate, object, None)
                            .map(|quad| Ok(quad?.into())),
                    ))
                }
            }
        }
    }

    pub fn named_graphs(&self) -> DecodingGraphIterator<'a> {
        DecodingGraphIterator {
            kind: match &self.kind {
                #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
                StorageReaderKind::RocksDb(reader) => {
                    DecodingGraphIteratorKind::RocksDb(reader.named_graphs())
                }
                StorageReaderKind::Memory(reader) => {
                    DecodingGraphIteratorKind::Memory(reader.named_graphs())
                }
            },
        }
    }

    pub fn contains_named_graph(&self, graph_name: &EncodedTerm) -> Result<bool, StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.contains_named_graph(graph_name),
            StorageReaderKind::Memory(reader) => Ok(reader.contains_named_graph(graph_name)),
        }
    }

    pub fn contains_str(&self, key: &StrHash) -> Result<bool, StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.contains_str(key),
            StorageReaderKind::Memory(reader) => Ok(reader.contains_str(key)),
        }
    }

    /// Validate that all the storage invariants held in the data
    pub fn validate(&self) -> Result<(), StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.validate(),
            StorageReaderKind::Memory(reader) => reader.validate(),
        }
    }

    pub fn subscribe_incremental_quads(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<Option<&EncodedTerm>>,
        notifier: Weak<IncrementalQueryNotifier>,
    ) -> (usize, Option<StorageReader<'static>>) {
        // Register before taking the fresh snapshot while commits are excluded. A commit is
        // therefore represented either in the snapshot or in the subscriber queue, never lost
        // in between the two operations.
        let _commit_guard = self.incremental_commit_lock.lock().unwrap();
        let mut state = self.incremental_state.lock().unwrap();
        let slot = state
            .free_subscriber_slots
            .pop()
            .unwrap_or_else(|| state.subscribers.len());
        let pattern = IncrementalQuadPattern {
            subject: subject.cloned(),
            predicate: predicate.cloned(),
            object: object.cloned(),
            graph_name: graph_name.map(<Option<&EncodedTerm>>::cloned),
        };
        state.subscription_index.insert(slot, &pattern);
        let subscriber = Some(IncrementalSubscriber {
            pattern,
            changes: VecDeque::new(),
            notifier,
        });
        if slot == state.subscribers.len() {
            state.subscribers.push(subscriber);
        } else {
            state.subscribers[slot] = subscriber;
        }
        drop(state);
        let snapshot = self.can_refresh.then(|| StorageReader {
            kind: match &self.kind {
                #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
                StorageReaderKind::RocksDb(reader) => {
                    StorageReaderKind::RocksDb(reader.latest_snapshot())
                }
                StorageReaderKind::Memory(reader) => {
                    StorageReaderKind::Memory(reader.latest_snapshot())
                }
            },
            incremental_state: Arc::clone(&self.incremental_state),
            incremental_commit_lock: Arc::clone(&self.incremental_commit_lock),
            can_refresh: true,
        });
        (slot, snapshot)
    }

    pub fn incremental_quad_subscription_changes(&self, id: usize) -> VecDeque<Delta<EncodedQuad>> {
        let mut state = self.incremental_state.lock().unwrap();
        state
            .subscribers
            .get_mut(id)
            .and_then(Option::as_mut)
            .map_or_else(VecDeque::new, |subscriber| take(&mut subscriber.changes))
    }

    pub fn unsubscribe_incremental_quads(&self, id: usize) {
        let mut state = self.incremental_state.lock().unwrap();
        if let Some(subscriber) = state.subscribers.get_mut(id).and_then(Option::take) {
            state.subscription_index.remove(id, &subscriber.pattern);
            state.free_subscriber_slots.push(id);
        }
    }
}

fn hash_deduplicate<T: Eq + std::hash::Hash + Clone, E>(
    iter: impl Iterator<Item = Result<T, E>>,
) -> impl Iterator<Item = Result<T, E>> {
    let mut already_seen = FxHashSet::with_capacity_and_hasher(iter.size_hint().0, FxBuildHasher);
    iter.filter(move |result| match result {
        Ok(value) => already_seen.insert(value.clone()),
        Err(_) => true,
    })
}

#[must_use]
pub struct DecodingQuadIterator<'a> {
    kind: DecodingQuadIteratorKind<'a>,
}

enum DecodingQuadIteratorKind<'a> {
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    RocksDb(RocksDbChainedDecodingQuadIterator<'a>),
    Memory(QuadIterator<'a>),
}

impl Iterator for DecodingQuadIterator<'_> {
    type Item = Result<EncodedQuad, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            DecodingQuadIteratorKind::RocksDb(iter) => iter.next(),
            DecodingQuadIteratorKind::Memory(iter) => iter.next().map(Ok),
        }
    }
}

#[must_use]
pub struct DecodingGraphIterator<'a> {
    kind: DecodingGraphIteratorKind<'a>,
}

enum DecodingGraphIteratorKind<'a> {
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    RocksDb(RocksDbDecodingGraphIterator<'a>),
    Memory(MemoryDecodingGraphIterator<'a>),
}

impl Iterator for DecodingGraphIterator<'_> {
    type Item = Result<EncodedTerm, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            DecodingGraphIteratorKind::RocksDb(iter) => iter.next(),
            DecodingGraphIteratorKind::Memory(iter) => iter.next().map(Ok),
        }
    }
}

impl StrLookup for StorageReader<'_> {
    fn get_str(&self, key: &StrHash) -> Result<Option<OxString>, StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => {
                let value = reader.get_str(key)?;
                if value.is_some() || !self.can_refresh {
                    Ok(value)
                } else {
                    reader.get_str_from_latest(key)
                }
            }
            StorageReaderKind::Memory(reader) => reader.get_str(key),
        }
    }
}

struct IncrementalTransactionState {
    storage: Storage,
    initial_snapshot: Option<StorageReader<'static>>,
    quad_states: HashMap<EncodedQuad, PendingQuadState>,
    trackable: bool,
    incremental_state: Arc<Mutex<IncrementalState>>,
    range_clears: Vec<RangeClear>,
    next_operation: u64,
}

#[derive(Clone, Copy)]
enum DesiredQuadState {
    Present,
    Absent,
}

struct PendingQuadState {
    desired: DesiredQuadState,
    operation: u64,
}

#[derive(Clone)]
enum RangeClear {
    Pattern(IncrementalQuadPattern, u64),
    NamedGraphs(u64),
}

impl IncrementalTransactionState {
    fn new(storage: &Storage) -> Self {
        let trackable = match &storage.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageKind::RocksDb(_) => true,
            StorageKind::Memory(_) => false,
        };
        Self {
            storage: storage.clone(),
            initial_snapshot: trackable.then(|| storage.snapshot()),
            quad_states: HashMap::new(),
            trackable,
            incremental_state: Arc::clone(&storage.incremental_state),
            range_clears: Vec::new(),
            next_operation: 0,
        }
    }

    fn next_operation(&mut self) -> u64 {
        let operation = self.next_operation;
        self.next_operation += 1;
        operation
    }

    fn set_quad_state(&mut self, quad: &Quad, state: DesiredQuadState) {
        if self.trackable {
            let operation = self.next_operation();
            // Only the final intent matters. Opposing operations in the same
            // transaction are reconciled against the initial snapshot below.
            self.quad_states
                .entry(quad.into())
                .and_modify(|pending| {
                    pending.desired = state;
                    pending.operation = operation;
                })
                .or_insert(PendingQuadState {
                    desired: state,
                    operation,
                });
        }
    }

    #[cfg_attr(
        not(all(not(target_family = "wasm"), feature = "rocksdb")),
        expect(dead_code)
    )]
    fn mark_range_clear(&mut self, pattern: IncrementalQuadPattern) {
        if self.trackable {
            let operation = self.next_operation();
            self.range_clears
                .push(RangeClear::Pattern(pattern, operation));
        }
    }

    #[cfg_attr(
        not(all(not(target_family = "wasm"), feature = "rocksdb")),
        expect(dead_code)
    )]
    fn mark_range_clear_named_graphs(&mut self) {
        if self.trackable {
            let operation = self.next_operation();
            self.range_clears.push(RangeClear::NamedGraphs(operation));
        }
    }

    fn touch_quads_for_pattern(
        &mut self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> Result<(), StorageError> {
        if self.trackable {
            let Some(initial_snapshot) = &self.initial_snapshot else {
                return Ok(());
            };
            // Also clear quads inserted earlier in this transaction: they are not
            // visible through the initial snapshot.
            for (quad, pending) in &mut self.quad_states {
                if subject.is_none_or(|subject| quad.subject == *subject)
                    && predicate.is_none_or(|predicate| quad.predicate == *predicate)
                    && object.is_none_or(|object| quad.object == *object)
                    && graph_name.is_none_or(|graph_name| quad.graph_name == *graph_name)
                {
                    pending.desired = DesiredQuadState::Absent;
                }
            }
            for quad in initial_snapshot.quads_for_pattern(subject, predicate, object, graph_name) {
                self.quad_states
                    .entry(quad?)
                    .and_modify(|pending| {
                        pending.desired = DesiredQuadState::Absent;
                    })
                    .or_insert(PendingQuadState {
                        desired: DesiredQuadState::Absent,
                        operation: self.next_operation,
                    });
            }
        }
        Ok(())
    }

    fn touch_all_named_graph_quads(&mut self) -> Result<(), StorageError> {
        if self.trackable {
            let Some(initial_snapshot) = &self.initial_snapshot else {
                return Ok(());
            };
            for (quad, pending) in &mut self.quad_states {
                if !quad.graph_name.is_default_graph() {
                    pending.desired = DesiredQuadState::Absent;
                }
            }
            for quad in initial_snapshot.quads_for_pattern(None, None, None, None) {
                let quad = quad?;
                if !quad.graph_name.is_default_graph() {
                    self.quad_states
                        .entry(quad)
                        .and_modify(|pending| {
                            pending.desired = DesiredQuadState::Absent;
                        })
                        .or_insert(PendingQuadState {
                            desired: DesiredQuadState::Absent,
                            operation: self.next_operation,
                        });
                }
            }
        }
        Ok(())
    }

    fn disable(&mut self) {
        self.trackable = false;
        self.quad_states.clear();
        self.range_clears.clear();
    }

    #[cfg_attr(
        not(all(not(target_family = "wasm"), feature = "rocksdb")),
        expect(dead_code)
    )]
    fn take_visible_additions(
        &mut self,
        before: &StorageReader<'_>,
        after: &StorageReader<'_>,
    ) -> Result<Vec<Delta<EncodedQuad>>, StorageError> {
        if !self.trackable {
            return Ok(Vec::new());
        }
        let mut changes = Vec::new();
        let mut visible = Vec::new();
        for (quad, pending) in &self.quad_states {
            let is_visible = after.contains(quad)?;
            if is_visible
                && matches!(pending.desired, DesiredQuadState::Present)
                && !before.contains(quad)?
            {
                changes.push(Delta::addition(quad.clone()));
            }
            if is_visible {
                visible.push(quad.clone());
            }
        }
        for quad in visible {
            self.quad_states.remove(&quad);
        }
        Ok(changes)
    }

    fn prepare_changes(mut self) -> Result<(Storage, Vec<Delta<EncodedQuad>>), StorageError> {
        if !self.trackable || (self.quad_states.is_empty() && self.range_clears.is_empty()) {
            return Ok((self.storage, Vec::new()));
        }
        let mut changes = Vec::with_capacity(self.quad_states.len());
        // The caller holds the commit-order lock. Compare with the state immediately before this
        // commit so overlapping RocksDB transactions cannot publish duplicate or stale deltas.
        let before_commit = self.storage.snapshot();
        for clear in self.range_clears.clone() {
            let (pattern, operation) = match clear {
                RangeClear::Pattern(pattern, operation) => (Some(pattern), operation),
                RangeClear::NamedGraphs(operation) => (None, operation),
            };
            let named_graphs_only = pattern.is_none();
            let (subject, predicate, object, graph_name) = if let Some(pattern) = &pattern {
                (
                    pattern.subject.as_ref(),
                    pattern.predicate.as_ref(),
                    pattern.object.as_ref(),
                    pattern.graph_name.as_ref().map(|graph_name| {
                        graph_name.as_ref().unwrap_or(&EncodedTerm::DefaultGraph)
                    }),
                )
            } else {
                (None, None, None, None)
            };
            for quad in before_commit.quads_for_pattern(subject, predicate, object, graph_name) {
                let quad = quad?;
                if named_graphs_only && quad.graph_name.is_default_graph() {
                    continue;
                }
                self.quad_states
                    .entry(quad)
                    .and_modify(|pending| {
                        if pending.operation <= operation {
                            pending.desired = DesiredQuadState::Absent;
                            pending.operation = operation;
                        }
                    })
                    .or_insert(PendingQuadState {
                        desired: DesiredQuadState::Absent,
                        operation,
                    });
            }
        }
        for (quad, pending) in self.quad_states {
            let had_before = before_commit.contains(&quad)?;
            match (had_before, pending.desired) {
                (false, DesiredQuadState::Present) => changes.push(Delta::addition(quad)),
                (true, DesiredQuadState::Absent) => changes.push(Delta::deletion(quad)),
                _ => {}
            }
        }
        Ok((self.storage, changes))
    }

    fn commit_and_publish(
        self,
        exact_changes: Option<Vec<(EncodedQuad, bool)>>,
        commit: impl FnOnce() -> Result<(), StorageError>,
    ) -> Result<(), StorageError> {
        let commit_lock = Arc::clone(&self.storage.incremental_commit_lock);
        let notifiers = {
            // Keep backend commit order and subscriber queue order identical.
            let _commit_guard = commit_lock
                .lock()
                .map_err(|_| StorageError::Other("incremental commit lock poisoned".into()))?;
            let (storage, changes) = if let Some(exact_changes) = exact_changes {
                let changes = exact_changes
                    .into_iter()
                    .map(|(quad, is_present)| {
                        if is_present {
                            Delta::addition(quad)
                        } else {
                            Delta::deletion(quad)
                        }
                    })
                    .collect();
                (self.storage.clone(), changes)
            } else {
                self.prepare_changes()?
            };
            commit()?;
            storage.queue_committed_quad_changes(changes)
        };
        // Waking may run arbitrary executor code, so it must happen outside of
        // both the commit-order lock and the incremental-state lock.
        notify_incremental_changes(notifiers);
        Ok(())
    }
}

#[must_use]
pub struct StorageTransaction<'a> {
    kind: StorageTransactionKind<'a>,
    incremental: IncrementalTransactionState,
}

enum StorageTransactionKind<'a> {
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    RocksDb(RocksDbStorageTransaction<'a>),
    Memory(MemoryStorageTransaction<'a>),
}

impl StorageTransaction<'_> {
    pub fn insert(&mut self, quad: Quad) {
        self.incremental
            .set_quad_state(&quad, DesiredQuadState::Present);
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.insert(quad),
            StorageTransactionKind::Memory(transaction) => {
                transaction.insert(quad);
            }
        }
    }

    pub fn insert_named_graph(&mut self, graph_name: NamedOrBlankNode) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => {
                transaction.insert_named_graph(graph_name)
            }
            StorageTransactionKind::Memory(transaction) => {
                transaction.insert_named_graph(graph_name);
            }
        }
    }

    pub fn remove(&mut self, quad: &Quad) {
        self.incremental
            .set_quad_state(quad, DesiredQuadState::Absent);
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.remove(quad),
            StorageTransactionKind::Memory(transaction) => transaction.remove(quad),
        }
    }

    pub fn clear_default_graph(&mut self) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => {
                self.incremental.mark_range_clear(IncrementalQuadPattern {
                    subject: None,
                    predicate: None,
                    object: None,
                    graph_name: Some(Some(EncodedTerm::DefaultGraph)),
                });
                transaction.clear_default_graph();
            }
            StorageTransactionKind::Memory(transaction) => {
                if self
                    .incremental
                    .touch_quads_for_pattern(None, None, None, Some(&EncodedTerm::DefaultGraph))
                    .is_err()
                {
                    self.incremental.disable();
                }
                transaction.clear_graph(&GraphName::DefaultGraph)
            }
        }
    }

    pub fn clear_all_named_graphs(&mut self) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => {
                self.incremental.mark_range_clear_named_graphs();
                transaction.clear_all_named_graphs();
            }
            StorageTransactionKind::Memory(transaction) => {
                if self.incremental.touch_all_named_graph_quads().is_err() {
                    self.incremental.disable();
                }
                transaction.clear_all_named_graphs();
            }
        }
    }

    pub fn clear_all_graphs(&mut self) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => {
                self.incremental.mark_range_clear(IncrementalQuadPattern {
                    subject: None,
                    predicate: None,
                    object: None,
                    graph_name: None,
                });
                transaction.clear_all_graphs();
            }
            StorageTransactionKind::Memory(transaction) => {
                if self
                    .incremental
                    .touch_quads_for_pattern(None, None, None, None)
                    .is_err()
                {
                    self.incremental.disable();
                }
                transaction.clear_all_graphs();
            }
        }
    }

    pub fn remove_all_named_graphs(&mut self) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => {
                self.incremental.mark_range_clear_named_graphs();
                transaction.remove_all_named_graphs();
            }
            StorageTransactionKind::Memory(transaction) => {
                if self.incremental.touch_all_named_graph_quads().is_err() {
                    self.incremental.disable();
                }
                transaction.remove_all_named_graphs();
            }
        }
    }

    pub fn clear(&mut self) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => {
                self.incremental.mark_range_clear(IncrementalQuadPattern {
                    subject: None,
                    predicate: None,
                    object: None,
                    graph_name: None,
                });
                transaction.clear();
            }
            StorageTransactionKind::Memory(transaction) => {
                if self
                    .incremental
                    .touch_quads_for_pattern(None, None, None, None)
                    .is_err()
                {
                    self.incremental.disable();
                }
                transaction.clear();
            }
        }
    }

    pub fn commit(self) -> Result<(), StorageError> {
        let Self { kind, incremental } = self;
        match kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => {
                incremental.commit_and_publish(None, || transaction.commit())
            }
            StorageTransactionKind::Memory(transaction) => {
                let changes = transaction.pending_quad_changes();
                incremental.commit_and_publish(Some(changes), || {
                    transaction.commit();
                    Ok(())
                })
            }
        }
    }
}

#[must_use]
pub struct StorageReadableTransaction<'a> {
    kind: StorageReadableTransactionKind<'a>,
    incremental: IncrementalTransactionState,
}

enum StorageReadableTransactionKind<'a> {
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    RocksDb(RocksDbStorageReadableTransaction<'a>),
    Memory(MemoryStorageTransaction<'a>),
}

impl StorageReadableTransaction<'_> {
    pub fn reader(&self) -> StorageReader<'_> {
        StorageReader {
            kind: match &self.kind {
                #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
                StorageReadableTransactionKind::RocksDb(transaction) => {
                    StorageReaderKind::RocksDb(transaction.reader())
                }
                StorageReadableTransactionKind::Memory(transaction) => {
                    StorageReaderKind::Memory(transaction.reader())
                }
            },
            incremental_state: Arc::clone(&self.incremental.incremental_state),
            incremental_commit_lock: Arc::clone(&self.incremental.storage.incremental_commit_lock),
            can_refresh: false,
        }
    }

    pub fn insert(&mut self, quad: Quad) {
        self.incremental
            .set_quad_state(&quad, DesiredQuadState::Present);
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => transaction.insert(quad),
            StorageReadableTransactionKind::Memory(transaction) => {
                transaction.insert(quad);
            }
        }
    }

    pub fn insert_named_graph(&mut self, graph_name: NamedOrBlankNode) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => {
                transaction.insert_named_graph(graph_name)
            }
            StorageReadableTransactionKind::Memory(transaction) => {
                transaction.insert_named_graph(graph_name);
            }
        }
    }

    pub fn remove(&mut self, quad: &Quad) {
        self.incremental
            .set_quad_state(quad, DesiredQuadState::Absent);
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => transaction.remove(quad),
            StorageReadableTransactionKind::Memory(transaction) => transaction.remove(quad),
        }
    }

    pub fn clear_graph(&mut self, graph_name: &GraphName) -> Result<(), StorageError> {
        let encoded_graph_name = EncodedTerm::from(graph_name);
        self.incremental
            .touch_quads_for_pattern(None, None, None, Some(&encoded_graph_name))?;
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => {
                transaction.clear_graph(graph_name)
            }
            StorageReadableTransactionKind::Memory(transaction) => {
                transaction.clear_graph(graph_name);
                Ok(())
            }
        }
    }

    pub fn clear_all_named_graphs(&mut self) -> Result<(), StorageError> {
        self.incremental.touch_all_named_graph_quads()?;
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => {
                transaction.clear_all_named_graphs()
            }
            StorageReadableTransactionKind::Memory(transaction) => {
                transaction.clear_all_named_graphs();
                Ok(())
            }
        }
    }

    pub fn clear_all_graphs(&mut self) -> Result<(), StorageError> {
        self.incremental
            .touch_quads_for_pattern(None, None, None, None)?;
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => transaction.clear_all_graphs(),
            StorageReadableTransactionKind::Memory(transaction) => {
                transaction.clear_all_graphs();
                Ok(())
            }
        }
    }

    pub fn remove_named_graph(
        &mut self,
        graph_name: &NamedOrBlankNode,
    ) -> Result<(), StorageError> {
        let encoded_graph_name = EncodedTerm::from(graph_name);
        self.incremental
            .touch_quads_for_pattern(None, None, None, Some(&encoded_graph_name))?;
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => {
                transaction.remove_named_graph(graph_name)
            }
            StorageReadableTransactionKind::Memory(transaction) => {
                transaction.remove_named_graph(graph_name);
                Ok(())
            }
        }
    }

    pub fn remove_all_named_graphs(&mut self) -> Result<(), StorageError> {
        self.incremental.touch_all_named_graph_quads()?;
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => {
                transaction.remove_all_named_graphs()
            }
            StorageReadableTransactionKind::Memory(transaction) => {
                transaction.remove_all_named_graphs();
                Ok(())
            }
        }
    }

    pub fn clear(&mut self) -> Result<(), StorageError> {
        self.incremental
            .touch_quads_for_pattern(None, None, None, None)?;
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => transaction.clear(),
            StorageReadableTransactionKind::Memory(transaction) => {
                transaction.clear();
                Ok(())
            }
        }
    }

    pub fn commit(self) -> Result<(), StorageError> {
        let Self { kind, incremental } = self;
        match kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => {
                incremental.commit_and_publish(None, || transaction.commit())
            }
            StorageReadableTransactionKind::Memory(transaction) => {
                let changes = transaction.pending_quad_changes();
                incremental.commit_and_publish(Some(changes), || {
                    transaction.commit();
                    Ok(())
                })
            }
        }
    }
}

#[must_use]
pub struct StorageBulkLoader<'a> {
    kind: StorageBulkLoaderKind<'a>,
    incremental: IncrementalTransactionState,
    non_atomic: bool,
}

enum StorageBulkLoaderKind<'a> {
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    RocksDb(RocksDbStorageBulkLoader<'a>),
    Memory(MemoryStorageBulkLoader<'a>),
}

impl StorageBulkLoader<'_> {
    pub fn on_progress(self, callback: impl Fn(u64) + Send + Sync + 'static) -> Self {
        match self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageBulkLoaderKind::RocksDb(loader) => Self {
                kind: StorageBulkLoaderKind::RocksDb(loader.on_progress(callback)),
                incremental: self.incremental,
                non_atomic: self.non_atomic,
            },
            StorageBulkLoaderKind::Memory(loader) => Self {
                kind: StorageBulkLoaderKind::Memory(loader.on_progress(callback)),
                incremental: self.incremental,
                non_atomic: self.non_atomic,
            },
        }
    }

    pub fn without_atomicity(self) -> Self {
        match self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageBulkLoaderKind::RocksDb(loader) => Self {
                kind: StorageBulkLoaderKind::RocksDb(loader.without_atomicity()),
                incremental: self.incremental,
                non_atomic: true,
            },
            StorageBulkLoaderKind::Memory(loader) => Self {
                kind: StorageBulkLoaderKind::Memory(loader),
                incremental: self.incremental,
                non_atomic: self.non_atomic,
            },
        }
    }

    #[cfg_attr(
        any(target_family = "wasm", not(feature = "rocksdb")),
        expect(clippy::unnecessary_wraps, unused_variables)
    )]
    pub fn load_batch(
        &mut self,
        quads: Vec<Quad>,
        max_num_threads: usize,
    ) -> Result<(), StorageError> {
        for quad in &quads {
            self.incremental
                .set_quad_state(quad, DesiredQuadState::Present);
        }
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageBulkLoaderKind::RocksDb(loader) => {
                if !self.non_atomic {
                    let result = loader.load_batch(quads, max_num_threads);
                    loader.run_progress_callbacks();
                    return result;
                }
                let storage = self.incremental.storage.clone();
                let result: Result<_, StorageError> = (|| {
                    // A non-atomic loader can ingest a previous batch while accepting this one.
                    // Serialize that ingestion with regular commits, then publish precisely the
                    // tracked additions that became visible during it.
                    let _commit_guard = storage.incremental_commit_lock.lock().map_err(|_| {
                        StorageError::Other("incremental commit lock poisoned".into())
                    })?;
                    let before = storage.snapshot();
                    loader.load_batch(quads, max_num_threads)?;
                    let after = storage.snapshot();
                    let changes = self.incremental.take_visible_additions(&before, &after)?;
                    Ok(storage.queue_committed_quad_changes(changes))
                })();
                loader.run_progress_callbacks();
                let notifiers = result?;
                notify_incremental_changes(notifiers);
                Ok(())
            }
            StorageBulkLoaderKind::Memory(loader) => {
                loader.load_batch(quads);
                Ok(())
            }
        }
    }

    pub fn commit(self) -> Result<(), StorageError> {
        let Self {
            kind, incremental, ..
        } = self;
        match kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageBulkLoaderKind::RocksDb(mut loader) => {
                let result = incremental.commit_and_publish(None, || loader.commit());
                loader.run_progress_callbacks();
                result
            }
            StorageBulkLoaderKind::Memory(loader) => {
                let changes = loader.pending_quad_changes();
                incremental.commit_and_publish(Some(changes), || {
                    loader.commit();
                    Ok(())
                })
            }
        }
    }
}

#[cfg(not(target_family = "wasm"))]
pub fn map_thread_result<R>(result: thread::Result<R>) -> io::Result<R> {
    result.map_err(|e| {
        io::Error::other(if let Ok(e) = e.downcast::<&dyn std::fmt::Display>() {
            format!("A loader processed crashed with {e}")
        } else {
            "A loader processed crashed with and unknown error".into()
        })
    })
}
