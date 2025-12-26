# ShEx API Documentation

This document describes the public API surface for ShEx (Shape Expressions) validation in Oxigraph across Rust, JavaScript/WASM, and Python bindings.

## Design Principles

1. **Consistency with SHACL**: The API mirrors the SHACL validation API for familiarity
2. **Minimal Surface**: Only essential types and functions are exposed
3. **Zero-cost Abstractions**: Rust API leverages ownership and borrowing
4. **Ergonomic Bindings**: JS and Python APIs feel native to their ecosystems
5. **Stability Guarantees**: Public API follows semantic versioning

---

## Rust API (`sparshex` crate)

### Core Types

#### `ShapesSchema`

The main container for ShEx shape definitions.

```rust
pub struct ShapesSchema { /* ... */ }

impl ShapesSchema {
    /// Creates a new empty schema.
    pub fn new() -> Self;

    /// Parses a ShEx schema from a string in ShExC (compact) syntax.
    pub fn parse(input: &str) -> Result<Self, ShexParseError>;

    /// Parses a ShEx schema from JSON format.
    pub fn from_json(input: &str) -> Result<Self, ShexParseError>;

    /// Creates a schema from an RDF graph (Turtle format).
    pub fn from_graph(graph: &Graph) -> Result<Self, ShexParseError>;

    /// Returns the number of shapes in the schema.
    pub fn len(&self) -> usize;

    /// Returns true if the schema contains no shapes.
    pub fn is_empty(&self) -> bool;

    /// Gets a shape by its identifier.
    pub fn get_shape(&self, id: &ShapeId) -> Option<&ShapeExpression>;

    /// Returns an iterator over all shapes.
    pub fn shapes(&self) -> impl Iterator<Item = (&ShapeId, &ShapeExpression)>;
}
```

#### `ShexValidator`

The validator that checks RDF graphs against ShEx schemas.

```rust
pub struct ShexValidator { /* ... */ }

impl ShexValidator {
    /// Creates a new validator for the given schema.
    pub fn new(schema: ShapesSchema) -> Self;

    /// Validates an entire graph against all target declarations.
    pub fn validate(&self, graph: &Graph) -> Result<ValidationReport, ShexValidationError>;

    /// Validates a specific node against a specific shape.
    pub fn validate_node(
        &self,
        graph: &Graph,
        focus: impl Into<Subject>,
        shape: &ShapeId,
    ) -> Result<ValidationReport, ShexValidationError>;

    /// Validates multiple nodes against their respective shapes.
    pub fn validate_shape_map(
        &self,
        graph: &Graph,
        shape_map: &[(Subject, ShapeId)],
    ) -> Result<ValidationReport, ShexValidationError>;
}
```

#### `ValidationReport`

The result of a validation operation.

```rust
pub struct ValidationReport { /* ... */ }

impl ValidationReport {
    /// Returns true if all validations passed (no failures).
    pub fn conforms(&self) -> bool;

    /// Returns the total number of results.
    pub fn result_count(&self) -> usize;

    /// Returns the number of failures.
    pub fn failure_count(&self) -> usize;

    /// Returns all validation results.
    pub fn results(&self) -> &[ValidationResult];

    /// Converts the report to an RDF graph representation.
    pub fn to_graph(&self) -> Graph;

    /// Serializes the report to Turtle format.
    pub fn to_turtle(&self) -> Result<String, std::io::Error>;
}
```

#### `ValidationResult`

A single validation result for a node-shape pair.

```rust
pub struct ValidationResult {
    /// The node that was validated.
    pub focus: Subject,

    /// The shape it was validated against.
    pub shape: ShapeId,

    /// Whether the validation passed.
    pub status: ValidationStatus,

    /// Human-readable reason for failure (if any).
    pub reason: Option<String>,

    /// The specific constraint that failed (if any).
    pub failing_constraint: Option<Box<dyn std::fmt::Debug>>,
}

pub enum ValidationStatus {
    /// Validation passed
    Conformant,
    /// Validation failed
    NonConformant,
}
```

### Shape Model Types

#### `ShapeExpression`

The core shape constraint type.

