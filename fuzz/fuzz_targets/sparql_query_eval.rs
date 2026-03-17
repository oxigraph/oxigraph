#![no_main]

use libfuzzer_sys::fuzz_target;
use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::graph::CanonicalizationAlgorithm;
use oxigraph::model::{Dataset, Graph, NamedNode};
use oxigraph::sparql::{
    DefaultServiceHandler, QueryEvaluationError, QueryResults, QuerySolutionIter, SparqlEvaluator,
};
use oxigraph::store::Store;
use oxigraph_fuzz::count_triple_blank_nodes;
use oxiri::Iri;
use oxrdf::{GraphNameRef, QuadRef};
use spareval::QueryEvaluator;
use spargebra::algebra::{GraphPattern, QueryDataset};
use spargebra::{Query, SparqlParser};
use std::sync::OnceLock;

fuzz_target!(|data: sparql_smith::Query| {
    static STORE: OnceLock<Store> = OnceLock::new();
    let store = STORE.get_or_init(|| {
        let store = Store::new().unwrap();
        store
            .load_from_slice(RdfFormat::TriG, sparql_smith::DATA_TRIG)
            .unwrap();
        store
    });

    static DATASET: OnceLock<Dataset> = OnceLock::new();
    let dataset = DATASET.get_or_init(|| {
        RdfParser::from(RdfFormat::TriG)
            .for_slice(sparql_smith::DATA_TRIG)
            .collect::<Result<_, _>>()
            .unwrap()
    });

    let query_str = data.to_string();
    if let Ok(query) = SparqlParser::new().parse_query(&query_str) {
        let with_opt = SparqlEvaluator::new()
            .with_default_service_handler(StoreServiceHandler {
                store: store.clone(),
            })
            .for_query(query.clone())
            .on_store(store)
            .execute();
        let without_opt = QueryEvaluator::new()
            .without_optimizations()
            .with_default_service_handler(DatasetServiceHandler {
                dataset: dataset.clone(),
            })
            .prepare(&query)
            .execute(dataset);
        match (with_opt, without_opt) {
            (Ok(with_opt), Ok(without_opt)) => {
                assert_eq!(
                    query_results_key(with_opt, query_str.contains(" REDUCED ")),
                    query_results_key(without_opt, query_str.contains(" REDUCED "))
                )
            }
            (Err(_), Err(_)) => (),
            (Ok(r), Err(e)) => {
                if !matches!(r, QueryResults::Boolean(false)) {
                    panic!("with optimizations passed whereas without optimizations failed: {e}")
                }
            }
            (Err(e), Ok(r)) => {
                if !matches!(r, QueryResults::Boolean(false)) {
                    panic!("without optimizations passed whereas with optimizations failed: {e}")
                }
            }
        }

        // Parsing roundtrip
        let roundtrip_query = SparqlParser::new()
            .parse_query(&query.to_string())
            .expect(&format!("Invalid roundtrip {query}"));

        let roundtrip = QueryEvaluator::new()
            .without_optimizations()
            .prepare(&roundtrip_query)
            .execute(dataset);
        let without_opt = QueryEvaluator::new()
            .without_optimizations()
            .with_default_service_handler(DatasetServiceHandler {
                dataset: dataset.clone(),
            })
            .prepare(&query)
            .execute(dataset);

        match (roundtrip, without_opt) {
            (Ok(roundtrip), Ok(without_opt)) => {
                assert_eq!(
                    query_results_key(roundtrip, query_str.contains(" REDUCED ")),
                    query_results_key(without_opt, query_str.contains(" REDUCED "))
                )
            }
            (Err(_), Err(_)) => (),
            (Ok(r), Err(e)) => {
                if !matches!(r, QueryResults::Boolean(false)) {
                    panic!("roundtripped passed whereas without optimizations failed: {e}")
                }
            }
            (Err(e), Ok(r)) => {
                if !matches!(r, QueryResults::Boolean(false)) {
                    panic!("roundtripped passed whereas with optimizations failed: {e}")
                }
            }
        }
    }
});

fn query_results_key(results: QueryResults, is_reduced: bool) -> String {
    match results {
        QueryResults::Solutions(iter) => {
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
        QueryResults::Graph(iter) => {
            let mut graph = iter.filter_map(Result::ok).collect::<Graph>();
            if graph.iter().map(count_triple_blank_nodes).sum::<usize>() > 4 {
                return String::new(); // canonicalization might be too slow
            };
            graph.canonicalize(CanonicalizationAlgorithm::Unstable);
            let mut triples = graph.into_iter().map(|t| t.to_string()).collect::<Vec<_>>();
            triples.sort_unstable();
            triples.join("\n")
        }
        QueryResults::Boolean(bool) => if bool { "true" } else { "false" }.into(),
    }
}

#[derive(Clone)]
struct StoreServiceHandler {
    store: Store,
}

impl DefaultServiceHandler for StoreServiceHandler {
    type Error = QueryEvaluationError;

    fn handle(
        &self,
        service_name: &NamedNode,
        pattern: &GraphPattern,
        base_iri: Option<&Iri<String>>,
    ) -> Result<QuerySolutionIter<'static>, QueryEvaluationError> {
        if !self
            .store
            .contains_named_graph(service_name)
            .map_err(|e| QueryEvaluationError::Dataset(Box::new(e)))?
        {
            return Err(QueryEvaluationError::Service("Graph does not exist".into()));
        }
        let QueryResults::Solutions(solutions) = SparqlEvaluator::new()
            .with_default_service_handler(self.clone())
            .for_query(Query::Select {
                dataset: Some(QueryDataset {
                    default: vec![service_name.clone()],
                    named: None,
                }),
                pattern: pattern.clone(),
                base_iri: base_iri.cloned(),
            })
            .on_store(&self.store)
            .execute()?
        else {
            unreachable!();
        };
        Ok(solutions)
    }
}

#[derive(Clone)]
struct DatasetServiceHandler {
    dataset: Dataset,
}

impl DefaultServiceHandler for DatasetServiceHandler {
    type Error = QueryEvaluationError;

    fn handle(
        &self,
        service_name: &NamedNode,
        pattern: &GraphPattern,
        base_iri: Option<&Iri<String>>,
    ) -> Result<QuerySolutionIter<'static>, QueryEvaluationError> {
        if self
            .dataset
            .quads_for_graph_name(service_name)
            .next()
            .is_none()
        {
            return Err(QueryEvaluationError::Service("Graph does not exist".into()));
        }

        let dataset = self
            .dataset
            .iter()
            .flat_map(|q| {
                if q.graph_name.is_default_graph() {
                    vec![]
                } else if q.graph_name == service_name.as_ref().into() {
                    vec![
                        QuadRef::new(q.subject, q.predicate, q.object, GraphNameRef::DefaultGraph),
                        q,
                    ]
                } else {
                    vec![q]
                }
            })
            .collect::<Dataset>();
        let evaluator = QueryEvaluator::new().with_default_service_handler(DatasetServiceHandler {
            dataset: dataset.clone(),
        });
        let QueryResults::Solutions(iter) = evaluator
            .prepare(&Query::Select {
                dataset: None,
                pattern: pattern.clone(),
                base_iri: base_iri.cloned(),
            })
            .execute(&dataset)?
        else {
            panic!()
        };
        Ok(QuerySolutionIter::new(
            iter.variables().into(),
            iter.collect::<Vec<_>>(),
        ))
    }
}
