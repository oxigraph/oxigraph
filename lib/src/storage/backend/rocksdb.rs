//! Code inspired by [Rust RocksDB](https://github.com/rust-rocksdb/rust-rocksdb) under Apache License 2.0.
//!
//! TODO: still has some memory leaks if the database opening fails

#![allow(unsafe_code)]

use crate::error::invalid_input_error;
use lazy_static::lazy_static;
use libc::{self, c_char, c_void, free};
use oxrocksdb_sys::*;
use rand::random;
use std::borrow::Borrow;
use std::collections::HashMap;
use std::env::temp_dir;
use std::ffi::{CStr, CString};
use std::fs::remove_dir_all;
use std::io::{Error, ErrorKind, Result};
use std::marker::PhantomData;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::thread::yield_now;
use std::{ptr, slice};

macro_rules! ffi_result {
    ( $($function:ident)::*() ) => {
        ffi_result_impl!($($function)::*())
    };

    ( $($function:ident)::*( $arg1:expr $(, $arg:expr)* $(,)? ) ) => {
        ffi_result_impl!($($function)::*($arg1 $(, $arg)* ,))
    };
}

macro_rules! ffi_result_impl {
    ( $($function:ident)::*( $($arg:expr,)*) ) => {{
        let mut err: *mut ::libc::c_char = ::std::ptr::null_mut();
        let result = $($function)::*($($arg,)* &mut err);
        if err.is_null() {
            Ok(result)
        } else {
            Err(convert_error(err))
        }
    }}
}

lazy_static! {
    static ref ROCKSDB_ENV: UnsafeEnv = {
        unsafe {
            let env = rocksdb_create_default_env();
            assert!(!env.is_null(), "rocksdb_create_default_env returned null");
            UnsafeEnv(env)
        }
    };
    static ref ROCKSDB_MEM_ENV: UnsafeEnv = {
        unsafe {
            let env = rocksdb_create_mem_env();
            assert!(!env.is_null(), "rocksdb_create_mem_env returned null");
            UnsafeEnv(env)
        }
    };
}

pub struct ColumnFamilyDefinition {
    pub name: &'static str,
    pub use_iter: bool,
    pub min_prefix_size: usize,
}

#[derive(Clone)]
pub struct Db(Arc<DbHandler>);

unsafe impl Send for Db {}
unsafe impl Sync for Db {}

struct DbHandler {
    db: *mut rocksdb_transactiondb_t,
    options: *mut rocksdb_options_t,
    transaction_options: *mut rocksdb_transaction_options_t,
    transactiondb_options: *mut rocksdb_transactiondb_options_t,
    read_options: *mut rocksdb_readoptions_t,
    write_options: *mut rocksdb_writeoptions_t,
    flush_options: *mut rocksdb_flushoptions_t,
    env_options: *mut rocksdb_envoptions_t,
    ingest_external_file_options: *mut rocksdb_ingestexternalfileoptions_t,
    compaction_options: *mut rocksdb_compactoptions_t,
    block_based_table_options: *mut rocksdb_block_based_table_options_t,
    column_family_names: Vec<&'static str>,
    cf_handles: Vec<*mut rocksdb_column_family_handle_t>,
    cf_options: Vec<*mut rocksdb_options_t>,
    path: PathBuf,
    remove_path: bool,
}

impl Drop for DbHandler {
    fn drop(&mut self) {
        unsafe {
            for cf_handle in &self.cf_handles {
                rocksdb_column_family_handle_destroy(*cf_handle);
            }
            rocksdb_transactiondb_close(self.db);
            for cf_option in &self.cf_options {
                rocksdb_options_destroy(*cf_option);
            }
            rocksdb_readoptions_destroy(self.read_options);
            rocksdb_writeoptions_destroy(self.write_options);
            rocksdb_flushoptions_destroy(self.flush_options);
            rocksdb_envoptions_destroy(self.env_options);
            rocksdb_ingestexternalfileoptions_destroy(self.ingest_external_file_options);
            rocksdb_compactoptions_destroy(self.compaction_options);
            rocksdb_transaction_options_destroy(self.transaction_options);
            rocksdb_transactiondb_options_destroy(self.transactiondb_options);
            rocksdb_options_destroy(self.options);
            rocksdb_block_based_options_destroy(self.block_based_table_options);
        }
        if self.remove_path && self.path.exists() {
            remove_dir_all(&self.path).unwrap();
        }
    }
}