```rust
pub enum ShapeExpression {
    /// A node constraint (datatype, value restrictions)
    NodeConstraint(NodeConstraint),

    /// A shape definition (triple expressions)
    Shape {
        id: Option<ShapeId>,
        closed: bool,
        extra: Vec<NamedNode>,
        expression: Option<TripleExpression>,
    },

    /// Conjunction of shapes
    And(ShapeAnd),

    /// Disjunction of shapes
    Or(ShapeOr),

    /// Negation of a shape
    Not(ShapeNot),

    /// Reference to another shape
    ShapeRef(ShapeRef),

    /// External shape (delegated validation)
    External,
}
```

#### `NodeConstraint`

Constraints on individual RDF nodes.

```rust
pub struct NodeConstraint {
    /// Required datatype
    pub datatype: Option<NamedNode>,

    /// Allowed values
    pub values: Vec<Term>,

    /// Minimum value (for numerics)
    pub min_value: Option<Literal>,

    /// Maximum value (for numerics)
    pub max_value: Option<Literal>,

    /// String pattern (regex)
    pub pattern: Option<String>,

    /// String length constraints
    pub min_length: Option<u32>,
    pub max_length: Option<u32>,
}
```

#### `TripleExpression`

Constraints on triple patterns.

```rust
pub enum TripleExpression {
    /// A single triple constraint
    TripleConstraint(TripleConstraint),

    /// Conjunction of expressions
    EachOf(Vec<TripleExpression>),

    /// Disjunction of expressions
    OneOf(Vec<TripleExpression>),
}
```

#### `TripleConstraint`

A constraint on a single triple pattern.

```rust
pub struct TripleConstraint {
    /// The predicate
    pub predicate: NamedNode,

    /// The value constraint
    pub value_expr: Option<ShapeExpression>,

    /// Cardinality constraints
    pub min: Option<u32>,
    pub max: Option<u32>, // None means unbounded

    /// Whether the predicate is inverse
    pub inverse: bool,
}
```

#### `ShapeId`

Identifier for a shape definition.

```rust
pub enum ShapeId {
    /// Named shape (IRI)
    Iri(NamedNode),

    /// Anonymous shape (blank node)
    BNode(BlankNode),
}
```

### Error Types

```rust
/// Errors that can occur when working with ShEx.
#[derive(Debug, thiserror::Error)]
pub enum ShexError {
    #[error(transparent)]
    Parse(#[from] ShexParseError),

    #[error(transparent)]
    Validation(#[from] ShexValidationError),
}

/// Errors that occur during schema parsing.
#[derive(Debug, thiserror::Error)]
pub enum ShexParseError {
    #[error("Syntax error: {0}")]
    Syntax(String),

    #[error("Invalid shape reference: {0}")]
    InvalidReference(String),

    #[error("Circular shape dependency: {0}")]
    CircularDependency(String),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Errors that occur during validation.
#[derive(Debug, thiserror::Error)]
pub enum ShexValidationError {
    #[error("Shape not found: {0}")]
    ShapeNotFound(String),

    #[error("Validation failed: {0}")]
    Failed(String),

    #[error("Internal error: {0}")]
    Internal(String),
}
```

### Convenience Functions

```rust
/// Parses a ShEx schema from a string.
pub fn parse_shex(input: &str) -> Result<ShapesSchema, ShexParseError>;

/// Validates data against a schema (convenience function).
pub fn validate(
    schema: &ShapesSchema,
    graph: &Graph,
) -> Result<ValidationReport, ShexValidationError>;
```

---

## JavaScript/WASM API (`oxigraph` npm package)

### Module Export

```typescript
import {
    ShexShapesSchema,
    ShexValidator,
    ShexValidationReport,
    ShexValidationResult,
    shexValidate,
} from 'oxigraph';
```

### `ShexShapesSchema`

```typescript
/**
 * A ShEx shapes schema for validation.
 */
export class ShexShapesSchema {
    /**
     * Creates a new empty shapes schema.
     */
    constructor();

    /**
     * Parses shapes from a string in ShExC (compact) format.
     *
     * @param data - The ShExC-formatted schema data
     * @throws {Error} If the data cannot be parsed
     */
    parse(data: string): void;

    /**
     * Parses shapes from JSON format.
     *
     * @param data - The JSON-formatted schema data
     * @throws {Error} If the data cannot be parsed
     */
    parseJson(data: string): void;

    /**
     * The number of shapes in the schema.
     */
    readonly size: number;

    /**
     * Returns true if the schema is empty.
     */
    isEmpty(): boolean;
}
```

