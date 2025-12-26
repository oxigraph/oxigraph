# Installation Guide

Complete installation instructions for all Oxigraph platforms and environments.

## Table of Contents

- [System Requirements](#system-requirements)
- [Rust Library](#rust-library)
- [Python Bindings](#python-bindings)
- [JavaScript Bindings](#javascript-bindings)
- [CLI Server](#cli-server)
- [Building from Source](#building-from-source)
- [Platform-Specific Notes](#platform-specific-notes)
- [Troubleshooting](#troubleshooting)

---

## System Requirements

### Minimum Requirements

- **Operating Systems**: Linux, macOS, Windows, or any platform supporting Rust/WASM
- **Memory**: 512 MB minimum (more recommended for large datasets)
- **Disk Space**: Varies by dataset size (RocksDB storage is ~2-3x raw data size)

### Platform-Specific Requirements

#### Rust
- Rust 1.70+ (latest stable recommended)
- Clang/LLVM (for RocksDB bindings)
- Git (for submodules when building from source)

#### Python
- Python 3.8+
- pip 19.0+ or conda

#### JavaScript
- Node.js 18+ (for server-side)
- Modern browser with WebAssembly support (for client-side)
  - Chrome 90+
  - Firefox 89+
  - Safari 15+
  - Edge 90+

#### CLI Server
- Docker, or
- Rust toolchain for compilation, or
- Pre-built binary for your platform

---

## Rust Library

### Using Cargo (Recommended)

Add Oxigraph to your `Cargo.toml`:

```toml
[dependencies]
oxigraph = "0.4"
```

Or via command line:

```bash
cargo add oxigraph
```

### Feature Flags

Customize your installation with feature flags:

```toml
[dependencies]
# Default (includes RocksDB)
oxigraph = "0.4"

# Without RocksDB (in-memory only)
oxigraph = { version = "0.4", default-features = false }

# With HTTP client for federated queries
oxigraph = { version = "0.4", features = ["http-client"] }

# With RDF 1.2 support
oxigraph = { version = "0.4", features = ["rdf-12"] }

# Multiple features
oxigraph = { version = "0.4", features = ["http-client", "rdf-12"] }
```

#### Available Features

| Feature | Description | Default |
|---------|-------------|---------|
| `rocksdb` | Enable persistent RocksDB storage | Yes |
| `http-client` | Enable HTTP client for SERVICE queries | No |
| `http-client-native-tls` | HTTP client with native TLS | No |
| `http-client-rustls-webpki` | HTTP client with Rustls and WebPKI roots | No |
| `http-client-rustls-native` | HTTP client with Rustls and system roots | No |
| `rdf-12` | Enable RDF 1.2 and SPARQL 1.2 features | No |
| `rocksdb-pkg-config` | Link against system RocksDB | No |

### Development Dependencies

For development, you may also want:

```toml
[dev-dependencies]
oxigraph = "0.4"
```

---

## Python Bindings

### Using pip (Recommended)

```bash
pip install pyoxigraph
```

For the latest version:

```bash
pip install --upgrade pyoxigraph
```

### Using conda

```bash
conda install -c conda-forge pyoxigraph
```

Or with mamba (faster):

```bash
mamba install -c conda-forge pyoxigraph
```

### Using Poetry

```bash
poetry add pyoxigraph
```

### Using Pipenv

```bash
pipenv install pyoxigraph
```

### Virtual Environment (Recommended)

Always use a virtual environment:

```bash
# Using venv
python -m venv venv
source venv/bin/activate  # On Windows: venv\Scripts\activate
pip install pyoxigraph

# Using conda
conda create -n myproject python=3.11
conda activate myproject
conda install -c conda-forge pyoxigraph
```

### Verify Installation

```bash
python -c "import pyoxigraph; print(pyoxigraph.__version__)"
```

---

## JavaScript Bindings

### Using npm

```bash
npm install oxigraph
```

### Using yarn

```bash
yarn add oxigraph
```

### Using pnpm

```bash
pnpm add oxigraph
```

### For Different Environments

#### Node.js (CommonJS)

```javascript
const oxigraph = require('oxigraph');
```

#### Node.js (ES Modules)

```javascript
import oxigraph from 'oxigraph/node.js';
```

#### Browser (ES Modules)

```html
<script type="module">
    import init, * as oxigraph from './node_modules/oxigraph/web.js';

    await init(); // Initialize WebAssembly
    // Now use oxigraph...
</script>
```

#### Webpack 5

Webpack 5 has built-in WebAssembly support. Just import normally:

```javascript
import * as oxigraph from 'oxigraph';
```

#### Vite

Create a `vite.config.js`:

```javascript
import { defineConfig } from 'vite';

export default defineConfig({
    optimizeDeps: {
        exclude: ['oxigraph']
    }
});
```

Then import:

```javascript
import init, * as oxigraph from 'oxigraph/web.js';
```

### TypeScript Support

Oxigraph includes TypeScript definitions out of the box:

```typescript
import * as oxigraph from 'oxigraph';

const store: oxigraph.Store = new oxigraph.Store();
```

### Verify Installation

```bash
node -e "const oxigraph = require('oxigraph'); console.log('Oxigraph loaded successfully');"
```

---

## CLI Server

### Docker (Easiest)

Pull the latest image:

```bash
docker pull ghcr.io/oxigraph/oxigraph:latest
```

Run the server:

```bash
docker run -d \
    --name oxigraph \
    -v $PWD/data:/data \
    -p 7878:7878 \
    ghcr.io/oxigraph/oxigraph:latest \
    serve --location /data --bind 0.0.0.0:7878
```

### Using Cargo

```bash
cargo install oxigraph-cli
```

This installs the `oxigraph` binary to `~/.cargo/bin/`.

Make sure `~/.cargo/bin` is in your PATH:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

### Using UV (Python Package Manager)

```bash
# Run once
uvx oxigraph serve --location ./data

# Or install
uv tool install oxigraph
oxigraph serve --location ./data
```

### Using Conda

```bash
conda install -c conda-forge oxigraph-server
```

### Pre-built Binaries

Download from [GitHub Releases](https://github.com/oxigraph/oxigraph/releases/latest):

1. Go to the latest release
2. Download the binary for your platform:
   - `oxigraph-x86_64-linux.tar.gz` (Linux x86_64)
   - `oxigraph-x86_64-macos.tar.gz` (macOS Intel)
   - `oxigraph-aarch64-macos.tar.gz` (macOS Apple Silicon)
   - `oxigraph-x86_64-windows.zip` (Windows)

Extract and add to PATH:

```bash
# Linux/macOS
tar -xzf oxigraph-*.tar.gz
sudo mv oxigraph /usr/local/bin/

# Or add to PATH
export PATH="$PWD:$PATH"
```

### Verify Installation

```bash
oxigraph --version
```

---

## Building from Source

### Prerequisites

1. **Rust toolchain** (latest stable):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Clang/LLVM** (for RocksDB):

   **Ubuntu/Debian:**
   ```bash
   sudo apt-get update
   sudo apt-get install build-essential clang git
   ```

   **Fedora/RHEL/CentOS:**
   ```bash
   sudo dnf install clang gcc gcc-c++ git
   ```

   **macOS:**
   ```bash
   xcode-select --install
   ```

   **Windows:**
   - Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/)
   - Install [LLVM](https://releases.llvm.org/download.html)

3. **Git** (with submodule support)

### Clone the Repository

```bash
git clone --recursive https://github.com/oxigraph/oxigraph.git
cd oxigraph
```

If you already cloned without `--recursive`:

```bash
git submodule update --init --recursive
```

### Build Rust Library

```bash
cd lib/oxigraph
cargo build --release
cargo test
```

### Build Python Bindings

Requirements:
- Python 3.8+
- maturin

```bash
cd python

# Install maturin
pip install maturin

# Development build
maturin develop

# Release build (creates wheel)
maturin build --release

# Install the wheel
pip install target/wheels/pyoxigraph-*.whl
```

#### For Multiple Python Versions

```bash
# Build for all installed Python versions
maturin build --release --compatibility manylinux2014

# Build with cpython stable ABI (works across Python versions)
maturin build --release --features abi3
```

### Build JavaScript Bindings

Requirements:
- Rust with wasm32-unknown-unknown target
- wasm-pack
- Node.js 18+

```bash
# Install wasm32 target
rustup target add wasm32-unknown-unknown

# Install wasm-pack
cargo install wasm-pack

# Build
cd js
npm install
npm run build
```

This creates the package in `js/pkg/`.

#### Test the Build

```bash
npm test
```

### Build CLI Server

```bash
cd cli
cargo build --release

# Binary is at: ../target/release/oxigraph
```

#### With Specific Features

```bash
# With rustls instead of native TLS
cargo build --release --no-default-features --features rustls-native

# Link against system RocksDB
cargo build --release --features rocksdb-pkg-config
```

### Build Everything

From the root directory:

```bash
# Check all crates
cargo check --all

# Build all crates
cargo build --all --release

# Run all tests
cargo test --all
```

---

## Platform-Specific Notes

### Linux

#### Alpine Linux

Alpine uses musl libc, which requires additional setup:

```bash
# Install dependencies
apk add build-base clang-dev git

# Build
cargo build --release
```

#### CentOS/RHEL 7

Requires newer compiler:

```bash
# Enable devtoolset
yum install centos-release-scl
yum install devtoolset-11
scl enable devtoolset-11 bash

# Then build normally
cargo build --release
```

### macOS

#### Apple Silicon (M1/M2)

Everything should work natively:

```bash
cargo build --release
```

#### Rosetta 2 (Intel emulation)

If you need Intel binaries:

```bash
rustup target add x86_64-apple-darwin
cargo build --release --target x86_64-apple-darwin
```

### Windows

#### Using MSVC

Recommended approach:

1. Install Visual Studio Build Tools
2. Install Rust (will auto-detect MSVC)
3. Build:

```powershell
cargo build --release
```

#### Using MinGW

Alternative approach:

```bash
rustup target add x86_64-pc-windows-gnu
cargo build --release --target x86_64-pc-windows-gnu
```

### WebAssembly

For browser/WASM builds:

```bash
rustup target add wasm32-unknown-unknown
cd js
wasm-pack build --target web
```

---

## Cross-Compilation

### For ARM64 (aarch64)

From x86_64 Linux:

```bash
# Install cross-compilation tools
sudo apt-get install gcc-aarch64-linux-gnu

# Add target
rustup target add aarch64-unknown-linux-gnu

# Configure cargo
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER=aarch64-linux-gnu-gcc

# Build
cargo build --release --target aarch64-unknown-linux-gnu
```

### Using cross

Easiest cross-compilation tool:

```bash
cargo install cross

# Build for any target
cross build --release --target aarch64-unknown-linux-gnu
cross build --release --target x86_64-pc-windows-gnu
```

---

## Docker Build

### Build Your Own Image

```bash
cd cli
docker build -t my-oxigraph -f Dockerfile ..
```

### Multi-arch Build

```bash
docker buildx build \
    --platform linux/amd64,linux/arm64 \
    -t my-oxigraph:latest \
    -f cli/Dockerfile \
    .
```

---

## Troubleshooting

### Rust Build Issues

#### "Could not find LLVM"

Install Clang/LLVM:

```bash
# Ubuntu/Debian
sudo apt-get install clang

# macOS
xcode-select --install

# Windows
# Download and install from https://releases.llvm.org/
```

#### "linking with cc failed"

Install build tools:

```bash
# Ubuntu/Debian
sudo apt-get install build-essential

# Fedora/RHEL
sudo dnf install gcc gcc-c++
```

#### Submodule Issues

```bash
# Reset submodules
git submodule deinit -f .
git submodule update --init --recursive
```

### Python Build Issues

#### "No matching distribution found"

Your platform may not have pre-built wheels. Build from source:

```bash
pip install maturin
git clone --recursive https://github.com/oxigraph/oxigraph.git
cd oxigraph/python
maturin build --release
pip install target/wheels/*.whl
```

#### "ImportError: cannot import name 'Store'"

Python version mismatch. Reinstall:

```bash
pip uninstall pyoxigraph
pip install --no-cache-dir pyoxigraph
```

### JavaScript Build Issues

#### "WebAssembly module is not available"

You may need to initialize WASM:

```javascript
import init, * as oxigraph from 'oxigraph/web.js';
await init();
```

#### "Cannot find module 'oxigraph/node.js'"

Specify the correct import path:

```javascript
// For Node.js
import oxigraph from 'oxigraph/node.js';

// For browser
import oxigraph from 'oxigraph/web.js';
```

#### Node.js version too old

```bash
# Check version
node --version

# Update Node.js (using nvm)
nvm install 18
nvm use 18
```

### CLI Issues

#### "Permission denied"

On Linux/macOS, make the binary executable:

```bash
chmod +x oxigraph
```

#### Port already in use

Change the port:

```bash
oxigraph serve --location ./data --bind localhost:8080
```

Or find and kill the process:

```bash
# Linux/macOS
lsof -i :7878
kill -9 <PID>

# Windows
netstat -ano | findstr :7878
taskkill /PID <PID> /F
```

### RocksDB Issues

#### "RocksDB: IO error"

Disk permission or space issue:

```bash
# Check permissions
ls -la ./data

# Check disk space
df -h
```

#### Use system RocksDB

If bundled RocksDB fails:

```bash
# Install RocksDB
sudo apt-get install librocksdb-dev  # Ubuntu/Debian
sudo dnf install rocksdb-devel        # Fedora/RHEL

# Build with system RocksDB
cargo build --features rocksdb-pkg-config
```

### Memory Issues

If builds fail with OOM:

```bash
# Increase memory for cargo
export CARGO_BUILD_JOBS=1

# Or limit parallel jobs
cargo build --release -j 2
```

---

## Performance Tuning

### RocksDB Optimization

For better performance, use these environment variables:

```bash
# Increase RocksDB memory budget
export ROCKSDB_TOTAL_WRITE_BUFFER_SIZE=2147483648  # 2GB

# Optimize for your workload
export ROCKSDB_MAX_BACKGROUND_JOBS=8
```

### Build Optimizations

For maximum performance, use:

```bash
# Link-time optimization
cargo build --release

# Or add to Cargo.toml:
[profile.release]
lto = true
codegen-units = 1
```

---

## Verification

After installation, verify everything works:

### Rust

```bash
cargo test -p oxigraph
```

### Python

```bash
python -c "from pyoxigraph import Store; s = Store(); print('OK')"
```

### JavaScript

```bash
node -e "const ox = require('oxigraph'); console.log('OK')"
```

### CLI

```bash
oxigraph --version
```

---

## Getting Help

If you encounter issues not covered here:

1. Check [GitHub Issues](https://github.com/oxigraph/oxigraph/issues)
2. Ask in [GitHub Discussions](https://github.com/oxigraph/oxigraph/discussions)
3. Join [Gitter Chat](https://gitter.im/oxigraph/community)
4. Read the [FAQ](faq.md)

For bugs, please [file an issue](https://github.com/oxigraph/oxigraph/issues/new) with:
- Your platform (OS, version)
- Rust/Python/Node.js version
- Full error message
- Steps to reproduce

---

## Next Steps

- [Quick Start Guide](quick-start.md) - Get started quickly
- [FAQ](faq.md) - Common questions
- [Contributing](CONTRIBUTING.md) - Help improve Oxigraph
