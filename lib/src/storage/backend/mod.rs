//! A storage backend
//! RocksDB is available, if not in memory

#[cfg(target_arch = "wasm32")]
pub use fallback::{ColumnFamily, ColumnFamilyDefinition, Db, Iter, Reader, Transaction};
#[cfg(not(target_arch = "wasm32"))]
pub use rocksdb::{
    ColumnFamily, ColumnFamilyDefinition, Db, Iter, Reader, SstFileWriter, Transaction,
};

#[cfg(target_arch = "wasm32")]
mod fallback;
#[cfg(not(target_arch = "wasm32"))]
mod rocksdb;
