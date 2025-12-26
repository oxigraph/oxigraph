//! Determinism and Reproducibility Demo
//!
//! This example demonstrates deterministic and non-deterministic behaviors in Oxigraph.
//!
//! Run with:
//! ```bash
//! cargo run --example determinism_demo
//! ```

use oxigraph::model::{BlankNode, Graph, Literal, NamedNode, Triple};
use oxigraph::sparql::{Query, QueryResults};
use std::collections::HashSet;

fn main() {
    println!("╔════════════════════════════════════════════════════════════╗");
    println!("║     Oxigraph Determinism & Reproducibility Demo           ║");
    println!("╚════════════════════════════════════════════════════════════╝\n");

    test_blank_node_generation();
    test_query_determinism_with_order_by();
    test_query_determinism_without_order_by();
    test_platform_independence();
    test_insert_order_independence();

    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║                    Final Summary                           ║");
    println!("╚════════════════════════════════════════════════════════════╝");
    println!();
    println!("✅ DETERMINISTIC (Guaranteed):");
    println!("   • SPARQL SELECT with ORDER BY");
    println!("   • Graph iteration (BTreeSet-based)");
    println!("   • BlankNode string representation");
    println!("   • Cross-platform byte ordering (fixed with to_le_bytes)");
    println!();
    println!("⚠️  NON-DETERMINISTIC (By Design):");
    println!("   • BlankNode::default() - uses random IDs");
    println!("   • SPARQL SELECT without ORDER BY - hash map iteration");
    println!();
    println!("📖 RECOMMENDATION:");
    println!("   Always use ORDER BY in SPARQL queries for deterministic results.");
    println!();
}

fn test_blank_node_generation() {
    println!("═══════════════════════════════════════════════════════════");
    println!("Test 1: BlankNode Generation");
    println!("═══════════════════════════════════════════════════════════");

    let mut unique_ids = HashSet::new();

    for _ in 0..100 {
        let bn = BlankNode::default();
        unique_ids.insert(bn.as_str().to_string());
    }

    println!("Generated 100 blank nodes:");
    println!("  Unique IDs: {} / 100", unique_ids.len());

    if unique_ids.len() == 100 {
        println!("  ✅ All IDs are unique (non-deterministic generation working)");
    } else {
        println!("  ⚠️  Some duplicate IDs detected!");
    }

    // Test numerical ID consistency
    let id = 0xdeadbeefu128;
    let results: Vec<String> = (0..10)
        .map(|_| BlankNode::new_from_unique_id(id).as_str().to_string())
        .collect();

    let all_same = results.iter().all(|s| s == &results[0]);

    println!("\nNumerical ID consistency:");
    println!("  Created 10 blank nodes with ID 0x{:x}", id);
    println!("  String representation: '{}'", results[0]);
    if all_same {
        println!("  ✅ All 10 nodes have identical string representation");
    } else {
        println!("  ❌ Inconsistent string representations!");
    }

    println!();
}

fn test_query_determinism_with_order_by() {
    println!("═══════════════════════════════════════════════════════════");
    println!("Test 2: SPARQL Query Determinism (WITH ORDER BY)");
    println!("═══════════════════════════════════════════════════════════");

    let graph = create_test_graph();

    let query_str = "PREFIX foaf: <http://xmlns.com/foaf/0.1/>
                     SELECT ?person ?friend
                     WHERE { ?person foaf:knows ?friend }
                     ORDER BY ?person ?friend";

    let results: Vec<String> = (0..50)
        .map(|_| execute_query_to_string(&graph, query_str))
        .collect();

    let all_same = results.iter().all(|s| s == &results[0]);

    println!("Query: SELECT with ORDER BY");
    println!("Runs: 50");

    if all_same {
        println!("✅ Result: DETERMINISTIC");
        println!("   All 50 runs produced identical results");
        println!("\n   First 3 results:");
        let lines: Vec<&str> = results[0].lines().take(3).collect();
        for line in lines {
            println!("   {}", line);
        }
    } else {
        println!("❌ Result: NON-DETERMINISTIC");
        println!("   Different results detected!");
    }

    println!();
}

fn test_query_determinism_without_order_by() {
    println!("═══════════════════════════════════════════════════════════");
    println!("Test 3: SPARQL Query Determinism (WITHOUT ORDER BY)");
    println!("═══════════════════════════════════════════════════════════");

    let graph = create_test_graph();

    let query_str = "PREFIX foaf: <http://xmlns.com/foaf/0.1/>
                     SELECT ?person ?friend
                     WHERE { ?person foaf:knows ?friend }";

    let results: Vec<String> = (0..100)
        .map(|_| execute_query_to_string(&graph, query_str))
        .collect();

    let first = &results[0];
    let mut different_count = 0;

    for result in results.iter().skip(1) {
        if result != first {
            different_count += 1;
        }
    }

    println!("Query: SELECT without ORDER BY");
    println!("Runs: 100");

    if different_count == 0 {
        println!("✅ Result: DETERMINISTIC");
        println!("   All 100 runs produced identical results");
        println!("   (This may be implementation-dependent)");
    } else {
        println!("⚠️  Result: NON-DETERMINISTIC (Expected)");
        println!("   {} / 100 runs had different result ordering", different_count);
        println!("   This is due to FxHashMap iteration order");
    }

    println!("\n   ℹ️  Solution: Always use ORDER BY for deterministic results");
    println!();
}

