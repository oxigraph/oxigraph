# sparshex - Shape Expressions (ShEx) for RDF

[![Latest Version](https://img.shields.io/crates/v/sparshex.svg)](https://crates.io/crates/sparshex)
[![Released API docs](https://docs.rs/sparshex/badge.svg)](https://docs.rs/sparshex)

ShEx (Shape Expressions) validation engine for RDF graphs.

This crate provides a complete implementation of Shape Expressions (ShEx), a schema language for RDF graphs that allows validation of RDF data against shape definitions.

## Features

- **Complete ShEx 2.0 internal representation**
- **Shape Expressions**: Conjunctions (AND), disjunctions (OR), negations (NOT)
- **Triple Constraints**: Validate triples with predicates and cardinality
- **Node Constraints**: Datatype, value sets, string/numeric facets, patterns
- **Cardinality**: Min/max occurrences with convenient constructors
- **Cycle Detection**: Detect and prevent cyclic shape references
- **Oxigraph Integration**: Built on oxrdf types for seamless integration

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
sparshex = "0.1"
```

## Example

```rust
use sparshex::{
    ShapesSchema, ShapeLabel, ShapeExpression, NodeConstraint,
    NodeKind, TripleConstraint, Cardinality, Shape
};
use oxrdf::NamedNode;

// Create a shapes schema
let mut schema = ShapesSchema::new();

// Define a shape label
let person_label = ShapeLabel::Iri(
    NamedNode::new("http://example.org/PersonShape").unwrap()
);

// Create a node constraint requiring IRI nodes
let node_constraint = NodeConstraint::with_node_kind(NodeKind::Iri);
let shape_expr = ShapeExpression::NodeConstraint(node_constraint);

// Add shape to schema
schema.add_shape(person_label.clone(), shape_expr);

// Validate that all shape references are defined
assert!(schema.validate_refs().is_ok());

// Detect cycles in shape references
assert!(schema.detect_cycles().is_ok());
```

## Shape Expression Types

### NodeConstraint

Validates properties of individual nodes:

```rust
use sparshex::{NodeConstraint, NodeKind, StringFacet};
use oxrdf::NamedNode;

let mut constraint = NodeConstraint::new();
constraint.node_kind = Some(NodeKind::Literal);
constraint.datatype = Some(NamedNode::new("http://www.w3.org/2001/XMLSchema#string").unwrap());
constraint.string_facets.push(StringFacet::MinLength(1));
constraint.string_facets.push(StringFacet::MaxLength(100));
```

### TripleConstraint

Validates triples with specific predicates:

```rust
use sparshex::{TripleConstraint, Cardinality, ShapeExpression};
use oxrdf::NamedNode;

let predicate = NamedNode::new("http://xmlns.com/foaf/0.1/name").unwrap();
let value_expr = ShapeExpression::NodeConstraint(NodeConstraint::new());

let constraint = TripleConstraint::with_value_expr(predicate, value_expr)
    .with_cardinality(Cardinality::one_or_more());
```

### Cardinality

Control occurrence counts:

```rust
use sparshex::Cardinality;

// Exactly 1 (default)
let c1 = Cardinality::default();

// Optional (0 or 1)
let c2 = Cardinality::optional();

// Zero or more (*)
let c3 = Cardinality::zero_or_more();

// One or more (+)
let c4 = Cardinality::one_or_more();

// Custom range
let c5 = Cardinality::new(2, Some(5)).unwrap();
```

## ShEx Specification

This implementation follows the [ShEx 2.0 specification](https://shex.io/shex-semantics/).

## License

This project is licensed under either of:

- Apache License, Version 2.0, ([LICENSE-APACHE](../../LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../../LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
