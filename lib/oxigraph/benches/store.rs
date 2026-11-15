#![expect(clippy::panic)]

use bzip2::read::MultiBzDecoder;
use codspeed_criterion_compat::{Criterion, Throughput, criterion_group, criterion_main};
use flate2::read::GzDecoder;
use incroxigraph::io::{JsonLdProfile, JsonLdProfileSet, RdfFormat, RdfParser, RdfSerializer};
use incroxigraph::model::Dataset;
use incroxigraph::sparql::{QueryEvaluationError, QueryResults, SparqlEvaluator};
use incroxigraph::store::Store;
use oxhttp::model::{Request, Uri};
use spareval::QueryEvaluator;
use spargebra::{Query, Update};
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::{fs, str};
use tempfile::{NamedTempFile, TempDir};

fn parse_bsbm(c: &mut Criterion) {
    if !workload_selected_by_filter("bsbm") {
        return;
    }
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
    if !workload_selected_by_filter("bsbm") {
        return;
    }
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
            let path = TempDir::new().unwrap();
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
            let path = TempDir::new().unwrap();
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
    let mut loader = store.bulk_loader();
    loader
        .load_from_slice(RdfParser::from_format(RdfFormat::NTriples).lenient(), data)
        .unwrap();
    loader.commit().unwrap();
    store.optimize().unwrap();
}

fn do_bulk_load_file(store: &Store, data_path: &Path) {
    let mut loader = store.bulk_loader();
    loader
        .load_from_reader(
            RdfParser::from_format(RdfFormat::NTriples).lenient(),
            File::open(data_path).unwrap_or_else(|error| {
                panic!(
                    "failed to read static WatDiv data {}: {error}",
                    data_path.display()
                )
            }),
        )
        .unwrap();
    loader.commit().unwrap();
    store.optimize().unwrap();
}

fn store_query_and_update(c: &mut Criterion) {
    if !workload_selected_by_filter("bsbm") {
        return;
    }
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
    let explore_select_operations: Vec<_> = explore_query_operations
        .iter()
        .filter_map(|o| match o {
            Operation::Query(q @ Query::Select(_)) => Some(Operation::Query(q.clone())),
            Operation::Query(_) | Operation::Update(_) => None,
        })
        .collect();
    let business_operations = bsbm_sparql_operation("businessIntelligence-1000.csv.bz2")
        .into_iter()
        .map(|op| match op {
            RawOperation::Query(q) => {
                Operation::Query(Query::from_str(&q.replace("# ", "")).unwrap())
            }
            RawOperation::Update(_) => unreachable!(),
        })
        .collect::<Vec<_>>();

    let mut group = c.benchmark_group("store operations");
    group.sample_size(10);

    {
        let memory_store = Store::new().unwrap();
        do_bulk_load(&memory_store, &data);
        let incremental_dataset = Dataset::from_iter(memory_store.iter().map(|quad| quad.unwrap()));
        group.bench_function(format!("BSBM explore {data_size} query in memory"), |b| {
            b.iter(|| run_operations(&memory_store, &explore_query_operations, true))
        });
        if !explore_select_operations.is_empty() {
            group.bench_function(
                format!("BSBM explore {data_size} SELECT query in memory with incremental engine"),
                |b| {
                    b.iter(|| {
                        run_operations_incremental(
                            &incremental_dataset,
                            &explore_select_operations,
                            true,
                            UnsupportedQueryHandling::Skip,
                        )
                    })
                },
            );
            if without_ops {
                group.bench_function(
                    format!(
                        "BSBM explore {data_size} SELECT query in memory with incremental engine without optimizations"
                    ),
                    |b| {
                        b.iter(|| run_operations_incremental(&incremental_dataset, &explore_select_operations, false, UnsupportedQueryHandling::Skip))
                    },
                );
            }
        }
        if without_ops {
            group.bench_function(
                format!("BSBM explore {data_size} query in memory without optimizations"),
                |b| b.iter(|| run_operations(&memory_store, &explore_query_operations, false)),
            );
        }
        group.bench_function(
            format!("BSBM explore {data_size} queryAndUpdate in memory"),
            |b| b.iter(|| run_operations(&memory_store, &explore_operations, true)),
        );
        if without_ops {
            group.bench_function(
                format!("BSBM explore {data_size} queryAndUpdate in memory without optimizations"),
                |b| b.iter(|| run_operations(&memory_store, &explore_operations, false)),
            );
            group.bench_function(
                format!("BSBM business intelligence {data_size} in memory"),
                |b| b.iter(|| run_operations(&memory_store, &business_operations, true)),
            );
            for (name, operations) in sparqloscope_operations() {
                group.bench_function(
                    format!("Sparqloscope BSBM {data_size} in memory - {name}"),
                    |b| b.iter(|| run_operations(&memory_store, &operations, true)),
                );
            }
        }
    }

    {
        let path = TempDir::new().unwrap();
        let disk_store = Store::open(&path).unwrap();
        do_bulk_load(&disk_store, &data);
        let incremental_dataset = Dataset::from_iter(disk_store.iter().map(|quad| quad.unwrap()));
        group.bench_function(format!("BSBM explore {data_size} query on disk"), |b| {
            b.iter(|| run_operations(&disk_store, &explore_query_operations, true))
        });
        if !explore_select_operations.is_empty() {
            group.bench_function(
                format!("BSBM explore {data_size} SELECT query on disk with incremental engine"),
                |b| {
                    b.iter(|| {
                        run_operations_incremental(
                            &incremental_dataset,
                            &explore_select_operations,
                            true,
                            UnsupportedQueryHandling::Skip,
                        )
                    })
                },
            );
            if without_ops {
                group.bench_function(
                    format!(
                        "BSBM explore {data_size} SELECT query on disk with incremental engine without optimizations"
                    ),
                    |b| {
                        b.iter(|| run_operations_incremental(&incremental_dataset, &explore_select_operations, false, UnsupportedQueryHandling::Skip))
                    },
                );
            }
        }
        if without_ops {
            group.bench_function(
                format!("BSBM explore {data_size} query on disk without optimizations"),
                |b| b.iter(|| run_operations(&disk_store, &explore_query_operations, false)),
            );
        }
        group.bench_function(
            format!("BSBM explore {data_size} queryAndUpdate on disk"),
            |b| b.iter(|| run_operations(&disk_store, &explore_operations, true)),
        );
        if without_ops {
            group.bench_function(
                format!("BSBM explore {data_size} queryAndUpdate on disk without optimizations"),
                |b| b.iter(|| run_operations(&disk_store, &explore_operations, false)),
            );
            group.bench_function(
                format!("BSBM business intelligence {data_size} on disk"),
                |b| b.iter(|| run_operations(&disk_store, &business_operations, true)),
            );
        }
    }
}

