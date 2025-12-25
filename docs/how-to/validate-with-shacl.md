# How to Validate RDF Data with SHACL

This guide explains how to use SHACL (Shapes Constraint Language) validation in Oxigraph to ensure your RDF data meets quality requirements.

## What is SHACL?

SHACL is a W3C standard for validating RDF graphs against a set of conditions (shapes). It helps ensure:
- Data quality and consistency
- Schema compliance
- Business rule enforcement
- Data integrity constraints

## SHACL Support in Oxigraph

Oxigraph supports SHACL validation through the `sparshacl` crate and provides bindings for:
- **Rust** - Full SHACL Core support
- **Python** - Via `pyoxigraph`
- **JavaScript** - Via the `oxigraph` npm package

### Supported Features
- ✅ SHACL Core constraint components
- ✅ Property paths (predicate, sequence, alternative, inverse, etc.)
- ✅ W3C-compliant validation reports
- ✅ Target declarations (targetClass, targetNode, targetSubjectsOf, targetObjectsOf)
- ✅ Logical constraints (sh:and, sh:or, sh:not, sh:xone)
- ⚠️ SPARQL constraints (with `sparql` feature flag in Rust)

## Using SHACL in Python

### Basic Validation

```python
from pyoxigraph import Store, shacl

# Create store and load data
store = Store()
store.load(input=open("data.ttl", "rb"), format=RdfFormat.TURTLE)

# Load SHACL shapes
shapes_graph = Store()
shapes_graph.load(open("shapes.ttl", "rb"), format=RdfFormat.TURTLE)

# Validate
validator = shacl.ShaclValidator(shapes_graph)
report = validator.validate(store)

# Check results
if report.conforms:
    print("✓ Data is valid!")
else:
    print("✗ Validation failed:")
    for result in report.results:
        print(f"  - {result}")
```

### Complete Example

```python
from pyoxigraph import Store, shacl, NamedNode

# Create data
data = """
@prefix ex: <http://example.com/> .
@prefix schema: <http://schema.org/> .

ex:Alice a schema:Person ;
    schema:name "Alice" ;
    schema:email "alice@example.com" .

ex:Bob a schema:Person ;
    schema:name "Bob" .
    # Missing email
"""

# Define SHACL shapes
shapes = """
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix schema: <http://schema.org/> .

ex:PersonShape a sh:NodeShape ;
    sh:targetClass schema:Person ;
    sh:property [
        sh:path schema:name ;
        sh:minCount 1 ;
        sh:maxCount 1 ;
        sh:datatype xsd:string ;
    ] ;
    sh:property [
        sh:path schema:email ;
        sh:minCount 1 ;
        sh:pattern "^[\\w.+-]+@[\\w.-]+\\.[a-zA-Z]{2,}$" ;
    ] .
"""

# Load data and shapes
store = Store()
store.load(input=data.encode(), format=RdfFormat.TURTLE)

shapes_store = Store()
shapes_store.load(shapes.encode(), format=RdfFormat.TURTLE)

# Validate
validator = shacl.ShaclValidator(shapes_store)
report = validator.validate(store)

# Process results
print(f"Conforms: {report.conforms}")
for result in report.results:
    print(f"Focus Node: {result.focus_node}")
    print(f"Path: {result.result_path}")
    print(f"Message: {result.message}")
    print(f"Severity: {result.severity}")
    print()
```

## Using SHACL in JavaScript

### Basic Validation

```javascript
import { Store, ShaclValidator } from 'oxigraph';

// Load data
const dataStore = new Store();
dataStore.load(dataRdf, { format: 'text/turtle' });

// Load shapes
const shapesStore = new Store();
shapesStore.load(shapesRdf, { format: 'text/turtle' });

// Validate
const validator = new ShaclValidator(shapesStore);
const report = validator.validate(dataStore);

// Check results
if (report.conforms) {
    console.log('✓ Data is valid!');
} else {
    console.log('✗ Validation failed:');
    for (const result of report.results) {
        console.log(`  Focus: ${result.focusNode}`);
        console.log(`  Path: ${result.resultPath}`);
        console.log(`  Message: ${result.message}`);
    }
}
```

### Complete Example

```javascript
import { Store, ShaclValidator } from 'oxigraph';

// Define data
const data = `
@prefix ex: <http://example.com/> .
@prefix schema: <http://schema.org/> .

