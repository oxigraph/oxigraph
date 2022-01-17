cd /workdir
yum -y install centos-release-scl-rh
yum -y install llvm-toolset-7.0
source scl_source enable llvm-toolset-7.0
curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal
curl --verbose -L "https://github.com/PyO3/maturin/releases/latest/download/maturin-%arch%-unknown-linux-musl.tar.gz" | tar -xz -C /usr/local/bin
export PATH="${PATH}:/root/.cargo/bin:/opt/python/cp37-cp37m/bin:/opt/python/cp38-cp38/bin:/opt/python/cp39-cp39/bin:/opt/python/cp310-cp310/bin"
maturin build --release --no-sdist -m python/Cargo.toml
