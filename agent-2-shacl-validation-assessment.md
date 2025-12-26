# Agent 2: SHACL Validation Assessment

## Executive Summary

Oxigraph's SHACL implementation (`sparshacl`) is a **well-engineered validation library with excellent diagnostics** but fundamentally **NOT suitable for production admission control**. It lacks store integration, incremental validation, and safe shape evolution. While it has good recursion safeguards, several pathological shape patterns can cause quadratic or worse performance. **Maturity: L2-L3 (Development suitable, NOT production-ready for write-time validation)**.

## Overall Maturity Score: **L2-L3**

### Breakdown by Category:
- Admission Control: **L1** (Not integrated with store)
- Incremental Revalidation: **L1** (Full revalidation only)
- Shape Evolution: **L1** (Immutable validators)
- Recursive/Pathological Shapes: **L3-L4** (Good safeguards, but gaps exist)
- Diagnostic Clarity: **L4-L5** (Excellent)

---

## Detailed Evaluation

### 1. Admission Control
**Maturity: L1 (Prototype only)**

**Analysis:**
- ❌ **NO store integration**: SHACL validation is completely decoupled from `Store`/`MemoryStore`
- ❌ **NO transaction hooks**: No pre-commit validation capability
- ❌ **NO write-time gating**: Validation is post-hoc only
- ✅ JS/Python bindings provide `validateStore()` but this only **reads** the store

**Evidence from code:**
```rust
// js/src/shacl.rs:229
pub fn validate_store(&self, store: &JsStore) -> Result<...> {
    // Extracts quads from store and builds a graph
    let graph = store.store.iter().filter_map(...).fold(Graph::new(), ...);
    let report = self.inner.validate(&graph)...
}
```

This is **read-only validation** with no mechanism to reject writes.

**Production Requirement:**
```
"When ingesting 1M triples, I need SHACL to reject invalid data with <1s validation time"
```

**Current Reality:**
- You can validate a 1M triple graph in memory, but there's no way to block the write
- No incremental validation means validating 1M triples every time
- No integration with bulk_loader() or transaction commits

**Verdict: FAIL** - Cannot gate writes, cannot be used for admission control.

---

### 2. Incremental Revalidation
**Maturity: L1 (Full revalidation required)**

**Analysis:**
- ❌ **Always validates entire graph**: `validator.validate(&graph)` processes all shapes × all targets
- ❌ **No delta/diff-based validation**: Cannot validate only changed nodes
- ❌ **No result caching**: Every validation starts from zero
- ❌ **No incremental algorithms**: All constraint checks are stateless

**Evidence from code:**
```rust
// lib/sparshacl/src/validator.rs:42
pub fn validate(&self, data_graph: &Graph) -> Result<ValidationReport, ShaclError> {
    let mut report = ValidationReport::new();

    // Validate all node shapes (lines 47-65)
    for node_shape in self.shapes_graph.node_shapes() {
        let focus_nodes = self.find_focus_nodes(&node_shape.base, data_graph);
        for focus_node in focus_nodes {
            self.validate_node_against_shape(...)?;
        }
    }

    // Validate all property shapes (lines 68-88)
    for prop_shape in self.shapes_graph.property_shapes() { ... }
}
```

No early exit, no change detection, no caching.

**Cost for incremental updates:**
- Adding 1 triple to a 1M triple graph: **validates entire 1M triple graph**
- Updating 1 property: **revalidates all shapes and all targets**

**Verdict: FAIL** - Unsuitable for continuous write workloads.

---

### 3. Shape Evolution
**Maturity: L1 (Immutable validators)**

**Analysis:**
- ❌ **Validators are immutable**: `ShaclValidator` created with frozen `ShapesGraph`
- ❌ **No hot-reload**: Updating shapes requires creating new validator
- ❌ **No migration strategy**: No way to transition shapes safely under load
- ❌ **No versioning**: Shapes have no version metadata

