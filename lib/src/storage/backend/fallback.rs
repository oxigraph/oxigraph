//! TODO: This storage is dramatically naive.

use crate::error::invalid_input_error;
use crate::storage::backend::{CompactionAction, CompactionFilter};
use std::collections::{hash_map, BTreeMap, HashMap, HashSet};
use std::ffi::CString;
use std::io::{Error, Result};
use std::sync::{Arc, RwLock};

pub struct ColumnFamilyDefinition {
    pub name: &'static str,
    pub merge_operator: Option<MergeOperator>,
    pub compaction_filter: Option<CompactionFilter>,
    pub use_iter: bool,
    pub min_prefix_size: usize,
}

#[derive(Clone)]
pub struct Db(Arc<DbInternals>);

#[derive(Default)]
struct DbInternals {
    trees: RwLock<HashMap<ColumnFamily, BTreeMap<Vec<u8>, Vec<u8>>>>,
    merge_operators: HashMap<ColumnFamily, MergeOperator>,
    compaction_filters: HashMap<ColumnFamily, CompactionFilter>,
}

impl Db {
    pub fn new(column_families: Vec<ColumnFamilyDefinition>) -> Result<Self> {
        let mut trees = HashMap::new();
        let mut merge_operators = HashMap::new();
        let mut compaction_filters = HashMap::new();
        for cf in column_families {
            let name = ColumnFamily(cf.name);
            trees.insert(name.clone(), BTreeMap::default());
            if let Some(me) = cf.merge_operator {
                merge_operators.insert(name.clone(), me);
            }
            if let Some(cf) = cf.compaction_filter {
                compaction_filters.insert(name.clone(), cf);
            }
        }
        trees.entry(ColumnFamily("default")).or_default(); // We make sure that "default" key exists.
        Ok(Self(Arc::new(DbInternals {
            trees: RwLock::new(trees),
            merge_operators,
            compaction_filters,
        })))
    }

    pub fn column_family(&self, name: &'static str) -> Option<ColumnFamily> {
        let name = ColumnFamily(name);
        if self.0.trees.read().unwrap().contains_key(&name) {
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
            .trees
            .read()
            .unwrap()
            .get(column_family)
            .and_then(|cf| cf.get(key).cloned()))
    }

    pub fn contains_key(&self, column_family: &ColumnFamily, key: &[u8]) -> Result<bool> {
        Ok(self
            .0
            .trees
            .read()
            .unwrap()
            .get(column_family)
            .map_or(false, |cf| cf.contains_key(key)))
    }

    pub fn new_batch(&self) -> WriteBatchWithIndex {
        WriteBatchWithIndex {
            by_cf: HashMap::new(),
            db: self.clone(),
            error: None,
        }
    }

    pub fn write(&self, batch: &mut WriteBatchWithIndex) -> Result<()> {
        if let Some(error) = batch.error.take() {
            return Err(error);
        }
        let mut trees = self.0.trees.write().unwrap();
        for (cf, ops) in batch.by_cf.drain() {
            let tree = trees.get_mut(&cf).ok_or_else(|| {
                invalid_input_error(format!("Unsupported column family {}", cf.0))
            })?;
            for k in ops.to_remove {
                tree.remove(&k);
            }
            for (k, v) in ops.to_insert {
                tree.insert(k, v);
            }
            for (k, v) in ops.to_merge {
                let v = self.exec_merge(&cf, &k, tree.get(&k).map(|v| v.as_slice()), &v)?;
                tree.insert(k, v);
            }
        }
        Ok(())
    }

    /*pub fn insert(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
        value: &[u8],
        _low_priority: bool,
    ) -> Result<()> {
        let mut db = self.0.write().unwrap();
        let tree = db.get_mut(column_family).unwrap();
        if let Some(value) = Self::exec_filter(tree, key, value.into()) {
            tree.tree.insert(key.into(), value.into())
        } else {
            tree.tree.remove(key)
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
                if let Some(value) =
                    Self::exec_filter(tree, key, Self::exec_merge(tree, key, None, value))
                {
                    e.insert(value);
                }
            }
            Entry::Occupied(mut e) => {
                if let Some(value) =
                    Self::exec_filter(tree, key, Self::exec_merge(tree, key, None, value))
                {
                    e.insert(value);
                } else {
                    e.remove();
                }
            }
        }
        Ok(())
    }*/

    fn exec_merge(
        &self,
        cf: &ColumnFamily,
        key: &[u8],
        base: Option<&[u8]>,
        value: &[u8],
    ) -> Result<Vec<u8>> {
        let merge = self.0.merge_operators.get(cf).ok_or_else(|| {
            invalid_input_error(format!("The column family {} has no merge operator", cf.0))
        })?;
        Ok((merge.full)(key, base, vec![value].into_iter()))
    }

    fn exec_partial_merge(
        &self,
        cf: &ColumnFamily,
        key: &[u8],
        a: &[u8],
        b: &[u8],
    ) -> Result<Vec<u8>> {
        let merge = self.0.merge_operators.get(cf).ok_or_else(|| {
            invalid_input_error(format!("The column family {} has no merge operator", cf.0))
        })?;
        Ok((merge.partial)(key, vec![a, b].into_iter()))
    }

