// Copyright (c) Facebook, Inc. and its affiliates. All Rights Reserved.

#include <memory>

#include "rocksdb/utilities/object_registry.h"
#include "rocksdb/version.h"

namespace ROCKSDB_NAMESPACE {
std::unordered_map<std::string, RegistrarFunc> ObjectRegistry::builtins_ = {};

const std::unordered_map<std::string, std::string>& GetRocksBuildProperties() {
  static std::unique_ptr<std::unordered_map<std::string, std::string>> props(
      new std::unordered_map<std::string, std::string>());
  return *props;
}

std::string GetRocksVersionAsString(bool with_patch) {
  std::string version =
      std::to_string(ROCKSDB_MAJOR) + "." + std::to_string(ROCKSDB_MINOR);
  if (with_patch) {
    return version + "." + std::to_string(ROCKSDB_PATCH);
  } else {
    return version;
  }
}

std::string GetRocksBuildInfoAsString(const std::string& program,
                                      bool verbose) {
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
}  // namespace ROCKSDB_NAMESPACE
