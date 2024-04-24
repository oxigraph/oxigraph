use crate::model::{GraphNameRef, NamedOrBlankNodeRef, QuadRef};
pub use crate::storage::error::{CorruptionError, LoaderError, SerializerError, StorageError};
use crate::storage::memory::{
    MemoryChainedDecodingQuadIterator, MemoryDecodingGraphIterator, MemoryStorage,
    MemoryStorageBulkLoader, MemoryStorageReader, MemoryStorageWriter,
};
use crate::storage::numeric_encoder::{EncodedQuad, EncodedTerm, StrHash, StrLookup};
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use crate::storage::rocksdb::{
    RocksDbChainedDecodingQuadIterator, RocksDbDecodingGraphIterator, RocksDbStorage,
    RocksDbStorageBulkLoader, RocksDbStorageReader, RocksDbStorageWriter,
};
use oxrdf::Quad;
use std::error::Error;
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use std::path::Path;

mod backend;
mod binary_encoder;
mod error;
mod memory;
pub mod numeric_encoder;
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
mod rocksdb;
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
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn new() -> Result<Self, StorageError> {
        Ok(Self {
            kind: StorageKind::RocksDb(RocksDbStorage::new()?),
        })
    }

    #[cfg(any(target_family = "wasm", not(feature = "rocksdb")))]
    pub fn new() -> Result<Self, StorageError> {
        Self::new_in_memory()
    }

    pub fn new_in_memory() -> Result<Self, StorageError> {
        Ok(Self {
            kind: StorageKind::Memory(MemoryStorage::new()?),
        })
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        Ok(Self {
            kind: StorageKind::RocksDb(RocksDbStorage::open(path)?),
        })
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn open_secondary(primary_path: &Path) -> Result<Self, StorageError> {
        Ok(Self {
            kind: StorageKind::RocksDb(RocksDbStorage::open_secondary(primary_path)?),
        })
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn open_persistent_secondary(
        primary_path: &Path,
        secondary_path: &Path,
    ) -> Result<Self, StorageError> {
        Ok(Self {
            kind: StorageKind::RocksDb(RocksDbStorage::open_persistent_secondary(
                primary_path,
                secondary_path,
            )?),
        })
    }

    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn open_read_only(path: &Path) -> Result<Self, StorageError> {
        Ok(Self {
            kind: StorageKind::RocksDb(RocksDbStorage::open_read_only(path)?),
        })
    }

    pub fn snapshot(&self) -> StorageReader {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageKind::RocksDb(storage) => StorageReader {
                kind: StorageReaderKind::RocksDb(storage.snapshot()),
            },
            StorageKind::Memory(storage) => StorageReader {
                kind: StorageReaderKind::Memory(storage.snapshot()),
            },
        }
    }

    pub fn transaction<'a, 'b: 'a, T, E: Error + 'static + From<StorageError>>(
        &'b self,
        f: impl Fn(StorageWriter<'a>) -> Result<T, E>,
    ) -> Result<T, E> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageKind::RocksDb(storage) => storage.transaction(|transaction| {
                f(StorageWriter {
                    kind: StorageWriterKind::RocksDb(transaction),
                })
            }),
            StorageKind::Memory(storage) => storage.transaction(|transaction| {
                f(StorageWriter {
                    kind: StorageWriterKind::Memory(transaction),
                })
            }),
        }
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

pub struct StorageReader {
    kind: StorageReaderKind,
}

enum StorageReaderKind {
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    RocksDb(RocksDbStorageReader),
    Memory(MemoryStorageReader),
}

impl StorageReader {
    pub fn len(&self) -> Result<usize, StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.len(),
            StorageReaderKind::Memory(reader) => reader.len(),
        }
    }

    pub fn is_empty(&self) -> Result<bool, StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.is_empty(),
            StorageReaderKind::Memory(reader) => reader.is_empty(),
        }
    }

    pub fn contains(&self, quad: &EncodedQuad) -> Result<bool, StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.contains(quad),
            StorageReaderKind::Memory(reader) => reader.contains(quad),
        }
    }

    pub fn quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> DecodingQuadIterator {
        match subject {
            Some(subject) => match predicate {
                Some(predicate) => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => self.quads_for_subject_predicate_object_graph(
                            subject, predicate, object, graph_name,
                        ),
                        None => self.quads_for_subject_predicate_object(subject, predicate, object),
                    },
                    None => match graph_name {
                        Some(graph_name) => {
                            self.quads_for_subject_predicate_graph(subject, predicate, graph_name)
                        }
                        None => self.quads_for_subject_predicate(subject, predicate),
                    },
                },
                None => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => {
                            self.quads_for_subject_object_graph(subject, object, graph_name)
                        }
                        None => self.quads_for_subject_object(subject, object),
                    },
                    None => match graph_name {
                        Some(graph_name) => self.quads_for_subject_graph(subject, graph_name),
                        None => self.quads_for_subject(subject),
                    },
                },
            },
            None => match predicate {
                Some(predicate) => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => {
                            self.quads_for_predicate_object_graph(predicate, object, graph_name)
                        }
                        None => self.quads_for_predicate_object(predicate, object),
                    },
                    None => match graph_name {
                        Some(graph_name) => self.quads_for_predicate_graph(predicate, graph_name),
                        None => self.quads_for_predicate(predicate),
                    },
                },
                None => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => self.quads_for_object_graph(object, graph_name),
                        None => self.quads_for_object(object),
                    },
                    None => match graph_name {
                        Some(graph_name) => self.quads_for_graph(graph_name),
                        None => self.quads(),
                    },
                },
            },
        }
    }

    pub fn quads(&self) -> DecodingQuadIterator {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::RocksDb(reader.quads()),
            },
            StorageReaderKind::Memory(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::Memory(reader.quads()),
            },
        }
    }

    fn quads_for_subject(&self, subject: &EncodedTerm) -> DecodingQuadIterator {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::RocksDb(reader.quads_for_subject(subject)),
            },
            StorageReaderKind::Memory(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::Memory(reader.quads_for_subject(subject)),
            },
        }
    }

    fn quads_for_subject_predicate(
        &self,
        subject: &EncodedTerm,
        predicate: &EncodedTerm,
    ) -> DecodingQuadIterator {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::RocksDb(
                    reader.quads_for_subject_predicate(subject, predicate),
                ),
            },
            StorageReaderKind::Memory(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::Memory(
                    reader.quads_for_subject_predicate(subject, predicate),
                ),
            },
        }
    }

    fn quads_for_subject_predicate_object(
        &self,
        subject: &EncodedTerm,
        predicate: &EncodedTerm,
        object: &EncodedTerm,
    ) -> DecodingQuadIterator {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::RocksDb(
                    reader.quads_for_subject_predicate_object(subject, predicate, object),
                ),
            },
            StorageReaderKind::Memory(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::Memory(
                    reader.quads_for_subject_predicate_object(subject, predicate, object),
                ),
            },
        }
    }

    fn quads_for_subject_object(
        &self,
        subject: &EncodedTerm,
        object: &EncodedTerm,
    ) -> DecodingQuadIterator {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::RocksDb(
                    reader.quads_for_subject_object(subject, object),
                ),
            },
            StorageReaderKind::Memory(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::Memory(
                    reader.quads_for_subject_object(subject, object),
                ),
            },
        }
    }

    fn quads_for_predicate(&self, predicate: &EncodedTerm) -> DecodingQuadIterator {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::RocksDb(reader.quads_for_predicate(predicate)),
            },
            StorageReaderKind::Memory(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::Memory(reader.quads_for_predicate(predicate)),
            },
        }
    }

    fn quads_for_predicate_object(
        &self,
        predicate: &EncodedTerm,
        object: &EncodedTerm,
    ) -> DecodingQuadIterator {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::RocksDb(
                    reader.quads_for_predicate_object(predicate, object),
                ),
            },
            StorageReaderKind::Memory(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::Memory(
                    reader.quads_for_predicate_object(predicate, object),
                ),
            },
        }
    }

    fn quads_for_object(&self, object: &EncodedTerm) -> DecodingQuadIterator {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::RocksDb(reader.quads_for_object(object)),
            },
            StorageReaderKind::Memory(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::Memory(reader.quads_for_object(object)),
            },
        }
    }

    fn quads_for_graph(&self, graph_name: &EncodedTerm) -> DecodingQuadIterator {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::RocksDb(reader.quads_for_graph(graph_name)),
            },
            StorageReaderKind::Memory(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::Memory(reader.quads_for_graph(graph_name)),
            },
        }
    }

    fn quads_for_subject_graph(
        &self,
        subject: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> DecodingQuadIterator {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::RocksDb(
                    reader.quads_for_subject_graph(subject, graph_name),
                ),
            },
            StorageReaderKind::Memory(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::Memory(
                    reader.quads_for_subject_graph(subject, graph_name),
                ),
            },
        }
    }

    fn quads_for_subject_predicate_graph(
        &self,
        subject: &EncodedTerm,
        predicate: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> DecodingQuadIterator {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::RocksDb(
                    reader.quads_for_subject_predicate_graph(subject, predicate, graph_name),
                ),
            },
            StorageReaderKind::Memory(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::Memory(
                    reader.quads_for_subject_predicate_graph(subject, predicate, graph_name),
                ),
            },
        }
    }

    fn quads_for_subject_predicate_object_graph(
        &self,
        subject: &EncodedTerm,
        predicate: &EncodedTerm,
        object: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> DecodingQuadIterator {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::RocksDb(
                    reader.quads_for_subject_predicate_object_graph(
                        subject, predicate, object, graph_name,
                    ),
                ),
            },
            StorageReaderKind::Memory(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::Memory(
                    reader.quads_for_subject_predicate_object_graph(
                        subject, predicate, object, graph_name,
                    ),
                ),
            },
        }
    }

    fn quads_for_subject_object_graph(
        &self,
        subject: &EncodedTerm,
        object: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> DecodingQuadIterator {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::RocksDb(
                    reader.quads_for_subject_object_graph(subject, object, graph_name),
                ),
            },
            StorageReaderKind::Memory(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::Memory(
                    reader.quads_for_subject_object_graph(subject, object, graph_name),
                ),
            },
        }
    }

    fn quads_for_predicate_graph(
        &self,
        predicate: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> DecodingQuadIterator {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::RocksDb(
                    reader.quads_for_predicate_graph(predicate, graph_name),
                ),
            },
            StorageReaderKind::Memory(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::Memory(
                    reader.quads_for_predicate_graph(predicate, graph_name),
                ),
            },
        }
    }

    fn quads_for_predicate_object_graph(
        &self,
        predicate: &EncodedTerm,
        object: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> DecodingQuadIterator {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::RocksDb(
                    reader.quads_for_predicate_object_graph(predicate, object, graph_name),
                ),
            },
            StorageReaderKind::Memory(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::Memory(
                    reader.quads_for_predicate_object_graph(predicate, object, graph_name),
                ),
            },
        }
    }

    fn quads_for_object_graph(
        &self,
        object: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> DecodingQuadIterator {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::RocksDb(
                    reader.quads_for_object_graph(object, graph_name),
                ),
            },
            StorageReaderKind::Memory(reader) => DecodingQuadIterator {
                kind: DecodingQuadIteratorKind::Memory(
                    reader.quads_for_object_graph(object, graph_name),
                ),
            },
        }
    }

    pub fn named_graphs(&self) -> DecodingGraphIterator {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => DecodingGraphIterator {
                kind: DecodingGraphIteratorKind::RocksDb(reader.named_graphs()),
            },
            StorageReaderKind::Memory(reader) => DecodingGraphIterator {
                kind: DecodingGraphIteratorKind::Memory(reader.named_graphs()),
            },
        }
    }

    pub fn contains_named_graph(&self, graph_name: &EncodedTerm) -> Result<bool, StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.contains_named_graph(graph_name),
            StorageReaderKind::Memory(reader) => reader.contains_named_graph(graph_name),
        }
    }

    pub fn contains_str(&self, key: &StrHash) -> Result<bool, StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.contains_str(key),
            StorageReaderKind::Memory(reader) => reader.contains_str(key),
        }
    }

    /// Validates that all the storage invariants held in the data
    pub fn validate(&self) -> Result<(), StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.validate(),
            StorageReaderKind::Memory(reader) => reader.validate(),
        }
    }
}

