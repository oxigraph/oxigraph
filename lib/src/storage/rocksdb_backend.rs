//! Code inspired by [https://github.com/rust-rocksdb/rust-rocksdb][Rust RocksDB] under Apache License 2.0.

#![allow(unsafe_code)]

use crate::error::invalid_input_error;
use libc::{self, c_char, c_void};
use oxrocksdb_sys::*;
use std::borrow::Borrow;
use std::env::temp_dir;
use std::ffi::{CStr, CString};
use std::io::{Error, ErrorKind, Result};
use std::marker::PhantomData;
use std::ops::Deref;
use std::path::Path;
use std::sync::Arc;
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

#[derive(Clone)]
pub struct Db(Arc<DbHandler>);

unsafe impl Send for Db {}
unsafe impl Sync for Db {}

struct DbHandler {
    db: *mut rocksdb_transactiondb_t,
    options: *mut rocksdb_options_t,
    txn_options: *mut rocksdb_transactiondb_options_t,
    read_options: *mut rocksdb_readoptions_t,
    write_options: *mut rocksdb_writeoptions_t,
    flush_options: *mut rocksdb_flushoptions_t,
    env: Option<*mut rocksdb_env_t>,
    column_families: Vec<&'static str>,
    cf_handles: Vec<*mut rocksdb_column_family_handle_t>,
}

impl Drop for DbHandler {
    fn drop(&mut self) {
        unsafe {
            for cf_handle in &self.cf_handles {
                rocksdb_column_family_handle_destroy(*cf_handle);
            }
            rocksdb_transactiondb_close(self.db);
            rocksdb_readoptions_destroy(self.read_options);
            rocksdb_writeoptions_destroy(self.write_options);
            rocksdb_flushoptions_destroy(self.flush_options);
            rocksdb_transactiondb_options_destroy(self.txn_options);
            rocksdb_options_destroy(self.options);
            if let Some(env) = self.env {
                rocksdb_env_destroy(env);
            }
        }
    }
}

impl Db {
    pub fn new(column_families: &'static [&'static str]) -> Result<Self> {
        let path = if cfg!(target_os = "linux") {
            "/dev/shm/".into()
        } else {
            temp_dir()
        }
        .join("oxigraph-temp-rocksdb");
        Ok(Self(Arc::new(Self::do_open(&path, column_families, true)?)))
    }

    pub fn open(path: &Path, column_families: &'static [&'static str]) -> Result<Self> {
        Ok(Self(Arc::new(Self::do_open(path, column_families, false)?)))
    }

    fn do_open(
        path: &Path,
        column_families: &'static [&'static str],
        in_memory: bool,
    ) -> Result<DbHandler> {
        let c_path = CString::new(
            path.to_str()
                .ok_or_else(|| invalid_input_error("The DB path is not valid UTF-8"))?,
        )
        .map_err(invalid_input_error)?;

        unsafe {
            let options = rocksdb_options_create();
            assert!(!options.is_null(), "rocksdb_options_create returned null");
            rocksdb_options_set_create_if_missing(options, 1);
            rocksdb_options_set_create_missing_column_families(options, 1);

            let txn_options = rocksdb_transactiondb_options_create();
            assert!(
                !txn_options.is_null(),
                "rocksdb_transactiondb_options_create returned null"
            );

            let env = if in_memory {
                let env = rocksdb_create_mem_env();
                if env.is_null() {
                    rocksdb_options_destroy(options);
                    rocksdb_transactiondb_options_destroy(txn_options);
                    return Err(other_error("Not able to create an in-memory environment."));
                }
                rocksdb_options_set_env(options, env);
                Some(env)
            } else {
                None
            };

            let mut column_families = column_families.to_vec();
            if !column_families.contains(&"default") {
                column_families.push("default")
            }
            let c_column_families = column_families
                .iter()
                .map(|cf| CString::new(*cf))
                .collect::<std::result::Result<Vec<_>, _>>()
                .map_err(invalid_input_error)?;
            let cf_options: Vec<*const rocksdb_options_t> = vec![options; column_families.len()];

            let mut cf_handles: Vec<*mut rocksdb_column_family_handle_t> =
                vec![ptr::null_mut(); column_families.len()];
            let db = ffi_result!(rocksdb_transactiondb_open_column_families(
                options,
                txn_options,
                c_path.as_ptr(),
                column_families.len().try_into().unwrap(),
                c_column_families
                    .iter()
                    .map(|cf| cf.as_ptr())
                    .collect::<Vec<_>>()
                    .as_ptr(),
                cf_options.as_ptr(),
                cf_handles.as_mut_ptr(),
            ))
            .map_err(|e| {
                rocksdb_options_destroy(options);
                rocksdb_transactiondb_options_destroy(txn_options);
                if let Some(env) = env {
                    rocksdb_env_destroy(env);
                }
                e
            })?;
            assert!(!db.is_null(), "rocksdb_create returned null");
            for handle in &cf_handles {
                if handle.is_null() {
                    rocksdb_transactiondb_close(db);
                    rocksdb_options_destroy(options);
                    rocksdb_transactiondb_options_destroy(txn_options);
                    if let Some(env) = env {
                        rocksdb_env_destroy(env);
                    }
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
                !read_options.is_null(),
                "rocksdb_writeoptions_create returned null"
            );
            if in_memory {
                rocksdb_writeoptions_disable_WAL(write_options, 1); // No need for WAL
            }
            let flush_options = rocksdb_flushoptions_create();
            assert!(
                !options.is_null(),
                "rocksdb_flushoptions_create returned null"
            );

            Ok(DbHandler {
                db,
                options,
                txn_options,
                read_options,
                write_options,
                flush_options,
                env,
                column_families,
                cf_handles,
            })
        }
    }

    pub fn column_family(&self, name: &'static str) -> Option<ColumnFamily> {
        for (cf_name, cf_handle) in self.0.column_families.iter().zip(&self.0.cf_handles) {
            if *cf_name == name {
                return Some(ColumnFamily(*cf_handle));
            }
        }
        None
    }

    pub fn flush(&self) -> Result<()> {
        unsafe { ffi_result!(rocksdb_transactiondb_flush(self.0.db, self.0.flush_options)) }
    }

    pub fn get(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
    ) -> Result<Option<PinnableSlice<'_>>> {
        unsafe {
            let slice = ffi_result!(rocksdb_transactiondb_get_pinned_cf(
                self.0.db,
                self.0.read_options,
                column_family.0,
                key.as_ptr() as *const c_char,
                key.len()
            ))?;
            Ok(if slice.is_null() {
                None
            } else {
                Some(PinnableSlice {
                    slice,
                    lifetime: PhantomData::default(),
                })
            })
        }
    }

    pub fn contains_key(&self, column_family: &ColumnFamily, key: &[u8]) -> Result<bool> {
        Ok(self.get(column_family, key)?.is_some()) //TODO: optimize
    }

    pub fn insert(&self, column_family: &ColumnFamily, key: &[u8], value: &[u8]) -> Result<()> {
        unsafe {
            ffi_result!(rocksdb_transactiondb_put_cf(
                self.0.db,
                self.0.write_options,
                column_family.0,
                key.as_ptr() as *const c_char,
                key.len(),
                value.as_ptr() as *const c_char,
                value.len(),
            ))
        }
    }

    pub fn insert_empty(&self, column_family: &ColumnFamily, key: &[u8]) -> Result<()> {
        self.insert(column_family, key, &[])
    }

    pub fn remove(&self, column_family: &ColumnFamily, key: &[u8]) -> Result<()> {
        unsafe {
            ffi_result!(rocksdb_transactiondb_delete_cf(
                self.0.db,
                self.0.write_options,
                column_family.0,
                key.as_ptr() as *const c_char,
                key.len()
            ))
        }
    }

    pub fn iter(&self, column_family: &ColumnFamily) -> Iter {
        self.scan_prefix(column_family, &[])
    }

    pub fn scan_prefix(&self, column_family: &ColumnFamily, prefix: &[u8]) -> Iter {
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
            let options = rocksdb_readoptions_create();
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
            let iter =
                rocksdb_transactiondb_create_iterator_cf(self.0.db, options, column_family.0);
            assert!(!iter.is_null(), "rocksdb_create_iterator returned null");
            if prefix.is_empty() {
                rocksdb_iter_seek_to_first(iter);
            } else {
                rocksdb_iter_seek(iter, prefix.as_ptr() as *const c_char, prefix.len());
            }
            let is_currently_valid = rocksdb_iter_valid(iter) != 0;
            Iter {
                iter,
                options,
                _upper_bound: upper_bound,
                _db: self.0.clone(),
                is_currently_valid,
            }
        }
    }

    pub fn len(&self, column_family: &ColumnFamily) -> Result<usize> {
        let mut count = 0;
        let mut iter = self.iter(column_family);
        while iter.is_valid() {
            count += 1;
            iter.next();
        }
        iter.status()?; // We makes sure there is no read problem
        Ok(count)
    }

    pub fn is_empty(&self, column_family: &ColumnFamily) -> Result<bool> {
        let iter = self.iter(column_family);
        iter.status()?; // We makes sure there is no read problem
        Ok(!iter.is_valid())
    }
}

