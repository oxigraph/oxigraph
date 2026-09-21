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

extern ROCKSDB_LIBRARY_API oxrocksdb_pinnable_handle_t*
oxrocksdb_writebatch_wi_get_pinned_cf_v2(
    rocksdb_writebatch_wi_t* wbwi, rocksdb_t* db,
    const rocksdb_readoptions_t* options,
    rocksdb_column_family_handle_t* column_family, const char* key,
    size_t keylen, char** errptr);

extern ROCKSDB_LIBRARY_API rocksdb_readoptions_t*
oxrocksdb_readoptions_create_copy(rocksdb_readoptions_t*);

typedef struct oxrocksdb_compaction_service_options_override_t
    oxrocksdb_compaction_service_options_override_t;

extern ROCKSDB_LIBRARY_API oxrocksdb_compaction_service_options_override_t*
oxrocksdb_compaction_service_options_override_create(void);

extern ROCKSDB_LIBRARY_API oxrocksdb_compaction_service_options_override_t*
oxrocksdb_compaction_service_options_override_create_from_options(
    rocksdb_options_t* option);

extern ROCKSDB_LIBRARY_API void
oxrocksdb_compaction_service_options_override_destroy(
    oxrocksdb_compaction_service_options_override_t* override_options);

typedef struct oxrocksdb_table_properties_t oxrocksdb_table_properties_t;
typedef struct oxrocksdb_user_collected_properties_t
    oxrocksdb_user_collected_properties_t;
typedef struct oxrocksdb_user_collected_properties_iterator_t
    oxrocksdb_user_collected_properties_iterator_t;
typedef struct oxrocksdb_table_properties_collector_t
    oxrocksdb_table_properties_collector_t;
typedef struct oxrocksdb_table_properties_collector_factory_t
    oxrocksdb_table_properties_collector_factory_t;

extern ROCKSDB_LIBRARY_API const oxrocksdb_user_collected_properties_t*
oxrocksdb_table_properties_get_user_properties(
    const oxrocksdb_table_properties_t* props);

extern ROCKSDB_LIBRARY_API void oxrocksdb_user_collected_properties_add(
    oxrocksdb_user_collected_properties_t* props, const char* k, size_t klen,
    const char* v, size_t vlen);

extern ROCKSDB_LIBRARY_API const char* oxrocksdb_user_collected_properties_get(
    const oxrocksdb_user_collected_properties_t* props, const char* key,
    size_t klen, size_t* vlen);

extern ROCKSDB_LIBRARY_API size_t oxrocksdb_user_collected_properties_len(
    const oxrocksdb_user_collected_properties_t* props);

extern ROCKSDB_LIBRARY_API oxrocksdb_user_collected_properties_iterator_t*
oxrocksdb_user_collected_properties_iter_create(
    const oxrocksdb_user_collected_properties_t* props);

extern ROCKSDB_LIBRARY_API void
oxrocksdb_user_collected_properties_iter_destroy(
    oxrocksdb_user_collected_properties_iterator_t* it);

extern ROCKSDB_LIBRARY_API unsigned char
oxrocksdb_user_collected_properties_iter_valid(
    const oxrocksdb_user_collected_properties_iterator_t* it);

extern ROCKSDB_LIBRARY_API void oxrocksdb_user_collected_properties_iter_next(
    oxrocksdb_user_collected_properties_iterator_t* it);

extern ROCKSDB_LIBRARY_API const char*
oxrocksdb_user_collected_properties_iter_key(
    const oxrocksdb_user_collected_properties_iterator_t* it, size_t* klen);

extern ROCKSDB_LIBRARY_API const char*
oxrocksdb_user_collected_properties_iter_value(
    const oxrocksdb_user_collected_properties_iterator_t* it, size_t* vlen);

extern ROCKSDB_LIBRARY_API void
oxrocksdb_compaction_service_options_override_add_table_properties_collector_factory(
    oxrocksdb_compaction_service_options_override_t* override_options,
    oxrocksdb_table_properties_collector_factory_t* factory);

extern ROCKSDB_LIBRARY_API oxrocksdb_table_properties_collector_t*
oxrocksdb_table_properties_collector_create(
    void* state, const char* (*name)(void*), void (*destruct)(void*),
    void (*add)(void*, const char* key, size_t key_len, const char* value,
                size_t value_len, uint32_t entry_type, uint64_t seq,
                uint64_t file_size),
    void (*finish)(void*, oxrocksdb_user_collected_properties_t* props));

