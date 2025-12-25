//! Basic Store Operations Example
//!
//! This example demonstrates fundamental operations with Oxigraph Store:
//! - Creating in-memory and persistent stores
//! - Inserting and querying RDF data
//! - Using transactions for atomic updates
//! - Bulk loading data for better performance
//!
//! Run with: cargo run -p oxigraph --example basic_store

use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::*;
use oxigraph::model::vocab::xsd;
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Oxigraph Basic Store Operations ===\n");

    // Example 1: Create an in-memory store
    in_memory_store_example()?;

    // Example 2: Create a persistent store (commented out to avoid file creation)
    // persistent_store_example()?;

    // Example 3: Insert and query data
    insert_and_query_example()?;

    // Example 4: Using transactions
    transaction_example()?;

    // Example 5: Bulk loading
    bulk_loading_example()?;

    // Example 6: Named graphs
    named_graphs_example()?;

    println!("\n=== All examples completed successfully! ===");
    Ok(())
}

/// Example 1: Creating an in-memory store
fn in_memory_store_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 1: In-Memory Store ---");

    // Create a new in-memory store
    let store = Store::new()?;
    println!("✓ Created in-memory store");

    // Insert a simple quad
    let ex = NamedNode::new("http://example.com")?;
    let name = NamedNode::new("http://schema.org/name")?;
    let alice = Literal::new_simple_literal("Alice");

    let quad = Quad::new(
        ex.clone(),
        name.clone(),
        alice.clone(),
        GraphName::DefaultGraph,
    );
    store.insert(&quad)?;
    println!("✓ Inserted quad: {:?}", quad);

    // Check if the quad exists
    assert!(store.contains(&quad)?);
    println!("✓ Verified quad exists in store");

    // Count quads in store
    let count = store.len()?;
    println!("✓ Store contains {} quads\n", count);

    Ok(())
}

/// Example 2: Creating a persistent store
#[allow(dead_code)]
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
fn persistent_store_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 2: Persistent Store ---");

    // Create or open a persistent store
    let store = Store::open("./example_data")?;
    println!("✓ Opened persistent store at ./example_data");

    // Data persists across restarts
    let ex = NamedNode::new("http://example.com/persistent")?;
    let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), GraphName::DefaultGraph);
    store.insert(&quad)?;
    println!("✓ Inserted quad (will persist on disk)");

    // Open read-only store (if you need concurrent readers)
    // let readonly = Store::open_read_only("./example_data")?;
    // println!("✓ Can open read-only views");

    println!();
    Ok(())
}

/// Example 3: Insert and query data
fn insert_and_query_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 3: Insert and Query ---");

    let store = Store::new()?;

    // Define some vocabulary
    let person_type = NamedNode::new("http://schema.org/Person")?;
    let name = NamedNode::new("http://schema.org/name")?;
    let age = NamedNode::new("http://schema.org/age")?;
    let rdf_type = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?;

    // Insert some people
    let alice = NamedNode::new("http://example.com/alice")?;
    store.insert(QuadRef::new(
        &alice,
        &rdf_type,
        &person_type,
        GraphNameRef::DefaultGraph,
    ))?;
    store.insert(QuadRef::new(
        &alice,
        &name,
        LiteralRef::new_simple_literal("Alice"),
        GraphNameRef::DefaultGraph,
    ))?;
    store.insert(QuadRef::new(
        &alice,
        &age,
        LiteralRef::new_typed_literal("30", xsd::INTEGER),
        GraphNameRef::DefaultGraph,
    ))?;

    let bob = NamedNode::new("http://example.com/bob")?;
    store.insert(QuadRef::new(
        &bob,
        &rdf_type,
        &person_type,
        GraphNameRef::DefaultGraph,
    ))?;
    store.insert(QuadRef::new(
        &bob,
        &name,
        LiteralRef::new_simple_literal("Bob"),
        GraphNameRef::DefaultGraph,
    ))?;
    store.insert(QuadRef::new(
        &bob,
        &age,
        LiteralRef::new_typed_literal("25", xsd::INTEGER),
        GraphNameRef::DefaultGraph,
    ))?;

    println!("✓ Inserted data about Alice and Bob");

    // Query all quads
    let all_quads: Vec<_> = store.quads_for_pattern(None, None, None, None).collect();
    println!("✓ Total quads in store: {}", all_quads.len());

    // Query specific pattern: all triples about alice
    let alice_quads: Result<Vec<_>, _> = store
        .quads_for_pattern(Some(alice.as_ref().into()), None, None, None)
        .collect();
    println!("✓ Quads about Alice: {}", alice_quads?.len());

    // Query by predicate: all names
    let name_quads: Result<Vec<_>, _> = store
        .quads_for_pattern(None, Some(name.as_ref().into()), None, None)
        .collect();
    println!("✓ Name triples found: {}", name_quads?.len());

    // Query all subjects of type Person
    let people: Result<Vec<_>, _> = store
        .quads_for_pattern(None, Some(rdf_type.as_ref().into()), Some(person_type.as_ref().into()), None)
        .collect();
    println!("✓ People in store: {}", people?.len());

    println!();
    Ok(())
}