// It is fine to not keep a lifetime: there is no way to use this type without the database being still in scope.
// So, no use after free possible.
#[derive(Clone)]
pub struct ColumnFamily(*mut rocksdb_column_family_handle_t);

unsafe impl Send for ColumnFamily {}
unsafe impl Sync for ColumnFamily {}

pub struct PinnableSlice<'a> {
    slice: *mut rocksdb_pinnableslice_t,
    lifetime: PhantomData<&'a ()>,
}

impl<'a> Drop for PinnableSlice<'a> {
    fn drop(&mut self) {
        unsafe {
            rocksdb_pinnableslice_destroy(self.slice);
        }
    }
}

impl<'a> Deref for PinnableSlice<'a> {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        unsafe {
            let mut len = 0;
            let val = rocksdb_pinnableslice_value(self.slice, &mut len);
            slice::from_raw_parts(val as *const u8, len)
        }
    }
}

impl<'a> AsRef<[u8]> for PinnableSlice<'a> {
    fn as_ref(&self) -> &[u8] {
        &*self
    }
}

impl<'a> Borrow<[u8]> for PinnableSlice<'a> {
    fn borrow(&self) -> &[u8] {
        &*self
    }
}

pub struct Iter {
    iter: *mut rocksdb_iterator_t,
    is_currently_valid: bool,
    _upper_bound: Option<Vec<u8>>,
    _db: Arc<DbHandler>, // needed to ensure that DB still lives while iter is used
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

    pub fn value(&self) -> Option<&[u8]> {
        if self.is_valid() {
            unsafe {
                let mut len = 0;
                let val = rocksdb_iter_value(self.iter, &mut len);
                Some(slice::from_raw_parts(val as *const u8, len))
            }
        } else {
            None
        }
    }
}

fn convert_error(ptr: *const c_char) -> Error {
    let message = unsafe {
        let s = CStr::from_ptr(ptr).to_string_lossy().into_owned();
        libc::free(ptr as *mut c_void);
        s
    };
    other_error(message)
}

fn other_error(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Error {
    Error::new(ErrorKind::InvalidInput, error)
}