### `ShexValidator`

```typescript
/**
 * A ShEx validator for validating RDF data against shapes.
 */
export class ShexValidator {
    /**
     * Creates a new validator with the given shapes schema.
     *
     * @param schema - The shapes schema to validate against
     */
    constructor(schema: ShexShapesSchema);

    /**
     * Validates data against the shapes schema.
     *
     * @param data - The data to validate (as Turtle string)
     * @returns A validation report
     * @throws {Error} If the data cannot be parsed or validation fails
     */
    validate(data: string): ShexValidationReport;

    /**
     * Validates a Store object against the shapes schema.
     *
     * @param store - The Store to validate
     * @returns A validation report
     * @throws {Error} If validation fails
     */
    validateStore(store: Store): ShexValidationReport;

    /**
     * Validates a specific node against a specific shape.
     *
     * @param store - The Store containing the data
     * @param focus - The node to validate
     * @param shape - The shape identifier (as IRI string)
     * @returns A validation report
     * @throws {Error} If validation fails
     */
    validateNode(store: Store, focus: Term, shape: string): ShexValidationReport;
}
```

### `ShexValidationReport`

```typescript
/**
 * A ShEx validation report.
 *
 * Contains the results of validating data against a shapes schema.
 */
export class ShexValidationReport {
    /**
     * Whether the data conforms to the shapes schema.
     * Returns true if there are no failures.
     */
    readonly conforms: boolean;

    /**
     * The total number of results.
     */
    readonly resultCount: number;

    /**
     * The number of failures.
     */
    readonly failureCount: number;

    /**
     * Returns the validation results as an array.
     */
    results(): ShexValidationResult[];

    /**
     * Returns the report as a Turtle string.
     */
    toTurtle(): string;
}
```

### `ShexValidationResult`

```typescript
/**
 * A single ShEx validation result.
 */
export class ShexValidationResult {
    /**
     * The focus node that was validated.
     */
    readonly focus: Term;

    /**
     * The shape it was validated against.
     */
    readonly shape: string;

    /**
     * Whether the validation passed.
     */
    readonly conformant: boolean;

    /**
     * The reason for failure (if any).
     */
    readonly reason?: string;
}
```

### Convenience Function

```typescript
/**
 * Validates RDF data against ShEx shapes (convenience function).
 *
 * @param schemaData - The shapes schema as a ShExC string
 * @param data - The data to validate as a Turtle string
 * @returns A validation report
 * @throws {Error} If the data cannot be parsed or validation fails
 */
export function shexValidate(
    schemaData: string,
    data: string
): ShexValidationReport;
```

### Usage Example

```javascript
import { ShexShapesSchema, ShexValidator } from 'oxigraph';

// Parse schema
const schema = new ShexShapesSchema();
schema.parse(`
    PREFIX ex: <http://example.org/>
    PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

    ex:PersonShape {
        ex:name xsd:string ;
        ex:age xsd:integer
    }
`);

// Create validator
const validator = new ShexValidator(schema);

// Validate data
const report = validator.validate(`
    PREFIX ex: <http://example.org/>

    ex:alice ex:name "Alice" ;
              ex:age 30 .
`);

if (report.conforms) {
    console.log("Valid!");
} else {
    for (const result of report.results()) {
        if (!result.conformant) {
            console.error(`Failure: ${result.reason}`);
        }
    }
}
```

---

## Python API (`pyoxigraph` package)

### Module Import

```python
from pyoxigraph import (
    ShexShapesSchema,
    ShexValidator,
    ShexValidationReport,
    ShexValidationResult,
    shex_validate,
)
```

### `ShexShapesSchema`

```python
class ShexShapesSchema:
    """A ShEx shapes schema for validation."""

    def __init__(self) -> None:
        """Creates a new empty shapes schema."""
        ...

    def parse(self, data: str) -> None:
        """
        Parses shapes from a string in ShExC (compact) format.

        :param data: The ShExC-formatted schema data
        :raises ValueError: If the data cannot be parsed
        """
        ...

    def parse_json(self, data: str) -> None:
        """
        Parses shapes from JSON format.

        :param data: The JSON-formatted schema data
        :raises ValueError: If the data cannot be parsed
        """
        ...

    def __len__(self) -> int:
        """Returns the number of shapes in the schema."""
        ...

    def is_empty(self) -> bool:
        """Returns True if the schema is empty."""
        ...
```

