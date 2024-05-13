FROM gcr.io/oss-fuzz-base/base-builder-rust:v1
RUN apt-get update && apt-get install -y llvm-dev libclang-dev clang && apt-get clean && rm --recursive --force /var/lib/apt/lists/*
COPY . $SRC/oxigraph
WORKDIR oxigraph
COPY .clusterfuzzlite/build.sh $SRC/
