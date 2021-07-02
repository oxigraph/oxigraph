use criterion::{criterion_group, criterion_main, Criterion};
use oxigraph::sparql::*;
use oxigraph_testsuite::files::read_file_to_string;
use oxigraph_testsuite::manifest::TestManifest;

criterion_group!(sparql, sparql_w3c_syntax_bench);

criterion_main!(sparql);

fn sparql_w3c_syntax_bench(c: &mut Criterion) {
    let manifest_urls = vec![
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/manifest-syntax.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/manifest-sparql11-query.ttl",
    ];
    let queries: Vec<_> = TestManifest::new(manifest_urls)
        .flat_map(|test| {
            let test = test.unwrap();
            if test.kind == "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#PositiveSyntaxTest"
                || test.kind
                == "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#PositiveSyntaxTest11" {
                test.action.map(|query| (read_file_to_string(&query).unwrap(), query))
            } else {
                None
            }
        })
        .collect();

    c.bench_function("query parser", |b| {
        b.iter(|| {
            for (query, base) in &queries {
                Query::parse(query, Some(base)).unwrap();
            }
        })
    });
}
