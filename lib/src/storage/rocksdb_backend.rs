use rocksdb::{ColumnFamily, DBPinnableSlice, DBRawIterator, Env, Error, Options, WriteBatch, DB};
use std::env::temp_dir;
use std::io::{self, Result};
use std::mem::transmute;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct Db(Arc<DB>);

impl Db {
    pub fn new(column_families: &[&str]) -> Result<Self> {
        //TODO: temp dir should not be useful
        let temp_dir = if cfg!(target_os = "linux") {
            "/dev/shm/oxigraph-rocksdb".into()
        } else {
            temp_dir().join("oxigraph-rocksdb-in-memory")
        };
        Ok(Self(Arc::new(Self::do_open(
            &temp_dir,
            column_families,
            true,
        )?)))
    }

    pub fn open(path: &Path, column_families: &[&str]) -> Result<Self> {
        Ok(Self(Arc::new(Self::do_open(path, column_families, false)?)))
    }

    fn do_open(path: &Path, column_families: &[&str], mem_env: bool) -> Result<DB> {
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        if mem_env {
            options.set_env(&Env::mem_env().map_err(map_err)?);
        }
        DB::open_cf(&options, path, column_families).map_err(map_err)
    }

    pub fn open_tree(&self, name: &'static str) -> Tree {
        Tree {
            db: self.0.clone(),
            cf_name: name,
        }
    }

    pub fn flush(&self) -> Result<()> {
        self.0.flush().map_err(map_err)
    }

    pub fn get(&self, key: &[u8]) -> Result<Option<DBPinnableSlice<'_>>> {
        self.0.get_pinned(key).map_err(map_err)
    }

    pub fn insert(&self, key: &[u8], value: &[u8]) -> Result<()> {
        self.0.put(key, value).map_err(map_err)
    }
}

#[derive(Clone)]
pub struct Tree {
    db: Arc<DB>,
    cf_name: &'static str,
}

impl Tree {
    pub fn get(&self, key: &[u8]) -> Result<Option<DBPinnableSlice<'_>>> {
        self.db.get_pinned_cf(self.get_cf(), key).map_err(map_err)
    }

    pub fn contains_key(&self, key: &[u8]) -> Result<bool> {
        Ok(self.get(key)?.is_some()) //TODO: optimize
    }

    pub fn insert(&self, key: &[u8], value: &[u8]) -> Result<()> {
        self.db.put_cf(self.get_cf(), key, value).map_err(map_err)
    }

    pub fn insert_empty(&self, key: &[u8]) -> Result<()> {
        self.insert(key, &[])
    }

    pub fn remove(&self, key: &[u8]) -> Result<()> {
        self.db.delete_cf(self.get_cf(), key).map_err(map_err)
    }

    pub fn clear(&self) -> Result<()> {
        let mut batch = WriteBatch::default();
        batch.delete_range_cf(self.get_cf(), [].as_ref(), [u8::MAX; 257].as_ref());
        self.db.write(batch).map_err(map_err)
    }

    pub fn iter(&self) -> Iter {
        self.scan_prefix(&[])
    }

    #[allow(unsafe_code)]
    pub fn scan_prefix(&self, prefix: &[u8]) -> Iter {
        let mut iter = self.db.raw_iterator_cf(self.get_cf());
        iter.seek(&prefix);
        // Safe because we clone the same DB from which we take an iterator
        unsafe { Iter::new(iter, self.db.clone(), prefix.into()) }
    }

    pub fn len(&self) -> usize {
        let mut count = 0;
        let mut iter = self.iter();
        while iter.is_valid() {
            count += 1;
            iter.next();
        }
        count
    }

    pub fn is_empty(&self) -> bool {
        self.iter().key().is_none()
    }

    #[allow(clippy::expect_used)]
    fn get_cf(&self) -> &ColumnFamily {
        self.db
            .cf_handle(self.cf_name)
            .expect("A column family that should exist in RocksDB does not exist")
    }
}

pub struct Iter {
    iter: DBRawIterator<'static>,
    prefix: Vec<u8>,
    _db: Arc<DB>, // needed to ensure that DB still lives while iter is used
}

impl Iter {
    /// Creates a static iterator from a non static one by keeping a ARC reference to the database
    /// Caller must ensure that the iterator belongs to the same database
    ///
    /// This unsafe method is required to get static iterators and ease the usage of the library.
    #[allow(unsafe_code, clippy::useless_transmute)]
    unsafe fn new(iter: DBRawIterator<'_>, db: Arc<DB>, prefix: Vec<u8>) -> Self {
        Self {
            iter: transmute(iter),
            prefix,
            _db: db,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.iter.valid()
    }

    pub fn key(&self) -> Option<&[u8]> {
        self.iter.key().filter(|k| k.starts_with(&self.prefix))
    }

    pub fn value(&self) -> Option<&[u8]> {
        self.iter.value()
    }

    pub fn next(&mut self) {
        self.iter.next()
    }
}

fn map_err(e: Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e)
}
