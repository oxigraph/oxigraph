//! Provides implementations of the `rudf::model::Graph` and `rudf::model::Dataset` traits.

pub mod isomorphism;
mod memory;
mod numeric_encoder;
mod rocksdb;
mod sparql;
mod store;

pub use store::memory::MemoryDataset;
pub use store::memory::MemoryGraph;
pub use store::rocksdb::RocksDbDataset;
