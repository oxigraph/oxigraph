# RDF Fundamentals

This document explains the fundamental concepts of RDF (Resource Description Framework) and how Oxigraph implements these concepts. The goal is to build your mental model of what RDF is and why it matters for representing knowledge.

## What is RDF and Why It Matters

RDF is a framework for representing information about resources in the web. At its core, RDF provides a way to make statements about things in a machine-readable format that can be shared, linked, and reasoned about across different systems.

### The Problem RDF Solves

Traditional databases use rigid schemas where you must define tables and columns upfront. This works well for structured data but becomes limiting when:

- Different systems need to share data without agreeing on a common schema
- Data models need to evolve without breaking existing applications
- You want to merge data from multiple sources with different structures
- Relationships between entities are as important as the entities themselves

RDF solves these problems by providing a flexible, self-describing data model where the schema and data coexist, and where everything can be linked using globally unique identifiers.

### The Graph Perspective

Unlike relational databases that organize data in tables, RDF organizes data as a **graph**. Think of it as a network where:

- **Nodes** represent things (people, places, concepts, values)
- **Edges** represent relationships between those things
- Everything can be connected to everything else

This graph structure mirrors how we naturally think about relationships: "Alice knows Bob", "Paris is in France", "This book was written by that author".

## The RDF Data Model: Triples

### The Triple Structure

The fundamental unit of RDF is the **triple**, a statement with three parts:

```
Subject → Predicate → Object
```

This can be read as: "Subject has a relationship (Predicate) to Object"

Examples:
```
<Alice>        <knows>           <Bob>
<Paris>        <locatedIn>       <France>
<Book#42>      <publishedOn>     "2023-01-15"
```

### Why Triples?

The triple structure is powerful because:

1. **Simplicity**: Every fact is broken down into atomic statements
2. **Composability**: You can combine triples to build complex knowledge
3. **Flexibility**: No predefined schema required
4. **Queryability**: The uniform structure makes pattern matching possible

### How Oxigraph Represents Triples

In Oxigraph, triples are represented by the `Triple` struct in the `oxrdf` crate:

```rust
pub struct Triple {
    subject: Subject,
    predicate: NamedNode,
    object: Term,
}
```

