#![no_main]

use lazy_static::lazy_static;
use libfuzzer_sys::fuzz_target;
use oxigraph::io::DatasetFormat;
use oxigraph::sparql::{Query, QueryOptions, QueryResults, QuerySolutionIter, RuleSet};
use oxigraph::store::Store;

lazy_static! {
    static ref STORE: Store = {
        let store = Store::new().unwrap();
        store
            .load_dataset(
                sparql_smith::DATA_TRIG.as_bytes(),
                DatasetFormat::TriG,
                None,
            )
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
            .query_opt(query.clone(), options.clone().without_optimizations())
            .unwrap();
        compare_results(with_opt, without_opt, &query_str);
        let with_opt_and_reasoning = STORE
            .query_opt(
                query.clone(),
                options
                    .clone()
                    .with_inference_rules(RuleSet::default())
                    .clone(),
            )
            .unwrap();
        let with_opt = STORE.query_opt(query.clone(), options).unwrap();
        compare_results(with_opt, with_opt_and_reasoning, &query_str);
    }
});

fn compare_results(a: QueryResults, b: QueryResults, query_str: &str) {
    match (a, b) {
        (QueryResults::Solutions(a), QueryResults::Solutions(b)) => {
            assert_eq!(
                query_solutions_key(a, query_str.contains(" REDUCED ")),
                query_solutions_key(b, query_str.contains(" REDUCED "))
            )
        }
        (QueryResults::Graph(_), QueryResults::Graph(_)) => unimplemented!(),
        (QueryResults::Boolean(a), QueryResults::Boolean(b)) => {
            assert_eq!(a, b)
        }
        _ => panic!("Different query result types"),
    }
}

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
