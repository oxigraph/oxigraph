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

typedef struct oxrocksdb_pinnable_handle_t oxrocksdb_pinnable_handle_t;

typedef struct oxrocksdb_slice_t {
  const char* data;
  size_t size;
} oxrocksdb_slice_t;

extern ROCKSDB_LIBRARY_API oxrocksdb_pinnable_handle_t*
oxrocksdb_get_pinned_cf_v2(rocksdb_t* db, const rocksdb_readoptions_t* options,
                           rocksdb_column_family_handle_t* column_family,
                           const char* key, size_t keylen, char** errptr);

extern ROCKSDB_LIBRARY_API const char* oxrocksdb_pinnable_handle_get_value(
    const oxrocksdb_pinnable_handle_t* handle, size_t* vallen);

extern ROCKSDB_LIBRARY_API void oxrocksdb_pinnable_handle_destroy(
    oxrocksdb_pinnable_handle_t* handle);

extern ROCKSDB_LIBRARY_API unsigned char oxrocksdb_get_into_buffer_cf(
    rocksdb_t* db, const rocksdb_readoptions_t* options,
    rocksdb_column_family_handle_t* column_family, const char* key,
    size_t keylen, char* buffer, size_t buffer_size, size_t* vallen,
    unsigned char* found, char** errptr);

extern ROCKSDB_LIBRARY_API oxrocksdb_slice_t
oxrocksdb_iter_key_slice(const rocksdb_iterator_t* iter);

extern ROCKSDB_LIBRARY_API unsigned char
oxrocksdb_writebatch_wi_get_into_buffer_cf(
    rocksdb_writebatch_wi_t* wbwi, rocksdb_t* db,
    const rocksdb_readoptions_t* options,
    rocksdb_column_family_handle_t* column_family, const char* key,
    size_t keylen, char* buffer, size_t buffer_size, size_t* vallen,
    unsigned char* found, char** errptr);

extern ROCKSDB_LIBRARY_API rocksdb_readoptions_t*
oxrocksdb_readoptions_create_copy(rocksdb_readoptions_t*);

#ifdef __cplusplus
}
#endif