**Evidence from code:**
```rust
// lib/sparshacl/src/validator.rs:30
impl ShaclValidator {
    pub fn new(shapes_graph: ShapesGraph) -> Self {
        Self { shapes_graph }  // Frozen at construction
    }
}
```

**Migration scenario:**
```
1. Create new validator with updated shapes
2. Stop all writes
3. Validate entire database with new shapes
4. If passes, swap validator
5. Resume writes
```

This requires **full write downtime**.

**Verdict: FAIL** - Not production-safe for evolving schemas.

---

### 4. Recursive/Pathological Shapes
**Maturity: L3-L4 (Good safeguards, but exploitable gaps)**

#### ✅ **Good Protections:**

1. **Recursion depth limit:**
```rust
// lib/sparshacl/src/validator.rs:21
const MAX_RECURSION_DEPTH: usize = 50;

// Lines 116-118, 162-164
if depth > MAX_RECURSION_DEPTH {
    return Err(ShaclValidationError::max_recursion_depth(depth).into());
}
```

2. **RDF list protections:**
```rust
// lib/sparshacl/src/model.rs:829
const MAX_LIST_LENGTH: usize = 10000;

// Lines 850-856, 894-900: Circular reference detection
if !visited.insert(current.clone()) {
    return Err(ShaclParseError::circular_list(current));
}
```

3. **Property path depth limit:**
```rust
// lib/sparshacl/src/path.rs:167
const MAX_DEPTH: usize = 100;

if depth > MAX_DEPTH {
    return;  // Silently stops, doesn't error
}
```

#### ⚠️ **Concerning Patterns:**

1. **Inverse paths on complex expressions:**
```rust
// lib/sparshacl/src/path.rs:217-239
Self::Inverse(inner) => {
    // For complex inverse paths, we need to iterate all triples
    for triple in graph {  // O(T) where T = total triples!!!
        let mut temp_results = Vec::new();
        inner.evaluate_into(graph, triple.subject.into(), &mut temp_results, ...);
        if temp_results.iter().any(|r| r.as_ref() == focus_node) {
            results.push(triple.subject.into_owned().into());
        }
    }
}
```

**Cost:** O(T × P) where T = total triples, P = path complexity

2. **ZeroOrMore paths with visited tracking:**
```rust
// lib/sparshacl/src/path.rs:242-258
Self::ZeroOrMore(inner) => {
    let focus_owned = focus_node.into_owned();
    if visited.insert(focus_owned.clone()) {
        results.push(focus_owned.clone());

        let mut temp_results = Vec::new();
        inner.evaluate_into(graph, focus_node, &mut temp_results, visited, depth + 1);

        for node in temp_results {
            if visited.insert(node.clone()) {
                results.push(node.clone());
                self.evaluate_into(graph, node.as_ref(), results, visited, depth + 1);
            }
        }
    }
}
```

**Cost:** O(N × D) where N = nodes reachable, D = max depth
- For deep hierarchies, this can explore thousands of nodes per validation

3. **Logical constraints multiply cost:**
```rust
// lib/sparshacl/src/validator.rs:716-738
Constraint::And(shape_ids) => {
    for value in value_nodes {
        for ref_shape_id in shape_ids {
            if !self.node_conforms_to_shape(context, value, ref_shape_id, depth + 1)? {
                // Creates temp report for each shape check (line 969)
                let mut temp_report = ValidationReport::new();
                self.validate_node_against_shape(context, &mut temp_report, ...);
            }
        }
    }
}
```

**Cost:** O(V × S) where V = value nodes, S = shapes in constraint

#### 🔴 **Unsafe Shape Patterns (See section below for details)**

**Verdict: PARTIAL PASS** - Has protections but can still be abused.

**Production Requirement:**
```
"When a shape references another shape recursively, I need validation
to terminate or be rejected"
```

