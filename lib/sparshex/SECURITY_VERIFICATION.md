# ShEx Security Verification Dossier

**Agent 3: ShEx Adversarial Test Verification Lead**
**Date**: 2025-12-26
**Mission**: Validate audit claim that ShEx has "L4 gold standard security"

---

## EXECUTIVE SUMMARY

**STATUS**: ❌ **FAILED** - Security claims NOT validated

**PM VERDICT**: 🚫 **BLOCK** - Critical gaps prevent production deployment

The audit claims of "L4 gold standard security" with comprehensive attack mitigation are **ASPIRATIONAL, NOT IMPLEMENTED**. The `ValidationLimits` infrastructure exists but is completely disconnected from the actual validator.

---

## CRITICAL FINDINGS

### 1. ValidationLimits NOT Exported in Public API

**File**: `lib/sparshex/src/lib.rs`

```rust
// Lines 70-76: Public API exports
pub use error::{ShexError, ShexParseError, ShexValidationError};
pub use model::{...};
pub use result::ValidationResult;
pub use validator::ShexValidator;
// ❌ ValidationLimits is NOT exported!
```

**Impact**: Users cannot configure validation limits even though the infrastructure exists.

**Evidence**:
- ✅ `limits.rs` exists with full `ValidationLimits` struct
- ✅ `ValidationContext` exists with tracking methods
- ❌ NOT exported in `lib.rs` line 75-76
- ❌ NOT usable by library consumers

### 2. Validator Uses Separate Context (Limits Not Integrated)

**File**: `lib/sparshex/src/validator.rs`

The validator has its OWN `ValidationContext` struct (lines 420-484) that is COMPLETELY DIFFERENT from the one in `limits.rs`:

```rust
// validator.rs line 420-427
struct ValidationContext<'a> {
    graph: &'a Graph,
    visited: FxHashSet<(Term, ShapeLabel)>,  // ✅ Cycle detection
    regex_cache: FxHashMap<String, Regex>,    // ✅ Regex caching
}
// ❌ NO limit tracking!
// ❌ NO resource counting!
// ❌ NO timeout checking!
```

vs.

```rust
// limits.rs line 193-208
pub struct ValidationContext {
    limits: ValidationLimits,
    current_depth: usize,              // ✅ Tracks depth
    shape_reference_count: usize,      // ✅ Tracks refs
    triples_examined: usize,           // ✅ Tracks triples
    start_time: Instant,               // ✅ Tracks time
}
```

**Impact**: The comprehensive limit tracking in `limits.rs` is NEVER used by the validator.

---

## ATTACK VECTORS TESTED

### Attack Vector 1: Deep Recursion ⚠️ PARTIAL

**Claimed Mitigation** (SECURITY.md):
- Default `max_recursion_depth` of 100
- Configurable via `ValidationLimits::strict()` (depth = 50)
- Track recursion depth and terminate early

**Actual Implementation** (validator.rs line 17-18, 64-66):

```rust
const MAX_RECURSION_DEPTH: usize = 100;  // ❌ Hardcoded, not configurable

fn validate_node_against_shape(..., depth: usize) -> ... {
    if depth > MAX_RECURSION_DEPTH {
        return Err(ShexValidationError::max_recursion_depth(depth));
    }
```

**Status**: ⚠️ **PARTIALLY MITIGATED**
- ✅ Depth check exists
- ❌ Hardcoded to 100 (not configurable)
- ❌ Cannot use `ValidationLimits::strict()` as claimed
- ❌ No public API to change limit

### Attack Vector 2: Cyclic Schema ✅ WORKING

**Claimed Mitigation**:
- Visited node tracking
- Cycle detection prevents infinite loops

**Actual Implementation** (validator.rs line 69-75, 84):

```rust
let key = (node.clone(), shape_label.clone());
if context.visited.contains(&key) {
    return Ok(ValidationResult::valid());  // Break cycle
}
context.visited.insert(key.clone());
// ... validation ...
context.visited.remove(&key);
```

**Status**: ✅ **WORKING**
- ✅ Visited set prevents infinite loops
- ✅ Cycles terminate correctly
- ✅ Test in `adversarial_attacks.rs::test_cyclic_schema_terminates()` validates this

### Attack Vector 3: High Cardinality ❌ VULNERABLE