impl Db {
    pub fn new(column_families: Vec<ColumnFamilyDefinition>) -> Result<Self> {
        let path = if cfg!(target_os = "linux") {
            "/dev/shm/".into()
        } else {
            temp_dir()
        }
        .join(format!("oxigraph-rocksdb-{}", random::<u128>()));
        Ok(Self(Arc::new(Self::do_open(path, column_families, true)?)))
    }

    pub fn open(path: &Path, column_families: Vec<ColumnFamilyDefinition>) -> Result<Self> {
        Ok(Self(Arc::new(Self::do_open(
            path.to_owned(),
            column_families,
            false,
        )?)))
    }

    fn do_open(
        path: PathBuf,
        mut column_families: Vec<ColumnFamilyDefinition>,
        in_memory: bool,
    ) -> Result<DbHandler> {
        let c_path = path_to_cstring(&path)?;

        unsafe {
            let options = rocksdb_options_create();
            assert!(!options.is_null(), "rocksdb_options_create returned null");
            rocksdb_options_set_create_if_missing(options, 1);
            rocksdb_options_set_create_missing_column_families(options, 1);
            rocksdb_options_optimize_level_style_compaction(options, 512 * 1024 * 1024);
            rocksdb_options_increase_parallelism(options, num_cpus::get().try_into().unwrap());
            rocksdb_options_set_info_log_level(options, 2); // We only log warnings
            rocksdb_options_set_max_log_file_size(options, 1024 * 1024); // Only 1MB log size
            rocksdb_options_set_recycle_log_file_num(options, 10); // We do not keep more than 10 log files
            rocksdb_options_set_compression(
                options,
                if in_memory {
                    rocksdb_no_compression
                } else {
                    rocksdb_lz4_compression
                }
                .try_into()
                .unwrap(),
            );
            rocksdb_options_set_env(
                options,
                if in_memory {
                    ROCKSDB_MEM_ENV.0
                } else {
                    ROCKSDB_ENV.0
                },
            );
            let block_based_table_options = rocksdb_block_based_options_create();
            assert!(
                !block_based_table_options.is_null(),
                "rocksdb_block_based_options_create returned null"
            );
            rocksdb_block_based_options_set_format_version(block_based_table_options, 5);
            rocksdb_block_based_options_set_index_block_restart_interval(
                block_based_table_options,
                16,
            );
            rocksdb_options_set_block_based_table_factory(options, block_based_table_options);

            let transactiondb_options = rocksdb_transactiondb_options_create();
            assert!(
                !transactiondb_options.is_null(),
                "rocksdb_transactiondb_options_create returned null"
            );

            if !column_families.iter().any(|c| c.name == "default") {
                column_families.push(ColumnFamilyDefinition {
                    name: "default",
                    use_iter: true,
                    min_prefix_size: 0,
                })
            }
            let column_family_names = column_families.iter().map(|c| c.name).collect::<Vec<_>>();
            let c_column_families = column_family_names
                .iter()
                .map(|name| CString::new(*name))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(invalid_input_error)?;
            let cf_options = column_families
                .into_iter()
                .map(|cf| {
                    let options = rocksdb_options_create_copy(options);
                    if !cf.use_iter {
                        rocksdb_options_optimize_for_point_lookup(options, 128);
                    }
                    if cf.min_prefix_size > 0 {
                        rocksdb_options_set_prefix_extractor(
                            options,
                            rocksdb_slicetransform_create_fixed_prefix(cf.min_prefix_size),
                        );
                    }
                    options
                })
                .collect::<Vec<_>>();

            let mut cf_handles: Vec<*mut rocksdb_column_family_handle_t> =
                vec![ptr::null_mut(); column_family_names.len()];
            let db = ffi_result!(rocksdb_transactiondb_open_column_families(
                options,
                transactiondb_options,
                c_path.as_ptr(),
                c_column_families.len().try_into().unwrap(),
                c_column_families
                    .iter()
                    .map(|cf| cf.as_ptr())
                    .collect::<Vec<_>>()
                    .as_ptr(),
                cf_options.as_ptr() as *const *const rocksdb_options_t,
                cf_handles.as_mut_ptr(),
            ))?;
            assert!(!db.is_null(), "rocksdb_create returned null");
            for handle in &cf_handles {
                if handle.is_null() {
                    rocksdb_transactiondb_close(db);
                    return Err(other_error(
                        "Received null column family handle from RocksDB.",
                    ));
                }
            }

            let read_options = rocksdb_readoptions_create();
            assert!(
                !read_options.is_null(),
                "rocksdb_readoptions_create returned null"
            );

            let write_options = rocksdb_writeoptions_create();
            assert!(
                !write_options.is_null(),
                "rocksdb_writeoptions_create returned null"
            );
            if in_memory {
                rocksdb_writeoptions_disable_WAL(write_options, 1); // No need for WAL
            }

            let flush_options = rocksdb_flushoptions_create();
            assert!(
                !flush_options.is_null(),
                "rocksdb_flushoptions_create returned null"
            );

            let env_options = rocksdb_envoptions_create();
            assert!(
                !env_options.is_null(),
                "rocksdb_envoptions_create returned null"
            );

            let ingest_external_file_options = rocksdb_ingestexternalfileoptions_create();
            assert!(
                !ingest_external_file_options.is_null(),
                "rocksdb_ingestexternalfileoptions_create returned null"
            );

            let compaction_options = rocksdb_compactoptions_create();
            assert!(
                !compaction_options.is_null(),
                "rocksdb_compactoptions_create returned null"
            );

            let transaction_options = rocksdb_transaction_options_create();
            assert!(
                !transaction_options.is_null(),
                "rocksdb_transaction_options_create returned null"
            );
            rocksdb_transaction_options_set_set_snapshot(transaction_options, 1);

            Ok(DbHandler {
                db,
                options,
                transaction_options,
                transactiondb_options,
                read_options,
                write_options,
                flush_options,
                env_options,
                ingest_external_file_options,
                compaction_options,
                block_based_table_options,
                column_family_names,
                cf_handles,
                cf_options,
                path,
                remove_path: in_memory,
            })
        }
    }

