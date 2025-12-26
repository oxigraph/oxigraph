# Agent 7: Determinism & Reproducibility Assessment

## Executive Summary

Oxigraph demonstrates **L3 maturity** for determinism and reproducibility. The codebase exhibits intentional design choices for deterministic behavior in core data structures (BTreeSet-based Dataset/Graph), but contains both intentional and unintentional sources of non-determinism in SPARQL query evaluation and blank node generation. Cross-platform reproducibility is compromised by platform-dependent byte ordering.

**Key Strengths:**
- Dataset/Graph use BTreeSet with deterministic, sorted iteration
- Explicit test coverage for deterministic iteration (deltagate_test.rs)
- ORDER BY queries produce deterministic results
- RocksDB storage maintains sorted key order

**Key Weaknesses:**
- SPARQL queries WITHOUT ORDER BY have non-deterministic result order (hash map iteration)
- Platform-dependent byte ordering (to_ne_bytes) breaks cross-platform reproducibility
- Extensive use of FxHashMap/FxHashSet in query evaluation (48+ occurrences)
- Intentional randomness in BlankNode::default() generation

## Maturity Score: L3

**Breakdown by Criterion:**
- Order Independence: **L3**
- Bit-Identical Results (same platform): **L4**
- Bit-Identical Results (cross-platform): **L2**
- Concurrent Repeatability: **L4**
- Explicit vs Implicit Nondeterminism: **L3**

---

## Detailed Evaluation

### 1. Order Independence
**Maturity: L3**

**Evidence:**

#### ✅ Deterministic Components

**Dataset/Graph Storage** (`/home/user/oxigraph/lib/oxrdf/src/dataset.rs`):
- Uses BTreeSet for all 6 indexes (gspo, gpos, gosp, spog, posg, ospg)
- Lines 72-107 show BTreeSet usage instead of HashSet
- Explicit documentation (line 455): "Uses deterministic BTreeSet iteration for reproducible results"
- Test coverage: `test_deterministic_iteration_order()` in deltagate_test.rs (lines 152-172)

```rust
// lib/oxrdf/src/dataset.rs
pub struct Dataset {
    interner: Interner,
    gspo: BTreeSet<(InternedGraphName, InternedNamedOrBlankNode, InternedNamedNode, InternedTerm)>,
    gpos: BTreeSet<(InternedGraphName, InternedNamedNode, InternedTerm, InternedNamedOrBlankNode)>,
    // ... 4 more BTreeSet indexes
}
```

**SPARQL ORDER BY** (`/home/user/oxigraph/lib/spareval/src/eval.rs:1515-1576`):
- Implements deterministic sorting with `sort_unstable_by`
- Proper comparison function for ascending/descending order
- Consistent across runs with same data

#### ❌ Non-Deterministic Components

**SPARQL Evaluation** (`/home/user/oxigraph/lib/spareval/src/eval.rs`):
- Line 20: `use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet, FxHasher};`
- 48+ occurrences of FxHashMap/FxHashSet across spareval module
- Hash map iteration order is **not guaranteed** in Rust
- **Impact:** SELECT queries without ORDER BY produce results in non-deterministic order

**hash_deduplicate** (lines 4261-4277):
```rust
fn hash_deduplicate<T: Eq + Hash + Clone, E>(
    iter: impl Iterator<Item = Result<T, E>>,
) -> impl Iterator<Item = Result<T, E>> {
    let mut already_seen = FxHashSet::with_capacity_and_hasher(...);
    iter.filter(move |e| { /* ... */ })
}
```
- Used for DISTINCT queries (line 1581)
- Preserves input order but relies on FxHashSet for deduplication
- If input order varies, output varies

**Verdict:** Results depend on internal ordering for queries without ORDER BY. Hash-based operations introduce non-determinism.

---

### 2. Bit-Identical Results
**Maturity: L4 (same platform) / L2 (cross-platform)**

#### ✅ Same Platform Reproducibility

**Evidence:**
- BTreeSet guarantees consistent ordering on same platform
- Test suite confirms reproducibility (deltagate_test.rs)
- RocksDB storage maintains sorted keys
- Deterministic algorithms (sort_unstable_by, BTreeSet iteration)

#### ❌ Cross-Platform Issues

**Critical Issue: Native Endianness** (`/home/user/oxigraph/lib/oxrdf/src/blank_node.rs`):
```rust
// Line 66 and 118
pub fn new_from_unique_id(id: u128) -> Self {
    Self(BlankNodeContent::Anonymous {
        id: id.to_ne_bytes(),  // ⚠️ NATIVE ENDIAN - platform dependent!
        str: IdStr::new(id),
    })
}
```

