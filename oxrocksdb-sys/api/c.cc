#include "c.h"

#include <rocksdb/db.h>
#include <rocksdb/utilities/checkpoint.h>
#include <rocksdb/utilities/transaction_db.h>

#include <vector>

using ROCKSDB_NAMESPACE::ColumnFamilyHandle;
using ROCKSDB_NAMESPACE::DB;
using ROCKSDB_NAMESPACE::IngestExternalFileOptions;
using ROCKSDB_NAMESPACE::PinnableSlice;
using ROCKSDB_NAMESPACE::ReadOptions;
using ROCKSDB_NAMESPACE::Slice;
using ROCKSDB_NAMESPACE::Status;
using ROCKSDB_NAMESPACE::Transaction;
using ROCKSDB_NAMESPACE::TransactionDB;
using std::vector;

// From RocksDB
extern "C" {
struct rocksdb_t {
  DB* rep;
};

struct rocksdb_column_family_handle_t {
  ColumnFamilyHandle* rep;
};

struct rocksdb_ingestexternalfileoptions_t {
  IngestExternalFileOptions rep;
};

struct rocksdb_pinnableslice_t {
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

struct rocksdb_transaction_t {
  Transaction* rep;
};

struct rocksdb_transactiondb_t {
  TransactionDB* rep;
};
}

static void SaveError(char** errptr, const Status& source) {
  if (!source.ok()) {
    *errptr = strdup(source.ToString().c_str());
  }
}

extern "C" {

void rocksdb_transactiondb_ingest_external_files(
    rocksdb_transactiondb_t* db, const rocksdb_ingestexternalfilearg_t* list,
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

rocksdb_pinnableslice_t* rocksdb_transaction_get_for_update_pinned_cf(
    rocksdb_transaction_t* txn, const rocksdb_readoptions_t* options,
    rocksdb_column_family_handle_t* column_family, const char* key,
    size_t keylen, char** errptr) {
  rocksdb_pinnableslice_t* v = new rocksdb_pinnableslice_t;
  Status s = txn->rep->GetForUpdate(options->rep, column_family->rep,
                                    Slice(key, keylen), &v->rep);
  if (!s.ok()) {
    delete v;
    if (!s.IsNotFound()) {
      SaveError(errptr, s);
    }
    return nullptr;
  }
  return v;
}

rocksdb_readoptions_t* rocksdb_readoptions_create_copy(
    rocksdb_readoptions_t* options) {
  return new rocksdb_readoptions_t(*options);
}
}