Each triple consists of:
- A **subject** (what you're talking about)
- A **predicate** (the property or relationship)
- An **object** (the value or target of the relationship)

## URIs, IRIs, and Namespaces

### Global Identifiers

RDF uses **IRIs** (Internationalized Resource Identifiers) to uniquely identify resources. An IRI is like a URL but can contain international characters.

Why IRIs matter:
- They're globally unique (no naming conflicts)
- They can be dereferenced to get more information
- They enable linking across different datasets
- They make the web of data possible

Example IRIs:
```
http://example.com/person/alice
http://schema.org/Person
http://www.w3.org/1999/02/22-rdf-syntax-ns#type
```

### Namespaces for Readability

Since IRIs can be long, RDF uses **namespaces** to create shortcuts:

```turtle
@prefix schema: <http://schema.org/> .
@prefix ex: <http://example.com/> .

ex:alice a schema:Person .
```

This is more readable than:
```turtle
<http://example.com/alice> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
```

### NamedNode in Oxigraph

Oxigraph represents IRIs using the `NamedNode` type:

```rust
pub struct NamedNode {
    iri: String,
}
```

Creating a `NamedNode`:
```rust
let person = NamedNode::new("http://schema.org/Person")?;
```

The `NamedNode::new()` method validates that the string is a valid IRI, ensuring data integrity.

## Literals and Datatypes

### What Are Literals?

While IRIs identify resources, **literals** represent actual values like numbers, dates, or text strings. Literals are the "leaf nodes" in your graph - they don't point to other resources.

### Types of Literals

#### 1. Simple Literals (Plain Strings)

Plain text without any type information:
```turtle
ex:message "Hello, World!" .
```

In Oxigraph:
```rust
let literal = Literal::new_simple_literal("Hello, World!");
```

#### 2. Language-Tagged Strings

Text with a language indicator:
```turtle
ex:greeting "Hello"@en .
ex:greeting "Bonjour"@fr .
```

This allows multilingual data where the same property can have values in different languages.

In Oxigraph:
```rust
let greeting_en = Literal::new_language_tagged_literal("Hello", "en")?;
let greeting_fr = Literal::new_language_tagged_literal("Bonjour", "fr")?;
```

#### 3. Typed Literals

Values with a specific datatype:
```turtle
ex:age "42"^^xsd:integer .
ex:price "19.99"^^xsd:decimal .
ex:birthday "1990-05-15"^^xsd:date .
```

The datatype (from XML Schema) tells you how to interpret the value.

In Oxigraph:
```rust
let age = Literal::new_typed_literal("42", xsd::INTEGER);
let date = Literal::new_typed_literal("1990-05-15", xsd::DATE);
```

### Why Datatypes Matter

Datatypes enable:
- **Validation**: Ensure values make sense (e.g., dates are valid)
- **Operations**: Compare, add, or sort values correctly
- **Interoperability**: Different systems interpret values the same way
- **Optimization**: Store and index values efficiently

### Oxigraph's Literal Implementation

The `Literal` struct in Oxigraph handles all three types:

```rust
pub struct Literal(LiteralContent);

enum LiteralContent {
    String(String),                          // Simple literal
    LanguageTaggedString {                   // Language-tagged
        value: String,
        language: String,
    },
    TypedLiteral {                           // Typed literal
        value: String,
        datatype: NamedNode,
    },
}
```

This internal representation is optimized for storage while providing a clean API.

## Blank Nodes

### The Concept

A **blank node** represents an anonymous resource - something that exists but doesn't have a global identifier. Think of it as a local, temporary ID.

Use cases:
- Intermediate objects you don't need to reference elsewhere
- Complex values that are only relevant in one context
- N-ary relationships (relationships with more than two participants)

Example - describing a person's address without giving the address its own IRI:
```turtle
ex:alice schema:address [
    schema:streetAddress "123 Main St" ;
    schema:city "Springfield" ;
    schema:postalCode "12345"
] .
```

The `[ ]` creates a blank node for the address.

### Blank Nodes vs. Named Nodes

**Named nodes** (IRIs):
- Globally identifiable
- Can be referenced from anywhere
- Persistent across datasets

**Blank nodes**:
- Only identifiable within a specific context
- Cannot be reliably referenced from outside
- May get different IDs when data is serialized/deserialized

### When to Use Blank Nodes

Use blank nodes when:
- The resource is not important enough to have its own IRI
- You're modeling intermediate structure
- You need existential quantification ("there exists something")

Avoid blank nodes when:
- The resource might need to be referenced later
- You're merging data from multiple sources
- You need stable, persistent identifiers

### Oxigraph's BlankNode

```rust
pub struct BlankNode {
    id: String,
}
```

Oxigraph automatically generates unique IDs for blank nodes within a dataset, ensuring they don't collide.

## Named Graphs and Quads

### Beyond Triples: The Need for Context

Triples are great for making statements, but they lack context:
- Who said this?
- When was this recorded?
- Is this statement still true?
- Which dataset does this come from?

### Named Graphs

A **named graph** is a set of triples associated with an IRI. The IRI identifies the graph and can have metadata about it:

```turtle
# In the graph http://example.com/graph1
ex:alice schema:age "30" .

# Metadata about the graph
<http://example.com/graph1>
    dc:created "2023-01-15" ;
    dc:source <http://trusted-source.com> .
```

This allows you to:
- Organize data into logical collections
- Track provenance (where data came from)
- Manage access control per graph
- Implement versioning

### Quads: Triples + Context

A **quad** is a triple with a fourth element - the graph name:

```
Subject → Predicate → Object → Graph
```

Example:
```
<Alice> <knows> <Bob> <Graph1>
```

This reads as: "In Graph1, Alice knows Bob"

### Default Graph

The **default graph** is a special graph that contains triples not explicitly assigned to a named graph. You can think of it as the "main" dataset.

### How Oxigraph Implements Quads

The `Quad` struct:

```rust
pub struct Quad {
    subject: Subject,
    predicate: NamedNode,
    object: Term,
    graph_name: GraphName,
}
```

Where `GraphName` can be:
- `DefaultGraph` - the unnamed default graph
- `NamedNode` - a named graph identified by an IRI
- `BlankNode` - an anonymous graph

### Storage Implications

Oxigraph stores all data as quads internally, even when you're working with triples. Triples are simply quads in the default graph:

```rust
// These are equivalent:
store.insert(&triple);
store.insert(&Quad::new(triple.subject, triple.predicate, triple.object, GraphName::DefaultGraph));
```

This unified storage model simplifies implementation while supporting both RDF datasets and simple triple collections.

## How Oxigraph Implements RDF Concepts

### Type Safety

Oxigraph uses Rust's type system to enforce RDF semantics:

- `NamedNode` - Only valid IRIs
- `Literal` - Properly typed values with validation
- `BlankNode` - Unique identifiers within context
- `Triple` - Subject-Predicate-Object structure
- `Quad` - Triple plus graph context

Type errors are caught at compile time, preventing invalid RDF data.

### Memory Efficiency

#### Borrowed vs. Owned Types

Oxigraph provides both owned and borrowed versions of RDF types:

- `NamedNode` and `NamedNodeRef<'a>`
- `Literal` and `LiteralRef<'a>`
- `Triple` and `TripleRef<'a>`

This allows zero-copy operations when possible, improving performance.

#### String Interning

Internally, Oxigraph uses string interning in its storage layer to avoid duplicating common IRIs and values. Each unique string is stored once and referenced by an integer ID.

### Validation

When creating RDF terms, Oxigraph validates:
- IRIs are syntactically correct
- Language tags conform to BCP47
- Datatypes are valid
- Values can be parsed according to their type

This validation happens at construction time, ensuring the database never contains invalid data.

### Collections: Dataset and Graph

Oxigraph provides in-memory collections:

#### Dataset
A collection of quads with fast lookup:

```rust
let mut dataset = Dataset::default();
dataset.insert(quad);
let quads: Vec<_> = dataset.iter().collect();
```

#### Graph
A collection of triples (quads in the default graph):

```rust
let mut graph = Graph::default();
graph.insert(triple);
let triples: Vec<_> = graph.iter().collect();
```

These collections implement standard Rust traits like `IntoIterator`, making them idiomatic to use.

## Common Patterns and Best Practices

### Using the Type System

Leverage Rust's type system:

```rust
// Compiler ensures only valid subjects
fn add_triple(subject: impl Into<Subject>, predicate: NamedNode, object: impl Into<Term>) {
    // ...
}
```

### Converting Between Types

Use `Into<T>` conversions:

```rust
let node = NamedNode::new("http://example.com")?;
let subject: Subject = node.into();
let term: Term = subject.into();
```

### Pattern Matching

Use Rust's pattern matching to handle RDF terms:

```rust
match term {
    Term::NamedNode(node) => println!("IRI: {}", node),
    Term::BlankNode(blank) => println!("Blank: {}", blank),
    Term::Literal(literal) => println!("Value: {}", literal),
    Term::Triple(triple) => println!("RDF-star triple"),
}
```

### Working with Literals

Extract typed values:

```rust
if let Some(int_value) = literal.to_integer() {
    println!("Integer: {}", int_value);
}
```

## RDF-star: Quoting Triples

RDF-star extends RDF to allow statements about statements:

```turtle
<< ex:alice ex:knows ex:bob >> ex:certainty "0.9" .
```

This says "The statement 'Alice knows Bob' has a certainty of 0.9".

Oxigraph supports RDF-star, allowing `Triple` as both subject and object in quads.

## Summary

RDF provides a flexible, graph-based data model built on simple concepts:

- **Triples**: Subject-Predicate-Object statements
- **IRIs**: Globally unique identifiers
- **Literals**: Typed values with datatypes
- **Blank Nodes**: Anonymous local identifiers
- **Quads**: Triples with graph context

Oxigraph implements these concepts with:
- Type-safe Rust structs
- Efficient memory representations
- Validation at construction
- Zero-copy operations where possible

Understanding these fundamentals is essential for working with Oxigraph and building applications on top of semantic technologies.
