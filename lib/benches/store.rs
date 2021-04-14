use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use oxigraph::model::{NamedNode, Quad};
use oxigraph::{MemoryStore, SledStore};
use rand::random;

criterion_group!(store_load, memory_load_bench, sled_load_bench);

criterion_main!(store_load);

fn memory_load_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory");
    group.nresamples(10);
    group.sample_size(10);
    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        let quads = create_quads(*size);
        group.bench_function(BenchmarkId::from_parameter(size), |b| {
            b.iter(|| {
                let store = MemoryStore::new();
                for quad in &quads {
                    store.insert(quad.clone());
                }
            });
        });
    }
    group.finish();
}

fn sled_load_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("sled");
    group.nresamples(10);
    group.sample_size(10);
    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        let quads = create_quads(*size);
        group.bench_function(BenchmarkId::from_parameter(size), |b| {
            b.iter(|| {
                let store = SledStore::new().unwrap();
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
                None,
            )
        })
        .collect()
}
