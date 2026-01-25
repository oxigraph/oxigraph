cd /workdir
dnf install -y clang
curl https://static.rust-lang.org/rustup/dist/%arch%-unknown-linux-gnu/rustup-init --output rustup-init
chmod +x rustup-init
./rustup-init -y --profile minimal
source "$HOME/.cargo/env"
cd python
uv run --locked --only-dev maturin develop --release --features abi3
uv run --locked --only-dev python generate_stubs.py pyoxigraph pyoxigraph.pyi --ruff
rm -rf ../target/wheels
uv run --locked --only-dev maturin build --release --features abi3 --compatibility manylinux_2_28
if [ %for_each_version% ]; then
  for VERSION in 10 11 12 13 14 14t; do
    uv run --locked --only-dev maturin build --release --interpreter "python3.$VERSION" --compatibility manylinux_2_28
  done
  uv run --locked --only-dev maturin build --release --interpreter "pypy3.11" --compatibility manylinux_2_28
fi
cd ../cli
uvx maturin build --release --no-default-features --features rustls-native,geosparql,rdf-12 --compatibility manylinux_2_28
