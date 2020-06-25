use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use oxigraph::model::{NamedNode, Quad};
use oxigraph::{MemoryStore, RocksDbStore, SledStore};
use rand::random;
use std::env::temp_dir;
use std::fs::remove_dir_all;

criterion_group!(
    store_load,
    memory_load_bench,
    sled_load_bench,
    rocksdb_load_bench
);

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

fn rocksdb_load_bench(c: &mut Criterion) {
    let mut group = c.benchmark_group("rocksdb");
    group.nresamples(10);
    group.sample_size(10);
    let temp_dir = temp_dir();
    for size in [100, 1_000, 10_000].iter() {
        group.throughput(Throughput::Elements(*size as u64));
        let quads = create_quads(*size);
        group.bench_function(BenchmarkId::from_parameter(size), |b| {
            b.iter(|| {
                let mut dir = temp_dir.clone();
                dir.push(random::<u64>().to_string());
                let store = RocksDbStore::open(&dir).unwrap();
                for quad in &quads {
                    store.insert(quad).unwrap();
                }
                remove_dir_all(&dir).unwrap();
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
