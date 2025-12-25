# RDF Basics in Oxigraph

This tutorial introduces the RDF (Resource Description Framework) data model and shows you how to work with RDF concepts in Oxigraph using Rust.

## What is RDF?

RDF is a standard for representing information on the web. Think of it as a way to describe things and their relationships using simple statements.

### The Triple Model

RDF data is organized in **triples** (also called statements). Each triple has three parts:

- **Subject**: What you're talking about
- **Predicate**: The property or relationship
- **Object**: The value or related resource

Example: "Alice knows Bob"
- Subject: Alice
- Predicate: knows
- Object: Bob

## RDF Node Types

Oxigraph provides several types to represent RDF nodes, all in the `oxigraph::model` module.

### 1. Named Nodes (IRIs)

Named nodes represent resources identified by IRIs (Internationalized Resource Identifiers). These are similar to URLs.

```rust
use oxigraph::model::NamedNode;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a named node
    let alice = NamedNode::new("http://example.org/alice")?;

    println!("Named node: {}", alice);
    // Output: Named node: <http://example.org/alice>

    // Get the IRI as a string
    println!("IRI: {}", alice.as_str());
    // Output: IRI: http://example.org/alice

    Ok(())
}
```

**Common patterns:**

```rust
use oxigraph::model::NamedNode;

// Using well-known vocabularies
let foaf_knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;
let rdf_type = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?;
let schema_person = NamedNode::new("http://schema.org/Person")?;

// Using vocabulary constants
use oxigraph::model::vocab::{rdf, rdfs, xsd};
let type_predicate = rdf::TYPE;  // Predefined constant
```

### 2. Blank Nodes

Blank nodes represent resources without a global identifier. They're useful for anonymous or local resources.

```rust
use oxigraph::model::BlankNode;

fn main() {
    // Create a blank node (automatically generated ID)
    let blank1 = BlankNode::default();
    let blank2 = BlankNode::default();

    println!("Blank node 1: {}", blank1);
    println!("Blank node 2: {}", blank2);
    // Output: _:b0, _:b1 (actual IDs may vary)

    // Create a blank node with a specific ID
    let blank_custom = BlankNode::new("person1").unwrap();
    println!("Custom blank node: {}", blank_custom);
    // Output: _:person1
}
```

**Use cases for blank nodes:**

```rust
use oxigraph::model::*;
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    // Representing an address without giving it a global identifier
    let person = NamedNode::new("http://example.org/alice")?;
    let has_address = NamedNode::new("http://example.org/hasAddress")?;
    let street = NamedNode::new("http://example.org/street")?;
    let city = NamedNode::new("http://example.org/city")?;

    // The address is a blank node
    let address = BlankNode::default();

    store.insert(&Quad::new(
        person.clone(),
        has_address,
        address.clone(),
        GraphName::DefaultGraph
    ))?;

    store.insert(&Quad::new(
        address.clone(),
        street,
        Literal::new_simple_literal("123 Main St"),
        GraphName::DefaultGraph
    ))?;

    store.insert(&Quad::new(
        address,
        city,
        Literal::new_simple_literal("Springfield"),
        GraphName::DefaultGraph
    ))?;

    Ok(())
}
```

### 3. Literals

Literals represent values like strings, numbers, dates, and more.

#### Simple Literals (Plain Strings)

```rust
use oxigraph::model::Literal;

fn main() {
    let name = Literal::new_simple_literal("Alice");
    println!("{}", name);
    // Output: "Alice"
}
```

#### Language-Tagged Literals

Use these for multilingual text:

```rust
use oxigraph::model::Literal;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // English
    let name_en = Literal::new_language_tagged_literal("Alice", "en")?;

    // French
    let name_fr = Literal::new_language_tagged_literal("Alice", "fr")?;

    // German
    let name_de = Literal::new_language_tagged_literal("Alice", "de")?;

    println!("English: {}", name_en);
    println!("French: {}", name_fr);
    println!("German: {}", name_de);
    // Output:
    // English: "Alice"@en
    // French: "Alice"@fr
    // German: "Alice"@de

    Ok(())
}
```

#### Typed Literals (Numbers, Dates, etc.)

For structured data with specific datatypes:

```rust
use oxigraph::model::Literal;
use oxigraph::model::vocab::xsd;

fn main() {
    // Integer
    let age = Literal::new_typed_literal("30", xsd::INTEGER);

    // Decimal
    let price = Literal::new_typed_literal("19.99", xsd::DECIMAL);

    // Boolean
    let is_active = Literal::new_typed_literal("true", xsd::BOOLEAN);

    // Date
    let birth_date = Literal::new_typed_literal("1990-01-15", xsd::DATE);

    // DateTime
    let timestamp = Literal::new_typed_literal(
        "2024-01-15T10:30:00Z",
        xsd::DATE_TIME
    );

    println!("Age: {}", age);
    println!("Price: {}", price);
    println!("Active: {}", is_active);
    println!("Birth date: {}", birth_date);
    println!("Timestamp: {}", timestamp);
}
```

**Complete example with different literal types:**

```rust
use oxigraph::model::*;
use oxigraph::model::vocab::xsd;
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    let person = NamedNode::new("http://example.org/alice")?;
    let name = NamedNode::new("http://xmlns.com/foaf/0.1/name")?;
    let age = NamedNode::new("http://xmlns.com/foaf/0.1/age")?;
    let email = NamedNode::new("http://xmlns.com/foaf/0.1/mbox")?;
    let description = NamedNode::new("http://purl.org/dc/terms/description")?;

    // Simple literal
    store.insert(&Quad::new(
        person.clone(),
        name.clone(),
        Literal::new_simple_literal("Alice"),
        GraphName::DefaultGraph
    ))?;

    // Language-tagged literal
    store.insert(&Quad::new(
        person.clone(),
        description.clone(),
        Literal::new_language_tagged_literal("A software developer", "en")?,
        GraphName::DefaultGraph
    ))?;

    // Typed literal (integer)
    store.insert(&Quad::new(
        person.clone(),
        age,
        Literal::new_typed_literal("30", xsd::INTEGER),
        GraphName::DefaultGraph
    ))?;

    // Typed literal (anyURI)
    store.insert(&Quad::new(
        person,
        email,
        Literal::new_typed_literal("mailto:alice@example.org", xsd::ANY_URI),
        GraphName::DefaultGraph
    ))?;

    println!("Stored {} quads", store.len()?);

    Ok(())
}
```

## Working with Triples

A triple is a statement with subject, predicate, and object. In Oxigraph, we typically work with **Quads** (which include an optional graph name).

### Creating Quads

```rust
use oxigraph::model::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subject = NamedNode::new("http://example.org/alice")?;
    let predicate = NamedNode::new("http://xmlns.com/foaf/0.1/name")?;
    let object = Literal::new_simple_literal("Alice");

    // Create a quad in the default graph
    let quad = Quad::new(
        subject,
        predicate,
        object,
        GraphName::DefaultGraph
    );

    println!("Quad: {}", quad);
    // Output: <http://example.org/alice> <http://xmlns.com/foaf/0.1/name> "Alice" .

    Ok(())
}
```

### Named Graphs

Quads can belong to named graphs, which help organize data into separate contexts:

```rust
use oxigraph::model::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let subject = NamedNode::new("http://example.org/alice")?;
    let predicate = NamedNode::new("http://xmlns.com/foaf/0.1/name")?;
    let object = Literal::new_simple_literal("Alice");

    // Quad in a named graph
    let graph_name = NamedNode::new("http://example.org/graph/users")?;
    let quad = Quad::new(
        subject,
        predicate,
        object,
        GraphName::NamedNode(graph_name)
    );

    println!("Quad with graph: {}", quad);

    Ok(())
}
```

## Working with Graphs and Datasets

### In-Memory Graph

A `Graph` is a collection of triples (without graph names):

```rust
use oxigraph::model::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut graph = Graph::default();

    // Create some nodes
    let alice = NamedNode::new("http://example.org/alice")?;
    let bob = NamedNode::new("http://example.org/bob")?;
    let knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;

    // Create a triple
    let triple = Triple::new(alice.clone(), knows, bob);

    // Insert into graph
    graph.insert(&triple);

    // Check if triple exists
    if graph.contains(&triple) {
        println!("Graph contains the triple!");
    }

    // Count triples
    println!("Graph has {} triples", graph.len());

    // Iterate over triples
    for t in graph.iter() {
        println!("Triple: {} {} {}", t.subject, t.predicate, t.object);
    }

    Ok(())
}
```

### In-Memory Dataset

A `Dataset` is a collection of quads (with graph names):

