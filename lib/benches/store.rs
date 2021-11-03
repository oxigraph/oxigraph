use criterion::{criterion_group, criterion_main, Criterion, Throughput};
use oxhttp::model::{Method, Request, Status};
use oxigraph::io::GraphFormat;
use oxigraph::model::GraphNameRef;
use oxigraph::sparql::{Query, QueryResults, Update};
use oxigraph::store::Store;
use std::fs::File;
use std::io::{BufRead, BufReader, Cursor, Read};
use std::path::Path;

fn store_load(c: &mut Criterion) {
    let mut data = Vec::new();
    read_data("explore-1000.nt.zst")
        .read_to_end(&mut data)
        .unwrap();

    let mut group = c.benchmark_group("store load");
    group.throughput(Throughput::Bytes(data.len() as u64));
    group.sample_size(10);
    group.bench_function("load BSBM explore 1000", |b| {
        b.iter(|| {
            let store = Store::new().unwrap();
            store
                .load_graph(
                    Cursor::new(&data),
                    GraphFormat::NTriples,
                    GraphNameRef::DefaultGraph,
                    None,
                )
                .unwrap();
        })
    });
}

fn store_query_and_update(c: &mut Criterion) {
    let mut data = Vec::new();
    read_data("explore-1000.nt.zst")
        .read_to_end(&mut data)
        .unwrap();
    let store = Store::new().unwrap();
    store
        .load_graph(
            Cursor::new(&data),
            GraphFormat::NTriples,
            GraphNameRef::DefaultGraph,
            None,
        )
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

    let mut group = c.benchmark_group("store operations");
    group.throughput(Throughput::Elements(operations.len() as u64));
    group.sample_size(10);
    group.bench_function("BSBM explore 1000 queryAndUpdate", |b| {
        b.iter(|| {
            for operation in &operations {
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
        })
    });
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

enum Operation {
    Query(Query),
    Update(Update),
}
