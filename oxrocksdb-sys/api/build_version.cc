// Copyright (c) Facebook, Inc. and its affiliates. All Rights Reserved.

#include <memory>

#include "rocksdb/version.h"
#include "util/string_util.h"

namespace ROCKSDB_NAMESPACE {
const std::unordered_map<std::string, std::string>& GetRocksBuildProperties() {
  static std::unique_ptr<std::unordered_map<std::string, std::string>> props(new std::unordered_map<std::string, std::string>());
  return *props;
}

std::string GetRocksVersionAsString(bool with_patch) {
  std::string version = ToString(ROCKSDB_MAJOR) + "." + ToString(ROCKSDB_MINOR);
  if (with_patch) {
    return version + "." + ToString(ROCKSDB_PATCH);
  } else {
    return version;
  }
}
  
std::string GetRocksBuildInfoAsString(const std::string& program, bool verbose) {
  std::string info = program + " (RocksDB) " + GetRocksVersionAsString(true);
  if (verbose) {
    for (const auto& it : GetRocksBuildProperties()) {
      info.append("\n    ");
      info.append(it.first);
      info.append(": ");
      info.append(it.second);
    }
  }
  return info;
}
} // namespace ROCKSDB_NAMESPACE