### `ShexValidator`

```python
class ShexValidator:
    """A ShEx validator for validating RDF data against shapes."""

    def __init__(self, schema: ShexShapesSchema) -> None:
        """
        Creates a new validator with the given shapes schema.

        :param schema: The shapes schema to validate against
        """
        ...

    def validate(self, data: str) -> ShexValidationReport:
        """
        Validates data against the shapes schema.

        :param data: The data to validate (as Turtle string)
        :return: A validation report
        :raises ValueError: If the data cannot be parsed
        :raises RuntimeError: If validation fails
        """
        ...

    def validate_graph(self, graph: Dataset) -> ShexValidationReport:
        """
        Validates a Dataset/Graph object against the shapes schema.

        :param graph: The Dataset to validate
        :return: A validation report
        :raises RuntimeError: If validation fails
        """
        ...

    def validate_node(
        self,
        graph: Dataset,
        focus: Term,
        shape: str
    ) -> ShexValidationReport:
        """
        Validates a specific node against a specific shape.

        :param graph: The Dataset containing the data
        :param focus: The node to validate
        :param shape: The shape identifier (as IRI string)
        :return: A validation report
        :raises RuntimeError: If validation fails
        """
        ...
```

### `ShexValidationReport`

```python
class ShexValidationReport:
    """
    A ShEx validation report.

    Contains the results of validating data against a shapes schema.
    """

    @property
    def conforms(self) -> bool:
        """
        Whether the data conforms to the shapes schema.

        Returns True if there are no failures.
        """
        ...

    @property
    def result_count(self) -> int:
        """The total number of results."""
        ...

    @property
    def failure_count(self) -> int:
        """The number of failures."""
        ...

    def results(self) -> list[ShexValidationResult]:
        """Returns the validation results as a list."""
        ...

    def to_turtle(self) -> str:
        """Returns the report as a Turtle string."""
        ...
```

### `ShexValidationResult`

```python
class ShexValidationResult:
    """A single ShEx validation result."""

    @property
    def focus(self) -> Term:
        """The focus node that was validated."""
        ...

    @property
    def shape(self) -> str:
        """The shape it was validated against."""
        ...

    @property
    def conformant(self) -> bool:
        """Whether the validation passed."""
        ...

    @property
    def reason(self) -> str | None:
        """The reason for failure (if any)."""
        ...
```

### Convenience Function

```python
def shex_validate(
    schema_data: str,
    data: str
) -> ShexValidationReport:
    """
    Validates RDF data against ShEx shapes (convenience function).

    :param schema_data: The shapes schema as a ShExC string
    :param data: The data to validate as a Turtle string
    :return: A validation report
    :raises ValueError: If the data cannot be parsed
    :raises RuntimeError: If validation fails
    """
    ...
```

### Usage Example

```python
from pyoxigraph import ShexShapesSchema, ShexValidator

# Parse schema
schema = ShexShapesSchema()
schema.parse("""
    PREFIX ex: <http://example.org/>
    PREFIX xsd: <http://www.w3.org/2001/XMLSchema#>

    ex:PersonShape {
        ex:name xsd:string ;
        ex:age xsd:integer
    }
""")

# Create validator
validator = ShexValidator(schema)

# Validate data
report = validator.validate("""
    PREFIX ex: <http://example.org/>

    ex:alice ex:name "Alice" ;
              ex:age 30 .
""")

if report.conforms:
    print("Valid!")
else:
    for result in report.results():
        if not result.conformant:
            print(f"Failure: {result.reason}")
```

---

## Stability Guarantees

### Stable (1.0+)

The following API elements are considered **stable** and will follow semantic versioning:

- Core types: `ShapesSchema`, `ShexValidator`, `ValidationReport`, `ValidationResult`
- Core functions: `parse_shex()`, `validate()`
- Shape model: `ShapeExpression`, `NodeConstraint`, `TripleConstraint`, `ShapeId`
- Error types: `ShexError`, `ShexParseError`, `ShexValidationError`
- Binding classes in JS and Python

### Unstable (Pre-1.0)

The following features may change in minor versions:

- Advanced shape features (recursion, semantic actions)
- Performance optimization flags
- Internal representation details
- Experimental validation modes

