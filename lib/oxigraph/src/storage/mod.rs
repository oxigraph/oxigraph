use crate::model::{GraphNameRef, NamedOrBlankNodeRef, QuadRef};
pub use crate::storage::error::{CorruptionError, LoaderError, SerializerError, StorageError};
use crate::storage::memory::{
    MemoryDecodingGraphIterator, MemoryStorage, MemoryStorageBulkLoader, MemoryStorageReader,
    MemoryStorageTransaction, QuadIterator,
};
use crate::storage::numeric_encoder::{EncodedQuad, EncodedTerm, StrHash, StrLookup};
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use crate::storage::rocksdb::{
    RocksDbChainedDecodingQuadIterator, RocksDbDecodingGraphIterator, RocksDbStorage,
    RocksDbStorageBulkLoader, RocksDbStorageReadableTransaction, RocksDbStorageReader,
    RocksDbStorageTransaction,
};
use oxrdf::Quad;
use std::path::Path;
#[cfg(not(target_family = "wasm"))]
use std::{io, thread};
use updatable_dataset::{
    BulkLoader, ReadWriteTransaction, Reader, UpdatableDataset, WriteOnlyTransaction,
};

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
pub(crate) mod updatable_dataset;

pub const DEFAULT_BULK_LOAD_BATCH_SIZE: usize = 1_000_000;

/// Low level storage primitives
#[derive(Clone)]
pub struct Storage {
    kind: StorageKind,
}

#[derive(Clone)]
enum StorageKind {
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    RocksDb(RocksDbStorage),
    Memory(MemoryStorage),
}

impl Storage {
    #[expect(clippy::unnecessary_wraps)]
    pub fn new() -> Result<Self, StorageError> {
        Ok(Self {
            kind: StorageKind::Memory(MemoryStorage::new()),
        })
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        Ok(Self {
            kind: StorageKind::RocksDb(RocksDbStorage::open(path)?),
        })
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn open_read_only(path: &Path) -> Result<Self, StorageError> {
        Ok(Self {
            kind: StorageKind::RocksDb(RocksDbStorage::open_read_only(path)?),
        })
    }
}

impl UpdatableDataset<'static> for Storage {
    type Error = StorageError;
    type Reader<'reader> = StorageReader<'reader>;
    type WriteOnlyTransaction<'transaction> = StorageTransaction<'transaction>;
    type ReadWriteTransaction<'transaction> = StorageReadableTransaction<'transaction>;
    type BulkLoader<'loader> = StorageBulkLoader<'loader>;

    fn snapshot(&self) -> StorageReader<'static> {
        StorageReader {
            kind: match &self.kind {
                #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
                StorageKind::RocksDb(storage) => StorageReaderKind::RocksDb(storage.snapshot()),
                StorageKind::Memory(storage) => StorageReaderKind::Memory(storage.snapshot()),
            },
        }
    }

    fn start_transaction(&self) -> Result<StorageTransaction<'_>, StorageError> {
        Ok(StorageTransaction {
            kind: match &self.kind {
                #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
                StorageKind::RocksDb(storage) => {
                    StorageTransactionKind::RocksDb(storage.start_transaction()?)
                }
                StorageKind::Memory(storage) => {
                    StorageTransactionKind::Memory(storage.start_transaction()?)
                }
            },
        })
    }

    fn start_readable_transaction(&self) -> Result<StorageReadableTransaction<'_>, StorageError> {
        Ok(StorageReadableTransaction {
            kind: match &self.kind {
                #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
                StorageKind::RocksDb(storage) => {
                    StorageReadableTransactionKind::RocksDb(storage.start_readable_transaction()?)
                }
                StorageKind::Memory(storage) => {
                    StorageReadableTransactionKind::Memory(storage.start_readable_transaction()?)
                }
            },
        })
    }

    fn flush(&self) -> Result<(), StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageKind::RocksDb(storage) => storage.flush(),
            StorageKind::Memory(storage) => storage.flush(),
        }
    }

    fn compact(&self) -> Result<(), StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageKind::RocksDb(storage) => storage.compact(),
            StorageKind::Memory(storage) => storage.compact(),
        }
    }

    fn backup(&self, target_directory: &Path) -> Result<(), StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageKind::RocksDb(storage) => storage.backup(target_directory),
            StorageKind::Memory(storage) => storage.backup(target_directory),
        }
    }

    fn bulk_loader(&self) -> Result<StorageBulkLoader<'_>, StorageError> {
        Ok(match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageKind::RocksDb(storage) => StorageBulkLoader {
                kind: StorageBulkLoaderKind::RocksDb(storage.bulk_loader()?),
            },
            StorageKind::Memory(storage) => StorageBulkLoader {
                kind: StorageBulkLoaderKind::Memory(storage.bulk_loader()?),
            },
        })
    }
}

