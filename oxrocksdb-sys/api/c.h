#pragma once

#include <rocksdb/c.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct rocksdb_ingestexternalfilearg_t {
  rocksdb_column_family_handle_t* column_family;
  char const* const* external_files;
  size_t external_files_len;
  rocksdb_ingestexternalfileoptions_t* options;
} rocksdb_ingestexternalfilearg_t;

extern ROCKSDB_LIBRARY_API void oxrocksdb_ingest_external_files(
    rocksdb_t* db, const rocksdb_ingestexternalfilearg_t* list,
    const size_t list_len, char** errptr);

extern ROCKSDB_LIBRARY_API rocksdb_iterator_t*
oxrocksdb_writebatch_wi_create_iterator_with_base_readopts_cf(
    rocksdb_writebatch_wi_t* wbwi, rocksdb_iterator_t* base_iterator,
    const rocksdb_readoptions_t* options, rocksdb_column_family_handle_t* cf);

extern ROCKSDB_LIBRARY_API rocksdb_pinnableslice_t*
oxrocksdb_writebatch_wi_get_pinned_from_batch_and_db_cf(
    rocksdb_writebatch_wi_t* wbwi, rocksdb_t* db,
    const rocksdb_readoptions_t* options,
    rocksdb_column_family_handle_t* column_family, const char* key,
    size_t keylen, char** errptr);  // TODO: remove when targeting 10.3+

extern ROCKSDB_LIBRARY_API rocksdb_readoptions_t*
oxrocksdb_readoptions_create_copy(rocksdb_readoptions_t*);

#ifdef __cplusplus
}
#endif