```rust
use oxigraph::model::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut dataset = Dataset::default();

    let alice = NamedNode::new("http://example.org/alice")?;
    let bob = NamedNode::new("http://example.org/bob")?;
    let knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;

    // Add quad to default graph
    dataset.insert(&Quad::new(
        alice.clone(),
        knows.clone(),
        bob.clone(),
        GraphName::DefaultGraph
    ));

    // Add quad to named graph
    let graph = NamedNode::new("http://example.org/graph/social")?;
    dataset.insert(&Quad::new(
        alice.clone(),
        knows,
        bob,
        GraphName::NamedNode(graph)
    ));

    println!("Dataset has {} quads", dataset.len());

    // Iterate over all quads
    for quad in dataset.iter() {
        println!(
            "Quad: {} {} {} in graph {:?}",
            quad.subject, quad.predicate, quad.object, quad.graph_name
        );
    }

    Ok(())
}
```

## Pattern Matching and Filtering

You can query graphs and datasets using patterns:

### Filtering Triples in a Graph

```rust
use oxigraph::model::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut graph = Graph::default();

    let alice = NamedNode::new("http://example.org/alice")?;
    let bob = NamedNode::new("http://example.org/bob")?;
    let charlie = NamedNode::new("http://example.org/charlie")?;
    let knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;
    let name = NamedNode::new("http://xmlns.com/foaf/0.1/name")?;

    // Add some triples
    graph.insert(&Triple::new(
        alice.clone(),
        knows.clone(),
        bob.clone()
    ));
    graph.insert(&Triple::new(
        alice.clone(),
        knows.clone(),
        charlie.clone()
    ));
    graph.insert(&Triple::new(
        alice.clone(),
        name,
        Literal::new_simple_literal("Alice")
    ));

    // Find all people Alice knows
    println!("People Alice knows:");
    for triple in graph.triples_for_subject(&alice) {
        if triple.predicate == knows.as_ref() {
            println!("  {}", triple.object);
        }
    }

    Ok(())
}
```

### Filtering Quads in a Store

```rust
use oxigraph::model::*;
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    let alice = NamedNode::new("http://example.org/alice")?;
    let bob = NamedNode::new("http://example.org/bob")?;
    let knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;
    let name = NamedNode::new("http://xmlns.com/foaf/0.1/name")?;

    // Add some data
    store.insert(&Quad::new(
        alice.clone(),
        knows.clone(),
        bob.clone(),
        GraphName::DefaultGraph
    ))?;
    store.insert(&Quad::new(
        alice.clone(),
        name,
        Literal::new_simple_literal("Alice"),
        GraphName::DefaultGraph
    ))?;

    // Query pattern: find all triples where Alice is the subject
    let results: Vec<_> = store
        .quads_for_pattern(
            Some(alice.as_ref().into()),  // subject
            None,                           // any predicate
            None,                           // any object
            None                            // any graph
        )
        .collect::<Result<Vec<_>, _>>()?;

    println!("Found {} quads about Alice:", results.len());
    for quad in results {
        println!("  {}", quad);
    }

    Ok(())
}
```

## Complete Example: Building a Knowledge Graph

Here's a comprehensive example that puts it all together:

