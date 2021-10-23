//! TODO: This storage is dramatically naive.

use std::collections::BTreeMap;
use std::io::Result;
use std::sync::{Arc, Mutex, RwLock};

#[derive(Clone)]
pub struct Db {
    trees: Arc<Mutex<BTreeMap<&'static str, Tree>>>,
    default: Tree,
}

impl Db {
    pub fn new(_column_families: &[&str]) -> Result<Self> {
        Ok(Self {
            trees: Arc::default(),
            default: Tree::default(),
        })
    }

    pub fn open_tree(&self, name: &'static str) -> Result<Tree> {
        Ok(self.trees.lock().unwrap().entry(name).or_default().clone())
    }

    pub fn flush(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Clone, Default)]
pub struct Tree {
    tree: Arc<RwLock<BTreeMap<Vec<u8>, Vec<u8>>>>,
}

impl Tree {
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self.tree.read().unwrap().get(key).map(|v| v.to_vec()))
    }

    pub fn contains_key(&self, key: &[u8]) -> Result<bool> {
        Ok(self.tree.read().unwrap().contains_key(key.as_ref()))
    }

    pub fn insert(&self, key: &[u8], value: &[u8]) -> Result<()> {
        self.tree.write().unwrap().insert(key.into(), value.into());
        Ok(())
    }

    pub fn insert_empty(&self, key: &[u8]) -> Result<()> {
        self.insert(key, &[])
    }

    pub fn remove(&self, key: &[u8]) -> Result<bool> {
        Ok(self.tree.write().unwrap().remove(key.as_ref()).is_some())
    }

    pub fn clear(&self) -> Result<()> {
        Ok(self.tree.write().unwrap().clear())
    }

    pub fn iter(&self) -> Iter {
        self.scan_prefix(&[])
    }

    pub fn scan_prefix(&self, prefix: &[u8]) -> Iter {
        let tree = self.tree.read().unwrap();
        let data: Vec<_> = if prefix.is_empty() {
            tree.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        } else {
            tree.range(prefix.to_vec()..)
                .take_while(|(k, _)| k.starts_with(prefix))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        };
        let mut iter = data.into_iter();
        let current = iter.next();
        Iter { iter, current }
    }

    pub fn len(&self) -> usize {
        self.tree.read().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.tree.read().unwrap().is_empty()
    }
}

pub struct Iter {
    iter: std::vec::IntoIter<(Vec<u8>, Vec<u8>)>,
    current: Option<(Vec<u8>, Vec<u8>)>,
}

impl Iter {
    pub fn key(&self) -> Option<&[u8]> {
        Some(&self.current.as_ref()?.0)
    }

    pub fn value(&self) -> Option<&[u8]> {
        Some(&self.current.as_ref()?.1)
    }

    pub fn next(&mut self) {
        self.current = self.iter.next();
    }

    pub fn status(&self) -> Result<()> {
        Ok(())
    }
}
