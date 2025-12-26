# ΔGate Protocol Support in oxrdf

## Overview

The `oxrdf` crate now provides comprehensive support for the ΔGate protocol, enabling efficient computation and application of deltas (Δ) for RDF datasets and graphs.

## What is ΔGate?

ΔGate is a protocol for computing differences between RDF graph states and applying those differences efficiently. It enables:

- **Delta Computation**: Calculate what changed between two graph states
- **Delta Application**: Apply changes to transform one state to another
- **Deterministic Operations**: Consistent results for hashing and verification
- **Set Operations**: Union, intersection, and difference operations on RDF data

## Core Capabilities

### 1. Set Operations

Both `Dataset` and `Graph` now support standard set operations:

```rust
use oxrdf::*;

let mut ds1 = Dataset::new();
let mut ds2 = Dataset::new();

// Union (⊔): All quads from both datasets
let union = ds1.union(&ds2);

// Difference (\): Quads in ds1 but not in ds2
let diff = ds1.difference(&ds2);

// Intersection (∩): Quads in both datasets
let intersection = ds1.intersection(&ds2);

// Symmetric Difference (Δ): Quads in either but not both
let sym_diff = ds1.symmetric_difference(&ds2);
```

### 2. Delta Computation

Compute the changes needed to transform one state to another:

```rust
use oxrdf::*;

let before = /* ... initial dataset ... */;
let after = /* ... modified dataset ... */;

// Compute delta: (Δ⁺, Δ⁻)
let (additions, removals) = before.diff(&after);

// additions (Δ⁺): quads to add
// removals (Δ⁻): quads to remove
```

### 3. Delta Application

Apply computed deltas to transform a dataset:

```rust
use oxrdf::*;

let mut dataset = /* ... initial state ... */;
let additions = /* ... quads to add ... */;
let removals = /* ... quads to remove ... */;

// Apply delta
dataset.apply_diff(&additions, &removals);
// dataset is now in the target state
```

### 4. Deterministic Iteration

All operations use `BTreeSet` internally, ensuring:
- **Consistent ordering**: Same iteration order every time
- **Reproducible hashing**: Deterministic results for verification
- **Canonical representation**: Stable serialization

```rust
use oxrdf::*;

let ds = /* ... dataset ... */;

// Iteration is always in the same order
let quads1: Vec<_> = ds.iter().collect();
let quads2: Vec<_> = ds.iter().collect();
assert_eq!(quads1, quads2);
```

## Implementation Details

### Internal Data Structure

Datasets use 6 `BTreeSet` indexes for efficient query patterns:
- **GSPO**: Graph, Subject, Predicate, Object
- **GPOS**: Graph, Predicate, Object, Subject
- **GOSP**: Graph, Object, Subject, Predicate
- **SPOG**: Subject, Predicate, Object, Graph
- **POSG**: Predicate, Object, Subject, Graph
- **OSPG**: Object, Subject, Predicate, Graph

The use of `BTreeSet` (instead of `HashSet`) provides:
1. **Deterministic ordering** for consistent delta computation
2. **Efficient range queries** for pattern matching
3. **Canonical iteration** for reproducible hashing

### Performance Characteristics

| Operation | Time Complexity | Space Complexity |
|-----------|----------------|------------------|
| `union(other)` | O(n + m) | O(n + m) |
| `difference(other)` | O(n log m) | O(n) |
| `intersection(other)` | O(n log m) | O(min(n,m)) |
| `diff(target)` | O(n log m + m log n) | O(n + m) |
| `apply_diff(add, rem)` | O(a log n + r log n) | O(1) |

Where:
- n = size of self
- m = size of other
- a = size of additions
- r = size of removals

## ΔGate Protocol Integration

### Computing Deltas

```rust
use oxrdf::*;

// State at version 1
let mut v1 = Dataset::new();
let ex1 = NamedNodeRef::new("http://example.com/1").unwrap();
v1.insert(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph));

// State at version 2
let mut v2 = Dataset::new();
let ex2 = NamedNodeRef::new("http://example.com/2").unwrap();
v2.insert(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph));

// Compute Δ(v1 → v2)
let (delta_plus, delta_minus) = v1.diff(&v2);

// delta_plus contains new quads (Δ⁺)
// delta_minus contains removed quads (Δ⁻)
```

### Applying Deltas

```rust
use oxrdf::*;

let mut current = /* ... current state ... */;
let delta_plus = /* ... received additions ... */;
let delta_minus = /* ... received removals ... */;

// Transform current state by applying delta
current.apply_diff(&delta_plus, &delta_minus);

// current is now at the new version
```

### Scope Envelope: Cover(O)

Extract the subgraph relevant to a specific object:

```rust
use oxrdf::*;

let dataset = /* ... full dataset ... */;
let object = NamedNodeRef::new("http://example.com/focus").unwrap();

// Get all quads mentioning this object
let mut cover = Dataset::new();
for quad in dataset.quads_for_object(&object) {
    cover.insert(quad);
}

// cover now contains Cover(O) - all quads involving the object
```

### Computing Δ for Scope Envelope

```rust
use oxrdf::*;

let before_full = /* ... full dataset before ... */;
let after_full = /* ... full dataset after ... */;
let object = /* ... focus object ... */;

// Extract scope envelopes
let mut before_cover = Dataset::new();
for quad in before_full.quads_for_object(&object) {
    before_cover.insert(quad);
}

let mut after_cover = Dataset::new();
for quad in after_full.quads_for_object(&object) {
    after_cover.insert(quad);
}

// Compute delta for this scope
let (delta_plus, delta_minus) = before_cover.diff(&after_cover);

// This delta represents changes to the scope envelope around the object
```

