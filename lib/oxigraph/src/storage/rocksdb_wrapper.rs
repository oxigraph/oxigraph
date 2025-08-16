//! Code inspired by [Rust RocksDB](https://github.com/rust-rocksdb/rust-rocksdb) under Apache License 2.0.

#![allow(
    unsafe_code,
    trivial_casts,
    clippy::undocumented_unsafe_blocks,
    clippy::panic_in_result_fn,
    clippy::unwrap_in_result
)]

use crate::storage::error::{CorruptionError, StorageError};
use oxrocksdb_sys::*;
use rand::random;
use std::borrow::Borrow;
#[cfg(unix)]
use std::cmp::min;
use std::collections::HashMap;
use std::error::Error;
use std::ffi::CString;
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::thread::available_parallelism;
use std::{fmt, io, ptr, slice};

macro_rules! ffi_result {
    ( $($function:ident)::*( $arg1:expr $(, $arg:expr)* $(,)? ) ) => {{
        let mut error: *mut ::std::ffi::c_char = ::std::ptr::null_mut();
        let result = $($function)::*($arg1 $(, $arg)* , &mut error);
        if error.is_null() {
            Ok(result)
        } else {
            Err(ErrorStatus(::std::ffi::CString::from_raw(error)))
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
    db: *mut rocksdb_t,
    options: *mut rocksdb_options_t,
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
}

unsafe impl Send for RwDbHandler {}

unsafe impl Sync for RwDbHandler {}

impl Drop for RwDbHandler {
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
            rocksdb_flushoptions_destroy(self.flush_options);
            rocksdb_envoptions_destroy(self.env_options);
            rocksdb_ingestexternalfileoptions_destroy(self.ingest_external_file_options);
            rocksdb_compactoptions_destroy(self.compaction_options);
            rocksdb_options_destroy(self.options);
            rocksdb_block_based_options_destroy(self.block_based_table_options);
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
    }
}

impl Db {
    pub fn open_read_write(
        path: &Path,
        column_families: Vec<ColumnFamilyDefinition>,
    ) -> Result<Self, StorageError> {
        let c_path = path_to_cstring(path)?;
        unsafe {
            let options = Self::db_options(true)?;
            rocksdb_options_set_create_if_missing(options, 1);
            rocksdb_options_set_create_missing_column_families(options, 1);
            rocksdb_options_set_compression(options, rocksdb_lz4_compression.try_into().unwrap());
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

            let db = ffi_result!(rocksdb_open_column_families(
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
            ))
            .map_err(|e| {
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
            rocksdb_readoptions_set_async_io(read_options, 1);

            let write_options = rocksdb_writeoptions_create();
            assert!(
                !write_options.is_null(),
                "rocksdb_writeoptions_create returned null"
            );

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
                    path: path.into(),
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
            let options = Self::db_options(true)?;
            let (column_family_names, c_column_family_names, cf_options) =
                Self::column_families_names_and_options(column_families, options);
            let mut cf_handles: Vec<*mut rocksdb_column_family_handle_t> =
                vec![ptr::null_mut(); column_family_names.len()];
            let c_num_column_families = c_column_family_names.len().try_into().unwrap();
            let db = ffi_result!(rocksdb_open_for_read_only_column_families(
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
                "rocksdb_open_for_read_only_column_families returned null"
            );
            for handle in &cf_handles {
                assert!(
                    !handle.is_null(),
                    "rocksdb_open_for_read_only_column_families returned a null column family"
                );
            }
            let read_options = rocksdb_readoptions_create();
            assert!(
                !read_options.is_null(),
                "rocksdb_readoptions_create returned null"
            );
            rocksdb_readoptions_set_async_io(read_options, 1);

            Ok(Self {
                inner: DbKind::ReadOnly(Arc::new(RoDbHandler {
                    db,
                    options,
                    read_options,
                    column_family_names,
                    cf_handles,
                    cf_options,
                })),
            })
        }
    }

    fn db_options(limit_max_open_files: bool) -> Result<*mut rocksdb_options_t, StorageError> {
        static ROCKSDB_ENV: OnceLock<UnsafeEnv> = OnceLock::new();
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
                        return Err(io::Error::other(format!(
                            "Oxigraph needs at least 96 file descriptors, \
                                    only {available_fd} allowed. \
                                    Run e.g. `ulimit -n 512` to allow 512 opened files"
                        ))
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
                ROCKSDB_ENV
                    .get_or_init(|| {
                        let env = rocksdb_create_default_env();
                        assert!(!env.is_null(), "rocksdb_create_default_env returned null");
                        UnsafeEnv(env)
                    })
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

    pub fn is_writable(&self) -> bool {
        match &self.inner {
            DbKind::ReadWrite(_) => true,
            DbKind::ReadOnly(_) => false,
        }
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
        Err(CorruptionError::from_missing_column_family_name(name).into())
    }

    #[must_use]
    pub fn snapshot(&self) -> Reader<'static> {
        unsafe {
            match &self.inner {
                DbKind::ReadOnly(db) => {
                    let options = oxrocksdb_readoptions_create_copy(db.read_options);
                    Reader {
                        inner: InnerReader::ReadOnly(Arc::clone(db)),
                        options,
                    }
                }
                DbKind::ReadWrite(db) => {
                    let options = oxrocksdb_readoptions_create_copy(db.read_options);
                    let snapshot = rocksdb_create_snapshot(db.db);
                    assert!(!snapshot.is_null(), "rocksdb_create_snapshot returned null");
                    rocksdb_readoptions_set_snapshot(options, snapshot);
                    Reader {
                        inner: InnerReader::ReadWrite(Arc::new(SnapshotReader {
                            db: Arc::clone(db),
                            snapshot,
                        })),
                        options,
                    }
                }
            }
        }
    }

    pub fn start_transaction(&self) -> Result<Transaction, StorageError> {
        let DbKind::ReadWrite(db) = &self.inner else {
            return Err(StorageError::Other(
                "Transaction are only possible on read-write instances".into(),
            ));
        };
        let batch = unsafe { rocksdb_writebatch_create() };
        assert!(!batch.is_null(), "rocksdb_writebatch_create returned null");
        Ok(Transaction {
            db: Arc::clone(db),
            batch,
        })
    }

    pub fn start_readable_transaction(&self) -> Result<ReadableTransaction<'_>, StorageError> {
        let DbKind::ReadWrite(db) = &self.inner else {
            return Err(StorageError::Other(
                "Transaction are only possible on read-write instances".into(),
            ));
        };
        let (batch, read_options, snapshot) = unsafe {
            let snapshot = rocksdb_create_snapshot(db.db);
            let options = oxrocksdb_readoptions_create_copy(db.read_options);
            rocksdb_readoptions_set_snapshot(options, snapshot);
            let batch = rocksdb_writebatch_wi_create(0, 1);
            (batch, options, snapshot)
        };
        assert!(!batch.is_null(), "rocksdb_writebatch_create returned null");
        Ok(ReadableTransaction {
            db,
            batch,
            snapshot,
            read_options,
        })
    }

    pub fn get(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
    ) -> Result<Option<PinnableSlice>, StorageError> {
        unsafe {
            let slice = match &self.inner {
                DbKind::ReadOnly(db) => {
                    ffi_result!(rocksdb_get_pinned_cf(
                        db.db,
                        db.read_options,
                        column_family.0,
                        key.as_ptr().cast(),
                        key.len(),
                    ))
                }
                DbKind::ReadWrite(db) => {
                    ffi_result!(rocksdb_get_pinned_cf(
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
        Ok(self.get(column_family, key)?.is_some()) // TODO: optimize
    }

    pub fn insert(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
        value: &[u8],
    ) -> Result<(), StorageError> {
        let DbKind::ReadWrite(db) = &self.inner else {
            return Err(StorageError::Other(
                "Inserts are only possible on read-write instances".into(),
            ));
        };
        unsafe {
            ffi_result!(rocksdb_put_cf(
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
    }

    pub fn flush(&self) -> Result<(), StorageError> {
        let DbKind::ReadWrite(db) = &self.inner else {
            return Err(StorageError::Other(
                "Flush is only possible on read-write instances".into(),
            ));
        };
        unsafe {
            ffi_result!(rocksdb_flush_cfs(
                db.db,
                db.flush_options,
                db.cf_handles.as_ptr().cast_mut(),
                db.cf_handles.len().try_into().unwrap()
            ))
        }?;
        Ok(())
    }

    pub fn compact(&self, column_family: &ColumnFamily) -> Result<(), StorageError> {
        let DbKind::ReadWrite(db) = &self.inner else {
            return Err(StorageError::Other(
                "Compact are only possible on read-write instances".into(),
            ));
        };
        unsafe {
            rocksdb_compact_range_cf_opt(
                db.db.cast(),
                column_family.0,
                db.compaction_options,
                ptr::null(),
                0,
                ptr::null(),
                0,
            )
        }
        Ok(())
    }

    pub fn new_sst_file(&self) -> Result<SstFileWriter, StorageError> {
        let DbKind::ReadWrite(db) = &self.inner else {
            return Err(StorageError::Other(
                "SST creation is only possible on read-write instances".into(),
            ));
        };
        let path = db.path.join(random::<u128>().to_string());
        unsafe {
            let writer = rocksdb_sstfilewriter_create(db.env_options, db.options);
            ffi_result!(rocksdb_sstfilewriter_open(
                writer,
                path_to_cstring(&path)?.as_ptr()
            ))
            .map_err(|e| {
                rocksdb_sstfilewriter_destroy(writer);
                e
            })?;
            Ok(SstFileWriter { writer, path })
        }
    }

    pub fn insert_stt_files(
        &self,
        ssts_for_cf: &[(ColumnFamily, PathBuf)],
    ) -> Result<(), StorageError> {
        let DbKind::ReadWrite(db) = &self.inner else {
            return Err(StorageError::Other(
                "SST ingestion is only possible on read-write instances".into(),
            ));
        };
        if ssts_for_cf.is_empty() {
            return Ok(()); // Rocksdb does not support empty lists
        }
        let mut paths_by_cf = HashMap::<_, Vec<_>>::new();
        for (cf, path) in ssts_for_cf {
            paths_by_cf
                .entry(cf)
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
            ffi_result!(oxrocksdb_ingest_external_files(
                db.db,
                args.as_ptr(),
                args.len()
            ))?;
        }
        Ok(())
    }

    pub fn backup(&self, target_directory: &Path) -> Result<(), StorageError> {
        let path = path_to_cstring(target_directory)?;
        unsafe {
            let checkpoint = ffi_result!(rocksdb_checkpoint_object_create(match &self.inner {
                DbKind::ReadOnly(db) => db.db,
                DbKind::ReadWrite(db) => db.db,
            }))?;
            assert!(
                !checkpoint.is_null(),
                "rocksdb_checkpoint_object_create returned null"
            );
            let result = ffi_result!(rocksdb_checkpoint_create(checkpoint, path.as_ptr(), 0));
            rocksdb_checkpoint_object_destroy(checkpoint);
            result
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

pub struct Reader<'a> {
    inner: InnerReader<'a>,
    options: *mut rocksdb_readoptions_t,
}

unsafe impl Send for Reader<'_> {}
unsafe impl Sync for Reader<'_> {}

#[derive(Clone)]
enum InnerReader<'a> {
    ReadOnly(Arc<RoDbHandler>),
    ReadWrite(Arc<SnapshotReader>),
    Transaction(TransactionReader<'a>),
}

struct SnapshotReader {
    db: Arc<RwDbHandler>,
    snapshot: *const rocksdb_snapshot_t,
}

unsafe impl Send for SnapshotReader {}
unsafe impl Sync for SnapshotReader {}

impl Drop for SnapshotReader {
    fn drop(&mut self) {
        unsafe { rocksdb_release_snapshot(self.db.db, self.snapshot) }
    }
}

#[derive(Clone)]
struct TransactionReader<'a> {
    db: &'a RwDbHandler,
    batch: *mut rocksdb_writebatch_wi_t,
}

unsafe impl Send for TransactionReader<'_> {}
unsafe impl Sync for TransactionReader<'_> {}

impl Clone for Reader<'_> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            options: unsafe { oxrocksdb_readoptions_create_copy(self.options) },
        }
    }
}

impl Drop for Reader<'_> {
    fn drop(&mut self) {
        unsafe { rocksdb_readoptions_destroy(self.options) }
    }
}

impl<'a> Reader<'a> {
    pub fn get(
        &self,
        column_family: &ColumnFamily,
        key: &[u8],
    ) -> Result<Option<PinnableSlice>, StorageError> {
        unsafe {
            let slice = match &self.inner {
                InnerReader::ReadOnly(inner) => {
                    ffi_result!(rocksdb_get_pinned_cf(
                        inner.db,
                        self.options,
                        column_family.0,
                        key.as_ptr().cast(),
                        key.len()
                    ))
                }
                InnerReader::ReadWrite(inner) => {
                    ffi_result!(rocksdb_get_pinned_cf(
                        inner.db.db,
                        self.options,
                        column_family.0,
                        key.as_ptr().cast(),
                        key.len()
                    ))
                }
                InnerReader::Transaction(inner) => {
                    ffi_result!(oxrocksdb_writebatch_wi_get_pinned_from_batch_and_db_cf(
                        inner.batch,
                        inner.db.db,
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
        Ok(self.get(column_family, key)?.is_some()) // TODO: optimize
    }

    #[expect(clippy::iter_not_returning_iterator)]
    pub fn iter(&self, column_family: &ColumnFamily) -> Iter<'a> {
        self.scan_prefix(column_family, &[])
    }

    pub fn scan_prefix(&self, column_family: &ColumnFamily, prefix: &[u8]) -> Iter<'a> {
        // We generate the upper bound
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
            let options = oxrocksdb_readoptions_create_copy(self.options);
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
                InnerReader::ReadOnly(inner) => {
                    rocksdb_create_iterator_cf(inner.db, options, column_family.0)
                }
                InnerReader::ReadWrite(inner) => {
                    rocksdb_create_iterator_cf(inner.db.db, options, column_family.0)
                }
                InnerReader::Transaction(inner) => {
                    oxrocksdb_writebatch_wi_create_iterator_with_base_readopts_cf(
                        inner.batch,
                        rocksdb_create_iterator_cf(inner.db.db, options, column_family.0),
                        options,
                        column_family.0,
                    )
                }
            };
            assert!(!iter.is_null(), "rocksdb_create_iterator returned null");
            if prefix.is_empty() {
                rocksdb_iter_seek_to_first(iter);
            } else {
                rocksdb_iter_seek(iter, prefix.as_ptr().cast(), prefix.len());
            }
            let is_currently_valid = rocksdb_iter_valid(iter) != 0;
            Iter {
                inner: iter,
                options,
                _upper_bound: upper_bound,
                _reader: self.clone(),
                is_currently_valid,
            }
        }
    }

    pub fn len(&self, column_family: &ColumnFamily) -> Result<usize, StorageError> {
        let mut count = 0;
        let mut iter = self.iter(column_family);
        while iter.is_valid() {
            count += 1;
            iter.next();
        }
        iter.status()?; // We make sure there is no read problem
        Ok(count)
    }

    pub fn is_empty(&self, column_family: &ColumnFamily) -> Result<bool, StorageError> {
        let iter = self.iter(column_family);
        iter.status()?; // We make sure there is no read problem
        Ok(!iter.is_valid())
    }
}

/// Write-only operation on the database
pub struct Transaction {
    db: Arc<RwDbHandler>,
    batch: *mut rocksdb_writebatch_t,
}

impl Drop for Transaction {
    fn drop(&mut self) {
        unsafe {
            rocksdb_writebatch_destroy(self.batch);
        }
    }
}

impl Transaction {
    pub fn insert(&mut self, column_family: &ColumnFamily, key: &[u8], value: &[u8]) {
        unsafe {
            rocksdb_writebatch_put_cf(
                self.batch,
                column_family.0,
                key.as_ptr().cast(),
                key.len(),
                value.as_ptr().cast(),
                value.len(),
            )
        }
    }

    pub fn insert_empty(&mut self, column_family: &ColumnFamily, key: &[u8]) {
        self.insert(column_family, key, &[])
    }

    pub fn remove(&mut self, column_family: &ColumnFamily, key: &[u8]) {
        unsafe {
            rocksdb_writebatch_delete_cf(
                self.batch,
                column_family.0,
                key.as_ptr().cast(),
                key.len(),
            )
        }
    }

    pub fn remove_range(&mut self, column_family: &ColumnFamily, start_key: &[u8], end_key: &[u8]) {
        unsafe {
            rocksdb_writebatch_delete_range_cf(
                self.batch,
                column_family.0,
                start_key.as_ptr().cast(),
                start_key.len(),
                end_key.as_ptr().cast(),
                end_key.len(),
            )
        }
    }

    pub fn commit(self) -> Result<(), StorageError> {
        unsafe {
            ffi_result!(rocksdb_write(self.db.db, self.db.write_options, self.batch))?;
        }
        Ok(())
    }
}

pub struct ReadableTransaction<'a> {
    db: &'a RwDbHandler,
    batch: *mut rocksdb_writebatch_wi_t,
    snapshot: *const rocksdb_snapshot_t,
    read_options: *mut rocksdb_readoptions_t,
}

unsafe impl Send for ReadableTransaction<'_> {}
unsafe impl Sync for ReadableTransaction<'_> {}

impl Drop for ReadableTransaction<'_> {
    fn drop(&mut self) {
        unsafe {
            rocksdb_writebatch_wi_destroy(self.batch);
            rocksdb_readoptions_destroy(self.read_options);
            rocksdb_release_snapshot(self.db.db, self.snapshot);
        }
    }
}

impl ReadableTransaction<'_> {
    pub fn reader(&self) -> Reader<'_> {
        Reader {
            inner: InnerReader::Transaction(TransactionReader {
                db: self.db,
                batch: self.batch,
            }),
            options: unsafe { oxrocksdb_readoptions_create_copy(self.read_options) },
        }
    }

    pub fn insert(&mut self, column_family: &ColumnFamily, key: &[u8], value: &[u8]) {
        unsafe {
            rocksdb_writebatch_wi_put_cf(
                self.batch,
                column_family.0,
                key.as_ptr().cast(),
                key.len(),
                value.as_ptr().cast(),
                value.len(),
            );
        }
    }

    pub fn insert_empty(&mut self, column_family: &ColumnFamily, key: &[u8]) {
        self.insert(column_family, key, &[])
    }

    pub fn remove(&mut self, column_family: &ColumnFamily, key: &[u8]) {
        unsafe {
            rocksdb_writebatch_wi_delete_cf(
                self.batch,
                column_family.0,
                key.as_ptr().cast(),
                key.len(),
            );
        }
    }

    pub fn commit(self) -> Result<(), StorageError> {
        unsafe {
            ffi_result!(rocksdb_write_writebatch_wi(
                self.db.db,
                self.db.write_options,
                self.batch
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

    fn deref(&self) -> &Self::Target {
        unsafe {
            let mut len = 0;
            let val = rocksdb_pinnableslice_value(self.0, &raw mut len);
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
            rocksdb_free(self.base.cast());
        }
    }
}

impl Deref for Buffer {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
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

pub struct Iter<'a> {
    inner: *mut rocksdb_iterator_t,
    is_currently_valid: bool,
    _upper_bound: Option<Vec<u8>>,
    _reader: Reader<'a>, // needed to ensure that DB still lives while iter is used
    options: *mut rocksdb_readoptions_t, /* needed to ensure that options still lives while iter is used */
}

impl Drop for Iter<'_> {
    fn drop(&mut self) {
        unsafe {
            rocksdb_iter_destroy(self.inner);
            rocksdb_readoptions_destroy(self.options);
        }
    }
}

unsafe impl Send for Iter<'_> {}

unsafe impl Sync for Iter<'_> {}

impl Iter<'_> {
    pub fn is_valid(&self) -> bool {
        self.is_currently_valid
    }

    pub fn status(&self) -> Result<(), StorageError> {
        unsafe {
            ffi_result!(rocksdb_iter_get_error(self.inner))?;
        }
        Ok(())
    }

    pub fn next(&mut self) {
        unsafe {
            rocksdb_iter_next(self.inner);
            self.is_currently_valid = rocksdb_iter_valid(self.inner) != 0;
        }
    }

    pub fn key(&self) -> Option<&[u8]> {
        if self.is_valid() {
            unsafe {
                let mut len = 0;
                let val = rocksdb_iter_key(self.inner, &raw mut len);
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
            ffi_result!(rocksdb_sstfilewriter_put(
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
            ffi_result!(rocksdb_sstfilewriter_finish(self.writer))?;
        }
        Ok(self.path.clone())
    }
}

struct ErrorStatus(CString);

impl ErrorStatus {
    fn message(&self) -> &str {
        self.0.to_str().unwrap_or("Invalid RocksDB error message")
    }
}

impl fmt::Debug for ErrorStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ErrorStatus")
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
        let mut parts = status.message().split(": ");
        match parts.next() {
            Some("IO error") => Self::Io(io::Error::new(
                match parts.next() {
                    Some("Timeout Acquiring Mutex" | "Timeout waiting to lock key") => {
                        io::ErrorKind::TimedOut
                    }
                    Some("No space left on device" | "Space limit reached") => {
                        io::ErrorKind::StorageFull
                    }
                    Some("Deadlock") => io::ErrorKind::Deadlock,
                    Some("Stale file handle") => io::ErrorKind::StaleNetworkFileHandle,
                    Some("Memory limit reached") => io::ErrorKind::OutOfMemory,
                    Some("No such file or directory") => io::ErrorKind::NotFound,
                    _ => io::ErrorKind::Other,
                },
                status,
            )),
            Some("Corruption") => Self::Corruption(CorruptionError::new(status)),
            _ => Self::Other(Box::new(status)),
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
fn available_file_descriptors() -> io::Result<Option<libc::rlim_t>> {
    let mut rlimit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    if unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &raw mut rlimit) } == 0 {
        Ok(Some(min(rlimit.rlim_cur, rlimit.rlim_max)))
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(windows)]
fn available_file_descriptors() -> io::Result<Option<libc::c_int>> {
    Ok(Some(512)) // https://docs.microsoft.com/en-us/cpp/c-runtime-library/file-handling
}

#[cfg(not(any(unix, windows)))]
fn available_file_descriptors() -> io::Result<Option<libc::c_int>> {
    Ok(None)
}
