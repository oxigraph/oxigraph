# Determinism and Reproducibility in Oxigraph

## Overview

This document describes deterministic and non-deterministic behaviors in Oxigraph, covering:
- SPARQL query result ordering
- BlankNode generation
- Platform-independent storage
- Cross-platform compatibility

## Deterministic Behaviors (Guaranteed)

### 1. SPARQL SELECT with ORDER BY

SPARQL queries that include an `ORDER BY` clause produce **deterministic, reproducible results**:

```sparql
SELECT ?person ?friend
WHERE { ?person foaf:knows ?friend }
ORDER BY ?person ?friend
```

**Guarantee**: Running this query multiple times will always produce results in the same order.

### 2. Graph Iteration

The `Graph` and `Dataset` types use `BTreeSet` internally, which provides:
- Deterministic iteration order
- Consistent ordering across runs
- Platform-independent behavior

```rust
use oxigraph::model::Graph;

let graph = Graph::new();
// Iteration order is deterministic
for triple in &graph {
    println!("{}", triple);
}
```

### 3. BlankNode String Representation

For a given numerical ID, BlankNode always produces the same string representation:

```rust
use oxigraph::model::BlankNode;

let bn1 = BlankNode::new_from_unique_id(0xdeadbeef);
let bn2 = BlankNode::new_from_unique_id(0xdeadbeef);

assert_eq!(bn1.as_str(), bn2.as_str()); // Always true
assert_eq!(bn1, bn2); // Always true
```

### 4. Cross-Platform Byte Ordering

**Fixed in v0.4**: BlankNode internal storage now uses `to_le_bytes()` instead of `to_ne_bytes()`:

- ✅ Databases created on x86_64 (little-endian) can be read on PowerPC (big-endian)
- ✅ Databases created on ARM (little-endian) can be read on MIPS (big-endian)
- ✅ No platform-specific byte ordering issues

## Non-Deterministic Behaviors (By Design)

### 1. BlankNode::default() Generation

`BlankNode::default()` uses **random number generation** to create unique IDs:

```rust
use oxigraph::model::BlankNode;

let bn1 = BlankNode::default();
let bn2 = BlankNode::default();

assert_ne!(bn1, bn2); // Different random IDs
```

**Rationale**: Blank nodes must be unique across sessions to avoid collisions.

**Use Case**: Appropriate for most applications where blank node uniqueness is required.

**Alternative**: If you need deterministic blank node IDs, use `BlankNode::new()` with a specific identifier:

```rust
let bn = BlankNode::new("my_deterministic_id")?;
```

### 2. SPARQL SELECT without ORDER BY

SPARQL queries **without** an `ORDER BY` clause may produce results in **non-deterministic order**:

```sparql
SELECT ?person ?friend
WHERE { ?person foaf:knows ?friend }
```

**Reason**: Internal query evaluation uses `FxHashMap` and `FxHashSet` for performance. Hash map iteration order is not guaranteed.

**Impact**:
- Results contain the same data
- Order may vary between runs
- Order may vary after database updates or restarts

**Solution**: Always use `ORDER BY` when result ordering matters:

```sparql
SELECT ?person ?friend
WHERE { ?person foaf:knows ?friend }
ORDER BY ?person ?friend
```

## Testing Determinism

### Run Determinism Tests

```bash
# Test SPARQL query determinism
cargo test -p spareval determinism

# Test platform reproducibility
cargo test -p oxrdf platform_reproducibility

# Run determinism demo
cargo run --example determinism_demo
```

### Expected Test Results

| Test | Expected Result |
|------|----------------|
| SELECT with ORDER BY | ✅ DETERMINISTIC (100% consistent) |
| SELECT without ORDER BY | ⚠️ MAY BE NON-DETERMINISTIC |
| BlankNode::default() | ⚠️ NON-DETERMINISTIC (random IDs) |
| BlankNode::new_from_unique_id() | ✅ DETERMINISTIC |
| Graph iteration | ✅ DETERMINISTIC |
| Insert order independence | ✅ DETERMINISTIC (with ORDER BY) |
| Cross-platform byte ordering | ✅ DETERMINISTIC (fixed) |

## Best Practices

### 1. Always Use ORDER BY for Deterministic Results

```rust
use oxigraph::store::Store;
use oxigraph::sparql::Query;

let store = Store::new()?;

// ❌ Non-deterministic result order
let query = Query::parse("SELECT ?s ?p ?o WHERE { ?s ?p ?o }", None)?;

// ✅ Deterministic result order
let query = Query::parse("SELECT ?s ?p ?o WHERE { ?s ?p ?o } ORDER BY ?s ?p ?o", None)?;
```

### 2. Use Named Blank Nodes for Reproducibility

