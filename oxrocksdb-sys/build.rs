// Code from https://github.com/rust-rocksdb/rust-rocksdb/blob/eb2d302682418b361a80ad8f4dcf335ade60dcf5/librocksdb-sys/build.rs
// License: https://github.com/rust-rocksdb/rust-rocksdb/blob/master/LICENSE

use std::env::var;
#[cfg(not(feature = "pkg-config"))]
use std::env::{remove_var, set_var};
#[cfg(not(feature = "pkg-config"))]
use std::path::Path;
use std::path::PathBuf;

#[cfg(not(feature = "pkg-config"))]
fn link(name: &str, bundled: bool) {
    let target = var("TARGET").unwrap();
    let target: Vec<_> = target.split('-').collect();
    if target.get(2) == Some(&"windows") {
        println!("cargo:rustc-link-lib=dylib={name}");
        if bundled && target.get(3) == Some(&"gnu") {
            let dir = var("CARGO_MANIFEST_DIR").unwrap();
            println!("cargo:rustc-link-search=native={}/{}", dir, target[0]);
        }
    }
}

fn bindgen_rocksdb_api(includes: &[PathBuf]) {
    println!("cargo:rerun-if-changed=api/");

    let mut builder = bindgen::Builder::default();
    for include in includes {
        builder = builder.clang_arg(format!("-I{}", include.display()));
    }
    builder
        .header("api/c.h")
        .allowlist_function("rocksdb_.*")
        .allowlist_function("oxrocksdb_.*")
        .allowlist_type("rocksdb_.*")
        .allowlist_var("rocksdb_.*")
        .generate()
        .unwrap()
        .write_to_file(PathBuf::from(var("OUT_DIR").unwrap()).join("bindings.rs"))
        .unwrap();
}

fn build_rocksdb_api(includes: &[PathBuf]) {
    let target = var("TARGET").unwrap();
    let mut config = cc::Build::new();
    for include in includes {
        config.include(include);
    }
    if target.contains("msvc") {
        config.flag("-EHsc").flag("-std:c++17");
    } else {
        config.flag("-std=c++17");
    }
    if target.contains("armv5te") || target.contains("riscv64gc") {
        println!("cargo:rustc-link-lib=atomic");
    }
    config.cpp(true).file("api/c.cc").compile("oxrocksdb_api");
}

