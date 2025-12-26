# Agent 1: SPARQL Maturity Assessment

## Executive Summary

**Overall Maturity Score: L2 (Development Suitable)**

**Production Readiness Verdict: NOT READY**

Oxigraph's SPARQL engine contains multiple critical flaws that make it **unsafe for production use** under high-QPS or adversarial query conditions. While the optimizer is well-designed and the code quality is high, the engine lacks fundamental resource controls that would prevent trivial denial-of-service attacks. Without query complexity limits, timeout enforcement, or memory bounds, a single malicious query can exhaust server resources.

---

## Maturity Score: L2/L5

**Why not L4 (Production Safe)?**
- No enforced query timeouts (only optional manual cancellation)
- Unbounded memory consumption in ORDER BY, GROUP BY, and property paths
- No query complexity limits or cost ceilings
- No concurrent query limits or QPS controls
- Non-deterministic result ordering in some cases

---

## Detailed Evaluation

### 1. High-QPS Concurrent Querying
**Maturity: L3 (Limited Production)**

#### Strengths:
- **Thread-safe Store implementation**: `/home/user/oxigraph/lib/oxigraph/src/store.rs:101-104` shows `Store` is `Clone`, backed by `Arc<Storage>`
- **Repeatable read isolation**: Lines 70-72 guarantee snapshot isolation per query
- **No global locks during query execution**: Each query gets its own snapshot view
- **Cancellation token mechanism exists**: `/home/user/oxigraph/lib/spareval/src/lib.rs:77,373` provides manual query cancellation

#### Critical Weaknesses:
- **No QPS limits**: Nothing prevents 10,000 concurrent queries from running simultaneously
- **No connection pooling**: Each `Store.clone()` shares storage but has no resource limits
- **No automatic timeout enforcement**: Timeouts require manual `CancellationToken` setup
- **Memory unbounded**: Each concurrent query can consume unlimited RAM

**Evidence**: `/home/user/oxigraph/lib/oxigraph/src/store.rs:58-64` shows threading support but no concurrency controls:
```rust
use std::sync::Arc;
#[cfg(not(target_family = "wasm"))]
use std::sync::mpsc;
#[cfg(not(target_family = "wasm"))]
use std::thread;
```

**Jobs-To-Be-Done Scenarios:**
- ❌ **"When executing 1000 concurrent SELECT queries, I need predictable memory consumption"**
  - FAIL: No per-query memory limits, unbounded accumulation possible
- ⚠️ **"When running high-QPS workload, I need latency p99 < 100ms"**
  - PARTIAL: Possible for simple queries, but no enforcement mechanism prevents slow queries from blocking

---

### 2. Adversarial Query Patterns
**Maturity: L1 (Prototype Only)**

#### Critical Vulnerabilities Found:

**A. Unbounded Transitive Closure (Property Paths)**
- **Location**: `/home/user/oxigraph/lib/spareval/src/eval.rs:4209-4238`
- **Issue**: Property paths like `?s :rel* ?o` build a COMPLETE HashSet of all reachable nodes before returning ANY results
- **Attack**: `SELECT * WHERE { ?s foaf:knows* ?o }` on a social graph with 1M users connected in a chain will materialize 1M nodes in RAM
- **Code Evidence**:
```rust
fn transitive_closure<T: Clone + Eq + Hash, E, NI: Iterator<Item = Result<T, E>>>(
    start: impl IntoIterator<Item = Result<T, E>>,
    mut next: impl FnMut(T) -> NI,
) -> impl Iterator<Item = Result<T, E>> {
    let mut todo = start.into_iter()...collect::<Vec<_>>();
    let mut all = todo.iter().cloned().collect::<FxHashSet<_>>();  // UNBOUNDED
    while let Some(e) = todo.pop() {
        for e in next(e) {
            match e {
                Ok(e) => {
                    if all.insert(e.clone()) {
                        todo.push(e)  // NO DEPTH LIMIT
                    }
                }
```
- **Severity**: CRITICAL - Trivial DoS, no depth limit, no result limit

**B. Unbounded ORDER BY**
- **Location**: `/home/user/oxigraph/lib/spareval/src/eval.rs:1548-1574`
- **Issue**: ORDER BY collects ALL results into a Vec before sorting
- **Attack**: `SELECT * WHERE { ?s ?p ?o } ORDER BY ?s` on 100M triples = 100M tuples in RAM
- **Code Evidence**:
```rust
values.sort_unstable_by(|a, b| {  // Line 1551
    for comp in &by {
        // ... sorting logic
    }
    Ordering::Equal
});
```
- **Severity**: CRITICAL - Memory exhaustion guaranteed on large datasets

