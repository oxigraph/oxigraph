if [ -f "rocksdb" ]
then
  cd rocksdb || exit
else
  git clone https://github.com/facebook/rocksdb.git
  cd rocksdb || exit
  git checkout v9.10.0
  git apply "../.github/workflows/install_rocksdb.patch"
  make shared_lib
fi
sudo make install-shared
sudo ldconfig /usr/local/lib