#[cfg(not(feature = "pkg-config"))]
fn build_rocksdb() {
    let target = var("TARGET").unwrap();

    let mut config = cc::Build::new();
    config
        .cpp(true)
        .include("rocksdb/include/")
        .include("rocksdb/")
        .file("api/build_version.cc")
        .define("NDEBUG", Some("1"))
        .define("LZ4", Some("1"))
        .include("lz4/lib/");

    let mut lib_sources = include_str!("rocksdb/src.mk")
        .split_once("LIB_SOURCES =")
        .unwrap()
        .1
        .split_once("ifeq")
        .unwrap()
        .0
        .split('\\')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>();

    if target.contains("x86_64") {
        // This is needed to enable hardware CRC32C. Technically, SSE 4.2 is
        // only available since Intel Nehalem (about 2010) and AMD Bulldozer
        // (about 2011).
        let target_feature = var("CARGO_CFG_TARGET_FEATURE").unwrap();
        let target_features: Vec<_> = target_feature.split(',').collect();
        if target_features.contains(&"sse2") {
            config.flag_if_supported("-msse2");
        }
        if target_features.contains(&"sse4.1") {
            config.flag_if_supported("-msse4.1");
        }
        if target_features.contains(&"sse4.2") {
            config.flag_if_supported("-msse4.2");
            config.define("HAVE_SSE42", Some("1"));
        }
        if target_features.contains(&"pclmulqdq") && !target.contains("android") {
            config.define("HAVE_PCLMUL", Some("1"));
            config.flag_if_supported("-mpclmul");
        }
        if target_features.contains(&"avx2") {
            config.define("HAVE_AVX2", Some("1"));
            config.flag_if_supported("-mavx2");
        }
        if target_features.contains(&"bmi1") {
            config.define("HAVE_BMI", Some("1"));
            config.flag_if_supported("-mbmi");
        }
        if target_features.contains(&"lzcnt") {
            config.define("HAVE_LZCNT", Some("1"));
            config.flag_if_supported("-mlzcnt");
        }
    }

    if target.contains("apple-ios") {
        config.define("OS_MACOSX", None);
        config.define("IOS_CROSS_COMPILE", None);
        config.define("PLATFORM", "IOS");
        config.define("NIOSTATS_CONTEXT", None);
        config.define("NPERF_CONTEXT", None);
        config.define("ROCKSDB_PLATFORM_POSIX", None);
        config.define("ROCKSDB_LIB_IO_POSIX", None);
        unsafe { remove_var("SDKROOT") }; // We override SDKROOT for cross-compilation
        unsafe { set_var("IPHONEOS_DEPLOYMENT_TARGET", "11.0") };
    } else if target.contains("darwin") {
        config.define("OS_MACOSX", None);
        config.define("ROCKSDB_PLATFORM_POSIX", None);
        config.define("ROCKSDB_LIB_IO_POSIX", None);
        unsafe { remove_var("SDKROOT") }; // We override SDKROOT for cross-compilation
    } else if target.contains("android") {
        config.define("OS_ANDROID", None);
        config.define("ROCKSDB_PLATFORM_POSIX", None);
        config.define("ROCKSDB_LIB_IO_POSIX", None);
    } else if target.contains("linux") {
        config.define("OS_LINUX", None);
        config.define("ROCKSDB_PLATFORM_POSIX", None);
        config.define("ROCKSDB_LIB_IO_POSIX", None);
    } else if target.contains("freebsd") {
        config.define("OS_FREEBSD", None);
        config.define("ROCKSDB_PLATFORM_POSIX", None);
        config.define("ROCKSDB_LIB_IO_POSIX", None);
    } else if target.contains("windows") {
        link("rpcrt4", false);
        link("shlwapi", false);
        config.define("DWIN32", None);
        config.define("OS_WIN", None);
        config.define("_MBCS", None);
        config.define("WIN64", None);
        config.define("NOMINMAX", None);
        config.define("ROCKSDB_WINDOWS_UTF8_FILENAMES", None);

        if target.contains("pc-windows-gnu") {
            // Tell MinGW to create localtime_r wrapper of localtime_s function.
            config.define("_POSIX_C_SOURCE", Some("1"));
            // Tell MinGW to use at least Windows Vista headers instead of the ones of Windows XP.
            // (This is minimum supported version of rocksdb)
            config.define("_WIN32_WINNT", Some("_WIN32_WINNT_VISTA"));
        }

        // Remove POSIX-specific sources
        lib_sources = lib_sources
            .iter()
            .copied()
            .filter(|file| {
                !matches!(
                    *file,
                    "port/port_posix.cc"
                        | "env/env_posix.cc"
                        | "env/fs_posix.cc"
                        | "env/io_posix.cc"
                )
            })
            .collect::<Vec<&'static str>>();

        // Add Windows-specific sources
        lib_sources.extend([
            "port/win/env_default.cc",
            "port/win/port_win.cc",
            "port/win/xpress_win.cc",
            "port/win/io_win.cc",
            "port/win/win_thread.cc",
            "port/win/env_win.cc",
            "port/win/win_logger.cc",
        ]);
    }

    config.define("ROCKSDB_SUPPORT_THREAD_LOCAL", None);

    if target.contains("msvc") {
        config.flag("-EHsc").flag("-std:c++17");
    } else {
        config.flag("-std=c++17").flag("-Wno-invalid-offsetof");
        if target.contains("x86_64") || target.contains("aarch64") {
            config.define("HAVE_UINT128_EXTENSION", Some("1"));
        }
    }

    for file in lib_sources {
        if file != "util/build_version.cc" {
            config.file(format!("rocksdb/{file}"));
        }
    }

    config.compile("rocksdb");
}

#[cfg(not(feature = "pkg-config"))]
fn build_lz4() {
    let mut config = cc::Build::new();
    config
        .file("lz4/lib/lz4.c")
        .file("lz4/lib/lz4frame.c")
        .file("lz4/lib/lz4hc.c")
        .file("lz4/lib/xxhash.c");
    if var("TARGET").unwrap() == "i686-pc-windows-gnu" {
        config.flag("-fno-tree-vectorize");
    }
    config.compile("lz4");
}

#[cfg(not(feature = "pkg-config"))]
fn main() {
    let includes = [Path::new("rocksdb/include").to_path_buf()];
    build_lz4();
    build_rocksdb();
    build_rocksdb_api(&includes);
    bindgen_rocksdb_api(&includes);
}

#[cfg(feature = "pkg-config")]
fn main() {
    let library = pkg_config::Config::new()
        .atleast_version("8.0.0")
        .probe("rocksdb")
        .unwrap();
    build_rocksdb_api(&library.include_paths);
    bindgen_rocksdb_api(&library.include_paths);
}
