#![no_main]

use libfuzzer_sys::fuzz_target;
use oxigraph::io::RdfFormat;
use oxigraph::model::NamedNode;
use oxigraph::sparql::{
    EvaluationError, Query, QueryOptions, QueryResults, QuerySolutionIter, ServiceHandler,
};
use oxigraph::store::Store;
use std::sync::OnceLock;

fuzz_target!(|data: sparql_smith::Query| {
    static STORE: OnceLock<Store> = OnceLock::new();
    let store = STORE.get_or_init(|| {
        let store = Store::new().unwrap();
        store
            .load_from_read(RdfFormat::TriG, sparql_smith::DATA_TRIG.as_bytes())
            .unwrap();
        store
    });

    let query_str = data.to_string();
    if let Ok(query) = Query::parse(&query_str, None) {
        let options = QueryOptions::default().with_service_handler(StoreServiceHandler {
            store: store.clone(),
        });
        let with_opt = store.query_opt(query.clone(), options.clone()).unwrap();
        let without_opt = store
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
        .filter_map(Result::ok)
        .map(|t| {
            let mut b = t
                .iter()
                .map(|(var, val)| format!("{var}: {val}"))
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

#[derive(Clone)]
struct StoreServiceHandler {
    store: Store,
}

impl ServiceHandler for StoreServiceHandler {
    type Error = EvaluationError;

    fn handle(
        &self,
        service_name: NamedNode,
        mut query: Query,
    ) -> Result<QueryResults, EvaluationError> {
        if !self.store.contains_named_graph(&service_name)? {
            return Err(EvaluationError::Service("Graph does not exist".into()));
        }
        query
            .dataset_mut()
            .set_default_graph(vec![service_name.into()]);
        self.store.query_opt(
            query,
            QueryOptions::default().with_service_handler(self.clone()),
        )
    }
}
