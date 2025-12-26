# N3 Rules PM Verification Status

**Date:** 2025-12-26
**Agent:** Agent 4 - N3 Rules Termination Test Agent
**PM Assessment:** L0-L1 (Not Implemented)

## Executive Summary

N3 rule **execution** is NOT implemented. While N3 syntax parsing and static rule-to-OWL conversion exist, there is no iterative rule engine that executes N3 rules with variable unification.

## Feature Status

| Feature | Status | Evidence |
|---------|--------|----------|
| N3 Syntax Parsing | ✅ YES | Uses `N3Parser` from oxttl |
| N3 Formula Extraction | ✅ YES | `formulas::extract_formulas()` in n3_integration.rs |
| N3 Rule Detection | ✅ YES | `N3RuleExtractor` can find `log:implies` statements |
| N3 Rule-to-OWL Conversion | ✅ PARTIAL | Only simple subclass patterns converted |
| **N3 Rule Execution** | ❌ NO | No rule engine, no iteration mechanism |
| **Variable Unification** | ❌ NO | Variables explicitly skipped (line 114, 129 n3_integration.rs) |
| **Termination Bounds** | N/A | No execution → no termination needed |

## What Exists

### 1. N3 Parsing Infrastructure
```rust
// File: lib/oxowl/src/n3_integration.rs
pub fn parse_n3_ontology<R: Read>(reader: R) -> Result<Ontology, OwlParseError>
```
- Parses N3 syntax successfully
- Extracts formulas from blank node graphs
- **Skips variables** in quad conversion (lines 114, 129)

### 2. N3 Rule Extraction
```rust
// File: lib/oxowl/src/n3_rules.rs
pub struct N3Rule {
    pub antecedent: Formula,
    pub consequent: Formula,
}
```
- Detects `log:implies` predicates
- Extracts antecedent and consequent formulas
- Pattern matching for simple cases (subclass, property implication)

### 3. Static Rule-to-OWL Conversion
```rust
// Line 82-93 in n3_rules.rs
pub fn to_owl_axioms(&self) -> Vec<Axiom>
```
- Converts pattern `{ ?x a :Dog } => { ?x a :Animal }` to `Dog ⊑ Animal`
- Only handles fixed patterns, NOT general rule execution

## What Does NOT Exist

### 1. Rule Execution Engine
**No code exists for:**
- Applying N3 rules to generate new triples
- Variable binding/unification
- Forward chaining with N3 rules
- Iteration over rule applications

**Proof:**
```bash
$ grep -r "unif\|bind\|substitute" lib/oxowl/src/
# No matches found
```

### 2. Variable Unification
**Variables are explicitly rejected:**
```rust
// Line 114 in n3_integration.rs
N3Term::Variable(_) => return None, // Skip variables
```

### 3. Termination Bounds for N3
**Only OWL reasoner has iteration limits:**
```rust
// lib/oxowl/src/reasoner/mod.rs:19
pub max_iterations: usize,  // Default: 100_000
```

This applies to **OWL 2 RL reasoning**, not N3 rule execution.

## OWL Reasoner vs N3 Rule Engine

| Feature | OWL Reasoner (EXISTS) | N3 Rule Engine (MISSING) |
|---------|----------------------|--------------------------|
| Rule Type | Fixed OWL 2 RL rules | User-defined N3 rules |
| Variables | No variables | Requires unification |
| Iteration | Yes (max 100k) | Not applicable |
| Location | `src/reasoner/mod.rs` | Does not exist |
| Example | `apply_transitive_property_rules()` | Would need `execute_n3_rules()` |

## Compilation Evidence

```bash
$ cargo check -p oxowl 2>&1
    Checking oxowl v0.1.0 (/home/user/oxigraph/lib/oxowl)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.64s
```

**Status:** ✅ Compiles successfully (after fixing N3Term::Triple issue)

## Code Archaeology

