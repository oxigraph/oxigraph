#pragma once

#include "../rocksdb/include/rocksdb/c.h"

#ifdef __cplusplus
extern "C" {
#endif

typedef enum rocksdb_status_code_t {
    rocksdb_status_code_ok = 0,
    rocksdb_status_code_not_found = 1,
    rocksdb_status_code_corruption = 2,
    rocksdb_status_code_not_supported = 3,
    rocksdb_status_code_invalid_argument = 4,
    rocksdb_status_code_io_error = 5,
    rocksdb_status_code_merge_in_progress = 6,
    rocksdb_status_code_incomplete = 7,
    rocksdb_status_code_shutdown_in_progress = 8,
    rocksdb_status_code_timed_out = 9,
    rocksdb_status_code_aborted = 10,
    rocksdb_status_code_busy = 11,
    rocksdb_status_code_expired = 12,
    rocksdb_status_code_try_again = 13,
    rocksdb_status_code_compaction_too_large = 14,
    rocksdb_status_code_column_family_dropped = 15,
} rocksdb_status_code_t;

typedef enum rocksdb_status_subcode_t {
    rocksdb_status_subcode_none = 0,
    rocksdb_status_subcode_mutex_timeout = 1,
    rocksdb_status_subcode_lock_timeout = 2,
    rocksdb_status_subcode_lock_limit = 3,
    rocksdb_status_subcode_no_space = 4,
    rocksdb_status_subcode_deadlock = 5,
    rocksdb_status_subcode_stale_file = 6,
    rocksdb_status_subcode_memory_limit = 7,
    rocksdb_status_subcode_space_limit = 8,
    rocksdb_status_subcode_path_not_found = 9,
    rocksdb_status_subcode_merge_operands_insufficient_capacity = 10,
    rocksdb_status_subcode_manual_compaction_paused = 11,
    rocksdb_status_subcode_overwritten = 12,
    rocksdb_status_subcode_txn_not_prepared = 13,
    rocksdb_status_subcode_io_fenced = 14,
} rocksdb_status_subcode_t;

typedef enum rocksdb_status_severity_t {
    rocksdb_status_severity_none = 0,
    rocksdb_status_severity_soft_error = 1,
    rocksdb_status_severity_hard_error = 2,
    rocksdb_status_severity_fatal_error = 3,
    rocksdb_status_severity_unrecoverable_error = 4,
} rocksdb_status_severity_t;

typedef struct rocksdb_status_t {
    rocksdb_status_code_t code;
    rocksdb_status_subcode_t subcode;
    rocksdb_status_severity_t severity;
    const char* string;
} rocksdb_status_t;

typedef struct rocksdb_ingestexternalfilearg_t {
    rocksdb_column_family_handle_t* column_family;
    char const* const* external_files;
    size_t external_files_len;
    rocksdb_ingestexternalfileoptions_t* options;
} rocksdb_ingestexternalfilearg_t;

extern ROCKSDB_LIBRARY_API rocksdb_transactiondb_t* rocksdb_transactiondb_open_column_families_with_status(
        const rocksdb_options_t* options,
        const rocksdb_transactiondb_options_t* txn_db_options, const char* name,
        int num_column_families, const char* const* column_family_names,
        const rocksdb_options_t* const* column_family_options,
        rocksdb_column_family_handle_t** column_family_handles, rocksdb_status_t* statusptr);


extern ROCKSDB_LIBRARY_API rocksdb_pinnableslice_t* rocksdb_transactiondb_get_pinned_cf_with_status(
        rocksdb_transactiondb_t* db, const rocksdb_readoptions_t* options,
        rocksdb_column_family_handle_t* column_family, const char* key,
        size_t keylen, rocksdb_status_t* statusptr);

extern ROCKSDB_LIBRARY_API void rocksdb_transactiondb_put_cf_with_status(
        rocksdb_transactiondb_t* txn_db, const rocksdb_writeoptions_t* options,
        rocksdb_column_family_handle_t* column_family, const char* key,
        size_t keylen, const char* val, size_t vallen, rocksdb_status_t* statusptr);

extern ROCKSDB_LIBRARY_API void rocksdb_transactiondb_flush_cf_with_status(
        rocksdb_transactiondb_t* db, const rocksdb_flushoptions_t* options,
        rocksdb_column_family_handle_t* column_family, rocksdb_status_t* statusptr);

extern ROCKSDB_LIBRARY_API void rocksdb_transactiondb_compact_range_cf_opt_with_status(
        rocksdb_transactiondb_t* db, rocksdb_column_family_handle_t* column_family,
        rocksdb_compactoptions_t* opt, const char* start_key, size_t start_key_len,
        const char* limit_key, size_t limit_key_len, rocksdb_status_t* statusptr);

extern ROCKSDB_LIBRARY_API void rocksdb_transactiondb_ingest_external_files_with_status(
        rocksdb_transactiondb_t* db, const rocksdb_ingestexternalfilearg_t* list,
        const size_t list_len, rocksdb_status_t* statusptr);

extern ROCKSDB_LIBRARY_API void rocksdb_transactiondb_create_checkpoint_with_status(
        rocksdb_transactiondb_t* db, const char* checkpoint_dir, rocksdb_status_t* statusptr);


extern ROCKSDB_LIBRARY_API void rocksdb_transaction_commit_with_status(
        rocksdb_transaction_t* txn, rocksdb_status_t* statusptr);

extern ROCKSDB_LIBRARY_API void rocksdb_transaction_rollback_with_status(
        rocksdb_transaction_t* txn, rocksdb_status_t* statusptr);

extern ROCKSDB_LIBRARY_API rocksdb_pinnableslice_t* rocksdb_transaction_get_pinned_cf_with_status(
        rocksdb_transaction_t* txn, const rocksdb_readoptions_t* options,
        rocksdb_column_family_handle_t* column_family, const char* key,
        size_t keylen, rocksdb_status_t* statusptr);

extern ROCKSDB_LIBRARY_API rocksdb_pinnableslice_t* rocksdb_transaction_get_for_update_pinned_cf_with_status(
        rocksdb_transaction_t* txn, const rocksdb_readoptions_t* options,
        rocksdb_column_family_handle_t* column_family, const char* key,
        size_t keylen, rocksdb_status_t* statusptr);

extern ROCKSDB_LIBRARY_API void rocksdb_transaction_put_cf_with_status(
        rocksdb_transaction_t* txn, rocksdb_column_family_handle_t* column_family,
        const char* key, size_t klen, const char* val, size_t vlen, rocksdb_status_t* statusptr);

extern ROCKSDB_LIBRARY_API void rocksdb_transaction_delete_cf_with_status(
        rocksdb_transaction_t* txn, rocksdb_column_family_handle_t* column_family,
        const char* key, size_t klen, rocksdb_status_t* statusptr);


extern ROCKSDB_LIBRARY_API void rocksdb_sstfilewriter_open_with_status(
        rocksdb_sstfilewriter_t* writer, const char* name, rocksdb_status_t* statusptr);

extern ROCKSDB_LIBRARY_API void rocksdb_sstfilewriter_put_with_status(
        rocksdb_sstfilewriter_t* writer, const char* key, size_t keylen,
        const char* val, size_t vallen, rocksdb_status_t* statusptr);

extern ROCKSDB_LIBRARY_API void rocksdb_sstfilewriter_finish_with_status(
        rocksdb_sstfilewriter_t* writer, rocksdb_status_t* statusptr);


extern ROCKSDB_LIBRARY_API void rocksdb_iter_get_status(
        const rocksdb_iterator_t*, rocksdb_status_t* statusptr);


extern ROCKSDB_LIBRARY_API rocksdb_readoptions_t* rocksdb_readoptions_create_copy(
        rocksdb_readoptions_t*);

#ifdef __cplusplus
}
#endif
