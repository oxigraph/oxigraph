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

rocksdb_writeoptions_t* rocksdb_writeoptions_create_copy(rocksdb_writeoptions_t* options) {
    return new rocksdb_writeoptions_t(*options);
}

}
