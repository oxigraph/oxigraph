# Developer Onboarding Guide

Welcome to Oxigraph! This guide will get you from zero to productive in the shortest time possible. Follow this guide step-by-step to ensure a smooth onboarding experience.

## Pre-flight Checklist

Before you begin, verify you have the necessary tools and system requirements.

### System Requirements

**Minimum Specifications:**
- **OS**: Linux, macOS, Windows, or any platform supporting Rust/WASM
- **RAM**: 2GB minimum, 8GB+ recommended for development
- **Disk**: 5GB free space for dependencies and builds
- **Internet**: Required for downloading dependencies

**Choose Your Platform:**

<details>
<summary><b>Rust Development</b></summary>

- Rust 1.70+ (latest stable recommended)
- Clang/LLVM (for RocksDB bindings)
- Git with submodule support
- Code editor with Rust support (VS Code, IntelliJ IDEA, etc.)

</details>

<details>
<summary><b>Python Development</b></summary>

- Python 3.8+ (3.11+ recommended)
- pip 19.0+ or conda
- Virtual environment tool (venv, conda, poetry)
- IDE with Python support (VS Code, PyCharm, etc.)

</details>

<details>
<summary><b>JavaScript Development</b></summary>

- Node.js 18+ (LTS version recommended)
- npm, yarn, or pnpm
- Modern browser (Chrome 90+, Firefox 89+, Safari 15+, Edge 90+)
- IDE with TypeScript support (VS Code recommended)

</details>

<details>
<summary><b>Using CLI/Server Only</b></summary>

- Docker, OR
- One of the above development environments, OR
- Pre-built binary for your platform

</details>

### Pre-flight Verification

Run these commands to verify your environment:

**For Rust:**
```bash
rustc --version  # Should be 1.70 or higher
cargo --version
clang --version  # Required for RocksDB
git --version
```

**For Python:**
```bash
python --version  # Should be 3.8 or higher
pip --version
```

**For JavaScript:**
```bash
node --version  # Should be 18 or higher
npm --version
```

**For Docker:**
```bash
docker --version
docker run hello-world  # Verify Docker is working
```

---

## Installation by Platform

Choose your platform and follow the corresponding guide.

### Option 1: Rust Setup (Recommended for Core Development)

#### Step 1: Install Rust Toolchain

```bash
# Install Rust using rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Follow the prompts, then:
source $HOME/.cargo/env

# Verify installation
rustc --version
cargo --version
```

#### Step 2: Install Build Dependencies

**Ubuntu/Debian:**
```bash
sudo apt-get update
sudo apt-get install -y build-essential clang libclang-dev git pkg-config
```

**Fedora/RHEL/CentOS:**
```bash
sudo dnf install -y clang gcc gcc-c++ git pkg-config
```

**macOS:**
```bash
xcode-select --install
# If prompted, install the full Xcode command line tools
```

