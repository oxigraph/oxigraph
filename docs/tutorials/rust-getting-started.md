# Getting Started with Oxigraph in Rust

This tutorial will help you get started with Oxigraph, a fast and compliant RDF graph database for Rust. By the end of this tutorial, you'll know how to create a store, add data, and query it using SPARQL.

## Prerequisites

Before starting, make sure you have:

- **Rust installed** (version 1.70 or later recommended)
  ```bash
  # Check your Rust version
  rustc --version

  # If you need to install Rust, visit https://rustup.rs/
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

- **Cargo** (comes with Rust)
  ```bash
  cargo --version
  ```

## Creating a New Project

Let's create a new Rust project to work with Oxigraph:

```bash
cargo new my-oxigraph-app
cd my-oxigraph-app
```

## Adding Oxigraph to Your Project

Open `Cargo.toml` and add Oxigraph as a dependency:

```toml
[package]
name = "my-oxigraph-app"
version = "0.1.0"
edition = "2021"

[dependencies]
oxigraph = "0.4"
```

For the latest version, check [crates.io/crates/oxigraph](https://crates.io/crates/oxigraph).

### Optional: In-Memory Only

If you only need an in-memory database (no persistent storage), disable the RocksDB feature:

```toml
[dependencies]
oxigraph = { version = "0.4", default-features = false }
```

## Your First Oxigraph Program

Let's create a simple program that creates a store, adds a triple, and queries it.

Open `src/main.rs` and replace its contents with:

```rust
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create an in-memory store
    let store = Store::new()?;

    println!("Store created successfully!");

    Ok(())
}
```

Run it:

```bash
cargo run
```

You should see: `Store created successfully!`

## Adding Your First Triple

Let's add some data to the store. We'll create a simple triple stating "Alice knows Bob":

```rust
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create an in-memory store
    let store = Store::new()?;

    // Create IRIs (named nodes)
    let alice = NamedNode::new("http://example.org/alice")?;
    let bob = NamedNode::new("http://example.org/bob")?;
    let knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;

    // Create a triple: Alice knows Bob
    let triple = Quad::new(
        alice.clone(),
        knows.clone(),
        bob.clone(),
        GraphName::DefaultGraph
    );

    // Insert the triple into the store
    store.insert(&triple)?;

    println!("Triple added: {} {} {}", alice, knows, bob);

    Ok(())
}
```

Run it:

```bash
cargo run
```

## Querying Data with SPARQL

Now let's query the data we just added using SPARQL:

```rust
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create an in-memory store
    let store = Store::new()?;

    // Add data
    let alice = NamedNode::new("http://example.org/alice")?;
    let bob = NamedNode::new("http://example.org/bob")?;
    let knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;

    store.insert(&Quad::new(
        alice.clone(),
        knows.clone(),
        bob.clone(),
        GraphName::DefaultGraph
    ))?;

    // Query the data
    let query = "
        SELECT ?person ?friend WHERE {
            ?person <http://xmlns.com/foaf/0.1/knows> ?friend .
        }
    ";

    // Execute the query
    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query(query)?
        .on_store(&store)
        .execute()?
    {
        println!("Query results:");
        while let Some(solution) = solutions.next() {
            let solution = solution?;
            println!(
                "  {} knows {}",
                solution.get("person").unwrap(),
                solution.get("friend").unwrap()
            );
        }
    }

    Ok(())
}
```

Output:
```
Query results:
  <http://example.org/alice> knows <http://example.org/bob>
