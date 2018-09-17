pub mod isomorphism;
mod memory;
mod numeric_encoder;
mod rocksdb;
mod store;

pub use store::memory::MemoryDataset;
pub use store::memory::MemoryGraph;
pub use store::rocksdb::RocksDbDataset;