**Current Reality:**
- Recursion terminates at depth 50 (good)
- But depth 50 × exponential branching = massive work
- Path evaluation can still blow up on dense graphs

---

### 5. Diagnostic Clarity
**Maturity: L4-L5 (Production-grade)**

#### ✅ **Excellent Error Reporting:**

```rust
// lib/sparshacl/src/report.rs:62-87
pub struct ValidationResult {
    pub focus_node: Term,              // Which node failed
    pub result_path: Option<PropertyPath>,  // Which property
    pub value: Option<Term>,           // What value caused failure
    pub source_shape: ShapeId,         // Which shape
    pub source_constraint_component: ConstraintComponent,  // Which constraint
    pub result_message: Option<String>,  // Human message
    pub result_severity: Severity,     // Violation/Warning/Info
    pub detail: Vec<ValidationResult>, // Nested details
}
```

**Example error messages:**
```rust
// lib/sparshacl/src/validator.rs:234-238
.with_message(format!(
    "Expected at least {} value(s), got {}",
    min, value_nodes.len()
))

// Line 287
.with_message(format!("Value is not an instance of <{}>", class.as_str()))
```

#### ✅ **Machine-Readable Reports:**

```rust
// lib/sparshacl/src/report.rs:220-320
pub fn to_graph(&self) -> Graph {
    // Serializes report to standard SHACL validation report RDF
    // sh:ValidationReport, sh:result, sh:focusNode, sh:resultPath, etc.
}
```

#### ✅ **Custom Messages:**

```rust
// lib/sparshacl/src/validator.rs:243-245
if let Some(msg) = &shape.message {
    result = result.with_message(msg.clone());
}
```

**Production Requirement:**
```
"When validation fails, I need to know exactly which constraint
failed on which node"
```

**Verdict: PASS** - Best-in-class diagnostics.

---

## Cost Model

### Complexity Analysis

#### **Base Cost (unavoidable):**
```
O(S × N × C × V)
```
Where:
- S = number of shapes
- N = average target nodes per shape
- C = average constraints per shape
- V = average value nodes per property

#### **Multipliers (shape-dependent):**

| Shape Feature | Cost Multiplier | Notes |
|--------------|----------------|-------|
| Simple property path | 1× | Direct lookup |
| Sequence path (length k) | k× | k graph hops |
| Alternative path (k alternatives) | k× | Tries each alternative |
| Inverse path (simple) | 1× | Indexed lookup |
| Inverse path (complex) | **T×** | Iterates all triples (!) |
| ZeroOrMore path | **N × D** | N nodes × D depth |
| OneOrMore path | **N × D** | Similar to ZeroOrMore |
| sh:and / sh:or / sh:xone (k shapes) | k× | Validates each nested shape |
| sh:not | 1× | Single conformance check |
| sh:node | 1× (recursive) | Adds depth |
| sh:qualifiedValueShape | V× | Checks each value |
| sh:closed | P× | P = properties of node |

#### **Worst-Case Scenarios:**

**Scenario 1: Dense hierarchy with ZeroOrMore**
```turtle
sh:path [ sh:zeroOrMorePath ex:parent ]
```
- Graph: 10,000 nodes in deep tree (depth 100)
- Cost: O(10,000 × 100) = **1M node visits per focus node**

**Scenario 2: Complex inverse path**
```turtle
sh:path [
    sh:inversePath [
        sh:alternativePath ( ex:p1 ex:p2 ex:p3 )
    ]
]
```
- Graph: 1M triples
- Cost: O(1M × 3) = **3M triple iterations per focus node**

**Scenario 3: Nested logical constraints**
```turtle
sh:and (
    [ sh:node Shape1 ]
    [ sh:node Shape2 ]
    [ sh:node Shape3 ]
)
```
Each nested shape can recurse, multiply by depth.

---

## Unsafe Shape Patterns

### 🔴 **CRITICAL: Avoid These in Production**

