cd /workdir
apk add clang-dev
curl https://static.rust-lang.org/rustup/dist/%arch%-unknown-linux-musl/rustup-init --output rustup-init
chmod +x rustup-init
./rustup-init -y --profile minimal
source "$HOME/.cargo/env"
cd python
uv sync --locked --only-group build
uv run maturin develop --release --features abi3
uv run python generate_stubs.py pyoxigraph pyoxigraph.pyi --ruff
uv run maturin build --release --features abi3 --compatibility musllinux_1_2
if [ %for_each_version% ]; then
  for VERSION in 8 9 10 11 12 13 13t; do
    uv run maturin build --release --interpreter "python3.$VERSION" --compatibility musllinux_1_2
  done
fi
