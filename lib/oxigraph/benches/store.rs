#![allow(clippy::panic)]

use bzip2::read::MultiBzDecoder;
use codspeed_criterion_compat::{Criterion, Throughput, criterion_group, criterion_main};
use oxhttp::model::{Request, Uri};
use oxigraph::io::{JsonLdProfile, JsonLdProfileSet, RdfFormat, RdfParser, RdfSerializer};
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use rand::random;
use spargebra::{Query, Update};
use std::env::temp_dir;
use std::fs::{File, remove_dir_all};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str;
use std::str::FromStr;

fn parse_bsbm(c: &mut Criterion) {
    let data = read_bz2_data("https://zenodo.org/records/12663333/files/dataset-1000.nt.bz2");
    do_parse(c, RdfFormat::NTriples, &data);
    do_parse(
        c,
        RdfFormat::Turtle,
        &convert_from_nt(&data, RdfFormat::Turtle),
    );
    do_parse(
        c,
        RdfFormat::RdfXml,
        &convert_from_nt(&data, RdfFormat::RdfXml),
    );
    do_parse(
        c,
        RdfFormat::JsonLd {
            profile: JsonLdProfileSet::empty(),
        },
        &convert_from_nt(
            &data,
            RdfFormat::JsonLd {
                profile: JsonLdProfileSet::empty(),
            },
        ),
    );
    do_parse(
        c,
        RdfFormat::JsonLd {
            profile: JsonLdProfile::Streaming.into(),
        },
        &convert_from_nt(
            &data,
            RdfFormat::JsonLd {
                profile: JsonLdProfile::Streaming.into(),
            },
        ),
    );
}

fn do_parse(c: &mut Criterion, format: RdfFormat, data: &[u8]) {
    let mut group = c.benchmark_group(format!("parse {format}"));
    group.throughput(Throughput::Bytes(data.len() as u64));
    group.sample_size(50);
    group.bench_function(format!("parse {format} BSBM explore 1000"), |b| {
        b.iter(|| {
            for r in RdfParser::from_format(format).for_slice(data) {
                r.unwrap();
            }
        })
    });
    group.bench_function(format!("parse {format} BSBM explore 1000 with Read"), |b| {
        b.iter(|| {
            for r in RdfParser::from_format(format).for_reader(data) {
                r.unwrap();
            }
        })
    });
    group.bench_function(format!("parse {format} BSBM explore 1000 unchecked"), |b| {
        b.iter(|| {
            for r in RdfParser::from_format(format).lenient().for_slice(data) {
                r.unwrap();
            }
        })
    });
    group.bench_function(
        format!("parse {format} BSBM explore 1000 unchecked with Read"),
        |b| {
            b.iter(|| {
                for r in RdfParser::from_format(format).lenient().for_reader(data) {
                    r.unwrap();
                }
            })
        },
    );
}

fn convert_from_nt(data: &[u8], to_format: RdfFormat) -> Vec<u8> {
    let mut serializer = RdfSerializer::from_format(to_format).for_writer(Vec::new());
    for quad in RdfParser::from_format(RdfFormat::NTriples).for_slice(data) {
        serializer.serialize_quad(&quad.unwrap()).unwrap();
    }
    serializer.finish().unwrap()
}

fn store_load(c: &mut Criterion) {
    let data = read_bz2_data("https://zenodo.org/records/12663333/files/dataset-1000.nt.bz2");
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
            let store = Store::open(&path).unwrap();
            do_load(&store, &data);
        })
    });
    group.bench_function("load BSBM explore 1000 in memory with bulk load", |b| {
        b.iter(|| {
            let store = Store::new().unwrap();
            do_bulk_load(&store, &data);
        })
    });
    group.bench_function("load BSBM explore 1000 in on disk with bulk load", |b| {
        b.iter(|| {
            let path = TempDir::default();
            let store = Store::open(&path).unwrap();
            do_bulk_load(&store, &data);
        })
    });
}

fn do_load(store: &Store, data: &[u8]) {
    store.load_from_slice(RdfFormat::NTriples, data).unwrap();
    store.optimize().unwrap();
}

fn do_bulk_load(store: &Store, data: &[u8]) {
    store
        .bulk_loader()
        .load_from_slice(RdfParser::from_format(RdfFormat::NTriples).lenient(), data)
        .unwrap();
    store.optimize().unwrap();
}

fn store_query_and_update(c: &mut Criterion) {
    for (data_size, without_opts) in [(1_000, true), (5_000, false)] {
        do_store_query_and_update(c, data_size, without_opts)
    }
}

