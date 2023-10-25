#!/bin/bash

# Use with `source build-devcloud.sh` to setup environment variables
# that control compilation and linking on the Intel(R) DevCloud with the
# Intel(R) oneAPI DPC++/C++ Compiler.

# Use the Intel(R) oneAPI DPC++/C++ Compiler
export CC=icx
export CXX=icpx

# Use libclang
export LIBCLANG_PATH="${ONEAPI_ROOT}"/tensorflow/latest/lib

# Use clang headers
export CPATH="${CPATH}":"${ONEAPI_ROOT}"/compiler/latest/linux/lib/clang/17/include

# Use local device storage for temp files instead of NFS to
# work-around Rust API errors from NFS mounted files not deleting as
# expected.
mkdir --parents "${PBS_SCRATCHDIR}"/tmp
export TMPDIR="${PBS_SCRATCHDIR}"/tmp

# For the rustix library use libc. Use clang linking.
export RUSTFLAGS="--cfg=rustix_use_libc -C linker=${CMPLR_ROOT}/linux/bin-llvm/clang"

# Build on local device storage for performance. Note that build
# artifacts will not persist between jobs.
# https://doc.rust-lang.org/cargo/reference/environment-variables.html#environment-variables-cargo-reads
export CARGO_TARGET_DIR="${PBS_SCRATCHDIR}"
