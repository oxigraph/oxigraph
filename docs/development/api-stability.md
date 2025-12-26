# API Stability Policy

This document defines Oxigraph's API stability guarantees, deprecation policy, and procedures for handling breaking changes.

## Table of Contents

- [Stability Guarantees](#stability-guarantees)
- [Versioning Policy](#versioning-policy)
- [Deprecation Policy](#deprecation-policy)
- [Breaking Change Process](#breaking-change-process)
- [Migration Support](#migration-support)
- [LTS Versions](#lts-versions)

---

## Stability Guarantees

### Public API Commitment

Oxigraph commits to maintaining **backward compatibility** for all public APIs within the same major version.

**What is a Public API?**

**Rust:**
- All `pub` items in the crate root or public modules
- All `pub` fields of `pub` structs
- All trait methods of `pub` traits
- All items documented in the official documentation

**Excluded from stability guarantees:**
- Items marked with `#[doc(hidden)]`
- Items in modules named `internal`, `private`, or `unstable`
- Items explicitly marked as unstable
- Test-only APIs

**Python:**
- All classes, functions, and methods in the `pyoxigraph` module
- All documented public attributes

**JavaScript:**
- All exported classes, functions, and interfaces
- All documented public properties and methods

### What Changes Are Compatible?

#### Compatible Changes (Allowed)

**Rust:**
- Adding new public items (functions, types, modules)
- Adding new trait methods with default implementations
- Adding new optional parameters with default values
- Making private items public
- Relaxing trait bounds
- Implementing additional traits for existing types
- Adding private fields to non-exhaustive structs
- Deprecating items (with migration path)

**Example:**
```rust
// Version 0.4.0
pub struct Store { }

impl Store {
    pub fn new() -> Result<Self> { }
}

// Version 0.5.0 - Compatible additions
impl Store {
    pub fn new() -> Result<Self> { }

    // New method - compatible
    pub fn with_capacity(capacity: usize) -> Result<Self> { }

    // New optional parameter via builder - compatible
    pub fn builder() -> StoreBuilder { }
}

// New trait implementation - compatible
impl Clone for Store { }
```

**Python/JavaScript:**
- Adding new classes, functions, or methods
- Adding new optional parameters with defaults
- Adding new attributes
- Making restrictive types more permissive (e.g., accepting more input types)

#### Breaking Changes (Require Major Version)

**Rust:**
- Removing or renaming public items
- Changing function signatures
- Removing trait implementations
- Adding required trait bounds
- Changing error types (unless using opaque errors)
- Making public fields private
- Removing or renaming public struct fields
- Adding fields to exhaustive structs
- Changing generic parameter bounds

**Example:**
```rust
// Version 0.5.0
pub fn parse(query: &str) -> Result<Query, ParseError> { }

// Version 1.0.0 - Breaking changes
pub fn parse(query: &str, options: ParseOptions) -> Result<Query, Error> { }
// ❌ Breaking: New required parameter
// ❌ Breaking: Different error type
```

**Python/JavaScript:**
- Removing classes, functions, or methods
- Changing method signatures
- Removing or renaming attributes
- Changing exception/error types
- Making permissive types more restrictive

---

## Versioning Policy

### Semantic Versioning

Oxigraph follows [Semantic Versioning 2.0.0](https://semver.org/):

```
MAJOR.MINOR.PATCH
```

- **MAJOR:** Breaking changes (incompatible API changes)
- **MINOR:** New features (backward-compatible)
- **PATCH:** Bug fixes (backward-compatible)

### Pre-1.0 Versioning

**Before version 1.0.0:**

- Minor version increments (0.x.0) **may include breaking changes**
- Patch version increments (0.x.y) are backward-compatible
- Breaking changes are clearly documented in CHANGELOG.md

**Example:**
- `0.4.0 → 0.4.1`: Bug fixes only, compatible
- `0.4.1 → 0.5.0`: May include breaking changes
- `0.5.0 → 1.0.0`: First stable release, commits to full semver

### Post-1.0 Versioning

**After version 1.0.0:**

Strict semantic versioning applies:

- `1.0.0 → 1.0.1`: Patch release (bug fixes only)
- `1.0.1 → 1.1.0`: Minor release (new features, backward-compatible)
- `1.1.0 → 2.0.0`: Major release (breaking changes allowed)

### Version Synchronization

**All Oxigraph components share the same version:**

- `oxigraph` (Rust crate)
- `pyoxigraph` (Python package)
- `oxigraph` (npm package)
- `oxigraph-cli` (CLI binary)

This ensures consistency and simplifies dependency management.

---

## Deprecation Policy

### Deprecation Process

When an API needs to be replaced or removed:

#### 1. Mark as Deprecated

**Rust:**
```rust
/// Loads data into the store.
///
/// # Deprecated
///
/// Deprecated since 0.5.0. Use [`Store::bulk_load`] instead.
/// This method will be removed in version 1.0.0.
///
/// # Migration
///
/// Replace:
/// ```ignore
/// store.load(data)?;
/// ```
///
/// With:
/// ```
/// # use oxigraph::store::Store;
/// # let store = Store::new()?;
/// # let data = &[];
/// store.bulk_load().load(data)?.finish()?;
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
#[deprecated(since = "0.5.0", note = "Use `bulk_load` instead")]
pub fn load(&self, data: &[u8]) -> Result<()> {
    // Implementation still works
}
```

**Python:**
```python
import warnings

def load(self, data: bytes) -> None:
    """Load data into the store.

    .. deprecated:: 0.5.0
        Use :meth:`bulk_load` instead. This method will be removed in 1.0.0.

    Migration:
        Replace ``store.load(data)`` with::

            loader = store.bulk_load()
            loader.load(data)
            loader.finish()
    """
    warnings.warn(
        "load() is deprecated, use bulk_load() instead",
        DeprecationWarning,
        stacklevel=2
    )
    # Implementation
```

**JavaScript:**
```typescript
/**
 * Loads data into the store.
 *
 * @deprecated Since 0.5.0. Use {@link bulkLoad} instead.
 * This method will be removed in 1.0.0.
 *
 * @example Migration
 * ```javascript
 * // Old
 * store.load(data);
 *
 * // New
 * const loader = store.bulkLoad();
 * loader.load(data);
 * loader.finish();
 * ```
 */
export function load(data: Uint8Array): void {
    console.warn('load() is deprecated, use bulkLoad() instead');
    // Implementation
}
```

#### 2. Update Documentation

- Add deprecation notice to API docs
- Add entry to CHANGELOG.md
- Add migration guide to docs
- Update examples to use new API

#### 3. Deprecation Timeline

**Minimum deprecation period:**

| Current Version | Deprecation Version | Removal Version | Minimum Time |
|----------------|---------------------|-----------------|--------------|
| 0.4.x | 0.5.0 | 0.7.0 or 1.0.0 | 2 minor releases |
| 1.0.x | 1.1.0 | 2.0.0 | 6 months |
| 2.0.x | 2.1.0 | 3.0.0 | 6 months |

**Example timeline:**
- **v0.5.0 (Jan 2024):** Feature deprecated, warnings added
- **v0.6.0 (Apr 2024):** Feature still available with warnings
- **v0.7.0 or 1.0.0 (Jul 2024+):** Feature removed

#### 4. Announce Deprecation

- Mention in release notes
- Blog post for major deprecations
- Update migration guide
- Notify users via GitHub Discussions

### Deprecation Levels

**Soft Deprecation:**
- Documentation marks as deprecated
- No runtime warnings
- For low-impact changes

**Hard Deprecation:**
- Runtime warnings (Rust: compile-time, Python/JS: runtime)
- For high-impact changes
- Requires migration before removal

**Immediate Removal:**
- Only for security issues or critical bugs
- Treated as breaking change
- Requires major version bump (or minor if pre-1.0)

---

## Breaking Change Process

### When Breaking Changes Are Necessary

**Acceptable reasons:**
- Security vulnerabilities
- Fundamental design flaws
- Performance critical improvements
- Standards compliance (W3C specs)
- Reducing maintenance burden

**Unacceptable reasons:**
- Minor API improvements
- Personal preference
- Inconsistent naming (unless widespread)

### Planning Breaking Changes

#### 1. Proposal Phase

1. **Create RFC (Request for Comments):**
   - Open GitHub Discussion
   - Describe the problem
   - Propose solution
   - Analyze impact
   - Suggest migration path

2. **Gather Feedback:**
   - Minimum 2 weeks for discussion
   - Address community concerns
   - Revise proposal if needed

3. **Decision:**
   - Maintainers approve or reject
   - Document decision

#### 2. Implementation Phase

1. **Create tracking issue**
2. **Implement new API** (if replacement)
3. **Deprecate old API** (if gradual migration)
4. **Write migration guide**
5. **Update all examples**

#### 3. Migration Phase

**For pre-1.0 versions:**
- Minimum 1 minor version with deprecation warnings
- Document in CHANGELOG.md

**For post-1.0 versions:**
- Minimum 2 minor versions OR 6 months (whichever is longer)
- Provide automated migration tool if possible

#### 4. Removal Phase

1. **Remove deprecated API**
2. **Bump major version**
3. **Update documentation**
4. **Publish migration guide**
5. **Announce in release notes**

### Breaking Change Checklist

Before releasing breaking changes:

- [ ] Migration guide written and tested
- [ ] All examples updated
- [ ] CHANGELOG.md updated with clear breaking changes section
- [ ] Automated migration tools provided (if applicable)
- [ ] Community notified in advance
- [ ] Documentation updated
- [ ] Old API deprecated for minimum period
- [ ] Tests cover both old and new behavior (during transition)

### Example: Breaking Change Migration

**Scenario:** Changing `Store::new()` to require a configuration parameter

**v0.5.0 - Current API:**
```rust
pub fn new() -> Result<Store> { }
```

**v0.6.0 - Introduce New API:**
```rust
// Old API - deprecated but still works
#[deprecated(since = "0.6.0", note = "Use `builder()` instead")]
pub fn new() -> Result<Store> {
    Self::builder().build()
}

// New API
pub fn builder() -> StoreBuilder {
    StoreBuilder::default()
}
```

**v0.7.0 or 1.0.0 - Remove Old API:**
```rust
// Old API removed
// Only new API remains
pub fn builder() -> StoreBuilder { }
```

---

## Migration Support

### Migration Guides

For each major/minor version with breaking changes, provide:

#### 1. Version-Specific Migration Guide

**Location:** `docs/migrations/v0.5-to-v0.6.md`

```markdown
# Migration Guide: v0.5 to v0.6

## Overview

Version 0.6 introduces new APIs for better performance and flexibility.
This guide helps you migrate your code from v0.5 to v0.6.

## Breaking Changes

### 1. Store Initialization

**Changed:** Store creation API

**Before (v0.5):**
```rust
let store = Store::new()?;
```

**After (v0.6):**
```rust
let store = Store::builder().build()?;

// Or with configuration:
let store = Store::builder()
    .with_capacity(1000)
    .with_cache_size(100_000)
    .build()?;
```

**Reason:** New API provides better control over store configuration.

**Timeline:** Old API deprecated in v0.6.0, will be removed in v1.0.0.

### 2. Query Results

**Changed:** Query results iterator type

**Before (v0.5):**
```rust
let results: Vec<QueryResult> = store.query(query)?;
```

**After (v0.6):**
```rust
let results = store.query(query)?;
// Results is now an iterator:
for result in results {
    // Process result
}

// Or collect:
let results: Vec<_> = store.query(query)?.collect();
```

**Reason:** Iterator provides better memory efficiency for large result sets.

## Deprecations

### `Store::load()`

**Status:** Deprecated, will be removed in v1.0.0

**Migration:**
```rust
// Old
store.load(data)?;

// New
store.bulk_load().load(data)?.finish()?;
```

## Automated Migration

We provide a migration tool:

```bash
cargo install oxigraph-migrate
oxigraph-migrate --from 0.5 --to 0.6 src/
```

## Getting Help

- [GitHub Discussions](https://github.com/oxigraph/oxigraph/discussions)
- [Migration FAQ](migration-faq.md)
```

#### 2. Automated Migration Tools

**Rust (using cargo-edit):**
```bash
# Update Cargo.toml
cargo upgrade oxigraph@0.6.0

# Run migration script
./scripts/migrate-0.5-to-0.6.sh
```

**Python:**
```python
# scripts/migrate_0_5_to_0_6.py
import ast
import sys

# AST-based code migration
# Transforms old API calls to new ones
```

**sed/awk scripts for simple replacements:**
```bash
#!/bin/bash
# migrate.sh

# Simple find-replace migrations
find src -name "*.rs" -exec sed -i 's/Store::new()/Store::builder().build()/g' {} \;
```

### Backward Compatibility Shims

When possible, provide compatibility layers:

```rust
// oxigraph-compat crate
pub mod v0_5 {
    use oxigraph::store::{Store as NewStore, StoreBuilder};

    /// Compatibility wrapper for v0.5 Store API
    pub struct Store {
        inner: NewStore,
    }

    impl Store {
        pub fn new() -> Result<Self> {
            Ok(Self {
                inner: StoreBuilder::default().build()?
            })
        }

        // Wrap other methods...
    }
}
```

Usage:
```rust
// Minimal migration - just change import
use oxigraph_compat::v0_5::Store;

// Rest of code unchanged
let store = Store::new()?;
```

---

## LTS Versions

### Long-Term Support Policy

**Currently:** No official LTS versions (pre-1.0)

**After 1.0.0:**

Oxigraph will provide **Long-Term Support (LTS)** for select versions:

- **LTS Duration:** 18 months
- **Support Includes:**
  - Critical bug fixes
  - Security patches
  - Documentation updates
- **No New Features:** LTS versions are maintenance-only

### LTS Version Schedule

**Planned (post-1.0):**

| Version | Release Date | LTS Until | Status |
|---------|-------------|-----------|--------|
| 1.0.x | TBD | TBD + 18mo | Future |
| 2.0.x | TBD | TBD + 18mo | Future |

**LTS releases:**
- Every major version becomes LTS
- Select minor versions may become LTS

### Version Support Matrix

| Version | Status | Support Type | End of Support |
|---------|--------|--------------|----------------|
| 0.3.x | End of Life | None | 2023-06-01 |
| 0.4.x | Maintenance | Bug fixes only | 6 months after 0.5.0 |
| 0.5.x | Current | Full support | Until next minor |
| 1.0.x | Future LTS | TBD | TBD |

### Choosing a Version

**Use latest stable (0.5.x) if:**
- You want newest features
- You can update regularly
- You're starting a new project

**Use LTS (future 1.0.x) if:**
- You need long-term stability
- Updates are costly
- Production deployment with infrequent updates

---

## API Stability by Component

### Rust Crates

| Crate | Stability | Notes |
|-------|-----------|-------|
| `oxigraph` | Stable API | Public API follows semver |
| `oxrdf` | Stable | Core RDF types, very stable |
| `spargebra` | Stable | SPARQL algebra, follows spec |
| `oxrdfio` | Stable | I/O interfaces |
| `spareval` | Semi-stable | May change for optimizations |
| Internal crates | Unstable | No stability guarantees |

### Python Package

- **Package:** `pyoxigraph`
- **Stability:** Follows same policy as Rust
- **Python Version Support:** Python 3.8+ (reviewed yearly)

### JavaScript Package

- **Package:** `oxigraph` (npm)
- **Stability:** Follows same policy as Rust
- **Node.js Version Support:** Node.js 18+ (LTS versions)

### CLI Binary

- **Binary:** `oxigraph-cli`
- **Stability:** Command-line interface follows semver
- **Configuration:** Config file format is versioned

---

## Handling External Dependencies

### Dependency Update Policy

**Rust dependencies:**
- Updated regularly for security
- Major version bumps may require Oxigraph major version bump
- Breaking changes in dependencies trigger evaluation

**Python dependencies:**
- Minimal dependencies in runtime
- Development dependencies updated regularly

**JavaScript dependencies:**
- Runtime dependencies minimized
- TypeScript types kept up-to-date

### MSRV (Minimum Supported Rust Version)

**Current MSRV:** 1.70.0

**MSRV Update Policy:**
- MSRV bumps are considered breaking changes (pre-1.0)
- After 1.0: MSRV bumps allowed in minor versions
- MSRV documented in `Cargo.toml`
- Support latest stable and N-2 versions

---

## Exception Handling

### Emergency Releases

For **critical security issues:**

1. Immediate patch release
2. May skip normal deprecation process
3. Clearly documented as emergency fix
4. Migration support provided if breaking

**Example:**
- v0.5.0 has security vulnerability
- v0.5.1 released immediately with breaking fix
- v0.5.1 clearly marked as security patch
- Migration guide provided

### Specification Compliance

When W3C specifications change:

- Oxigraph will follow the updated specification
- May introduce breaking changes if necessary
- Controlled by feature flags when possible
- Migration period provided

**Example:**
```rust
// RDF 1.1 features (default)
#[cfg(not(feature = "rdf-12"))]
pub fn parse(input: &str) -> Result<Graph> {
    // RDF 1.1 behavior
}

// RDF 1.2 features (opt-in)
#[cfg(feature = "rdf-12")]
pub fn parse(input: &str) -> Result<Graph> {
    // RDF 1.2 behavior
}
```

---

## Feedback and Questions

### Reporting Stability Issues

If you encounter unexpected breaking changes:

1. **Check CHANGELOG.md** for documentation
2. **Search GitHub Issues** for existing reports
3. **Open new issue** if not documented:
   - Describe the breaking change
   - Show old vs new behavior
   - Explain impact
   - Suggest migration path

### Requesting Stability Exceptions

To request expedited breaking changes:

1. Open GitHub Discussion
2. Explain why change is critical
3. Show that current API is fundamentally broken
4. Propose migration plan
5. Maintainers will evaluate

---

## Additional Resources

- [Semantic Versioning](https://semver.org/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Cargo SemVer Compatibility](https://doc.rust-lang.org/cargo/reference/semver.html)
- [Release Process](release-process.md)