    pub fn column_family(&self, name: &'static str) -> Option<ColumnFamily> {
        for (cf, cf_handle) in self.0.column_family_names.iter().zip(&self.0.cf_handles) {
            if *cf == name {
                return Some(ColumnFamily(*cf_handle));
            }
        }
        None
    }

    #[must_use]
    pub fn snapshot(&self) -> Reader {
        unsafe {
            let snapshot = rocksdb_transactiondb_create_snapshot(self.0.db);
            assert!(
                !snapshot.is_null(),
                "rocksdb_transactiondb_create_snapshot returned null"
            );
            let options = rocksdb_readoptions_create_copy(self.0.read_options);
            rocksdb_readoptions_set_snapshot(options, snapshot);
            Reader {
                inner: InnerReader::Snapshot(Rc::new(InnerSnapshot {
                    db: self.0.clone(),
                    snapshot,
                })),
                options,
            }
        }
    }

    pub fn transaction<'a, 'b: 'a, T>(
        &'b self,
        f: impl Fn(Transaction<'a>) -> Result<T>,
    ) -> Result<T> {
        loop {
            let transaction = unsafe {
                let transaction = rocksdb_transaction_begin(
                    self.0.db,
                    self.0.write_options,
                    self.0.transaction_options,
                    ptr::null_mut(),
                );
                assert!(
                    !transaction.is_null(),
                    "rocksdb_transaction_begin returned null"
                );
                transaction
            };
            let read_options = unsafe {
                let options = rocksdb_readoptions_create_copy(self.0.read_options);
                rocksdb_readoptions_set_snapshot(
                    options,
                    rocksdb_transaction_get_snapshot(transaction),
                );
                options
            };
            let result = f(Transaction {
                transaction: Rc::new(transaction),
                read_options,
                _lifetime: PhantomData::default(),
            });
            match result {
                Ok(result) => {
                    unsafe {
                        ffi_result!(rocksdb_transaction_commit(transaction))?;
                        rocksdb_transaction_destroy(transaction);
                        rocksdb_readoptions_destroy(read_options);
                    }
                    return Ok(result);
                }
                Err(e) => {
                    unsafe {
                        ffi_result!(rocksdb_transaction_rollback(transaction))?;
                        rocksdb_transaction_destroy(transaction);
                        rocksdb_readoptions_destroy(read_options);
                    }
                    let is_conflict_error = e.get_ref().map_or(false, |e| {
                        let msg = e.to_string();
                        msg == "Resource busy: "
                            || msg == "Operation timed out: Timeout waiting to lock key"
                    });
                    if is_conflict_error {
                        // We give a chance to the OS to do something else before retrying in order to help avoiding an other conflict
                        yield_now();
                    } else {
                        // We raise the error
                        return Err(e);
                    }
                }
            }
        }
    }