## Graph-Level Operations

All operations are also available for `Graph` (single RDF graph):

```rust
use oxrdf::*;

let g1 = Graph::new();
let g2 = Graph::new();

// Same operations as Dataset
let union = g1.union(&g2);
let (additions, removals) = g1.diff(&g2);
g1.apply_diff(&additions, &removals);
```

## Integration with Existing Features

### Canonicalization

Combine with existing canonicalization for isomorphism-aware deltas:

```rust
use oxrdf::*;
use oxrdf::dataset::CanonicalizationAlgorithm;

let mut ds1 = /* ... dataset with blank nodes ... */;
let mut ds2 = /* ... dataset with blank nodes ... */;

// Canonicalize first for blank node normalization
ds1.canonicalize(CanonicalizationAlgorithm::Unstable);
ds2.canonicalize(CanonicalizationAlgorithm::Unstable);

// Now compute canonical delta
let (additions, removals) = ds1.diff(&ds2);
```

### Named Graphs

Delta operations work seamlessly with named graphs:

```rust
use oxrdf::*;

let g1 = NamedNodeRef::new("http://graph.com/1").unwrap();
let g2 = NamedNodeRef::new("http://graph.com/2").unwrap();

let mut ds1 = Dataset::new();
let ex = NamedNodeRef::new("http://example.com").unwrap();
ds1.insert(QuadRef::new(ex, ex, ex, g1));

let mut ds2 = Dataset::new();
ds2.insert(QuadRef::new(ex, ex, ex, g2));

// Delta includes graph name changes
let (additions, removals) = ds1.diff(&ds2);
```

## Best Practices

### 1. Use Deterministic Iteration

When computing hashes for delta verification:

```rust
use oxrdf::*;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

fn compute_hash(dataset: &Dataset) -> u64 {
    let mut hasher = DefaultHasher::new();
    // Iteration order is guaranteed stable
    for quad in dataset.iter() {
        quad.hash(&mut hasher);
    }
    hasher.finish()
}
```

### 2. Batch Delta Application

For multiple deltas, batch them for efficiency:

```rust
use oxrdf::*;

let mut dataset = Dataset::new();
let deltas = vec![
    (delta1_add, delta1_rem),
    (delta2_add, delta2_rem),
    // ...
];

// Combine deltas
let mut combined_add = Dataset::new();
let mut combined_rem = Dataset::new();

for (add, rem) in deltas {
    combined_add = combined_add.union(&add);
    combined_rem = combined_rem.union(&rem);
}

// Apply once
dataset.apply_diff(&combined_add, &combined_rem);
```

### 3. Verify Roundtrip

Always verify delta computation is reversible:

```rust
use oxrdf::*;

let original = /* ... dataset ... */;
let modified = /* ... dataset ... */;

// Compute delta
let (additions, removals) = original.diff(&modified);

// Apply and verify
let mut result = original.clone();
result.apply_diff(&additions, &removals);
assert_eq!(result, modified);
```

## Testing

Comprehensive test suite in `tests/deltagate_test.rs`:

```bash
cargo test -p oxrdf --test deltagate_test
```

Tests cover:
- Set operations (union, difference, intersection, symmetric difference)
- Delta computation and application
- Roundtrip verification
- Deterministic iteration
- Named graph support
- Edge cases (empty deltas, idempotent operations)

## Future Enhancements

Potential improvements for ΔGate support:

1. **Optimized Delta Encoding**: Compact binary representation for transmission
2. **Incremental Updates**: Stream-based delta application for large datasets
3. **Conflict Detection**: Identify conflicting concurrent deltas
4. **Delta Compression**: Minimize delta size for common patterns
5. **Lazy Evaluation**: Compute deltas on-demand without materializing

## API Reference

### Dataset Methods

```rust
impl Dataset {
    // Set operations
    pub fn union(&self, other: &Self) -> Self;
    pub fn difference(&self, other: &Self) -> Self;
    pub fn intersection(&self, other: &Self) -> Self;
    pub fn symmetric_difference(&self, other: &Self) -> Self;

    // Delta operations
    pub fn diff(&self, target: &Self) -> (Self, Self);
    pub fn apply_diff(&mut self, additions: &Self, removals: &Self);

    // Existing operations (now with deterministic iteration)
    pub fn iter(&self) -> Iter<'_>;
    pub fn contains<'a>(&self, quad: impl Into<QuadRef<'a>>) -> bool;
    pub fn insert<'a>(&mut self, quad: impl Into<QuadRef<'a>>) -> bool;
    pub fn remove<'a>(&mut self, quad: impl Into<QuadRef<'a>>) -> bool;
}
```

### Graph Methods

```rust
impl Graph {
    // Set operations
    pub fn union(&self, other: &Self) -> Self;
    pub fn difference(&self, other: &Self) -> Self;
    pub fn intersection(&self, other: &Self) -> Self;
    pub fn symmetric_difference(&self, other: &Self) -> Self;

    // Delta operations
    pub fn diff(&self, target: &Self) -> (Self, Self);
    pub fn apply_diff(&mut self, additions: &Self, removals: &Self);
}
```

## Contributing

When extending ΔGate support:

1. Maintain deterministic iteration (use `BTreeSet`, not `HashSet`)
2. Preserve set operation semantics (union, intersection, etc.)
3. Add doctests for all new methods
4. Add integration tests in `tests/deltagate_test.rs`
5. Document time/space complexity
6. Consider canonicalization interactions

## License

Same as oxigraph: MIT OR Apache-2.0
