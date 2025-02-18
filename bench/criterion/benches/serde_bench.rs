use std::hint::black_box;
use criterion::{criterion_group, criterion_main, Criterion};
use oxrdf::{NamedNode, BlankNode, Literal, Triple, Term};
use serde_json;

fn serialize_named_node(c: &mut Criterion) {
    let named_node = NamedNode::new("http://example.com").unwrap();
    c.bench_function("serialize_named_node", |b| b.iter(|| serde_json::to_string(&named_node)));
}

fn deserialize_named_node(c: &mut Criterion) {
    let named_node = NamedNode::new("http://example.com").unwrap();
    let named_node_json = serde_json::to_string(&named_node).unwrap();
    c.bench_function("deserialize_named_node", |b| b.iter(|| serde_json::from_str::<NamedNode>(&named_node_json)));
}

fn serialize_blank_node(c: &mut Criterion) {
    let blank_node = BlankNode::new("1").unwrap();
    c.bench_function("serialize_blank_node", |b| b.iter(|| serde_json::to_string(&blank_node)));
}

fn deserialize_blank_node(c: &mut Criterion) {
    let blank_node = BlankNode::new("1").unwrap();
    let blank_node_json = serde_json::to_string(&blank_node).unwrap();
    c.bench_function("deserialize_blank_node", |b| b.iter(|| serde_json::from_str::<BlankNode>(&blank_node_json)));
}

fn serialize_simple_literal(c: &mut Criterion) {
    let literal = Literal::new_simple_literal("1");
    c.bench_function("serialize_literal", |b| b.iter(|| serde_json::to_string(&literal)));
}

fn deserialize_simple_literal(c: &mut Criterion) {
    let literal = Literal::new_simple_literal("1");
    let literal_json = serde_json::to_string(&literal).unwrap();
    c.bench_function("deserialize_literal", |b| b.iter(|| serde_json::from_str::<Literal>(&literal_json)));
}

fn serialize_typed_literal(c: &mut Criterion) {
    let named_node = NamedNode::new("http://example.com").unwrap();
    let literal = Literal::new_typed_literal("1", named_node);
    c.bench_function("serialize_typed_literal", |b| b.iter(|| serde_json::to_string(&literal)));
}

fn deserialize_typed_literal(c: &mut Criterion) {
    let named_node = NamedNode::new("http://example.com").unwrap();
    let literal = Literal::new_typed_literal("1", named_node);
    let literal_json = serde_json::to_string(&literal).unwrap();
    c.bench_function("deserialize_typed_literal", |b| b.iter(|| serde_json::from_str::<Literal>(&literal_json)));
}

fn serialize_triple(c: &mut Criterion) {
    let named_node = NamedNode::new("http://example.com").unwrap();
    let blank_node = BlankNode::new("1").unwrap();
    let literal = Literal::new_simple_literal("1");
    let triple = Triple::new(
        blank_node,
        named_node,
        Term::Literal(literal),
    );
    c.bench_function("serialize_triple", |b| b.iter(|| serde_json::to_string(&triple)));
}

fn deserialize_triple(c: &mut Criterion) {
    let named_node = NamedNode::new("http://example.com").unwrap();
    let blank_node = BlankNode::new("1").unwrap();
    let literal = Literal::new_simple_literal("1");
    let triple = Triple::new(
        blank_node,
        named_node,
        Term::Literal(literal),
    );
    let triple_json = serde_json::to_string(&triple).unwrap();
    c.bench_function("deserialize_triple", |b| b.iter(|| serde_json::from_str::<Triple>(&triple_json)));
}

criterion_group!(
    benches,
    serialize_named_node,
    deserialize_named_node,
    serialize_blank_node,
    deserialize_blank_node,
    serialize_simple_literal,
    deserialize_simple_literal,
    serialize_typed_literal,
    deserialize_typed_literal,
    serialize_triple,
    deserialize_triple
);
criterion_main!(benches);