If you need deterministic blank nodes across runs:

```rust
use oxigraph::model::BlankNode;

// ❌ Non-deterministic
let bn = BlankNode::default();

// ✅ Deterministic
let bn = BlankNode::new("my_id")?;
```

### 3. Test Your Queries

If your application requires deterministic behavior, add tests:

```rust
#[test]
fn test_query_determinism() {
    let query = "SELECT ?x WHERE { ?x ?y ?z } ORDER BY ?x";
    let results: Vec<_> = (0..100)
        .map(|_| execute_query(query))
        .collect();

    // All results should be identical
    assert!(results.iter().all(|r| r == &results[0]));
}
```

## Migration Guide

### Fixing Platform-Specific Code

**Before (v0.3 and earlier):**
```rust
// ❌ Platform-specific (to_ne_bytes)
let bytes = id.to_ne_bytes();
```

**After (v0.4+):**
```rust
// ✅ Platform-independent (to_le_bytes)
let bytes = id.to_le_bytes();
```

### Database Migration

Databases created with v0.3 or earlier using `to_ne_bytes()` are **not compatible** with v0.4+.

**Migration steps:**
1. Export data from old database using SPARQL CONSTRUCT or RDF serialization
2. Create new database with v0.4+
3. Import data into new database

```bash
# Export old database
oxigraph dump old-db.rocksdb > export.nq

# Import to new database
oxigraph load new-db.rocksdb < export.nq
```

## Technical Details

### FxHashMap Usage

Oxigraph uses `FxHashMap` (from `rustc_hash`) for performance:

**Locations:**
- `lib/spareval/src/eval.rs` - Query evaluation (DISTINCT, GROUP BY)
- `lib/spareval/src/update.rs` - Blank node mapping
- `lib/oxowl/src/` - OWL reasoning

**Characteristics:**
- Fast hashing (no cryptographic guarantees needed)
- Non-deterministic iteration order
- Hash values may change between process runs

**Mitigation**: Use `ORDER BY` in queries when ordering matters.

### BlankNode Random Generation

**Algorithm**: `rand::random()` generates a random u128 ID.

**Filtering**: IDs starting with digits `0-9` are rejected to ensure RDF/XML compatibility.

**Format**: IDs are formatted as hexadecimal strings (e.g., `a1b2c3d4`).

**Collision Probability**: With 128-bit IDs, collisions are astronomically unlikely (< 2^-64 for millions of nodes).

### Byte Ordering Fix

**Problem**: `to_ne_bytes()` uses native endianness, causing incompatibility between architectures.

**Solution**: `to_le_bytes()` always uses little-endian byte order, ensuring portability.

**Impact**:
- Databases are now portable across all platforms
- Existing databases from v0.3 need migration
- No performance impact (same CPU instructions on little-endian systems)

## FAQ

### Q: Why doesn't SELECT without ORDER BY guarantee ordering?

**A**: SPARQL 1.1 specification states that result order is undefined unless ORDER BY is specified. Using hash-based data structures (FxHashMap) improves performance but doesn't guarantee ordering.

### Q: Will my queries produce different results on different runs?

**A**: No, the **data** is always the same. Only the **order** may vary if you don't use ORDER BY.

### Q: How do I ensure deterministic behavior in tests?

**A**: Use ORDER BY in all queries and use named blank nodes (`BlankNode::new("id")`) instead of `BlankNode::default()`.

### Q: Is the byte ordering fix a breaking change?

**A**: Yes. Databases created with v0.3 or earlier cannot be read by v0.4+. You must export and re-import your data.

### Q: Can I seed the random number generator for deterministic blank nodes?

**A**: Currently, no. If you need deterministic blank nodes, use `BlankNode::new()` with explicit IDs.

## References

- [SPARQL 1.1 Query Language](https://www.w3.org/TR/sparql11-query/) - Section 15.1 (Solution Sequence Modifiers)
- [RDF 1.1 Concepts](https://www.w3.org/TR/rdf11-concepts/) - Section 3.4 (Blank Nodes)
- Test Suite: `lib/spareval/tests/determinism.rs`
- Test Suite: `lib/oxrdf/tests/platform_reproducibility.rs`
- Demo: `lib/oxigraph/examples/determinism_demo.rs`

## Change Log

### v0.4.0 (2024)
- ✅ Fixed platform-specific byte ordering (to_ne_bytes → to_le_bytes)
- ✅ Added determinism test suite
- ✅ Added platform reproducibility tests
- ✅ Added determinism demo example
- ✅ Documented non-deterministic behaviors

### v0.3.x and earlier
- ⚠️ Used to_ne_bytes() (platform-specific)
- ⚠️ Databases not portable across endianness