fn store_watdiv(c: &mut Criterion) {
    if !workload_selected_by_filter("watdiv") {
        return;
    }
    let scale = std::env::var("WATDIV_STORE_SCALE")
        .ok()
        .map_or(100, |scale| {
            scale
                .parse::<usize>()
                .unwrap_or_else(|error| panic!("WATDIV_STORE_SCALE must be an integer: {error}"))
        });
    let data_path = std::env::var_os("WATDIV_STORE_DATA").map_or_else(
        || {
            repo_root()
                .join("target/watdiv/dataset")
                .join(format!("watdiv.{scale}.nt"))
        },
        PathBuf::from,
    );
    let workloads_root = std::env::var_os("WATDIV_STORE_WORKLOAD_DIR").map_or_else(
        || repo_root().join("target/watdiv/workloads"),
        PathBuf::from,
    );
    ensure_watdiv_data(&data_path, scale);
    let stress_dir = workloads_root.join(format!("watdiv-stress-{scale}"));
    ensure_watdiv_stress_queries(&stress_dir, scale);
    let stress_queries = watdiv_sparql_queries(
        &stress_dir,
        &["test.1", "test.2", "test.3", "test.4", "test.5"],
    );

    let mut group = c.benchmark_group("store WatDiv operations in memory");
    group.sample_size(10);

    let store = Store::new().unwrap();
    do_bulk_load_file(&store, &data_path);
    for (batch_id, queries) in stress_queries.chunks(10).enumerate() {
        let first_query_id = batch_id * 10 + 1;
        let last_query_id = first_query_id + queries.len() - 1;
        group.bench_function(
            format!("WatDiv stress {scale} queries {first_query_id}-{last_query_id} in memory"),
            |b| b.iter(|| run_operations(&store, queries, true)),
        );
        group.bench_function(
            format!("WatDiv stress {scale} queries {first_query_id}-{last_query_id} in memory with incremental engine"),
            |b| b.iter(|| run_store_operations_incremental(&store, queries)),
        );
    }
}

fn watdiv_download_dir() -> PathBuf {
    repo_root().join("target/watdiv/downloads")
}

fn download_watdiv_archive(url: &str) -> PathBuf {
    let directory = watdiv_download_dir();
    fs::create_dir_all(&directory).unwrap();
    let path = directory.join(url.rsplit('/').next().unwrap());
    if !path.exists() {
        let client = oxhttp::Client::new()
            .with_redirection_limit(5)
            .with_user_agent(concat!("Oxigraph/", env!("CARGO_PKG_VERSION")))
            .unwrap();
        let request = Request::builder().uri(url).body(()).unwrap();
        let response = client.request(request).unwrap();
        assert!(
            response.status().is_success(),
            "{url} returned {}",
            response.status()
        );
        let mut temp = NamedTempFile::new_in(&directory).unwrap();
        std::io::copy(&mut response.into_body(), &mut temp).unwrap();
        temp.persist(&path).unwrap();
    }
    path
}

