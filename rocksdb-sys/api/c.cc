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

void rocksdb_transactiondb_write_writebatch_wi(
        rocksdb_transactiondb_t* db,
        const rocksdb_writeoptions_t* options,
        rocksdb_writebatch_wi_t* wbwi,
        char** errptr) {
    WriteBatch* wb = wbwi->rep->GetWriteBatch();
    SaveError(errptr, db->rep->Write(options->rep, wb));
}

char* rocksdb_transactiondb_writebatch_wi_get_from_batch_and_db_cf(
        rocksdb_writebatch_wi_t* wbwi,
        rocksdb_transactiondb_t* db,
        const rocksdb_readoptions_t* options,
        rocksdb_column_family_handle_t* column_family,
        const char* key, size_t keylen,
        size_t* vallen,
        char** errptr) {
    char* result = nullptr;
    std::string tmp;
    Status s = wbwi->rep->GetFromBatchAndDB(db->rep, options->rep, column_family->rep,
                                            Slice(key, keylen), &tmp);
    if (s.ok()) {
        *vallen = tmp.size();
        result = CopyString(tmp);
    } else {
        *vallen = 0;
        if (!s.IsNotFound()) {
            SaveError(errptr, s);
        }
    }
    return result;
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

rocksdb_writeoptions_t* rocksdb_writeoptions_create_copy(rocksdb_writeoptions_t* options) {
    return new rocksdb_writeoptions_t(*options);
}

}