**C. Unbounded GROUP BY**
- **Location**: `/home/user/oxigraph/lib/spareval/src/eval.rs:1683-1716`
- **Issue**: GROUP BY materializes ALL groups in a HashMap before returning results
- **Attack**: `SELECT ?s (COUNT(*) AS ?c) WHERE { ?s ?p ?o } GROUP BY ?s` with 10M distinct subjects = 10M HashMap entries
- **Code Evidence**:
```rust
let mut accumulators_for_group = FxHashMap::<
    Vec<Option<D::InternalTerm>>,
    Vec<AccumulatorWrapper<'_, D::InternalTerm>>,
>::default();  // Line 1683-1686
// ... materializes entire child iterator:
child(from)
    .filter_map(...)
    .for_each(|tuple| {  // Processes ALL before returning
        let key = key_variables.iter()...collect();
        let key_accumulators = accumulators_for_group.entry(key)...
```
- **Severity**: CRITICAL - Guaranteed OOM on large cardinality GROUP BY

**D. Cartesian Product Materialization**
- **Location**: `/home/user/oxigraph/lib/spareval/src/eval.rs:3741-3774`
- **Issue**: Cartesian joins materialize the "build" side completely
- **Attack**: `SELECT * WHERE { { ?s1 ?p1 ?o1 } { ?s2 ?p2 ?o2 } }` (no shared variables) on 1K triples each = 1M results buffered
- **Code Evidence**:
```rust
struct CartesianProductJoinIterator<'a, T> {
    probe_iter: Peekable<InternalTuplesIterator<'a, T>>,
    built: Vec<InternalTuple<T>>,  // ENTIRE BUILD SIDE IN MEMORY
    buffered_results: Vec<Result<InternalTuple<T>, QueryEvaluationError>>,
}
```
- **Severity**: HIGH - Memory proportional to build side size

**E. OPTIONAL Join Pathology**
- **Location**: `/home/user/oxigraph/lib/spareval/src/eval.rs:1405-1434`
- **Issue**: LEFT JOIN builds entire right side into memory
- **Attack**: `SELECT * WHERE { ?s ?p ?o OPTIONAL { ?s :largeProperty ?value } }` where right side is 10M triples
- **Code Evidence**:
```rust
let mut right_values = InternalTupleSet::new(keys.clone());
right_values.extend(right(from.clone()).filter_map(...)  // FULL MATERIALIZATION
```
- **Severity**: HIGH - Right side unbounded

#### Optimizer Quality (The Good News):
- **Location**: `/home/user/oxigraph/lib/sparopt/src/optimizer.rs:15-728`
- **Greedy join reordering** (lines 479-727): Picks smallest patterns first, uses cardinality estimates
- **Filter pushdown** (lines 276-477): Moves filters closer to base patterns
- **Cardinality estimation** (lines 892-1088): Estimates pattern sizes to guide join ordering
- **Hash join with key detection** (lines 879-890): Uses join keys when available

**BUT**: Optimizer cannot prevent adversarial queries, only optimize legitimate ones.

---

### 3. Determinism
**Maturity: L2 (Development Suitable)**

#### Non-Deterministic Cases Found:

**A. Hash Map Iteration Order**
- **Location**: `/home/user/oxigraph/lib/spareval/src/eval.rs:1683,1723`
- **Issue**: GROUP BY results returned in `FxHashMap` iteration order, which is not stable
- **Evidence**:
```rust
use rustc_hash::{FxHashMap, FxHashSet};  // Line 20
// Later:
accumulators_for_group.into_iter().map(...)  // Line 1723 - hash iteration order
```
- **Impact**: Same query, different result order on repeated execution
- **Severity**: MEDIUM - Results are correct, but ordering varies

**B. Hash Deduplication**
- **Location**: `/home/user/oxigraph/lib/spareval/src/eval.rs:4261-4277`
- **Issue**: DISTINCT uses `FxHashSet`, order depends on hash values
- **Impact**: Non-deterministic result ordering without ORDER BY

