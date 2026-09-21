#include "c.h"

#include <rocksdb/db.h>
#include <rocksdb/listener.h>
#include <rocksdb/table_properties.h>
#include <rocksdb/utilities/write_batch_with_index.h>

#include <cstring>
#include <memory>
#include <vector>

using ROCKSDB_NAMESPACE::ColumnFamilyHandle;
using ROCKSDB_NAMESPACE::CompactionJobInfo;
using ROCKSDB_NAMESPACE::CompactionServiceOptionsOverride;
using ROCKSDB_NAMESPACE::DB;
using ROCKSDB_NAMESPACE::EntryType;
using ROCKSDB_NAMESPACE::EventListener;
using ROCKSDB_NAMESPACE::FlushJobInfo;
using ROCKSDB_NAMESPACE::IngestExternalFileOptions;
using ROCKSDB_NAMESPACE::Iterator;
using ROCKSDB_NAMESPACE::Options;
using ROCKSDB_NAMESPACE::PinnableSlice;
using ROCKSDB_NAMESPACE::ReadOptions;
using ROCKSDB_NAMESPACE::SequenceNumber;
using ROCKSDB_NAMESPACE::Slice;
using ROCKSDB_NAMESPACE::Status;
using ROCKSDB_NAMESPACE::TableProperties;
using ROCKSDB_NAMESPACE::TablePropertiesCollection;
using ROCKSDB_NAMESPACE::TablePropertiesCollector;
using ROCKSDB_NAMESPACE::TablePropertiesCollectorFactory;
using ROCKSDB_NAMESPACE::UserCollectedProperties;
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

