//! Provides implementations of the `rudf::model::Graph` and `rudf::model::Dataset` traits.

pub(crate) mod encoded;
pub mod isomorphism;
mod memory;
pub(crate) mod numeric_encoder;
#[cfg(feature = "rocksdb")]
mod rocksdb;

pub use store::memory::MemoryDataset;
pub use store::memory::MemoryGraph;
#[cfg(feature = "rocksdb")]
pub use store::rocksdb::RocksDbDataset;