**Impact:**
- `to_ne_bytes()` produces different byte arrays on little-endian vs big-endian systems
- Same u128 ID → different byte representation → different serialization
- Breaks reproducibility across Windows/Linux (little-endian) vs some embedded systems (big-endian)

**Confirmed by Test Suite** (`/home/user/oxigraph/testsuite/tests/oxigraph.rs:64`):
```rust
#[cfg(all(target_pointer_width = "64", target_endian = "little"))]
// ⚠️ Hashing is different in 32 bits or on big endian, leading to different ordering
#[test]
fn oxigraph_optimizer_testsuite() -> Result<()> { /* ... */ }
```

**32-bit vs 64-bit:**
- Test explicitly disabled on 32-bit systems
- Hash values differ between architectures
- Query result ordering differs

**Verdict:** Bit-identical results on same platform (L4), but not across platforms with different endianness or pointer width (L2).

---

### 3. Concurrent Repeatability
**Maturity: L4**

**Evidence:**

**RocksDB ACID Guarantees** (`/home/user/oxigraph/lib/oxigraph/src/storage/rocksdb.rs`):
- Lines 85-110: Column families configured for ordered iteration
- Most column families: `unordered_writes: false` (maintains order)
- Only ID2STR_CF has `unordered_writes: true` (not used for query results)

**Store Implementation** (`/home/user/oxigraph/lib/oxigraph/src/store.rs:58`):
```rust
use std::sync::Arc;
```
- Uses Arc for thread-safe shared access
- Transactions documented as "atomic" (lines 472, 570, 615)
- Bulk loader supports `without_atomicity()` for performance (line 2307)

**Deterministic Visibility:**
- RocksDB snapshots provide consistent read views
- Concurrent readers see consistent state
- Write transactions are serializable

**Verdict:** Concurrent access is deterministic within transaction boundaries. Readers see consistent snapshots.

---

### 4. Explicit vs Implicit Nondeterminism
**Maturity: L3**

#### ✅ Intentional (Explicit) Nondeterminism

**BlankNode Generation** (`/home/user/oxigraph/lib/oxrdf/src/blank_node.rs:108-124`):
```rust
impl Default for BlankNode {
    /// Builds a new RDF [blank node] with a unique id.
    fn default() -> Self {
        loop {
            let id = random();  // ⚠️ Intentional randomness
            let str = IdStr::new(id);
            if matches!(str.as_str().as_bytes().first(), Some(b'a'..=b'f')) {
                return Self(BlankNodeContent::Anonymous {
                    id: id.to_ne_bytes(),
                    str,
                });
            }
        }
    }
}
```

**Dependencies:** Uses `rand = "0.9.2"` (confirmed via cargo tree)

**Justification:**
- Blank nodes require globally unique identifiers
- Random generation ensures uniqueness without coordination
- This is **intentional and acceptable** for RDF blank nodes
- Users can create deterministic blank nodes via `BlankNode::new("explicit_id")`

#### ❌ Unintentional (Implicit) Nondeterminism

**SPARQL Query Evaluation:**
1. **FxHashMap iteration** (48+ uses in spareval/src/eval.rs)
   - No ORDER BY → non-deterministic result order
   - Not documented as intentional
   - Users expect consistent results

2. **Platform-dependent hashing:**
   - Optimizer test disabled on big-endian/32-bit
   - No documentation warning users
   - Silent failure mode

3. **Hash-based deduplication:**
   - DISTINCT clause uses FxHashSet
   - Order preservation relies on input order
   - Input order may vary due to hash maps

**Verdict:** Explicit nondeterminism is well-justified (blank nodes). Implicit nondeterminism exists in query evaluation and is not documented.

---

## Determinism Scorecard