pub struct DecodingQuadIterator {
    kind: DecodingQuadIteratorKind,
}

enum DecodingQuadIteratorKind {
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    RocksDb(RocksDbChainedDecodingQuadIterator),
    Memory(MemoryChainedDecodingQuadIterator),
}

impl Iterator for DecodingQuadIterator {
    type Item = Result<EncodedQuad, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            DecodingQuadIteratorKind::RocksDb(iter) => iter.next(),
            DecodingQuadIteratorKind::Memory(iter) => iter.next(),
        }
    }
}

pub struct DecodingGraphIterator {
    kind: DecodingGraphIteratorKind,
}

enum DecodingGraphIteratorKind {
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    RocksDb(RocksDbDecodingGraphIterator),
    Memory(MemoryDecodingGraphIterator),
}

impl Iterator for DecodingGraphIterator {
    type Item = Result<EncodedTerm, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            DecodingGraphIteratorKind::RocksDb(iter) => iter.next(),
            DecodingGraphIteratorKind::Memory(iter) => iter.next(),
        }
    }
}

impl StrLookup for StorageReader {
    fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageReaderKind::RocksDb(reader) => reader.get_str(key),
            StorageReaderKind::Memory(reader) => reader.get_str(key),
        }
    }
}

pub struct StorageWriter<'a> {
    kind: StorageWriterKind<'a>,
}

