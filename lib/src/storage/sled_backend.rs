use std::io::Result;
use std::path::Path;

#[derive(Clone)]
pub struct Db(sled::Db);

impl Db {
    pub fn new() -> Result<Self> {
        Ok(Self(sled::Config::new().temporary(true).open()?))
    }

    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self(sled::Config::new().path(path).open()?))
    }

    pub fn open_tree(&self, name: &'static str) -> Result<Tree> {
        Ok(Tree(self.0.open_tree(name)?))
    }

    pub fn flush(&self) -> Result<()> {
        self.0.flush()?;
        Ok(())
    }

    pub async fn flush_async(&self) -> Result<()> {
        self.0.flush_async().await?;
        Ok(())
    }

    pub fn get(&self, key: &[u8]) -> Result<Option<sled::IVec>> {
        Ok(self.0.get(key)?)
    }

    pub fn insert(&self, key: &[u8], value: impl Into<sled::IVec>) -> Result<bool> {
        Ok(self.0.insert(key, value)?.is_none())
    }
}

#[derive(Clone)]
pub struct Tree(sled::Tree);

impl Tree {
    pub fn get(&self, key: &[u8]) -> Result<Option<sled::IVec>> {
        Ok(self.0.get(key)?)
    }

    pub fn contains_key(&self, key: &[u8]) -> Result<bool> {
        Ok(self.0.contains_key(key)?)
    }

    pub fn insert(&self, key: &[u8], value: impl Into<sled::IVec>) -> Result<bool> {
        Ok(self.0.insert(key, value)?.is_none())
    }

    pub fn insert_empty(&self, key: &[u8]) -> Result<bool> {
        self.insert(key, &[])
    }

    pub fn merge(&self, key: &[u8], value: &[u8]) -> Result<()> {
        self.0.merge(key, value)?;
        Ok(())
    }

    pub fn remove(&self, key: &[u8]) -> Result<bool> {
        Ok(self.0.remove(key)?.is_some())
    }

    pub fn update_and_fetch<V: Into<sled::IVec>>(
        &self,
        key: &[u8],
        f: impl FnMut(Option<&[u8]>) -> Option<V>,
    ) -> Result<Option<sled::IVec>> {
        Ok(self.0.update_and_fetch(key, f)?)
    }

    pub fn clear(&self) -> Result<()> {
        Ok(self.0.clear()?)
    }

    pub fn iter(&self) -> sled::Iter {
        self.0.iter()
    }

    pub fn scan_prefix(&self, prefix: &[u8]) -> sled::Iter {
        self.0.scan_prefix(prefix)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn flush(&self) -> Result<()> {
        self.0.flush()?;
        Ok(())
    }

    pub fn set_merge_operator(
        &mut self,
        merge_operator: impl Fn(&[u8], Option<&[u8]>, &[u8]) -> Option<Vec<u8>> + 'static,
    ) {
        self.0.set_merge_operator(merge_operator)
    }

    pub fn as_sled(&self) -> &sled::Tree {
        &self.0
    }
}

pub type Iter = sled::Iter;
