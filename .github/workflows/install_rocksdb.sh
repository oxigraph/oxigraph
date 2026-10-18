set -euo pipefail

if [ ! -d rocksdb ]; then
  git clone --branch v9.10.0 --depth 1 https://github.com/facebook/rocksdb.git
  git -C rocksdb apply "../.github/workflows/install_rocksdb.patch"
fi
cd rocksdb
# The cached library is shared by runners with different CPU instruction sets.
make PORTABLE=1 shared_lib
sudo make PORTABLE=1 install-shared
sudo ldconfig /usr/local/lib
