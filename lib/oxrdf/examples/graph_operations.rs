//! Example demonstrating Graph and Dataset operations
//!
//! This example shows how to:
//! - Create and populate a Graph
//! - Insert and query triples
//! - Iterate over graph contents
//! - Use pattern-based queries
//! - Work with Datasets (multiple named graphs)
//! - Perform graph operations
//!
//! Run with: cargo run -p oxrdf --example graph_operations

use oxrdf::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Graph and Dataset Operations Example ===\n");

    // ==========================================
    // Creating and Populating a Graph
    // ==========================================
    println!("1. Creating and Populating a Graph:");

    // Create a new empty graph
    let mut graph = Graph::new();

    // Define some RDF terms
    let alice = NamedNode::new("http://example.org/alice")?;
    let bob = NamedNode::new("http://example.org/bob")?;
    let charlie = NamedNode::new("http://example.org/charlie")?;

    let foaf_name = NamedNode::new("http://xmlns.com/foaf/0.1/name")?;
    let foaf_knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;
    let foaf_age = NamedNode::new("http://xmlns.com/foaf/0.1/age")?;
    let rdf_type = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?;
    let foaf_person = NamedNode::new("http://xmlns.com/foaf/0.1/Person")?;

    // Insert triples into the graph
    // Note: insert() returns true if the triple was newly inserted, false if it already existed
    graph.insert(TripleRef::new(&alice, &rdf_type, &foaf_person));
    graph.insert(TripleRef::new(&alice, &foaf_name, LiteralRef::new_simple_literal("Alice")));
    graph.insert(TripleRef::new(&alice, &foaf_age, &Literal::from(30)));
    graph.insert(TripleRef::new(&alice, &foaf_knows, &bob));

    graph.insert(TripleRef::new(&bob, &rdf_type, &foaf_person));
    graph.insert(TripleRef::new(&bob, &foaf_name, LiteralRef::new_simple_literal("Bob")));
    graph.insert(TripleRef::new(&bob, &foaf_age, &Literal::from(25)));
    graph.insert(TripleRef::new(&bob, &foaf_knows, &charlie));

    graph.insert(TripleRef::new(&charlie, &rdf_type, &foaf_person));
    graph.insert(TripleRef::new(&charlie, &foaf_name, LiteralRef::new_simple_literal("Charlie")));
    graph.insert(TripleRef::new(&charlie, &foaf_age, &Literal::from(35)));

    println!("  Graph created with {} triples", graph.len());
    println!();

    // ==========================================
    // Iterating Over All Triples
    // ==========================================
    println!("2. Iterating Over All Triples:");

    println!("  All triples in the graph:");
    for triple in graph.iter() {
        println!("    {}", triple);
    }
    println!();

    // Alternative: use the graph as an iterator directly
    println!("  Using for-in loop:");
    let mut count = 0;
    for _triple in &graph {
        count += 1;
    }
    println!("    Counted {} triples", count);
    println!();

    // ==========================================
    // Querying Triples by Subject
    // ==========================================
    println!("3. Querying Triples by Subject:");

    println!("  All triples about Alice:");
    for triple in graph.triples_for_subject(&alice) {
        println!("    {}", triple);
    }
    println!();

    // ==========================================
    // Querying Triples by Predicate
    // ==========================================
    println!("4. Querying Triples by Predicate:");

    println!("  All 'knows' relationships:");
    for triple in graph.triples_for_predicate(&foaf_knows) {
        println!("    {} knows {}", triple.subject, triple.object);
    }
    println!();

    // ==========================================
    // Querying Specific Values
    // ==========================================
    println!("5. Querying Specific Values:");

    // Get the name of Alice
    if let Some(name) = graph.object_for_subject_predicate(&alice, &foaf_name) {
        if let TermRef::Literal(lit) = name {
            println!("  Alice's name: {}", lit.value());
        }
    }

    // Get all objects for a subject-predicate pair
    println!("  People Alice knows:");
    for object in graph.objects_for_subject_predicate(&alice, &foaf_knows) {
        println!("    {}", object);
    }
    println!();

    // ==========================================
    // Checking for Triple Existence
    // ==========================================
    println!("6. Checking for Triple Existence:");

    let triple_exists = graph.contains(TripleRef::new(&alice, &foaf_knows, &bob));
    println!("  Does Alice know Bob? {}", triple_exists);

    let triple_not_exists = graph.contains(TripleRef::new(&charlie, &foaf_knows, &alice));
    println!("  Does Charlie know Alice? {}", triple_not_exists);
    println!();

    // ==========================================
    // Removing Triples
    // ==========================================
    println!("7. Removing Triples:");

    let before_count = graph.len();
    println!("  Triples before removal: {}", before_count);

    // Remove a specific triple
    let removed = graph.remove(TripleRef::new(&alice, &foaf_knows, &bob));
    println!("  Removed triple? {}", removed);

    let after_count = graph.len();
    println!("  Triples after removal: {}", after_count);
    println!();

    // ==========================================
    // Working with Datasets (Multiple Graphs)
    // ==========================================
    println!("8. Working with Datasets (Multiple Graphs):");

    // Create a new dataset
    let mut dataset = Dataset::new();

    // Define graph names
    let social_graph = NamedNode::new("http://example.org/graphs/social")?;
    let work_graph = NamedNode::new("http://example.org/graphs/work")?;

    // Insert quads into different named graphs
    dataset.insert(QuadRef::new(&alice, &foaf_name, LiteralRef::new_simple_literal("Alice"), &social_graph));
    dataset.insert(QuadRef::new(&alice, &foaf_knows, &bob, &social_graph));
    dataset.insert(QuadRef::new(&bob, &foaf_name, LiteralRef::new_simple_literal("Bob"), &social_graph));

    // Work-related triples in a different graph
    let works_at = NamedNode::new("http://example.org/worksAt")?;
    let company = NamedNode::new("http://example.org/AcmeCorp")?;
    dataset.insert(QuadRef::new(&alice, &works_at, &company, &work_graph));
    dataset.insert(QuadRef::new(&bob, &works_at, &company, &work_graph));

    // Also insert some triples in the default graph
    dataset.insert(QuadRef::new(&alice, &rdf_type, &foaf_person, GraphNameRef::DefaultGraph));

    println!("  Dataset created with {} quads", dataset.len());
    println!();

    // ==========================================
    // Iterating Over Dataset Quads
    // ==========================================
    println!("9. Iterating Over Dataset Quads:");

    println!("  All quads in the dataset:");
    for quad in dataset.iter() {
        println!("    {}", quad);
    }
    println!();

    // ==========================================
    // Accessing Specific Graphs in a Dataset
    // ==========================================
    println!("10. Accessing Specific Graphs in a Dataset:");

    // Get a read-only view of a specific graph
    let social_graph_view = dataset.graph(&social_graph);
    println!("  Triples in social graph:");
    for triple in social_graph_view.iter() {
        println!("    {}", triple);
    }
    println!();

    println!("  Triples in work graph:");
    for triple in dataset.graph(&work_graph).iter() {
        println!("    {}", triple);
    }
    println!();

    // ==========================================
    // Modifying Graphs in a Dataset
    // ==========================================
    println!("11. Modifying Graphs in a Dataset:");

    // Get a mutable view of a graph
    {
        let mut social_graph_mut = dataset.graph_mut(&social_graph);

        // Add a new triple to the graph
        social_graph_mut.insert(TripleRef::new(&bob, &foaf_knows, &charlie));
        social_graph_mut.insert(TripleRef::new(&charlie, &foaf_name, LiteralRef::new_simple_literal("Charlie")));

        println!("  Added triples to social graph");
        println!("  Social graph now has {} triples", social_graph_mut.len());
    }

    println!("  Dataset now has {} quads", dataset.len());
    println!();

    // ==========================================
    // Querying Datasets by Graph Name
    // ==========================================
    println!("12. Querying Datasets by Graph Name:");

    println!("  All quads in the social graph:");
    for quad in dataset.quads_for_graph_name(&social_graph) {
        println!("    {}", quad);
    }
    println!();

    // ==========================================
    // Pattern-Based Queries on Datasets
    // ==========================================
    println!("13. Pattern-Based Queries on Datasets:");

    println!("  All quads with Alice as subject:");
    for quad in dataset.quads_for_subject(&alice) {
        println!("    {}", quad);
    }
    println!();

    println!("  All quads with foaf:name predicate:");
    for quad in dataset.quads_for_predicate(&foaf_name) {
        println!("    {}", quad);
    }
    println!();

    // ==========================================
    // Building Graphs from Iterators
    // ==========================================
    println!("14. Building Graphs from Iterators:");

    // Create a vector of triples
    let triples = vec![
        Triple::new(
            NamedNode::new("http://example.org/subject1")?,
            NamedNode::new("http://example.org/predicate")?,
            Literal::new_simple_literal("object1"),  // Owned literals are fine for Triple
        ),
        Triple::new(
            NamedNode::new("http://example.org/subject2")?,
            NamedNode::new("http://example.org/predicate")?,
            Literal::new_simple_literal("object2"),  // Owned literals are fine for Triple
        ),
    ];

    // Build a graph from the iterator
    let graph_from_iter: Graph = triples.into_iter().collect();
    println!("  Graph built from iterator has {} triples", graph_from_iter.len());

    for triple in &graph_from_iter {
        println!("    {}", triple);
    }
    println!();

    // ==========================================
    // Extending Graphs
    // ==========================================
    println!("15. Extending Graphs:");

    let mut my_graph = Graph::new();
    println!("  Initial graph size: {}", my_graph.len());

    // Add multiple triples at once using extend
    let new_triples = vec![
        TripleRef::new(
            NamedNodeRef::new("http://example.org/s1")?,
            NamedNodeRef::new("http://example.org/p")?,
            LiteralRef::new_simple_literal("o1"),
        ),
        TripleRef::new(
            NamedNodeRef::new("http://example.org/s2")?,
            NamedNodeRef::new("http://example.org/p")?,
            LiteralRef::new_simple_literal("o2"),
        ),
        TripleRef::new(
            NamedNodeRef::new("http://example.org/s3")?,
            NamedNodeRef::new("http://example.org/p")?,
            LiteralRef::new_simple_literal("o3"),
        ),
    ];

    my_graph.extend(new_triples);
    println!("  Graph size after extend: {}", my_graph.len());
    println!();

    // ==========================================
    // Displaying Graphs
    // ==========================================
    println!("16. Displaying Graphs:");

    let display_graph = Graph::new();
    let mut temp_graph = display_graph;
    temp_graph.insert(TripleRef::new(
        NamedNodeRef::new("http://example.org/subject")?,
        NamedNodeRef::new("http://example.org/predicate")?,
        LiteralRef::new_simple_literal("object"),
    ));

    // The Display trait formats the graph in N-Triples-like syntax
    println!("  Graph display:");
    println!("{}", temp_graph);

    // ==========================================
    // Clearing Graphs and Datasets
    // ==========================================
    println!("17. Clearing Graphs and Datasets:");

    let mut clear_graph = Graph::new();
    clear_graph.insert(TripleRef::new(&alice, &foaf_name, LiteralRef::new_simple_literal("Alice")));
    println!("  Graph has {} triples", clear_graph.len());

    clear_graph.clear();
    println!("  After clear, graph has {} triples", clear_graph.len());
    println!("  Is graph empty? {}", clear_graph.is_empty());
    println!();

    // ==========================================
    // Advanced: Blank Nodes in Graphs
    // ==========================================
    println!("18. Advanced: Blank Nodes in Graphs:");

    let mut graph_with_bnodes = Graph::new();

    // Create some blank nodes
    let person1 = BlankNode::default();
    let person2 = BlankNode::default();

    graph_with_bnodes.insert(TripleRef::new(&person1, &foaf_name, LiteralRef::new_simple_literal("Anonymous Person 1")));
    graph_with_bnodes.insert(TripleRef::new(&person1, &foaf_knows, &person2));
    graph_with_bnodes.insert(TripleRef::new(&person2, &foaf_name, LiteralRef::new_simple_literal("Anonymous Person 2")));

    println!("  Graph with blank nodes:");
    for triple in &graph_with_bnodes {
        println!("    {}", triple);
    }
    println!();

    // ==========================================
    // Advanced: Multi-Graph Queries
    // ==========================================
    println!("19. Advanced: Multi-Graph Queries:");

    println!("  Finding all graphs that contain information about Alice:");
    let alice_quads: Vec<_> = dataset.quads_for_subject(&alice).collect();
    let mut graphs_with_alice = std::collections::HashSet::new();

    for quad in alice_quads {
        graphs_with_alice.insert(quad.graph_name.to_string());
    }

    println!("  Alice appears in {} graph(s):", graphs_with_alice.len());
    for graph_name in graphs_with_alice {
        println!("    {}", graph_name);
    }
    println!();

    // ==========================================
    // Performance Tips
    // ==========================================
    println!("20. Performance Tips:");
    println!("  - Use TripleRef and QuadRef for temporary operations to avoid allocations");
    println!("  - Use insert() to check if a triple is new before adding");
    println!("  - Use contains() to check existence without modification");
    println!("  - Iterate directly with for loops for best performance");
    println!("  - Use pattern-based queries (triples_for_subject, etc.) for efficient lookups");
    println!("  - Extend graphs with batches of triples for bulk insertions");
    println!();

    println!("=== Example Complete ===");

    Ok(())
}