#### **1. ZeroOrMore with QualifiedValueShape**
```turtle
ex:DangerousShape a sh:NodeShape ;
    sh:targetClass ex:Person ;
    sh:property [
        sh:path [ sh:zeroOrMorePath ex:parent ] ;
        sh:qualifiedValueShape [
            sh:property [
                sh:path ex:role ;
                sh:hasValue "admin"
            ]
        ] ;
        sh:qualifiedMinCount 1
    ] .
```

**Why unsafe:**
- Explores entire ancestor tree for every Person
- Checks `sh:qualifiedValueShape` on every ancestor
- Dense graph × deep tree = exponential blow-up

**Attack:**
```
Create deeply nested hierarchy with 10k nodes
→ Every validation walks entire tree
→ 10k nodes × 100 depth × qualified checks = millions of operations
```

---

#### **2. Complex Inverse Paths**
```turtle
ex:DangerousShape a sh:NodeShape ;
    sh:targetClass ex:Document ;
    sh:property [
        sh:path [
            sh:inversePath [
                sh:alternativePath ( ex:references ex:citedBy ex:mentions )
            ]
        ] ;
        sh:minCount 1
    ] .
```

**Why unsafe:**
- Iterates **every triple in the graph** (see path.rs:226)
- Checks if subject→predicate→object matches for all 3 predicates
- O(T × 3) where T = total triples

**Attack:**
```
Load 1M triples into database
→ Every Document validation scans all 1M triples × 3 predicates
→ Single validation = 3M triple iterations
```

---

#### **3. Closed Shapes on High-Degree Nodes**
```turtle
ex:StrictShape a sh:NodeShape ;
    sh:targetClass ex:Entity ;
    sh:closed true ;
    sh:ignoredProperties ( rdf:type ) ;
    sh:property [
        sh:path ex:name ;
        sh:minCount 1
    ] .
```

**Why unsafe:**
- `sh:closed` requires iterating **all properties** of the subject
- High-degree nodes (100s of properties) × many targets = slow

**Attack:**
```
Create nodes with 1000 properties each
→ sh:closed checks all 1000 properties against allowed list
→ 1000 nodes × 1000 properties = 1M property checks
```

---

#### **4. Deep Recursion via sh:node**
```turtle
ex:NodeA a sh:NodeShape ;
    sh:node ex:NodeB .

ex:NodeB a sh:NodeShape ;
    sh:node ex:NodeC .

# ... continues for 50 levels
```

**Why unsafe:**
- Recursion limit is 50 (good!)
- But still allows validating 50 levels deep
- With complex constraints at each level = massive work

**Attack:**
```
Create 50-deep shape chain with complex constraints
→ Each level validates all constraints
→ Depth 50 × complex constraints = long validation times
```

---

#### **5. QualifiedValueShape with Overlapping Constraints**
```turtle
ex:TeamShape a sh:NodeShape ;
    sh:targetClass ex:Team ;
    sh:property [
        sh:path ex:member ;
        sh:qualifiedValueShape [ sh:class ex:Leader ] ;
        sh:qualifiedMinCount 1 ;
        sh:qualifiedMaxCount 2
    ] ;
    sh:property [
        sh:path ex:member ;
        sh:qualifiedValueShape [ sh:class ex:Member ] ;
        sh:qualifiedMinCount 5
    ] .
```

**Why unsafe:**
- Multiple `qualifiedValueShape` on same path
- Each validates **all value nodes** independently
- N members × 2 qualified shapes = 2N validations

**Attack:**
```
Team with 1000 members
→ Check leader qualification: 1000 member checks
→ Check member qualification: 1000 member checks
→ Total: 2000 qualified validations per team
```

---

### 🟡 **CAUTION: Monitor Performance**

#### **1. sh:equals / sh:disjoint on Large Value Sets**
```rust
// validator.rs:578-602
Constraint::Equals(property) => {
    let other_values: FxHashSet<_> = get_property_values(...).into_iter().collect();
    let value_set: FxHashSet<_> = value_nodes.iter().cloned().collect();

    if value_set != other_values { ... }
}
```