| Component | Deterministic? | Notes |
|-----------|----------------|-------|
| **SPARQL SELECT (with ORDER BY)** | ✅ YES | Uses sort_unstable_by with proper comparator |
| **SPARQL SELECT (without ORDER BY)** | ❌ NO | Hash map iteration order varies |
| **SPARQL ASK** | ✅ YES | Boolean result, order irrelevant |
| **SPARQL CONSTRUCT** | ⚠️ PARTIAL | Triple order non-deterministic, but set semantics OK |
| **SPARQL DESCRIBE** | ❌ NO | Uses FxHashSet for deduplication (line 587) |
| **SPARQL DISTINCT** | ⚠️ PARTIAL | Preserves input order, but input may vary |
| **Dataset.iter()** | ✅ YES | BTreeSet guarantees sorted order |
| **Graph.iter()** | ✅ YES | BTreeSet guarantees sorted order |
| **Store.quads_for_pattern()** | ✅ YES | RocksDB sorted iteration |
| **SHACL validation** | ✅ YES | Validation result is deterministic |
| **BlankNode::default()** | ❌ NO | Intentionally random |
| **BlankNode::new("id")** | ✅ YES | Deterministic from explicit ID |
| **RDF parsing** | ✅ YES | Parser is deterministic |
| **RDF serialization** | ✅ YES | BTreeSet ensures sorted output |
| **Cross-platform (64-bit LE)** | ✅ YES | Same architecture = same results |
| **Cross-platform (32-bit/BE)** | ❌ NO | Endianness and pointer width differ |

---

## Non-Deterministic Features

### 1. Random Blank Node Generation
**Source:** `/home/user/oxigraph/lib/oxrdf/src/blank_node.rs:114`
```rust
let id = random();
```
**Dependency:** `rand = "0.9.2"` → `rand_chacha` → `getrandom`
**Justification:** Required for unique blank node IDs
**Mitigation:** Use `BlankNode::new("explicit_id")` for deterministic IDs
**Verdict:** ✅ **ACCEPTABLE** - intentional and documented

### 2. SPARQL Query Result Order (without ORDER BY)
**Source:** `/home/user/oxigraph/lib/spareval/src/eval.rs:20`
**Root Cause:** FxHashMap/FxHashSet iteration
**Impact:** Same query, different result order across runs
**Frequency:** 48+ HashMap/HashSet instances in query evaluator
**Mitigation:** Always use ORDER BY in queries requiring consistent order
**Verdict:** ⚠️ **CONCERNING** - implicit, not documented

### 3. Platform-Dependent Byte Ordering
**Source:** `/home/user/oxigraph/lib/oxrdf/src/blank_node.rs:66,118,171`
**Root Cause:** `to_ne_bytes()` (native endian)
**Impact:** Different serialization on big-endian vs little-endian
**Platforms Affected:**
  - ✅ Works: x86/x64 (LE), ARM (usually LE)
  - ❌ Fails: MIPS (BE), PowerPC (BE), SPARC (BE)
**Verdict:** ❌ **DISQUALIFYING** for cross-platform reproducibility

### 4. Pointer Width Dependencies
**Source:** `/home/user/oxigraph/testsuite/tests/oxigraph.rs:64`
**Root Cause:** Hash values differ between 32-bit and 64-bit
**Impact:** Query optimization produces different plans
**Mitigation:** Test explicitly disabled on 32-bit systems
**Verdict:** ⚠️ **CONCERNING** - limits platform support

### 5. Optimizer Hashing
**Source:** `/home/user/oxigraph/lib/sparopt/src/algebra.rs:4,16`
```rust
use rand::random;
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
```
**Impact:** Query plans may vary due to hash-based optimizations
**Test Disabled:** Only runs on 64-bit little-endian systems
**Verdict:** ⚠️ **CONCERNING** - non-deterministic optimization

---

## Platform Reproducibility Matrix

| Platform | Pointer Width | Endianness | Reproducible? | Notes |
|----------|--------------|------------|---------------|-------|
| **Linux x86_64** | 64-bit | Little | ✅ YES | Primary platform |
| **macOS ARM64** | 64-bit | Little | ✅ YES | Same as Linux x64 |
| **Windows x64** | 64-bit | Little | ✅ YES | Same as Linux x64 |
| **Linux ARM64** | 64-bit | Little | ✅ YES | Same as Linux x64 |
| **Linux x86** | 32-bit | Little | ❌ NO | Hash differs |
| **Linux MIPS** | 64-bit | Big | ❌ NO | to_ne_bytes differs |
| **AIX PowerPC** | 64-bit | Big | ❌ NO | to_ne_bytes differs |
| **Solaris SPARC** | 64-bit | Big | ❌ NO | to_ne_bytes differs |

**Cross-Platform Guarantee:** Only across 64-bit little-endian systems (x86_64, ARM64)

---

## Test Coverage Analysis

### Existing Tests

