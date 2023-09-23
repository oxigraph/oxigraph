//! Code inspired by [Rust RocksDB](https://github.com/rust-rocksdb/rust-rocksdb) under Apache License 2.0.

#![allow(
    unsafe_code,
    trivial_casts,
    clippy::undocumented_unsafe_blocks,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result
)]

use crate::storage::error::{CorruptionError, StorageError};
use libc::{self, c_void, free};
use oxrocksdb_sys::*;
use rand::random;
use std::borrow::Borrow;
use std::cmp::min;
use std::collections::HashMap;
use std::env::temp_dir;
use std::error::Error;
use std::ffi::{CStr, CString};
use std::fmt;
use std::fs::remove_dir_all;
use std::io;
use std::marker::PhantomData;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::sync::{Arc, OnceLock};
use std::thread::{available_parallelism, yield_now};
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
        let mut status = rocksdb_status_t {
            code: rocksdb_status_code_t_rocksdb_status_code_ok,
            subcode: rocksdb_status_subcode_t_rocksdb_status_subcode_none,
            severity: rocksdb_status_severity_t_rocksdb_status_severity_none,
            string: ptr::null()
        };
        let result = $($function)::*($($arg,)* &mut status);
        if status.code == rocksdb_status_code_t_rocksdb_status_code_ok {
            Ok(result)
        } else {
            Err(ErrorStatus(status))
        }
    }}
}

pub struct ColumnFamilyDefinition {
    pub name: &'static str,
    pub use_iter: bool,
    pub min_prefix_size: usize,
    pub unordered_writes: bool,
}

#[derive(Clone)]
pub struct Db {
    inner: DbKind,
}

#[derive(Clone)]
enum DbKind {
    ReadOnly(Arc<RoDbHandler>),
    ReadWrite(Arc<RwDbHandler>),
}

struct RwDbHandler {
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
    in_memory: bool,
    path: PathBuf,
}

unsafe impl Send for RwDbHandler {}

unsafe impl Sync for RwDbHandler {}

impl Drop for RwDbHandler {
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
        if self.in_memory {
            #[allow(clippy::let_underscore_must_use)]
            let _: io::Result<()> = remove_dir_all(&self.path);
        }
    }
}

struct RoDbHandler {
    db: *mut rocksdb_t,
    options: *mut rocksdb_options_t,
    read_options: *mut rocksdb_readoptions_t,
    column_family_names: Vec<&'static str>,
    cf_handles: Vec<*mut rocksdb_column_family_handle_t>,
    cf_options: Vec<*mut rocksdb_options_t>,
    is_secondary: bool,
    path_to_remove: Option<PathBuf>,
}

unsafe impl Send for RoDbHandler {}

unsafe impl Sync for RoDbHandler {}

impl Drop for RoDbHandler {
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
            rocksdb_options_destroy(self.options);
        }
        if let Some(path) = &self.path_to_remove {
            #[allow(clippy::let_underscore_must_use)]
            let _: io::Result<()> = remove_dir_all(path);
        }
    }
}

impl Db {
    pub fn new(column_families: Vec<ColumnFamilyDefinition>) -> Result<Self, StorageError> {
        Self::open_read_write(None, column_families)
    }

