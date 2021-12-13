#include "../rocksdb/db/c.cc"
#include "c.h"

extern "C" {

rocksdb_pinnableslice_t* rocksdb_transactiondb_get_pinned_cf(
        rocksdb_transactiondb_t* db, const rocksdb_readoptions_t* options,
        rocksdb_column_family_handle_t* column_family, const char* key,
        size_t keylen, char** errptr) {
    rocksdb_pinnableslice_t* v = new (rocksdb_pinnableslice_t);
    Status s = db->rep->Get(options->rep, column_family->rep, Slice(key, keylen),
                            &v->rep);
    if (!s.ok()) {
        delete v;
        if (!s.IsNotFound()) {
            SaveError(errptr, s);
        }
        return nullptr;
    }
    return v;
}

void rocksdb_transactiondb_flush_cf(
        rocksdb_transactiondb_t* db,
        const rocksdb_flushoptions_t* options,
        rocksdb_column_family_handle_t* column_family,
        char** errptr) {
    SaveError(errptr, db->rep->Flush(options->rep, column_family->rep));
}

void rocksdb_transactiondb_compact_range_cf_opt(rocksdb_transactiondb_t* db,
                                  rocksdb_column_family_handle_t* column_family,
                                  rocksdb_compactoptions_t* opt,
                                  const char* start_key, size_t start_key_len,
                                  const char* limit_key, size_t limit_key_len,
                                  char** errptr) {
    Slice a, b;
    SaveError(errptr, db->rep->CompactRange(
            opt->rep, column_family->rep,
            // Pass nullptr Slice if corresponding "const char*" is nullptr
            (start_key ? (a = Slice(start_key, start_key_len), &a) : nullptr),
            (limit_key ? (b = Slice(limit_key, limit_key_len), &b) : nullptr)));
}

void rocksdb_transactiondb_ingest_external_file_cf(
        rocksdb_transactiondb_t* db, rocksdb_column_family_handle_t* handle,
        const char* const* file_list, const size_t list_len,
        const rocksdb_ingestexternalfileoptions_t* opt, char** errptr) {
    std::vector<std::string> files(list_len);
    for (size_t i = 0; i < list_len; ++i) {
        files[i] = std::string(file_list[i]);
    }
    SaveError(errptr, db->rep->IngestExternalFile(handle->rep, files, opt->rep));
}

void rocksdb_transactiondb_ingest_external_files(
        rocksdb_transactiondb_t* db, const rocksdb_ingestexternalfilearg_t* list,
        const size_t list_len, char** errptr) {
    std::vector<rocksdb::IngestExternalFileArg> args(list_len);
    for (size_t i = 0; i < list_len; ++i) {
        args[i].column_family = list[i].column_family->rep;
        std::vector<std::string> files(list[i].external_files_len);
        for (size_t j = 0; j < list[i].external_files_len; ++j) {
            files[j] = std::string(list[i].external_files[j]);
        }
        args[i].external_files = files;
        args[i].options = list[i].options->rep;
    }
    SaveError(errptr, db->rep->IngestExternalFiles(args));
}

rocksdb_pinnableslice_t* rocksdb_transaction_get_pinned_cf(
        rocksdb_transaction_t* txn, const rocksdb_readoptions_t* options,
        rocksdb_column_family_handle_t* column_family, const char* key,
        size_t keylen, char** errptr) {
    rocksdb_pinnableslice_t* v = new (rocksdb_pinnableslice_t);
    Status s = txn->rep->Get(options->rep, column_family->rep, Slice(key, keylen),
                            &v->rep);
    if (!s.ok()) {
        delete v;
        if (!s.IsNotFound()) {
            SaveError(errptr, s);
        }
        return nullptr;
    }
    return v;
}

rocksdb_pinnableslice_t* rocksdb_transaction_get_for_update_pinned_cf(
        rocksdb_transaction_t* txn, const rocksdb_readoptions_t* options,
        rocksdb_column_family_handle_t* column_family, const char* key,
        size_t keylen, char** errptr) {
    rocksdb_pinnableslice_t* v = new (rocksdb_pinnableslice_t);
    Status s = txn->rep->GetForUpdate(options->rep, column_family->rep, Slice(key, keylen),
                             &v->rep);
    if (!s.ok()) {
        delete v;
        if (!s.IsNotFound()) {
            SaveError(errptr, s);
        }
        return nullptr;
    }
    return v;
}

rocksdb_readoptions_t* rocksdb_readoptions_create_copy(rocksdb_readoptions_t* options) {
    return new rocksdb_readoptions_t(*options);
}

}
