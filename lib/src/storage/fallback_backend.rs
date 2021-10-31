//! TODO: This storage is dramatically naive.

use std::collections::BTreeMap;
use std::io::Result;
use std::sync::{Arc, RwLock};

pub struct ColumnFamilyDefinition {
    pub name: &'static str,
}

#[derive(Clone)]
pub struct Db(Arc<RwLock<BTreeMap<ColumnFamily, BTreeMap<Vec<u8>, Vec<u8>>>>>);

impl Db {
    pub fn new(column_families: Vec<ColumnFamilyDefinition>) -> Result<Self> {
        let mut trees = BTreeMap::new();
        for cf in column_families {
            trees.insert(ColumnFamily(cf.name), BTreeMap::default());
        }
        trees.entry(ColumnFamily("default")).or_default(); // We make sure that "default" key exists.
        Ok(Self(Arc::new(RwLock::new(trees))))
    }

    pub fn column_family(&self, name: &'static str) -> Option<ColumnFamily> {
        let name = ColumnFamily(name);
        if self.0.read().unwrap().contains_key(&name) {
            Some(name)
        } else {
            None
        }
    }

    pub fn flush(&self) -> Result<()> {
        Ok(())
    }

    pub fn get(&self, column_family: &ColumnFamily, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self
            .0
            .read()
            .unwrap()
            .get(column_family)
            .unwrap()
            .get(key)
            .map(|v| v.to_vec()))
    }

    pub fn contains_key(&self, column_family: &ColumnFamily, key: &[u8]) -> Result<bool> {
        Ok(self
            .0
            .read()
            .unwrap()
            .get(column_family)
            .unwrap()
            .contains_key(key.as_ref()))
    }

    pub fn insert(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
        value: &[u8],
        _low_priority: bool,
    ) -> Result<()> {
        self.0
            .write()
            .unwrap()
            .get_mut(column_family)
            .unwrap()
            .insert(key.into(), value.into());
        Ok(())
    }

    pub fn insert_empty(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
        low_priority: bool,
    ) -> Result<()> {
        self.insert(column_family, key, &[], low_priority)
    }

    pub fn remove(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
        _low_priority: bool,
    ) -> Result<bool> {
        Ok(self
            .0
            .write()
            .unwrap()
            .get_mut(column_family)
            .unwrap()
            .remove(key.as_ref())
            .is_some())
    }

    pub fn iter(&self, column_family: &ColumnFamily) -> Iter {
        self.scan_prefix(column_family, &[])
    }

    pub fn scan_prefix(&self, column_family: &ColumnFamily, prefix: &[u8]) -> Iter {
        let trees = self.0.read().unwrap();
        let tree = trees.get(column_family).unwrap();
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

    pub fn len(&self, column_family: &ColumnFamily) -> Result<usize> {
        Ok(self.0.read().unwrap().get(column_family).unwrap().len())
    }

    pub fn is_empty(&self, column_family: &ColumnFamily) -> Result<bool> {
        Ok(self
            .0
            .read()
            .unwrap()
            .get(column_family)
            .unwrap()
            .is_empty())
    }
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq)]
pub struct ColumnFamily(&'static str);

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