#### Deterministic Cases (Well-Handled):
- **Triple pattern matching**: Uses database iterator order (deterministic per snapshot)
- **JOIN results**: Order preserved from inputs when no hashing
- **UNION**: Order is deterministic (concatenation of branches)
- **ORDER BY**: Explicit sort provides determinism

**Verdict**: Deterministic within a transaction snapshot, but result ORDER is non-deterministic unless explicitly specified.

---

### 4. Latency Bounds
**Maturity: L1 (Prototype Only)**

#### Timeout Mechanism Found:
- **Location**: `/home/user/oxigraph/lib/spareval/src/lib.rs:343-376`
- **Mechanism**: Optional `CancellationToken`
- **Code**:
```rust
pub fn with_cancellation_token(mut self, cancellation_token: CancellationToken) -> Self {
    self.cancellation_token = Some(cancellation_token);
    self
}
```
- **Checking**: `/home/user/oxigraph/lib/spareval/src/eval.rs:107-113,226-228,248`
```rust
cancellation_token.ensure_alive()?;
```

#### Critical Gaps:
- ❌ **No automatic timeout**: Timeout requires manual token creation and cancellation from another thread
- ❌ **No query complexity limit**: Optimizer estimates cost but doesn't reject expensive queries
- ❌ **No per-operator timeout**: Only checked at specific points (quad iteration, graph enumeration)
- ❌ **No worst-case ceiling**: Transitive closure can run unbounded even with cancellation
- ❌ **Regex complexity limit exists** (`/home/user/oxigraph/lib/spareval/src/expression.rs:30`: `REGEX_SIZE_LIMIT: usize = 1_000_000`) but no other limits

**Worst-Case Scenarios:**
1. **Property path explosion**: `?s :rel{1,1000} ?o` with branching factor 10 = 10^1000 paths (limited only by memory)
2. **Cartesian product**: N x M join with N=10K, M=10K = 100M results
3. **Deep recursion in EXISTS**: Nested EXISTS with property paths can stack overflow

**Evidence of No Limits**:
```rust
// sparopt/src/optimizer.rs:1016 - Estimates size but doesn't reject
(false, false, false) => 1_000_000_000,  // Unbounded triple pattern
```

---

### 5. Explainability
**Maturity: L4 (Production Safe)**

#### Strengths:
- **Excellent error types**: `/home/user/oxigraph/lib/spareval/src/error.rs:8-89`
  - Specific errors: `UnboundService`, `UnsupportedCustomFunction`, `Cancelled`
  - Source error chaining with `#[source]`
  - Detailed context (function name, arity mismatch)

- **Query explanation API**: `/home/user/oxigraph/lib/spareval/src/lib.rs:705-814`
  - `explain()` method returns `QueryExplanation`
  - JSON serialization of query plan
  - Planning duration tracking
  - Per-operator statistics (if enabled with `compute_statistics()`)

- **Plan visualization**: `/home/user/oxigraph/lib/spareval/src/eval.rs:4491-4635`
  - Pretty-printed plan trees
  - Shows join algorithms, filter expressions, property paths
  - Includes cardinality estimates in debug output

**Example Error Message Quality**:
```rust
#[error("The custom function {name} requires between {} and {} arguments, but {actual} were given", .expected.start(), .expected.end())]
UnsupportedCustomFunctionArity {
    name: NamedNode,
    expected: RangeInclusive<usize>,
    actual: usize,
}
```

#### Weaknesses:
- **No query cost estimation in error**: When query times out, no indication of estimated cost
- **Limited stack traces**: Rust errors don't show which triple pattern caused the issue
- **No intermediate progress reporting**: Can't see which part of query is slow

---

## JTBD Pass/Fail Matrix

