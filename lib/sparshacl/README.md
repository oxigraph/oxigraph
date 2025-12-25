# sparshacl

[![Latest Version](https://img.shields.io/crates/v/sparshacl.svg)](https://crates.io/crates/sparshacl)
[![Released API docs](https://docs.rs/sparshacl/badge.svg)](https://docs.rs/sparshacl)

sparshacl is a Rust implementation of the [W3C SHACL (Shapes Constraint Language)](https://www.w3.org/TR/shacl/) specification for validating RDF graphs against a set of conditions called "shapes".

## Features

- **SHACL Core** - Full support for SHACL Core constraint components
- **Property Paths** - Support for SHACL property paths (predicate, sequence, alternative, inverse, etc.)
- **Validation Reports** - W3C-compliant validation report generation
- **Target Declarations** - Support for targetClass, targetNode, targetSubjectsOf, targetObjectsOf
- **Logical Constraints** - sh:and, sh:or, sh:not, sh:xone
- **SPARQL Constraints** - Optional SPARQL-based constraint support (with `sparql` feature)

## Usage

```rust
use sparshacl::{ShaclValidator, ShapesGraph, ValidationReport};
use oxrdf::{Graph, Dataset};

// Load shapes graph
let shapes_graph = ShapesGraph::from_graph(&shapes)?;

// Create validator
let validator = ShaclValidator::new(shapes_graph);

// Validate data graph
let report = validator.validate(&data_graph)?;

// Check conformance
if report.conforms() {
    println!("Data conforms to shapes!");
} else {
    for result in report.results() {
        println!("Violation: {:?}", result);
    }
}
```

## Feature Flags

- `sparql` - Enable SPARQL-based constraints (sh:sparql)

## License

This project is licensed under either of

* Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.