/// Example 4: Using transactions for atomic updates
fn transaction_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 4: Transactions ---");

    let store = Store::new()?;

    // Transactions ensure atomicity - either all changes succeed or none do
    {
        let mut transaction = store.start_transaction()?;

        let alice = NamedNode::new("http://example.com/alice")?;
        let bob = NamedNode::new("http://example.com/bob")?;
        let knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;

        // Multiple inserts in a single transaction
        transaction.insert(QuadRef::new(
            &alice,
            &knows,
            &bob,
            GraphNameRef::DefaultGraph,
        ));
        transaction.insert(QuadRef::new(
            &bob,
            &knows,
            &alice,
            GraphNameRef::DefaultGraph,
        ));

        println!("✓ Added data to transaction (not yet committed)");

        // Commit the transaction
        transaction.commit()?;
        println!("✓ Transaction committed");
    }

    // Verify data was inserted
    let knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;
    let knows_relations: Result<Vec<_>, _> = store
        .quads_for_pattern(None, Some(knows.as_ref().into()), None, None)
        .collect();
    println!("✓ Verified {} 'knows' relationships in store", knows_relations?.len());

    // Example of transaction rollback (implicit via drop)
    {
        let mut transaction = store.start_transaction()?;
        let charlie = NamedNode::new("http://example.com/charlie")?;
        transaction.insert(QuadRef::new(
            &charlie,
            &knows,
            &charlie,
            GraphNameRef::DefaultGraph,
        ));
        // Transaction dropped without commit - changes are lost
        println!("✓ Created transaction but didn't commit (will rollback)");
    }

    // Verify charlie wasn't added
    let charlie = NamedNode::new("http://example.com/charlie")?;
    let charlie_quads: Result<Vec<_>, _> = store
        .quads_for_pattern(Some(charlie.as_ref().into()), None, None, None)
        .collect();
    assert_eq!(charlie_quads?.len(), 0);
    println!("✓ Verified rollback - Charlie not in store");

    println!();
    Ok(())
}

/// Example 5: Bulk loading for better performance
fn bulk_loading_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 5: Bulk Loading ---");

    let store = Store::new()?;

    // For large datasets, use bulk_loader for much better performance
    let nquads_data = r#"
