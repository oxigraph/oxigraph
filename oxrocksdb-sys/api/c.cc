#include "c.h"

#include <rocksdb/db.h>
#include <rocksdb/utilities/write_batch_with_index.h>

#include <cstring>
#include <vector>

using ROCKSDB_NAMESPACE::ColumnFamilyHandle;
using ROCKSDB_NAMESPACE::DB;
using ROCKSDB_NAMESPACE::IngestExternalFileOptions;
using ROCKSDB_NAMESPACE::Iterator;
using ROCKSDB_NAMESPACE::PinnableSlice;
using ROCKSDB_NAMESPACE::ReadOptions;
using ROCKSDB_NAMESPACE::Slice;
using ROCKSDB_NAMESPACE::Status;
using ROCKSDB_NAMESPACE::WriteBatch;
using ROCKSDB_NAMESPACE::WriteBatchWithIndex;
using std::vector;

// From RocksDB
extern "C" {
struct rocksdb_t {
  DB* rep;
};

struct rocksdb_iterator_t {
  Iterator* rep;
};

struct rocksdb_column_family_handle_t {
  ColumnFamilyHandle* rep;
};

struct rocksdb_writebatch_wi_t {
  WriteBatchWithIndex* rep;
};

struct rocksdb_ingestexternalfileoptions_t {
  IngestExternalFileOptions rep;
};

struct oxrocksdb_pinnable_handle_t {
  PinnableSlice rep;
};

struct rocksdb_readoptions_t {
  ReadOptions rep;
  // stack variables to set pointers to in ReadOptions
  Slice upper_bound;
  Slice lower_bound;
  Slice timestamp;
  Slice iter_start_ts;
};
}

static void SaveError(char** errptr, const Status& source) {
  if (!source.ok()) {
    *errptr = strdup(source.ToString().c_str());
  }
}

extern "C" {

void oxrocksdb_ingest_external_files(
    rocksdb_t* db, const rocksdb_ingestexternalfilearg_t* list,
    const size_t list_len, char** errptr) {
  vector<rocksdb::IngestExternalFileArg> args(list_len);
  for (size_t i = 0; i < list_len; ++i) {
    args[i].column_family = list[i].column_family->rep;
    vector<std::string> files(list[i].external_files_len);
    for (size_t j = 0; j < list[i].external_files_len; ++j) {
      files[j] = std::string(list[i].external_files[j]);
    }
    args[i].external_files = files;
    args[i].options = list[i].options->rep;
  }
  SaveError(errptr, db->rep->IngestExternalFiles(args));
}

rocksdb_iterator_t*
oxrocksdb_writebatch_wi_create_iterator_with_base_readopts_cf(
    rocksdb_writebatch_wi_t* wbwi, rocksdb_iterator_t* base_iterator,
    const rocksdb_readoptions_t* options, rocksdb_column_family_handle_t* cf) {
  rocksdb_iterator_t* result = new rocksdb_iterator_t;
  result->rep = wbwi->rep->NewIteratorWithBase(cf->rep, base_iterator->rep,
                                               &options->rep);
  delete base_iterator;
  return result;
}

oxrocksdb_pinnable_handle_t* oxrocksdb_get_pinned_cf_v2(
    rocksdb_t* db, const rocksdb_readoptions_t* options,
    rocksdb_column_family_handle_t* column_family, const char* key,
    size_t keylen, char** errptr) {
  oxrocksdb_pinnable_handle_t* handle = new (oxrocksdb_pinnable_handle_t);
  Status s = db->rep->Get(options->rep, column_family->rep, Slice(key, keylen),
                          &handle->rep);
  if (!s.ok()) {
    delete handle;
    if (!s.IsNotFound()) {
      SaveError(errptr, s);
    }
    return nullptr;
  }
  return handle;
}

const char* oxrocksdb_pinnable_handle_get_value(
    const oxrocksdb_pinnable_handle_t* handle, size_t* vallen) {
  if (!handle) {
    *vallen = 0;
    return nullptr;
  }
  *vallen = handle->rep.size();
  return handle->rep.data();
}

void oxrocksdb_pinnable_handle_destroy(oxrocksdb_pinnable_handle_t* handle) {
  delete handle;
}

unsigned char oxrocksdb_get_into_buffer_cf(
    rocksdb_t* db, const rocksdb_readoptions_t* options,
    rocksdb_column_family_handle_t* column_family, const char* key,
    size_t keylen, char* buffer, size_t buffer_size, size_t* vallen,
    unsigned char* found, char** errptr) {
  PinnableSlice pinnable_val;
  Status s = db->rep->Get(options->rep, column_family->rep, Slice(key, keylen),
                          &pinnable_val);
  if (s.ok()) {
    *found = 1;
    *vallen = pinnable_val.size();
    if (buffer_size >= pinnable_val.size()) {
      memcpy(buffer, pinnable_val.data(), pinnable_val.size());
      return 1;
    }
    return 0;
  } else {
    *found = 0;
    *vallen = 0;
    if (!s.IsNotFound()) {
      SaveError(errptr, s);
    }
    return 0;
  }
}

oxrocksdb_slice_t oxrocksdb_iter_key_slice(const rocksdb_iterator_t* iter) {
  const Slice key = iter->rep->key();
  return oxrocksdb_slice_t{key.data(), key.size()};
}

oxrocksdb_pinnable_handle_t*
oxrocksdb_writebatch_wi_get_pinned_from_batch_and_db_cf(
    rocksdb_writebatch_wi_t* wbwi, rocksdb_t* db,
    const rocksdb_readoptions_t* options,
    rocksdb_column_family_handle_t* column_family, const char* key,
    size_t keylen, char** errptr) {
  oxrocksdb_pinnable_handle_t* v = new (oxrocksdb_pinnable_handle_t);
  Status s = wbwi->rep->GetFromBatchAndDB(
      db->rep, options->rep, column_family->rep, Slice(key, keylen), &v->rep);
  if (!s.ok()) {
    delete (v);
    if (!s.IsNotFound()) {
      SaveError(errptr, s);
    }
    return nullptr;
  }
  return v;
}
rocksdb_readoptions_t* oxrocksdb_readoptions_create_copy(
    rocksdb_readoptions_t* options) {
  return new rocksdb_readoptions_t(*options);
}
}
