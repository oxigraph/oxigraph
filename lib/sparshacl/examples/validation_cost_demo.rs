//! Demonstration of SHACL validation cost characteristics.
//!
//! This example shows concrete measurements of validation scaling behavior
//! to help understand production implications.
//!
//! Run with: cargo run -p sparshacl --example validation_cost_demo

use oxrdf::{Graph, Literal, NamedNode, Triple, vocab::{rdf, xsd}};
use sparshacl::{ShaclValidator, ShapesGraph};
use std::time::Instant;

fn create_shape() -> ShapesGraph {
    let mut shapes_graph = Graph::new();
    let shape = NamedNode::new("http://example.org/PersonShape").unwrap();
    let person_class = NamedNode::new("http://example.org/Person").unwrap();
    let name_prop = NamedNode::new("http://example.org/name").unwrap();
    let prop_shape = oxrdf::BlankNode::default();

    let shacl_node_shape = NamedNode::new("http://www.w3.org/ns/shacl#NodeShape").unwrap();
    let shacl_target_class = NamedNode::new("http://www.w3.org/ns/shacl#targetClass").unwrap();
    let shacl_property = NamedNode::new("http://www.w3.org/ns/shacl#property").unwrap();
    let shacl_path = NamedNode::new("http://www.w3.org/ns/shacl#path").unwrap();
    let shacl_min_count = NamedNode::new("http://www.w3.org/ns/shacl#minCount").unwrap();

    shapes_graph.insert(&Triple::new(shape.clone(), rdf::TYPE, shacl_node_shape));
    shapes_graph.insert(&Triple::new(shape.clone(), shacl_target_class, person_class));
    shapes_graph.insert(&Triple::new(shape.clone(), shacl_property, prop_shape.clone()));
    shapes_graph.insert(&Triple::new(prop_shape.clone(), shacl_path, name_prop));
    shapes_graph.insert(&Triple::new(
        prop_shape,
        shacl_min_count,
        Literal::new_typed_literal("1", xsd::INTEGER),
    ));

    ShapesGraph::from_graph(&shapes_graph).unwrap()
}

fn create_graph(person_count: usize, thing_count: usize) -> Graph {
    let mut graph = Graph::new();
    let person_class = NamedNode::new("http://example.org/Person").unwrap();
    let thing_class = NamedNode::new("http://example.org/Thing").unwrap();
    let name_prop = NamedNode::new("http://example.org/name").unwrap();
    let value_prop = NamedNode::new("http://example.org/value").unwrap();

    for i in 0..person_count {
        let person = NamedNode::new(format!("http://example.org/person{}", i)).unwrap();
        graph.insert(&Triple::new(person.clone(), rdf::TYPE, person_class.clone()));
        graph.insert(&Triple::new(
            person,
            name_prop.clone(),
            Literal::new_simple_literal(format!("Person {}", i)),
        ));
    }

    for i in 0..thing_count {
        let thing = NamedNode::new(format!("http://example.org/thing{}", i)).unwrap();
        graph.insert(&Triple::new(thing.clone(), rdf::TYPE, thing_class.clone()));
        graph.insert(&Triple::new(
            thing,
            value_prop.clone(),
            Literal::new_simple_literal(format!("Thing {}", i)),
        ));
    }

    graph
}

