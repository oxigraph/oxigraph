use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use oxigraph::model::{Dataset, Graph, GraphName, NamedNode, Quad, Triple};
use oxigraph::store::Store;
use rand::random;

criterion_group!(
    store_load,
    graph_load_bench,
    dataset_load_bench,
    sled_load_bench
);

criterion_main!(store_load);

fn graph_load_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("graph");
    group.nresamples(10);
    group.sample_size(10);
    for size in [100, 1_000, 10_000] {
        group.throughput(Throughput::Elements(size as u64));
        let triples: Vec<_> = create_quads(size).into_iter().map(Triple::from).collect();
        group.bench_function(BenchmarkId::from_parameter(size), |b| {
            b.iter(|| triples.iter().collect::<Graph>());
        });
    }
    group.finish();
}

fn dataset_load_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("dataset");
    group.nresamples(10);
    group.sample_size(10);
    for size in [100, 1_000, 10_000] {
        group.throughput(Throughput::Elements(size as u64));
        let quads = create_quads(size);
        group.bench_function(BenchmarkId::from_parameter(size), |b| {
            b.iter(|| quads.iter().collect::<Dataset>());
        });
    }
    group.finish();
}

fn sled_load_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("sled");
    group.nresamples(10);
    group.sample_size(10);
    for size in [100, 1_000, 10_000] {
        group.throughput(Throughput::Elements(size as u64));
        let quads = create_quads(size);
        group.bench_function(BenchmarkId::from_parameter(size), |b| {
            b.iter(|| {
                let store = Store::new().unwrap();
                for quad in &quads {
                    store.insert(quad).unwrap();
                }
            });
        });
    }
    group.finish();
}

fn create_quads(size: u64) -> Vec<Quad> {
    (0..size)
        .map(|_| {
            Quad::new(
                NamedNode::new_unchecked(format!(
                    "http://example.com/id/{}",
                    random::<u64>() % size
                )),
                NamedNode::new_unchecked(format!(
                    "http://example.com/id/{}",
                    random::<u64>() % size
                )),
                NamedNode::new_unchecked(format!(
                    "http://example.com/id/{}",
                    random::<u64>() % size
                )),
                GraphName::DefaultGraph,
            )
        })
        .collect()
}