| Job To Be Done | Pass/Fail | Evidence |
|----------------|-----------|----------|
| Execute 1000 concurrent SELECT queries with p99 < 100ms | ❌ FAIL | No QPS limits, no per-query resource controls |
| Handle pathological property path `?s :rel* ?o` on 1M node graph | ❌ FAIL | Unbounded transitive closure (`eval.rs:4209`), materializes all nodes |
| Execute `SELECT * WHERE {?s ?p ?o} ORDER BY ?s` on 100M triples | ❌ FAIL | Unbounded Vec materialization (`eval.rs:1551`) |
| Run same query twice, get identical result order | ⚠️ PARTIAL | Deterministic data, non-deterministic order without ORDER BY |
| Reject query exceeding 10-second timeout automatically | ❌ FAIL | Manual `CancellationToken` only, no automatic timeout |
| Handle nested OPTIONAL with 10 levels | ⚠️ PARTIAL | Works but each level can materialize unbounded right side |
| Execute Cartesian product of 1K x 1K patterns | ❌ FAIL | Materializes 1M tuples (`eval.rs:3743`) |
| Get detailed error when custom function fails | ✅ PASS | Excellent error messages (`error.rs:28-33`) |
| Query 10 billion triples with GROUP BY on 10M groups | ❌ FAIL | HashMap materialization (`eval.rs:1683`) |
| Use EXPLAIN to understand slow query | ✅ PASS | Full query plan with statistics (`lib.rs:1075-1113`) |
| Cancel long-running query from another thread | ✅ PASS | `CancellationToken` works (`lib.rs:373`) |
| Filter 1B triples with complex FILTER expression | ✅ PASS | Streaming evaluation, no materialization |

---

## Known Unsafe Query Patterns

### CRITICAL (Guaranteed Resource Exhaustion):

1. **Unbounded Property Paths**
   ```sparql
   SELECT * WHERE { ?s foaf:knows* ?person }
   # On social graph: materializes entire reachability set
   ```
   **File**: `eval.rs:4209-4238`

2. **Large ORDER BY**
   ```sparql
   SELECT * WHERE { ?s ?p ?o } ORDER BY ?s
   # On 100M triples: 100M tuples in RAM
   ```
   **File**: `eval.rs:1548-1574`

3. **High-Cardinality GROUP BY**
   ```sparql
   SELECT ?s (COUNT(*) AS ?c) WHERE { ?s ?p ?o } GROUP BY ?s
   # On 10M subjects: 10M HashMap entries
   ```
   **File**: `eval.rs:1683-1716`

4. **Cartesian Product**
   ```sparql
   SELECT * WHERE {
     { ?s1 ?p1 ?o1 }
     { ?s2 ?p2 ?o2 }
   }
   # 1K x 1K = 1M tuples materialized
   ```
   **File**: `eval.rs:3741-3774`

### HIGH (Likely Resource Exhaustion):

5. **Large RIGHT side of OPTIONAL**
   ```sparql
   SELECT * WHERE {
     ?s ?p ?o
     OPTIONAL { ?s :property ?value }
   }
   # If :property has 10M values, 10M tuples in HashSet
   ```
   **File**: `eval.rs:1405-1434`

6. **Nested Property Paths**
   ```sparql
   SELECT * WHERE { ?s (:rel1/:rel2)* ?o }
   # Exponential explosion: each level multiplies paths
   ```
   **File**: `eval.rs:2996-3012`

7. **DISTINCT on Large Result Set**
   ```sparql
   SELECT DISTINCT * WHERE { ?s ?p ?o }
   # On 100M triples: 100M HashSet insertions
   ```
   **File**: `eval.rs:4261-4277`

### MEDIUM (Possible Resource Issues):

8. **Deep UNION**
   ```sparql
   SELECT * WHERE {
     { PATTERN_1 } UNION { PATTERN_2 } UNION ... UNION { PATTERN_1000 }
   }
   # 1000 iterators stacked
   ```
   **File**: `eval.rs:1453-1471`

9. **Complex Aggregates**
   ```sparql
   SELECT (SAMPLE(?o) AS ?s1) (SAMPLE(?o) AS ?s2) ... (SAMPLE(?o) AS ?s100)
   WHERE { ?s ?p ?o } GROUP BY ?s
   # 100 accumulators per group
   ```
   **File**: `eval.rs:1668-1677`

---

## Required Mitigations to Reach L4

### P0 (Blocking Production Use):

1. **Implement query timeout enforcement**
   - Add default 30-second timeout
   - Make timeout configurable per query
   - Auto-cancel on timeout, return partial results or error
   - **File to modify**: `spareval/src/lib.rs`, `spareval/src/eval.rs`

2. **Add result size limits**
   - Limit ORDER BY to max 10K rows (configurable)
   - Limit GROUP BY to max 10K groups (configurable)
   - Fail fast when limit exceeded
   - **File to modify**: `spareval/src/eval.rs:1548,1683`

3. **Bound transitive closure depth**
   - Add max depth parameter (default 1000)
   - Add max results parameter (default 100K)
   - Return error when exceeded
   - **File to modify**: `spareval/src/eval.rs:4209-4259`