fn ensure_watdiv_data(path: &Path, scale: usize) {
    if path.exists() {
        return;
    }
    let size = match scale {
        100 => "10M",
        1000 => "100M",
        _ => panic!(
            "No official pre-generated WatDiv dataset for scale {scale}; provide WATDIV_STORE_DATA or use scale 100/1000"
        ),
    };
    let url = format!("https://dsg.uwaterloo.ca/watdiv/watdiv.{size}.tar.bz2");
    let archive_path = download_watdiv_archive(&url);
    let mut archive = tar::Archive::new(MultiBzDecoder::new(File::open(archive_path).unwrap()));
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mut output = NamedTempFile::new_in(path.parent().unwrap()).unwrap();
    let expected = format!("watdiv.{size}.nt");
    let mut found = false;
    for entry in archive.entries().unwrap() {
        let mut entry = entry.unwrap();
        if entry.path().unwrap() == Path::new(&expected) {
            std::io::copy(&mut entry, &mut output).unwrap();
            found = true;
            break;
        }
    }
    assert!(found, "{url} does not contain {expected}");
    output.persist(path).unwrap();
}

fn ensure_watdiv_stress_queries(directory: &Path, scale: usize) {
    let names = ["test.1", "test.2", "test.3", "test.4", "test.5"];
    if names
        .iter()
        .all(|name| directory.join(format!("{name}.sparql")).exists())
    {
        return;
    }
    assert!(
        scale == 100 || scale == 1000,
        "No official pre-generated WatDiv stress queries for scale {scale}; provide WATDIV_STORE_WORKLOAD_DIR or use scale 100/1000"
    );
    let url = "https://dsg.uwaterloo.ca/watdiv/stress-workloads.tar.gz";
    let archive_path = download_watdiv_archive(url);
    let mut archive = tar::Archive::new(GzDecoder::new(File::open(archive_path).unwrap()));
    fs::create_dir_all(directory).unwrap();
    for entry in archive.entries().unwrap() {
        let mut entry = entry.unwrap();
        let path = entry.path().unwrap();
        if path.parent() != Some(Path::new(directory.file_name().unwrap()))
            || path
                .extension()
                .is_none_or(|extension| extension != "sparql")
        {
            continue;
        }
        let target = directory.join(path.file_name().unwrap());
        let mut output = NamedTempFile::new_in(directory).unwrap();
        std::io::copy(&mut entry, &mut output).unwrap();
        output.persist(target).unwrap();
    }
    assert!(
        names
            .iter()
            .all(|name| directory.join(format!("{name}.sparql")).exists()),
        "{url} does not contain the complete query set for scale {scale}"
    );
}

fn run_operations(store: &Store, operations: &[Operation], with_opts: bool) {
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
            Operation::Update(u) => evaluator
                .clone()
                .for_update(u.clone())
                .on_store(store)
                .execute()
                .unwrap(),
        }
    }
}

#[derive(Clone, Copy)]
enum UnsupportedQueryHandling {
    Skip,
}

fn run_operations_incremental(
    dataset: &Dataset,
    operations: &[Operation],
    with_opts: bool,
    unsupported: UnsupportedQueryHandling,
) {
    for operation in operations {
        let Operation::Query(query) = operation else {
            panic!("incremental operation benchmark only supports queries");
        };
        let mut evaluator = QueryEvaluator::new();
        if !with_opts {
            evaluator = evaluator.without_optimizations();
        }
        let mut state = match evaluator
            .prepare(query)
            .execute_incremental_results(dataset)
        {
            Ok(state) => state,
            Err(error) => {
                if matches!(unsupported, UnsupportedQueryHandling::Skip)
                    && is_unsupported_incremental_error(&error)
                {
                    continue;
                }
                panic!("incremental query execution failed: {error}");
            }
        };
        match state.results() {
            Ok(spareval::IncrementalQueryResults::Solutions(results)) => for _ in results {},
            Ok(spareval::IncrementalQueryResults::Boolean(_)) => (),
            Ok(spareval::IncrementalQueryResults::Graph(results)) => for _ in results {},
            Err(error)
                if matches!(unsupported, UnsupportedQueryHandling::Skip)
                    && is_unsupported_incremental_error(&error) => {}
            Err(error) => panic!("incremental query results failed: {error}"),
        }
    }
}

