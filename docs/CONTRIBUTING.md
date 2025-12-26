# Contributing to Oxigraph

Thank you for your interest in contributing to Oxigraph! This guide will help you get started.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Ways to Contribute](#ways-to-contribute)
- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Development Workflow](#development-workflow)
- [Testing Guidelines](#testing-guidelines)
- [Code Style](#code-style)
- [Submitting Changes](#submitting-changes)
- [Documentation](#documentation)
- [Community](#community)

---

## Code of Conduct

We are committed to providing a welcoming and inclusive environment. Please:

- **Be respectful** of differing viewpoints and experiences
- **Be collaborative** and constructive in discussions
- **Focus on what's best** for the community
- **Show empathy** towards other community members

Unacceptable behavior includes harassment, trolling, insults, or other unprofessional conduct.

---

## Ways to Contribute

### Report Bugs

Found a bug? [Open an issue](https://github.com/oxigraph/oxigraph/issues/new) with:

- **Clear title**: Describe the issue briefly
- **Environment**: OS, Rust/Python/Node.js version, Oxigraph version
- **Steps to reproduce**: Minimal example to trigger the bug
- **Expected behavior**: What should happen
- **Actual behavior**: What actually happens
- **Additional context**: Error messages, logs, etc.

**Example:**
```markdown
## Bug: Query returns incorrect results for OPTIONAL pattern

**Environment:**
- OS: Ubuntu 22.04
- Oxigraph: 0.4.0
- Rust: 1.75

**Steps to reproduce:**
1. Load attached data.ttl
2. Run query: `SELECT * WHERE { ?s :p1 ?o . OPTIONAL { ?s :p2 ?x } }`

**Expected:** Should return 5 results
**Actual:** Returns 3 results

**Error message:**
None, but results are missing data.
```

### Suggest Features

Have an idea? [Start a discussion](https://github.com/oxigraph/oxigraph/discussions/new?category=ideas) with:

- **Use case**: What problem does it solve?
- **Proposed solution**: How might it work?
- **Alternatives**: Other approaches considered?
- **Impact**: Who benefits? Breaking changes?

### Improve Documentation

Documentation is always welcome! You can:

- Fix typos or unclear explanations
- Add examples
- Improve API documentation
- Write tutorials
- Translate documentation

### Contribute Code

See sections below for development workflow.

### Help Others

- Answer questions in [Discussions](https://github.com/oxigraph/oxigraph/discussions)
- Help in [Gitter chat](https://gitter.im/oxigraph/community)
- Review pull requests
- Write blog posts or tutorials

---

## Development Setup

### Prerequisites

1. **Rust toolchain** (latest stable):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   ```

2. **Clang** (for RocksDB):
   ```bash
   # Ubuntu/Debian
   sudo apt-get install clang

   # macOS
   xcode-select --install

   # Windows
   # Install Visual Studio Build Tools + LLVM
   ```

3. **Git** with submodules support

4. **Optional tools**:
   - Python 3.8+ and maturin (for Python bindings)
   - Node.js 18+ and wasm-pack (for JavaScript bindings)
   - Docker (for testing containers)

### Clone the Repository

```bash
git clone --recursive https://github.com/oxigraph/oxigraph.git
cd oxigraph
```

If you forgot `--recursive`:
```bash
git submodule update --init --recursive
```

### Verify Your Setup

```bash
# Check Rust is working
cargo --version
rustc --version

# Build everything
cargo build --all

# Run tests
cargo test --all

# Success!
```

---

## Project Structure

```
oxigraph/
├── lib/                      # Rust crates
│   ├── oxigraph/            # Main database library
│   ├── oxrdf/               # RDF data model
│   ├── oxrdfio/             # RDF I/O (parsing/serialization)
│   ├── oxttl/               # Turtle/N-Triples/N-Quads/TriG
│   ├── oxrdfxml/            # RDF/XML parser
│   ├── oxjsonld/            # JSON-LD parser
│   ├── spargebra/           # SPARQL algebra
│   ├── spareval/            # SPARQL evaluation
│   ├── sparopt/             # SPARQL optimizer
│   ├── sparesults/          # SPARQL results formats
│   ├── sparshacl/           # SHACL validation
│   ├── spargeo/             # GeoSPARQL
│   ├── sparql-smith/        # SPARQL fuzzing
│   └── oxsdatatypes/        # XSD datatypes
├── cli/                      # CLI server (oxigraph-cli)
├── python/                   # Python bindings (pyoxigraph)
├── js/                       # JavaScript/WASM bindings
├── testsuite/                # W3C test suites
├── bench/                    # Benchmarks
├── fuzz/                     # Fuzzing targets
└── docs/                     # Documentation

Key files:
├── Cargo.toml               # Workspace configuration
├── CLAUDE.md                # AI development guide
├── README.md                # Main README
└── .github/
    └── workflows/           # CI/CD pipelines
```

### Component Responsibilities

| Component | Purpose | Language |
|-----------|---------|----------|
| `oxrdf` | RDF data model (NamedNode, Literal, etc.) | Rust |
| `oxrdfio` | Unified RDF parsing/serialization | Rust |
| `spargebra` | SPARQL query parsing to algebra | Rust |
| `spareval` | SPARQL query evaluation engine | Rust |
| `sparopt` | SPARQL query optimization | Rust |
| `oxigraph` | Main database with RocksDB storage | Rust |
| `cli` | Command-line server binary | Rust |
| `python` | PyO3 bindings for Python | Rust + Python |
| `js` | WASM bindings for JavaScript | Rust + JS |

---

## Development Workflow

### 1. Pick an Issue

- Browse [open issues](https://github.com/oxigraph/oxigraph/issues)
- Look for `good first issue` or `help wanted` labels
- Comment on the issue to claim it

### 2. Create a Branch

```bash
# Update main
git checkout main
git pull origin main

# Create feature branch
git checkout -b fix/issue-123-short-description
# or
git checkout -b feature/new-sparql-function
```

**Branch naming:**
- `fix/issue-XXX-description` - Bug fixes
- `feature/description` - New features
- `docs/description` - Documentation
- `refactor/description` - Code refactoring
- `test/description` - Test additions

### 3. Make Changes

**Keep changes focused:**
- One issue/feature per pull request
- Related changes can be grouped
- Don't mix refactoring with features

**Write good commit messages:**
```bash
# Good
git commit -m "Fix OPTIONAL query bug when no bindings exist

Resolves #123 by handling empty binding sets correctly in the
OPTIONAL operator evaluation."

# Bad
git commit -m "fix bug"
```

### 4. Test Your Changes

```bash
# Run all tests
cargo test --all

# Run specific crate tests
cargo test -p oxigraph

# Run with logging
RUST_LOG=debug cargo test

# Format code
cargo fmt --all

# Lint code
cargo clippy --all
```

### 5. Update Documentation

- Add/update doc comments for public APIs
- Update README if needed
- Add examples for new features
- Update CHANGELOG (for maintainers)

### 6. Submit Pull Request

See [Submitting Changes](#submitting-changes) below.

---

## Testing Guidelines

### Types of Tests

#### 1. Unit Tests

Place in the same file as the code:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_named_node_creation() {
        let node = NamedNode::new("http://example.com").unwrap();
        assert_eq!(node.as_str(), "http://example.com");
    }

    #[test]
    fn test_invalid_iri() {
        assert!(NamedNode::new("not a valid iri").is_err());
    }
}
```

#### 2. Integration Tests

Place in `tests/` directory:

```rust
// tests/store_tests.rs
use oxigraph::store::Store;

#[test]
fn test_store_insert_and_query() {
    let store = Store::new().unwrap();
    // Test full workflow
}
```

#### 3. W3C Test Suites

Oxigraph runs official W3C test suites:

```bash
cargo test -p oxigraph --test testsuite
```

Located in `testsuite/` directory.

#### 4. Benchmarks

Add benchmarks in `bench/`:

```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_query(c: &mut Criterion) {
    let store = setup_store();
    c.bench_function("simple query", |b| {
        b.iter(|| {
            store.query(black_box("SELECT * WHERE { ?s ?p ?o }"))
        });
    });
}

criterion_group!(benches, benchmark_query);
criterion_main!(benches);
```

Run benchmarks:
```bash
cargo bench
```

### Testing Best Practices

**Do:**
- Test both success and failure cases
- Test edge cases (empty input, very large input, etc.)
- Use descriptive test names
- Keep tests independent (no shared state)
- Use `assert_eq!` with expected value first

**Don't:**
- Depend on external services in tests
- Use random data (use fixed seeds if needed)
- Write tests that might fail randomly
- Leave commented-out test code

### Python Tests

```bash
cd python
python -m pytest tests/
```

Write tests in `python/tests/`:

```python
import unittest
from pyoxigraph import Store, NamedNode

class TestStore(unittest.TestCase):
    def test_insert_and_query(self):
        store = Store()
        # Test implementation
```

### JavaScript Tests

```bash
cd js
npm test
```

Write tests in `js/test/`:

```javascript
import { describe, it, expect } from 'vitest';
import oxigraph from '../pkg/node.js';

describe('Store', () => {
    it('should insert and query data', () => {
        const store = new oxigraph.Store();
        // Test implementation
    });
});
```

---

## Code Style

### Rust Code Style

**Follow standard Rust conventions:**

- Use `rustfmt` (enforced by CI):
  ```bash
  cargo fmt --all
  ```

- Use `clippy` (enforced by CI):
  ```bash
  cargo clippy --all -- -D warnings
  ```

**Specific guidelines:**

1. **Naming:**
   ```rust
   // Types: PascalCase
   struct NamedNode { }

   // Functions/variables: snake_case
   fn parse_query() { }
   let user_name = "Alice";

   // Constants: SCREAMING_SNAKE_CASE
   const MAX_BUFFER_SIZE: usize = 1024;
   ```

2. **Documentation:**
   ```rust
   /// Parses a SPARQL query string.
   ///
   /// # Arguments
   ///
   /// * `query` - The SPARQL query string to parse
   ///
   /// # Errors
   ///
   /// Returns an error if the query is not valid SPARQL.
   ///
   /// # Examples
   ///
   /// ```
   /// use oxigraph::sparql::Query;
   ///
   /// let query = Query::parse("SELECT * WHERE { ?s ?p ?o }")?;
   /// ```
   pub fn parse(query: &str) -> Result<Query, ParseError> {
       // Implementation
   }
   ```

3. **Error handling:**
   ```rust
   // Use Result for fallible operations
   pub fn parse(input: &str) -> Result<T, ParseError>

   // Use thiserror for error types
   #[derive(Debug, thiserror::Error)]
   pub enum ParseError {
       #[error("Invalid IRI: {0}")]
       InvalidIri(String),
   }
   ```

4. **Public APIs:**
   ```rust
   // Use Into<T> for flexibility
   pub fn add(&mut self, quad: impl Into<Quad>)

   // Use impl Iterator for lazy evaluation
   pub fn quads(&self) -> impl Iterator<Item = &Quad>

   // Provide builder patterns where appropriate
   pub fn query_builder(&self) -> QueryBuilder
   ```

### Python Code Style

Follow PEP 8:

```python
# Use snake_case
def parse_query(query_string):
    pass

# Type hints
from typing import Iterator

def get_triples() -> Iterator[Triple]:
    pass

# Docstrings
def load(self, data: bytes, mime_type: str) -> None:
    """Load RDF data into the store.

    Args:
        data: The RDF data as bytes
        mime_type: MIME type of the data (e.g., "text/turtle")

    Raises:
        ValueError: If the data is invalid
    """
    pass
```

### JavaScript Code Style

Use Biome (configured in `js/`):

```bash
cd js
npm run fmt  # Auto-format
```

**Conventions:**

```javascript
// camelCase for variables and functions
const userName = "Alice";
function parseQuery() { }

// PascalCase for classes
class Store { }

// Use const/let, not var
const immutable = 42;
let mutable = 0;

// Prefer arrow functions
const filter = items => items.filter(x => x > 0);

// Document public APIs
/**
 * Executes a SPARQL query.
 * @param {string} query - The SPARQL query string
 * @returns {Array} Query results
 */
query(query) { }
```

### Git Commit Style

```bash
# Format:
# <type>: <subject>
#
# <body>
#
# <footer>

# Example:
git commit -m "fix: Handle empty OPTIONAL results correctly

The OPTIONAL operator was failing when the optional pattern
produced no bindings. This adds proper handling for that case.

Fixes #123"
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation
- `test`: Test additions/fixes
- `refactor`: Code refactoring
- `perf`: Performance improvements
- `chore`: Maintenance tasks

---

## Submitting Changes

### Before Submitting

**Checklist:**

- [ ] Tests pass: `cargo test --all`
- [ ] Code is formatted: `cargo fmt --all`
- [ ] No clippy warnings: `cargo clippy --all`
- [ ] Documentation is updated
- [ ] Commit messages are clear
- [ ] Branch is up-to-date with main

### Creating a Pull Request

1. **Push your branch:**
   ```bash
   git push origin fix/issue-123-description
   ```

2. **Open PR on GitHub:**
   - Go to https://github.com/oxigraph/oxigraph
   - Click "New Pull Request"
   - Select your branch
   - Fill in the template

3. **PR Template:**
   ```markdown
   ## Description
   Brief description of changes

   ## Motivation
   Why is this change needed?

   ## Changes
   - List of changes made
   - Another change

   ## Testing
   How was this tested?

   ## Related Issues
   Fixes #123
   ```

### Example PR Description

```markdown
## Description
Add support for SPARQL 1.2 ADJUST function

## Motivation
SPARQL 1.2 introduces the ADJUST function for datetime manipulation.
This is needed for full SPARQL 1.2 compliance.

## Changes
- Add `adjust` function to `spareval/src/functions.rs`
- Add tests for various datetime adjustments
- Update documentation with examples

## Testing
- Added unit tests for normal cases
- Added tests for edge cases (leap years, DST transitions)
- Verified against SPARQL 1.2 spec examples
- All W3C test suites still pass

## Related Issues
Fixes #456
Part of #123 (SPARQL 1.2 support)
```

### Review Process

1. **Automated checks run** (CI/CD):
   - Compilation
   - Tests
   - Formatting
   - Clippy lints

2. **Maintainers review** your code:
   - May request changes
   - May ask questions
   - May suggest improvements

3. **Address feedback:**
   ```bash
   # Make changes
   git add .
   git commit -m "Address review feedback"
   git push
   ```

4. **Approval and merge:**
   - Once approved, maintainers will merge
   - Your contribution is now part of Oxigraph!

### After Merge

- Delete your branch (optional)
- Pull latest main
- Check the [release notes](https://github.com/oxigraph/oxigraph/releases)
- Celebrate! 🎉

---

## Documentation

### Code Documentation

**Rust:**
```rust
/// Parses a Turtle document.
///
/// This function parses RDF data in the Turtle format.
///
/// # Arguments
///
/// * `input` - The Turtle document as a string
///
/// # Errors
///
/// Returns [`ParseError`] if the input is not valid Turtle.
///
/// # Examples
///
/// ```
/// use oxigraph::io::parse_turtle;
///
/// let data = "<http://example.com/s> <http://example.com/p> \"value\" .";
/// let triples = parse_turtle(data)?;
/// ```
pub fn parse_turtle(input: &str) -> Result<Vec<Triple>, ParseError> {
    // Implementation
}
```

**Python:**
```python
def load(self, data: bytes, mime_type: str) -> None:
    """Load RDF data into the store.

    Args:
        data: The RDF data as bytes.
        mime_type: The MIME type (e.g., "text/turtle").

    Raises:
        ValueError: If the data cannot be parsed.
        IOError: If a system error occurs.

    Example:
        >>> store = Store()
        >>> store.load(b'<s> <p> "o" .', format=RdfFormat.TURTLE)
    """
    pass
```

**JavaScript (TypeScript):**
```typescript
/**
 * Loads RDF data into the store.
 *
 * @param data - The RDF data as a string
 * @param options - Loading options
 * @param options.format - RDF format (e.g., "text/turtle")
 * @throws {Error} If the data cannot be parsed
 * @example
 * ```javascript
 * store.load('<s> <p> "o" .', { format: "text/turtle" });
 * ```
 */
load(data: string, options: { format: string }): void;
```

### README and Guides

- Keep READMEs up-to-date with code changes
- Include practical examples
- Link to detailed documentation
- Keep language clear and simple

---

## Community

### Getting Help

- [GitHub Discussions](https://github.com/oxigraph/oxigraph/discussions) - Questions and ideas
- [Gitter Chat](https://gitter.im/oxigraph/community) - Real-time chat
- [Stack Overflow](https://stackoverflow.com/questions/tagged/oxigraph) - Tag: `oxigraph`

### Communication Guidelines

**Be Clear:**
- Provide context
- Include examples
- Show what you've tried

**Be Patient:**
- Maintainers are volunteers
- Reviews take time
- Not all features can be accepted

**Be Collaborative:**
- Discuss before large changes
- Accept feedback gracefully
- Help others when you can

### Maintainers

Current maintainers:
- [@Tpt](https://github.com/Tpt) - Lead maintainer

### Recognition

Contributors are recognized in:
- [GitHub Contributors](https://github.com/oxigraph/oxigraph/graphs/contributors)
- Release notes for significant contributions
- Our gratitude!

---

## Advanced Topics

### Adding a New RDF Format

1. **Create parser crate** in `lib/ox<format>/`:
   ```rust
   // lib/oxnewformat/src/lib.rs
   pub struct NewFormatParser { }

   impl NewFormatParser {
       pub fn parse(&self, input: &str) -> Result<Vec<Triple>> {
           // Implementation
       }
   }
   ```

2. **Register in `oxrdfio`:**
   ```rust
   // lib/oxrdfio/src/format.rs
   pub enum RdfFormat {
       // ...
       NewFormat,
   }
   ```

3. **Add tests** including W3C test suite if available

4. **Update documentation**

5. **Submit PR**

### Adding a SPARQL Function

1. **Implement in `spareval`:**
   ```rust
   // lib/spareval/src/functions.rs
   pub fn evaluate_new_function(args: &[Term]) -> Result<Term> {
       // Implementation
   }
   ```

2. **Register function:**
   ```rust
   // In function registry
   register_function("http://example.com/newFunc", evaluate_new_function);
   ```

3. **Add tests**

4. **Update docs**

### Performance Optimization

**Before optimizing:**
1. Profile to find bottlenecks
2. Write benchmarks
3. Optimize
4. Measure improvement

**Profiling:**
```bash
# Linux perf
cargo build --release
perf record ./target/release/benchmark
perf report

# Flamegraph
cargo install flamegraph
cargo flamegraph --bench my_benchmark
```

### Release Process

(For maintainers)

1. Update version in `Cargo.toml`
2. Update CHANGELOG.md
3. Run all tests
4. Create git tag
5. Push tag (CI creates release)
6. Publish crates
7. Publish Python wheels
8. Publish npm package

---

## License

By contributing, you agree that your contributions will be licensed under the same terms as Oxigraph (dual Apache 2.0 / MIT).

Include this in your commits:

```
Unless you explicitly state otherwise, any contribution intentionally
submitted for inclusion in Oxigraph by you, as defined in the Apache-2.0
license, shall be dual licensed as above, without any additional terms
or conditions.
```

---

## Questions?

- Read the [FAQ](faq.md)
- Ask in [Discussions](https://github.com/oxigraph/oxigraph/discussions)
- Join [Gitter](https://gitter.im/oxigraph/community)

**Thank you for contributing to Oxigraph!** 🎉
