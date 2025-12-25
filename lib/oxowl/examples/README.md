# OWL 2 Examples for Oxowl

This directory contains practical examples demonstrating the oxowl library's capabilities for working with OWL 2 ontologies.

## Available Examples

### 1. basic_ontology.rs
**Run**: `cargo run -p oxowl --example basic_ontology`

A foundational example showing:
- Creating an ontology with IRI
- Declaring OWL classes
- Building class hierarchies with SubClassOf axioms
- Defining disjoint classes
- Creating individuals and class assertions
- Basic reasoning with OWL 2 RL (when `reasoner-rl` feature is enabled)

**Concepts demonstrated:**
- Animal kingdom hierarchy (Animal ⊃ Mammal ⊃ Dog/Cat)
- Disjointness constraints
- Type inference

### 2. reasoning.rs
**Run**: `cargo run -p oxowl --example reasoning --features reasoner-rl`

An advanced reasoning example featuring:
- Complex class hierarchies with multiple inheritance
- Transitive and symmetric object properties
- Inverse property relationships
- Property chain reasoning
- Individual assertions and relationships
- Comprehensive OWL 2 RL reasoning
- Consistency checking

**Concepts demonstrated:**
- Family tree ontology with Parent, Father, Mother, Grandparent
- Property characteristics (transitive hasAncestor, symmetric hasSibling)
- Inverse properties (hasChild ⇔ hasParent)
- Type propagation through class hierarchy
- Inferred knowledge extraction

### 3. property_restrictions.rs
**Run**: `cargo run -p oxowl --example property_restrictions`

Demonstrates OWL 2 property restrictions:
- Existential restrictions (∃hasChild.Person)
- Universal restrictions (∀hasChild.Student)
- HasValue restrictions
- Cardinality restrictions (min, max, exact)
- Defining classes via restrictions

**Concepts demonstrated:**
- Parent ≡ Person ⊓ ∃hasChild.Person
- StrictParent ⊑ Parent ⊓ ∀hasChild.Student
- Cardinality constraints (exactly 2 children, at least 3 children)
- Qualified cardinality restrictions

### 4. data_properties.rs
**Run**: `cargo run -p oxowl --example data_properties`

Shows data property usage with literal values:
- Data property declarations
- Domain and range constraints
- Functional vs. non-functional properties
- Data property hierarchies
- Typed literal assertions (xsd:integer, xsd:decimal, xsd:string)

**Concepts demonstrated:**
- Person database with hasName, hasAge, hasEmail, hasSalary
- Functional properties (unique values)
- Property hierarchies (hasFirstName ⊑ hasName)
- XSD datatype usage

## Feature Flags

Some examples require specific feature flags:

- `reasoner-rl`: Enables OWL 2 RL reasoning engine
  - Required for: `reasoning.rs`, optional in `basic_ontology.rs` and `property_restrictions.rs`

## Learning Path

Recommended order for learning:

1. **Start with**: `basic_ontology.rs` - Learn the fundamentals
2. **Then try**: `property_restrictions.rs` - Understand complex class expressions
3. **Next**: `data_properties.rs` - Work with literal data
4. **Finally**: `reasoning.rs` - See the reasoner in action

## Common Patterns

### Creating an Ontology
```rust
let mut ontology = Ontology::with_iri("http://example.org/my-ontology")?;
```

### Defining a Class Hierarchy
```rust
let animal = OwlClass::new(NamedNode::new("http://example.org/Animal")?);
let mammal = OwlClass::new(NamedNode::new("http://example.org/Mammal")?);

ontology.add_axiom(Axiom::subclass_of(
    ClassExpression::class(mammal.clone()),
    ClassExpression::class(animal.clone()),
));
```

### Creating Individuals
```rust
let alice = Individual::Named(NamedNode::new("http://example.org/Alice")?);
ontology.add_axiom(Axiom::class_assertion(
    ClassExpression::class(person.clone()),
    alice.clone(),
));
```

### Using the Reasoner
```rust
use oxowl::{Reasoner, RlReasoner};

let mut reasoner = RlReasoner::new(&ontology);
reasoner.classify()?;

let types = reasoner.get_types(&individual);
let subclasses = reasoner.get_sub_classes(&class, direct: true);
```

## Building

To build all examples:
```bash
cargo build -p oxowl --examples
```

To build with reasoning support:
```bash
cargo build -p oxowl --examples --features reasoner-rl
```

## Additional Resources

- [OWL 2 Specification](https://www.w3.org/TR/owl2-overview/)
- [OWL 2 Primer](https://www.w3.org/TR/owl2-primer/)
- [OWL 2 Profiles](https://www.w3.org/TR/owl2-profiles/)
- [Oxigraph Documentation](https://docs.rs/oxigraph/)
