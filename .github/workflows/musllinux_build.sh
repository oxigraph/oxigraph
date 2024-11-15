cd /workdir
apk add clang-dev
curl https://static.rust-lang.org/rustup/dist/%arch%-unknown-linux-musl/rustup-init --output rustup-init
chmod +x rustup-init
./rustup-init -y --profile minimal
source "$HOME/.cargo/env"
cd python
python3.13 -m venv venv
source venv/bin/activate
pip install -r requirements.build.txt
maturin develop --release
python generate_stubs.py pyoxigraph pyoxigraph.pyi --ruff
maturin build --release --features abi3 --compatibility musllinux_1_2
if [ %for_each_version% ]; then
  for VERSION in 8 9 10 11 12 13 13t; do
    maturin build --release --interpreter "python3.$VERSION" --compatibility musllinux_1_2
  done
fi
