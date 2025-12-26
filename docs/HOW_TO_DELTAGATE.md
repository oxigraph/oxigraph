# How-To Guide: ΔGate Operations

**Goal-oriented practical guides for working with ΔGate in Oxigraph**

This guide assumes you are familiar with RDF basics and Oxigraph. For introductory material, see the tutorials and reference documentation.

---

## Table of Contents

1. [How to Compute Δ Between Universe States](#how-to-compute-δ-between-universe-states)
2. [How to Validate Admissibility with SHACL](#how-to-validate-admissibility-with-shacl)
3. [How to Generate Receipts](#how-to-generate-receipts)
4. [How to Apply Atomic Mutations](#how-to-apply-atomic-mutations)
5. [How to Work with Scope Envelopes](#how-to-work-with-scope-envelopes)
6. [How to Verify Roundtrip Consistency](#how-to-verify-roundtrip-consistency)
7. [How to Serialize Capsules for Transport](#how-to-serialize-capsules-for-transport)
8. [How to Use ΔGate in the Browser](#how-to-use-δgate-in-the-browser)

---

## How to Compute Δ Between Universe States

**Goal:** Calculate the minimal set of changes (additions and removals) between two RDF dataset states.

### Rust

```rust
use oxrdf::{Dataset, QuadRef, NamedNodeRef, GraphNameRef};

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

### Python

```python
from pyoxigraph import Store, NamedNode, Quad, DefaultGraph

# State at version 1
v1 = Store()
ex1 = NamedNode("http://example.com/1")
v1.add(Quad(ex1, ex1, ex1, DefaultGraph()))

# State at version 2
v2 = Store()
ex2 = NamedNode("http://example.com/2")
v2.add(Quad(ex2, ex2, ex2, DefaultGraph()))

# Compute delta manually
delta_plus = [q for q in v2 if q not in v1]
delta_minus = [q for q in v1 if q not in v2]
```

### JavaScript

```javascript
import { Dataset, namedNode, quad, defaultGraph } from 'oxigraph';

// State at version 1
const v1 = new Dataset();
const ex1 = namedNode("http://example.com/1");
v1.add(quad(ex1, ex1, ex1, defaultGraph()));

// State at version 2
const v2 = new Dataset();
const ex2 = namedNode("http://example.com/2");
v2.add(quad(ex2, ex2, ex2, defaultGraph()));

// Compute delta using set operations
const deltaPlus = v2.difference(v1);   // Quads in v2 but not v1
const deltaMinus = v1.difference(v2);  // Quads in v1 but not v2
```

**When to use this:**
- Before applying changes to verify what will change
- For creating change logs or audit trails
- To compute minimal updates for synchronization

---

## How to Validate Admissibility with SHACL

**Goal:** Verify that a proposed change satisfies all constraints before admission.

### Rust

```rust
use sparshacl::{ShaclValidator, ShapesGraph};
use oxrdf::{Graph, Dataset};
use oxrdfio::{RdfFormat, RdfParser};

// 1. Load SHACL shapes (constraints Σ)
let shapes_turtle = r#"
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.com/> .

ex:PersonShape
    a sh:NodeShape ;
    sh:targetClass ex:Person ;
    sh:property [
        sh:path ex:name ;
        sh:minCount 1 ;
        sh:datatype xsd:string ;
    ] ;
    sh:property [
        sh:path ex:age ;
        sh:datatype xsd:integer ;
        sh:minInclusive 0 ;
    ] .
"#;

let mut shapes_graph = Graph::new();
RdfParser::from_format(RdfFormat::Turtle)
    .parse_read(shapes_turtle.as_bytes())
    .for_each(|triple| {
        shapes_graph.insert(&triple.unwrap());
    });

let shapes = ShapesGraph::from_graph(&shapes_graph)?;
let validator = ShaclValidator::new(shapes);

// 2. Apply proposed delta to current state
let mut proposed_state = current_state.clone();
proposed_state.apply_diff(&delta_plus, &delta_minus);

// 3. Validate admissibility
let report = validator.validate(&proposed_state)?;

if report.conforms() {
    println!("✓ Change is admissible");
    // Proceed with mutation
} else {
    println!("✗ Change violates constraints:");
    for result in report.results() {
        println!("  - Focus: {:?}", result.focus_node());
        println!("    Path: {:?}", result.result_path());
        println!("    Message: {:?}", result.message());
    }
    // Reject mutation
}
```

### Python

```python
from pyoxigraph import Store, parse

# 1. Load SHACL shapes
shapes_store = Store()
shapes_turtle = """
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.com/> .

ex:PersonShape
    a sh:NodeShape ;
    sh:targetClass ex:Person ;
    sh:property [
        sh:path ex:name ;
        sh:minCount 1 ;
    ] .
"""
for quad in parse(shapes_turtle, format="text/turtle"):
    shapes_store.add(quad)

# 2. Apply proposed delta
proposed_state = current_state.copy()
for quad in delta_plus:
    proposed_state.add(quad)
for quad in delta_minus:
    proposed_state.remove(quad)

# 3. Validate using SPARQL ASK queries (manual SHACL)
validation_query = """
PREFIX ex: <http://example.com/>
ASK {
    ?person a ex:Person .
    FILTER NOT EXISTS { ?person ex:name ?name }
}
"""
has_violations = proposed_state.query(validation_query)

if not has_violations:
    print("✓ Change is admissible")
else:
    print("✗ Change violates constraints")
```

### JavaScript

```javascript
import { ShaclValidator, ShaclShapesGraph } from 'oxigraph';

// 1. Load SHACL shapes
const shapes = new ShaclShapesGraph();
shapes.parse(`
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix ex: <http://example.com/> .

ex:PersonShape
    a sh:NodeShape ;
    sh:targetClass ex:Person ;
    sh:property [
        sh:path ex:name ;
        sh:minCount 1 ;
    ] .
`);

const validator = new ShaclValidator(shapes);

// 2. Apply proposed delta
const proposedState = currentState.clone();
proposedState.extend(deltaPlus);
for (const quad of deltaMinus) {
    proposedState.delete(quad);
}

// 3. Validate admissibility
const report = validator.validateStore(proposedState);

if (report.conforms) {
    console.log('✓ Change is admissible');
    // Proceed with mutation
} else {
    console.log('✗ Change violates constraints:');
    for (const result of report.results()) {
        console.log(`  - ${result.message}`);
    }
    // Reject mutation
}
```

**When to use this:**
- Before admitting any universe mutation
- To enforce typing constraints (O ⊨ Σ)
- To prevent invalid state transitions

---

## How to Generate Receipts

**Goal:** Create a cryptographic proof of admissibility binding state, constraints, and governance trace.

### Rust

```rust
use oxrdf::{Dataset, Literal, NamedNode};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Generate receipt for an admitted change
fn generate_receipt(
    o_before: &Dataset,
    o_after: &Dataset,
    delta_plus: &Dataset,
    delta_minus: &Dataset,
    validation_report: &ValidationReport,
) -> Dataset {
    let mut receipt = Dataset::new();
    let receipt_ns = "http://example.com/receipt/";

    // 1. State binding: hash(O_before), hash(O_after), hash(Δ)
    let hash_before = compute_hash(o_before);
    let hash_after = compute_hash(o_after);
    let hash_delta_plus = compute_hash(delta_plus);
    let hash_delta_minus = compute_hash(delta_minus);

    let receipt_id = NamedNode::new(format!("{}{}", receipt_ns, uuid::Uuid::new_v4())).unwrap();

    receipt.insert(QuadRef::new(
        &receipt_id,
        &NamedNode::new("http://example.com/hashBefore").unwrap(),
        &Literal::new_simple_literal(hash_before.to_string()),
        GraphNameRef::DefaultGraph
    ));

    receipt.insert(QuadRef::new(
        &receipt_id,
        &NamedNode::new("http://example.com/hashAfter").unwrap(),
        &Literal::new_simple_literal(hash_after.to_string()),
        GraphNameRef::DefaultGraph
    ));

    // 2. Proof outcomes: typing, idempotence, invariants
    receipt.insert(QuadRef::new(
        &receipt_id,
        &NamedNode::new("http://example.com/conforms").unwrap(),
        &Literal::new_simple_literal(validation_report.conforms().to_string()),
        GraphNameRef::DefaultGraph
    ));

    // 3. Governance trace (simplified)
    receipt.insert(QuadRef::new(
        &receipt_id,
        &NamedNode::new("http://example.com/timestamp").unwrap(),
        &Literal::new_typed_literal(
            chrono::Utc::now().to_rfc3339(),
            NamedNode::new("http://www.w3.org/2001/XMLSchema#dateTime").unwrap()
        ),
        GraphNameRef::DefaultGraph
    ));

    // 4. Reversibility: include delta for rollback
    let delta_graph = NamedNode::new(format!("{}delta", receipt_ns)).unwrap();
    for quad in delta_plus.iter() {
        receipt.insert(quad.in_graph(&delta_graph));
    }

    receipt
}

fn compute_hash(dataset: &Dataset) -> u64 {
    let mut hasher = DefaultHasher::new();
    // Iteration order is guaranteed stable (BTreeSet)
    for quad in dataset.iter() {
        quad.hash(&mut hasher);
    }
    hasher.finish()
}
```

### Python

```python
import hashlib
from datetime import datetime
from pyoxigraph import Store, NamedNode, Literal, Quad, DefaultGraph

def generate_receipt(o_before, o_after, delta_plus, delta_minus, conforms):
    """Generate receipt for an admitted change"""
    receipt = Store()
    receipt_ns = "http://example.com/receipt/"

    # 1. State binding
    hash_before = compute_hash(o_before)
    hash_after = compute_hash(o_after)

    receipt_id = NamedNode(f"{receipt_ns}{uuid.uuid4()}")

    receipt.add(Quad(
        receipt_id,
        NamedNode("http://example.com/hashBefore"),
        Literal(hash_before),
        DefaultGraph()
    ))

    receipt.add(Quad(
        receipt_id,
        NamedNode("http://example.com/hashAfter"),
        Literal(hash_after),
        DefaultGraph()
    ))

    # 2. Proof outcomes
    receipt.add(Quad(
        receipt_id,
        NamedNode("http://example.com/conforms"),
        Literal(str(conforms)),
        DefaultGraph()
    ))

    # 3. Governance trace
    receipt.add(Quad(
        receipt_id,
        NamedNode("http://example.com/timestamp"),
        Literal(datetime.utcnow().isoformat(),
                datatype=NamedNode("http://www.w3.org/2001/XMLSchema#dateTime")),
        DefaultGraph()
    ))

    return receipt

def compute_hash(store):
    """Compute deterministic hash of store contents"""
    h = hashlib.sha256()
    # Sort quads for deterministic hashing
    quads = sorted(str(q) for q in store)
    for quad_str in quads:
        h.update(quad_str.encode())
    return h.hexdigest()
```

### JavaScript

```javascript
import { Dataset, namedNode, literal, quad, defaultGraph } from 'oxigraph';
import { v4 as uuidv4 } from 'uuid';

function generateReceipt(oBefore, oAfter, deltaPlus, deltaMinus, conforms) {
    const receipt = new Dataset();
    const receiptNs = "http://example.com/receipt/";

    // 1. State binding
    const hashBefore = computeHash(oBefore);
    const hashAfter = computeHash(oAfter);

    const receiptId = namedNode(`${receiptNs}${uuidv4()}`);

    receipt.add(quad(
        receiptId,
        namedNode("http://example.com/hashBefore"),
        literal(hashBefore),
        defaultGraph()
    ));

    receipt.add(quad(
        receiptId,
        namedNode("http://example.com/hashAfter"),
        literal(hashAfter),
        defaultGraph()
    ));

    // 2. Proof outcomes
    receipt.add(quad(
        receiptId,
        namedNode("http://example.com/conforms"),
        literal(String(conforms)),
        defaultGraph()
    ));

    // 3. Governance trace
    receipt.add(quad(
        receiptId,
        namedNode("http://example.com/timestamp"),
        literal(new Date().toISOString(), {
            datatype: namedNode("http://www.w3.org/2001/XMLSchema#dateTime")
        }),
        defaultGraph()
    ));

    return receipt;
}

async function computeHash(dataset) {
    // Sort quads for deterministic hashing
    const quads = Array.from(dataset).sort((a, b) =>
        a.toString().localeCompare(b.toString())
    );

    const encoder = new TextEncoder();
    const data = encoder.encode(quads.map(q => q.toString()).join('\n'));

    const hashBuffer = await crypto.subtle.digest('SHA-256', data);
    const hashArray = Array.from(new Uint8Array(hashBuffer));
    return hashArray.map(b => b.toString(16).padStart(2, '0')).join('');
}
```

**When to use this:**
- After successfully validating and applying a change
- To create audit trails
- To enable deterministic rollback
- For governance compliance

---

## How to Apply Atomic Mutations

**Goal:** Apply validated changes to the universe with all-or-nothing guarantees.

### Rust

```rust
use oxrdf::Dataset;

// Atomic application of delta
fn apply_atomic_mutation(
    universe: &mut Dataset,
    delta_plus: &Dataset,
    delta_minus: &Dataset,
) -> Result<(), String> {
    // Transaction-like behavior (all-or-nothing)

    // 1. Remove quads
    for quad in delta_minus.iter() {
        if !universe.remove(&quad) {
            return Err(format!("Failed to remove quad: {:?}", quad));
        }
    }

    // 2. Add quads
    for quad in delta_plus.iter() {
        if !universe.insert(&quad) {
            // Rollback removals if addition fails
            for quad in delta_minus.iter() {
                universe.insert(&quad);
            }
            return Err(format!("Failed to add quad: {:?}", quad));
        }
    }

    Ok(())
}
```

### Python

```python
from pyoxigraph import Store

def apply_atomic_mutation(universe, delta_plus, delta_minus):
    """Apply delta with atomic guarantees"""

    # Create snapshot for potential rollback
    snapshot = [quad for quad in universe]

    try:
        # 1. Remove quads
        for quad in delta_minus:
            universe.remove(quad)

        # 2. Add quads
        for quad in delta_plus:
            universe.add(quad)

        return True

    except Exception as e:
        # Rollback on failure
        universe.clear()
        for quad in snapshot:
            universe.add(quad)
        raise Exception(f"Atomic mutation failed: {e}")
```

### JavaScript

```javascript
import { Store } from 'oxigraph';

async function applyAtomicMutation(store, deltaPlus, deltaMinus) {
    // Option 1: Use SPARQL UPDATE for atomicity
    const deleteData = Array.from(deltaMinus)
        .map(q => q.toString())
        .join(' .\n    ');

    const insertData = Array.from(deltaPlus)
        .map(q => q.toString())
        .join(' .\n    ');

    await store.updateAsync(`
        DELETE DATA {
            ${deleteData}
        }
        INSERT DATA {
            ${insertData}
        }
    `);

    // Option 2: Use extend() for bulk atomic insert
    // store.extend(deltaPlus);
    // Note: No bulk delete, must iterate
}
```

**When to use this:**
- After successful admissibility validation
- When applying receipt-gated mutations
- To ensure universe consistency

---

## How to Work with Scope Envelopes

**Goal:** Extract and analyze the subgraph affected by a change (Cover(O)).

### Rust

```rust
use oxrdf::{Dataset, NamedNode, NamedNodeRef};

fn compute_scope_envelope(
    dataset: &Dataset,
    focus_subject: &NamedNodeRef,
) -> Dataset {
    let mut envelope = Dataset::new();

    // Get all quads with focus subject
    for quad in dataset.quads_for_subject(focus_subject) {
        envelope.insert(&quad);

        // Recursively expand to objects (1-hop)
        if let Term::NamedNode(obj) = quad.object {
            for quad2 in dataset.quads_for_subject(obj.as_ref()) {
                envelope.insert(&quad2);
            }
        }
    }

    envelope
}

// Example: Track what changes in a scope
fn scope_delta(
    before: &Dataset,
    after: &Dataset,
    focus: &NamedNodeRef,
) -> (Dataset, Dataset) {
    let before_scope = compute_scope_envelope(before, focus);
    let after_scope = compute_scope_envelope(after, focus);

    before_scope.diff(&after_scope)
}
```

### Python

```python
def compute_scope_envelope(store, focus_subject):
    """Extract scope envelope around a focus node"""
    from pyoxigraph import Store

    envelope = Store()

    # Get all quads with focus subject
    for quad in store.quads_for_subject(focus_subject):
        envelope.add(quad)

        # Expand to objects (1-hop)
        if hasattr(quad.object, 'value'):  # NamedNode
            for quad2 in store.quads_for_subject(quad.object):
                envelope.add(quad2)

    return envelope
```

### JavaScript

```javascript
function computeScopeEnvelope(dataset, focusSubject) {
    const envelope = new Dataset();

    // Get all quads with focus subject
    for (const quad of dataset.quadsForSubject(focusSubject)) {
        envelope.add(quad);

        // Expand to objects (1-hop)
        if (quad.object.termType === 'NamedNode') {
            for (const quad2 of dataset.quadsForSubject(quad.object)) {
                envelope.add(quad2);
            }
        }
    }

    return envelope;
}
```

**When to use this:**
- To declare the maximal affected envelope of a change
- To scope validation to relevant subgraphs
- For targeted reconciliation

---

## How to Verify Roundtrip Consistency

**Goal:** Ensure that applying a delta and its inverse returns to the original state.

### Rust

```rust
fn verify_roundtrip(
    original: &Dataset,
    delta_plus: &Dataset,
    delta_minus: &Dataset,
) -> bool {
    let mut modified = original.clone();

    // Apply forward delta
    modified.apply_diff(delta_plus, delta_minus);

    // Apply reverse delta
    modified.apply_diff(delta_minus, delta_plus);

    // Should equal original
    original == &modified
}

#[test]
fn test_delta_roundtrip() {
    let original = Dataset::new();
    // ... populate original ...

    let modified = Dataset::new();
    // ... populate modified ...

    let (delta_plus, delta_minus) = original.diff(&modified);

    assert!(verify_roundtrip(&original, &delta_plus, &delta_minus));
}
```

**When to use this:**
- To verify delta computation correctness
- Before generating rollback capsules
- In test suites for delta operations

---

## How to Serialize Capsules for Transport

**Goal:** Package a delta with metadata for network transmission.

### Rust

```rust
use oxrdfio::{RdfFormat, RdfSerializer};

fn serialize_capsule(
    delta_plus: &Dataset,
    delta_minus: &Dataset,
    metadata: &Dataset,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut capsule = Dataset::new();

    // Use named graphs to separate components
    let delta_plus_graph = NamedNode::new("urn:deltagate:delta:plus")?;
    let delta_minus_graph = NamedNode::new("urn:deltagate:delta:minus")?;
    let metadata_graph = NamedNode::new("urn:deltagate:metadata")?;

    // Add deltas to respective graphs
    for quad in delta_plus.iter() {
        capsule.insert(&quad.in_graph(&delta_plus_graph));
    }

    for quad in delta_minus.iter() {
        capsule.insert(&quad.in_graph(&delta_minus_graph));
    }

    for quad in metadata.iter() {
        capsule.insert(&quad.in_graph(&metadata_graph));
    }

    // Serialize as TriG (supports named graphs)
    let mut buffer = Vec::new();
    let mut serializer = RdfSerializer::from_format(RdfFormat::TriG)
        .serialize_to_write(&mut buffer);

    for quad in capsule.iter() {
        serializer.serialize_quad(&quad)?;
    }
    serializer.finish()?;

    Ok(String::from_utf8(buffer)?)
}
```

### JavaScript

```javascript
import { serializeAsync, RdfFormat } from 'oxigraph';

async function serializeCapsule(deltaPlus, deltaMinus, metadata) {
    const capsule = new Dataset();

    // Use named graphs
    const deltaPlusGraph = namedNode("urn:deltagate:delta:plus");
    const deltaMinusGraph = namedNode("urn:deltagate:delta:minus");
    const metadataGraph = namedNode("urn:deltagate:metadata");

    // Add deltas to respective graphs
    for (const q of deltaPlus) {
        capsule.add(quad(q.subject, q.predicate, q.object, deltaPlusGraph));
    }

    for (const q of deltaMinus) {
        capsule.add(quad(q.subject, q.predicate, q.object, deltaMinusGraph));
    }

    for (const q of metadata) {
        capsule.add(quad(q.subject, q.predicate, q.object, metadataGraph));
    }

    // Serialize as TriG
    return await serializeAsync(capsule, RdfFormat.TRIG);
}
```

**When to use this:**
- For network transmission of capsules
- For persistent storage of change history
- For integration with external systems

---

## How to Use ΔGate in the Browser

**Goal:** Implement ΔGate operations in browser environments using WASM.

### Complete Browser Example

```html
<!DOCTYPE html>
<html>
<head>
    <title>ΔGate Browser Demo</title>
</head>
<body>
    <h1>ΔGate Browser Operations</h1>
    <div id="output"></div>

    <script type="module">
        import init, * as oxigraph from './node_modules/oxigraph/web.js';

        (async function() {
            await init(); // Initialize WASM

            const output = document.getElementById('output');

            // 1. Create universe states
            const v1 = new oxigraph.Dataset();
            const ex1 = oxigraph.namedNode("http://example.com/1");
            v1.add(oxigraph.quad(ex1, ex1, ex1, oxigraph.defaultGraph()));

            const v2 = new oxigraph.Dataset();
            const ex2 = oxigraph.namedNode("http://example.com/2");
            v2.add(oxigraph.quad(ex2, ex2, ex2, oxigraph.defaultGraph()));

            // 2. Compute delta
            const deltaPlus = v2.difference(v1);
            const deltaMinus = v1.difference(v2);

            output.innerHTML += `<p>Δ⁺: ${deltaPlus.size} additions</p>`;
            output.innerHTML += `<p>Δ⁻: ${deltaMinus.size} removals</p>`;

            // 3. Validate with SHACL
            const shapes = new oxigraph.ShaclShapesGraph();
            shapes.parse(`
                @prefix sh: <http://www.w3.org/ns/shacl#> .
                @prefix ex: <http://example.com/> .

                ex:Shape a sh:NodeShape ;
                    sh:targetNode ex:2 ;
                    sh:property [
                        sh:path ex:2 ;
                        sh:minCount 1 ;
                    ] .
            `);

            const validator = new oxigraph.ShaclValidator(shapes);

            // Create store from dataset for validation
            const store = new oxigraph.Store();
            store.extend(v2);

            const report = validator.validateStore(store);

            if (report.conforms) {
                output.innerHTML += '<p style="color: green;">✓ Valid</p>';

                // 4. Apply atomically
                const universe = new oxigraph.Store();
                universe.extend(v1);

                // Atomic update via SPARQL
                await universe.updateAsync(`
                    DELETE DATA { ${Array.from(deltaMinus).join(' .\n')} }
                    INSERT DATA { ${Array.from(deltaPlus).join(' .\n')} }
                `);

                output.innerHTML += `<p>Universe now has ${universe.size} quads</p>`;
            } else {
                output.innerHTML += '<p style="color: red;">✗ Invalid</p>';
            }
        })();
    </script>
</body>
</html>
```

**When to use this:**
- For client-side RDF validation
- For offline-capable applications
- For browser-based knowledge graph tools

---

## Best Practices

### 1. Always Validate Before Applying
Never apply a delta without validating admissibility first.

### 2. Use Deterministic Iteration for Hashing
Oxrdf's BTreeSet-based storage ensures consistent iteration order.

### 3. Scope Deltas to Minimal Changes
Compute the smallest delta possible to minimize reconciliation overhead.

### 4. Generate Receipts for All Mutations
Every admitted change should have a corresponding receipt for audit.

### 5. Test Roundtrip Consistency
Always verify that delta application is reversible when expected.

### 6. Use Async APIs in Browsers
Prevent UI freezing by using async operations for large datasets.

---

## Troubleshooting

### Delta Computation Returns Empty Sets
- **Cause**: States are identical
- **Solution**: Verify inputs differ, check blank node handling

### SHACL Validation Always Fails
- **Cause**: Shapes graph malformed or targets incorrect
- **Solution**: Validate shapes graph syntax, verify sh:targetClass

### Atomic Mutation Partially Applies
- **Cause**: Individual operations not wrapped in transaction
- **Solution**: Use SPARQL UPDATE or implement rollback logic

### Hash Computation Inconsistent
- **Cause**: Non-deterministic iteration
- **Solution**: Ensure using BTreeSet-based Dataset, not custom implementation

---

## See Also

- [ΔGate Overview](/home/user/oxigraph/docs/DELTAGATE_OVERVIEW.md) - Conceptual foundation
- [ΔGate Implementation Guide](/home/user/oxigraph/lib/oxrdf/DELTAGATE.md) - Technical reference
- [SHACL Specification](https://www.w3.org/TR/shacl/) - Constraint language details
- [RDF 1.1 Concepts](https://www.w3.org/TR/rdf11-concepts/) - RDF fundamentals