    pub fn flush(&self, column_family: &ColumnFamily) -> Result<()> {
        unsafe {
            ffi_result!(rocksdb_transactiondb_flush_cf(
                self.0.db,
                self.0.flush_options,
                column_family.0,
            ))
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    pub fn compact(&self, column_family: &ColumnFamily) -> Result<()> {
        unsafe {
            ffi_result!(rocksdb_transactiondb_compact_range_cf_opt(
                self.0.db,
                column_family.0,
                self.0.compaction_options,
                ptr::null(),
                0,
                ptr::null(),
                0,
            ))
        }
    }

    pub fn new_sst_file(&self) -> Result<SstFileWriter> {
        unsafe {
            let path = self.0.path.join(random::<u128>().to_string());
            let writer = rocksdb_sstfilewriter_create(self.0.env_options, self.0.options);
            ffi_result!(rocksdb_sstfilewriter_open(
                writer,
                path_to_cstring(&path)?.as_ptr()
            ))?;
            Ok(SstFileWriter { writer, path })
        }
    }

    pub fn insert_stt_files(&self, ssts_for_cf: &[(&ColumnFamily, PathBuf)]) -> Result<()> {
        let mut paths_by_cf = HashMap::<_, Vec<_>>::new();
        for (cf, path) in ssts_for_cf {
            paths_by_cf
                .entry(*cf)
                .or_default()
                .push(path_to_cstring(path)?);
        }
        let cpaths_by_cf = paths_by_cf
            .iter()
            .map(|(cf, paths)| (*cf, paths.iter().map(|p| p.as_ptr()).collect::<Vec<_>>()))
            .collect::<Vec<_>>();
        let args = cpaths_by_cf
            .iter()
            .map(|(cf, p)| rocksdb_ingestexternalfilearg_t {
                column_family: cf.0,
                external_files: p.as_ptr(),
                external_files_len: p.len(),
                options: self.0.ingest_external_file_options,
            })
            .collect::<Vec<_>>();
        unsafe {
            ffi_result!(rocksdb_transactiondb_ingest_external_files(
                self.0.db,
                args.as_ptr(),
                args.len()
            ))
        }
    }
}

// It is fine to not keep a lifetime: there is no way to use this type without the database being still in scope.
// So, no use after free possible.
#[derive(Clone, Eq, PartialEq, Hash)]
pub struct ColumnFamily(*mut rocksdb_column_family_handle_t);

unsafe impl Send for ColumnFamily {}
unsafe impl Sync for ColumnFamily {}

pub struct Reader {
    inner: InnerReader,
    options: *mut rocksdb_readoptions_t,
}

#[derive(Clone)]
enum InnerReader {
    Snapshot(Rc<InnerSnapshot>),
    Transaction(Weak<*mut rocksdb_transaction_t>),
}

struct InnerSnapshot {
    db: Arc<DbHandler>,
    snapshot: *const rocksdb_snapshot_t,
}

impl Drop for InnerSnapshot {
    fn drop(&mut self) {
        unsafe { rocksdb_transactiondb_release_snapshot(self.db.db, self.snapshot) }
    }
}

impl Clone for Reader {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            options: unsafe { rocksdb_readoptions_create_copy(self.options) },
        }
    }
}

impl Drop for Reader {
    fn drop(&mut self) {
        unsafe { rocksdb_readoptions_destroy(self.options) }
    }
}

impl Reader {
    pub fn get(&self, column_family: &ColumnFamily, key: &[u8]) -> Result<Option<PinnableSlice>> {
        unsafe {
            let slice = match &self.inner {
                InnerReader::Snapshot(inner) => ffi_result!(rocksdb_transactiondb_get_pinned_cf(
                    inner.db.db,
                    self.options,
                    column_family.0,
                    key.as_ptr() as *const c_char,
                    key.len()
                )),
                InnerReader::Transaction(inner) => {
                    if let Some(inner) = inner.upgrade() {
                        ffi_result!(rocksdb_transaction_get_pinned_cf(
                            *inner,
                            self.options,
                            column_family.0,
                            key.as_ptr() as *const c_char,
                            key.len()
                        ))
                    } else {
                        return Err(invalid_input_error("The transaction is already ended"));
                    }
                }
            }?;
            Ok(if slice.is_null() {
                None
            } else {
                Some(PinnableSlice(slice))
            })
        }
    }

