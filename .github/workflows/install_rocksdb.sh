if [ -f "rocksdb" ]
then
  cd rocksdb || exit
else
  git clone https://github.com/facebook/rocksdb.git
  cd rocksdb || exit
  git checkout v8.0.0
  make shared_lib
fi
sudo make install-shared
sudo ldconfig /usr/local/lib
