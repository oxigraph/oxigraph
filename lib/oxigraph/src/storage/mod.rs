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
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use std::path::Path;

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

    pub fn snapshot(&self) -> StorageReader<'static> {
        StorageReader {
            kind: match &self.kind {
                #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
                StorageKind::RocksDb(storage) => StorageReaderKind::RocksDb(storage.snapshot()),
                StorageKind::Memory(storage) => StorageReaderKind::Memory(storage.snapshot()),
            },
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

    pub fn bulk_loader(&self) -> StorageBulkLoader {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageKind::RocksDb(storage) => StorageBulkLoader {
                kind: StorageBulkLoaderKind::RocksDb(storage.bulk_loader()),
            },
            StorageKind::Memory(storage) => StorageBulkLoader {
                kind: StorageBulkLoaderKind::Memory(storage.bulk_loader()),
            },
        }
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

#[cfg_attr(
    not(all(not(target_family = "wasm"), feature = "rocksdb")),
    expect(clippy::unnecessary_wraps)
)]
impl<'a> StorageReader<'a> {
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

#[cfg_attr(
    not(all(not(target_family = "wasm"), feature = "rocksdb")),
    expect(clippy::unnecessary_wraps)
)]
impl StorageTransaction<'_> {
    pub fn insert(&mut self, quad: QuadRef<'_>) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.insert(quad),
            StorageTransactionKind::Memory(transaction) => {
                transaction.insert(quad);
            }
        }
    }

    pub fn insert_named_graph(&mut self, graph_name: NamedOrBlankNodeRef<'_>) {
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

    pub fn remove(&mut self, quad: QuadRef<'_>) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.remove(quad),
            StorageTransactionKind::Memory(transaction) => transaction.remove(quad),
        }
    }

    pub fn clear_default_graph(&mut self) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.clear_default_graph(),
            StorageTransactionKind::Memory(transaction) => {
                transaction.clear_graph(GraphNameRef::DefaultGraph)
            }
        }
    }

    pub fn clear_all_named_graphs(&mut self) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.clear_all_named_graphs(),
            StorageTransactionKind::Memory(transaction) => transaction.clear_all_named_graphs(),
        }
    }

    pub fn clear_all_graphs(&mut self) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.clear_all_graphs(),
            StorageTransactionKind::Memory(transaction) => transaction.clear_all_graphs(),
        }
    }

    pub fn remove_all_named_graphs(&mut self) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.remove_all_named_graphs(),
            StorageTransactionKind::Memory(transaction) => transaction.remove_all_named_graphs(),
        }
    }

    pub fn clear(&mut self) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.clear(),
            StorageTransactionKind::Memory(transaction) => transaction.clear(),
        }
    }

    pub fn commit(self) -> Result<(), StorageError> {
        match self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageTransactionKind::RocksDb(transaction) => transaction.commit(),
            StorageTransactionKind::Memory(transaction) => {
                transaction.commit();
                Ok(())
            }
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

#[cfg_attr(
    not(all(not(target_family = "wasm"), feature = "rocksdb")),
    expect(clippy::unnecessary_wraps)
)]
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
        }
    }

    pub fn insert(&mut self, quad: QuadRef<'_>) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => transaction.insert(quad),
            StorageReadableTransactionKind::Memory(transaction) => {
                transaction.insert(quad);
            }
        }
    }

    pub fn insert_named_graph(&mut self, graph_name: NamedOrBlankNodeRef<'_>) {
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

    pub fn remove(&mut self, quad: QuadRef<'_>) {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => transaction.remove(quad),
            StorageReadableTransactionKind::Memory(transaction) => transaction.remove(quad),
        }
    }

    pub fn clear_graph(&mut self, graph_name: GraphNameRef<'_>) -> Result<(), StorageError> {
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
        graph_name: NamedOrBlankNodeRef<'_>,
    ) -> Result<(), StorageError> {
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
        match self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReadableTransactionKind::RocksDb(transaction) => transaction.commit(),
            StorageReadableTransactionKind::Memory(transaction) => {
                transaction.commit();
                Ok(())
            }
        }
    }
}

#[must_use]
pub struct StorageBulkLoader {
    kind: StorageBulkLoaderKind,
}

enum StorageBulkLoaderKind {
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    RocksDb(RocksDbStorageBulkLoader),
    Memory(MemoryStorageBulkLoader),
}

impl StorageBulkLoader {
    #[cfg_attr(
        not(all(not(target_family = "wasm"), feature = "rocksdb")),
        expect(unused_variables)
    )]
    pub fn with_num_threads(self, num_threads: usize) -> Self {
        match self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageBulkLoaderKind::RocksDb(loader) => Self {
                kind: StorageBulkLoaderKind::RocksDb(loader.with_num_threads(num_threads)),
            },
            StorageBulkLoaderKind::Memory(loader) => Self {
                kind: StorageBulkLoaderKind::Memory(loader),
            },
        }
    }

    #[allow(unused_variables, clippy::allow_attributes)]
    pub fn with_max_memory_size_in_megabytes(self, max_memory_size: usize) -> Self {
        match self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageBulkLoaderKind::RocksDb(loader) => Self {
                kind: StorageBulkLoaderKind::RocksDb(
                    loader.with_max_memory_size_in_megabytes(max_memory_size),
                ),
            },
            StorageBulkLoaderKind::Memory(loader) => Self {
                kind: StorageBulkLoaderKind::Memory(loader),
            },
        }
    }

    pub fn on_progress(self, callback: impl Fn(u64) + Send + Sync + 'static) -> Self {
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

    pub fn load<EI, EO: From<StorageError> + From<EI>>(
        &self,
        quads: impl IntoIterator<Item = Result<Quad, EI>>,
    ) -> Result<(), EO> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageBulkLoaderKind::RocksDb(loader) => loader.load(quads),
            StorageBulkLoaderKind::Memory(loader) => loader.load(quads),
        }
    }
}