### Files Examined
1. `/home/user/oxigraph/lib/oxowl/src/n3_rules.rs` (227 lines)
   - Rule extraction and pattern matching
   - NO execution logic
2. `/home/user/oxigraph/lib/oxowl/src/n3_integration.rs` (319 lines)
   - N3 parsing and formula extraction
   - Variables skipped
3. `/home/user/oxigraph/lib/oxowl/src/reasoner/mod.rs` (664 lines)
   - OWL 2 RL forward chaining
   - NOT N3 rule execution

### Test Coverage
- `/home/user/oxigraph/lib/oxowl/tests/n3_integration.rs` (376 lines)
- Tests parsing, formula extraction, static conversion
- **No tests for rule execution** (because it doesn't exist)

## Blocking Issues

### Critical Gap: No Rule Execution Engine
To implement N3 rule execution would require:

1. **Variable Unification Engine**
   - Bind variables in antecedent to RDF terms
   - Substitute bindings into consequent
   - Generate new triples

2. **Forward Chaining Loop**
   ```rust
   // Pseudocode - does not exist
   loop {
       for rule in n3_rules {
           for binding in unify(rule.antecedent, graph) {
               let new_triple = substitute(rule.consequent, binding);
               graph.insert(new_triple);
           }
       }
   }
   ```

3. **Termination Detection**
   - Fixpoint detection (no new triples)
   - Iteration limit enforcement
   - Cycle detection for recursive rules

4. **Safety Bounds**
   - Max iterations per rule
   - Max triples generated
   - Timeout mechanism

### Example of Missing Functionality

**N3 Rule:**
```n3
{ ?x :parent ?y . ?y :parent ?z } => { ?x :grandparent ?z }
```

**What exists:** Can parse and detect this rule
**What's missing:** Cannot execute it to generate `:grandparent` triples

## PM Verdict

**STATUS: ✅ VERIFIED - N3 Rule Execution Not Implemented**

The PM assessment of L0-L1 is **CORRECT**.

### Evidence Summary
1. ✅ Crate compiles
2. ✅ N3 parsing works (static only)
3. ❌ N3 rule execution does not exist
4. ❌ Variable unification not implemented
5. ❌ Termination bounds N/A (no execution to terminate)

### Gap Analysis
- **20% exists:** Parsing, extraction, pattern conversion
- **80% missing:** Execution engine, unification, iteration

### Recommended Action
If N3 rule execution is desired, implement:
1. Unification algorithm (`unify(pattern, graph) -> Vec<Binding>`)
2. Substitution engine (`substitute(formula, binding) -> Vec<Triple>`)
3. Forward chaining loop with fixpoint detection
4. Safety bounds (max iterations, max triples, timeout)

### Test Creation Status
Created placeholder tests in `/home/user/oxigraph/lib/oxowl/tests/n3_adversarial.rs`:
- 1 test runs: N3 rule extraction (works)
- 5 tests ignored: N3 rule execution (not implemented)

## Reproducibility

```bash
# Verify compilation
cd /home/user/oxigraph
cargo check -p oxowl

# Verify no unification code
grep -r "unify\|substitute.*var" lib/oxowl/src/
# Result: No matches

# Verify variables are skipped
grep -A2 "Variable(_)" lib/oxowl/src/n3_integration.rs
# Result: "return None" - variables not processed

# Run existing tests
cargo test -p oxowl --test n3_integration
# Result: Tests pass (only test parsing, not execution)

# Run adversarial tests
cargo test -p oxowl --test n3_adversarial
# Result: 1 passed; 0 failed; 5 ignored
```

## Conclusion

N3 rule execution infrastructure does not exist in oxowl. The codebase only supports:
- Static N3 parsing
- Formula extraction
- Pattern-based conversion to OWL axioms

No dynamic rule execution, variable unification, or iteration mechanism exists.
The PM L0-L1 assessment is accurate and verified.

---
**Verification Complete**
Agent 4 - N3 Rules Termination Test Agent
