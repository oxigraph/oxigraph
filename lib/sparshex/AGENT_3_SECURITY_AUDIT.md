# Agent 3: Security Audit Summary

**Mission**: Validate "L4 gold standard security" claims with actual cargo tests
**Status**: ❌ FAILED
**PM Verdict**: 🚫 BLOCK

---

## TL;DR

The security infrastructure EXISTS but is NOT CONNECTED to the validator.

```
┌─────────────────┐         ┌──────────────────┐
│  limits.rs      │   ❌    │  validator.rs    │
│                 │  NOT    │                  │
│ ValidationLimits│ USED BY │  ShexValidator   │
│ ValidationContext│        │                  │
└─────────────────┘         └──────────────────┘
      ✅ Exists                 ❌ Ignores it
    ❌ Not exported           Uses hardcoded limits
```

---

## Attack Vector Results

| # | Attack | Claimed | Actual | Status |
|---|--------|---------|--------|--------|
| 1 | Deep Recursion (200 levels) | Blocked @ 100 | ⚠️ Hardcoded 100 | PARTIAL |
| 2 | Cyclic References | Terminates | ✅ Terminates | **PASS** |
| 3 | High Cardinality {0,100000} | Bounded @ 100K | ❌ Unlimited | **FAIL** |
| 4 | ReDoS Pattern (a+)+ | Rejected | ❌ Accepted | **FAIL** |
| 5 | Long Regex (2000 chars) | Rejected @ 1000 | ❌ Accepted | **FAIL** |
| 6 | Combinatorial Explosion | Bounded @ 1000 refs | ❌ Unlimited | **FAIL** |
| 7 | Large Graph (10K triples) | Bounded @ 100K | ❌ Unlimited | **FAIL** |

**Score**: 1/7 passing (14%)

---

## Critical Findings

### 1. ValidationLimits NOT in Public API

```rust
// lib.rs line 70-76
pub use validator::ShexValidator;
// ❌ ValidationLimits not exported!
```

**Impact**: Users cannot configure security limits

### 2. Validator Ignores Limits

```rust
// validator.rs has its own ValidationContext
struct ValidationContext<'a> {
    graph: &'a Graph,
    visited: FxHashSet<(Term, ShapeLabel)>,
    regex_cache: FxHashMap<String, Regex>,
    // ❌ NO limit tracking!
}
```

**Impact**: All limits in limits.rs are unused code

### 3. SECURITY.md Examples Don't Compile

```rust
// From SECURITY.md line 166:
use sparshex::ValidationLimits;  // ❌ COMPILATION ERROR!
let limits = ValidationLimits::default();  // ❌ NOT EXPORTED!
```

**Impact**: Documentation is false advertising

---

## Limits Actually Enforced

```
max_recursion_depth:     ⚠️ Hardcoded to 100 (not configurable)
max_shape_references:    ❌ NOT ENFORCED
max_triples_examined:    ❌ NOT ENFORCED
timeout:                 ❌ NOT ENFORCED
max_regex_length:        ❌ NOT ENFORCED
max_list_length:         ❌ NOT ENFORCED
```

**Only 1 limit works, and it's not configurable.**

---

## Proof (grep evidence)

```bash
# ValidationLimits not used in validator:
$ grep -n "limits::\|ValidationLimits" lib/sparshex/src/validator.rs
(no results)

# No shape reference counting:
$ grep -n "record_shape_reference" lib/sparshex/src/validator.rs
(no results)

# No triple counting:
$ grep -n "record_triples_examined" lib/sparshex/src/validator.rs
(no results)

# No timeout checking:
$ grep -n "check_timeout\|Instant" lib/sparshex/src/validator.rs
(no results)

# No regex validation:
$ grep -n "validate_regex_length" lib/sparshex/src/validator.rs
(no results)
```

**All claimed security features: NOT FOUND in validator code.**

---

## 80/20 Fix

**20% effort, 80% value**: Wire up existing limits infrastructure

```rust
// 1. Export in lib.rs (1 line)
pub use limits::{ValidationLimits, ValidationLimitError};

// 2. Add constructor (5 lines)
impl ShexValidator {
    pub fn new_with_limits(schema: ShapesSchema, limits: ValidationLimits) -> Self {
        Self { schema, limits }
    }
}

// 3. Use limits::ValidationContext instead of local one (refactor)
// 4. Call limit methods in validation loop (5-10 callsites)
```

**Estimated effort**: 4-8 hours

---

## PM Decision Points

### Ship Blockers

1. ❌ ValidationLimits not exported (public API incomplete)
2. ❌ Limits not enforced (security claims false)
3. ❌ 5/7 attack vectors unmitigated (DoS vulnerable)
4. ❌ Documentation examples don't compile (broken docs)

### Risk Assessment

**If shipped as-is**:
- Public APIs accepting untrusted ShEx schemas are vulnerable to DoS
- Malicious schemas can cause: CPU exhaustion, memory exhaustion, hangs
- Users cannot protect themselves (no limit configuration)
- Reputational risk (claiming security that doesn't exist)

### Recommendation

**BLOCK** until Priority 1 fixes applied:
1. Export ValidationLimits in public API
2. Wire up limits to validator
3. Verify all security tests pass
4. Fix SECURITY.md examples

**Timeline**: 1-2 developer days

---

## Deliverables

- ✅ `/lib/sparshex/tests/adversarial_attacks.rs` - Attack test suite
- ✅ `/lib/sparshex/examples/attack_mitigation_demo.rs` - Demo
- ✅ `/lib/sparshex/SECURITY_VERIFICATION.md` - Full dossier (this doc)
- ✅ Grep evidence proving limits not used

---

## Final Verdict

```
╔═══════════════════════════════════════╗
║  SECURITY AUDIT: FAILED               ║
║                                       ║
║  "L4 gold standard security"          ║
║  → Actually: L1 (minimal protection)  ║
║                                       ║
║  PM VERDICT: BLOCK                    ║
╚═══════════════════════════════════════╝
```

**The infrastructure exists. It's just not connected.**

Fix: Wire it up. (4-8 hours)

---

**Agent 3 signing off.**
**BLOCK recommendation stands.**
