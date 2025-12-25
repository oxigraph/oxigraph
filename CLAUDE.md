# Claude Code Guide for Oxigraph

## Project Overview

Oxigraph is a graph database implementing the SPARQL standard. It provides:
- A Rust library (`oxigraph`)
- A standalone server (`oxigraph-server`)
- Python bindings (`pyoxigraph`)
- JavaScript/WASM bindings (`oxigraph` npm package)

## Repository Structure

```
oxigraph/
├── lib/                    # Core Rust libraries
│   ├── oxigraph/          # Main database library
│   ├── oxrdf/             # RDF data model
│   ├── oxrdfio/           # RDF I/O (parsing/serialization)
│   ├── oxttl/             # Turtle/N-Triples/N-Quads/TriG parser
│   ├── oxrdfxml/          # RDF/XML parser
│   ├── oxjsonld/          # JSON-LD parser
│   ├── spargebra/         # SPARQL algebra
│   ├── spareval/          # SPARQL evaluation
│   ├── sparopt/           # SPARQL optimizer
│   ├── sparesults/        # SPARQL results formats
│   ├── sparshacl/         # SHACL validation
│   ├── spargeo/           # GeoSPARQL extension
│   ├── sparql-smith/      # SPARQL fuzzing
│   └── oxsdatatypes/      # XSD datatypes
├── cli/                   # Command-line server (oxigraph-server)
├── python/                # Python bindings (pyoxigraph)
├── js/                    # JavaScript/WASM bindings
├── testsuite/             # W3C test suites
├── bench/                 # Benchmarks
└── fuzz/                  # Fuzzing targets
```

## Build Commands

```bash
# Check all crates
cargo check --all

# Build everything
cargo build --all

# Run tests
cargo test --all

# Build CLI server
cargo build -p oxigraph-cli --release

# Build Python wheel
cd python && maturin build --release

# Build JS/WASM
cd js && wasm-pack build --target web
```

## Core Crates

### oxrdf - RDF Data Model
Located in `lib/oxrdf/`. Defines core RDF types:
- `NamedNode` - IRI references
- `BlankNode` - Blank nodes
- `Literal` - RDF literals with datatype/language
- `Term` - Union of node types
- `Triple`, `Quad` - RDF statements
- `Dataset` - In-memory quad collection
- `Graph` - In-memory triple collection

### oxigraph - Database
Located in `lib/oxigraph/`. Main database with:
- `Store` - Persistent RocksDB-backed store
- `MemoryStore` - In-memory store
- SPARQL query and update execution
- Transaction support

### SPARQL Stack
- `spargebra` - SPARQL algebra representation
- `spareval` - Query evaluation engine
- `sparopt` - Query optimization
- `sparesults` - Results serialization (JSON, XML, CSV, TSV)

### I/O Stack
- `oxrdfio` - Unified RDF I/O interface
- `oxttl` - Turtle family parsers
- `oxrdfxml` - RDF/XML parser
- `oxjsonld` - JSON-LD parser

## Coding Conventions

### Rust Style
- Follow standard Rust conventions
- Use `clippy` for linting: `cargo clippy --all`
- Format with `rustfmt`: `cargo fmt --all`
- Error types implement `std::error::Error`
- Use `thiserror` for error definitions

### Public API Patterns
```rust
// Use Result for fallible operations
pub fn parse(input: &str) -> Result<T, ParseError>

// Use impl Iterator for lazy evaluation
pub fn iter(&self) -> impl Iterator<Item = &Quad>

// Use Into<T> for flexible parameters
pub fn add(&mut self, quad: impl Into<Quad>)
```

### Testing
- Unit tests in same file with `#[cfg(test)]`
- Integration tests in `tests/` directory
- Use W3C test suites in `testsuite/`

## JavaScript Bindings (js/)

### Architecture
```
js/src/
├── lib.rs       # Entry point
├── model.rs     # RDF terms, Dataset
├── store.rs     # Store with SPARQL
├── io.rs        # RDF parsing/serialization
├── sparql.rs    # SPARQL result types
├── shacl.rs     # SHACL validation
└── utils.rs     # Utilities
```

### Key Conventions

