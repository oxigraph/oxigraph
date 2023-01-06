//! A storage backend
//! RocksDB is available, if not in memory

#[cfg(target_family = "wasm")]
pub use fallback::{ColumnFamily, ColumnFamilyDefinition, Db, Iter, Reader, Transaction};
#[cfg(not(target_family = "wasm"))]
pub use rocksdb::{
    ColumnFamily, ColumnFamilyDefinition, Db, Iter, Reader, SstFileWriter, Transaction,
};

#[cfg(target_family = "wasm")]
mod fallback;
#[cfg(not(target_family = "wasm"))]
mod rocksdb;
