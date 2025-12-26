# ShEx Internal Representation Design

## Overview

This document describes the internal representation (IR) for ShEx (Shape Expressions) in Oxigraph, implemented in `lib/sparshex/src/model.rs`.

## Design Principles

1. **Fully Normalized Representation**: All shapes are stored in a canonical form
2. **Deterministic Serialization**: Same logical shape always produces same representation
3. **Clear Cycle Handling**: Explicit cycle detection for shape references
4. **Oxigraph Integration**: Built on `oxrdf` types (NamedNode, Literal, Term, etc.)
5. **SHACL Pattern Consistency**: Follows similar patterns as `sparshacl` for API consistency

## Core Types

### ShapeLabel

Unique identifier for shapes (IRI or blank node):

```rust
pub enum ShapeLabel {
    Iri(NamedNode),
    BNode(BlankNode),
}
```

**Design Decision**: Separate from `ShapeExpression` to allow for efficient indexing and reference resolution.

### ShapeExpression

Main shape type representing all possible shape expressions:

```rust
pub enum ShapeExpression {
    ShapeAnd(Vec<ShapeExpression>),      // Conjunction
    ShapeOr(Vec<ShapeExpression>),       // Disjunction
    ShapeNot(Box<ShapeExpression>),      // Negation
    NodeConstraint(NodeConstraint),       // Node validation
    Shape(Shape),                         // Triple constraints
    ShapeExternal,                        // External reference
    ShapeRef(ShapeLabel),                // Reference to another shape
}
```

**Key Features**:
- Recursive structure supporting complex compositions
- `collect_refs()` method for extracting all shape references (cycle detection)
- Clear separation between structural operators and constraints

### Shape

Concrete shape with triple constraints:

```rust
pub struct Shape {
    pub label: Option<ShapeLabel>,
    pub closed: bool,
    pub extra: Vec<NamedNode>,
    pub triple_constraints: Vec<TripleConstraint>,
    pub annotations: Vec<Annotation>,
}
```

**Design Decision**: 
- `closed` flag controls whether extra properties are allowed
- `extra` list specifies exceptions for closed shapes
- Supports multiple triple constraints (AND semantics)

### TripleConstraint

Constraint on triples with a specific predicate:

```rust
pub struct TripleConstraint {
    pub predicate: NamedNode,
    pub value_expr: Option<Box<ShapeExpression>>,
    pub cardinality: Cardinality,
    pub inverse: bool,
    pub annotations: Vec<Annotation>,
}
```

**Key Features**:
- Optional value expression (if None, any value allowed)
- Cardinality constraint (min/max occurrences)
- `inverse` flag for reverse property paths (object -> subject)
- Boxed `ShapeExpression` to break recursive type cycles

### Cardinality

Min/max occurrence constraints:

```rust
pub struct Cardinality {
    pub min: u32,
    pub max: Option<u32>,  // None = unbounded
}
```

**Constructors**:
- `exactly(n)` - exactly n occurrences
- `optional()` - 0 or 1 (?)
- `zero_or_more()` - 0+ (*)
- `one_or_more()` - 1+ (+)
- `new(min, max)` - custom range with validation

**Validation**: `new()` ensures `max >= min` if max is specified.

### NodeConstraint

Constraints on individual node values:

```rust
pub struct NodeConstraint {
    pub node_kind: Option<NodeKind>,
    pub datatype: Option<NamedNode>,
    pub string_facets: Vec<StringFacet>,
    pub numeric_facets: Vec<NumericFacet>,
    pub values: Vec<ValueSetValue>,
}
```

**Facet Types**:

**String Facets**:
- `MinLength(usize)` - minimum string length
- `MaxLength(usize)` - maximum string length
- `Pattern { pattern: String, flags: Option<String> }` - regex matching

**Numeric Facets**:
- `MinInclusive(NumericLiteral)` - >= constraint
- `MinExclusive(NumericLiteral)` - > constraint
- `MaxInclusive(NumericLiteral)` - <= constraint
- `MaxExclusive(NumericLiteral)` - < constraint
- `TotalDigits(u32)` - total number of digits
- `FractionDigits(u32)` - fractional digits

### NodeKind

Type constraint for nodes:

```rust
pub enum NodeKind {
    Iri,         // IRI node
    BNode,       // Blank node
    Literal,     // Literal value
    NonLiteral,  // IRI or blank node (not literal)
}
```

**Design Decision**: `NonLiteral` covers both IRIs and blank nodes, matching ShEx semantics.

### ValueSetValue

Value set constraints with stem matching:

```rust
pub enum ValueSetValue {
    ObjectValue(Term),
    IriStem(String),
    IriStemRange { stem: String, exclusions: Vec<ValueSetValue> },
    LiteralStem(String),
    LiteralStemRange { stem: String, exclusions: Vec<ValueSetValue> },
    LanguageStem(String),
    LanguageStemRange { stem: String, exclusions: Vec<ValueSetValue> },
}
```

**Key Features**:
- Exact value matching with `ObjectValue`
- Prefix matching with stems (e.g., "http://example.org/*")
- Exclusions for stem ranges (e.g., "all URIs starting with X except Y")
- Separate stems for IRIs, literals, and language tags

### ShapesSchema

Collection of shapes with validation support:

```rust
pub struct ShapesSchema {
    shapes: FxHashMap<ShapeLabel, Arc<ShapeExpression>>,
    start: Option<ShapeLabel>,
    imports: Vec<NamedNode>,
    all_labels: Vec<ShapeLabel>,
}
```