### Breaking Changes

Breaking changes will only occur in major version bumps:

- Removal of public API elements
- Changes to function signatures
- Semantic changes to validation behavior

---

## Implementation Notes

### Rust Implementation

- **Zero-copy parsing**: Schema parsing uses string references where possible
- **Lazy validation**: Shapes are only evaluated when needed
- **Thread safety**: All types are `Send + Sync` when applicable
- **Error handling**: All public APIs use `Result<T, E>` for error handling

### JavaScript/WASM Bindings

- **Memory management**: Uses `wasm-bindgen` automatic memory management
- **Error conversion**: Rust errors are converted to JavaScript `Error` objects
- **Type conversions**: RDF terms converted via `JsTerm` wrapper
- **Iterator support**: Collections support JavaScript iteration protocols

### Python Bindings

- **GIL handling**: Long operations release the GIL using `py.allow_threads()`
- **Error mapping**: Rust errors map to appropriate Python exception types
- **Type hints**: Full type annotations for IDE support
- **Pythonic naming**: Methods follow Python conventions (snake_case)

---

## Comparison with SHACL

ShEx and SHACL are both RDF validation languages but with different philosophies:

| Feature | ShEx | SHACL |
|---------|------|-------|
| **Syntax** | Compact, human-friendly | RDF-based (Turtle/SPARQL) |
| **Expressiveness** | Pattern matching focus | Constraint checking focus |
| **Closed shapes** | Native support | Via `sh:closed` |
| **Recursion** | First-class support | Limited |
| **Target selection** | Shape maps | Target declarations |

The Oxigraph API design intentionally mirrors SHACL for consistency, despite ShEx having different semantics.

---

## Migration Guide

### From SHACL to ShEx

```rust
// SHACL
use sparshacl::{ShaclValidator, ShapesGraph};
let shapes = ShapesGraph::from_graph(&graph)?;
let validator = ShaclValidator::new(shapes);
let report = validator.validate(&data)?;

// ShEx (similar API)
use sparshex::{ShexValidator, ShapesSchema};
let schema = ShapesSchema::from_graph(&graph)?;
let validator = ShexValidator::new(schema);
let report = validator.validate(&data)?;
```

### Key Differences

1. **Schema format**: ShEx uses `ShapesSchema` instead of `ShapesGraph`
2. **Parsing**: ShEx supports ShExC (compact) syntax via `parse()`
3. **Validation**: ShEx supports `validate_node()` for targeted validation
4. **Results**: ShEx results are `conformant/non-conformant` vs SHACL's severity levels

---

## Performance Considerations

### Schema Parsing

- **ShExC parsing**: O(n) where n is schema size
- **JSON parsing**: Faster than ShExC, use for production
- **Graph parsing**: Slowest, requires RDF parsing + conversion

### Validation

- **Graph size**: Linear in number of triples
- **Shape complexity**: Exponential in worst case (recursive shapes)
- **Caching**: Validator caches shape lookup, reuse for multiple validations

### Memory Usage

- **Schema**: ~100 bytes per shape + constraint overhead
- **Report**: ~50 bytes per validation result
- **Graph**: Depends on `oxrdf::Graph` (typically 40 bytes per triple)

### Optimization Tips

1. **Reuse validators**: Create once, validate many times
2. **Use JSON**: For large schemas, parse JSON not ShExC
3. **Batch validation**: Use `validate_shape_map()` for multiple nodes
4. **Stream parsing**: For huge graphs, validate incrementally

---

## Future Enhancements

Planned features for future releases:

- [ ] ShEx 2.1 advanced features (semantic actions, external shapes)
- [ ] ShEx extensions (e.g., ShExMap for RDF transformation)
- [ ] Incremental validation for streaming data
- [ ] Schema composition and imports
- [ ] SPARQL-based shape targets
- [ ] Performance profiling and optimization hooks

---

## References

- [ShEx Specification](https://shex.io/shex-semantics/)
- [ShEx Primer](https://shex.io/shex-primer/)
- [SHACL vs ShEx Comparison](https://www.w3.org/2014/data-shapes/wiki/Comparison_of_SHACL_and_ShEx)
- [Oxigraph Documentation](https://docs.rs/oxigraph/)
- [RDF 1.1 Concepts](https://www.w3.org/TR/rdf11-concepts/)