**Windows:**
1. Install [Visual Studio Build Tools](https://visualstudio.microsoft.com/downloads/)
   - Select "Desktop development with C++"
2. Install [LLVM](https://releases.llvm.org/download.html)
3. Restart your terminal

#### Step 3: Clone Oxigraph Repository

```bash
# Clone with all submodules
git clone --recursive https://github.com/oxigraph/oxigraph.git
cd oxigraph

# Or if already cloned:
git submodule update --init --recursive
```

#### Step 4: First Build

```bash
# Build all crates (this will take 5-10 minutes on first run)
cargo build --all

# Run tests to verify everything works
cargo test --lib -p oxigraph

# Expected: All tests should pass
```

#### Step 5: Verify Success

```bash
# Build the CLI server
cargo build -p oxigraph-cli --release

# Run it
./target/release/oxigraph --version

# You should see version information
```

**Success Indicator:** You can run `oxigraph --version` and see output like:
```
oxigraph 0.4.0
```

---

### Option 2: Python Setup

#### Step 1: Set Up Python Environment

**Using venv (Recommended):**
```bash
# Create a virtual environment
python -m venv oxigraph-env

# Activate it
source oxigraph-env/bin/activate  # Linux/macOS
# OR
oxigraph-env\Scripts\activate  # Windows
```

**Using conda:**
```bash
# Create conda environment
conda create -n oxigraph python=3.11
conda activate oxigraph
```

#### Step 2: Install PyOxigraph

```bash
# Install from PyPI
pip install pyoxigraph

# Or with conda
conda install -c conda-forge pyoxigraph
```

#### Step 3: Verify Installation

```bash
# Test the installation
python -c "import pyoxigraph; print(f'PyOxigraph {pyoxigraph.__version__} installed successfully!')"
```

#### Step 4: First Success Test

Create a file `test_oxigraph.py`:

```python
from pyoxigraph import Store, NamedNode, Literal, Quad, DefaultGraph

# Create store
store = Store()

# Add data
ex = NamedNode("http://example.org/test")
name = NamedNode("http://schema.org/name")
store.add(Quad(ex, name, Literal("Success!"), DefaultGraph()))

# Query
for result in store.query("SELECT ?value WHERE { ?s ?p ?value }"):
    print(f"✓ Oxigraph is working! Got: {result['value'].value}")
```

Run it:
```bash
python test_oxigraph.py

# Expected output:
# ✓ Oxigraph is working! Got: Success!
```

**Success Indicator:** You see the success message printed.

---

### Option 3: JavaScript Setup

#### Step 1: Set Up Node.js Project

```bash
# Create a new directory
mkdir my-oxigraph-project
cd my-oxigraph-project

# Initialize npm project
npm init -y
```

#### Step 2: Install Oxigraph

```bash
npm install oxigraph

# Verify installation
npm list oxigraph
```

#### Step 3: First Success Test

Create `test-oxigraph.js`:

```javascript
const oxigraph = require('oxigraph');

// Create store
const store = new oxigraph.Store();

// Add data
const ex = oxigraph.namedNode("http://example.org/test");
const name = oxigraph.namedNode("http://schema.org/name");
store.add(oxigraph.triple(ex, name, oxigraph.literal("Success!")));

// Query
for (const binding of store.query("SELECT ?value WHERE { ?s ?p ?value }")) {
    console.log(`✓ Oxigraph is working! Got: ${binding.get("value").value}`);
}
```

Run it:
```bash
node test-oxigraph.js

# Expected output:
# ✓ Oxigraph is working! Got: Success!
```

**Success Indicator:** You see the success message printed.

**For TypeScript:**

```bash
npm install --save-dev typescript @types/node

# Create tsconfig.json
npx tsc --init
```

Create `test-oxigraph.ts`:
```typescript
import * as oxigraph from 'oxigraph';

const store = new oxigraph.Store();
console.log("✓ TypeScript + Oxigraph working!");
```

---

### Option 4: Docker Setup (Easiest for Server)

#### Step 1: Pull Docker Image

```bash
docker pull ghcr.io/oxigraph/oxigraph:latest
```

#### Step 2: Run Server

```bash
# Create data directory
mkdir -p oxigraph-data

# Start server
docker run -d \
  --name oxigraph \
  -v $(pwd)/oxigraph-data:/data \
  -p 7878:7878 \
  ghcr.io/oxigraph/oxigraph:latest \
  serve --location /data --bind 0.0.0.0:7878
```

#### Step 3: Verify Server is Running

```bash
# Check container status
docker ps | grep oxigraph

# Test the endpoint
curl http://localhost:7878/

# Or open in browser: http://localhost:7878
```

**Success Indicator:** You see the YASGUI web interface when you visit http://localhost:7878

---

## First Success: Hello World Examples

These are minimal examples that prove your installation works.

### Rust Hello World

```rust
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;
    println!("✓ Rust: Oxigraph store created successfully!");
    Ok(())
}
```

Run:
```bash
cargo add oxigraph  # In your project
cargo run
```

### Python Hello World

```python
from pyoxigraph import Store

store = Store()
print("✓ Python: Oxigraph store created successfully!")
```

### JavaScript Hello World

```javascript
const oxigraph = require('oxigraph');

const store = new oxigraph.Store();
console.log("✓ JavaScript: Oxigraph store created successfully!");
```

### CLI Hello World

```bash
# Load sample data
echo '<http://example.org/s> <http://example.org/p> "Hello Oxigraph" .' > hello.ttl

# Start server with data
docker run -p 7878:7878 -v $(pwd):/data ghcr.io/oxigraph/oxigraph:latest \
  serve --location /tmp/db --bind 0.0.0.0:7878

# Load data
curl -X POST -H 'Content-Type: text/turtle' -T hello.ttl http://localhost:7878/store

# Query
curl -X POST -H 'Content-Type: application/sparql-query' \
  --data 'SELECT * WHERE { ?s ?p ?o }' \
  http://localhost:7878/query
```

---

## Common Environment Issues & Solutions

### Issue: "Could not find LLVM" (Rust)

**Symptoms:** Build fails with LLVM-related errors

**Solution:**

**Linux:**
```bash
sudo apt-get install clang libclang-dev llvm
```

**macOS:**
```bash
xcode-select --install
brew install llvm
```

**Windows:**
Download and install LLVM from https://releases.llvm.org/download.html

---

### Issue: "No matching distribution found" (Python)

**Symptoms:** `pip install pyoxigraph` fails

**Solution:**

**Option 1: Update pip and retry**
```bash
pip install --upgrade pip
pip install pyoxigraph
```

**Option 2: Use conda**
```bash
conda install -c conda-forge pyoxigraph
```

**Option 3: Build from source**
```bash
pip install maturin
git clone --recursive https://github.com/oxigraph/oxigraph.git
cd oxigraph/python
maturin build --release
pip install target/wheels/*.whl
```

---

### Issue: "WebAssembly module is not available" (JavaScript)

**Symptoms:** Import errors or WASM initialization failures

**Solution:**

**For Node.js:**
```bash
# Check Node version (must be 18+)
node --version

# Update if needed using nvm
nvm install 18
nvm use 18
```

**For Browser:**
```javascript
import init, * as oxigraph from 'oxigraph/web.js';

// Initialize WASM before use
await init();

const store = new oxigraph.Store();
```

---

### Issue: Port Already in Use (CLI Server)

**Symptoms:** Server fails to start, "address already in use"

**Solution:**

**Option 1: Use a different port**
```bash
docker run -p 8080:7878 ... # Use port 8080 instead
```

**Option 2: Stop the conflicting process**
```bash
# Linux/macOS
lsof -i :7878
kill -9 <PID>

# Windows
netstat -ano | findstr :7878
taskkill /PID <PID> /F
```

---

### Issue: Submodule Errors (Rust)

**Symptoms:** "Could not find crate" or "no such file or directory"

**Solution:**
```bash
# Reset and update submodules
git submodule deinit -f .
git submodule update --init --recursive

# Then rebuild
cargo clean
cargo build
```

---

### Issue: Permission Denied (Linux/macOS)

**Symptoms:** Cannot write to data directory

**Solution:**
```bash
# Fix permissions on data directory
chmod -R 755 ./data
chown -R $USER:$USER ./data

# For Docker, ensure volume mount has correct permissions
docker run --user $(id -u):$(id -g) ...
```

---

## IDE Setup

### VS Code (Recommended for All Platforms)

#### For Rust

1. Install VS Code from https://code.visualstudio.com/

2. Install extensions:
   ```bash
   code --install-extension rust-lang.rust-analyzer
   code --install-extension serayuzgur.crates
   code --install-extension vadimcn.vscode-lldb  # For debugging
   ```

3. Configure `.vscode/settings.json`:
   ```json
   {
     "rust-analyzer.checkOnSave.command": "clippy",
     "rust-analyzer.cargo.features": "all"
   }
   ```

4. Configure debugging in `.vscode/launch.json`:
   ```json
   {
     "version": "0.2.0",
     "configurations": [
       {
         "type": "lldb",
         "request": "launch",
         "name": "Debug Oxigraph",
         "cargo": {
           "args": ["build", "--bin=oxigraph", "--package=oxigraph-cli"]
         },
         "args": [],
         "cwd": "${workspaceFolder}"
       }
     ]
   }
   ```

#### For Python

1. Install extensions:
   ```bash
   code --install-extension ms-python.python
   code --install-extension ms-python.vscode-pylance
   code --install-extension ms-python.debugpy
   ```

2. Select your virtual environment:
   - Press `Ctrl+Shift+P` (or `Cmd+Shift+P` on macOS)
   - Type "Python: Select Interpreter"
   - Choose your oxigraph-env or conda environment

3. Configure debugging in `.vscode/launch.json`:
   ```json
   {
     "version": "0.2.0",
     "configurations": [
       {
         "name": "Python: Current File",
         "type": "python",
         "request": "launch",
         "program": "${file}",
         "console": "integratedTerminal"
       }
     ]
   }
   ```

#### For JavaScript/TypeScript

1. Install extensions:
   ```bash
   code --install-extension dbaeumer.vscode-eslint
   code --install-extension esbenp.prettier-vscode
   ```

2. Configure `.vscode/settings.json`:
   ```json
   {
     "editor.formatOnSave": true,
     "editor.defaultFormatter": "esbenp.prettier-vscode"
   }
   ```

3. Configure debugging in `.vscode/launch.json`:
   ```json
   {
     "version": "0.2.0",
     "configurations": [
       {
         "type": "node",
         "request": "launch",
         "name": "Launch Program",
         "skipFiles": ["<node_internals>/**"],
         "program": "${workspaceFolder}/index.js"
       }
     ]
   }
   ```

---

### IntelliJ IDEA / CLion (for Rust)

1. Install Rust plugin:
   - Go to Settings → Plugins
   - Search for "Rust"
   - Install and restart

2. Import project:
   - File → Open → Select oxigraph directory
   - Choose "Import as Cargo project"

3. Set up debugging:
   - Run → Edit Configurations
   - Add new "Cargo Command" configuration
   - Set command: `run --bin oxigraph`

---

### PyCharm (for Python)

1. Open the project directory

2. Configure interpreter:
   - File → Settings → Project → Python Interpreter
   - Add interpreter → Select your virtual environment

3. Install pyoxigraph:
   - Terminal in PyCharm: `pip install pyoxigraph`

4. Set up debugging:
   - Run → Edit Configurations
   - Add new Python configuration
   - Set script path to your Python file

---

## Debugging Configuration

### Rust Debugging

**Enable backtraces:**
```bash
RUST_BACKTRACE=1 cargo run
RUST_BACKTRACE=full cargo run  # More detailed
```

**Enable logging:**
```bash
RUST_LOG=debug cargo run
RUST_LOG=oxigraph=trace cargo run  # Oxigraph-specific logs
```

**Debug with VS Code:**
Set breakpoints in code, then press F5 to start debugging.

---

### Python Debugging

**Enable verbose output:**
```python
import logging
logging.basicConfig(level=logging.DEBUG)
```

**Use pdb for interactive debugging:**
```python
import pdb; pdb.set_trace()  # Set breakpoint
```

**Debug with VS Code:**
Set breakpoints, press F5.

---

### JavaScript Debugging

**Node.js debugging:**
```bash
node --inspect-brk test-oxigraph.js

# Then attach VS Code debugger or Chrome DevTools
```

**Browser debugging:**
- Open Chrome DevTools (F12)
- Go to Sources tab
- Set breakpoints in your code
- Refresh page

---

## Verification Checklist

Before moving forward, ensure you can:

- [ ] Install Oxigraph in your chosen environment
- [ ] Create a store successfully
- [ ] Add a triple/quad to the store
- [ ] Execute a simple SPARQL query
- [ ] Load data from a file or string
- [ ] View results in your terminal/console
- [ ] (Optional) Run the server and access via browser
- [ ] (Optional) Set breakpoints and debug code

---

## Next Steps

Congratulations! You've successfully set up Oxigraph. Here's what to do next:

1. **Follow the Learning Path:** See [learning-path.md](learning-path.md) for structured tutorials from beginner to advanced

2. **Try the Quick Start:** Check out [quick-start.md](quick-start.md) for more complete examples

3. **Reference the Cheatsheet:** Keep [cheatsheet.md](cheatsheet.md) handy for quick lookups

4. **Explore Tutorials:**
   - [Rust Getting Started](tutorials/rust-getting-started.md)
   - [Python Getting Started](tutorials/python-getting-started.md)
   - [JavaScript Getting Started](tutorials/javascript-getting-started.md)

5. **Join the Community:**
   - [GitHub Discussions](https://github.com/oxigraph/oxigraph/discussions)
   - [Gitter Chat](https://gitter.im/oxigraph/community)

---

## Getting Help

If you encounter issues not covered here:

1. **Check the FAQ:** [faq.md](faq.md)
2. **Search GitHub Issues:** [github.com/oxigraph/oxigraph/issues](https://github.com/oxigraph/oxigraph/issues)
3. **Ask in Discussions:** [github.com/oxigraph/oxigraph/discussions](https://github.com/oxigraph/oxigraph/discussions)
4. **Join Gitter Chat:** [gitter.im/oxigraph/community](https://gitter.im/oxigraph/community)

When asking for help, include:
- Your platform (OS, version)
- Rust/Python/Node.js version
- Full error message
- Steps to reproduce

---

**You're now ready to build with Oxigraph! Happy coding!**
