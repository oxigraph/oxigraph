//! A storage backend
//! RocksDB is available, if not in memory

#[cfg(target_arch = "wasm32")]
pub use fallback::{ColumnFamily, Db, Iter, MergeOperator};
#[cfg(not(target_arch = "wasm32"))]
pub use rocksdb::{ColumnFamily, Db, Iter, MergeOperator};
use std::ffi::CString;

#[cfg(target_arch = "wasm32")]
mod fallback;
#[cfg(not(target_arch = "wasm32"))]
mod rocksdb;

pub struct ColumnFamilyDefinition {
    pub name: &'static str,
    pub merge_operator: Option<MergeOperator>,
    pub compaction_filter: Option<CompactionFilter>,
    pub use_iter: bool,
    pub min_prefix_size: usize,
}

pub struct CompactionFilter {
    pub filter: fn(&[u8], &[u8]) -> CompactionAction,
    pub name: CString,
}

#[warn(dead_code)]
pub enum CompactionAction {
    Keep,
    Remove,
}