#[must_use]
pub struct StorageReader<'a> {
    kind: StorageReaderKind<'a>,
}

enum StorageReaderKind<'a> {
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    RocksDb(RocksDbStorageReader<'a>),
    Memory(MemoryStorageReader<'a>),
}

impl<'a> Reader<'a> for StorageReader<'a> {
    type Error = StorageError;
    type QuadIterator<'iter>
        = DecodingQuadIterator<'iter>
    where
        Self: 'iter;
    type TermIterator<'iter>
        = DecodingGraphIterator<'iter>
    where
        Self: 'iter;

    fn len(&self) -> Result<usize, StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.len(),
            StorageReaderKind::Memory(reader) => reader.len(),
        }
    }

    fn is_empty(&self) -> Result<bool, StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.is_empty(),
            StorageReaderKind::Memory(reader) => reader.is_empty(),
        }
    }

    fn contains(&self, quad: &EncodedQuad) -> Result<bool, StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.contains(quad),
            StorageReaderKind::Memory(reader) => reader.contains(quad),
        }
    }

    fn quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> Self::QuadIterator<'a> {
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

    fn named_graphs(&self) -> Self::TermIterator<'a> {
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

    fn contains_named_graph(&self, graph_name: &EncodedTerm) -> Result<bool, StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.contains_named_graph(graph_name),
            StorageReaderKind::Memory(reader) => reader.contains_named_graph(graph_name),
        }
    }

    fn contains_str(&self, key: &StrHash) -> Result<bool, StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.contains_str(key),
            StorageReaderKind::Memory(reader) => reader.contains_str(key),
        }
    }

    fn validate(&self) -> Result<(), StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.validate(),
            StorageReaderKind::Memory(reader) => reader.validate(),
        }
    }
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
            DecodingQuadIteratorKind::Memory(iter) => iter.next(),
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
            DecodingGraphIteratorKind::Memory(iter) => iter.next(),
        }
    }
}

impl StrLookup for StorageReader<'_> {
    fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.get_str(key),
            StorageReaderKind::Memory(reader) => reader.get_str(key),
        }
    }
}

#[must_use]
pub struct StorageTransaction<'a> {
    kind: StorageTransactionKind<'a>,
}

enum StorageTransactionKind<'a> {
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    RocksDb(RocksDbStorageTransaction<'a>),
    Memory(MemoryStorageTransaction<'a>),
}

impl WriteOnlyTransaction<'_> for StorageTransaction<'_> {
    type Error = StorageError;

    fn insert(&mut self, quad: QuadRef<'_>) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.insert(quad),
            StorageTransactionKind::Memory(transaction) => {
                transaction.insert(quad);
            }
        }
    }

    fn insert_named_graph(&mut self, graph_name: NamedOrBlankNodeRef<'_>) {
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

    fn remove(&mut self, quad: QuadRef<'_>) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.remove(quad),
            StorageTransactionKind::Memory(transaction) => transaction.remove(quad),
        }
    }

    fn clear_default_graph(&mut self) -> Result<(), StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.clear_default_graph(),
            StorageTransactionKind::Memory(transaction) => {
                transaction.clear_graph(GraphNameRef::DefaultGraph)
            }
        }
    }

    fn clear_graph(&mut self, graph_name: GraphNameRef<'_>) -> Result<(), StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.clear_graph(graph_name),
            StorageTransactionKind::Memory(transaction) => transaction.clear_graph(graph_name),
        }
    }

    fn clear_all_named_graphs(&mut self) -> Result<(), StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.clear_all_named_graphs(),
            StorageTransactionKind::Memory(transaction) => transaction.clear_all_named_graphs(),
        }
    }

    fn clear_all_graphs(&mut self) -> Result<(), StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.clear_all_graphs(),
            StorageTransactionKind::Memory(transaction) => transaction.clear_all_graphs(),
        }
    }

    fn remove_all_named_graphs(&mut self) -> Result<(), StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.remove_all_named_graphs(),
            StorageTransactionKind::Memory(transaction) => transaction.remove_all_named_graphs(),
        }
    }

    fn clear(&mut self) -> Result<(), StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.clear(),
            StorageTransactionKind::Memory(transaction) => transaction.clear(),
        }
    }

    fn commit(self) -> Result<(), StorageError> {
        match self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.commit(),
            StorageTransactionKind::Memory(transaction) => transaction.commit(),
        }
    }
}

#[must_use]
pub struct StorageReadableTransaction<'a> {
    kind: StorageReadableTransactionKind<'a>,
}

