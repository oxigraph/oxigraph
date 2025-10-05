# Oxigraph WASM for Typst Plugins Specification

## Overview

This specification outlines the implementation of a Typst-compatible WASM build of Oxigraph using a dedicated `oxigraph-wasm` feature flag. The goal is to create a pure `wasm32-unknown-unknown` build that can function as a Typst plugin without JavaScript runtime dependencies.

## Problem Statement

Current Oxigraph WASM builds are incompatible with Typst plugins due to:

1. **Target Mismatch**: Existing WASM uses `wasm32-unknown-unknown` with `wasm-bindgen` for JavaScript interop
2. **Dependency Conflicts**: Dependencies like `getrandom` assume JavaScript Web APIs or native environments
3. **API Incompatibility**: Current exports are JavaScript-specific, not C-compatible as required by Typst

## Solution: `oxigraph-wasm` Feature Flag

### Feature Configuration

```toml
# lib/oxigraph/Cargo.toml
[features]
oxigraph-wasm = [
    "oxsdatatypes/custom-now",
    "dep:wasm-minimal"  # hypothetical minimal WASM runtime
]
```

### Key Changes

#### 1. Dependency Management
- **Remove**: `getrandom/wasm_js`, `js-sys`, `wasm-bindgen`
- **Disable**: RocksDB backend (file system incompatible)
- **Enable**: Memory-only storage backend
- **Use**: `custom-now` feature for time operations

#### 2. Time Provider Implementation
```rust
// Custom implementation for Typst environment
#[no_mangle]
pub extern "C" fn custom_ox_now() -> Duration {
    // Return Unix epoch or fixed time
    // Suitable for document generation workflows
    Duration::from_seconds_since_unix_epoch(0)
}
```

#### 3. Storage Backend
- **Primary**: In-memory storage only
- **Rationale**: File system access unavailable in Typst plugin sandbox
- **Trade-off**: Data persistence vs. compatibility

#### 4. Export Interface
```rust
// C-compatible exports for Typst plugin system
#[no_mangle]
pub extern "C" fn oxigraph_create_store() -> *mut Store;

#[no_mangle]
pub extern "C" fn oxigraph_query_sparql(
    store: *mut Store,
    query: *const c_char,
    result: *mut c_char,
    result_len: usize
) -> i32;

#[no_mangle]
pub extern "C" fn oxigraph_add_triple(
    store: *mut Store,
    subject: *const c_char,
    predicate: *const c_char,
    object: *const c_char
) -> i32;
```

## Build Configuration

### Compilation Target
```bash
cargo build --target wasm32-unknown-unknown \
    --no-default-features \
    --features oxigraph-wasm
```

### Profile Configuration
```toml
[profile.wasm]
inherits = "release"
opt-level = "z"          # Minimize size
lto = true
codegen-units = 1
panic = "abort"
strip = true
```

## Implementation Phases

### Phase 1: Feature Flag Setup
- Add `oxigraph-wasm` feature to workspace dependencies
- Configure conditional compilation for WASM-specific code
- Disable incompatible features (RocksDB, HTTP client)

### Phase 2: Dependency Resolution
- Implement custom time provider using `custom-now` feature
- Remove JavaScript-specific dependencies
- Create minimal WASM runtime adapter

### Phase 3: Memory Store Implementation
- Ensure in-memory storage works without file system
- Test RDF operations with memory-only backend
- Validate SPARQL query execution

### Phase 4: C-Compatible Exports
- Design C API for Typst plugin consumption
- Implement memory management for cross-boundary calls
- Create error handling compatible with C interface

### Phase 5: Testing & Validation
- Test compilation with pure `wasm32-unknown-unknown`
- Validate basic RDF operations work correctly
- Ensure no JavaScript runtime dependencies remain

## Limitations & Trade-offs

### Limitations
1. **No Persistence**: Memory-only storage, data lost between sessions
2. **Fixed Time**: DateTime operations return static values
3. **Reduced Functionality**: Some HTTP and file system features unavailable
4. **Size Constraints**: Must fit within Typst plugin size limits

### Acceptable Trade-offs
- **Performance vs. Compatibility**: Slower operations acceptable for document generation
- **Features vs. Size**: Minimal feature set for compatibility
- **Persistence vs. Portability**: In-memory storage for pure WASM compatibility

## Success Criteria

1. ✅ Compiles successfully with `--target wasm32-unknown-unknown --features oxigraph-wasm`
2. ✅ No dependencies on JavaScript runtime or Web APIs
3. ✅ Exports C-compatible functions for Typst consumption
4. ✅ Basic RDF operations (add, query) function correctly
5. ✅ WASM binary size within reasonable limits for plugin usage

## Future Enhancements

- **Custom Allocators**: Optimize memory usage for plugin environment
- **Streaming Parsers**: Handle large RDF documents efficiently
- **Plugin-Specific APIs**: Tailored interfaces for common Typst use cases
- **Error Recovery**: Robust error handling for document processing workflows