fn run_store_operations_incremental(store: &Store, operations: &[Operation]) {
    for operation in operations {
        let Operation::Query(query) = operation else {
            panic!("incremental operation benchmark only supports queries");
        };
        let mut state = SparqlEvaluator::new()
            .for_query(query.clone())
            .on_store(store)
            .execute_incremental_results()
            .unwrap_or_else(|error| panic!("incremental query execution failed: {error}"));
        let results = state
            .results()
            .unwrap_or_else(|error| panic!("incremental query results failed: {error}"));
        match results {
            incroxigraph::sparql::IncrementalQueryResults::Solutions(results) => {
                for _ in results {}
            }
            incroxigraph::sparql::IncrementalQueryResults::Boolean(_) => (),
            incroxigraph::sparql::IncrementalQueryResults::Graph(results) => for _ in results {},
        }
    }
}

fn is_unsupported_incremental_error(error: &QueryEvaluationError) -> bool {
    let QueryEvaluationError::Unexpected(error) = error else {
        return false;
    };
    error
        .downcast_ref::<std::io::Error>()
        .is_some_and(|error| error.kind() == std::io::ErrorKind::Unsupported)
}

fn sparql_parsing(c: &mut Criterion) {
    if !workload_selected_by_filter("bsbm") {
        return;
    }
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
criterion_group!(
    store,
    sparql_parsing,
    store_query_and_update,
    store_watdiv,
    store_load
);
criterion_main!(parse, store);

fn workload_selected_by_filter(workload: &str) -> bool {
    let mut bsbm = false;
    let mut watdiv = false;
    for arg in std::env::args().skip(1) {
        let arg = arg.to_ascii_lowercase();
        bsbm |= arg.contains("bsbm");
        watdiv |= arg.contains("watdiv");
    }
    match (bsbm, watdiv) {
        (true, false) => workload == "bsbm",
        (false, true) => workload == "watdiv",
        _ => true,
    }
}

fn read_bz2_data(url: &str) -> Vec<u8> {
    let url = Uri::from_str(url).unwrap();
    let target_data_dir = Path::new("benches").join("data");
    fs::create_dir_all(&target_data_dir).unwrap();
    let file_path = target_data_dir.join(url.path().split('/').next_back().unwrap());
    if !file_path.exists() {
        let client = oxhttp::Client::new()
            .with_redirection_limit(5)
            .with_user_agent(concat!("Oxigraph/", env!("CARGO_PKG_VERSION")))
            .unwrap();
        let request = Request::builder().uri(&url).body(()).unwrap();
        let response = client.request(request).unwrap();
        assert!(
            response.status().is_success(),
            "{url} returned {} with body:\n{}",
            response.status(),
            response.into_body().to_string().unwrap()
        );
        std::io::copy(
            &mut response.into_body(),
            &mut File::create(&file_path).unwrap(),
        )
        .unwrap();
    }
    let mut buf = Vec::new();
    MultiBzDecoder::new(File::open(&file_path).unwrap())
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

fn watdiv_sparql_queries(directory: &Path, names: &[&str]) -> Vec<Operation> {
    names
        .iter()
        .flat_map(|name| {
            let path = directory.join(format!("{name}.sparql"));
            read_watdiv_queries(&path)
        })
        .map(|query| Operation::Query(Query::from_str(&query).unwrap()))
        .collect()
}

fn read_watdiv_queries(path: &Path) -> Vec<String> {
    let mut queries = Vec::new();
    let mut query = String::new();
    let mut brace_depth = 0_i64;
    for line in fs::read_to_string(path).unwrap().lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if !query.is_empty() {
            query.push(' ');
        }
        query.push_str(line);
        brace_depth += i64::try_from(line.matches('{').count()).unwrap();
        brace_depth -= i64::try_from(line.matches('}').count()).unwrap();
        if brace_depth == 0 && line.ends_with('}') {
            queries.push(std::mem::take(&mut query));
        }
    }
    assert!(
        query.is_empty(),
        "incomplete WatDiv query in {}",
        path.display()
    );
    queries
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_owned()
}

fn sparqloscope_operations() -> Vec<(String, Vec<Operation>)> {
    csv::Reader::from_reader(include_bytes!("sparqloscope-bsbm-5000.csv").as_slice())
        .records()
        .map(|record| {
            let record = record.unwrap();
            (
                record[0].into(),
                vec![Operation::Query(Query::from_str(&record[1]).unwrap())],
            )
        })
        .collect()
}

#[derive(Clone)]
enum RawOperation {
    Query(String),
    Update(String),
}

#[derive(Clone)]
enum Operation {
    Query(Query),
    Update(Update),
}