enum StorageReadableTransactionKind<'a> {
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    RocksDb(RocksDbStorageReadableTransaction<'a>),
    Memory(MemoryStorageTransaction<'a>),
}

impl ReadWriteTransaction<'_> for StorageReadableTransaction<'_> {
    type Reader<'reader>
        = StorageReader<'reader>
    where
        Self: 'reader;

    fn reader(&self) -> StorageReader<'_> {
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
        }
    }

    fn remove_named_graph(
        &mut self,
        graph_name: NamedOrBlankNodeRef<'_>,
    ) -> Result<(), StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => {
                transaction.remove_named_graph(graph_name)
            }
            StorageReadableTransactionKind::Memory(transaction) => {
                transaction.remove_named_graph(graph_name)
            }
        }
    }
}

impl WriteOnlyTransaction<'_> for StorageReadableTransaction<'_> {
    type Error = StorageError;

    fn insert(&mut self, quad: QuadRef<'_>) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => transaction.insert(quad),
            StorageReadableTransactionKind::Memory(transaction) => transaction.insert(quad),
        }
    }

    fn insert_named_graph(&mut self, graph_name: NamedOrBlankNodeRef<'_>) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => {
                transaction.insert_named_graph(graph_name)
            }
            StorageReadableTransactionKind::Memory(transaction) => {
                transaction.insert_named_graph(graph_name)
            }
        }
    }

    fn remove(&mut self, quad: QuadRef<'_>) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => transaction.remove(quad),
            StorageReadableTransactionKind::Memory(transaction) => transaction.remove(quad),
        }
    }

    fn clear_graph(&mut self, graph_name: GraphNameRef<'_>) -> Result<(), StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => {
                transaction.clear_graph(graph_name)
            }
            StorageReadableTransactionKind::Memory(transaction) => {
                transaction.clear_graph(graph_name)
            }
        }
    }

    fn clear_all_named_graphs(&mut self) -> Result<(), StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => {
                transaction.clear_all_named_graphs()
            }
            StorageReadableTransactionKind::Memory(transaction) => {
                transaction.clear_all_named_graphs()
            }
        }
    }

    fn clear_all_graphs(&mut self) -> Result<(), StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => transaction.clear_all_graphs(),
            StorageReadableTransactionKind::Memory(transaction) => transaction.clear_all_graphs(),
        }
    }

    fn remove_all_named_graphs(&mut self) -> Result<(), StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => {
                transaction.remove_all_named_graphs()
            }
            StorageReadableTransactionKind::Memory(transaction) => {
                transaction.remove_all_named_graphs()
            }
        }
    }

    fn clear(&mut self) -> Result<(), StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => transaction.clear(),
            StorageReadableTransactionKind::Memory(transaction) => transaction.clear(),
        }
    }

    fn commit(self) -> Result<(), StorageError> {
        match self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => transaction.commit(),
            StorageReadableTransactionKind::Memory(transaction) => transaction.commit(),
        }
    }
}

#[must_use]
pub struct StorageBulkLoader<'a> {
    kind: StorageBulkLoaderKind<'a>,
}

enum StorageBulkLoaderKind<'a> {
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    RocksDb(RocksDbStorageBulkLoader<'a>),
    Memory(MemoryStorageBulkLoader<'a>),
}

impl BulkLoader<'_> for StorageBulkLoader<'_> {
    type Error = StorageError;

    fn on_progress(self, callback: impl Fn(u64) + Send + Sync + 'static) -> Self {
        match self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageBulkLoaderKind::RocksDb(loader) => Self {
                kind: StorageBulkLoaderKind::RocksDb(loader.on_progress(callback)),
            },
            StorageBulkLoaderKind::Memory(loader) => Self {
                kind: StorageBulkLoaderKind::Memory(loader.on_progress(callback)),
            },
        }
    }

    fn without_atomicity(self) -> Self {
        match self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageBulkLoaderKind::RocksDb(loader) => Self {
                kind: StorageBulkLoaderKind::RocksDb(loader.without_atomicity()),
            },
            StorageBulkLoaderKind::Memory(loader) => Self {
                kind: StorageBulkLoaderKind::Memory(loader),
            },
        }
    }

    fn load_batch(&mut self, quads: Vec<Quad>, max_num_threads: usize) -> Result<(), StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageBulkLoaderKind::RocksDb(loader) => loader.load_batch(quads, max_num_threads),
            StorageBulkLoaderKind::Memory(loader) => loader.load_batch(quads, max_num_threads),
        }
    }

    fn commit(self) -> Result<(), StorageError> {
        match self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageBulkLoaderKind::RocksDb(loader) => loader.commit(),
            StorageBulkLoaderKind::Memory(loader) => loader.commit(),
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
