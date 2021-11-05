//! TODO: This storage is dramatically naive.

use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::ffi::CString;
use std::io::Result;
use std::iter::{once, Once};
use std::sync::{Arc, RwLock};

pub struct ColumnFamilyDefinition {
    pub name: &'static str,
    pub merge_operator: Option<MergeOperator>,
    pub compaction_filter: Option<CompactionFilter>,
    pub use_iter: bool,
    pub min_prefix_size: usize,
}

#[derive(Clone)]
pub struct Db(Arc<RwLock<BTreeMap<ColumnFamily, Tree>>>);

#[derive(Default)]
struct Tree {
    tree: BTreeMap<Vec<u8>, Vec<u8>>,
    merge_operator: Option<MergeOperator>,
    compaction_filter: Option<CompactionFilter>,
}

impl Db {
    pub fn new(column_families: Vec<ColumnFamilyDefinition>) -> Result<Self> {
        let mut trees = BTreeMap::new();
        for cf in column_families {
            trees.insert(
                ColumnFamily(cf.name),
                Tree {
                    tree: BTreeMap::default(),
                    merge_operator: cf.merge_operator,
                    compaction_filter: cf.compaction_filter,
                },
            );
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

    pub fn flush(&self, _column_family: &ColumnFamily) -> Result<()> {
        Ok(())
    }

    pub fn get(&self, column_family: &ColumnFamily, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(self
            .0
            .read()
            .unwrap()
            .get(column_family)
            .unwrap()
            .tree
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
            .tree
            .contains_key(key.as_ref()))
    }

    pub fn insert(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
        value: &[u8],
        _low_priority: bool,
    ) -> Result<()> {
        let mut db = self.0.write().unwrap();
        let tree = db.get_mut(column_family).unwrap();
        let action = if let Some(filter) = &tree.compaction_filter {
            (filter.filter)(key, value)
        } else {
            CompactionAction::Keep
        };
        match action {
            CompactionAction::Keep => tree.tree.insert(key.into(), value.into()),
            CompactionAction::Remove => tree.tree.remove(key),
            CompactionAction::Replace(value) => tree.tree.insert(key.into(), value),
        };
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
            .tree
            .remove(key.as_ref())
            .is_some())
    }

    pub fn merge(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
        value: &[u8],
        _low_priority: bool,
    ) -> Result<()> {
        let mut db = self.0.write().unwrap();
        let tree = db.get_mut(column_family).unwrap();
        match tree.tree.entry(key.into()) {
            Entry::Vacant(e) => {
                let value = if let Some(merge) = &tree.merge_operator {
                    (merge.full)(key, None, once(value))
                } else {
                    value.into()
                };
                let action = if let Some(filter) = &tree.compaction_filter {
                    (filter.filter)(key, &value)
                } else {
                    CompactionAction::Keep
                };
                match action {
                    CompactionAction::Keep => {
                        e.insert(value);
                    }
                    CompactionAction::Remove => (),
                    CompactionAction::Replace(value) => {
                        e.insert(value);
                    }
                }
            }
            Entry::Occupied(mut e) => {
                let value = if let Some(merge) = &tree.merge_operator {
                    (merge.full)(key, Some(&e.get()), once(value))
                } else {
                    value.into()
                };
                let action = if let Some(filter) = &tree.compaction_filter {
                    (filter.filter)(key, &value)
                } else {
                    CompactionAction::Keep
                };
                match action {
                    CompactionAction::Keep => e.insert(value),
                    CompactionAction::Remove => e.remove(),
                    CompactionAction::Replace(value) => e.insert(value),
                };
            }
        }
        Ok(())
    }

    pub fn iter(&self, column_family: &ColumnFamily) -> Iter {
        self.scan_prefix(column_family, &[])
    }

    pub fn scan_prefix(&self, column_family: &ColumnFamily, prefix: &[u8]) -> Iter {
        let trees = self.0.read().unwrap();
        let tree = &trees.get(column_family).unwrap().tree;
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
        Ok(self
            .0
            .read()
            .unwrap()
            .get(column_family)
            .unwrap()
            .tree
            .len())
    }

    pub fn is_empty(&self, column_family: &ColumnFamily) -> Result<bool> {
        Ok(self
            .0
            .read()
            .unwrap()
            .get(column_family)
            .unwrap()
            .tree
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

pub struct MergeOperator {
    pub full: fn(&[u8], Option<&[u8]>, SlicesIterator<'_>) -> Vec<u8>,
    pub partial: fn(&[u8], SlicesIterator<'_>) -> Vec<u8>,
    pub name: CString,
}

pub type SlicesIterator<'a> = Once<&'a [u8]>;

pub struct CompactionFilter {
    pub filter: fn(&[u8], &[u8]) -> CompactionAction,
    pub name: CString,
}

#[allow(dead_code)]
pub enum CompactionAction {
    Keep,
    Remove,
    Replace(Vec<u8>),
}