enum StorageWriterKind<'a> {
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    RocksDb(RocksDbStorageWriter<'a>),
    Memory(MemoryStorageWriter<'a>),
}

impl<'a> StorageWriter<'a> {
    pub fn reader(&self) -> StorageReader {
        match &self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageWriterKind::RocksDb(writer) => StorageReader {
                kind: StorageReaderKind::RocksDb(writer.reader()),
            },
            StorageWriterKind::Memory(writer) => StorageReader {
                kind: StorageReaderKind::Memory(writer.reader()),
            },
        }
    }

    pub fn insert(&mut self, quad: QuadRef<'_>) -> Result<bool, StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageWriterKind::RocksDb(writer) => writer.insert(quad),
            StorageWriterKind::Memory(writer) => writer.insert(quad),
        }
    }

    pub fn insert_named_graph(
        &mut self,
        graph_name: NamedOrBlankNodeRef<'_>,
    ) -> Result<bool, StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageWriterKind::RocksDb(writer) => writer.insert_named_graph(graph_name),
            StorageWriterKind::Memory(writer) => writer.insert_named_graph(graph_name),
        }
    }

    pub fn remove(&mut self, quad: QuadRef<'_>) -> Result<bool, StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageWriterKind::RocksDb(writer) => writer.remove(quad),
            StorageWriterKind::Memory(writer) => writer.remove(quad),
        }
    }

    pub fn clear_graph(&mut self, graph_name: GraphNameRef<'_>) -> Result<(), StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageWriterKind::RocksDb(writer) => writer.clear_graph(graph_name),
            StorageWriterKind::Memory(writer) => writer.clear_graph(graph_name),
        }
    }

    pub fn clear_all_named_graphs(&mut self) -> Result<(), StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageWriterKind::RocksDb(writer) => writer.clear_all_named_graphs(),
            StorageWriterKind::Memory(writer) => writer.clear_all_named_graphs(),
        }
    }

    pub fn clear_all_graphs(&mut self) -> Result<(), StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageWriterKind::RocksDb(writer) => writer.clear_all_graphs(),
            StorageWriterKind::Memory(writer) => writer.clear_all_graphs(),
        }
    }

    pub fn remove_named_graph(
        &mut self,
        graph_name: NamedOrBlankNodeRef<'_>,
    ) -> Result<bool, StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageWriterKind::RocksDb(writer) => writer.remove_named_graph(graph_name),
            StorageWriterKind::Memory(writer) => writer.remove_named_graph(graph_name),
        }
    }

    pub fn remove_all_named_graphs(&mut self) -> Result<(), StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageWriterKind::RocksDb(writer) => writer.remove_all_named_graphs(),
            StorageWriterKind::Memory(writer) => writer.remove_all_named_graphs(),
        }
    }

    pub fn clear(&mut self) -> Result<(), StorageError> {
        match &mut self.kind {
            #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
            StorageWriterKind::RocksDb(writer) => writer.clear(),
            StorageWriterKind::Memory(writer) => writer.clear(),
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
    #[allow(unused_variables)]
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

    #[allow(unused_variables)]
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

    pub fn on_progress(self, callback: impl Fn(u64) + 'static) -> Self {
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

    #[allow(clippy::trait_duplication_in_bounds)]
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
