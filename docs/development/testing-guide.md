# Testing Guide

This guide explains how to run tests, write new tests, and maintain test quality in Oxigraph.

## Table of Contents

- [Running Tests Locally](#running-tests-locally)
- [Test Categories](#test-categories)
- [Writing New Tests](#writing-new-tests)
- [Coverage Requirements](#coverage-requirements)
- [CI Test Configuration](#ci-test-configuration)
- [Troubleshooting](#troubleshooting)

---

## Running Tests Locally

### Quick Start

```bash
# Run all tests (Rust)
cargo test --all

# Run tests for specific crate
cargo test -p oxigraph
cargo test -p oxrdf
cargo test -p spargebra

# Run specific test
cargo test test_name

# Run with output visible
cargo test -- --nocapture

# Run with logging
RUST_LOG=debug cargo test
```

### Platform-Specific Tests

#### Rust Tests

```bash
# All Rust tests
cargo test --all

# With all features
cargo test --all --all-features

# Without default features
cargo test --no-default-features

# Release mode (slower compilation, faster execution)
cargo test --release
```

#### Python Tests

```bash
cd python

# Install development dependencies
pip install -e .[dev]
# or with uv
uv pip install -e .[dev]

# Run all tests
python -m pytest

# Run with coverage
python -m pytest --cov=pyoxigraph --cov-report=html

# Run specific test file
python -m pytest tests/test_store.py

# Run specific test
python -m pytest tests/test_store.py::test_insert_data

# Verbose output
python -m pytest -v
```

#### JavaScript Tests

```bash
cd js

# Install dependencies
npm install

# Run all tests
npm test

# Run with coverage
npm run test:coverage

# Run in watch mode
npm run test:watch

# Test specific file
npm test -- store.test.js
```

### W3C Test Suites

Oxigraph includes official W3C test suites for compliance testing:

```bash
# Run all W3C test suites
cargo test -p oxigraph --test testsuite

# Run with verbose output
cargo test -p oxigraph --test testsuite -- --nocapture

# Run specific test suite
cargo test -p oxigraph --test sparql11
cargo test -p oxigraph --test rdf12
```

### Performance Tests

```bash
# Run benchmarks
cargo bench

# Specific benchmark
cargo bench --bench query_evaluation

# With profiling
cargo bench --bench query_evaluation -- --profile-time=10
```

---

## Test Categories

### 1. Unit Tests

**Purpose:** Test individual functions and components in isolation.

**Location:** Same file as the code being tested, in `#[cfg(test)]` modules.

**Example:**
```rust
// lib/oxrdf/src/named_node.rs

impl NamedNode {
    pub fn new(iri: impl Into<String>) -> Result<Self, IriParseError> {
        // Implementation
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_iri() {
        let node = NamedNode::new("http://example.com/resource").unwrap();
        assert_eq!(node.as_str(), "http://example.com/resource");
    }

    #[test]
    fn test_invalid_iri() {
        assert!(NamedNode::new("not a valid iri").is_err());
    }

    #[test]
    fn test_iri_with_fragment() {
        let node = NamedNode::new("http://example.com/resource#section").unwrap();
        assert_eq!(node.as_str(), "http://example.com/resource#section");
    }
}
```

**Run:**
```bash
cargo test -p oxrdf test_valid_iri
```

### 2. Integration Tests

**Purpose:** Test interactions between multiple components.

**Location:** `tests/` directory in each crate.

**Example:**
```rust
// lib/oxigraph/tests/store_integration.rs

use oxigraph::store::Store;
use oxigraph::model::*;

#[test]
fn test_store_insert_and_query() {
    let store = Store::new().unwrap();

    // Insert data
    let ex = NamedNodeRef::new("http://example.com/").unwrap();
    store.insert(&QuadRef::new(
        ex.into_owned().as_ref(),
        ex.into_owned().as_ref(),
        LiteralRef::new_simple_literal("value"),
        GraphNameRef::DefaultGraph,
    )).unwrap();

    // Query data
    let results = store.query("SELECT * WHERE { ?s ?p ?o }").unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn test_transaction_rollback() {
    let store = Store::new().unwrap();

    // Start transaction
    let mut transaction = store.transaction(TransactionMode::Write).unwrap();

    // Insert data in transaction
    transaction.insert(&quad).unwrap();

    // Rollback
    drop(transaction);

    // Verify data was not persisted
    assert_eq!(store.len(), 0);
}
```

**Run:**
```bash
cargo test -p oxigraph --test store_integration
```

### 3. W3C Compliance Tests

**Purpose:** Ensure compliance with W3C standards (SPARQL, RDF, etc.).

**Location:** `testsuite/` directory with submodules to official W3C test suites.

**Test Suites Included:**
- SPARQL 1.1 Query
- SPARQL 1.1 Update
- SPARQL 1.1 JSON Results
- SPARQL 1.1 XML Results
- RDF 1.1 Turtle
- RDF 1.1 N-Triples
- RDF 1.1 N-Quads
- RDF 1.1 TriG
- RDF/XML
- JSON-LD
- SHACL

**Run:**
```bash
# All compliance tests
cargo test -p oxigraph --test testsuite

# Specific suite
RUST_TEST_THREADS=1 cargo test -p oxigraph --test testsuite sparql11_query
```

### 4. Fuzzing Tests

**Purpose:** Find edge cases and security issues through randomized testing.

**Location:** `fuzz/` directory.

**Run:**
```bash
# Install cargo-fuzz
cargo install cargo-fuzz

# Run specific fuzzer
cargo fuzz run sparql_parser

# Run with corpus
cargo fuzz run sparql_parser fuzz/corpus/sparql_parser

# Run for specific duration
cargo fuzz run sparql_parser -- -max_total_time=300
```

### 5. Benchmarks

**Purpose:** Track performance and prevent regressions.

**Location:** `bench/` directory.

**Run:**
```bash
# All benchmarks
cargo bench

# Specific benchmark
cargo bench --bench query_performance

# Compare with baseline
cargo bench --bench query_performance -- --save-baseline main
cargo bench --bench query_performance -- --baseline main
```

### 6. Documentation Tests

**Purpose:** Ensure code examples in documentation work correctly.

**Location:** Embedded in doc comments.

**Example:**
```rust
/// Parses a SPARQL query.
///
/// # Examples
///
/// ```
/// use oxigraph::sparql::Query;
///
/// let query = Query::parse("SELECT * WHERE { ?s ?p ?o }", None)?;
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
pub fn parse(query: &str, base_iri: Option<&str>) -> Result<Query> {
    // Implementation
}
```

**Run:**
```bash
# Test all documentation examples
cargo test --doc

# Specific crate
cargo test --doc -p oxigraph
```

---

## Writing New Tests

### Test Naming Conventions

```rust
// Unit tests: descriptive names
#[test]
fn test_named_node_creation_with_valid_iri() { }

#[test]
fn test_query_execution_with_optional_pattern() { }

// Integration tests: focus on scenario
#[test]
fn transaction_rollback_discards_changes() { }

#[test]
fn concurrent_reads_do_not_block() { }
```

### Test Structure: AAA Pattern

**Arrange, Act, Assert:**

```rust
#[test]
fn test_store_insert_and_retrieve() {
    // Arrange: Set up test data
    let store = Store::new().unwrap();
    let subject = NamedNode::new("http://example.com/s").unwrap();
    let predicate = NamedNode::new("http://example.com/p").unwrap();
    let object = Literal::new_simple_literal("value");
    let quad = Quad::new(subject, predicate, object, None);

    // Act: Perform the operation
    store.insert(&quad).unwrap();

    // Assert: Verify the result
    let results: Vec<_> = store.quads_for_pattern(None, None, None, None).collect();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0], quad);
}
```

### Testing Error Cases

```rust
#[test]
fn test_invalid_iri_returns_error() {
    let result = NamedNode::new("not a valid iri");
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(err.to_string().contains("invalid IRI"));
}

#[test]
#[should_panic(expected = "index out of bounds")]
fn test_index_out_of_bounds() {
    let vec = vec![1, 2, 3];
    let _ = vec[10];  // Should panic
}
```

### Testing Async Code

```rust
#[tokio::test]
async fn test_async_query() {
    let store = Store::new().unwrap();

    let result = store.query_async("SELECT * WHERE { ?s ?p ?o }").await;

    assert!(result.is_ok());
}
```

### Parameterized Tests

```rust
#[test]
fn test_rdf_formats() {
    let test_cases = vec![
        ("text/turtle", RdfFormat::Turtle),
        ("application/n-triples", RdfFormat::NTriples),
        ("application/rdf+xml", RdfFormat::RdfXml),
        ("application/ld+json", RdfFormat::JsonLd),
    ];

    for (mime_type, expected_format) in test_cases {
        let format = RdfFormat::from_mime_type(mime_type).unwrap();
        assert_eq!(format, expected_format, "Failed for {}", mime_type);
    }
}
```

### Test Fixtures

```rust
// Helper function for common setup
fn create_test_store_with_data() -> Store {
    let store = Store::new().unwrap();

    // Load test data
    let data = r#"
        @prefix ex: <http://example.com/> .
        ex:s1 ex:p1 "value1" .
        ex:s2 ex:p2 "value2" .
    "#;

    store.load_from_reader(
        RdfFormat::Turtle,
        data.as_bytes(),
    ).unwrap();

    store
}

#[test]
fn test_with_fixture() {
    let store = create_test_store_with_data();

    let count = store.len();
    assert_eq!(count, 2);
}
```

### Python Tests

```python
# python/tests/test_store.py

import pytest
from pyoxigraph import Store, NamedNode, Literal, Quad

class TestStore:
    def test_insert_and_query(self):
        """Test basic insert and query operations."""
        store = Store()

        # Insert data
        ex = NamedNode("http://example.com/")
        store.add(Quad(ex, ex, Literal("value")))

        # Query
        results = list(store.query("SELECT * WHERE { ?s ?p ?o }"))
        assert len(results) == 1

    def test_invalid_sparql_raises_error(self):
        """Test that invalid SPARQL raises appropriate error."""
        store = Store()

        with pytest.raises(SyntaxError):
            store.query("INVALID SPARQL")

    @pytest.mark.parametrize("format,data", [
        ("text/turtle", '<http://ex.com/s> <http://ex.com/p> "o" .'),
        ("application/n-triples", '<http://ex.com/s> <http://ex.com/p> "o" .'),
    ])
    def test_load_formats(self, format, data):
        """Test loading different RDF formats."""
        store = Store()
        store.load(data.encode(), mime_type=format)
        assert len(store) == 1
```

### JavaScript Tests

```javascript
// js/test/store.test.js

import { describe, it, expect, beforeEach } from 'vitest';
import oxigraph from '../pkg/node.js';

describe('Store', () => {
    let store;

    beforeEach(() => {
        store = new oxigraph.Store();
    });

    it('should insert and query data', () => {
        const ex = oxigraph.namedNode('http://example.com/');
        const quad = oxigraph.quad(ex, ex, oxigraph.literal('value'));

        store.add(quad);

        const results = [...store.match()];
        expect(results).toHaveLength(1);
    });

    it('should throw on invalid SPARQL', () => {
        expect(() => {
            store.query('INVALID SPARQL');
        }).toThrow();
    });

    it.each([
        ['text/turtle', '<http://ex.com/s> <http://ex.com/p> "o" .'],
        ['application/n-triples', '<http://ex.com/s> <http://ex.com/p> "o" .'],
    ])('should load %s format', (format, data) => {
        store.load(data, format);
        expect(store.size).toBe(1);
    });
});
```

---

## Coverage Requirements

### Minimum Coverage Targets

- **Rust crates:** 80% line coverage
- **Python bindings:** 85% coverage
- **JavaScript bindings:** 85% coverage
- **Critical paths:** 95% coverage (SPARQL evaluation, parsing)

### Checking Coverage

#### Rust Coverage (with tarpaulin)

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage report
cargo tarpaulin --all --out Html --output-dir coverage

# View report
open coverage/index.html

# CI-friendly output
cargo tarpaulin --all --out Xml
```

#### Rust Coverage (with llvm-cov)

```bash
# Install llvm-cov
cargo install cargo-llvm-cov

# Generate coverage
cargo llvm-cov --all --html

# Open report
cargo llvm-cov --all --open

# For CI
cargo llvm-cov --all --lcov --output-path lcov.info
```

#### Python Coverage

```bash
cd python

# Run with coverage
python -m pytest --cov=pyoxigraph --cov-report=html

# View report
open htmlcov/index.html

# Terminal report
python -m pytest --cov=pyoxigraph --cov-report=term

# CI report
python -m pytest --cov=pyoxigraph --cov-report=xml
```

#### JavaScript Coverage

```bash
cd js

# Run with coverage
npm run test:coverage

# View report
open coverage/index.html
```

### Coverage Best Practices

**Do:**
- Focus on meaningful coverage, not just numbers
- Test edge cases and error paths
- Cover public API surfaces completely
- Include integration tests in coverage

**Don't:**
- Write tests just to increase coverage percentage
- Ignore untested error paths
- Skip testing because "it's just a wrapper"

### Excluding Code from Coverage

```rust
// Exclude debug/display implementations
#[cfg(not(tarpaulin_include))]
impl fmt::Debug for MyStruct {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

// Exclude specific functions
#[cfg_attr(tarpaulin, skip)]
fn this_is_never_called_in_tests() {
    // ...
}
```

---

## CI Test Configuration

### GitHub Actions Workflows

Oxigraph uses GitHub Actions for continuous integration:

#### Test Workflow (`.github/workflows/tests.yml`)

Runs on every PR and push to `main`/`next`:

```yaml
# Key jobs:
- fmt: Check code formatting
- clippy: Lint checks
- test_rust: Rust tests on Linux/macOS/Windows
- test_python: Python tests on multiple versions
- test_js: JavaScript tests
- testsuite: W3C compliance tests
- coverage: Generate coverage reports
```

**Triggered by:**
- Pull requests to `main` or `next`
- Pushes to `main` or `next`
- Manual workflow dispatch

**Platform Matrix:**
- Linux (Ubuntu latest)
- macOS (latest)
- Windows (latest)

**Rust Versions:**
- Stable (current)
- MSRV (Minimum Supported Rust Version: 1.70.0)
- Nightly (for additional checks)

#### How Tests Run in CI

1. **Code Quality Checks:**
   ```bash
   cargo fmt -- --check
   cargo clippy --all-targets -- -D warnings
   ```

2. **Unit and Integration Tests:**
   ```bash
   cargo test --all
   cargo test --all --release
   ```

3. **W3C Compliance:**
   ```bash
   cargo test -p oxigraph --test testsuite
   ```

4. **Python Tests:**
   ```bash
   maturin develop
   python -m pytest
   ```

5. **JavaScript Tests:**
   ```bash
   wasm-pack test --node
   npm test
   ```

6. **Coverage Report:**
   ```bash
   cargo tarpaulin --all --out Xml
   # Upload to Codecov
   ```

### Running CI Tests Locally

**Simulate CI environment:**

```bash
#!/bin/bash
# ci-local.sh

echo "Running CI checks locally..."

echo "1. Format check..."
cargo fmt --all -- --check

echo "2. Clippy..."
cargo clippy --all-targets -- -D warnings

echo "3. Tests..."
cargo test --all

echo "4. W3C tests..."
cargo test -p oxigraph --test testsuite

echo "5. Python tests..."
cd python && python -m pytest && cd ..

echo "6. JS tests..."
cd js && npm test && cd ..

echo "All CI checks passed!"
```

### CI Configuration Files

**Key files:**
- `.github/workflows/tests.yml` - Main test workflow
- `.github/workflows/artifacts.yml` - Build artifacts (wheels, binaries)
- `.github/workflows/nightly.yml` - Nightly tests with Rust nightly
- `.github/codecov.yml` - Coverage configuration

### Debugging CI Failures

**Common issues:**

1. **Format check failed:**
   ```bash
   # Fix locally
   cargo fmt --all
   git add .
   git commit --amend
   ```

2. **Clippy warnings:**
   ```bash
   # See warnings
   cargo clippy --all-targets

   # Fix and commit
   ```

3. **Test failures:**
   ```bash
   # Run the exact failing test
   cargo test test_name -- --nocapture

   # Check for platform-specific issues
   ```

4. **Timeout:**
   ```bash
   # Some tests may be slow in CI
   # Consider adding #[ignore] for slow tests
   # Run separately with --ignored
   ```

---

## Troubleshooting

### Common Test Issues

#### Issue: Tests hang indefinitely

**Cause:** Deadlock or infinite loop

**Solution:**
```bash
# Run with timeout
cargo test --timeout 30

# Enable logging to find where it hangs
RUST_LOG=debug cargo test -- --nocapture
```

#### Issue: Flaky tests (pass/fail randomly)

**Cause:** Race conditions, timing issues, or shared state

**Solution:**
```rust
// Add explicit synchronization
use std::sync::Mutex;

lazy_static! {
    static ref TEST_MUTEX: Mutex<()> = Mutex::new(());
}

#[test]
fn test_with_mutex() {
    let _guard = TEST_MUTEX.lock().unwrap();
    // Test code - only one at a time
}

// Or use serial_test crate
#[serial]
#[test]
fn test_serial() {
    // Runs serially, not in parallel
}
```

#### Issue: Tests pass locally but fail in CI

**Cause:** Platform differences, missing dependencies, or environment variables

**Solution:**
```bash
# Check CI logs for error details
# Try to reproduce CI environment with Docker

# For Linux CI environment:
docker run -it --rm -v $(pwd):/workspace rust:latest bash
cd /workspace
cargo test
```

#### Issue: Out of memory in tests

**Cause:** Large datasets or memory leaks

**Solution:**
```bash
# Run tests sequentially
cargo test -- --test-threads=1

# Or split into smaller test suites
cargo test -p small_crate
```

#### Issue: W3C tests fail

**Cause:** Submodules not initialized or spec compliance issue

**Solution:**
```bash
# Update submodules
git submodule update --init --recursive

# Check specific failing test
cargo test -p oxigraph --test testsuite specific_test -- --nocapture
```

### Performance Issues

```bash
# Profile tests to find slow tests
cargo test --release -- --nocapture --test-threads=1 | ts -i

# Run only fast tests
cargo test --lib

# Skip slow integration tests
cargo test --lib --bins
```

### Getting Help

- Check [GitHub Issues](https://github.com/oxigraph/oxigraph/issues)
- Ask in [Discussions](https://github.com/oxigraph/oxigraph/discussions)
- Join [Gitter chat](https://gitter.im/oxigraph/community)

---

## Additional Resources

- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [pytest Documentation](https://docs.pytest.org/)
- [Vitest Documentation](https://vitest.dev/)
- [W3C Test Suites](https://www.w3.org/wiki/RdfTestSuites)