```

## Complete Working Example

Here's a complete example that demonstrates the core functionality:

```rust
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Oxigraph Getting Started Demo ===\n");

    // 1. Create a store
    let store = Store::new()?;
    println!("✓ Store created");

    // 2. Create some RDF nodes
    let alice = NamedNode::new("http://example.org/alice")?;
    let bob = NamedNode::new("http://example.org/bob")?;
    let charlie = NamedNode::new("http://example.org/charlie")?;
    let knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;
    let name = NamedNode::new("http://xmlns.com/foaf/0.1/name")?;

    // 3. Add triples to the store
    store.insert(&Quad::new(
        alice.clone(),
        knows.clone(),
        bob.clone(),
        GraphName::DefaultGraph
    ))?;

    store.insert(&Quad::new(
        alice.clone(),
        knows.clone(),
        charlie.clone(),
        GraphName::DefaultGraph
    ))?;

    store.insert(&Quad::new(
        alice.clone(),
        name.clone(),
        Literal::new_simple_literal("Alice"),
        GraphName::DefaultGraph
    ))?;

    store.insert(&Quad::new(
        bob.clone(),
        name.clone(),
        Literal::new_simple_literal("Bob"),
        GraphName::DefaultGraph
    ))?;

    println!("✓ Data inserted\n");

    // 4. Query with SPARQL
    let query = r#"
        PREFIX foaf: <http://xmlns.com/foaf/0.1/>

        SELECT ?personName ?friendName WHERE {
            ?person foaf:knows ?friend .
            ?person foaf:name ?personName .
            ?friend foaf:name ?friendName .
        }
    "#;

    println!("Running SPARQL query...");
    if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query(query)?
        .on_store(&store)
        .execute()?
    {
        println!("\nResults:");
        println!("--------------------");
        while let Some(solution) = solutions.next() {
            let solution = solution?;
            println!(
                "{} knows {}",
                solution.get("personName").unwrap(),
                solution.get("friendName").unwrap()
            );
        }
    }

    Ok(())
}
```

Expected output:
```
=== Oxigraph Getting Started Demo ===

✓ Store created
✓ Data inserted

Running SPARQL query...

Results:
--------------------
"Alice" knows "Bob"
```

## Using Persistent Storage

To persist your data to disk using RocksDB:

```rust
use oxigraph::store::Store;
use oxigraph::io::RdfFormat;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create or open a store at the specified path
    let store = Store::open("./my_data")?;

    // Now use the store as before...
    // Data will be persisted to the "./my_data" directory

    // Example: Load some data
    let data = r#"<http://example.com/s> <http://example.com/p> "value" ."#;
    store.load_from_reader(RdfFormat::Turtle, data.as_bytes())?;

    Ok(())
}
```

Note: Persistent storage requires the `rocksdb` feature (enabled by default).

## Loading Data from Files

Oxigraph can load RDF data from various formats (Turtle, RDF/XML, N-Triples, etc.):

```rust
use oxigraph::io::RdfFormat;
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;

    // Sample Turtle data
    let data = r#"
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix ex: <http://example.org/> .

ex:alice foaf:name "Alice" ;
         foaf:knows ex:bob .

ex:bob foaf:name "Bob" .
    "#;

    // Load the data
    store.load_from_reader(RdfFormat::Turtle, data.as_bytes())?;

    println!("Data loaded successfully!");

    Ok(())
}
```

## Next Steps

Now that you've learned the basics:

1. **Learn more about RDF**: Read the [RDF Basics Tutorial](./rust-rdf-basics.md) to understand the RDF data model in depth
2. **Master SPARQL**: Check out the [SPARQL Queries Tutorial](./rust-sparql-queries.md) for advanced query techniques
3. **Explore the API docs**: Run `cargo doc --open` to see detailed API documentation
4. **Join the community**: Visit [GitHub Discussions](https://github.com/oxigraph/oxigraph/discussions) or [Gitter chat](https://gitter.im/oxigraph/community)

## Common Patterns

### Error Handling

Use `?` operator for simple error propagation:

```rust
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;
    let node = NamedNode::new("http://example.org/test")?;
    store.insert(&Quad::new(
        node.clone(),
        node.clone(),
        node,
        GraphName::DefaultGraph
    ))?;
    Ok(())
}
```

### Checking if Data Exists

```rust
let quad = Quad::new(alice, knows, bob, GraphName::DefaultGraph);
if store.contains(&quad)? {
    println!("Triple exists!");
}
```

### Counting Triples

```rust
let count = store.len()?;
println!("Store contains {} quads", count);
```

## Troubleshooting

### Compilation Errors

If you see linking errors related to RocksDB:
- On Linux: Install `libclang-dev` and `build-essential`
- On macOS: Install Xcode Command Line Tools
- On Windows: Install Visual Studio Build Tools

### Memory Issues

For very large datasets:
- Use persistent storage instead of in-memory
- Consider using bulk loading for initial data import
- Use query limits and pagination

## Summary

You've learned how to:

- ✅ Set up Oxigraph in a Rust project
- ✅ Create an in-memory or persistent store
- ✅ Add triples to the store
- ✅ Query data using SPARQL
- ✅ Load data from RDF files

Continue to the next tutorial to learn more about the RDF data model and how to work with different types of RDF nodes!
