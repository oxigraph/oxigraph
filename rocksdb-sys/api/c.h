#pragma once

#include "../rocksdb/include/rocksdb/c.h"

#ifdef __cplusplus
extern "C" {
#endif

extern ROCKSDB_LIBRARY_API rocksdb_pinnableslice_t* rocksdb_transactiondb_get_pinned_cf(
        rocksdb_transactiondb_t* db, const rocksdb_readoptions_t* options,
        rocksdb_column_family_handle_t* column_family, const char* key,
        size_t keylen, char** errptr);

extern ROCKSDB_LIBRARY_API void rocksdb_transactiondb_flush_cf(
        rocksdb_transactiondb_t* db, const rocksdb_flushoptions_t* options,
        rocksdb_column_family_handle_t* column_family, char** errptr);

extern ROCKSDB_LIBRARY_API void rocksdb_transactiondb_compact_range_cf_opt(
        rocksdb_transactiondb_t* db, rocksdb_column_family_handle_t* column_family,
        rocksdb_compactoptions_t* opt, const char* start_key, size_t start_key_len,
        const char* limit_key, size_t limit_key_len, char** errptr);

extern ROCKSDB_LIBRARY_API rocksdb_writeoptions_t* rocksdb_writeoptions_create_copy(
        rocksdb_writeoptions_t*);

#ifdef __cplusplus
}
#endif
