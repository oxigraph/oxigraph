#pragma once

#include "../rocksdb/include/rocksdb/c.h"

extern ROCKSDB_LIBRARY_API void rocksdb_transactiondb_flush(
        rocksdb_transactiondb_t* db, const rocksdb_flushoptions_t* options, char** errptr);