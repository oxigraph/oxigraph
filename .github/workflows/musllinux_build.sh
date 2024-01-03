cd /workdir
apk add clang-dev
curl https://static.rust-lang.org/rustup/dist/%arch%-unknown-linux-musl/rustup-init --output rustup-init
chmod +x rustup-init
./rustup-init -y --profile minimal
source "$HOME/.cargo/env"
export PATH="${PATH}:/opt/python/cp37-cp37m/bin:/opt/python/cp38-cp38/bin:/opt/python/cp39-cp39/bin:/opt/python/cp310-cp310/bin:/opt/python/cp311-cp311/bin"
cd python
python3.12 -m venv venv
source venv/bin/activate
pip install -r requirements.dev.txt
maturin develop --release --no-default-features --features rustls
python generate_stubs.py pyoxigraph pyoxigraph.pyi --ruff
maturin build --release --no-default-features --features abi3 --features rustls --compatibility musllinux_1_2
if [ %for_each_version% ]; then
  for VERSION in 8 9 10 11 12; do
    maturin build --release --no-default-features --features rustls --interpreter "python3.$VERSION" --compatibility musllinux_1_2
  done
fi
