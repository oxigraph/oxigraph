Oxigraph RocksDB bindings
=========================

[RocksDB](http://rocksdb.org/) bindings for [Oxigraph](https://oxigraph.org).

By default it builds RocksDB as part of this crate.
It is also possible to dynamically link to RocksDB using the disabled by default `pkg-config` feature.
In this case [pkg-config](https://crates.io/crates/pkg-config) will be used to link to RocksDB.
Refer to this crate documentation if you want to configure the library lookup.

Based on [librocksdb-sys](https://crates.io/crates/librocksdb-sys) under Apache v2 license.