<http://example.com/book1> <http://purl.org/dc/terms/title> "The Great Gatsby" .
<http://example.com/book1> <http://purl.org/dc/terms/creator> "F. Scott Fitzgerald" .
<http://example.com/book1> <http://purl.org/dc/terms/date> "1925"^^<http://www.w3.org/2001/XMLSchema#gYear> .
<http://example.com/book2> <http://purl.org/dc/terms/title> "1984" .
<http://example.com/book2> <http://purl.org/dc/terms/creator> "George Orwell" .
<http://example.com/book2> <http://purl.org/dc/terms/date> "1949"^^<http://www.w3.org/2001/XMLSchema#gYear> .
<http://example.com/book3> <http://purl.org/dc/terms/title> "To Kill a Mockingbird" .
<http://example.com/book3> <http://purl.org/dc/terms/creator> "Harper Lee" .
<http://example.com/book3> <http://purl.org/dc/terms/date> "1960"^^<http://www.w3.org/2001/XMLSchema#gYear> .
"#;

    println!("✓ Preparing to bulk load {} lines of N-Triples data", nquads_data.lines().count());

    // Use bulk loader - more efficient than individual inserts
    let mut loader = store.bulk_loader();
    loader.load_from_slice(RdfFormat::NTriples, nquads_data.as_bytes())?;
    loader.commit()?;

    println!("✓ Bulk loaded data successfully");

    // Verify the data
    let count = store.len()?;
    println!("✓ Store now contains {} quads", count);

    // You can also load from a parser with custom options
    let turtle_data = r#"
@prefix dc: <http://purl.org/dc/terms/> .
@prefix ex: <http://example.com/> .

ex:book4 dc:title "Brave New World" ;
         dc:creator "Aldous Huxley" ;
         dc:date "1932"^^<http://www.w3.org/2001/XMLSchema#gYear> .
"#;

    let mut loader = store.bulk_loader();
    loader.load_from_reader(
        RdfParser::from_format(RdfFormat::Turtle),
        turtle_data.as_bytes(),
    )?;
    loader.commit()?;

    println!("✓ Loaded additional Turtle data");
    println!("✓ Final store size: {} quads", store.len()?);

    println!();
    Ok(())
}

/// Example 6: Working with named graphs
fn named_graphs_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 6: Named Graphs ---");

    let store = Store::new()?;

    // Named graphs allow you to group triples
    let graph1 = NamedNode::new("http://example.com/graph1")?;
    let graph2 = NamedNode::new("http://example.com/graph2")?;

    let ex = NamedNode::new("http://example.com/subject")?;
    let pred = NamedNode::new("http://example.com/predicate")?;
    let obj = Literal::new_simple_literal("value");

    // Insert into different named graphs
    store.insert(QuadRef::new(
        &ex,
        &pred,
        &obj,
        graph1.as_ref(),
    ))?;

    store.insert(QuadRef::new(
        &ex,
        &pred,
        &obj,
        graph2.as_ref(),
    ))?;

    // Also insert into default graph
    store.insert(QuadRef::new(
        &ex,
        &pred,
        &obj,
        GraphNameRef::DefaultGraph,
    ))?;

    println!("✓ Inserted same triple into 2 named graphs and default graph");

    // Query specific named graph
    let graph1_quads: Result<Vec<_>, _> = store
        .quads_for_pattern(None, None, None, Some(graph1.as_ref().into()))
        .collect();
    println!("✓ Graph1 contains {} quads", graph1_quads?.len());

    // Query default graph
    let default_quads: Result<Vec<_>, _> = store
        .quads_for_pattern(None, None, None, Some(GraphNameRef::DefaultGraph))
        .collect();
    println!("✓ Default graph contains {} quads", default_quads?.len());

    // List all named graphs
    let named_graphs: Vec<_> = store.named_graphs().collect();
    println!("✓ Store has {} named graphs", named_graphs.len());

    // You can also create empty named graphs
    store.insert_named_graph(graph1.as_ref())?;
    println!("✓ Explicitly created named graph (already existed)");

    // And remove all quads from a named graph
    store.clear_graph(graph2.as_ref())?;
    let graph2_quads: Result<Vec<_>, _> = store
        .quads_for_pattern(None, None, None, Some(graph2.as_ref().into()))
        .collect();
    assert_eq!(graph2_quads?.len(), 0);
    println!("✓ Cleared graph2");

    println!();
    Ok(())
}