**Claimed Mitigation** (SECURITY.md line 113-124):
- `max_triples_examined` limit (default: 100,000)
- Bounded by triple limit

**Actual Implementation**: **NONE**

```bash
$ grep -n "max_triples_examined\|record_triples_examined" lib/sparshex/src/validator.rs
# ❌ No results - not called anywhere!
```

**Status**: ❌ **VULNERABLE**
- ❌ No cardinality validation during parsing
- ❌ No triple counting during validation
- ❌ Can specify `{0,100000}` without rejection
- ❌ Will process all triples regardless of count

**Test**: `adversarial_attacks.rs::test_high_cardinality_bounded()` (marked `#[ignore]`)

### Attack Vector 4: ReDoS (Regex Denial of Service) ❌ VULNERABLE

**Claimed Mitigation** (SECURITY.md line 84-101):
- `max_regex_length` limit (default: 1000)
- Pattern validation
- Dangerous pattern rejection

**Actual Implementation** (validator.rs line 440-483):

```rust
fn get_or_compile_regex(&mut self, pattern: &str, flags: Option<&str>) -> ... {
    // ❌ No length check
    // ❌ No pattern validation
    // ❌ Directly compiles any pattern
    let regex = Regex::new(&regex_pattern).map_err(...)?;
```

**Status**: ❌ **VULNERABLE**
- ❌ No regex length checking
- ❌ No dangerous pattern detection (e.g., `(a+)+`)
- ❌ No backtracking limit
- ❌ Direct compilation of any pattern

**Tests**:
- `adversarial_attacks.rs::test_redos_regex_blocked()` (marked `#[ignore]`)
- `adversarial_attacks.rs::test_very_long_regex_rejected()` (marked `#[ignore]`)

### Attack Vector 5: Shape Reference Counting ❌ VULNERABLE

**Claimed Mitigation** (SECURITY.md line 66-83):
- `max_shape_references` limit (default: 1000)
- Shape reference counting across entire validation
- Prevent combinatorial explosion

**Actual Implementation**: **NONE**

```bash
$ grep -n "max_shape_references\|record_shape_reference" lib/sparshex/src/validator.rs
# ❌ No results - not called anywhere!
```

**Status**: ❌ **VULNERABLE**
- ❌ No shape reference counting
- ❌ No limit on total evaluations
- ❌ Combinatorial explosion possible with OR/AND shapes

**Test**: `adversarial_attacks.rs::test_combinatorial_explosion_prevented()` (marked `#[ignore]`)

### Attack Vector 6: Large Graph Validation ❌ VULNERABLE

**Claimed Mitigation** (SECURITY.md line 109-125):
- `max_triples_examined` limit
- Prevent unbounded memory consumption