fn do_store_query_and_update(c: &mut Criterion, data_size: usize, without_ops: bool) {
    let data = read_bz2_data(&format!(
        "https://zenodo.org/records/12663333/files/dataset-{data_size}.nt.bz2"
    ));
    let explore_operations = bsbm_sparql_operation("exploreAndUpdate-1000.csv.bz2")
        .into_iter()
        .map(|op| match op {
            RawOperation::Query(q) => Operation::Query(Query::from_str(&q).unwrap()),
            RawOperation::Update(q) => Operation::Update(Update::from_str(&q).unwrap()),
        })
        .collect::<Vec<_>>();
    let explore_query_operations = explore_operations
        .iter()
        .filter(|o| matches!(o, Operation::Query(_)))
        .cloned()
        .collect::<Vec<_>>();
    let business_operations = bsbm_sparql_operation("businessIntelligence-1000.csv.bz2")
        .into_iter()
        .map(|op| match op {
            RawOperation::Query(q) => {
                Operation::Query(Query::from_str(&q.replace('#', "")).unwrap())
            }
            RawOperation::Update(_) => unreachable!(),
        })
        .collect::<Vec<_>>();

    let mut group = c.benchmark_group("store operations");
    group.sample_size(10);

    {
        let memory_store = Store::new().unwrap();
        do_bulk_load(&memory_store, &data);
        group.bench_function(format!("BSBM explore {data_size} query in memory"), |b| {
            b.iter(|| run_operation(&memory_store, &explore_query_operations, true))
        });
        if without_ops {
            group.bench_function(
                format!("BSBM explore {data_size} query in memory without optimizations"),
                |b| b.iter(|| run_operation(&memory_store, &explore_query_operations, false)),
            );
        }
        group.bench_function(
            format!("BSBM explore {data_size} queryAndUpdate in memory"),
            |b| b.iter(|| run_operation(&memory_store, &explore_operations, true)),
        );
        if without_ops {
            group.bench_function(
                format!("BSBM explore {data_size} queryAndUpdate in memory without optimizations"),
                |b| b.iter(|| run_operation(&memory_store, &explore_operations, false)),
            );
            group.bench_function(
                format!("BSBM business intelligence {data_size} in memory"),
                |b| b.iter(|| run_operation(&memory_store, &business_operations, true)),
            );
        }
    }

    {
        let path = TempDir::default();
        let disk_store = Store::open(&path).unwrap();
        do_bulk_load(&disk_store, &data);
        group.bench_function(format!("BSBM explore {data_size} query on disk"), |b| {
            b.iter(|| run_operation(&disk_store, &explore_query_operations, true))
        });
        if without_ops {
            group.bench_function(
                format!("BSBM explore {data_size} query on disk without optimizations"),
                |b| b.iter(|| run_operation(&disk_store, &explore_query_operations, false)),
            );
        }
        group.bench_function(
            format!("BSBM explore {data_size} queryAndUpdate on disk"),
            |b| b.iter(|| run_operation(&disk_store, &explore_operations, true)),
        );
        if without_ops {
            group.bench_function(
                format!("BSBM explore {data_size} queryAndUpdate on disk without optimizations"),
                |b| b.iter(|| run_operation(&disk_store, &explore_operations, false)),
            );
            group.bench_function(
                format!("BSBM business intelligence {data_size} on disk"),
                |b| b.iter(|| run_operation(&disk_store, &business_operations, true)),
            );
        }
    }
}

fn run_operation(store: &Store, operations: &[Operation], with_opts: bool) {
    let mut evaluator = SparqlEvaluator::new();
    if !with_opts {
        evaluator = evaluator.without_optimizations();
    }
    for operation in operations {
        match operation {
            Operation::Query(q) => match evaluator
                .clone()
                .for_query(q.clone())
                .on_store(store)
                .execute()
                .unwrap()
            {
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
            Operation::Update(u) => store.update_opt(u.clone(), evaluator.clone()).unwrap(),
        }
    }
}

fn sparql_parsing(c: &mut Criterion) {
    let operations = bsbm_sparql_operation("exploreAndUpdate-1000.csv.bz2");
    let mut group = c.benchmark_group("sparql parsing");
    group.sample_size(10);
    group.throughput(Throughput::Bytes(
        operations
            .iter()
            .map(|o| match o {
                RawOperation::Query(q) => q.len(),
                RawOperation::Update(u) => u.len(),
            })
            .sum::<usize>() as u64,
    ));
    group.bench_function("BSBM query and update set", |b| {
        b.iter(|| {
            for operation in &operations {
                match operation {
                    RawOperation::Query(q) => {
                        Query::from_str(q).unwrap();
                    }
                    RawOperation::Update(u) => {
                        Update::from_str(u).unwrap();
                    }
                }
            }
        })
    });
}

criterion_group!(parse, parse_bsbm);
criterion_group!(store, sparql_parsing, store_query_and_update, store_load);

criterion_main!(parse, store);

fn read_bz2_data(url: &str) -> Vec<u8> {
    let url = Uri::from_str(url).unwrap();
    let file_name = url.path().split('/').next_back().unwrap().to_owned();
    if !Path::new(&file_name).exists() {
        let client = oxhttp::Client::new().with_redirection_limit(5);
        let request = Request::builder().uri(&url).body(()).unwrap();
        let response = client.request(request).unwrap();
        assert!(response.status().is_success(), "{url}");
        std::io::copy(
            &mut response.into_body(),
            &mut File::create(&file_name).unwrap(),
        )
        .unwrap();
    }
    let mut buf = Vec::new();
    MultiBzDecoder::new(File::open(&file_name).unwrap())
        .read_to_end(&mut buf)
        .unwrap();
    buf
}

fn bsbm_sparql_operation(file_name: &str) -> Vec<RawOperation> {
    csv::Reader::from_reader(read_bz2_data(&format!("https://zenodo.org/records/12663333/files/{file_name}")).as_slice()).records()
        .collect::<Result<Vec<_>, _>>().unwrap()
        .into_iter()
        .rev()
        .take(300) // We take only 10 groups
        .map(|l| {
            match &l[1] {
                "query" => RawOperation::Query(l[2].into()),
                "update" => RawOperation::Update(l[2].into()),
                _ => panic!("Unexpected operation kind {}", &l[1]),
            }
        })
        .collect()
}

#[derive(Clone)]
enum RawOperation {
    Query(String),
    Update(String),
}

#[allow(clippy::large_enum_variant, clippy::allow_attributes)]
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

impl AsRef<Path> for TempDir {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        remove_dir_all(&self.0).unwrap()
    }
}