extern ROCKSDB_LIBRARY_API void oxrocksdb_table_properties_collector_destroy(
    oxrocksdb_table_properties_collector_t* c);

extern ROCKSDB_LIBRARY_API oxrocksdb_table_properties_collector_factory_t*
oxrocksdb_table_properties_collector_factory_create(
    void* state, const char* (*name)(void*), void (*destruct)(void*),
    oxrocksdb_table_properties_collector_t* (
        *create_table_properties_collector)(void*, uint32_t cf));

extern ROCKSDB_LIBRARY_API void
oxrocksdb_table_properties_collector_factory_destroy(
    oxrocksdb_table_properties_collector_factory_t* factory);

extern ROCKSDB_LIBRARY_API void
oxrocksdb_options_add_table_properties_collector_factory(
    rocksdb_options_t*, oxrocksdb_table_properties_collector_factory_t*);

typedef struct oxrocksdb_table_properties_collection_t
    oxrocksdb_table_properties_collection_t;
typedef struct oxrocksdb_table_properties_collection_iterator_t
    oxrocksdb_table_properties_collection_iterator_t;
typedef struct oxrocksdb_eventlistener_t oxrocksdb_eventlistener_t;

extern ROCKSDB_LIBRARY_API size_t oxrocksdb_table_properties_collection_len(
    const oxrocksdb_table_properties_collection_t* props);

extern ROCKSDB_LIBRARY_API void oxrocksdb_table_properties_collection_destroy(
    oxrocksdb_table_properties_collection_t* props);

extern ROCKSDB_LIBRARY_API oxrocksdb_table_properties_collection_iterator_t*
oxrocksdb_table_properties_collection_iter_create(
    const oxrocksdb_table_properties_collection_t* collection);

extern ROCKSDB_LIBRARY_API void
oxrocksdb_table_properties_collection_iter_destroy(
    oxrocksdb_table_properties_collection_iterator_t* it);

extern ROCKSDB_LIBRARY_API unsigned char
oxrocksdb_table_properties_collection_iter_valid(
    const oxrocksdb_table_properties_collection_iterator_t* it);

extern ROCKSDB_LIBRARY_API void
oxrocksdb_table_properties_collection_iter_next(
    oxrocksdb_table_properties_collection_iterator_t* it);

extern ROCKSDB_LIBRARY_API const char*
oxrocksdb_table_properties_collection_iter_key(
    const oxrocksdb_table_properties_collection_iterator_t* it, size_t* klen);

extern ROCKSDB_LIBRARY_API const oxrocksdb_table_properties_t*
oxrocksdb_table_properties_collection_iter_value(
    const oxrocksdb_table_properties_collection_iterator_t* it);

extern ROCKSDB_LIBRARY_API oxrocksdb_table_properties_collection_t*
oxrocksdb_get_properties_of_all_tables_cf(rocksdb_t* db,
                                          rocksdb_column_family_handle_t* cf,
                                          char** errptr);

extern ROCKSDB_LIBRARY_API const oxrocksdb_table_properties_t*
oxrocksdb_flushjobinfo_table_properties(const rocksdb_flushjobinfo_t* info);

extern ROCKSDB_LIBRARY_API const oxrocksdb_table_properties_collection_t*
oxrocksdb_compactionjobinfo_table_properties(
    const rocksdb_compactionjobinfo_t* info);

extern ROCKSDB_LIBRARY_API oxrocksdb_eventlistener_t*
oxrocksdb_eventlistener_create(
    void* state, void (*destructor)(void*),
    void (*on_flush_begin)(void*, rocksdb_t*, const rocksdb_flushjobinfo_t*),
    void (*on_flush_completed)(void*, rocksdb_t*,
                               const rocksdb_flushjobinfo_t*),
    void (*on_compaction_begin)(void*, rocksdb_t*,
                                const rocksdb_compactionjobinfo_t*),
    void (*on_compaction_completed)(void*, rocksdb_t*,
                                    const rocksdb_compactionjobinfo_t*));

extern ROCKSDB_LIBRARY_API void oxrocksdb_eventlistener_destroy(
    oxrocksdb_eventlistener_t* t);

extern ROCKSDB_LIBRARY_API void oxrocksdb_options_add_eventlistener(
    rocksdb_options_t* opt, oxrocksdb_eventlistener_t* t);

#ifdef __cplusplus
}
#endif