**Key Methods**:

```rust
// Add/retrieve shapes
pub fn add_shape(&mut self, label: ShapeLabel, expr: ShapeExpression)
pub fn get_shape(&self, label: &ShapeLabel) -> Option<&Arc<ShapeExpression>>

// Validation
pub fn validate_refs(&self) -> Result<(), ShexParseError>
pub fn detect_cycles(&self) -> Result<(), ShexParseError>

// Metadata
pub fn set_start(&mut self, label: ShapeLabel)
pub fn add_import(&mut self, import: NamedNode)
```

**Cycle Detection Algorithm**:
Uses depth-first search with a recursion stack to detect cycles in shape references:

```rust
fn detect_cycles_impl(
    &self,
    label: &ShapeLabel,
    visited: &mut FxHashSet<ShapeLabel>,
    rec_stack: &mut FxHashSet<ShapeLabel>,
) -> Result<(), ShexParseError>
```

- Maintains `visited` set for all processed nodes
- Maintains `rec_stack` for current DFS path
- Cycle detected when visiting a node already in `rec_stack`

## Memory Management

**Arc Usage**: 
- `ShapeExpression` instances stored in `Arc` for efficient sharing
- Allows multiple references without deep cloning
- Critical for recursive shapes and cross-references

**FxHashMap**:
- Uses `rustc-hash` for fast, non-cryptographic hashing
- Performance-optimized for compiler-like workloads (many small strings)

## Integration with Oxigraph

### Type Mapping

| ShEx Concept | Oxigraph Type |
|--------------|---------------|
| IRI | `NamedNode` |
| Blank Node | `BlankNode` |
| Literal | `Literal` |
| Term | `Term` (union type) |
| Graph | `Graph` (for validation) |

### Error Handling

All errors use `thiserror` for consistent error formatting:

```rust
#[derive(Debug, thiserror::Error)]
pub enum ShexParseError {
    #[error("Invalid shape expression for {shape}: {message}")]
    InvalidShape { shape: Box<Term>, message: String },
    // ... more variants
}
```

## Comparison with SHACL Model

| Aspect | ShEx (sparshex) | SHACL (sparshacl) |
|--------|-----------------|-------------------|
| **Shape ID** | `ShapeLabel` | `ShapeId` |
| **Main Type** | `ShapeExpression` (enum) | `NodeShape` / `PropertyShape` (structs) |
| **Constraints** | `TripleConstraint` | `PropertyShape` + `Constraint` |
| **Cardinality** | Built-in `Cardinality` type | `MinCount` / `MaxCount` constraints |
| **Cycles** | Explicit cycle detection | Handled during validation |
| **Schema** | `ShapesSchema` | `ShapesGraph` |

## Future Extensions

The design supports future extensions:

1. **Semantic Actions**: Can be added to `TripleConstraint` and `Shape`
2. **Custom Facets**: Extensible facet system in `NodeConstraint`
3. **External Schemas**: `ShapeExternal` ready for import mechanism
4. **Annotations**: Already supported for extensibility

## Examples

### Simple IRI Constraint

```rust
let constraint = NodeConstraint::with_node_kind(NodeKind::Iri);
let expr = ShapeExpression::NodeConstraint(constraint);
```

### Triple Constraint with Cardinality

```rust
let predicate = NamedNode::new("http://xmlns.com/foaf/0.1/name").unwrap();
let value_constraint = NodeConstraint::with_datatype(
    NamedNode::new("http://www.w3.org/2001/XMLSchema#string").unwrap()
);
let value_expr = ShapeExpression::NodeConstraint(value_constraint);

let tc = TripleConstraint::with_value_expr(predicate, value_expr)
    .with_cardinality(Cardinality::one_or_more());
```

### Shape Reference with Cycle Detection

```rust
let mut schema = ShapesSchema::new();
let label1 = ShapeLabel::Iri(NamedNode::new("http://example.org/Shape1").unwrap());
let label2 = ShapeLabel::Iri(NamedNode::new("http://example.org/Shape2").unwrap());

schema.add_shape(label1.clone(), ShapeExpression::ShapeRef(label2.clone()));
schema.add_shape(label2.clone(), ShapeExpression::ShapeRef(label1.clone()));

// This will detect the cycle
assert!(schema.detect_cycles().is_err());
```

## Testing

The model includes comprehensive unit tests:

- `test_cardinality_*` - Cardinality constructors and validation
- `test_shape_label_display` - Display formatting
- `test_shapes_schema_*` - Schema operations and validation
- `test_node_kind_matches` - Node kind matching logic

Run tests with:
```bash
cargo test -p sparshex --lib
```

## Performance Considerations

1. **Lazy Evaluation**: Shape references not resolved until validation
2. **Efficient Indexing**: O(1) shape lookup via `FxHashMap`
3. **Minimal Cloning**: `Arc` for shared ownership
4. **Cycle Detection**: O(V + E) where V = shapes, E = references

## Compliance

This internal representation is designed to support full ShEx 2.0 compliance:
- ✅ Node constraints (datatype, kind, facets)
- ✅ Triple constraints with cardinality
- ✅ Shape operators (AND, OR, NOT)
- ✅ Value sets with stems
- ✅ Closed shapes with extra properties
- ✅ Shape references and cycle detection
- ✅ Annotations
- 🔄 Semantic actions (structure ready, not implemented)
- 🔄 External schemas (structure ready, not implemented)
