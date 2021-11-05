//! Code inspired by [https://github.com/rust-rocksdb/rust-rocksdb][Rust RocksDB] under Apache License 2.0.
//!
//! TODO: still has some memory leaks if the database opening fails

#![allow(unsafe_code)]

use crate::error::invalid_input_error;
use crate::storage::backend::{ColumnFamilyDefinition, CompactionAction, CompactionFilter};
use libc::{self, c_char, c_int, c_uchar, c_void, size_t};
use oxrocksdb_sys::*;
use std::borrow::Borrow;
use std::env::temp_dir;
use std::ffi::{CStr, CString};
use std::io::{Error, ErrorKind, Result};
use std::iter::Zip;
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
    read_options: *mut rocksdb_readoptions_t,
    write_options: *mut rocksdb_writeoptions_t,
    low_priority_write_options: *mut rocksdb_writeoptions_t,
    flush_options: *mut rocksdb_flushoptions_t,
    compaction_options: *mut rocksdb_compactoptions_t,
    env: Option<*mut rocksdb_env_t>,
    column_family_names: Vec<&'static str>,
    cf_handles: Vec<*mut rocksdb_column_family_handle_t>,
    cf_options: Vec<*mut rocksdb_options_t>,
    cf_compaction_filters: Vec<*mut rocksdb_compactionfilter_t>,
}

