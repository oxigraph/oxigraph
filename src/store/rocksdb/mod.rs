mod storage;

use errors::*;
use std::path::Path;
use store::rocksdb::storage::*;
use store::store::StoreDataset;

pub type RocksDbDataset = StoreDataset<RocksDbStore>;

impl RocksDbDataset {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::new_from_store(RocksDbStore::open(path)?))
    }
}