ex:Alice a schema:Person ;
    schema:name "Alice" ;
    schema:age 30 .

ex:Bob a schema:Person ;
    schema:name "Bob" ;
    schema:age -5 .
`;

// Define shapes
const shapes = `
@prefix sh: <http://www.w3.org/ns/shacl#> .
@prefix schema: <http://schema.org/> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

ex:PersonShape a sh:NodeShape ;
    sh:targetClass schema:Person ;
    sh:property [
        sh:path schema:name ;
        sh:minCount 1 ;
        sh:datatype xsd:string ;
    ] ;
    sh:property [
        sh:path schema:age ;
        sh:datatype xsd:integer ;
        sh:minInclusive 0 ;
        sh:maxInclusive 150 ;
    ] .
`;

// Load and validate
const dataStore = new Store();
dataStore.load(data, { format: 'text/turtle' });

const shapesStore = new Store();
shapesStore.load(shapes, { format: 'text/turtle' });

const validator = new ShaclValidator(shapesStore);
const report = validator.validate(dataStore);

// Display results
console.log(`Validation: ${report.conforms ? 'PASSED' : 'FAILED'}`);

if (!report.conforms) {
    for (const result of report.results) {
        console.log('\nViolation:');
        console.log(`  Focus Node: ${result.focusNode}`);
        console.log(`  Property Path: ${result.resultPath}`);
        console.log(`  Value: ${result.value}`);
        console.log(`  Message: ${result.message}`);
        console.log(`  Severity: ${result.severity}`);
    }
}
```

## Using SHACL in Rust

### Basic Validation

```rust
use sparshacl::{ShaclValidator, ShapesGraph};
use oxrdf::Graph;
use oxigraph::io::{RdfFormat, RdfParser};
use std::fs::File;

// Load shapes graph
let shapes_parser = RdfParser::from_format(RdfFormat::Turtle)
    .for_reader(File::open("shapes.ttl")?);
let mut shapes_graph = Graph::new();
for triple in shapes_parser {
    shapes_graph.insert(triple?);
}

// Create validator
let shapes = ShapesGraph::from_graph(&shapes_graph)?;
let validator = ShaclValidator::new(shapes);

// Load data graph
let data_parser = RdfParser::from_format(RdfFormat::Turtle)
    .for_reader(File::open("data.ttl")?);
let mut data_graph = Graph::new();
for triple in data_parser {
    data_graph.insert(triple?);
}

// Validate
let report = validator.validate(&data_graph)?;

// Check results
if report.conforms() {
    println!("✓ Data is valid!");
} else {
    println!("✗ Validation failed:");
    for result in report.results() {
        println!("  Violation: {:?}", result);
    }
}
```

### Complete Example

```rust
use sparshacl::{ShaclValidator, ShapesGraph};
use oxrdf::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create data graph
    let mut data = Graph::new();
    let ex = Namespace::new("http://example.com/")?;
    let schema = Namespace::new("http://schema.org/")?;

    data.insert(TripleRef::new(
        ex.get("Alice")?,
        vocab::rdf::TYPE,
        schema.get("Person")?,
    ));
    data.insert(TripleRef::new(
        ex.get("Alice")?,
        schema.get("name")?,
        Literal::new_simple_literal("Alice"),
    ));
    data.insert(TripleRef::new(
        ex.get("Alice")?,
        schema.get("age")?,
        Literal::new_typed_literal("30", vocab::xsd::INTEGER),
    ));

    // Create shapes graph
    let mut shapes = Graph::new();
    let sh = Namespace::new("http://www.w3.org/ns/shacl#")?;

    // Define PersonShape
    let person_shape = ex.get("PersonShape")?;
    shapes.insert(TripleRef::new(
        person_shape,
        vocab::rdf::TYPE,
        sh.get("NodeShape")?,
    ));
    shapes.insert(TripleRef::new(
        person_shape,
        sh.get("targetClass")?,
        schema.get("Person")?,
    ));

    // Validate
    let shapes_graph = ShapesGraph::from_graph(&shapes)?;
    let validator = ShaclValidator::new(shapes_graph);
    let report = validator.validate(&data)?;

    println!("Conforms: {}", report.conforms());

    Ok(())
}
```