**camelCase Naming:**
```rust
#[wasm_bindgen(js_name = namedGraphs)]
pub fn named_graphs(&self) -> Result<Box<[JsValue]>, JsValue>
```

**TypeScript Custom Sections:**
```rust
#[wasm_bindgen(typescript_custom_section)]
const TS: &str = r###"
export class Store {
    readonly size: number;
    // ...
}
"###;
```

**Symbol.iterator via build script:**
```rust
#[wasm_bindgen(skip_typescript)]
pub fn __iterator(&self) -> JsValue { ... }
```

Wired in `build_package.mjs`:
```javascript
[Symbol.iterator]() { return this.__iterator(); }
```

**Collection Methods Pattern:**
```rust
#[wasm_bindgen(js_name = forEach)]
pub fn for_each(&self, callback: &Function, this_arg: &JsValue) -> Result<(), JsValue> {
    let this = if this_arg.is_undefined() { JsValue::NULL } else { this_arg.clone() };
    // ...
}
```

### JS API Surface
- **Store**: Full Array API (forEach, map, filter, reduce, find, etc.)
- **Dataset**: In-memory with same collection methods
- **RdfFormat**: Static constants (TURTLE, N_TRIPLES, etc.)
- **QueryResultsFormat**: CSV, JSON, TSV, XML
- **SHACL**: Validation with ShaclValidator

## Python Bindings (python/)

### Architecture
Built with PyO3 and maturin. Key files:
- `src/store.rs` - Store class
- `src/model.rs` - RDF terms
- `src/io.rs` - Parsing/serialization
- `src/sparql.rs` - Query results

### Build
```bash
cd python
maturin develop  # Development build
maturin build --release  # Release wheel
```

## CLI Server (cli/)

### Features
- HTTP SPARQL endpoint
- SPARQL Graph Store Protocol
- Multiple RDF format support
- Persistent storage with RocksDB

### Build & Run
```bash
cargo build -p oxigraph-cli --release
./target/release/oxigraph serve --location ./data
```

## Feature Flags

### oxigraph crate
- `rocksdb` - Enable RocksDB backend (default)
- `http-client` - Enable HTTP client for SERVICE queries

### oxigraph-js crate
- `geosparql` - Enable GeoSPARQL functions
- `rdf-12` - Enable RDF 1.2 features (directional language tags)

## Testing

### Run All Tests
```bash
cargo test --all
```

### W3C Test Suites
```bash
cargo test -p oxigraph --test testsuite
```

### Python Tests
```bash
cd python && python -m pytest
```

### JS Tests
```bash
cd js && npm test
```

## Performance Tips

1. **Bulk Loading**: Use `bulk_loader()` for large imports
2. **Transactions**: Batch writes in transactions
3. **Query Optimization**: SPARQL optimizer handles most cases
4. **Indexes**: Store maintains SPO, POS, OSP indexes

## Common Tasks

### Add New RDF Format
1. Create parser in `lib/ox<format>/`
2. Register in `lib/oxrdfio/src/format.rs`
3. Update `RdfFormat` enum
4. Add tests with W3C test suite

### Add SPARQL Function
1. Implement in `lib/spareval/src/function.rs`
2. Register in function registry
3. Add tests

### Add JS Method
1. Add Rust method in `js/src/*.rs`
2. Use `#[wasm_bindgen]` with `js_name`
3. Add TypeScript in custom section
4. Add test in `js/test/`

### Add Python Method
1. Add method in `python/src/*.rs`
2. Use `#[pymethods]` attribute
3. Add docstring for help()
4. Add test in `python/tests/`

## Debugging

### Rust
```bash
RUST_BACKTRACE=1 cargo test
RUST_LOG=debug cargo run
```

### WASM
```javascript
console.log(store.toString());
```

### Python
```python
import logging
logging.basicConfig(level=logging.DEBUG)
```

## Documentation

- Rust docs: `cargo doc --open`
- Python docs: Built-in docstrings
- JS docs: TypeScript definitions
- Website: https://oxigraph.org/

## CI/CD

GitHub Actions workflows in `.github/workflows/`:
- `build.yml` - Build and test all targets
- `release.yml` - Publish releases
- `docs.yml` - Deploy documentation
