//! A storage backend
//! RocksDB is available, if not in memory

#[cfg(any(target_family = "wasm", not(feature = "rocksdb")))]
pub use fallback::{ColumnFamily, ColumnFamilyDefinition, Db, Iter, Reader, Transaction};
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
pub use rocksdb::{ColumnFamily, ColumnFamilyDefinition, Db, Iter, Reader, Transaction};

#[cfg(any(target_family = "wasm", not(feature = "rocksdb")))]
mod fallback;
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
mod rocksdb;