impl Drop for DbHandler {
    fn drop(&mut self) {
        unsafe {
            for cf_handle in &self.cf_handles {
                rocksdb_column_family_handle_destroy(*cf_handle);
            }
            rocksdb_close(self.db);
            for cf_option in &self.cf_options {
                rocksdb_options_destroy(*cf_option);
            }
            rocksdb_readoptions_destroy(self.read_options);
            rocksdb_writeoptions_destroy(self.write_options);
            rocksdb_writeoptions_destroy(self.low_priority_write_options);
            rocksdb_flushoptions_destroy(self.flush_options);
            rocksdb_compactoptions_destroy(self.compaction_options);
            rocksdb_options_destroy(self.options);
            if let Some(env) = self.env {
                rocksdb_env_destroy(env);
            }
            for cf_compact in &self.cf_compaction_filters {
                rocksdb_compactionfilter_destroy(*cf_compact);
            }
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
        .join("oxigraph-temp-rocksdb");
        Ok(Self(Arc::new(Self::do_open(
            &path,
            column_families,
            true,
            false,
        )?)))
    }

    pub fn open(
        path: &Path,
        column_families: Vec<ColumnFamilyDefinition>,
        for_bulk_load: bool,
    ) -> Result<Self> {
        Ok(Self(Arc::new(Self::do_open(
            path,
            column_families,
            false,
            for_bulk_load,
        )?)))
    }

    fn do_open(
        path: &Path,
        mut column_families: Vec<ColumnFamilyDefinition>,
        in_memory: bool,
        for_bulk_load: bool,
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
            rocksdb_options_optimize_level_style_compaction(options, 512 * 1024 * 1024);
            rocksdb_options_increase_parallelism(options, num_cpus::get().try_into().unwrap());
            rocksdb_options_set_info_log_level(options, 2); // We only log warnings
            rocksdb_options_set_max_log_file_size(options, 1024 * 1024); // Only 1MB log size
            rocksdb_options_set_recycle_log_file_num(options, 10); // We do not keep more than 10 log files
            rocksdb_options_set_max_successive_merges(options, 5); // merge are expensive, let's pay the price once
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
            if for_bulk_load {
                rocksdb_options_prepare_for_bulk_load(options);
            }

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

            if !column_families.iter().any(|c| c.name == "default") {
                column_families.push(ColumnFamilyDefinition {
                    name: "default",
                    merge_operator: None,
                    compaction_filter: None,
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
            let mut cf_compaction_filters = Vec::new();
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
                    if let Some(merge) = cf.merge_operator {
                        // mergeoperator delete is done automatically
                        let merge = rocksdb_mergeoperator_create(
                            Box::into_raw(Box::new(merge)) as *mut c_void,
                            Some(merge_destructor),
                            Some(merge_full),
                            Some(merge_partial),
                            Some(merge_delete_value),
                            Some(merge_name),
                        );
                        assert!(
                            !merge.is_null(),
                            "rocksdb_mergeoperator_create returned null"
                        );
                        rocksdb_options_set_merge_operator(options, merge);
                    }
                    if let Some(compact) = cf.compaction_filter {
                        let compact = rocksdb_compactionfilter_create(
                            Box::into_raw(Box::new(compact)) as *mut c_void,
                            Some(compactionfilter_destructor),
                            Some(compactionfilter_filter),
                            Some(compactionfilter_name),
                        );
                        assert!(
                            !compact.is_null(),
                            "rocksdb_compactionfilter_create returned null"
                        );
                        rocksdb_options_set_compaction_filter(options, compact);
                        cf_compaction_filters.push(compact);
                    }
                    options
                })
                .collect::<Vec<_>>();

            let mut cf_handles: Vec<*mut rocksdb_column_family_handle_t> =
                vec![ptr::null_mut(); column_family_names.len()];
            let db = ffi_result!(rocksdb_open_column_families(
                options,
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
                    rocksdb_close(db);
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

            let low_priority_write_options = rocksdb_writeoptions_create_copy(write_options);
            assert!(
                !low_priority_write_options.is_null(),
                "rocksdb_writeoptions_create_copy returned null"
            );
            rocksdb_writeoptions_set_low_pri(low_priority_write_options, 1);

            let flush_options = rocksdb_flushoptions_create();
            assert!(
                !flush_options.is_null(),
                "rocksdb_flushoptions_create returned null"
            );

            let compaction_options = rocksdb_compactoptions_create();
            assert!(
                !compaction_options.is_null(),
                "rocksdb_compactoptions_create returned null"
            );

            Ok(DbHandler {
                db,
                options,
                read_options,
                write_options,
                low_priority_write_options,
                flush_options,
                compaction_options,
                env,
                column_family_names,
                cf_handles,
                cf_options,
                cf_compaction_filters,
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

    pub fn flush(&self, column_family: &ColumnFamily) -> Result<()> {
        unsafe {
            ffi_result!(rocksdb_flush_cf(
                self.0.db,
                self.0.flush_options,
                column_family.0,
            ))
        }
    }

    #[allow(clippy::unnecessary_wraps)]
    pub fn compact(&self, column_family: &ColumnFamily) -> Result<()> {
        unsafe {
            rocksdb_compact_range_cf_opt(
                self.0.db,
                column_family.0,
                self.0.compaction_options,
                ptr::null(),
                0,
                ptr::null(),
                0,
            );
        }
        Ok(())
    }

    pub fn get(&self, column_family: &ColumnFamily, key: &[u8]) -> Result<Option<PinnableSlice>> {
        unsafe {
            let slice = ffi_result!(rocksdb_get_pinned_cf(
                self.0.db,
                self.0.read_options,
                column_family.0,
                key.as_ptr() as *const c_char,
                key.len()
            ))?;
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

    pub fn insert(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
        value: &[u8],
        low_priority: bool,
    ) -> Result<()> {
        unsafe {
            ffi_result!(rocksdb_put_cf(
                self.0.db,
                if low_priority {
                    self.0.low_priority_write_options
                } else {
                    self.0.write_options
                },
                column_family.0,
                key.as_ptr() as *const c_char,
                key.len(),
                value.as_ptr() as *const c_char,
                value.len(),
            ))
        }
    }

    pub fn merge(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
        value: &[u8],
        low_priority: bool,
    ) -> Result<()> {
        unsafe {
            ffi_result!(rocksdb_merge_cf(
                self.0.db,
                if low_priority {
                    self.0.low_priority_write_options
                } else {
                    self.0.write_options
                },
                column_family.0,
                key.as_ptr() as *const c_char,
                key.len(),
                value.as_ptr() as *const c_char,
                value.len(),
            ))
        }
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
        low_priority: bool,
    ) -> Result<()> {
        unsafe {
            ffi_result!(rocksdb_delete_cf(
                self.0.db,
                if low_priority {
                    self.0.low_priority_write_options
                } else {
                    self.0.write_options
                },
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
            let iter = rocksdb_create_iterator_cf(self.0.db, options, column_family.0);
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

pub struct MergeOperator {
    pub full: fn(&[u8], Option<&[u8]>, SlicesIterator<'_>) -> Vec<u8>,
    pub partial: fn(&[u8], SlicesIterator<'_>) -> Vec<u8>,
    pub name: CString,
}

unsafe extern "C" fn merge_destructor(operator: *mut c_void) {
    Box::from_raw(operator as *mut MergeOperator);
}

unsafe extern "C" fn merge_full(
    operator: *mut c_void,
    key: *const c_char,
    key_length: size_t,
    existing_value: *const c_char,
    existing_value_len: size_t,
    operands_list: *const *const c_char,
    operands_list_length: *const size_t,
    num_operands: c_int,
    success: *mut u8,
    new_value_length: *mut size_t,
) -> *mut c_char {
    let operator = &*(operator as *const MergeOperator);
    let result = (operator.full)(
        slice::from_raw_parts(key as *const u8, key_length),
        if existing_value.is_null() {
            None
        } else {
            Some(slice::from_raw_parts(
                existing_value as *const u8,
                existing_value_len,
            ))
        },
        SlicesIterator::new(operands_list, operands_list_length, num_operands),
    );
    *new_value_length = result.len();
    *success = 1_u8;
    Box::into_raw(result.into_boxed_slice()) as *mut c_char
}

pub unsafe extern "C" fn merge_partial(
    operator: *mut c_void,
    key: *const c_char,
    key_length: size_t,
    operands_list: *const *const c_char,
    operands_list_length: *const size_t,
    num_operands: c_int,
    success: *mut u8,
    new_value_length: *mut size_t,
) -> *mut c_char {
    let operator = &*(operator as *const MergeOperator);
    let result = (operator.partial)(
        slice::from_raw_parts(key as *const u8, key_length),
        SlicesIterator::new(operands_list, operands_list_length, num_operands),
    );
    *new_value_length = result.len();
    *success = 1_u8;
    Box::into_raw(result.into_boxed_slice()) as *mut c_char
}

unsafe extern "C" fn merge_delete_value(
    _operator: *mut c_void,
    value: *const c_char,
    value_length: size_t,
) {
    if !value.is_null() {
        Box::from_raw(slice::from_raw_parts_mut(value as *mut u8, value_length));
    }
}

unsafe extern "C" fn merge_name(operator: *mut c_void) -> *const c_char {
    let operator = &*(operator as *const MergeOperator);
    operator.name.as_ptr()
}

pub struct SlicesIterator<'a>(
    Zip<std::slice::Iter<'a, *const c_char>, std::slice::Iter<'a, size_t>>,
);

impl<'a> SlicesIterator<'a> {
    unsafe fn new(
        operands_list: *const *const c_char,
        operands_list_length: *const size_t,
        num_operands: c_int,
    ) -> Self {
        let num_operands = usize::try_from(num_operands).unwrap();
        Self(
            slice::from_raw_parts(operands_list, num_operands)
                .iter()
                .zip(slice::from_raw_parts(operands_list_length, num_operands)),
        )
    }
}

impl<'a> Iterator for SlicesIterator<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        let (slice, len) = self.0.next()?;
        Some(unsafe { slice::from_raw_parts(*slice as *const u8, *len) })
    }
}

unsafe extern "C" fn compactionfilter_destructor(filter: *mut c_void) {
    Box::from_raw(filter as *mut CompactionFilter);
}

unsafe extern "C" fn compactionfilter_filter(
    filter: *mut c_void,
    _level: c_int,
    key: *const c_char,
    key_length: size_t,
    existing_value: *const c_char,
    value_length: size_t,
    _new_value: *mut *mut c_char,
    _new_value_length: *mut size_t,
    _value_changed: *mut c_uchar,
) -> c_uchar {
    let filter = &*(filter as *const CompactionFilter);
    match (filter.filter)(
        slice::from_raw_parts(key as *const u8, key_length),
        slice::from_raw_parts(existing_value as *const u8, value_length),
    ) {
        CompactionAction::Keep => 0,
        CompactionAction::Remove => 1,
    }
}

unsafe extern "C" fn compactionfilter_name(filter: *mut c_void) -> *const c_char {
    let filter = &*(filter as *const CompactionFilter);
    filter.name.as_ptr()
}
