//! A storage backend
//! RocksDB is available, if not in memory

#[cfg(target_arch = "wasm32")]
pub use fallback::{
    ColumnFamily, ColumnFamilyDefinition, Db, Iter, MergeOperator, WriteBatchWithIndex,
};
#[cfg(not(target_arch = "wasm32"))]
pub use rocksdb::{
    ColumnFamily, ColumnFamilyDefinition, Db, Iter, MergeOperator, Reader, SstFileWriter,
    Transaction,
};
use std::ffi::CString;

#[cfg(target_arch = "wasm32")]
mod fallback;
#[cfg(not(target_arch = "wasm32"))]
mod rocksdb;

pub struct CompactionFilter {
    pub filter: fn(&[u8], &[u8]) -> CompactionAction,
    pub name: CString,
}

#[warn(dead_code)]
pub enum CompactionAction {
    Keep,
    Remove,
}
