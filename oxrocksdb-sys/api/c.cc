#include "../rocksdb/db/c.cc"
#include "c.h"

static bool SaveStatus(rocksdb_status_t* target, const Status source) {
    target->code = static_cast<rocksdb_status_code_t>(source.code());
    target->subcode = static_cast<rocksdb_status_subcode_t>(source.subcode());
    target->severity = static_cast<rocksdb_status_severity_t>(source.severity());
    if(source.ok()) {
        target->string = nullptr;
    } else {
        std::string msg = source.ToString();
        char *string = new char[msg.size() + 1];   //we need extra char for NUL
        memcpy(string, msg.c_str(), msg.size() + 1);
        target->string = string;
    }
    return !source.ok();
}

extern "C" {


rocksdb_transactiondb_t* rocksdb_transactiondb_open_column_families_with_status(
        const rocksdb_options_t* options,
        const rocksdb_transactiondb_options_t* txn_db_options, const char* name,
        int num_column_families, const char* const* column_family_names,
        const rocksdb_options_t* const* column_family_options,
        rocksdb_column_family_handle_t** column_family_handles, rocksdb_status_t* statusptr) {
    std::vector<ColumnFamilyDescriptor> column_families;
    for (int i = 0; i < num_column_families; i++) {
        column_families.push_back(ColumnFamilyDescriptor(
                std::string(column_family_names[i]),
                ColumnFamilyOptions(column_family_options[i]->rep)));
    }

    TransactionDB* txn_db;
    std::vector<ColumnFamilyHandle*> handles;
    if (SaveStatus(statusptr, TransactionDB::Open(options->rep, txn_db_options->rep,
                                              std::string(name), column_families,
                                              &handles, &txn_db))) {
        return nullptr;
    }

    for (size_t i = 0; i < handles.size(); i++) {
        rocksdb_column_family_handle_t* c_handle =
                new rocksdb_column_family_handle_t;
        c_handle->rep = handles[i];
        column_family_handles[i] = c_handle;
    }
    rocksdb_transactiondb_t* result = new rocksdb_transactiondb_t;
    result->rep = txn_db;
    return result;
}

rocksdb_pinnableslice_t* rocksdb_transactiondb_get_pinned_cf_with_status(
        rocksdb_transactiondb_t* db, const rocksdb_readoptions_t* options,
        rocksdb_column_family_handle_t* column_family, const char* key,
        size_t keylen, rocksdb_status_t* statusptr) {
    rocksdb_pinnableslice_t* v = new rocksdb_pinnableslice_t;
    Status s = db->rep->Get(options->rep, column_family->rep, Slice(key, keylen),
                            &v->rep);
    if (!s.ok()) {
        delete v;
        if (!s.IsNotFound()) {
            SaveStatus(statusptr, s);
        }
        return nullptr;
    }
    return v;
}

void rocksdb_transactiondb_put_cf_with_status(rocksdb_transactiondb_t* txn_db,
                                  const rocksdb_writeoptions_t* options,
                                  rocksdb_column_family_handle_t* column_family,
                                  const char* key, size_t keylen,
                                  const char* val, size_t vallen,
                                  rocksdb_status_t* statusptr) {
    SaveStatus(statusptr, txn_db->rep->Put(options->rep, column_family->rep,
                                       Slice(key, keylen), Slice(val, vallen)));
}

void rocksdb_transactiondb_flush_cf_with_status(
        rocksdb_transactiondb_t* db,
        const rocksdb_flushoptions_t* options,
        rocksdb_column_family_handle_t* column_family,
        rocksdb_status_t* statusptr) {
    SaveStatus(statusptr, db->rep->Flush(options->rep, column_family->rep));
}

void rocksdb_transactiondb_compact_range_cf_opt_with_status(rocksdb_transactiondb_t* db,
                                  rocksdb_column_family_handle_t* column_family,
                                  rocksdb_compactoptions_t* opt,
                                  const char* start_key, size_t start_key_len,
                                  const char* limit_key, size_t limit_key_len,
                                  rocksdb_status_t* statusptr) {
    Slice a, b;
    SaveStatus(statusptr, db->rep->CompactRange(
            opt->rep, column_family->rep,
            // Pass nullptr Slice if corresponding "const char*" is nullptr
            (start_key ? (a = Slice(start_key, start_key_len), &a) : nullptr),
            (limit_key ? (b = Slice(limit_key, limit_key_len), &b) : nullptr)));
}

void rocksdb_transactiondb_ingest_external_files_with_status(
        rocksdb_transactiondb_t* db, const rocksdb_ingestexternalfilearg_t* list,
        const size_t list_len, rocksdb_status_t* statusptr) {
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
    SaveStatus(statusptr, db->rep->IngestExternalFiles(args));
}

void rocksdb_transactiondb_create_checkpoint_with_status(
        rocksdb_transactiondb_t* db, const char* checkpoint_dir,
        rocksdb_status_t* statusptr) {
    Checkpoint* checkpoint;
    Status s = Checkpoint::Create(db->rep, &checkpoint);
    if (!s.ok()) {
        SaveStatus(statusptr, s);
        return;
    }
    SaveStatus(statusptr, checkpoint->CreateCheckpoint(std::string(checkpoint_dir)));
    delete checkpoint;
}


void rocksdb_transaction_commit_with_status(rocksdb_transaction_t* txn, rocksdb_status_t* statusptr) {
    SaveStatus(statusptr, txn->rep->Commit());
}

void rocksdb_transaction_rollback_with_status(rocksdb_transaction_t* txn, rocksdb_status_t* statusptr) {
    SaveStatus(statusptr, txn->rep->Rollback());
}

rocksdb_pinnableslice_t* rocksdb_transaction_get_pinned_cf_with_status(
        rocksdb_transaction_t* txn, const rocksdb_readoptions_t* options,
        rocksdb_column_family_handle_t* column_family, const char* key,
        size_t keylen, rocksdb_status_t* statusptr) {
    rocksdb_pinnableslice_t* v = new rocksdb_pinnableslice_t;
    Status s = txn->rep->Get(options->rep, column_family->rep, Slice(key, keylen),
                            &v->rep);
    if (!s.ok()) {
        delete v;
        if (!s.IsNotFound()) {
            SaveStatus(statusptr, s);
        }
        return nullptr;
    }
    return v;
}

rocksdb_pinnableslice_t* rocksdb_transaction_get_for_update_pinned_cf_with_status(
        rocksdb_transaction_t* txn, const rocksdb_readoptions_t* options,
        rocksdb_column_family_handle_t* column_family, const char* key,
        size_t keylen, rocksdb_status_t* statusptr) {
    rocksdb_pinnableslice_t* v = new rocksdb_pinnableslice_t;
    Status s = txn->rep->GetForUpdate(options->rep, column_family->rep, Slice(key, keylen),
                             &v->rep);
    if (!s.ok()) {
        delete v;
        if (!s.IsNotFound()) {
            SaveStatus(statusptr, s);
        }
        return nullptr;
    }
    return v;
}

void rocksdb_transaction_put_cf_with_status(rocksdb_transaction_t* txn,
                                rocksdb_column_family_handle_t* column_family,
                                const char* key, size_t klen, const char* val,
                                size_t vlen, rocksdb_status_t* statusptr) {
    SaveStatus(statusptr, txn->rep->Put(column_family->rep, Slice(key, klen),
                                    Slice(val, vlen)));
}

void rocksdb_transaction_delete_cf_with_status(
        rocksdb_transaction_t* txn, rocksdb_column_family_handle_t* column_family,
        const char* key, size_t klen, rocksdb_status_t* statusptr) {
    SaveStatus(statusptr, txn->rep->Delete(column_family->rep, Slice(key, klen)));
}


void rocksdb_sstfilewriter_open_with_status(rocksdb_sstfilewriter_t* writer,
                                            const char* name, rocksdb_status_t* statusptr) {
    SaveStatus(statusptr, writer->rep->Open(std::string(name)));
}


void rocksdb_sstfilewriter_put_with_status(rocksdb_sstfilewriter_t* writer, const char* key,
                               size_t keylen, const char* val, size_t vallen,
                               rocksdb_status_t* statusptr) {
    SaveStatus(statusptr, writer->rep->Put(Slice(key, keylen), Slice(val, vallen)));
}

void rocksdb_sstfilewriter_finish_with_status(rocksdb_sstfilewriter_t* writer,
                                              rocksdb_status_t* statusptr) {
    SaveStatus(statusptr, writer->rep->Finish(nullptr));
}


void rocksdb_iter_get_status(const rocksdb_iterator_t* iter, rocksdb_status_t* statusptr) {
    SaveStatus(statusptr, iter->rep->status());
}


rocksdb_readoptions_t* rocksdb_readoptions_create_copy(rocksdb_readoptions_t* options) {
    return new rocksdb_readoptions_t(*options);
}

}