fn main() {
    println!("╔═══════════════════════════════════════════════════════════════════╗");
    println!("║        SHACL Validation Cost Demonstration                       ║");
    println!("╚═══════════════════════════════════════════════════════════════════╝");
    println!();

    let shapes = create_shape();
    let validator = ShaclValidator::new(shapes);

    println!("Shape Configuration:");
    println!("  - Targets: ex:Person instances");
    println!("  - Constraint: sh:minCount 1 on ex:name property");
    println!();

    // ========================================================================
    // Experiment 1: Does validation scale with graph size or affected nodes?
    // ========================================================================

    println!("═══════════════════════════════════════════════════════════════════");
    println!("Experiment 1: Scaling with Graph Size vs Affected Nodes");
    println!("═══════════════════════════════════════════════════════════════════");
    println!();
    println!("Setup: Constant 10 Person instances, variable Thing instances");
    println!("Question: Does adding non-targeted data affect validation time?");
    println!();

    let configs = vec![
        (10, 0, "Baseline"),
        (10, 10_000, "10K noise triples"),
        (10, 100_000, "100K noise triples"),
    ];

    println!("{:<8} {:<10} {:<15} {:<15}", "Persons", "Things", "Total Triples", "Time (ms)");
    println!("{}", "─".repeat(60));

    let mut baseline_time = 0.0;
    for (i, (person_count, thing_count, label)) in configs.iter().enumerate() {
        let graph = create_graph(*person_count, *thing_count);
        let total_triples = graph.len();

        let start = Instant::now();
        let report = validator.validate(&graph).unwrap();
        let duration = start.elapsed();
        let time_ms = duration.as_secs_f64() * 1000.0;

        if i == 0 {
            baseline_time = time_ms;
        }

        println!(
            "{:<8} {:<10} {:<15} {:<15.3}  {}",
            person_count, thing_count, total_triples, time_ms, label
        );

        assert!(report.conforms(), "Expected valid data");
    }

    println!();
    let final_config = configs.last().unwrap();
    let final_graph = create_graph(final_config.0, final_config.1);
    let start = Instant::now();
    validator.validate(&final_graph).unwrap();
    let final_time = start.elapsed().as_secs_f64() * 1000.0;

    let overhead = ((final_time / baseline_time) - 1.0) * 100.0;

    println!("Result:");
    if overhead < 50.0 {
        println!("  ✓ Adding 100K noise triples increased validation time by {:.1}%", overhead);
        println!("  ✓ Validation primarily scales with affected nodes, not total size");
    } else {
        println!("  ✗ Adding 100K noise triples increased validation time by {:.1}%", overhead);
        println!("  ✗ Validation cost includes graph scanning overhead");
    }
    println!();

    // ========================================================================
    // Experiment 2: Incremental validation
    // ========================================================================

    println!("═══════════════════════════════════════════════════════════════════");
    println!("Experiment 2: Incremental Validation Support");
    println!("═══════════════════════════════════════════════════════════════════");
    println!();
    println!("Scenario: Large graph + 1 new triple");
    println!("Question: Can we validate only the new triple?");
    println!();

    let mut large_graph = create_graph(100, 100_000);
    println!("Base graph: {} triples", large_graph.len());

    let start = Instant::now();
    validator.validate(&large_graph).unwrap();
    let full_time = start.elapsed();
    println!("Full validation: {:.3} ms", full_time.as_secs_f64() * 1000.0);

    // Add 1 new person
    let new_person = NamedNode::new("http://example.org/newPerson").unwrap();
    large_graph.insert(&Triple::new(
        new_person.clone(),
        rdf::TYPE,
        NamedNode::new("http://example.org/Person").unwrap(),
    ));
    large_graph.insert(&Triple::new(
        new_person,
        NamedNode::new("http://example.org/name").unwrap(),
        Literal::new_simple_literal("New Person"),
    ));

    println!("Added: 1 new Person (2 triples)");

    let start = Instant::now();
    validator.validate(&large_graph).unwrap();
    let incremental_time = start.elapsed();
    println!("Re-validation: {:.3} ms", incremental_time.as_secs_f64() * 1000.0);

    println!();
    println!("Result:");
    println!("  ✗ No incremental validation support");
    println!("  ✗ Adding 1 triple requires full re-validation");
    println!("  ✗ Time ratio: {:.1}%", (incremental_time.as_secs_f64() / full_time.as_secs_f64()) * 100.0);
    println!();

    // ========================================================================
    // Experiment 3: Per-node validation cost
    // ========================================================================

    println!("═══════════════════════════════════════════════════════════════════");
    println!("Experiment 3: Per-Node Validation Cost");
    println!("═══════════════════════════════════════════════════════════════════");
    println!();
    println!("Measure: Time to validate each affected node");
    println!();

    println!("{:<12} {:<15} {:<20}", "Node Count", "Total Time (ms)", "Time/Node (μs)");
    println!("{}", "─".repeat(50));

    for person_count in [10, 100, 1_000, 10_000] {
        let graph = create_graph(person_count, 0);

        let start = Instant::now();
        validator.validate(&graph).unwrap();
        let duration = start.elapsed();

        let total_time_ms = duration.as_secs_f64() * 1000.0;
        let time_per_node_us = (duration.as_micros() as f64) / (person_count as f64);

        println!("{:<12} {:<15.3} {:<20.2}", person_count, total_time_ms, time_per_node_us);
    }

    println!();
    println!("Result:");
    println!("  Per-node cost should be roughly constant if validation is efficient");
    println!();

    // ========================================================================
    // Summary and Production Implications
    // ========================================================================

    println!("═══════════════════════════════════════════════════════════════════");
    println!("SUMMARY: Production Implications");
    println!("═══════════════════════════════════════════════════════════════════");
    println!();
    println!("Findings:");
    println!("  1. Target Discovery: Requires scanning type triples (rdf:type)");
    println!("  2. Validation: Scales primarily with affected node count");
    println!("  3. Incremental: NOT SUPPORTED - every validation is full scan");
    println!();
    println!("Admission Control Requirements:");
    println!("  ✓ Shape complexity: Check constraint count and path depth");
    println!("  ✓ Target scope: Limit targetClass to bounded sets");
    println!("  ✗ Incremental validation: Cannot optimize for small updates");
    println!();
    println!("Cost Model:");
    println!("  validation_cost = O(target_discovery + affected_nodes × validation_per_node)");
    println!("  where:");
    println!("    - target_discovery ≈ O(type_triples) for targetClass");
    println!("    - affected_nodes = number of matching targets");
    println!("    - validation_per_node = shape complexity × avg_degree");
    println!();
    println!("Recommendations:");
    println!("  1. Use targetNode for known entities (O(1) discovery)");
    println!("  2. Limit targetClass to small, indexed classes");
    println!("  3. Bound shape complexity (max constraints, path depth)");
    println!("  4. Consider pre-validation before large inserts");
    println!("  5. Monitor validation time as graph grows");
    println!();
}