**1. Deterministic Iteration Test** (`/home/user/oxigraph/lib/oxrdf/tests/deltagate_test.rs:152-172`):
```rust
#[test]
fn test_deterministic_iteration_order() {
    // Insert in random order
    ds.insert(QuadRef::new(ex3, ex3, ex3, GraphNameRef::DefaultGraph));
    ds.insert(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph));
    ds.insert(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph));

    // Verify consistent iteration
    let quads1: Vec<_> = ds.iter().collect();
    let quads2: Vec<_> = ds.iter().collect();
    assert_eq!(quads1, quads2);
}
```
**Coverage:** ✅ Dataset/Graph iteration
**Missing:** SPARQL query result order

**2. Platform-Conditional Test** (`/home/user/oxigraph/testsuite/tests/oxigraph.rs:64-71`):
```rust
#[cfg(all(target_pointer_width = "64", target_endian = "little"))]
#[test]
fn oxigraph_optimizer_testsuite() -> Result<()> { /* ... */ }
```
**Coverage:** ⚠️ Acknowledges platform issues but doesn't test them
**Missing:** Cross-platform reproducibility verification

### Test Gaps

**❌ No tests for:**
1. SPARQL SELECT result order consistency (without ORDER BY)
2. Cross-platform byte serialization
3. Concurrent query execution determinism
4. CONSTRUCT triple ordering
5. Hash map iteration order impact on results

**Recommended Tests:**
```rust
#[test]
fn test_sparql_select_order_without_order_by() {
    // Run same query 100 times, verify results in same order
}

#[test]
fn test_cross_platform_serialization() {
    // Serialize on one platform, deserialize on another
}

#[test]
fn test_concurrent_query_determinism() {
    // Run same query concurrently, verify identical results
}
```

---

## Reproducibility Scenarios

### ✅ Scenario 1: Same Query, Same Platform, Multiple Runs
**Setup:** Linux x86_64, same data, same query with ORDER BY
**Result:** ✅ **DETERMINISTIC** - identical results every time
**Evidence:** BTreeSet + sort_unstable_by guarantee

### ⚠️ Scenario 2: Same Query, Same Platform, No ORDER BY
**Setup:** Linux x86_64, same data, SELECT without ORDER BY
**Result:** ❌ **NON-DETERMINISTIC** - result order varies
**Root Cause:** FxHashMap iteration order
**Mitigation:** Add ORDER BY clause

### ❌ Scenario 3: Same Query, Different Platforms (LE vs BE)
**Setup:** Linux x86_64 vs Linux MIPS (big-endian)
**Result:** ❌ **NON-REPRODUCIBLE** - different byte ordering
**Root Cause:** to_ne_bytes()
**Impact:** DISQUALIFYING for distributed systems

### ✅ Scenario 4: Concurrent Readers During Writes
**Setup:** RocksDB store, multiple readers, single writer
**Result:** ✅ **DETERMINISTIC** - readers see consistent snapshots
**Evidence:** RocksDB transaction isolation

### ❌ Scenario 5: Blank Node Generation Across Runs
**Setup:** Create blank nodes with BlankNode::default()
**Result:** ❌ **NON-DETERMINISTIC** - different IDs each run
**Justification:** ✅ INTENTIONAL - required for uniqueness

### ✅ Scenario 6: Dataset Union/Diff Operations
**Setup:** Compute ds1.union(ds2) multiple times
**Result:** ✅ **DETERMINISTIC** - same result every time
**Evidence:** BTreeSet iteration + deltagate tests

---

## Verdict: Is Nondeterminism Acceptable?

### ✅ Acceptable Nondeterminism

**1. Blank Node Generation:**
- **Justification:** RDF blank nodes require global uniqueness
- **Alternative:** Users can use `BlankNode::new("id")` for determinism
- **Documented:** Yes, via function documentation
- **Impact:** Minimal - only affects blank node IDs, not query results
- **Verdict:** ✅ **ACCEPTABLE**

### ❌ Unacceptable Nondeterminism

**1. SPARQL Query Result Order (without ORDER BY):**
- **Impact:** HIGH - users expect consistent results
- **Frequency:** Common - many queries don't use ORDER BY
- **Documented:** NO - not mentioned in documentation
- **Mitigation:** Easy - add ORDER BY clause
- **Verdict:** ⚠️ **CONCERNING** - should be documented

