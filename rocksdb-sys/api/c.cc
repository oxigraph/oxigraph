#include "../rocksdb/db/c.cc"
#include "c.h"

void rocksdb_transactiondb_flush(
        rocksdb_t* db,
        const rocksdb_flushoptions_t* options,
        char** errptr) {
    SaveError(errptr, db->rep->Flush(options->rep));
}
