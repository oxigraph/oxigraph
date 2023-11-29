cd /workdir
apk add clang-dev
curl https://static.rust-lang.org/rustup/dist/%arch%-unknown-linux-musl/rustup-init --output rustup-init
chmod +x rustup-init
./rustup-init -y --profile minimal
source "$HOME/.cargo/env"
export PATH="${PATH}:/opt/python/cp37-cp37m/bin:/opt/python/cp38-cp38/bin:/opt/python/cp39-cp39/bin:/opt/python/cp310-cp310/bin:/opt/python/cp311-cp311/bin"
cd python
python3.10 -m venv venv
source venv/bin/activate
pip install -r requirements.dev.txt
maturin develop --release -m Cargo.toml
python generate_stubs.py pyoxigraph pyoxigraph.pyi --black
maturin build --release -m Cargo.toml --features abi3 --compatibility musllinux_1_1
if [ %for_each_version% ]; then
  for VERSION in 7 8 9 10 11 12; do
    maturin build --release -m Cargo.toml --interpreter "python3.$VERSION" --compatibility musllinux_1_1
  done
fi
