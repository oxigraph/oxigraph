// Code from https://github.com/rust-rocksdb/rust-rocksdb/blob/eb2d302682418b361a80ad8f4dcf335ade60dcf5/librocksdb-sys/build.rs
// License: https://github.com/rust-rocksdb/rust-rocksdb/blob/master/LICENSE

use std::env::var;
use std::path::PathBuf;

fn link(name: &str, bundled: bool) {
    let target = var("TARGET").unwrap();
    let target: Vec<_> = target.split('-').collect();
    if target.get(2) == Some(&"windows") {
        println!("cargo:rustc-link-lib=dylib={}", name);
        if bundled && target.get(3) == Some(&"gnu") {
            let dir = var("CARGO_MANIFEST_DIR").unwrap();
            println!("cargo:rustc-link-search=native={}/{}", dir, target[0]);
        }
    }
}

fn bindgen_rocksdb() {
    bindgen::Builder::default()
        .header("api/c.h")
        .ctypes_prefix("libc")
        .size_t_is_usize(true)
        .allowlist_function("rocksdb_.*")
        .allowlist_type("rocksdb_.*")
        .allowlist_var("rocksdb_.*")
        .generate()
        .expect("unable to generate rocksdb bindings")
        .write_to_file(PathBuf::from(var("OUT_DIR").unwrap()).join("bindings.rs"))
        .expect("unable to write rocksdb bindings");
}

fn build_rocksdb() {
    let target = var("TARGET").unwrap();

    let mut config = cc::Build::new();
    config
        .cpp(true)
        .include("rocksdb/include/")
        .include("rocksdb/")
        .file("api/c.cc")
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
    } else if target.contains("aarch64") {
        lib_sources.push("util/crc32c_arm64.cc")
    }

    if target.contains("darwin") {
        config.define("OS_MACOSX", None);
        config.define("ROCKSDB_PLATFORM_POSIX", None);
        config.define("ROCKSDB_LIB_IO_POSIX", None);
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
        config.define("WITH_WINDOWS_UTF8_FILENAMES", "ON");

        if target == "x86_64-pc-windows-gnu" {
            // Tell MinGW to create localtime_r wrapper of localtime_s function.
            config.define("_POSIX_C_SOURCE", Some("1"));
            // Tell MinGW to use at least Windows Vista headers instead of the ones of Windows XP.
            // (This is minimum supported version of rocksdb)
            config.define("_WIN32_WINNT", Some("_WIN32_WINNT_VISTA"));
        }

        // Remove POSIX-specific sources
        lib_sources = lib_sources
            .iter()
            .cloned()
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
        lib_sources.push("port/win/port_win.cc");
        lib_sources.push("port/win/env_win.cc");
        lib_sources.push("port/win/env_default.cc");
        lib_sources.push("port/win/win_logger.cc");
        lib_sources.push("port/win/io_win.cc");
        lib_sources.push("port/win/win_thread.cc");
    }

    config.define("ROCKSDB_SUPPORT_THREAD_LOCAL", None);

    if target.contains("msvc") {
        config.flag("-EHsc").flag("-std:c++17");
    } else {
        config.flag("-std=c++17").flag("-Wno-invalid-offsetof");
    }

    for file in lib_sources {
        if file == "db/c.cc" || file == "util/build_version.cc" {
            continue;
        }
        let file = "rocksdb/".to_string() + file;
        config.file(&file);
    }
    config.compile("rocksdb");
}

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

fn main() {
    println!("cargo:rerun-if-changed=api/");
    bindgen_rocksdb();
    build_lz4();
    build_rocksdb();
}