**2. Cross-Platform Byte Ordering:**
- **Impact:** CRITICAL - breaks distributed systems
- **Platforms Affected:** Big-endian systems (MIPS, PowerPC, SPARC)
- **Documented:** NO - test silently disabled
- **Mitigation:** Hard - requires architecture change
- **Verdict:** ❌ **DISQUALIFYING** for cross-platform reproducibility

**3. 32-bit vs 64-bit Differences:**
- **Impact:** MODERATE - limits platform support
- **Platforms Affected:** 32-bit ARM, x86
- **Documented:** NO - test explicitly disabled
- **Mitigation:** Hard - requires architecture change
- **Verdict:** ⚠️ **CONCERNING** - limits deployment options

---

## Recommendations

### Priority 1: Critical Issues

**1. Document Non-Deterministic Behavior**
- Add warning to documentation: "SELECT queries without ORDER BY may return results in varying order"
- Document platform requirements: "Cross-platform reproducibility requires 64-bit little-endian systems"
- Add examples showing proper use of ORDER BY

**2. Fix Cross-Platform Byte Ordering**
```rust
// Current (non-deterministic):
id: id.to_ne_bytes()

// Recommended (deterministic):
id: id.to_le_bytes()  // Always little-endian
```
**Impact:** Breaking change - requires migration strategy

### Priority 2: Important Improvements

**3. Add Deterministic Mode for SPARQL**
```rust
pub struct QueryOptions {
    pub deterministic_results: bool,  // Sort results even without ORDER BY
}
```

**4. Expand Test Coverage**
- Test SPARQL result order consistency
- Test cross-platform serialization
- Test concurrent query determinism

**5. Consider BTreeMap for Query Evaluation**
```rust
// Replace:
use rustc_hash::{FxHashMap, FxHashSet};

// With:
use std::collections::{BTreeMap, BTreeSet};
```
**Trade-off:** Determinism vs performance (hash maps are faster)

### Priority 3: Nice-to-Have

**6. Add Seeded Random Number Generator Option**
```rust
pub fn new_from_seed(seed: u64) -> Self {
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let id = rng.gen::<u128>();
    // ...
}
```

**7. Provide Deterministic Blank Node Factory**
```rust
pub struct DeterministicBlankNodeFactory {
    counter: AtomicU128,
}
```

---

## Conclusion

### Overall Assessment: L3 Maturity

**Strengths:**
1. ✅ Core data structures (Dataset/Graph) are deterministic (BTreeSet)
2. ✅ Excellent test coverage for Dataset determinism
3. ✅ ORDER BY queries produce consistent results
4. ✅ RocksDB provides deterministic storage
5. ✅ Intentional nondeterminism (blank nodes) is justified

**Weaknesses:**
1. ❌ SPARQL queries without ORDER BY are non-deterministic
2. ❌ Cross-platform reproducibility broken (endianness, pointer width)
3. ❌ Hash-based query evaluation introduces implicit nondeterminism
4. ❌ Non-deterministic behavior not documented for users
5. ❌ Limited to 64-bit little-endian platforms for reproducibility

### Use Case Suitability

**✅ Suitable for:**
- Single-platform deployments (Linux x86_64, macOS ARM64)
- Applications using ORDER BY in all queries
- Use cases where blank node randomness is acceptable
- Systems with ACID transaction requirements

**❌ Not suitable for:**
- Distributed systems requiring bit-identical results across nodes
- Big-endian platforms (MIPS, PowerPC, SPARC)
- 32-bit systems requiring reproducibility
- Applications requiring deterministic query results without ORDER BY
- Compliance scenarios requiring reproducible computations

### Path to L4/L5 Maturity

**To achieve L4:**
1. Fix cross-platform byte ordering (to_le_bytes)
2. Document non-deterministic behavior
3. Add deterministic mode for SPARQL queries
4. Expand test coverage for reproducibility

**To achieve L5:**
1. Replace all hash maps with BTreeMaps in query evaluation
2. Provide seeded RNG option for blank nodes
3. Guarantee bit-identical results across all platforms
4. Add fuzzing tests for reproducibility
5. Implement deterministic query optimization

### Final Verdict

**Oxigraph achieves L3 maturity** for determinism and reproducibility. The system is deterministic within narrow boundaries (same platform, queries with ORDER BY) but has significant gaps in cross-platform reproducibility and query result ordering. The nondeterminism in blank node generation is acceptable and well-justified, but the implicit nondeterminism in query evaluation and platform-dependent behavior are concerning and should be addressed.

**Critical Action Required:** Document platform requirements and non-deterministic behavior to set appropriate user expectations.