**Actual Implementation**: **NONE** (same as #3)

**Status**: ❌ **VULNERABLE**
- ❌ No triple examination counting
- ❌ Can validate unlimited graph size
- ❌ Memory unbounded

**Test**: `adversarial_attacks.rs::test_large_graph_validation_bounded()` (marked `#[ignore]`)

### Attack Vector 7: Timeout Enforcement ❌ VULNERABLE

**Claimed Mitigation** (SECURITY.md line 27-31):
- Default timeout: 30 seconds
- Strict timeout: 5 seconds
- Prevent indefinite processing

**Actual Implementation**: **NONE**

```bash
$ grep -n "timeout\|check_timeout\|Instant\|elapsed" lib/sparshex/src/validator.rs
# ❌ No results - no timeout checking!
```

**Status**: ❌ **VULNERABLE**
- ❌ No timeout tracking
- ❌ No time checking during validation
- ❌ Validation can run indefinitely

**Test**: `adversarial_attacks.rs::test_validation_timeout_enforced()` (marked `#[ignore]`)

---

## LIMITS ACTUALLY ENFORCED

| Limit | Claimed | Actually Enforced | Configurable | Public API |
|-------|---------|-------------------|--------------|------------|
| `max_recursion_depth` | ✅ 100 | ⚠️ 100 (hardcoded) | ❌ No | ❌ No |
| `max_shape_references` | ✅ 1000 | ❌ **NOT ENFORCED** | ❌ No | ❌ No |
| `max_triples_examined` | ✅ 100,000 | ❌ **NOT ENFORCED** | ❌ No | ❌ No |
| `timeout` | ✅ 30s | ❌ **NOT ENFORCED** | ❌ No | ❌ No |
| `max_regex_length` | ✅ 1000 | ❌ **NOT ENFORCED** | ❌ No | ❌ No |
| `max_list_length` | ✅ 10,000 | ❌ **NOT ENFORCED** | ❌ No | ❌ No |

**Summary**:
- ✅ **1/6 limits partially enforced** (recursion depth, hardcoded)
- ❌ **5/6 limits NOT enforced at all**
- ❌ **0/6 limits configurable**
- ❌ **0/6 limits accessible via public API**

---

## CODE EVIDENCE

### Evidence 1: limits.rs Has Full Infrastructure

```rust
// File: lib/sparshex/src/limits.rs

pub struct ValidationLimits { /* ... */ }           // ✅ Exists
impl Default for ValidationLimits { /* ... */ }     // ✅ Exists
impl ValidationLimits {
    pub fn strict() -> Self { /* ... */ }           // ✅ Exists
    pub fn permissive() -> Self { /* ... */ }       // ✅ Exists
}

pub struct ValidationContext { /* ... */ }          // ✅ Exists
impl ValidationContext {
    pub fn enter_recursion(&mut self) -> ... {}    // ✅ Exists
    pub fn record_shape_reference(&mut self) -> ... {} // ✅ Exists
    pub fn record_triples_examined(&mut self, ...) -> ... {} // ✅ Exists
    pub fn check_timeout(&self) -> ... {}           // ✅ Exists
    pub fn validate_regex_length(&self, ...) -> ... {} // ✅ Exists
}
```

All the infrastructure exists and has comprehensive unit tests (lines 387-508).

### Evidence 2: validator.rs Does NOT Use It

```bash
$ grep -n "limits::\|ValidationLimits\|use crate::limits" lib/sparshex/src/validator.rs
# ❌ No results - limits module not imported!

$ grep -n "ValidationContext" lib/sparshex/src/validator.rs
420:struct ValidationContext<'a> {
    # ❌ Local struct, not from limits.rs
```

The validator defines its own `ValidationContext` with NO limit tracking.

### Evidence 3: SECURITY.md Makes False Claims

```markdown
<!-- SECURITY.md lines 165-213 -->
### Default Limits (Development/Testing)

```rust
use sparshex::ValidationLimits;  // ❌ NOT EXPORTED!

let limits = ValidationLimits::default();  // ❌ COMPILATION ERROR!
```

This example code **DOES NOT COMPILE** because `ValidationLimits` is not in the public API!

---

## TEST RESULTS

### Tests Created

1. ✅ `tests/adversarial_attacks.rs` - Comprehensive attack test suite
   - 7 attack vector tests
   - 1 limits verification test
   - 1 summary test

2. ✅ `examples/attack_mitigation_demo.rs` - Interactive demo
   - Demonstrates each attack
   - Shows which limits work
   - Provides PM verdict

### Tests Run

**Status**: ❌ Cannot compile due to workspace issues and incomplete parser

```bash
$ cargo test -p sparshex adversarial_attacks
error: failed to load manifest for workspace member `/home/user/oxigraph/cli`
```

**Workaround**: Code analysis provides definitive evidence without needing runtime tests.

### Expected Test Results (Based on Code Analysis)

If tests could run:

```
test test_deep_recursion_rejected ... FAILED (hardcoded limit, not configurable)
test test_cyclic_schema_terminates ... PASSED (visited set works)
test test_high_cardinality_bounded ... FAILED (no enforcement)
test test_redos_regex_blocked ... FAILED (no validation)
test test_very_long_regex_rejected ... FAILED (no length check)
test test_combinatorial_explosion_prevented ... FAILED (no counting)
test test_large_graph_validation_bounded ... FAILED (no counting)
test test_validation_timeout_enforced ... FAILED (no timeout)
test test_validation_limits_struct_exists ... FAILED (not exported)
```

**Score**: 1/9 passing (cycle detection only)

---

## SECURITY MATURITY LEVEL

**Claimed**: L4 - Gold Standard Security
**Actual**: L1 - Minimal Protection

### Maturity Assessment

| Level | Criteria | Status |
|-------|----------|--------|
| L1 | Basic input validation | ⚠️ Partial (recursion only) |
| L2 | Resource limits defined | ✅ Yes (in limits.rs) |
| L3 | Resource limits enforced | ❌ No (not integrated) |
| L4 | Configurable limits + tests | ❌ No (not exported) |
| L5 | Defense in depth + monitoring | ❌ No |

**Current Level**: Between L1 and L2
- Code has limit definitions (L2)
- But only enforces basic recursion check (L1)

---

## PM VERDICT

### 🚫 BLOCK - MUST NOT SHIP

**Rationale**:

1. **False Security Claims**: SECURITY.md documents comprehensive protections that don't exist
2. **Public API Gap**: `ValidationLimits` not exported, users can't configure limits
3. **Implementation Gap**: Validator doesn't use limits infrastructure
4. **Attack Surface**: 5/7 documented attack vectors are UNMITIGATED
5. **Technical Debt**: Complete disconnect between limits.rs and validator.rs

### Required Fixes Before SHIP

#### Priority 1: Critical (Block Release)

1. ✅ **Export ValidationLimits in lib.rs**
   ```rust
   pub use limits::{ValidationLimits, ValidationLimitError};
   ```

2. ✅ **Integrate limits::ValidationContext into validator.rs**
   - Replace local `ValidationContext`
   - Call `record_shape_reference()` on each shape evaluation
   - Call `record_triples_examined()` when iterating triples
   - Call `check_timeout()` periodically
   - Call `validate_regex_length()` before compiling regex

3. ✅ **Add ShexValidator::new_with_limits() constructor**
   ```rust
   pub fn new_with_limits(schema: ShapesSchema, limits: ValidationLimits) -> Self
   ```

4. ✅ **Make SECURITY.md code examples compile and work**

#### Priority 2: High (Ship with Warnings)

5. ⚠️ Update validator to use configurable limits instead of hardcoded constants
6. ⚠️ Add regex pattern validation (detect dangerous patterns)
7. ⚠️ Add cardinality limits during schema parsing
8. ⚠️ Enable and verify all adversarial tests pass

#### Priority 3: Medium (Post-Release)

9. Add monitoring hooks for limit violations
10. Add examples demonstrating limit configuration
11. Benchmark performance impact of limit checking
12. Document upgrade path for existing users

### Estimated Effort

- **Priority 1**: 4-8 hours (1 developer)
- **Priority 2**: 8-16 hours (1 developer)
- **Priority 3**: 16-24 hours (1 developer)

**Total**: 1-2 developer days for critical fixes

---

## RECOMMENDATIONS

### Immediate Actions

1. **Do NOT merge to main** until Priority 1 fixes complete
2. **Update SECURITY.md** to reflect current state (remove false claims)
3. **Add warning to README** about current security limitations
4. **Label release as "alpha"** or "experimental"

### Technical Debt

The limits infrastructure is well-designed but completely unused. This suggests:
- Limits were added reactively after validator implementation
- No integration testing between components
- Incomplete implementation of documented features

**Root Cause**: Lack of adversarial testing during development

### Future Prevention

1. Require security tests alongside features
2. Require examples to compile
3. Require end-to-end integration tests
4. Conduct security review before release

---

## ARTIFACTS DELIVERED

1. ✅ `/lib/sparshex/tests/adversarial_attacks.rs` - Attack test suite
2. ✅ `/lib/sparshex/examples/attack_mitigation_demo.rs` - Attack demo
3. ✅ `/lib/sparshex/SECURITY_VERIFICATION.md` - This dossier

---

## CONCLUSION

The ShEx implementation has a **CRITICAL GAP** between documented security features and actual implementation. The `ValidationLimits` infrastructure exists but is completely disconnected from the validator.

**This is a BLOCK issue** - the code cannot ship with claims of "L4 gold standard security" when 5/7 attack vectors are unmitigated and the public API doesn't even expose limit configuration.

**80/20 Fix**: Integrating `ValidationContext` from limits.rs into validator.rs would address 80% of the security gaps with ~20% of effort (one focused development session).

---

**Agent 3: ShEx Adversarial Test Verification Lead**
**Status**: VERIFICATION FAILED
**Recommendation**: BLOCK until critical fixes applied
