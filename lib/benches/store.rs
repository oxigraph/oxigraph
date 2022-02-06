use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use oxhttp::model::{Method, Request, Status};
use oxigraph::io::GraphFormat;
use oxigraph::model::GraphNameRef;
use oxigraph::sparql::{Query, QueryResults, Update};
use oxigraph::store::Store;
use rand::random;
use std::env::temp_dir;
use std::fs::{remove_dir_all, File};
use std::io::{BufRead, BufReader, Cursor, Read};
use std::path::{Path, PathBuf};

fn store_load(c: &mut Criterion) {
    {
        let mut data = Vec::new();
        read_data("explore-1000.nt.zst")
            .read_to_end(&mut data)
            .unwrap();

        let mut group = c.benchmark_group("store load");
        group.throughput(Throughput::Bytes(data.len() as u64));
        group.sample_size(10);
        group.bench_function("load BSBM explore 1000 in memory", |b| {
            b.iter(|| {
                let store = Store::new().unwrap();
                do_load(&store, &data);
            })
        });
        group.bench_function("load BSBM explore 1000 in on disk", |b| {
            b.iter(|| {
                let path = TempDir::default();
                let store = Store::open(&path.0).unwrap();
                do_load(&store, &data);
            })
        });
        group.bench_function("load BSBM explore 1000 in on disk with bulk load", |b| {
            b.iter(|| {
                let path = TempDir::default();
                let store = Store::open(&path.0).unwrap();
                do_bulk_load(&store, &data);
            })
        });
    }

    {
        let mut data = Vec::new();
        read_data("explore-10000.nt.zst")
            .read_to_end(&mut data)
            .unwrap();

        let mut group = c.benchmark_group("store load large");
        group.throughput(Throughput::Bytes(data.len() as u64));
        group.sample_size(10);
        group.bench_function("load BSBM explore 10000 in on disk with bulk load", |b| {
            b.iter(|| {
                let path = TempDir::default();
                let store = Store::open(&path.0).unwrap();
                do_bulk_load(&store, &data);
            })
        });
    }
}

fn do_load(store: &Store, data: &[u8]) {
    store
        .load_graph(
            Cursor::new(&data),
            GraphFormat::NTriples,
            GraphNameRef::DefaultGraph,
            None,
        )
        .unwrap();
    store.optimize().unwrap();
}

fn do_bulk_load(store: &Store, data: &[u8]) {
    store
        .bulk_loader()
        .load_graph(
            Cursor::new(&data),
            GraphFormat::NTriples,
            GraphNameRef::DefaultGraph,
            None,
        )
        .unwrap();
    store.optimize().unwrap();
}

fn store_query_and_update(c: &mut Criterion) {
    let mut data = Vec::new();
    read_data("explore-1000.nt.zst")
        .read_to_end(&mut data)
        .unwrap();

    let operations = read_data("mix-exploreAndUpdate-1000.tsv.zst")
        .lines()
        .map(|l| {
            let l = l.unwrap();
            let mut parts = l.trim().split('\t');
            let kind = parts.next().unwrap();
            let operation = parts.next().unwrap();
            match kind {
                "query" => Operation::Query(Query::parse(operation, None).unwrap()),
                "update" => Operation::Update(Update::parse(operation, None).unwrap()),
                _ => panic!("Unexpected operation kind {}", kind),
            }
        })
        .collect::<Vec<_>>();
    let query_operations = operations
        .iter()
        .filter(|o| matches!(o, Operation::Query(_)))
        .cloned()
        .collect::<Vec<_>>();

    let mut group = c.benchmark_group("store operations");
    group.throughput(Throughput::Elements(operations.len() as u64));
    group.sample_size(10);

    {
        let memory_store = Store::new().unwrap();
        do_bulk_load(&memory_store, &data);
        group.bench_function("BSBM explore 1000 query in memory", |b| {
            b.iter(|| run_operation(&memory_store, &query_operations))
        });
        group.bench_function("BSBM explore 1000 queryAndUpdate in memory", |b| {
            b.iter(|| run_operation(&memory_store, &operations))
        });
    }

    {
        let path = TempDir::default();
        let disk_store = Store::open(&path.0).unwrap();
        do_bulk_load(&disk_store, &data);
        group.bench_function("BSBM explore 1000 query on disk", |b| {
            b.iter(|| run_operation(&disk_store, &query_operations))
        });
        group.bench_function("BSBM explore 1000 queryAndUpdate on disk", |b| {
            b.iter(|| run_operation(&disk_store, &operations))
        });
    }
}

fn run_operation(store: &Store, operations: &[Operation]) {
    for operation in operations {
        match operation {
            Operation::Query(q) => match store.query(q.clone()).unwrap() {
                QueryResults::Boolean(_) => (),
                QueryResults::Solutions(s) => {
                    for s in s {
                        s.unwrap();
                    }
                }
                QueryResults::Graph(g) => {
                    for t in g {
                        t.unwrap();
                    }
                }
            },
            Operation::Update(u) => store.update(u.clone()).unwrap(),
        }
    }
}

criterion_group!(store, store_query_and_update, store_load);

criterion_main!(store);

fn read_data(file: &str) -> impl BufRead {
    if !Path::new(file).exists() {
        let mut client = oxhttp::Client::new();
        client.set_redirection_limit(5);
        let url = format!(
            "https://github.com/Tpt/bsbm-tools/releases/download/v0.2/{}",
            file
        );
        let request = Request::builder(Method::GET, url.parse().unwrap()).build();
        let response = client.request(request).unwrap();
        assert_eq!(
            response.status(),
            Status::OK,
            "{}",
            response.into_body().to_string().unwrap()
        );
        std::io::copy(&mut response.into_body(), &mut File::create(file).unwrap()).unwrap();
    }
    BufReader::new(zstd::Decoder::new(File::open(file).unwrap()).unwrap())
}

#[allow(clippy::large_enum_variant)]
#[derive(Clone)]
enum Operation {
    Query(Query),
    Update(Update),
}

struct TempDir(PathBuf);

impl Default for TempDir {
    fn default() -> Self {
        Self(temp_dir().join(format!("oxigraph-bench-{}", random::<u128>())))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        remove_dir_all(&self.0).unwrap()
    }
}