## Common SHACL Shapes

### Required Property

```turtle
@prefix sh: <http://www.w3.org/ns/shacl#> .

ex:Shape a sh:NodeShape ;
    sh:targetClass ex:Person ;
    sh:property [
        sh:path ex:name ;
        sh:minCount 1 ;  # Required
    ] .
```

### Datatype Constraint

```turtle
ex:Shape a sh:NodeShape ;
    sh:targetClass ex:Person ;
    sh:property [
        sh:path ex:age ;
        sh:datatype xsd:integer ;
    ] .
```

### Value Range

```turtle
ex:Shape a sh:NodeShape ;
    sh:targetClass ex:Person ;
    sh:property [
        sh:path ex:age ;
        sh:minInclusive 0 ;
        sh:maxInclusive 150 ;
    ] .
```

### String Pattern

```turtle
ex:Shape a sh:NodeShape ;
    sh:targetClass ex:Person ;
    sh:property [
        sh:path ex:email ;
        sh:pattern "^[\\w.+-]+@[\\w.-]+\\.[a-zA-Z]{2,}$" ;
    ] .
```

### Value from List

```turtle
ex:Shape a sh:NodeShape ;
    sh:targetClass ex:Person ;
    sh:property [
        sh:path ex:status ;
        sh:in ( "active" "inactive" "pending" ) ;
    ] .
```

### Cardinality Constraints

```turtle
ex:Shape a sh:NodeShape ;
    sh:targetClass ex:Person ;
    sh:property [
        sh:path ex:email ;
        sh:minCount 1 ;
        sh:maxCount 3 ;  # 1-3 emails
    ] .
```

### Class Constraint

```turtle
ex:Shape a sh:NodeShape ;
    sh:targetClass ex:Person ;
    sh:property [
        sh:path ex:employer ;
        sh:class ex:Organization ;  # Must be an Organization
    ] .
```

### Node Kind

```turtle
ex:Shape a sh:NodeShape ;
    sh:targetClass ex:Person ;
    sh:property [
        sh:path ex:homepage ;
        sh:nodeKind sh:IRI ;  # Must be IRI, not literal or blank node
    ] .
```

## Advanced SHACL Features

### Logical Constraints

```turtle
# AND constraint
ex:Shape a sh:NodeShape ;
    sh:targetClass ex:Adult ;
    sh:and (
        [ sh:property [ sh:path ex:age ; sh:minInclusive 18 ] ]
        [ sh:property [ sh:path ex:name ; sh:minCount 1 ] ]
    ) .

# OR constraint
ex:Shape a sh:NodeShape ;
    sh:targetClass ex:Contact ;
    sh:or (
        [ sh:property [ sh:path ex:email ; sh:minCount 1 ] ]
        [ sh:property [ sh:path ex:phone ; sh:minCount 1 ] ]
    ) .

# NOT constraint
ex:Shape a sh:NodeShape ;
    sh:targetClass ex:Person ;
    sh:not [
        sh:property [ sh:path ex:status ; sh:hasValue "deleted" ]
    ] .
```

### Property Paths

```turtle
# Sequence path
ex:Shape sh:property [
    sh:path ( ex:address ex:city ) ;  # address/city
    sh:minCount 1 ;
] .

# Alternative path
ex:Shape sh:property [
    sh:path [ sh:alternativePath ( ex:name ex:label ) ] ;
    sh:minCount 1 ;
] .

# Inverse path
ex:Shape sh:property [
    sh:path [ sh:inversePath ex:knows ] ;  # ?x ex:knows FOCUS
    sh:minCount 1 ;
] .

# Zero or more path
ex:Shape sh:property [
    sh:path [ sh:zeroOrMorePath ex:parent ] ;  # Ancestors
    sh:class ex:Person ;
] .
```

### Qualified Value Shapes

```turtle
ex:Shape a sh:NodeShape ;
    sh:targetClass ex:Department ;
    sh:property [
        sh:path ex:employee ;
        sh:qualifiedValueShape [
            sh:property [ sh:path ex:role ; sh:hasValue "manager" ]
        ] ;
        sh:qualifiedMinCount 1 ;  # At least one manager
    ] .
```

## Validation Report Structure

A validation report contains:

