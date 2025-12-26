#![allow(clippy::panic)]

use codspeed_criterion_compat::{criterion_group, criterion_main, Criterion, Throughput};
use oxrdf::{NamedNode, Term};
use sparshex::{
    Cardinality, NodeConstraint, NodeKind, Shape, ShapeExpression, ShapeLabel, ShapesSchema,
    TripleConstraint, ValueSetValue,
};

/// Benchmark schema construction with simple shapes
/// Tests O(n) performance for building schemas
fn schema_construction_simple(c: &mut Criterion) {
    let mut group = c.benchmark_group("schema construction simple");

    for size in [10, 100, 1_000, 10_000] {
        group.throughput(Throughput::Elements(size));
        group.bench_function(format!("build schema with {size} simple shapes"), |b| {
            b.iter(|| create_simple_schema(size))
        });
    }

    group.finish();
}

/// Benchmark schema construction with nested shapes
/// Tests O(n*m) performance for building schemas with references
fn schema_construction_nested(c: &mut Criterion) {
    let mut group = c.benchmark_group("schema construction nested");

    for depth in [2, 5, 10, 20] {
        group.bench_function(format!("build schema with nesting depth {depth}"), |b| {
            b.iter(|| create_nested_schema(100, depth))
        });
    }

    group.finish();
}

/// Benchmark shape reference validation
/// Tests O(n*m) performance for checking all references are defined
fn reference_validation(c: &mut Criterion) {
    let mut group = c.benchmark_group("reference validation");

    for size in [10, 100, 1_000, 10_000] {
        let schema = create_simple_schema_with_refs(size);
        group.throughput(Throughput::Elements(size));
        group.bench_function(format!("validate refs in schema with {size} shapes"), |b| {
            b.iter(|| schema.validate_refs().unwrap())
        });
    }

    group.finish();
}

/// Benchmark cycle detection in shape references
/// Tests O(n+e) performance for detecting cycles
fn cycle_detection(c: &mut Criterion) {
    let mut group = c.benchmark_group("cycle detection");

    for size in [10, 100, 1_000] {
        let schema_acyclic = create_simple_schema_with_refs(size);
        group.throughput(Throughput::Elements(size));
        group.bench_function(format!("detect cycles acyclic schema {size} shapes"), |b| {
            b.iter(|| schema_acyclic.detect_cycles().unwrap())
        });
    }

    // Test with actual cycles
    for cycle_length in [5, 10, 20, 50] {
        let schema_cyclic = create_cyclic_schema(cycle_length);
        group.bench_function(
            format!("detect cycles cyclic schema length {cycle_length}"),
            |b| b.iter(|| schema_cyclic.detect_cycles().is_err()),
        );
    }

    group.finish();
}

/// Benchmark node constraint operations
/// Tests performance of constraint checking logic
fn node_constraint_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("node constraint operations");

    // Test node kind matching
    for count in [100, 1_000, 10_000, 100_000] {
        let terms: Vec<_> = (0..count)
            .map(|i| Term::NamedNode(NamedNode::new_unchecked(format!("http://example.org/n{i}"))))
            .collect();

        group.throughput(Throughput::Elements(count));
        group.bench_function(format!("node kind matching {count} terms"), |b| {
            b.iter(|| {
                for term in &terms {
                    NodeKind::Iri.matches(term);
                }
            })
        });
    }

    group.finish();
}

/// Benchmark value set constraint operations
/// Tests O(n) lookup performance
fn value_set_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("value set operations");

    for set_size in [10, 100, 1_000, 10_000] {
        group.throughput(Throughput::Elements(set_size));
        group.bench_function(format!("build value set constraint size {set_size}"), |b| {
            b.iter(|| create_value_set_constraint(set_size))
        });
    }

    group.finish();
}

/// Benchmark cardinality checking
/// Tests performance of cardinality.allows() method
fn cardinality_checking(c: &mut Criterion) {
    let mut group = c.benchmark_group("cardinality checking");

    let cardinalities = vec![
        ("exactly_1", Cardinality::exactly(1)),
        ("optional", Cardinality::optional()),
        ("zero_or_more", Cardinality::zero_or_more()),
        ("one_or_more", Cardinality::one_or_more()),
        ("range_2_10", Cardinality::new(2, Some(10)).unwrap()),
    ];

    for (name, card) in cardinalities {
        group.bench_function(format!("check cardinality {name} 1M times"), |b| {
            b.iter(|| {
                for i in 0..1_000_000 {
                    card.allows((i % 20) as u32);
                }
            })
        });
    }

    group.finish();
}

/// Benchmark triple constraint creation
/// Tests performance of building shape structures
fn triple_constraint_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("triple constraint creation");

    for count in [10, 100, 1_000, 10_000] {
        group.throughput(Throughput::Elements(count));
        group.bench_function(format!("create {count} triple constraints"), |b| {
            b.iter(|| {
                let mut shape = Shape::new();
                for i in 0..count {
                    let predicate = NamedNode::new_unchecked(format!("http://example.org/p{i}"));
                    let tc = TripleConstraint::new(predicate)
                        .with_cardinality(Cardinality::one_or_more());
                    shape.add_triple_constraint(tc);
                }
                shape
            })
        });
    }

    group.finish();
}