**Cost:** O(V1 + V2) where V1, V2 = value counts
- Acceptable for small value sets
- Problematic when properties have 100s of values

---

#### **2. Pattern Matching on Long Strings**
```rust
// validator.rs:396-418
Constraint::Pattern { pattern, flags } => {
    let regex = context.get_or_compile_regex(pattern, flags.as_deref())?;
    for value in value_nodes {
        if !regex.is_match(&str_value) { ... }
    }
}
```

**Cost:** O(V × S) where S = string length
- Regex matching can be expensive
- Pathological regexes exist (ReDoS)

**Mitigation:** No timeout on regex matching

---

#### **3. Class Hierarchy Traversal**
```rust
// model.rs:144-159
fn get_class_hierarchy(graph: &Graph, class: &NamedNode) -> Vec<Term> {
    let mut classes = vec![Term::NamedNode(class.clone())];
    let mut to_check: Vec<Term> = vec![Term::NamedNode(class.clone())];

    while let Some(current) = to_check.pop() {
        for subclass in graph.subjects_for_predicate_object(rdfs::SUB_CLASS_OF, current.as_ref()) {
            // Recursively find all subclasses
        }
    }
}
```

**Cost:** O(H × N) where H = hierarchy depth, N = subclasses
- Called for every `sh:targetClass`
- Deep ontologies = expensive

---

### ✅ **Safe Patterns**

1. **Simple property paths** - Direct, indexed access
2. **sh:datatype, sh:nodeKind** - Fast type checks
3. **sh:minCount, sh:maxCount** - Just count values
4. **sh:minLength, sh:maxLength** - String length check
5. **sh:minInclusive, sh:maxInclusive** - Numeric comparison
6. **sh:in with small lists** - HashSet lookup

---

## Security Issues

### ✅ **Protected:**
1. ✅ Circular list detection (model.rs:855)
2. ✅ List length limit (MAX_LIST_LENGTH = 10000)
3. ✅ Recursion depth limit (MAX_RECURSION_DEPTH = 50)
4. ✅ Path depth limit (MAX_DEPTH = 100)
5. ✅ Negative value rejection (minCount, maxCount, minLength, maxLength)

### ⚠️ **Unprotected:**
1. ⚠️ **No validation timeout**: Long-running validations never abort
2. ⚠️ **No memory limits**: Large result sets can exhaust memory
3. ⚠️ **No ReDoS protection**: sh:pattern accepts arbitrary regexes
4. ⚠️ **No cost estimation**: Can't predict if shape is safe before running

---

## Production Readiness Verdict

### **Overall: NOT READY for Production Admission Control**

| Requirement | Status | Score |
|-------------|--------|-------|
| Gate writes before ingest | ❌ FAIL | L1 |
| Incremental revalidation | ❌ FAIL | L1 |
| Safe shape evolution | ❌ FAIL | L1 |
| Terminate pathological shapes | ⚠️ PARTIAL | L3 |
| Clear diagnostic messages | ✅ PASS | L5 |

---

### **What Works:**
✅ **Excellent for batch validation** - Validate static datasets
✅ **Great for development** - Catch errors during development
✅ **Perfect for CI/CD** - Validate RDF files in test pipelines
✅ **Best-in-class diagnostics** - Production-quality error messages

### **What Doesn't Work:**
❌ **Write-time admission control** - Cannot gate database writes
❌ **Continuous validation** - No incremental algorithms
❌ **High-throughput workloads** - Full revalidation too expensive
❌ **Live shape updates** - Immutable validators

---

## Recommendations

### **For the Oxigraph Team:**

To make SHACL production-ready for admission control:

