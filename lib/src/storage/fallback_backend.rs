use std::collections::btree_map::Entry;
use std::collections::BTreeMap;
use std::io::Result;
use std::path::Path;
use std::sync::{Arc, Mutex, RwLock};

#[derive(Clone)]
pub struct Db {
    trees: Arc<Mutex<BTreeMap<&'static str, Tree>>>,
    default: Tree,
}

impl Db {
    pub fn new() -> Result<Self> {
        Ok(Self {
            trees: Arc::default(),
            default: Tree::new(),
        })
    }

    pub fn open_tree(&self, name: &'static str) -> Result<Tree> {
        Ok(self
            .trees
            .lock()
            .unwrap()
            .entry(name)
            .or_insert_with(Tree::new)
            .clone())
    }

    pub fn flush(&self) -> Result<()> {
        Ok(())
    }

    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        self.default.get(key)
    }

    pub fn insert(&self, key: &[u8], value: impl Into<Vec<u8>>) -> Result<bool> {
        self.default.insert(key.into(), value)
    }
}
#[derive(Clone)]
pub struct Tree {
    tree: Arc<RwLock<BTreeMap<Vec<u8>, Vec<u8>>>>,
    merge_operator: Arc<dyn Fn(&[u8], Option<&[u8]>, &[u8]) -> Option<Vec<u8>> + 'static>,
}

impl Tree {
    fn new() -> Self {
        Self {
            tree: Arc::default(),
            merge_operator: Arc::new(|_, _, v| Some(v.into())),
        }
    }
    pub fn get(&self, key: &[u8]) -> Result<Option<Vec<u8>>> {
        Ok(
            self.tree.read().unwrap().get(key).map(|v| v.clone()), //TODO: avoid clone
        )
    }

    pub fn contains_key(&self, key: &[u8]) -> Result<bool> {
        Ok(self.tree.read().unwrap().contains_key(key.as_ref()))
    }

    pub fn insert(&self, key: &[u8], value: impl Into<Vec<u8>>) -> Result<bool> {
        Ok(self
            .tree
            .write()
            .unwrap()
            .insert(key.into(), value.into())
            .is_none())
    }

    pub fn insert_empty(&self, key: &[u8]) -> Result<bool> {
        self.insert(key, [])
    }

    pub fn merge(&self, key: &[u8], value: &[u8]) -> Result<()> {
        match self.tree.write().unwrap().entry(key.into()) {
            Entry::Occupied(e) => match (self.merge_operator)(key.as_ref(), Some(e.get()), value) {
                Some(v) => {
                    *e.into_mut() = v;
                }
                None => {
                    e.remove();
                }
            },
            Entry::Vacant(e) => {
                if let Some(v) = (self.merge_operator)(key.as_ref(), None, value) {
                    e.insert(v);
                }
            }
        }
        Ok(())
    }

    pub fn remove(&self, key: &[u8]) -> Result<bool> {
        Ok(self.tree.write().unwrap().remove(key.as_ref()).is_some())
    }

    pub fn update_and_fetch<V: Into<Vec<u8>>>(
        &self,
        key: &[u8],
        mut f: impl FnMut(Option<&[u8]>) -> Option<V>,
    ) -> Result<Option<Vec<u8>>> {
        Ok(match self.tree.write().unwrap().entry(key.into()) {
            Entry::Occupied(e) => match f(Some(e.get())) {
                Some(v) => {
                    let v = v.into();
                    let e_mut = e.into_mut();
                    e_mut.clear();
                    e_mut.extend_from_slice(&v);
                    Some(v)
                }
                None => {
                    e.remove();
                    None
                }
            },
            Entry::Vacant(e) => match f(None) {
                Some(v) => {
                    let v = v.into();
                    e.insert(v.clone());
                    Some(v)
                }
                None => None,
            },
        })
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
            tree.iter()
                .map(|(k, v)| Ok((k.clone(), v.clone())))
                .collect()
        } else {
            tree.range(prefix.to_vec()..)
                .take_while(|(k, _)| k.starts_with(prefix))
                .map(|(k, v)| Ok((k.clone(), v.clone())))
                .collect()
        };
        data.into_iter()
    }

    pub fn len(&self) -> usize {
        self.tree.read().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.tree.read().unwrap().is_empty()
    }

    pub fn flush(&self) -> Result<()> {
        Ok(())
    }

    pub fn set_merge_operator(
        &mut self,
        merge_operator: impl Fn(&[u8], Option<&[u8]>, &[u8]) -> Option<Vec<u8>> + 'static,
    ) {
        self.merge_operator = Arc::new(merge_operator)
    }
}

pub type Iter = std::vec::IntoIter<Result<(Vec<u8>, Vec<u8>)>>;