/// Benchmark shape cloning
/// Tests memory and copy performance
fn shape_cloning(c: &mut Criterion) {
    let mut group = c.benchmark_group("shape cloning");

    for size in [10, 100, 1_000] {
        let schema = create_simple_schema(size);
        group.throughput(Throughput::Elements(size));
        group.bench_function(format!("clone schema with {size} shapes"), |b| {
            b.iter(|| schema.clone())
        });
    }

    group.finish();
}

/// Benchmark schema queries
/// Tests performance of schema lookup operations
fn schema_queries(c: &mut Criterion) {
    let mut group = c.benchmark_group("schema queries");

    for size in [100, 1_000, 10_000] {
        let schema = create_simple_schema(size);
        let labels: Vec<_> = schema.labels().cloned().collect();

        group.throughput(Throughput::Elements(size));
        group.bench_function(format!("lookup all {size} shapes"), |b| {
            b.iter(|| {
                for label in &labels {
                    schema.get_shape(label).unwrap();
                }
            })
        });

        group.bench_function(format!("iterate {size} shapes"), |b| {
            b.iter(|| {
                for (_label, _shape) in schema.shapes() {
                    // Just iterate
                }
            })
        });
    }

    group.finish();
}

// Helper functions to create test schemas

fn create_simple_schema(shape_count: u64) -> ShapesSchema {
    let mut schema = ShapesSchema::new();

    for i in 0..shape_count {
        let label = ShapeLabel::Iri(NamedNode::new_unchecked(format!(
            "http://example.org/Shape{i}"
        )));
        let node_constraint = NodeConstraint::with_node_kind(NodeKind::Iri);
        let expr = ShapeExpression::NodeConstraint(node_constraint);
        schema.add_shape(label, expr);
    }

    schema
}

fn create_nested_schema(shapes_per_level: u64, depth: u32) -> ShapesSchema {
    let mut schema = ShapesSchema::new();

    for level in 0..depth {
        for i in 0..shapes_per_level {
            let label = ShapeLabel::Iri(NamedNode::new_unchecked(format!(
                "http://example.org/Shape{level}_{i}"
            )));

            let mut shape = Shape::new();

            // Add some properties
            let name_pred = NamedNode::new_unchecked("http://example.org/name");
            let name_tc = TripleConstraint::new(name_pred);
            shape.add_triple_constraint(name_tc);

            // Reference next level
            if level < depth - 1 {
                let next_label = ShapeLabel::Iri(NamedNode::new_unchecked(format!(
                    "http://example.org/Shape{}_{i}",
                    level + 1
                )));
                let next_pred = NamedNode::new_unchecked("http://example.org/next");
                let next_tc = TripleConstraint::with_value_expr(
                    next_pred,
                    ShapeExpression::ShapeRef(next_label),
                );
                shape.add_triple_constraint(next_tc);
            }

            schema.add_shape(label, ShapeExpression::Shape(shape));
        }
    }

    schema
}

fn create_simple_schema_with_refs(shape_count: u64) -> ShapesSchema {
    let mut schema = ShapesSchema::new();

    // Create a base shape
    let base_label = ShapeLabel::Iri(NamedNode::new_unchecked(
        "http://example.org/BaseShape".to_owned(),
    ));
    schema.add_shape(
        base_label.clone(),
        ShapeExpression::NodeConstraint(NodeConstraint::with_node_kind(NodeKind::Iri)),
    );

    // Create shapes that reference the base shape
    for i in 0..shape_count {
        let label = ShapeLabel::Iri(NamedNode::new_unchecked(format!(
            "http://example.org/Shape{i}"
        )));
        let mut shape = Shape::new();
        let pred = NamedNode::new_unchecked(format!("http://example.org/prop{i}"));
        let tc = TripleConstraint::with_value_expr(
            pred,
            ShapeExpression::ShapeRef(base_label.clone()),
        );
        shape.add_triple_constraint(tc);
        schema.add_shape(label, ShapeExpression::Shape(shape));
    }

    schema
}

fn create_cyclic_schema(cycle_length: u64) -> ShapesSchema {
    let mut schema = ShapesSchema::new();

    for i in 0..cycle_length {
        let label = ShapeLabel::Iri(NamedNode::new_unchecked(format!(
            "http://example.org/Shape{i}"
        )));
        let next_label = ShapeLabel::Iri(NamedNode::new_unchecked(format!(
            "http://example.org/Shape{}",
            (i + 1) % cycle_length
        )));

        let mut shape = Shape::new();
        let pred = NamedNode::new_unchecked("http://example.org/next");
        let tc =
            TripleConstraint::with_value_expr(pred, ShapeExpression::ShapeRef(next_label));
        shape.add_triple_constraint(tc);
        schema.add_shape(label, ShapeExpression::Shape(shape));
    }

    schema
}

fn create_value_set_constraint(set_size: u64) -> NodeConstraint {
    let mut constraint = NodeConstraint::new();

    for i in 0..set_size {
        let term = Term::NamedNode(NamedNode::new_unchecked(format!(
            "http://example.org/value{i}"
        )));
        constraint.add_value(ValueSetValue::ObjectValue(term));
    }

    constraint
}

criterion_group!(
    schema_benches,
    schema_construction_simple,
    schema_construction_nested,
    reference_validation,
    cycle_detection,
    node_constraint_operations,
    value_set_operations,
    cardinality_checking,
    triple_constraint_creation,
    shape_cloning,
    schema_queries
);

criterion_main!(schema_benches);
