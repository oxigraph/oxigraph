#![no_main]

use libfuzzer_sys::fuzz_target;
use oxigraph::io::RdfFormat;
use oxigraph::model::graph::CanonicalizationAlgorithm;
use oxigraph::model::{Graph, NamedNode};
use oxigraph::sparql::{EvaluationError, Query, QueryOptions, QueryResults, ServiceHandler};
use oxigraph::store::Store;
use oxigraph_fuzz::count_triple_blank_nodes;
use std::sync::OnceLock;

fuzz_target!(|data: sparql_smith::Query| {
    static STORE: OnceLock<Store> = OnceLock::new();
    let store = STORE.get_or_init(|| {
        let store = Store::new().unwrap();
        store
            .load_from_reader(RdfFormat::TriG, sparql_smith::DATA_TRIG.as_bytes())
            .unwrap();
        store
    });

    let query_str = data.to_string();
    if let Ok(query) = Query::parse(&query_str, None) {
        let options = QueryOptions::default().with_service_handler(StoreServiceHandler {
            store: store.clone(),
        });
        let with_opt = store.query_opt(query.clone(), options.clone());
        let without_opt = store.query_opt(query, options.without_optimizations());
        assert_eq!(
            query_results_key(with_opt, query_str.contains(" REDUCED ")),
            query_results_key(without_opt, query_str.contains(" REDUCED "))
        )
    }
});

fn query_results_key(results: Result<QueryResults, EvaluationError>, is_reduced: bool) -> String {
    match results {
        Ok(QueryResults::Solutions(iter)) => {
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
        Ok(QueryResults::Graph(iter)) => {
            let mut graph = iter.filter_map(Result::ok).collect::<Graph>();
            if graph.iter().map(count_triple_blank_nodes).sum::<usize>() > 4 {
                return String::new(); // canonicalization might be too slow
            };
            graph.canonicalize(CanonicalizationAlgorithm::Unstable);
            let mut triples = graph.into_iter().map(|t| t.to_string()).collect::<Vec<_>>();
            triples.sort_unstable();
            triples.join("\n")
        }
        Ok(QueryResults::Boolean(bool)) => if bool { "true" } else { "" }.into(),
        Err(_) => String::new(),
    }
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