    pub fn contains_key(&self, column_family: &ColumnFamily, key: &[u8]) -> Result<bool> {
        Ok(self.get(column_family, key)?.is_some()) //TODO: optimize
    }

    pub fn iter(&self, column_family: &ColumnFamily) -> Result<Iter> {
        self.scan_prefix(column_family, &[])
    }

    pub fn scan_prefix(&self, column_family: &ColumnFamily, prefix: &[u8]) -> Result<Iter> {
        //We generate the upper bound
        let upper_bound = {
            let mut bound = prefix.to_vec();
            let mut found = false;
            for c in bound.iter_mut().rev() {
                if *c < u8::MAX {
                    *c += 1;
                    found = true;
                    break;
                }
            }
            if found {
                Some(bound)
            } else {
                None
            }
        };

        unsafe {
            let options = rocksdb_readoptions_create_copy(self.options);
            assert!(
                !options.is_null(),
                "rocksdb_readoptions_create returned null"
            );
            if let Some(upper_bound) = &upper_bound {
                rocksdb_readoptions_set_iterate_upper_bound(
                    options,
                    upper_bound.as_ptr() as *const c_char,
                    upper_bound.len(),
                );
            }
            let iter = match &self.inner {
                InnerReader::Snapshot(inner) => {
                    rocksdb_transactiondb_create_iterator_cf(inner.db.db, options, column_family.0)
                }
                InnerReader::Transaction(inner) => {
                    if let Some(inner) = inner.upgrade() {
                        rocksdb_transaction_create_iterator_cf(*inner, options, column_family.0)
                    } else {
                        return Err(invalid_input_error("The transaction is already ended"));
                    }
                }
            };
            assert!(!iter.is_null(), "rocksdb_create_iterator returned null");
            if prefix.is_empty() {
                rocksdb_iter_seek_to_first(iter);
            } else {
                rocksdb_iter_seek(iter, prefix.as_ptr() as *const c_char, prefix.len());
            }
            let is_currently_valid = rocksdb_iter_valid(iter) != 0;
            Ok(Iter {
                iter,
                options,
                _upper_bound: upper_bound,
                _reader: self.clone(),
                is_currently_valid,
            })
        }
    }

    pub fn len(&self, column_family: &ColumnFamily) -> Result<usize> {
        let mut count = 0;
        let mut iter = self.iter(column_family)?;
        while iter.is_valid() {
            count += 1;
            iter.next();
        }
        iter.status()?; // We makes sure there is no read problem
        Ok(count)
    }

    pub fn is_empty(&self, column_family: &ColumnFamily) -> Result<bool> {
        let iter = self.iter(column_family)?;
        iter.status()?; // We makes sure there is no read problem
        Ok(!iter.is_valid())
    }
}

pub struct Transaction<'a> {
    transaction: Rc<*mut rocksdb_transaction_t>,
    read_options: *mut rocksdb_readoptions_t,
    _lifetime: PhantomData<&'a ()>,
}

impl Transaction<'_> {
    pub fn reader(&self) -> Reader {
        Reader {
            inner: InnerReader::Transaction(Rc::downgrade(&self.transaction)),
            options: unsafe { rocksdb_readoptions_create_copy(self.read_options) },
        }
    }

    pub fn contains_key_for_update(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
    ) -> Result<bool> {
        unsafe {
            let slice = ffi_result!(rocksdb_transaction_get_for_update_pinned_cf(
                *self.transaction,
                self.read_options,
                column_family.0,
                key.as_ptr() as *const c_char,
                key.len()
            ))?;
            Ok(!slice.is_null())
        }
    }

    pub fn insert(&mut self, column_family: &ColumnFamily, key: &[u8], value: &[u8]) -> Result<()> {
        unsafe {
            ffi_result!(rocksdb_transaction_put_cf(
                *self.transaction,
                column_family.0,
                key.as_ptr() as *const c_char,
                key.len(),
                value.as_ptr() as *const c_char,
                value.len(),
            ))
        }
    }

    pub fn insert_empty(&mut self, column_family: &ColumnFamily, key: &[u8]) -> Result<()> {
        self.insert(column_family, key, &[])
    }

    pub fn remove(&mut self, column_family: &ColumnFamily, key: &[u8]) -> Result<()> {
        unsafe {
            ffi_result!(rocksdb_transaction_delete_cf(
                *self.transaction,
                column_family.0,
                key.as_ptr() as *const c_char,
                key.len(),
            ))
        }
    }
}

pub struct PinnableSlice(*mut rocksdb_pinnableslice_t);