```turtle
@prefix sh: <http://www.w3.org/ns/shacl#> .

# Report
[] a sh:ValidationReport ;
    sh:conforms false ;
    sh:result [
        a sh:ValidationResult ;
        sh:focusNode ex:Bob ;
        sh:resultPath ex:email ;
        sh:resultSeverity sh:Violation ;
        sh:sourceConstraintComponent sh:MinCountConstraintComponent ;
        sh:sourceShape ex:PersonShape ;
        sh:resultMessage "Must have at least 1 email" ;
    ] .
```

### Accessing Report Details

```python
# Python
for result in report.results:
    print(f"Focus: {result.focus_node}")           # The problematic node
    print(f"Path: {result.result_path}")           # The property path
    print(f"Severity: {result.severity}")          # Violation/Warning/Info
    print(f"Message: {result.message}")            # Human-readable message
    print(f"Value: {result.value}")                # The problematic value
    print(f"Component: {result.source_component}") # Constraint type
```

```javascript
// JavaScript
for (const result of report.results) {
    console.log(`Focus: ${result.focusNode}`);
    console.log(`Path: ${result.resultPath}`);
    console.log(`Severity: ${result.severity}`);
    console.log(`Message: ${result.message}`);
}
```

## Integrating SHACL Validation

### Validate Before Import

```python
def safe_import(data_file, shapes_file, store):
    # Load data into temporary store
    temp = Store()
    temp.load(open(data_file, "rb"), format=RdfFormat.TURTLE)

    # Load shapes
    shapes = Store()
    shapes.load(open(shapes_file, "rb"), format=RdfFormat.TURTLE)

    # Validate
    validator = shacl.ShaclValidator(shapes)
    report = validator.validate(temp)

    if report.conforms:
        # Import into main store
        for quad in temp:
            store.add(quad)
        print(f"✓ Imported {len(temp)} quads")
    else:
        print("✗ Validation failed:")
        for result in report.results:
            print(f"  - {result.message}")
        raise ValueError("Data does not conform to shapes")
```

### API Endpoint Validation

```python
from flask import Flask, request, jsonify
from pyoxigraph import Store, shacl

app = Flask(__name__)
store = Store("data")
shapes = Store()
shapes.load(open("shapes.ttl", "rb"), format=RdfFormat.TURTLE)
validator = shacl.ShaclValidator(shapes)

@app.route('/api/data', methods=['POST'])
def add_data():
    # Parse input
    temp = Store()
    temp.load(request.data, format=request.content_type)

    # Validate
    report = validator.validate(temp)

    if report.conforms:
        # Add to store
        for quad in temp:
            store.add(quad)
        return jsonify({"status": "ok"}), 200
    else:
        # Return validation errors
        errors = [str(r.message) for r in report.results]
        return jsonify({"status": "error", "errors": errors}), 400
```

## Best Practices

1. **Separate Shapes from Data** - Keep shapes in separate files/graphs
2. **Use Descriptive Messages** - Add custom messages to shapes
3. **Start Simple** - Begin with basic constraints and add complexity
4. **Test Incrementally** - Validate small datasets first
5. **Version Your Shapes** - Track changes to validation rules
6. **Document Constraints** - Use rdfs:comment to explain shapes
7. **Use Severity Levels** - Distinguish violations from warnings
8. **Optimize for Performance** - Complex shapes can be slow on large datasets

## Troubleshooting

### Shapes Not Applied

Ensure targets are correctly specified:

```turtle
# Must have a target!
ex:PersonShape a sh:NodeShape ;
    sh:targetClass schema:Person ;  # ← Target required
    sh:property [...] .
```

### No Validation Errors But Data Seems Wrong

Check that your data actually matches the target:

```turtle
# Shape targets schema:Person
sh:targetClass schema:Person ;

# But data uses ex:Person (different!)
ex:Alice a ex:Person .  # Won't be validated!
```

### Performance Issues

- Limit the number of targets
- Simplify property paths
- Use specific constraints instead of SPARQL-based ones
- Validate incrementally rather than entire dataset at once

## Next Steps

- Learn about [importing validated data](import-rdf-data.md)
- Set up [automated validation in CI/CD](run-sparql-server.md)
- Optimize validation with [performance tips](optimize-performance.md)
- Explore the [SHACL specification](https://www.w3.org/TR/shacl/)
