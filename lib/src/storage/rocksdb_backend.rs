//! Code inspired by [https://github.com/rust-rocksdb/rust-rocksdb][Rust RocksDB] under Apache License 2.0.

#![allow(unsafe_code)]

use crate::error::invalid_input_error;
use libc::{self, c_char, c_void};
use librocksdb_sys::*;
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
    db: *mut rocksdb_t,
    options: *mut rocksdb_options_t,
    env: Option<*mut rocksdb_env_t>,
    column_families: Vec<&'static str>,
    cf_handles: Vec<*mut rocksdb_column_family_handle_t>,
}

impl Drop for DbHandler {
    fn drop(&mut self) {
        unsafe {
            rocksdb_close(self.db);
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
            let env = if in_memory {
                let env = rocksdb_create_mem_env();
                if env.is_null() {
                    rocksdb_options_destroy(options);
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
            let cf_options = column_families
                .iter()
                .map(|_| {
                    let options: *const rocksdb_options_t = rocksdb_options_create();
                    assert!(!options.is_null(), "rocksdb_options_create returned null");
                    options
                })
                .collect::<Vec<_>>();

            let mut cf_handles: Vec<*mut rocksdb_column_family_handle_t> =
                vec![ptr::null_mut(); column_families.len()];
            let db = ffi_result!(rocksdb_open_column_families(
                options,
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
                if let Some(env) = env {
                    rocksdb_env_destroy(env);
                }
                e
            })?;
            assert!(!db.is_null(), "rocksdb_create returned null");
            for handle in &cf_handles {
                if handle.is_null() {
                    rocksdb_close(db);
                    rocksdb_options_destroy(options);
                    if let Some(env) = env {
                        rocksdb_env_destroy(env);
                    }
                    return Err(other_error(
                        "Received null column family handle from RocksDB.",
                    ));
                }
            }

            Ok(DbHandler {
                db,
                options,
                env,
                column_families,
                cf_handles,
            })
        }
    }

    pub fn open_tree(&self, name: &'static str) -> Result<Tree> {
        for (cf_name, cf_handle) in self.0.column_families.iter().zip(&self.0.cf_handles) {
            if *cf_name == name {
                return Ok(Tree {
                    db: self.0.clone(),
                    cf_handle: *cf_handle,
                });
            }
        }
        Err(other_error(format!(
            "The column family {} does not exist",
            name
        )))
    }

    pub fn flush(&self) -> Result<()> {
        unsafe {
            let options = rocksdb_flushoptions_create();
            assert!(
                !options.is_null(),
                "rocksdb_flushoptions_create returned null"
            );
            let r = ffi_result!(rocksdb_flush(self.0.db, options));
            rocksdb_flushoptions_destroy(options);
            r
        }
    }
}

#[derive(Clone)]
pub struct Tree {
    db: Arc<DbHandler>,
    cf_handle: *mut rocksdb_column_family_handle_t,
}

unsafe impl Send for Tree {}
unsafe impl Sync for Tree {}

impl Tree {
    pub fn get(&self, key: &[u8]) -> Result<Option<PinnableSlice<'_>>> {
        unsafe {
            let options = rocksdb_readoptions_create();
            assert!(
                !options.is_null(),
                "rocksdb_readoptions_create returned null"
            );
            let r = ffi_result!(rocksdb_get_pinned_cf(
                self.db.db,
                options,
                self.cf_handle,
                key.as_ptr() as *const c_char,
                key.len()
            ));
            rocksdb_readoptions_destroy(options);
            let slice = r?;
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

    pub fn contains_key(&self, key: &[u8]) -> Result<bool> {
        Ok(self.get(key)?.is_some()) //TODO: optimize
    }

    pub fn insert(&self, key: &[u8], value: &[u8]) -> Result<()> {
        unsafe {
            let options = rocksdb_writeoptions_create();
            assert!(
                !options.is_null(),
                "rocksdb_writeoptions_create returned null"
            );
            let r = ffi_result!(rocksdb_put_cf(
                self.db.db,
                options,
                self.cf_handle,
                key.as_ptr() as *const c_char,
                key.len(),
                value.as_ptr() as *const c_char,
                value.len(),
            ));
            rocksdb_writeoptions_destroy(options);
            r
        }
    }

    pub fn insert_empty(&self, key: &[u8]) -> Result<()> {
        self.insert(key, &[])
    }

    pub fn remove(&self, key: &[u8]) -> Result<()> {
        unsafe {
            let options = rocksdb_writeoptions_create();
            assert!(
                !options.is_null(),
                "rocksdb_writeoptions_create returned null"
            );
            let r = ffi_result!(rocksdb_delete_cf(
                self.db.db,
                options,
                self.cf_handle,
                key.as_ptr() as *const c_char,
                key.len()
            ));
            rocksdb_writeoptions_destroy(options);
            r
        }
    }

    pub fn clear(&self) -> Result<()> {
        unsafe {
            let options = rocksdb_writeoptions_create();
            assert!(
                !options.is_null(),
                "rocksdb_writeoptions_create returned null"
            );
            let start = [];
            let end = [c_char::MAX; 257];
            let r = ffi_result!(rocksdb_delete_range_cf(
                self.db.db,
                options,
                self.cf_handle,
                start.as_ptr(),
                start.len(),
                end.as_ptr(),
                end.len(),
            ));
            rocksdb_writeoptions_destroy(options);
            r
        }
    }

    pub fn iter(&self) -> Iter {
        self.scan_prefix(&[])
    }

    pub fn scan_prefix(&self, prefix: &[u8]) -> Iter {
        unsafe {
            let options = rocksdb_readoptions_create();
            assert!(
                !options.is_null(),
                "rocksdb_readoptions_create returned null"
            );
            let iter = rocksdb_create_iterator_cf(self.db.db, options, self.cf_handle);
            assert!(!options.is_null(), "rocksdb_create_iterator returned null");
            if prefix.is_empty() {
                rocksdb_iter_seek_to_first(iter);
            } else {
                rocksdb_iter_seek(iter, prefix.as_ptr() as *const c_char, prefix.len());
            }
            Iter {
                iter,
                _options: options,
                prefix: prefix.to_vec(),
                _db: self.db.clone(),
            }
        }
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
        !self.iter().is_valid()
    }
}

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
    prefix: Vec<u8>,
    _db: Arc<DbHandler>, // needed to ensure that DB still lives while iter is used
    _options: *mut rocksdb_readoptions_t, // needed to ensure that options still lives while iter is used
}

unsafe impl Send for Iter {}
unsafe impl Sync for Iter {}

impl Iter {
    pub fn is_valid(&self) -> bool {
        unsafe {
            if rocksdb_iter_valid(self.iter) == 0 {
                return false;
            }
            let mut len = 0;
            let val = rocksdb_iter_key(self.iter, &mut len);
            slice::from_raw_parts(val as *const u8, len).starts_with(&self.prefix)
        }
    }

    pub fn status(&self) -> Result<()> {
        unsafe { ffi_result!(rocksdb_iter_get_error(self.iter)) }
    }

    pub fn next(&mut self) {
        unsafe {
            rocksdb_iter_next(self.iter);
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