impl Drop for PinnableSlice {
    fn drop(&mut self) {
        unsafe {
            rocksdb_pinnableslice_destroy(self.0);
        }
    }
}

impl Deref for PinnableSlice {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        unsafe {
            let mut len = 0;
            let val = rocksdb_pinnableslice_value(self.0, &mut len);
            slice::from_raw_parts(val as *const u8, len)
        }
    }
}

impl AsRef<[u8]> for PinnableSlice {
    fn as_ref(&self) -> &[u8] {
        &*self
    }
}

impl Borrow<[u8]> for PinnableSlice {
    fn borrow(&self) -> &[u8] {
        &*self
    }
}

impl From<PinnableSlice> for Vec<u8> {
    fn from(value: PinnableSlice) -> Self {
        value.to_vec()
    }
}

pub struct Buffer {
    base: *mut u8,
    len: usize,
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            free(self.base as *mut c_void);
        }
    }
}

impl Deref for Buffer {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.base, self.len) }
    }
}

impl AsRef<[u8]> for Buffer {
    fn as_ref(&self) -> &[u8] {
        &*self
    }
}

impl Borrow<[u8]> for Buffer {
    fn borrow(&self) -> &[u8] {
        &*self
    }
}

impl From<Buffer> for Vec<u8> {
    fn from(value: Buffer) -> Self {
        value.to_vec()
    }
}

pub struct Iter {
    iter: *mut rocksdb_iterator_t,
    is_currently_valid: bool,
    _upper_bound: Option<Vec<u8>>,
    _reader: Reader, // needed to ensure that DB still lives while iter is used
    options: *mut rocksdb_readoptions_t, // needed to ensure that options still lives while iter is used
}

impl Drop for Iter {
    fn drop(&mut self) {
        unsafe {
            rocksdb_iter_destroy(self.iter);
            rocksdb_readoptions_destroy(self.options);
        }
    }
}

unsafe impl Send for Iter {}
unsafe impl Sync for Iter {}

impl Iter {
    pub fn is_valid(&self) -> bool {
        self.is_currently_valid
    }

    pub fn status(&self) -> Result<()> {
        unsafe { ffi_result!(rocksdb_iter_get_error(self.iter)) }
    }

    pub fn next(&mut self) {
        unsafe {
            rocksdb_iter_next(self.iter);
            self.is_currently_valid = rocksdb_iter_valid(self.iter) != 0;
        }
    }

    pub fn key(&self) -> Option<&[u8]> {
        if self.is_valid() {
            unsafe {
                let mut len = 0;
                let val = rocksdb_iter_key(self.iter, &mut len);
                Some(slice::from_raw_parts(val as *const u8, len))
            }
        } else {
            None
        }
    }
}

pub struct SstFileWriter {
    writer: *mut rocksdb_sstfilewriter_t,
    path: PathBuf,
}

impl Drop for SstFileWriter {
    fn drop(&mut self) {
        unsafe {
            rocksdb_sstfilewriter_destroy(self.writer);
        }
    }
}

impl SstFileWriter {
    pub fn insert(&mut self, key: &[u8], value: &[u8]) -> Result<()> {
        unsafe {
            ffi_result!(rocksdb_sstfilewriter_put(
                self.writer,
                key.as_ptr() as *const c_char,
                key.len(),
                value.as_ptr() as *const c_char,
                value.len(),
            ))
        }
    }

    pub fn insert_empty(&mut self, key: &[u8]) -> Result<()> {
        self.insert(key, &[])
    }

    pub fn finish(self) -> Result<PathBuf> {
        unsafe {
            ffi_result!(rocksdb_sstfilewriter_finish(self.writer))?;
        }
        Ok(self.path.clone())
    }
}

fn convert_error(ptr: *const c_char) -> Error {
    let message = unsafe {
        let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
        free(ptr as *mut c_void);
        s
    };
    other_error(message)
}

fn other_error(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Error {
    Error::new(ErrorKind::Other, error)
}

struct UnsafeEnv(*mut rocksdb_env_t);

// Hack for lazy_static. OK because only written in lazy static and used in a thread-safe way by RocksDB
unsafe impl Sync for UnsafeEnv {}

fn path_to_cstring(path: &Path) -> Result<CString> {
    CString::new(
        path.to_str()
            .ok_or_else(|| invalid_input_error("The DB path is not valid UTF-8"))?,
    )
    .map_err(invalid_input_error)
}
