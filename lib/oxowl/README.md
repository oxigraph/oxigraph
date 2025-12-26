# OxOWL

[![Latest Version](https://img.shields.io/crates/v/oxowl.svg)](https://crates.io/crates/oxowl)
[![Released API docs](https://docs.rs/oxowl/badge.svg)](https://docs.rs/oxowl)

OxOWL provides comprehensive OWL 2 Web Ontology Language support for Oxigraph, enabling ontology-based knowledge representation and reasoning.

## Features

- **Complete OWL 2 Data Model**: Classes, properties, individuals, and axioms
- **Class Expressions**: Union, intersection, complement, restrictions
- **OWL 2 Profiles**: OWL 2 RL (Rule Language) reasoning profile
- **Forward-Chaining Reasoner**: RDFS+ entailment with property reasoning
- **RDF Integration**: Parse OWL ontologies from any RDF format

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
oxowl = "0.1"
```

Or use cargo:

```bash
cargo add oxowl
```

## Quick Start

```rust
use oxowl::{Ontology, Axiom, ClassExpression, OwlClass};
use oxrdf::NamedNode;

// Create an ontology
let mut ontology = Ontology::new();
ontology.set_iri(NamedNode::new("http://example.org/animals")?);

// Define classes
let animal = OwlClass::new(NamedNode::new("http://example.org/Animal")?);
let dog = OwlClass::new(NamedNode::new("http://example.org/Dog")?);

// Add subclass axiom: Dog ⊑ Animal
ontology.add_axiom(Axiom::SubClassOf {
    sub_class: ClassExpression::Class(dog.clone()),
    super_class: ClassExpression::Class(animal.clone()),
});

println!("Ontology has {} axioms", ontology.axiom_count());
```

## Reasoning

OxOWL includes an OWL 2 RL reasoner for forward-chaining inference:

```rust
use oxowl::{Ontology, RlReasoner, Reasoner};

// Load or create an ontology
let ontology = Ontology::new();

// Create and run reasoner
let mut reasoner = RlReasoner::new(&ontology);
reasoner.classify()?;

// Query inferred knowledge
let types = reasoner.get_types(&individual);
let subclasses = reasoner.get_sub_classes(&class, true); // direct only
```

### Reasoning Capabilities

- **Class hierarchy**: Transitive closure of subclass relations
- **RDFS entailment**: Domain/range inference, property hierarchies
- **Property characteristics**: Transitive, symmetric, inverse properties
- **Type propagation**: Instance classification through hierarchy

## Examples

See the [examples directory](./examples/) for comprehensive tutorials:

| Example | Description |
|---------|-------------|
| [basic_ontology.rs](./examples/basic_ontology.rs) | Fundamentals of ontology creation |
| [reasoning.rs](./examples/reasoning.rs) | OWL 2 RL reasoning and inference |
| [property_restrictions.rs](./examples/property_restrictions.rs) | Complex class expressions |
| [data_properties.rs](./examples/data_properties.rs) | Working with literals |

Run an example:

```bash
cargo run -p oxowl --example basic_ontology
```

Full learning path available in [examples/README.md](./examples/README.md).

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `reasoner-rl` | Yes | OWL 2 RL reasoning engine |

## OWL 2 Support

### Entities
- Named classes, object properties, data properties
- Annotation properties
- Named and anonymous individuals

### Class Expressions
- Boolean: `ObjectUnionOf`, `ObjectIntersectionOf`, `ObjectComplementOf`
- Enumeration: `ObjectOneOf`
- Restrictions: `ObjectSomeValuesFrom`, `ObjectAllValuesFrom`, `ObjectHasValue`
- Cardinality: `ObjectMinCardinality`, `ObjectMaxCardinality`, `ObjectExactCardinality`
- Data restrictions: `DataSomeValuesFrom`, `DataAllValuesFrom`, `DataHasValue`

### Axioms
- Class axioms: `SubClassOf`, `EquivalentClasses`, `DisjointClasses`
- Property axioms: `SubObjectPropertyOf`, `InverseObjectProperties`
- Property characteristics: Transitive, Symmetric, Functional, Reflexive
- Assertions: `ClassAssertion`, `ObjectPropertyAssertion`

## Integration with ΔGate

OxOWL provides the ontology layer for [ΔGate](../../docs/DELTAGATE_OVERVIEW.md) universe definition:

- **Universe definition**: OWL ontologies define the structure of O
- **Constraint integration**: Works with SHACL (sparshacl) for Σ validation
- **Reasoning for μ**: OWL 2 RL reasoning supports reconciliation

## Documentation

- [API Documentation](https://docs.rs/oxowl)
- [Examples](./examples/README.md)
- [OWL 2 Specification](https://www.w3.org/TR/owl2-overview/)
- [OWL 2 RL Profile](https://www.w3.org/TR/owl2-profiles/#OWL_2_RL)

## License

This project is licensed under either of Apache License, Version 2.0 or MIT license at your option.
