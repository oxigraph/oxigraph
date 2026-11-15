set -euo pipefail

cd /workdir
dnf install -y clang
curl --fail --show-error --location https://static.rust-lang.org/rustup/dist/%arch%-unknown-linux-gnu/rustup-init --output rustup-init
chmod +x rustup-init
./rustup-init -y --profile minimal
source "$HOME/.cargo/env"
cd python
uv run --locked --only-dev maturin develop --release --locked --features abi3
uv run --locked --only-dev python generate_stubs.py pyincroxigraph pyincroxigraph.pyi --ruff
rm -rf ../target/wheels
uv run --locked --only-dev python -m unittest discover -s tests
uv run --locked --only-dev maturin build --release --locked --features abi3 --compatibility manylinux_2_28
uvx twine check ../target/wheels/pyincroxigraph-*.whl
