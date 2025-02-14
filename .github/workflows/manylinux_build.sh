cd /workdir
yum -y install centos-release-scl-rh
yum -y install llvm-toolset-7.0
source scl_source enable llvm-toolset-7.0
curl https://static.rust-lang.org/rustup/dist/%arch%-unknown-linux-gnu/rustup-init --output rustup-init
chmod +x rustup-init
./rustup-init -y --profile minimal
source "$HOME/.cargo/env"
cd python
uv venv
uv pip install -r requirements.build.txt
source .venv/bin/activate
maturin develop --release
python generate_stubs.py pyoxigraph pyoxigraph.pyi --ruff
maturin build --release --features abi3 --compatibility manylinux2014
if [ %for_each_version% ]; then
  for VERSION in 8 9 10 11 12 13 13t; do
    maturin build --release --interpreter "python3.$VERSION" --compatibility manylinux2014
  done
  for VERSION in 9 10; do
    maturin build --release --interpreter "pypy3.$VERSION" --compatibility manylinux2014
  done
fi
