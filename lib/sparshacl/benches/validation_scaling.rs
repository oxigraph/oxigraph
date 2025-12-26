//! Benchmark suite for SHACL validation scaling analysis.
//!
//! This benchmark demonstrates whether SHACL validation cost scales with:
//! - Total graph size
//! - Number of affected nodes
//!
//! Run with: cargo bench -p sparshacl validation_scaling

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use oxrdf::{Graph, Literal, NamedNode, Triple, vocab::{rdf, xsd}};
use sparshacl::{ShaclValidator, ShapesGraph};

/// Create a SHACL shape targeting Person class with name constraint.
fn create_person_shape() -> ShapesGraph {
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

/// Create a data graph with specified counts of different entity types.
fn create_mixed_graph(person_count: usize, thing_count: usize) -> Graph {
    let mut graph = Graph::new();
    let person_class = NamedNode::new("http://example.org/Person").unwrap();
    let thing_class = NamedNode::new("http://example.org/Thing").unwrap();
    let name_prop = NamedNode::new("http://example.org/name").unwrap();
    let value_prop = NamedNode::new("http://example.org/value").unwrap();

    // Create Person instances (targeted by validation)
    for i in 0..person_count {
        let person = NamedNode::new(format!("http://example.org/person{}", i)).unwrap();
        graph.insert(&Triple::new(person.clone(), rdf::TYPE, person_class.clone()));
        graph.insert(&Triple::new(
            person,
            name_prop.clone(),
            Literal::new_simple_literal(format!("Person {}", i)),
        ));
    }

    // Create Thing instances (not targeted - noise)
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

/// Benchmark: Validation with constant target nodes, variable graph size.
///
/// This benchmark answers: Does validation cost scale with total graph size?
fn bench_validation_vs_graph_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation_vs_graph_size");

    let shapes = create_person_shape();
    let validator = ShaclValidator::new(shapes);

    // Constant 10 Person nodes, variable Thing nodes
    let sizes = vec![0, 1_000, 10_000, 50_000];

    for thing_count in sizes {
        let graph = create_mixed_graph(10, thing_count);
        let total_triples = graph.len();

        group.throughput(Throughput::Elements(total_triples as u64));
        group.bench_with_input(
            BenchmarkId::new("graph_size", total_triples),
            &graph,
            |b, g| {
                b.iter(|| {
                    let report = validator.validate(black_box(g)).unwrap();
                    black_box(report);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Validation with variable target nodes, minimal graph size.
///
/// This benchmark answers: Does validation cost scale with affected nodes?
fn bench_validation_vs_target_nodes(c: &mut Criterion) {
    let mut group = c.benchmark_group("validation_vs_target_nodes");

    let shapes = create_person_shape();
    let validator = ShaclValidator::new(shapes);

    // Variable Person nodes, no Thing nodes
    let sizes = vec![10, 100, 1_000, 10_000];

    for person_count in sizes {
        let graph = create_mixed_graph(person_count, 0);

        group.throughput(Throughput::Elements(person_count as u64));
        group.bench_with_input(
            BenchmarkId::new("target_nodes", person_count),
            &graph,
            |b, g| {
                b.iter(|| {
                    let report = validator.validate(black_box(g)).unwrap();
                    black_box(report);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Target node discovery cost.
///
/// This isolates the cost of finding focus nodes (targetClass matching).
fn bench_target_discovery(c: &mut Criterion) {
    let mut group = c.benchmark_group("target_discovery");

    let shapes = create_person_shape();
    let validator = ShaclValidator::new(shapes);

    // Vary graph size with constant small target count
    let configs = vec![
        (10, 1_000),
        (10, 10_000),
        (10, 50_000),
    ];

    for (person_count, thing_count) in configs {
        let graph = create_mixed_graph(person_count, thing_count);
        let total_triples = graph.len();

        group.throughput(Throughput::Elements(total_triples as u64));
        group.bench_with_input(
            BenchmarkId::new("discovery", total_triples),
            &graph,
            |b, g| {
                b.iter(|| {
                    let report = validator.validate(black_box(g)).unwrap();
                    black_box(report);
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_validation_vs_graph_size,
    bench_validation_vs_target_nodes,
    bench_target_discovery
);
criterion_main!(benches);