```rust
use oxigraph::model::*;
use oxigraph::model::vocab::{rdf, xsd};
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Building a Knowledge Graph ===\n");

    let store = Store::new()?;

    // Define vocabulary
    let foaf_person = NamedNode::new("http://xmlns.com/foaf/0.1/Person")?;
    let foaf_name = NamedNode::new("http://xmlns.com/foaf/0.1/name")?;
    let foaf_knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;
    let foaf_age = NamedNode::new("http://xmlns.com/foaf/0.1/age")?;
    let ex = "http://example.org/";

    // Create people
    let alice = NamedNode::new(&format!("{}alice", ex))?;
    let bob = NamedNode::new(&format!("{}bob", ex))?;
    let charlie = NamedNode::new(&format!("{}charlie", ex))?;

    // Alice's data
    store.insert(&Quad::new(
        alice.clone(),
        rdf::TYPE,
        foaf_person.clone(),
        GraphName::DefaultGraph
    ))?;
    store.insert(&Quad::new(
        alice.clone(),
        foaf_name.clone(),
        Literal::new_simple_literal("Alice"),
        GraphName::DefaultGraph
    ))?;
    store.insert(&Quad::new(
        alice.clone(),
        foaf_age.clone(),
        Literal::new_typed_literal("30", xsd::INTEGER),
        GraphName::DefaultGraph
    ))?;

    // Bob's data
    store.insert(&Quad::new(
        bob.clone(),
        rdf::TYPE,
        foaf_person.clone(),
        GraphName::DefaultGraph
    ))?;
    store.insert(&Quad::new(
        bob.clone(),
        foaf_name.clone(),
        Literal::new_simple_literal("Bob"),
        GraphName::DefaultGraph
    ))?;
    store.insert(&Quad::new(
        bob.clone(),
        foaf_age.clone(),
        Literal::new_typed_literal("25", xsd::INTEGER),
        GraphName::DefaultGraph
    ))?;

    // Charlie's data with language tags
    store.insert(&Quad::new(
        charlie.clone(),
        rdf::TYPE,
        foaf_person,
        GraphName::DefaultGraph
    ))?;
    store.insert(&Quad::new(
        charlie.clone(),
        foaf_name.clone(),
        Literal::new_language_tagged_literal("Charlie", "en")?,
        GraphName::DefaultGraph
    ))?;
    store.insert(&Quad::new(
        charlie.clone(),
        foaf_name.clone(),
        Literal::new_language_tagged_literal("Carlos", "es")?,
        GraphName::DefaultGraph
    ))?;

    // Relationships
    store.insert(&Quad::new(
        alice.clone(),
        foaf_knows.clone(),
        bob.clone(),
        GraphName::DefaultGraph
    ))?;
    store.insert(&Quad::new(
        alice.clone(),
        foaf_knows.clone(),
        charlie.clone(),
        GraphName::DefaultGraph
    ))?;
    store.insert(&Quad::new(
        bob,
        foaf_knows,
        charlie,
        GraphName::DefaultGraph
    ))?;

    println!("Knowledge graph built successfully!");
    println!("Total quads: {}\n", store.len()?);

    // Query the data
    println!("Alice's friends:");
    for quad in store.quads_for_pattern(
        Some(alice.as_ref().into()),
        Some(NamedNodeRef::new("http://xmlns.com/foaf/0.1/knows")?.into()),
        None,
        None
    ) {
        let quad = quad?;
        println!("  {}", quad.object);
    }

    Ok(())
}
```

## Best Practices

### 1. Use Vocabulary Constants

Instead of creating IRIs repeatedly, use constants:

```rust
use oxigraph::model::vocab::{rdf, rdfs, xsd};

// Good
let type_predicate = rdf::TYPE;

// Less efficient (creates new NamedNode each time)
let type_predicate = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?;
```

### 2. Reuse Named Nodes

```rust
// Create once
let knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;

// Reuse (clone is cheap for NamedNode)
store.insert(&Quad::new(alice.clone(), knows.clone(), bob, GraphName::DefaultGraph))?;
store.insert(&Quad::new(charlie.clone(), knows.clone(), david, GraphName::DefaultGraph))?;
```

### 3. Use References When Possible

```rust
// When you don't need ownership, use references
let quad = QuadRef::new(
    alice.as_ref(),
    knows.as_ref(),
    bob.as_ref(),
    GraphNameRef::DefaultGraph
);
```

### 4. Validate IRIs Early

```rust
// This can fail, so handle it early
let node = NamedNode::new("not a valid IRI");
match node {
    Ok(n) => println!("Valid: {}", n),
    Err(e) => eprintln!("Invalid IRI: {}", e),
}
```

## Summary

You've learned:

- ✅ The RDF triple model (subject, predicate, object)
- ✅ How to create Named Nodes (IRIs)
- ✅ How to work with Blank Nodes
- ✅ How to create different types of Literals
- ✅ How to build Triples and Quads
- ✅ How to use Graphs and Datasets
- ✅ How to filter and query RDF data

## Next Steps

- **SPARQL Queries**: Learn how to query your RDF data with powerful SPARQL queries in the [SPARQL Queries Tutorial](./rust-sparql-queries.md)
- **API Documentation**: Explore the [oxigraph API docs](https://docs.rs/oxigraph) for more details
- **RDF Standards**: Read the [RDF 1.1 Primer](https://www.w3.org/TR/rdf11-primer/) to understand RDF in depth

Continue building your knowledge graph skills with SPARQL!
