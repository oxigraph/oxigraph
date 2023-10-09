# Build Notes

## Intel DevCloud

Operating System: Ubuntu 20.04.2 LTS (focal)  

Using the default oneAPI version set at login:

```
source build-devcloud.sh
```

Run core tests.

```
cargo test --test oxigraph
cargo test --test parser
cargo test --test sparql
cargo test --test store
```

Run all tests.

```
cargo test --all-features
```