4. **Add per-query memory limits**
   - Track allocations in ORDER BY, GROUP BY, hash joins
   - Fail when exceeding limit (e.g., 1GB per query)
   - **File to modify**: `spareval/src/eval.rs` (all materializing operations)

### P1 (High Priority):

5. **Implement QPS limits**
   - Add semaphore-based concurrency control
   - Configurable max concurrent queries (default 100)
   - Queue excess queries or reject
   - **File to modify**: `oxigraph/src/store.rs`

6. **Add query complexity limits**
   - Use optimizer cost estimates to reject expensive queries
   - Configurable complexity threshold
   - Allow admin override for specific queries
   - **File to modify**: `sparopt/src/optimizer.rs`, `spareval/src/lib.rs`

7. **Streaming aggregations (where possible)**
   - For simple aggregates (COUNT, SUM) without GROUP BY, stream instead of materializing
   - Reduces memory for common case
   - **File to modify**: `spareval/src/eval.rs:1656-1746`

### P2 (Nice to Have):

8. **Add deterministic mode**
   - Option to sort hash-based results (GROUP BY, DISTINCT)
   - Use `BTreeMap` instead of `HashMap` when determinism required
   - **File to modify**: `spareval/src/eval.rs:1683,4264`

9. **Progress reporting API**
   - Callback for intermediate progress (rows processed, time elapsed)
   - Helps debugging slow queries
   - **File to modify**: `spareval/src/eval.rs`

10. **Cost-based warnings**
    - Return warnings for potentially expensive queries
    - Don't block, but inform user
    - **File to modify**: `spareval/src/lib.rs`

---

## Production Readiness Verdict

### VERDICT: NOT READY

**Explicit Reasoning:**

Oxigraph's SPARQL engine is a **well-architected prototype** with excellent code quality, good optimizer design, and solid error handling. However, it lacks the **fundamental guardrails** required for production deployment under adversarial or high-load conditions.

### Why Not Production Ready:

1. **Trivial DoS Vectors**: A single malicious query can exhaust all server memory
   - `SELECT * WHERE { ?s :rel* ?o } ORDER BY ?s` = guaranteed OOM
   - No mitigation exists except manual query review

2. **No Resource Isolation**: Concurrent queries compete unbounded for RAM
   - 100 concurrent GROUP BY queries can each allocate 1GB
   - No per-query limits, no QPS controls

3. **No Automatic Failsafes**: All protections require manual setup
   - Timeouts need `CancellationToken` + background thread
   - Production systems need automatic, enforced limits

### Where It IS Safe to Use:

- ✅ **Trusted query environments**: Internal tools with known query patterns
- ✅ **Low concurrency**: < 10 concurrent queries, trusted users
- ✅ **Small datasets**: < 1M triples, no adversarial patterns
- ✅ **Development/testing**: Excellent for SPARQL compliance testing

### Where It Is NOT Safe:

- ❌ **Public SPARQL endpoints**: Trivially attackable
- ❌ **High-QPS APIs**: No rate limiting or resource controls
- ❌ **Large datasets with complex queries**: ORDER BY/GROUP BY will OOM
- ❌ **Multi-tenant environments**: No query isolation

### Path to Production (Estimated Effort):

- **P0 mitigations**: ~2-3 weeks (timeouts, result limits, depth bounds)
- **P1 mitigations**: ~2-3 weeks (QPS limits, complexity limits, streaming aggs)
- **Comprehensive testing**: ~1-2 weeks (adversarial query suite, stress testing)

**Total: ~5-8 weeks** to reach L4 (Production Safe)

---

## Conclusion

Oxigraph is a **high-quality L2 system** that works well for development and trusted environments. The optimizer is sophisticated, the error handling is excellent, and the concurrency model is sound. However, **production deployment requires significant hardening** to prevent resource exhaustion attacks.

**Recommendation**: Do not deploy to untrusted or high-load environments without implementing at least P0 mitigations. For internal tools with controlled queries and low concurrency, Oxigraph can be used today with manual monitoring of resource consumption.

**Positive Notes**:
- The codebase is clean and well-structured, making mitigations straightforward to add
- The optimizer provides a solid foundation for adding cost-based limits
- The cancellation mechanism can be extended to automatic timeouts
- Error handling and explainability are already production-grade

This is **fixable** with focused effort on resource limits and failsafes.