    fn exec_filter(&self, cf: &ColumnFamily, key: &[u8], value: Vec<u8>) -> Option<Vec<u8>> {
        let action = if let Some(filter) = self.0.compaction_filters.get(cf) {
            (filter.filter)(key, &value)
        } else {
            CompactionAction::Keep
        };
        match action {
            CompactionAction::Keep => Some(value),
            CompactionAction::Remove => None,
        }
    }

    pub fn iter(&self, column_family: &ColumnFamily) -> Iter {
        self.scan_prefix(column_family, &[])
    }

    pub fn scan_prefix(&self, column_family: &ColumnFamily, prefix: &[u8]) -> Iter {
        let trees = self.0.trees.read().unwrap();
        let tree = if let Some(tree) = trees.get(column_family) {
            tree
        } else {
            return Iter {
                iter: Vec::new().into_iter(),
                current: None,
            };
        };
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
            .trees
            .read()
            .unwrap()
            .get(column_family)
            .map_or(0, |tree| tree.len()))
    }

    pub fn is_empty(&self, column_family: &ColumnFamily) -> Result<bool> {
        Ok(self
            .0
            .trees
            .read()
            .unwrap()
            .get(column_family)
            .map_or(true, |tree| tree.is_empty()))
    }
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct ColumnFamily(&'static str);

pub struct WriteBatchWithIndex {
    by_cf: HashMap<ColumnFamily, WriteBatchWithIndexCF>,
    db: Db,
    error: Option<Error>,
}

#[derive(Default)]
struct WriteBatchWithIndexCF {
    // Evaluation order insert/remove then merge
    to_insert: HashMap<Vec<u8>, Vec<u8>>,
    to_merge: HashMap<Vec<u8>, Vec<u8>>,
    to_remove: HashSet<Vec<u8>>,
}

impl WriteBatchWithIndex {
    pub fn insert(&mut self, column_family: &ColumnFamily, key: &[u8], value: &[u8]) {
        let cf_state = self.by_cf.entry(column_family.clone()).or_default();
        cf_state.to_insert.insert(key.into(), value.into());
        cf_state.to_merge.remove(key);
        cf_state.to_remove.remove(key);
    }

    pub fn insert_empty(&mut self, column_family: &ColumnFamily, key: &[u8]) {
        self.insert(column_family, key, &[])
    }

    pub fn remove(&mut self, column_family: &ColumnFamily, key: &[u8]) {
        let cf_state = self.by_cf.entry(column_family.clone()).or_default();
        cf_state.to_insert.remove(key);
        cf_state.to_merge.remove(key);
        cf_state.to_remove.insert(key.into());
    }

    pub fn merge(&mut self, column_family: &ColumnFamily, key: &[u8], value: &[u8]) {
        let cf_state = self.by_cf.entry(column_family.clone()).or_default();
        match cf_state.to_merge.entry(key.into()) {
            hash_map::Entry::Vacant(e) => {
                e.insert(value.into());
            }
            hash_map::Entry::Occupied(mut e) => {
                match self
                    .db
                    .exec_partial_merge(column_family, key, e.get(), value)
                {
                    Ok(value) => {
                        e.insert(value);
                    }
                    Err(e) => self.error = Some(e),
                }
            }
        }
    }

    pub fn get(&self, column_family: &ColumnFamily, key: &[u8]) -> Result<Option<Vec<u8>>> {
        if let Some(cf_state) = self.by_cf.get(column_family) {
            let value = if cf_state.to_remove.contains(key) {
                None
            } else if let Some(value) = cf_state.to_insert.get(key) {
                Some(value.clone())
            } else {
                self.db.get(column_family, key)?
            };
            Ok(if let Some(merge) = cf_state.to_merge.get(key) {
                Some(
                    self.db
                        .exec_merge(column_family, key, value.as_deref(), merge)?,
                )
            } else {
                value
            }
            .and_then(|value| self.db.exec_filter(column_family, key, value)))
        } else {
            self.db.get(column_family, key)
        }
    }

    pub fn contains_key(&self, column_family: &ColumnFamily, key: &[u8]) -> Result<bool> {
        Ok(self.get(column_family, key)?.is_some()) //TODO: optimize
    }

    pub fn clear(&mut self) {
        self.by_cf.clear();
    }

    pub fn len(&self) -> usize {
        self.by_cf
            .values()
            .map(|v| v.to_insert.len() + v.to_remove.len() + v.to_merge.len())
            .sum()
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

pub struct MergeOperator {
    pub full: fn(&[u8], Option<&[u8]>, SlicesIterator<'_>) -> Vec<u8>,
    pub partial: fn(&[u8], SlicesIterator<'_>) -> Vec<u8>,
    pub name: CString,
}

pub type SlicesIterator<'a> = std::vec::IntoIter<&'a [u8]>;