1. **Add Store Integration (P0)**
   ```rust
   impl Store {
       pub fn set_shacl_validator(&mut self, validator: ShaclValidator) -> Result<()> { ... }

       // In transaction commit:
       fn commit_with_validation(&self, quads: &[Quad]) -> Result<()> {
           if let Some(validator) = &self.validator {
               let delta = Graph::from_quads(quads);
               let report = validator.validate_delta(&delta)?;
               if !report.conforms() {
                   return Err(ValidationError::ShaclViolation(report));
               }
           }
           self.commit_internal(quads)
       }
   }
   ```

2. **Implement Incremental Validation (P0)**
   - Track which shapes apply to which subjects
   - On write, only revalidate affected shapes
   - Cache validation results, invalidate on change

3. **Add Timeout/Cost Limits (P1)**
   ```rust
   pub struct ValidationConfig {
       max_duration: Duration,
       max_operations: usize,
       max_result_size: usize,
   }
   ```

4. **Shape Safety Checker (P1)**
   - Analyze shapes before deployment
   - Detect unsafe patterns (inverse complex paths, nested ZeroOrMore)
   - Provide cost estimates

5. **Hot-Reload Validators (P2)**
   - Allow updating shapes without downtime
   - Versioned shape graphs
   - Gradual rollout

---

### **For Production Users:**

**DO:**
- ✅ Use SHACL for batch validation
- ✅ Validate RDF files before loading
- ✅ Use in development/testing pipelines
- ✅ Rely on excellent error messages

**DON'T:**
- ❌ Use for real-time write validation
- ❌ Assume validation scales with data size
- ❌ Deploy unsafe shape patterns
- ❌ Update shapes under production load

**WORKAROUND for Admission Control:**
```javascript
// Pseudo-code for application-level validation

async function insertWithValidation(store, quads, validator) {
    // 1. Validate BEFORE write
    const tempGraph = buildGraph(quads);
    const report = validator.validate(tempGraph);

    if (!report.conforms()) {
        throw new ValidationError(report);
    }

    // 2. Only then write to store
    await store.insertQuads(quads);

    // Limitation: Race condition if concurrent writes!
}
```

This is **NOT safe** for concurrent writes but may be acceptable for single-writer scenarios.

---

## Conclusion

Oxigraph's SHACL implementation is **well-crafted library code** with excellent error reporting and good recursion protections. However, it is fundamentally **architected for post-hoc validation**, not admission control.

**For production use requiring write-time validation:**
- **Current state: NOT READY (L2-L3)**
- **Required: L4+ with store integration and incremental validation**

**Estimated effort to production-ready admission control:**
- Store integration: 2-3 weeks
- Incremental validation: 4-6 weeks
- Safety analysis: 1-2 weeks
- **Total: ~2-3 months of focused development**

Until then, SHACL in Oxigraph is **suitable for development and batch validation** but **NOT for gating production database writes**.

---

## Appendix: Test Coverage Analysis

Based on `/home/user/oxigraph/lib/sparshacl/tests/integration.rs`:

**Well-tested:**
- ✅ All core constraints (minCount, maxCount, datatype, class, etc.)
- ✅ Property paths (inverse, sequence, alternative, zeroOrMore, oneOrMore)
- ✅ Logical constraints (sh:and, sh:or, sh:xone, sh:not)
- ✅ Severity levels (Violation, Warning, Info)
- ✅ Security protections (circular lists, length limits, negative values)
- ✅ N3 formula validation (formulas[0].to_graph())

**Under-tested:**
- ⚠️ Performance with large datasets (no 1M triple tests)
- ⚠️ Pathological shape patterns (no adversarial tests)
- ⚠️ Concurrent validation (no threading tests)
- ⚠️ Memory limits (no OOM prevention tests)

**Missing:**
- ❌ Benchmarks for cost model validation
- ❌ Timeout/cancellation tests
- ❌ Incremental validation tests (doesn't exist)
- ❌ Store integration tests (doesn't exist)

---

**END OF ASSESSMENT**
