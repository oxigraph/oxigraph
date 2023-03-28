// Copyright (c) Facebook, Inc. and its affiliates. All Rights Reserved.

#include <memory>

#include "rocksdb/utilities/object_registry.h"
#include "rocksdb/version.h"
#ifdef SPEEDB
#include "speedb/version.h"
#endif

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

#ifdef SPEEDB
std::string GetSpeedbVersionAsString(bool with_patch) {
  std::string version =
      std::to_string(SPEEDB_MAJOR) + "." + std::to_string(SPEEDB_MINOR);
  if (with_patch) {
    version += "." + std::to_string(SPEEDB_PATCH);
  }
  return version;
}
#endif

std::string GetRocksBuildInfoAsString(const std::string& program,
                                      bool verbose) {
  std::string info = program;
#ifdef SPEEDB
  info += " (Speedb " + GetSpeedbVersionAsString(true) + " )";
#endif
  info += " (RocksDB " + GetRocksVersionAsString(true) + " )";
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

#ifdef SPEEDB
std::string GetRocksDebugPropertiesAsString() { return ""; }
#endif
}  // namespace ROCKSDB_NAMESPACE