unsigned char oxrocksdb_writebatch_wi_get_into_buffer_cf(
    rocksdb_writebatch_wi_t* wbwi, rocksdb_t* db,
    const rocksdb_readoptions_t* options,
    rocksdb_column_family_handle_t* column_family, const char* key,
    size_t keylen, char* buffer, size_t buffer_size, size_t* vallen,
    unsigned char* found, char** errptr) {
  PinnableSlice pinnable_val;
  Status s =
      wbwi->rep->GetFromBatchAndDB(db->rep, options->rep, column_family->rep,
                                   Slice(key, keylen), &pinnable_val);
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

oxrocksdb_pinnable_handle_t* oxrocksdb_writebatch_wi_get_pinned_cf_v2(
    rocksdb_writebatch_wi_t* wbwi, rocksdb_t* db,
    const rocksdb_readoptions_t* options,
    rocksdb_column_family_handle_t* column_family, const char* key,
    size_t keylen, char** errptr) {
  oxrocksdb_pinnable_handle_t* handle = new (oxrocksdb_pinnable_handle_t);
  Status s =
      wbwi->rep->GetFromBatchAndDB(db->rep, options->rep, column_family->rep,
                                   Slice(key, keylen), &handle->rep);
  if (!s.ok()) {
    delete handle;
    if (!s.IsNotFound()) {
      SaveError(errptr, s);
    }
    return nullptr;
  }
  return handle;
}

rocksdb_readoptions_t* oxrocksdb_readoptions_create_copy(
    rocksdb_readoptions_t* options) {
  return new rocksdb_readoptions_t(*options);
}

// From RocksDB
struct rocksdb_options_t {
  Options rep;
};

struct rocksdb_flushjobinfo_t {
  FlushJobInfo rep;
};

struct rocksdb_compactionjobinfo_t {
  CompactionJobInfo rep;
};

struct oxrocksdb_table_properties_t {
  const TableProperties rep;
};

struct oxrocksdb_table_properties_collection_t {
  TablePropertiesCollection rep;
};

struct oxrocksdb_table_properties_collection_iterator_t {
  TablePropertiesCollection::const_iterator cur_;
  TablePropertiesCollection::const_iterator end_;
};

struct oxrocksdb_user_collected_properties_t {
  UserCollectedProperties rep;
};

struct oxrocksdb_user_collected_properties_iterator_t {
  UserCollectedProperties::const_iterator cur_;
  UserCollectedProperties::const_iterator end_;
};


struct oxrocksdb_compaction_service_options_override_t {
  CompactionServiceOptionsOverride rep;
};

// RocksDB deletes TablePropertiesCollector* returned by the factory, so the
// C handle is the collector itself (same pattern as rocksdb_compactionfilter_t).
struct oxrocksdb_table_properties_collector_t
    : public TablePropertiesCollector {
  void* state_;
  const char* (*name_)(void*);
  void (*destruct_)(void*);
  void (*add_)(void*, const char* key, size_t key_len, const char* value,
               size_t value_len, uint32_t entry_type, uint64_t seq,
               uint64_t file_size);
  void (*finish_)(void*, oxrocksdb_user_collected_properties_t* props);

  oxrocksdb_table_properties_collector_t(
      void* state, const char* (*name)(void*), void (*destruct)(void*),
      void (*add)(void*, const char* key, size_t key_len, const char* value,
                  size_t value_len, uint32_t entry_type, uint64_t seq,
                  uint64_t file_size),
      void (*finish)(void*, oxrocksdb_user_collected_properties_t* props))
      : state_(state),
        name_(name),
        destruct_(destruct),
        add_(add),
        finish_(finish) {}

  ~oxrocksdb_table_properties_collector_t() override { destruct_(state_); }

  Status AddUserKey(const Slice& key, const Slice& value, EntryType entry_type,
                    SequenceNumber seq, uint64_t file_size) override {
    add_(state_, key.data(), key.size(), value.data(), value.size(),
         static_cast<uint32_t>(entry_type), seq, file_size);
    return Status::OK();
  }

  Status Finish(UserCollectedProperties* properties) override {
    finish_(state_,
            reinterpret_cast<oxrocksdb_user_collected_properties_t*>(
                properties));
    return Status::OK();
  }

  UserCollectedProperties GetReadableProperties() const override {
    return UserCollectedProperties();
  }

  const char* Name() const override { return name_(state_); }
};

// Concrete factory stored in a shared_ptr, matching
// rocksdb_table_properties_collector_factory_t in RocksDB's C API.
struct OxTablePropertiesCollectorFactory
    : public TablePropertiesCollectorFactory {
  void* state_;
  const char* (*name_)(void*);
  void (*destruct_)(void*);
  oxrocksdb_table_properties_collector_t* (*create_table_properties_collector_)(
      void*, uint32_t cf);

  OxTablePropertiesCollectorFactory(
      void* state, const char* (*name)(void*), void (*destruct)(void*),
      oxrocksdb_table_properties_collector_t* (
          *create_table_properties_collector)(void*, uint32_t cf))
      : state_(state),
        name_(name),
        destruct_(destruct),
        create_table_properties_collector_(create_table_properties_collector) {}

  ~OxTablePropertiesCollectorFactory() override { destruct_(state_); }

  TablePropertiesCollector* CreateTablePropertiesCollector(
      TablePropertiesCollectorFactory::Context ctx) override {
    return create_table_properties_collector_(state_, ctx.column_family_id);
  }

  const char* Name() const override { return name_(state_); }
};

struct oxrocksdb_table_properties_collector_factory_t {
  std::shared_ptr<TablePropertiesCollectorFactory> rep;
};

struct OxEventListener : public EventListener {
  void* state_;
  void (*destructor_)(void*);
  void (*on_flush_begin_)(void*, rocksdb_t*, const rocksdb_flushjobinfo_t*);
  void (*on_flush_completed_)(void*, rocksdb_t*,
                              const rocksdb_flushjobinfo_t*);
  void (*on_compaction_begin_)(void*, rocksdb_t*,
                               const rocksdb_compactionjobinfo_t*);
  void (*on_compaction_completed_)(void*, rocksdb_t*,
                                   const rocksdb_compactionjobinfo_t*);

  OxEventListener(void* state, void (*destructor)(void*),
                  void (*on_flush_begin)(void*, rocksdb_t*,
                                         const rocksdb_flushjobinfo_t*),
                  void (*on_flush_completed)(void*, rocksdb_t*,
                                             const rocksdb_flushjobinfo_t*),
                  void (*on_compaction_begin)(void*, rocksdb_t*,
                                              const rocksdb_compactionjobinfo_t*),
                  void (*on_compaction_completed)(
                      void*, rocksdb_t*, const rocksdb_compactionjobinfo_t*))
      : state_(state),
        destructor_(destructor),
        on_flush_begin_(on_flush_begin),
        on_flush_completed_(on_flush_completed),
        on_compaction_begin_(on_compaction_begin),
        on_compaction_completed_(on_compaction_completed) {}

  ~OxEventListener() override {
    if (destructor_) {
      destructor_(state_);
    }
  }

  void OnFlushBegin(DB* db, const FlushJobInfo& info) override {
    if (!on_flush_begin_) {
      return;
    }
    rocksdb_t c_db = {db};
    on_flush_begin_(state_, &c_db,
                    reinterpret_cast<const rocksdb_flushjobinfo_t*>(&info));
  }

  void OnFlushCompleted(DB* db, const FlushJobInfo& info) override {
    if (!on_flush_completed_) {
      return;
    }
    rocksdb_t c_db = {db};
    on_flush_completed_(state_, &c_db,
                        reinterpret_cast<const rocksdb_flushjobinfo_t*>(&info));
  }

  void OnCompactionBegin(DB* db, const CompactionJobInfo& info) override {
    if (!on_compaction_begin_) {
      return;
    }
    rocksdb_t c_db = {db};
    on_compaction_begin_(
        state_, &c_db,
        reinterpret_cast<const rocksdb_compactionjobinfo_t*>(&info));
  }

  void OnCompactionCompleted(DB* db, const CompactionJobInfo& info) override {
    if (!on_compaction_completed_) {
      return;
    }
    rocksdb_t c_db = {db};
    on_compaction_completed_(
        state_, &c_db,
        reinterpret_cast<const rocksdb_compactionjobinfo_t*>(&info));
  }
};

struct oxrocksdb_eventlistener_t {
  std::shared_ptr<EventListener> rep;
};

// Table Properties functions

const oxrocksdb_user_collected_properties_t*
oxrocksdb_table_properties_get_user_properties(
    const oxrocksdb_table_properties_t* props) {
  return reinterpret_cast<const oxrocksdb_user_collected_properties_t*>(
      &props->rep.user_collected_properties);
}


// User Collected Properties functions

void oxrocksdb_user_collected_properties_add(
    oxrocksdb_user_collected_properties_t* props, const char* k, size_t klen,
    const char* v, size_t vlen) {
  props->rep.emplace(
      std::make_pair(std::string(k, klen), std::string(v, vlen)));
}

const char* oxrocksdb_user_collected_properties_get(
    const oxrocksdb_user_collected_properties_t* props, const char* key,
    size_t klen, size_t* vlen) {
  auto val = props->rep.find(std::string(key, klen));
  if (val == props->rep.end()) {
    return nullptr;
  }
  *vlen = val->second.size();
  return val->second.data();
}

size_t oxrocksdb_user_collected_properties_len(
    const oxrocksdb_user_collected_properties_t* props) {
  return props->rep.size();
}

oxrocksdb_user_collected_properties_iterator_t*
oxrocksdb_user_collected_properties_iter_create(
    const oxrocksdb_user_collected_properties_t* props) {
  auto it = new oxrocksdb_user_collected_properties_iterator_t;
  it->cur_ = props->rep.begin();
  it->end_ = props->rep.end();
  return it;
}

void oxrocksdb_user_collected_properties_iter_destroy(
    oxrocksdb_user_collected_properties_iterator_t* it) {
  delete it;
}

unsigned char oxrocksdb_user_collected_properties_iter_valid(
    const oxrocksdb_user_collected_properties_iterator_t* it) {
  return it->cur_ != it->end_;
}

void oxrocksdb_user_collected_properties_iter_next(
    oxrocksdb_user_collected_properties_iterator_t* it) {
  ++(it->cur_);
}

const char* oxrocksdb_user_collected_properties_iter_key(
    const oxrocksdb_user_collected_properties_iterator_t* it, size_t* klen) {
  *klen = it->cur_->first.size();
  return it->cur_->first.data();
}

const char* oxrocksdb_user_collected_properties_iter_value(
    const oxrocksdb_user_collected_properties_iterator_t* it, size_t* vlen) {
  *vlen = it->cur_->second.size();
  return it->cur_->second.data();
}

// CompactionServiceOptionsOverride functions
oxrocksdb_compaction_service_options_override_t*
oxrocksdb_compaction_service_options_override_create() {
  return new oxrocksdb_compaction_service_options_override_t;
}

oxrocksdb_compaction_service_options_override_t*
oxrocksdb_compaction_service_options_override_create_from_options(
    rocksdb_options_t* options) {
  if (!options) {
    return nullptr;
  }

  oxrocksdb_compaction_service_options_override_t* override_opts =
      new oxrocksdb_compaction_service_options_override_t;

  // Copy all relevant options from rocksdb_options_t
  override_opts->rep.env = options->rep.env;
  override_opts->rep.file_checksum_gen_factory =
      options->rep.file_checksum_gen_factory;
  override_opts->rep.comparator = options->rep.comparator;
  override_opts->rep.merge_operator = options->rep.merge_operator;
  override_opts->rep.compaction_filter = options->rep.compaction_filter;
  override_opts->rep.compaction_filter_factory =
      options->rep.compaction_filter_factory;
  override_opts->rep.prefix_extractor = options->rep.prefix_extractor;
  override_opts->rep.table_factory = options->rep.table_factory;
  override_opts->rep.sst_partitioner_factory =
      options->rep.sst_partitioner_factory;
  override_opts->rep.listeners = options->rep.listeners;
  override_opts->rep.statistics = options->rep.statistics;
  override_opts->rep.info_log = options->rep.info_log;
  override_opts->rep.table_properties_collector_factories =
      options->rep.table_properties_collector_factories;

  return override_opts;
}

void oxrocksdb_compaction_service_options_override_destroy(
    oxrocksdb_compaction_service_options_override_t* override_options) {
  if (override_options) {
    delete override_options;
  }
}

void oxrocksdb_compaction_service_options_override_add_table_properties_collector_factory(
    oxrocksdb_compaction_service_options_override_t* override_options,
    oxrocksdb_table_properties_collector_factory_t* factory) {
  if (override_options && factory) {
    override_options->rep.table_properties_collector_factories.push_back(
        factory->rep);
  }
}


// Table Properties Collector functions

oxrocksdb_table_properties_collector_t*
oxrocksdb_table_properties_collector_create(
    void* state, const char* (*name)(void*), void (*destruct)(void*),
    void (*add)(void*, const char* key, size_t key_len, const char* value,
                size_t value_len, uint32_t entry_type, uint64_t seq,
                uint64_t file_size),
    void (*finish)(void*, oxrocksdb_user_collected_properties_t* props)) {
  return new oxrocksdb_table_properties_collector_t(state, name, destruct, add,
                                                    finish);
}

void oxrocksdb_table_properties_collector_destroy(
    oxrocksdb_table_properties_collector_t* c) {
  delete c;
}

oxrocksdb_table_properties_collector_factory_t*
oxrocksdb_table_properties_collector_factory_create(
    void* state, const char* (*name)(void*), void (*destruct)(void*),
    oxrocksdb_table_properties_collector_t* (
        *create_table_properties_collector)(void*, uint32_t cf)) {
  auto f = new oxrocksdb_table_properties_collector_factory_t;
  f->rep = std::make_shared<OxTablePropertiesCollectorFactory>(
      state, name, destruct, create_table_properties_collector);
  return f;
}

void oxrocksdb_table_properties_collector_factory_destroy(
    oxrocksdb_table_properties_collector_factory_t* factory) {
  delete factory;
}

void oxrocksdb_options_add_table_properties_collector_factory(
    rocksdb_options_t* opt,
    oxrocksdb_table_properties_collector_factory_t* factory) {
  if (opt && factory) {
    opt->rep.table_properties_collector_factories.push_back(factory->rep);
  }
}

// Table Properties Collection functions

size_t oxrocksdb_table_properties_collection_len(
    const oxrocksdb_table_properties_collection_t* props) {
  return props->rep.size();
}

void oxrocksdb_table_properties_collection_destroy(
    oxrocksdb_table_properties_collection_t* props) {
  delete props;
}

oxrocksdb_table_properties_collection_iterator_t*
oxrocksdb_table_properties_collection_iter_create(
    const oxrocksdb_table_properties_collection_t* collection) {
  auto it = new oxrocksdb_table_properties_collection_iterator_t;
  it->cur_ = collection->rep.begin();
  it->end_ = collection->rep.end();
  return it;
}

void oxrocksdb_table_properties_collection_iter_destroy(
    oxrocksdb_table_properties_collection_iterator_t* it) {
  delete it;
}

unsigned char oxrocksdb_table_properties_collection_iter_valid(
    const oxrocksdb_table_properties_collection_iterator_t* it) {
  return it->cur_ != it->end_;
}

void oxrocksdb_table_properties_collection_iter_next(
    oxrocksdb_table_properties_collection_iterator_t* it) {
  ++(it->cur_);
}

const char* oxrocksdb_table_properties_collection_iter_key(
    const oxrocksdb_table_properties_collection_iterator_t* it, size_t* klen) {
  *klen = it->cur_->first.size();
  return it->cur_->first.data();
}

const oxrocksdb_table_properties_t*
oxrocksdb_table_properties_collection_iter_value(
    const oxrocksdb_table_properties_collection_iterator_t* it) {
  if (it->cur_->second) {
    return reinterpret_cast<const oxrocksdb_table_properties_t*>(
        it->cur_->second.get());
  }
  return nullptr;
}

oxrocksdb_table_properties_collection_t*
oxrocksdb_get_properties_of_all_tables_cf(
    rocksdb_t* db, rocksdb_column_family_handle_t* cf, char** errptr) {
  auto props = new oxrocksdb_table_properties_collection_t;
  Status s = db->rep->GetPropertiesOfAllTables(cf->rep, &props->rep);
  if (!s.ok()) {
    delete props;
    SaveError(errptr, s);
    return nullptr;
  }
  return props;
}

const oxrocksdb_table_properties_t* oxrocksdb_flushjobinfo_table_properties(
    const rocksdb_flushjobinfo_t* info) {
  return reinterpret_cast<const oxrocksdb_table_properties_t*>(
      &info->rep.table_properties);
}

const oxrocksdb_table_properties_collection_t*
oxrocksdb_compactionjobinfo_table_properties(
    const rocksdb_compactionjobinfo_t* info) {
  return reinterpret_cast<const oxrocksdb_table_properties_collection_t*>(
      &info->rep.table_properties);
}

// Event listener functions

oxrocksdb_eventlistener_t* oxrocksdb_eventlistener_create(
    void* state, void (*destructor)(void*),
    void (*on_flush_begin)(void*, rocksdb_t*, const rocksdb_flushjobinfo_t*),
    void (*on_flush_completed)(void*, rocksdb_t*,
                               const rocksdb_flushjobinfo_t*),
    void (*on_compaction_begin)(void*, rocksdb_t*,
                                const rocksdb_compactionjobinfo_t*),
    void (*on_compaction_completed)(void*, rocksdb_t*,
                                    const rocksdb_compactionjobinfo_t*)) {
  auto t = new oxrocksdb_eventlistener_t;
  t->rep = std::make_shared<OxEventListener>(
      state,
      destructor,
      on_flush_begin,
      on_flush_completed,
      on_compaction_begin,
      on_compaction_completed
    );
  return t;
}

void oxrocksdb_eventlistener_destroy(oxrocksdb_eventlistener_t* t) { delete t; }

void oxrocksdb_options_add_eventlistener(rocksdb_options_t* opt,
                                         oxrocksdb_eventlistener_t* t) {
  if (opt && t) {
    opt->rep.listeners.push_back(t->rep);
  }
}

}
