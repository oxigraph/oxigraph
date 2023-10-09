#!/bin/bash

export CC=icx
export CXX=icpx
export LIBCLANG_PATH="${ONEAPI_ROOT}"/tensorflow/latest/lib
export CPATH="${CPATH}":"${ONEAPI_ROOT}"/compiler/latest/linux/lib/clang/17/include
mkdir "${PBS_SCRATCHDIR}"/tmp
export TMPDIR="${PBS_SCRATCHDIR}"/tmp
export RUSTFLAGS=--cfg=rustix_use_libc