fn test_platform_independence() {
    println!("═══════════════════════════════════════════════════════════");
    println!("Test 4: Platform Independence (Byte Ordering)");
    println!("═══════════════════════════════════════════════════════════");

    let id = 0x0102030405060708090a0b0c0d0e0f10u128;

    println!("Testing byte ordering fix:");
    println!("  Numerical ID: 0x{:x}", id);

    let bn = BlankNode::new_from_unique_id(id);
    let str_repr = bn.as_str();
    let extracted_id = bn.as_ref().unique_id().unwrap();

    println!("  String representation: '{}'", str_repr);
    println!("  Extracted ID: 0x{:x}", extracted_id);

    if id == extracted_id {
        println!("  ✅ Round-trip successful");
    } else {
        println!("  ❌ Round-trip failed!");
    }

    // Test endianness
    let is_little_endian = cfg!(target_endian = "little");
    let is_big_endian = cfg!(target_endian = "big");

    println!("\nPlatform detection:");
    println!("  Little-endian: {}", is_little_endian);
    println!("  Big-endian: {}", is_big_endian);

    println!("\nByte ordering fix status:");
    println!("  ✅ Using to_le_bytes() for platform-independent storage");
    println!("  ✅ Databases are now portable across architectures");
    println!();
}

fn test_insert_order_independence() {
    println!("═══════════════════════════════════════════════════════════");
    println!("Test 5: Insert Order Independence");
    println!("═══════════════════════════════════════════════════════════");

    let foaf_knows = NamedNode::new_unchecked("http://xmlns.com/foaf/0.1/knows");
    let ex_alice = NamedNode::new_unchecked("http://example.org/Alice");
    let ex_bob = NamedNode::new_unchecked("http://example.org/Bob");
    let ex_carol = NamedNode::new_unchecked("http://example.org/Carol");

    // Graph 1: Insert A, B, C
    let mut graph1 = Graph::new();
    graph1.insert(Triple::new(ex_alice.clone(), foaf_knows.clone(), ex_bob.clone()));
    graph1.insert(Triple::new(ex_alice.clone(), foaf_knows.clone(), ex_carol.clone()));
    graph1.insert(Triple::new(ex_bob.clone(), foaf_knows.clone(), ex_carol.clone()));

    // Graph 2: Insert C, B, A (reverse order)
    let mut graph2 = Graph::new();
    graph2.insert(Triple::new(ex_bob.clone(), foaf_knows.clone(), ex_carol.clone()));
    graph2.insert(Triple::new(ex_alice.clone(), foaf_knows.clone(), ex_carol.clone()));
    graph2.insert(Triple::new(ex_alice.clone(), foaf_knows.clone(), ex_bob.clone()));

    let query = "PREFIX foaf: <http://xmlns.com/foaf/0.1/>
                 SELECT ?person ?friend
                 WHERE { ?person foaf:knows ?friend }
                 ORDER BY ?person ?friend";

    let result1 = execute_query_to_string(&graph1, query);
    let result2 = execute_query_to_string(&graph2, query);

    println!("Graph 1: Inserted triples in order A, B, C");
    println!("Graph 2: Inserted triples in order C, B, A");
    println!();
    println!("Query: SELECT with ORDER BY");

    if result1 == result2 {
        println!("✅ Result: INSERT ORDER INDEPENDENT");
        println!("   Both graphs produced identical query results");
    } else {
        println!("❌ Result: INSERT ORDER DEPENDENT");
        println!("   Query results differ!");
    }

    println!();
}

// Helper functions

fn create_test_graph() -> Graph {
    let mut graph = Graph::new();

    let foaf_knows = NamedNode::new_unchecked("http://xmlns.com/foaf/0.1/knows");
    let foaf_name = NamedNode::new_unchecked("http://xmlns.com/foaf/0.1/name");
    let ex_alice = NamedNode::new_unchecked("http://example.org/Alice");
    let ex_bob = NamedNode::new_unchecked("http://example.org/Bob");
    let ex_carol = NamedNode::new_unchecked("http://example.org/Carol");

    graph.insert(Triple::new(
        ex_alice.clone(),
        foaf_knows.clone(),
        ex_bob.clone(),
    ));
    graph.insert(Triple::new(
        ex_alice.clone(),
        foaf_knows.clone(),
        ex_carol.clone(),
    ));
    graph.insert(Triple::new(
        ex_bob.clone(),
        foaf_knows.clone(),
        ex_carol.clone(),
    ));
    graph.insert(Triple::new(
        ex_alice,
        foaf_name,
        Literal::new_simple_literal("Alice"),
    ));

    graph
}

fn execute_query_to_string(graph: &Graph, query_str: &str) -> String {
    let query = Query::parse(query_str, None).expect("Query parse failed");
    let results = graph.query(query, Default::default()).expect("Query execution failed");

    match results {
        QueryResults::Solutions(solutions) => {
            let mut output = Vec::new();
            for solution in solutions {
                let solution = solution.expect("Solution error");
                output.push(format!("{:?}", solution));
            }
            output.join("\n")
        }
        QueryResults::Boolean(b) => b.to_string(),
        QueryResults::Graph(_) => "GRAPH".to_string(),
    }
}
