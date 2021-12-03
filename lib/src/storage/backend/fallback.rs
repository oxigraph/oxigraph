//! TODO: This storage is dramatically naive.

use crate::error::invalid_input_error;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::io::Result;
use std::mem::transmute;
use std::rc::{Rc, Weak};
use std::sync::{Arc, RwLock, RwLockWriteGuard};

pub struct ColumnFamilyDefinition {
    pub name: &'static str,
    pub use_iter: bool,
    pub min_prefix_size: usize,
}

#[derive(Clone)]
pub struct Db(Arc<RwLock<HashMap<ColumnFamily, BTreeMap<Vec<u8>, Vec<u8>>>>>);

impl Db {
    pub fn new(column_families: Vec<ColumnFamilyDefinition>) -> Result<Self> {
        let mut trees = HashMap::new();
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

    #[must_use]
    pub fn snapshot(&self) -> Reader {
        Reader(InnerReader::Simple(self.0.clone()))
    }

    pub fn transaction<'a, 'b: 'a, T>(
        &'b self,
        f: impl Fn(Transaction<'a>) -> Result<T>,
    ) -> Result<T> {
        f(Transaction(Rc::new(RefCell::new(self.0.write().unwrap()))))
    }
}

#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash)]
pub struct ColumnFamily(&'static str);

pub struct Reader(InnerReader);

enum InnerReader {
    Simple(Arc<RwLock<HashMap<ColumnFamily, BTreeMap<Vec<u8>, Vec<u8>>>>>),
    Transaction(
        Weak<RefCell<RwLockWriteGuard<'static, HashMap<ColumnFamily, BTreeMap<Vec<u8>, Vec<u8>>>>>>,
    ),
}

impl Reader {
    pub fn get(&self, column_family: &ColumnFamily, key: &[u8]) -> Result<Option<Vec<u8>>> {
        match &self.0 {
            InnerReader::Simple(reader) => Ok(reader
                .read()
                .unwrap()
                .get(column_family)
                .and_then(|cf| cf.get(key).cloned())),
            InnerReader::Transaction(reader) => {
                if let Some(reader) = reader.upgrade() {
                    Ok((*reader)
                        .borrow()
                        .get(column_family)
                        .and_then(|cf| cf.get(key).cloned()))
                } else {
                    Err(invalid_input_error("The transaction is already ended"))
                }
            }
        }
    }

    pub fn contains_key(&self, column_family: &ColumnFamily, key: &[u8]) -> Result<bool> {
        match &self.0 {
            InnerReader::Simple(reader) => Ok(reader
                .read()
                .unwrap()
                .get(column_family)
                .map_or(false, |cf| cf.contains_key(key))),
            InnerReader::Transaction(reader) => {
                if let Some(reader) = reader.upgrade() {
                    Ok((*reader)
                        .borrow()
                        .get(column_family)
                        .map_or(false, |cf| cf.contains_key(key)))
                } else {
                    Err(invalid_input_error("The transaction is already ended"))
                }
            }
        }
    }

    pub fn iter(&self, column_family: &ColumnFamily) -> Result<Iter> {
        self.scan_prefix(column_family, &[])
    }

    pub fn scan_prefix(&self, column_family: &ColumnFamily, prefix: &[u8]) -> Result<Iter> {
        let data: Vec<_> = match &self.0 {
            InnerReader::Simple(reader) => {
                let trees = reader.read().unwrap();
                let tree = if let Some(tree) = trees.get(column_family) {
                    tree
                } else {
                    return Ok(Iter {
                        iter: Vec::new().into_iter(),
                        current: None,
                    });
                };
                if prefix.is_empty() {
                    tree.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
                } else {
                    tree.range(prefix.to_vec()..)
                        .take_while(|(k, _)| k.starts_with(prefix))
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect()
                }
            }
            InnerReader::Transaction(reader) => {
                if let Some(reader) = reader.upgrade() {
                    let trees = (*reader).borrow();
                    let tree = if let Some(tree) = trees.get(column_family) {
                        tree
                    } else {
                        return Ok(Iter {
                            iter: Vec::new().into_iter(),
                            current: None,
                        });
                    };
                    if prefix.is_empty() {
                        tree.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
                    } else {
                        tree.range(prefix.to_vec()..)
                            .take_while(|(k, _)| k.starts_with(prefix))
                            .map(|(k, v)| (k.clone(), v.clone()))
                            .collect()
                    }
                } else {
                    return Err(invalid_input_error("The transaction is already ended"));
                }
            }
        };
        let mut iter = data.into_iter();
        let current = iter.next();
        Ok(Iter { iter, current })
    }

    pub fn len(&self, column_family: &ColumnFamily) -> Result<usize> {
        match &self.0 {
            InnerReader::Simple(reader) => Ok(reader
                .read()
                .unwrap()
                .get(column_family)
                .map_or(0, |tree| tree.len())),
            InnerReader::Transaction(reader) => {
                if let Some(reader) = reader.upgrade() {
                    Ok((*reader)
                        .borrow()
                        .get(column_family)
                        .map_or(0, |tree| tree.len()))
                } else {
                    Err(invalid_input_error("The transaction is already ended"))
                }
            }
        }
    }

    pub fn is_empty(&self, column_family: &ColumnFamily) -> Result<bool> {
        match &self.0 {
            InnerReader::Simple(reader) => Ok(reader
                .read()
                .unwrap()
                .get(column_family)
                .map_or(true, |tree| tree.is_empty())),
            InnerReader::Transaction(reader) => {
                if let Some(reader) = reader.upgrade() {
                    Ok((*reader)
                        .borrow()
                        .get(column_family)
                        .map_or(true, |tree| tree.is_empty()))
                } else {
                    Err(invalid_input_error("The transaction is already ended"))
                }
            }
        }
    }
}

pub struct Transaction<'a>(
    Rc<RefCell<RwLockWriteGuard<'a, HashMap<ColumnFamily, BTreeMap<Vec<u8>, Vec<u8>>>>>>,
);

impl Transaction<'_> {
    #[allow(unsafe_code)]
    pub fn reader(&self) -> Reader {
        // This transmute is safe because we take a weak reference and the only Rc reference used is guarded by the lifetime.
        Reader(InnerReader::Transaction(Rc::downgrade(unsafe {
            transmute(&self.0)
        })))
    }

    pub fn contains_key_for_update(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
    ) -> Result<bool> {
        Ok((*self.0)
            .borrow()
            .get(column_family)
            .map_or(false, |cf| cf.contains_key(key)))
    }

    pub fn insert(&mut self, column_family: &ColumnFamily, key: &[u8], value: &[u8]) -> Result<()> {
        self.0
            .borrow_mut()
            .get_mut(column_family)
            .unwrap()
            .insert(key.into(), value.into());
        Ok(())
    }

    pub fn insert_empty(&mut self, column_family: &ColumnFamily, key: &[u8]) -> Result<()> {
        self.insert(column_family, key, &[])
    }

    pub fn remove(&mut self, column_family: &ColumnFamily, key: &[u8]) -> Result<()> {
        self.0
            .borrow_mut()
            .get_mut(column_family)
            .unwrap()
            .remove(key);
        Ok(())
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
