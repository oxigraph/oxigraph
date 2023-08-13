#![no_main]

use lazy_static::lazy_static;
use libfuzzer_sys::fuzz_target;
use oxigraph::io::RdfFormat;
use oxigraph::sparql::{Query, QueryOptions, QueryResults, QuerySolutionIter};
use oxigraph::store::Store;

lazy_static! {
    static ref STORE: Store = {
        let store = Store::new().unwrap();
        store
            .load_dataset(sparql_smith::DATA_TRIG.as_bytes(), RdfFormat::TriG, None)
            .unwrap();
        store
    };
}

fuzz_target!(|data: sparql_smith::Query| {
    let query_str = data.to_string();
    if let Ok(query) = Query::parse(&query_str, None) {
        let options = QueryOptions::default();
        let with_opt = STORE.query_opt(query.clone(), options.clone()).unwrap();
        let without_opt = STORE
            .query_opt(query, options.without_optimizations())
            .unwrap();
        match (with_opt, without_opt) {
            (QueryResults::Solutions(with_opt), QueryResults::Solutions(without_opt)) => {
                assert_eq!(
                    query_solutions_key(with_opt, query_str.contains(" REDUCED ")),
                    query_solutions_key(without_opt, query_str.contains(" REDUCED "))
                )
            }
            (QueryResults::Graph(_), QueryResults::Graph(_)) => unimplemented!(),
            (QueryResults::Boolean(with_opt), QueryResults::Boolean(without_opt)) => {
                assert_eq!(with_opt, without_opt)
            }
            _ => panic!("Different query result types"),
        }
    }
});

fn query_solutions_key(iter: QuerySolutionIter, is_reduced: bool) -> String {
    // TODO: ordering
    let mut b = iter
        .into_iter()
        .map(|t| {
            let mut b = t
                .unwrap()
                .iter()
                .map(|(var, val)| format!("{}: {}", var, val))
                .collect::<Vec<_>>();
            b.sort_unstable();
            b.join(" ")
        })
        .collect::<Vec<_>>();
    b.sort_unstable();
    if is_reduced {
        b.dedup();
    }
    b.join("\n")
}