    pub fn open_read_write(
        path: Option<&Path>,
        column_families: Vec<ColumnFamilyDefinition>,
    ) -> Result<Self, StorageError> {
        let (path, in_memory) = if let Some(path) = path {
            (path.to_path_buf(), false)
        } else {
            (tmp_path(), true)
        };
        let c_path = path_to_cstring(&path)?;
        unsafe {
            let options = Self::db_options(true, in_memory)?;
            rocksdb_options_set_create_if_missing(options, 1);
            rocksdb_options_set_create_missing_column_families(options, 1);
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
            #[cfg(feature = "rocksdb-debug")]
            {
                rocksdb_options_set_info_log_level(options, 0);
                rocksdb_options_enable_statistics(options);
                rocksdb_options_set_stats_dump_period_sec(options, 60);
            }

            let (column_family_names, c_column_family_names, cf_options) =
                Self::column_families_names_and_options(column_families, options);
            let mut cf_handles: Vec<*mut rocksdb_column_family_handle_t> =
                vec![ptr::null_mut(); column_family_names.len()];
            let c_num_column_families = c_column_family_names.len().try_into().unwrap();

            let transactiondb_options = rocksdb_transactiondb_options_create();
            assert!(
                !transactiondb_options.is_null(),
                "rocksdb_transactiondb_options_create returned null"
            );

            let db = ffi_result!(rocksdb_transactiondb_open_column_families_with_status(
                options,
                transactiondb_options,
                c_path.as_ptr(),
                c_num_column_families,
                c_column_family_names
                    .iter()
                    .map(|cf| cf.as_ptr())
                    .collect::<Vec<_>>()
                    .as_ptr(),
                cf_options.as_ptr().cast(),
                cf_handles.as_mut_ptr(),
            ))
            .map_err(|e| {
                rocksdb_transactiondb_options_destroy(transactiondb_options);
                for cf_option in &cf_options {
                    rocksdb_options_destroy(*cf_option);
                }
                rocksdb_options_destroy(options);
                rocksdb_block_based_options_destroy(block_based_table_options);
                e
            })?;
            assert!(!db.is_null(), "rocksdb_create returned null");
            for handle in &cf_handles {
                assert!(
                    !handle.is_null(),
                    "rocksdb_readoptions_create returned a null column family"
                );
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

            let transaction_options = rocksdb_transaction_options_create();
            assert!(
                !transaction_options.is_null(),
                "rocksdb_transaction_options_create returned null"
            );
            rocksdb_transaction_options_set_set_snapshot(transaction_options, 1);

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

            Ok(Self {
                inner: DbKind::ReadWrite(Arc::new(RwDbHandler {
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
                    in_memory,
                    path,
                })),
            })
        }
    }

    pub fn open_secondary(
        primary_path: &Path,
        secondary_path: Option<&Path>,
        column_families: Vec<ColumnFamilyDefinition>,
    ) -> Result<Self, StorageError> {
        let c_primary_path = path_to_cstring(primary_path)?;
        let (secondary_path, in_memory) = if let Some(path) = secondary_path {
            (path.to_path_buf(), false)
        } else {
            (tmp_path(), true)
        };
        let c_secondary_path = path_to_cstring(&secondary_path)?;
        unsafe {
            let options = Self::db_options(false, false)?;
            let (column_family_names, c_column_family_names, cf_options) =
                Self::column_families_names_and_options(column_families, options);
            let mut cf_handles: Vec<*mut rocksdb_column_family_handle_t> =
                vec![ptr::null_mut(); column_family_names.len()];
            let c_num_column_families = c_column_family_names.len().try_into().unwrap();
            let db = ffi_result!(rocksdb_open_as_secondary_column_families_with_status(
                options,
                c_primary_path.as_ptr(),
                c_secondary_path.as_ptr(),
                c_num_column_families,
                c_column_family_names
                    .iter()
                    .map(|cf| cf.as_ptr())
                    .collect::<Vec<_>>()
                    .as_ptr(),
                cf_options.as_ptr().cast(),
                cf_handles.as_mut_ptr(),
            ))
            .map_err(|e| {
                for cf_option in &cf_options {
                    rocksdb_options_destroy(*cf_option);
                }
                rocksdb_options_destroy(options);
                e
            })?;
            assert!(
                !db.is_null(),
                "rocksdb_open_for_read_only_column_families_with_status returned null"
            );
            for handle in &cf_handles {
                assert!(
                    !handle.is_null(),
                    "rocksdb_open_for_read_only_column_families_with_status returned a null column family"
                );
            }
            let read_options = rocksdb_readoptions_create();
            assert!(
                !read_options.is_null(),
                "rocksdb_readoptions_create returned null"
            );
            Ok(Self {
                inner: DbKind::ReadOnly(Arc::new(RoDbHandler {
                    db,
                    options,
                    read_options,
                    column_family_names,
                    cf_handles,
                    cf_options,
                    is_secondary: true,
                    path_to_remove: in_memory.then_some(secondary_path),
                })),
            })
        }
    }

    pub fn open_read_only(
        path: &Path,
        column_families: Vec<ColumnFamilyDefinition>,
    ) -> Result<Self, StorageError> {
        unsafe {
            let c_path = path_to_cstring(path)?;
            let options = Self::db_options(true, false)?;
            let (column_family_names, c_column_family_names, cf_options) =
                Self::column_families_names_and_options(column_families, options);
            let mut cf_handles: Vec<*mut rocksdb_column_family_handle_t> =
                vec![ptr::null_mut(); column_family_names.len()];
            let c_num_column_families = c_column_family_names.len().try_into().unwrap();
            let db = ffi_result!(rocksdb_open_for_read_only_column_families_with_status(
                options,
                c_path.as_ptr(),
                c_num_column_families,
                c_column_family_names
                    .iter()
                    .map(|cf| cf.as_ptr())
                    .collect::<Vec<_>>()
                    .as_ptr(),
                cf_options.as_ptr().cast(),
                cf_handles.as_mut_ptr(),
                0, // false
            ))
            .map_err(|e| {
                for cf_option in &cf_options {
                    rocksdb_options_destroy(*cf_option);
                }
                rocksdb_options_destroy(options);
                e
            })?;
            assert!(
                !db.is_null(),
                "rocksdb_open_for_read_only_column_families_with_status returned null"
            );
            for handle in &cf_handles {
                assert!(
                    !handle.is_null(),
                    "rocksdb_open_for_read_only_column_families_with_status returned a null column family"
                );
            }
            let read_options = rocksdb_readoptions_create();
            assert!(
                !read_options.is_null(),
                "rocksdb_readoptions_create returned null"
            );

            Ok(Self {
                inner: DbKind::ReadOnly(Arc::new(RoDbHandler {
                    db,
                    options,
                    read_options,
                    column_family_names,
                    cf_handles,
                    cf_options,
                    is_secondary: false,
                    path_to_remove: None,
                })),
            })
        }
    }

    fn db_options(
        limit_max_open_files: bool,
        in_memory: bool,
    ) -> Result<*mut rocksdb_options_t, StorageError> {
        static ROCKSDB_ENV: OnceLock<UnsafeEnv> = OnceLock::new();
        static ROCKSDB_MEM_ENV: OnceLock<UnsafeEnv> = OnceLock::new();

        unsafe {
            let options = rocksdb_options_create();
            assert!(!options.is_null(), "rocksdb_options_create returned null");
            rocksdb_options_optimize_level_style_compaction(options, 512 * 1024 * 1024);
            rocksdb_options_increase_parallelism(
                options,
                available_parallelism()?.get().try_into().unwrap(),
            );
            if limit_max_open_files {
                if let Some(available_fd) = available_file_descriptors()? {
                    if available_fd < 96 {
                        rocksdb_options_destroy(options);
                        return Err(io::Error::new(
                            io::ErrorKind::Other,
                            format!(
                                "Oxigraph needs at least 96 file descriptors, \
                                    only {available_fd} allowed. \
                                    Run e.g. `ulimit -n 512` to allow 512 opened files"
                            ),
                        )
                        .into());
                    }
                    rocksdb_options_set_max_open_files(
                        options,
                        (available_fd - 48).try_into().unwrap(),
                    )
                }
            } else {
                rocksdb_options_set_max_open_files(options, -1);
            }
            rocksdb_options_set_info_log_level(options, 2); // We only log warnings
            rocksdb_options_set_max_log_file_size(options, 1024 * 1024); // Only 1MB log size
            rocksdb_options_set_recycle_log_file_num(options, 10); // We do not keep more than 10 log files
            rocksdb_options_set_env(
                options,
                if in_memory {
                    ROCKSDB_MEM_ENV.get_or_init(|| {
                        let env = rocksdb_create_mem_env();
                        assert!(!env.is_null(), "rocksdb_create_mem_env returned null");
                        UnsafeEnv(env)
                    })
                } else {
                    ROCKSDB_ENV.get_or_init(|| {
                        let env = rocksdb_create_default_env();
                        assert!(!env.is_null(), "rocksdb_create_default_env returned null");
                        UnsafeEnv(env)
                    })
                }
                .0,
            );
            Ok(options)
        }
    }

    fn column_families_names_and_options(
        mut column_families: Vec<ColumnFamilyDefinition>,
        base_options: *mut rocksdb_options_t,
    ) -> (Vec<&'static str>, Vec<CString>, Vec<*mut rocksdb_options_t>) {
        if !column_families.iter().any(|c| c.name == "default") {
            column_families.push(ColumnFamilyDefinition {
                name: "default",
                use_iter: true,
                min_prefix_size: 0,
                unordered_writes: false,
            })
        }
        let column_family_names = column_families.iter().map(|c| c.name).collect::<Vec<_>>();
        let c_column_family_names = column_family_names
            .iter()
            .map(|name| CString::new(*name).unwrap())
            .collect();

        let cf_options = column_families
            .into_iter()
            .map(|cf| unsafe {
                let options = rocksdb_options_create_copy(base_options);
                if !cf.use_iter {
                    rocksdb_options_optimize_for_point_lookup(options, 128);
                }
                if cf.min_prefix_size > 0 {
                    rocksdb_options_set_prefix_extractor(
                        options,
                        rocksdb_slicetransform_create_fixed_prefix(cf.min_prefix_size),
                    );
                }
                if cf.unordered_writes {
                    rocksdb_options_set_unordered_write(options, 1);
                }
                options
            })
            .collect::<Vec<_>>();
        (column_family_names, c_column_family_names, cf_options)
    }

    pub fn column_family(&self, name: &'static str) -> Result<ColumnFamily, StorageError> {
        let (column_family_names, cf_handles) = match &self.inner {
            DbKind::ReadOnly(db) => (&db.column_family_names, &db.cf_handles),
            DbKind::ReadWrite(db) => (&db.column_family_names, &db.cf_handles),
        };
        for (cf, cf_handle) in column_family_names.iter().zip(cf_handles) {
            if *cf == name {
                return Ok(ColumnFamily(*cf_handle));
            }
        }
        Err(CorruptionError::msg(format!("Column family {name} does not exist")).into())
    }

    #[must_use]
    pub fn snapshot(&self) -> Reader {
        unsafe {
            match &self.inner {
                DbKind::ReadOnly(db) => {
                    if db.is_secondary {
                        // We try to refresh (and ignore the errors)
                        #[allow(clippy::let_underscore_must_use)]
                        let _: Result<(), ErrorStatus> =
                            ffi_result!(rocksdb_try_catch_up_with_primary_with_status(db.db));
                    }
                    let options = rocksdb_readoptions_create_copy(db.read_options);
                    Reader {
                        inner: InnerReader::PlainDb(Arc::clone(db)),
                        options,
                    }
                }
                DbKind::ReadWrite(db) => {
                    let options = rocksdb_readoptions_create_copy(db.read_options);
                    let snapshot = rocksdb_transactiondb_create_snapshot(db.db);
                    assert!(
                        !snapshot.is_null(),
                        "rocksdb_transactiondb_create_snapshot returned null"
                    );
                    rocksdb_readoptions_set_snapshot(options, snapshot);
                    Reader {
                        inner: InnerReader::TransactionalSnapshot(Rc::new(TransactionalSnapshot {
                            db: Arc::clone(db),
                            snapshot,
                        })),
                        options,
                    }
                }
            }
        }
    }

    pub fn transaction<'a, 'b: 'a, T, E: Error + 'static + From<StorageError>>(
        &'b self,
        f: impl Fn(Transaction<'a>) -> Result<T, E>,
    ) -> Result<T, E> {
        if let DbKind::ReadWrite(db) = &self.inner {
            loop {
                let transaction = unsafe {
                    let transaction = rocksdb_transaction_begin(
                        db.db,
                        db.write_options,
                        db.transaction_options,
                        ptr::null_mut(),
                    );
                    assert!(
                        !transaction.is_null(),
                        "rocksdb_transaction_begin returned null"
                    );
                    transaction
                };
                let (read_options, snapshot) = unsafe {
                    let options = rocksdb_readoptions_create_copy(db.read_options);
                    let snapshot = rocksdb_transaction_get_snapshot(transaction);
                    rocksdb_readoptions_set_snapshot(options, snapshot);
                    (options, snapshot)
                };
                let result = f(Transaction {
                    transaction: Rc::new(transaction),
                    read_options,
                    _lifetime: PhantomData,
                });
                match result {
                    Ok(result) => {
                        unsafe {
                            let r =
                                ffi_result!(rocksdb_transaction_commit_with_status(transaction));
                            rocksdb_transaction_destroy(transaction);
                            rocksdb_readoptions_destroy(read_options);
                            free(snapshot as *mut c_void);
                            r.map_err(StorageError::from)?; // We make sure to also run destructors if the commit fails
                        }
                        return Ok(result);
                    }
                    Err(e) => {
                        unsafe {
                            let r =
                                ffi_result!(rocksdb_transaction_rollback_with_status(transaction));
                            rocksdb_transaction_destroy(transaction);
                            rocksdb_readoptions_destroy(read_options);
                            free(snapshot as *mut c_void);
                            r.map_err(StorageError::from)?; // We make sure to also run destructors if the commit fails
                        }
                        // We look for the root error
                        let mut error: &(dyn Error + 'static) = &e;
                        while let Some(e) = error.source() {
                            error = e;
                        }
                        let is_conflict_error =
                            error.downcast_ref::<ErrorStatus>().map_or(false, |e| {
                                e.0.code == rocksdb_status_code_t_rocksdb_status_code_busy
                                    || e.0.code
                                        == rocksdb_status_code_t_rocksdb_status_code_timed_out
                                    || e.0.code
                                        == rocksdb_status_code_t_rocksdb_status_code_try_again
                            });
                        if is_conflict_error {
                            // We give a chance to the OS to do something else before retrying in order to help avoiding another conflict
                            yield_now();
                        } else {
                            // We raise the error
                            return Err(e);
                        }
                    }
                }
            }
        } else {
            Err(
                StorageError::Other("Transaction are only possible on read-write instances".into())
                    .into(),
            )
        }
    }

    pub fn get(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
    ) -> Result<Option<PinnableSlice>, StorageError> {
        unsafe {
            let slice = match &self.inner {
                DbKind::ReadOnly(db) => {
                    ffi_result!(rocksdb_get_pinned_cf_with_status(
                        db.db,
                        db.read_options,
                        column_family.0,
                        key.as_ptr().cast(),
                        key.len(),
                    ))
                }
                DbKind::ReadWrite(db) => {
                    ffi_result!(rocksdb_transactiondb_get_pinned_cf_with_status(
                        db.db,
                        db.read_options,
                        column_family.0,
                        key.as_ptr().cast(),
                        key.len()
                    ))
                }
            }?;
            Ok(if slice.is_null() {
                None
            } else {
                Some(PinnableSlice(slice))
            })
        }
    }

    pub fn contains_key(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
    ) -> Result<bool, StorageError> {
        Ok(self.get(column_family, key)?.is_some()) //TODO: optimize
    }

    pub fn insert(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
        value: &[u8],
    ) -> Result<(), StorageError> {
        if let DbKind::ReadWrite(db) = &self.inner {
            unsafe {
                ffi_result!(rocksdb_transactiondb_put_cf_with_status(
                    db.db,
                    db.write_options,
                    column_family.0,
                    key.as_ptr().cast(),
                    key.len(),
                    value.as_ptr().cast(),
                    value.len(),
                ))
            }?;
            Ok(())
        } else {
            Err(StorageError::Other(
                "Inserts are only possible on read-write instances".into(),
            ))
        }
    }

    pub fn flush(&self, column_family: &ColumnFamily) -> Result<(), StorageError> {
        if let DbKind::ReadWrite(db) = &self.inner {
            unsafe {
                ffi_result!(rocksdb_transactiondb_flush_cf_with_status(
                    db.db,
                    db.flush_options,
                    column_family.0,
                ))
            }?;
            Ok(())
        } else {
            Err(StorageError::Other(
                "Flush is only possible on read-write instances".into(),
            ))
        }
    }

    pub fn compact(&self, column_family: &ColumnFamily) -> Result<(), StorageError> {
        if let DbKind::ReadWrite(db) = &self.inner {
            unsafe {
                ffi_result!(rocksdb_transactiondb_compact_range_cf_opt_with_status(
                    db.db,
                    column_family.0,
                    db.compaction_options,
                    ptr::null(),
                    0,
                    ptr::null(),
                    0,
                ))
            }?;
            Ok(())
        } else {
            Err(StorageError::Other(
                "Compaction is only possible on read-write instances".into(),
            ))
        }
    }

    pub fn new_sst_file(&self) -> Result<SstFileWriter, StorageError> {
        if let DbKind::ReadWrite(db) = &self.inner {
            let path = db.path.join(random::<u128>().to_string());
            unsafe {
                let writer = rocksdb_sstfilewriter_create(db.env_options, db.options);
                ffi_result!(rocksdb_sstfilewriter_open_with_status(
                    writer,
                    path_to_cstring(&path)?.as_ptr()
                ))
                .map_err(|e| {
                    rocksdb_sstfilewriter_destroy(writer);
                    e
                })?;
                Ok(SstFileWriter { writer, path })
            }
        } else {
            Err(StorageError::Other(
                "SST creation is only possible on read-write instances".into(),
            ))
        }
    }

    pub fn insert_stt_files(
        &self,
        ssts_for_cf: &[(&ColumnFamily, PathBuf)],
    ) -> Result<(), StorageError> {
        if let DbKind::ReadWrite(db) = &self.inner {
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
                    options: db.ingest_external_file_options,
                })
                .collect::<Vec<_>>();
            unsafe {
                ffi_result!(rocksdb_transactiondb_ingest_external_files_with_status(
                    db.db,
                    args.as_ptr(),
                    args.len()
                ))?;
            }
            Ok(())
        } else {
            Err(StorageError::Other(
                "SST ingestion is only possible on read-write instances".into(),
            ))
        }
    }

    pub fn backup(&self, target_directory: &Path) -> Result<(), StorageError> {
        let path = path_to_cstring(target_directory)?;
        match &self.inner {
            DbKind::ReadOnly(db) => unsafe {
                if db.is_secondary {
                    ffi_result!(rocksdb_try_catch_up_with_primary_with_status(db.db))?;
                }
                ffi_result!(rocksdb_create_checkpoint_with_status(db.db, path.as_ptr()))
            },
            DbKind::ReadWrite(db) => {
                if db.in_memory {
                    return Err(StorageError::Other(
                        "It is not possible to backup an in-memory database".into(),
                    ));
                }
                unsafe {
                    ffi_result!(rocksdb_transactiondb_create_checkpoint_with_status(
                        db.db,
                        path.as_ptr()
                    ))
                }
            }
        }?;
        Ok(())
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
    TransactionalSnapshot(Rc<TransactionalSnapshot>),
    Transaction(Weak<*mut rocksdb_transaction_t>),
    PlainDb(Arc<RoDbHandler>),
}

struct TransactionalSnapshot {
    db: Arc<RwDbHandler>,
    snapshot: *const rocksdb_snapshot_t,
}

impl Drop for TransactionalSnapshot {
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
    pub fn get(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
    ) -> Result<Option<PinnableSlice>, StorageError> {
        unsafe {
            let slice = match &self.inner {
                InnerReader::TransactionalSnapshot(inner) => {
                    ffi_result!(rocksdb_transactiondb_get_pinned_cf_with_status(
                        inner.db.db,
                        self.options,
                        column_family.0,
                        key.as_ptr().cast(),
                        key.len()
                    ))
                }
                InnerReader::Transaction(inner) => {
                    let Some(inner) = inner.upgrade() else {
                        return Err(StorageError::Other(
                            "The transaction is already ended".into(),
                        ));
                    };
                    ffi_result!(rocksdb_transaction_get_pinned_cf_with_status(
                        *inner,
                        self.options,
                        column_family.0,
                        key.as_ptr().cast(),
                        key.len()
                    ))
                }
                InnerReader::PlainDb(inner) => {
                    ffi_result!(rocksdb_get_pinned_cf_with_status(
                        inner.db,
                        self.options,
                        column_family.0,
                        key.as_ptr().cast(),
                        key.len()
                    ))
                }
            }?;
            Ok(if slice.is_null() {
                None
            } else {
                Some(PinnableSlice(slice))
            })
        }
    }

    pub fn contains_key(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
    ) -> Result<bool, StorageError> {
        Ok(self.get(column_family, key)?.is_some()) //TODO: optimize
    }

    #[allow(clippy::iter_not_returning_iterator)]
    pub fn iter(&self, column_family: &ColumnFamily) -> Result<Iter, StorageError> {
        self.scan_prefix(column_family, &[])
    }

    pub fn scan_prefix(
        &self,
        column_family: &ColumnFamily,
        prefix: &[u8],
    ) -> Result<Iter, StorageError> {
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
            found.then_some(bound)
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
                    upper_bound.as_ptr().cast(),
                    upper_bound.len(),
                );
            }
            let iter = match &self.inner {
                InnerReader::TransactionalSnapshot(inner) => {
                    rocksdb_transactiondb_create_iterator_cf(inner.db.db, options, column_family.0)
                }
                InnerReader::Transaction(inner) => {
                    let Some(inner) = inner.upgrade() else {
                        return Err(StorageError::Other(
                            "The transaction is already ended".into(),
                        ));
                    };
                    rocksdb_transaction_create_iterator_cf(*inner, options, column_family.0)
                }
                InnerReader::PlainDb(inner) => {
                    rocksdb_create_iterator_cf(inner.db, options, column_family.0)
                }
            };
            assert!(!iter.is_null(), "rocksdb_create_iterator returned null");
            if prefix.is_empty() {
                rocksdb_iter_seek_to_first(iter);
            } else {
                rocksdb_iter_seek(iter, prefix.as_ptr().cast(), prefix.len());
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

    pub fn len(&self, column_family: &ColumnFamily) -> Result<usize, StorageError> {
        let mut count = 0;
        let mut iter = self.iter(column_family)?;
        while iter.is_valid() {
            count += 1;
            iter.next();
        }
        iter.status()?; // We makes sure there is no read problem
        Ok(count)
    }

    pub fn is_empty(&self, column_family: &ColumnFamily) -> Result<bool, StorageError> {
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

    pub fn get_for_update(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
    ) -> Result<Option<PinnableSlice>, StorageError> {
        unsafe {
            let slice = ffi_result!(rocksdb_transaction_get_for_update_pinned_cf_with_status(
                *self.transaction,
                self.read_options,
                column_family.0,
                key.as_ptr().cast(),
                key.len()
            ))?;
            Ok(if slice.is_null() {
                None
            } else {
                Some(PinnableSlice(slice))
            })
        }
    }

    pub fn contains_key_for_update(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
    ) -> Result<bool, StorageError> {
        Ok(self.get_for_update(column_family, key)?.is_some()) //TODO: optimize
    }

    pub fn insert(
        &mut self,
        column_family: &ColumnFamily,
        key: &[u8],
        value: &[u8],
    ) -> Result<(), StorageError> {
        unsafe {
            ffi_result!(rocksdb_transaction_put_cf_with_status(
                *self.transaction,
                column_family.0,
                key.as_ptr().cast(),
                key.len(),
                value.as_ptr().cast(),
                value.len(),
            ))?;
        }
        Ok(())
    }

    pub fn insert_empty(
        &mut self,
        column_family: &ColumnFamily,
        key: &[u8],
    ) -> Result<(), StorageError> {
        self.insert(column_family, key, &[])
    }

    pub fn remove(&mut self, column_family: &ColumnFamily, key: &[u8]) -> Result<(), StorageError> {
        unsafe {
            ffi_result!(rocksdb_transaction_delete_cf_with_status(
                *self.transaction,
                column_family.0,
                key.as_ptr().cast(),
                key.len(),
            ))?;
        }
        Ok(())
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
            slice::from_raw_parts(val.cast(), len)
        }
    }
}

impl AsRef<[u8]> for PinnableSlice {
    fn as_ref(&self) -> &[u8] {
        self
    }
}

impl Borrow<[u8]> for PinnableSlice {
    fn borrow(&self) -> &[u8] {
        self
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
            free(self.base.cast());
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
        self
    }
}

impl Borrow<[u8]> for Buffer {
    fn borrow(&self) -> &[u8] {
        self
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

#[allow(clippy::non_send_fields_in_send_ty)]
unsafe impl Send for Iter {}

unsafe impl Sync for Iter {}

impl Iter {
    pub fn is_valid(&self) -> bool {
        self.is_currently_valid
    }

    pub fn status(&self) -> Result<(), StorageError> {
        unsafe {
            ffi_result!(rocksdb_iter_get_status(self.iter))?;
        }
        Ok(())
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
                Some(slice::from_raw_parts(val.cast(), len))
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
    pub fn insert(&mut self, key: &[u8], value: &[u8]) -> Result<(), StorageError> {
        unsafe {
            ffi_result!(rocksdb_sstfilewriter_put_with_status(
                self.writer,
                key.as_ptr().cast(),
                key.len(),
                value.as_ptr().cast(),
                value.len(),
            ))?;
        }
        Ok(())
    }

    pub fn insert_empty(&mut self, key: &[u8]) -> Result<(), StorageError> {
        self.insert(key, &[])
    }

    pub fn finish(self) -> Result<PathBuf, StorageError> {
        unsafe {
            ffi_result!(rocksdb_sstfilewriter_finish_with_status(self.writer))?;
        }
        Ok(self.path.clone())
    }
}

struct ErrorStatus(rocksdb_status_t);

unsafe impl Send for ErrorStatus {}
unsafe impl Sync for ErrorStatus {}

impl Drop for ErrorStatus {
    fn drop(&mut self) {
        if !self.0.string.is_null() {
            unsafe {
                free(self.0.string as *mut c_void);
            }
        }
    }
}

impl ErrorStatus {
    fn message(&self) -> &str {
        if self.0.string.is_null() {
            "Unknown error"
        } else {
            unsafe { CStr::from_ptr(self.0.string) }
                .to_str()
                .unwrap_or("Invalid error message")
        }
    }
}

impl fmt::Debug for ErrorStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErrorStatus")
            .field("code", &self.0.code)
            .field("subcode", &self.0.subcode)
            .field("severity", &self.0.severity)
            .field("message", &self.message())
            .finish()
    }
}

impl fmt::Display for ErrorStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

impl Error for ErrorStatus {}

impl From<ErrorStatus> for StorageError {
    fn from(status: ErrorStatus) -> Self {
        if status.0.code == rocksdb_status_code_t_rocksdb_status_code_io_error {
            let kind =
                if status.0.subcode == rocksdb_status_subcode_t_rocksdb_status_subcode_no_space {
                    io::ErrorKind::Other // TODO ErrorKind::StorageFull
                } else if status.0.subcode
                    == rocksdb_status_subcode_t_rocksdb_status_subcode_path_not_found
                {
                    io::ErrorKind::NotFound
                } else {
                    io::ErrorKind::Other
                };
            Self::Io(io::Error::new(kind, status))
        } else if status.0.code == rocksdb_status_code_t_rocksdb_status_code_corruption {
            Self::Corruption(CorruptionError::new(status))
        } else {
            Self::Other(Box::new(status))
        }
    }
}

struct UnsafeEnv(*mut rocksdb_env_t);

// Hack for OnceCell. OK because only written in OnceCell and used in a thread-safe way by RocksDB
unsafe impl Send for UnsafeEnv {}
unsafe impl Sync for UnsafeEnv {}

fn path_to_cstring(path: &Path) -> Result<CString, StorageError> {
    Ok(CString::new(path.to_str().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "The DB path is not valid UTF-8",
        )
    })?)
    .map_err(|e| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("The DB path contains null bytes: {e}"),
        )
    })?)
}

#[cfg(unix)]
fn available_file_descriptors() -> io::Result<Option<u64>> {
    let mut rlimit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut rlimit) } == 0 {
        Ok(Some(min(rlimit.rlim_cur, rlimit.rlim_max)))
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(windows)]
fn available_file_descriptors() -> io::Result<Option<u64>> {
    Ok(Some(512)) // https://docs.microsoft.com/en-us/cpp/c-runtime-library/file-handling
}

#[cfg(not(any(unix, windows)))]
fn available_file_descriptors() -> io::Result<Option<u64>> {
    Ok(None)
}

fn tmp_path() -> PathBuf {
    if cfg!(target_os = "linux") {
        "/dev/shm/".into()
    } else {
        temp_dir()
    }
    .join(format!("oxigraph-rocksdb-{}", random::<u128>()))
}
