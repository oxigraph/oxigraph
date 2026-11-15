set -euo pipefail

cd /workdir
apk add clang-dev
curl --fail --show-error --location https://static.rust-lang.org/rustup/dist/%arch%-unknown-linux-musl/rustup-init --output rustup-init
chmod +x rustup-init
./rustup-init -y --profile minimal
source "$HOME/.cargo/env"
cd python
uv run --locked --only-dev maturin develop --release --locked --features abi3
uv run --locked --only-dev python generate_stubs.py pyincroxigraph pyincroxigraph.pyi --ruff
rm -rf ../target/wheels
uv run --locked --only-dev python -m unittest discover -s tests
uv run --locked --only-dev maturin build --release --locked --features abi3 --compatibility musllinux_1_2
uvx twine check ../target/wheels/pyincroxigraph-*.whl
