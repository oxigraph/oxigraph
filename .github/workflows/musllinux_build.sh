cd /workdir
apk add clang-dev
curl https://static.rust-lang.org/rustup/dist/%arch%-unknown-linux-musl/rustup-init --output rustup-init
chmod +x rustup-init
./rustup-init -y --profile minimal
source "$HOME/.cargo/env"
cd python
uv run --locked --only-dev maturin develop --release --features abi3
uv run --locked --only-dev python generate_stubs.py pyoxigraph pyoxigraph.pyi --ruff
rm -rf ../target/wheels
uv run --locked --only-dev maturin build --release --features abi3 --compatibility musllinux_1_2
cd ../cli
uvx maturin build --release --no-default-features --features rustls-native,geosparql,rdf-12 --compatibility musllinux_1_2
