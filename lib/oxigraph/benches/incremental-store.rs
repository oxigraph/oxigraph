#![expect(clippy::panic)]

use bzip2::read::MultiBzDecoder;
use codspeed_criterion_compat::{
    BatchSize, Criterion, Throughput, criterion_group, criterion_main,
};
use incroxigraph::io::{RdfFormat, RdfParser};
use incroxigraph::model::{GraphName, NamedNode, NamedOrBlankNode, Quad, Term};
use incroxigraph::sparql::{
    Delta, IncrementalQueryDeltasState, IncrementalQueryResults, IncrementalQueryResultsState,
    IncrementalSelectResults, PreparedSparqlQuery, QueryResults, QueryResultsDelta,
    SparqlEvaluator,
};
use incroxigraph::store::Store;
use oxhttp::model::{Request, Uri};
use spareval::IncrementalQueryableDataset;
use spargebra::Query;
use spargebra::algebra::QueryExpression;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::fs::File;
use std::hint::black_box;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Instant;
use tempfile::NamedTempFile;
use zip::ZipArchive;

fn select_results(results: IncrementalQueryResults<'_>) -> IncrementalSelectResults<'_> {
    let IncrementalQueryResults::Solutions(solutions) = results else {
        panic!("expected SELECT results");
    };
    solutions
}

fn incremental_store_bsbm(c: &mut Criterion) {
    if !workload_selected_by_filter("bsbm") {
        return;
    }
    do_incremental_query_update(c, 5_000);
}

fn incremental_store_watdiv(c: &mut Criterion) {
    if !workload_selected_by_filter("watdiv") {
        return;
    }
    let config = IncrementalWatdivConfig::published_workload();
    ensure_published_watdiv_workload(&config);
    let workload = Arc::new(prepare_incremental_watdiv_workload(&config));
    let measurements = validate_incremental_watdiv_workload(&workload);
    write_incremental_watdiv_results(&config, &workload, &measurements);
    print_incremental_watdiv_stats(&config, &workload, &measurements);

    let mut group = c.benchmark_group("store incremental operations");
    group.sample_size(10);
    group.throughput(Throughput::Elements(1));

    for case in workload.cases.clone() {
        {
            let workload = Arc::clone(&workload);
            let case = case.clone();
            group.bench_function(
                format!(
                    "WatDiv static {} stream {} seed {} fraction {} query {} {} incremental in memory",
                    config.static_scale,
                    config.stream_scale,
                    config.seed,
                    fraction_id(case.initial_stream_fraction),
                    case.query_id,
                    case.update_type.id()
                ),
                move |b| {
                    b.iter_batched_ref(
                        || initialize_incremental_watdiv_case_with_updates(&workload, &case),
                        |(store, state, _initial_result_count)| {
                            black_box(run_incremental_watdiv_case_without_updates(
                                store, state, &workload, &case,
                            ));
                        },
                        BatchSize::PerIteration,
                    )
                },
            );
        }

        {
            let workload = Arc::clone(&workload);
            let case = case.clone();
            group.bench_function(
                format!(
                    "WatDiv static {} stream {} seed {} fraction {} query {} {} reevaluate in memory",
                    config.static_scale,
                    config.stream_scale,
                    config.seed,
                    fraction_id(case.initial_stream_fraction),
                    case.query_id,
                    case.update_type.id()
                ),
                move |b| {
                    b.iter_batched_ref(
                        || initialize_final_watdiv_store(&workload, &case),
                        |store| {
                            black_box(count_watdiv_query_results(
                                store,
                                &workload.queries[case.query_id - 1],
                            ));
                        },
                        BatchSize::PerIteration,
                    )
                },
            );
        }
    }
}

fn do_bulk_load(store: &Store, data: &[u8]) {
    let mut loader = store.bulk_loader();
    loader
        .load_from_slice(RdfParser::from_format(RdfFormat::NTriples).lenient(), data)
        .unwrap();
    loader.commit().unwrap();
    store.optimize().unwrap();
}

fn do_incremental_query_update(c: &mut Criterion, data_size: usize) {
    let data = read_bz2_data(&format!(
        "https://zenodo.org/records/12663333/files/dataset-{data_size}.nt.bz2"
    ));
    let workload = generate_incremental_bsbm_workload(&data, data_size);
    // optional stats
    let stats = collect_incremental_bsbm_stats(&data, &workload);
    print_incremental_bsbm_stats(data_size, &workload, &stats);

    let mut group = c.benchmark_group("store incremental operations");
    group.sample_size(10);

    for query_id in 1..=4 {
        for scenario in IncrementalBsbmScenario::ALL {
            let case_workload = IncrementalBsbmWorkload {
                cases: workload
                    .cases
                    .iter()
                    .filter(|case| case.query_id == query_id && case.scenario == scenario)
                    .cloned()
                    .collect(),
            };
            group.throughput(Throughput::Elements(case_workload.cases.len() as u64));
            {
                let data = data.clone();
                let case_workload = case_workload.clone();
                group.bench_function(
                    format!(
                        "BSBM explore {data_size} query {query_id} {} incremental product workload in memory with index time",
                        scenario.id()
                    ),
                    move |b| {
                        b.iter_batched(
                            || initialize_incremental_workload_states_without_updates(&data, &case_workload),
                            |states| {
                                black_box(run_incremental_bsbm_workload_with_updates(states));
                            },
                            BatchSize::SmallInput,
                        )
                    },
                );
            }

            {
                let data = data.clone();
                let case_workload = case_workload.clone();
                group.bench_function(
                    format!(
                        "BSBM explore {data_size} query {query_id} {} incremental product workload in memory without index time",
                        scenario.id()
                    ),
                    move |b| {
                        b.iter_batched(
                            || initialize_incremental_workload_states_with_updates(&data, &case_workload),
                            |states| {
                                black_box(run_incremental_bsbm_workload(states));
                            },
                            BatchSize::SmallInput,
                        )
                    },
                );
            }

            {
                let data = data.clone();
                let case_workload = case_workload.clone();
                group.bench_function(
                    format!(
                        "BSBM explore {data_size} query {query_id} {} recompute product workload in memory with index time",
                        scenario.id()
                    ),
                    move |b| {
                        b.iter_batched(
                            || initialize_recompute_workload_stores_without_updates(&data, &case_workload),
                            |states| {
                                black_box(run_recompute_bsbm_workload_with_updates(states));
                            },
                            BatchSize::SmallInput,
                        )
                    },
                );
            }

            {
                let data = data.clone();
                let case_workload = case_workload.clone();
                group.bench_function(
                    format!(
                        "BSBM explore {data_size} query {query_id} {} recompute product workload in memory without index time",
                        scenario.id()
                    ),
                    move |b| {
                        b.iter_batched(
                            || initialize_recompute_workload_stores_with_updates(&data, &case_workload),
                            |states| {
                                black_box(run_recompute_bsbm_workload(states));
                            },
                            BatchSize::SmallInput,
                        )
                    },
                );
            }
        }
    }
}

#[derive(Clone)]
struct IncrementalBsbmWorkload {
    cases: Vec<IncrementalBsbmCase>,
}

#[derive(Clone)]
struct IncrementalBsbmCase {
    query_id: usize,
    scenario: IncrementalBsbmScenario,
    query: Query,
    base_removals: Vec<Quad>,
    additions: Vec<Quad>,
    deletions: Vec<Quad>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum IncrementalBsbmScenario {
    RelevantDeletion,
    RelevantInsertion,
    IrrelevantDeletion,
    IrrelevantInsertion,
}

impl IncrementalBsbmScenario {
    const ALL: [Self; 4] = [
        Self::RelevantDeletion,
        Self::RelevantInsertion,
        Self::IrrelevantDeletion,
        Self::IrrelevantInsertion,
    ];

    fn name(self) -> &'static str {
        match self {
            Self::RelevantDeletion => "relevant deletion",
            Self::RelevantInsertion => "relevant insertion",
            Self::IrrelevantDeletion => "irrelevant deletion",
            Self::IrrelevantInsertion => "irrelevant insertion",
        }
    }

    fn id(self) -> &'static str {
        match self {
            Self::RelevantDeletion => "relevant deletion",
            Self::RelevantInsertion => "relevant addition",
            Self::IrrelevantDeletion => "irrelevant deletion",
            Self::IrrelevantInsertion => "irrelevant addition",
        }
    }
}

#[derive(Clone)]
struct BsbmProduct {
    iri: NamedNode,
    product_type: Option<NamedNode>,
    features: Vec<NamedNode>,
    numeric1: Option<i64>,
    numeric2: Option<i64>,
    numeric3: Option<i64>,
}

fn generate_incremental_bsbm_workload(data: &[u8], data_size: usize) -> IncrementalBsbmWorkload {
    let store = initialize_store(data);
    let products = collect_bsbm_products(&store);
    let mut cases = Vec::new();

    for query_id in 1..=4 {
        let (query, relevant_product) = choose_incremental_bsbm_query(&store, &products, query_id)
            .unwrap_or_else(|| {
                panic!("could not instantiate BSBM Explore query {query_id} with 0-3 products for dataset {data_size}")
            });
        let relevant_triples = triples_involving_product(&store, &relevant_product);
        let irrelevant_product = choose_irrelevant_product(&products, query_id, &relevant_product)
            .unwrap_or_else(|| {
                panic!("could not find irrelevant product for BSBM Explore query {query_id} and dataset {data_size}")
            });
        let irrelevant_triples = triples_involving_product(&store, &irrelevant_product);

        cases.push(build_incremental_bsbm_case(
            query_id,
            IncrementalBsbmScenario::RelevantDeletion,
            query.clone(),
            Vec::new(),
            Vec::new(),
            relevant_triples.clone(),
        ));
        cases.push(build_incremental_bsbm_case(
            query_id,
            IncrementalBsbmScenario::RelevantInsertion,
            query.clone(),
            relevant_triples.clone(),
            relevant_triples,
            Vec::new(),
        ));
        cases.push(build_incremental_bsbm_case(
            query_id,
            IncrementalBsbmScenario::IrrelevantDeletion,
            query.clone(),
            Vec::new(),
            Vec::new(),
            irrelevant_triples.clone(),
        ));
        cases.push(build_incremental_bsbm_case(
            query_id,
            IncrementalBsbmScenario::IrrelevantInsertion,
            query,
            irrelevant_triples.clone(),
            irrelevant_triples,
            Vec::new(),
        ));
    }

    IncrementalBsbmWorkload { cases }
}

fn build_incremental_bsbm_case(
    query_id: usize,
    scenario: IncrementalBsbmScenario,
    query: Query,
    base_removals: Vec<Quad>,
    additions: Vec<Quad>,
    deletions: Vec<Quad>,
) -> IncrementalBsbmCase {
    IncrementalBsbmCase {
        query_id,
        scenario,
        query,
        base_removals,
        additions,
        deletions,
    }
}

fn collect_bsbm_products(store: &Store) -> Vec<BsbmProduct> {
    let rdf_type = named_node("http://www.w3.org/1999/02/22-rdf-syntax-ns#type");
    let product_feature =
        named_node("http://www4.wiwiss.fu-berlin.de/bizer/bsbm/v01/vocabulary/productFeature");
    let numeric1 = named_node(
        "http://www4.wiwiss.fu-berlin.de/bizer/bsbm/v01/vocabulary/productPropertyNumeric1",
    );
    let numeric2 = named_node(
        "http://www4.wiwiss.fu-berlin.de/bizer/bsbm/v01/vocabulary/productPropertyNumeric2",
    );
    let numeric3 = named_node(
        "http://www4.wiwiss.fu-berlin.de/bizer/bsbm/v01/vocabulary/productPropertyNumeric3",
    );
    let product_prefix =
        "http://www4.wiwiss.fu-berlin.de/bizer/bsbm/v01/instances/dataFromProducer";
    let product_type_prefix =
        "http://www4.wiwiss.fu-berlin.de/bizer/bsbm/v01/instances/ProductType";

    let mut products = HashMap::new();
    for quad in store.quads_for_pattern(None, None, None, None) {
        let quad = quad.unwrap();
        let NamedOrBlankNode::NamedNode(product) = quad.subject else {
            continue;
        };
        if !product.as_str().contains("/Product") || !product.as_str().starts_with(product_prefix) {
            continue;
        }
        let product_entry = products
            .entry(product.clone())
            .or_insert_with(|| BsbmProduct {
                iri: product,
                product_type: None,
                features: Vec::new(),
                numeric1: None,
                numeric2: None,
                numeric3: None,
            });
        if quad.predicate == rdf_type {
            if let Term::NamedNode(product_type) = quad.object
                && product_type.as_str().starts_with(product_type_prefix)
            {
                product_entry.product_type = Some(product_type);
            }
        } else if quad.predicate == product_feature {
            if let Term::NamedNode(feature) = quad.object {
                product_entry.features.push(feature);
            }
        } else if quad.predicate == numeric1 {
            if let Term::Literal(value) = quad.object {
                product_entry.numeric1 = value.value().parse().ok();
            }
        } else if quad.predicate == numeric2 {
            if let Term::Literal(value) = quad.object {
                product_entry.numeric2 = value.value().parse().ok();
            }
        } else if quad.predicate == numeric3
            && let Term::Literal(value) = quad.object
        {
            product_entry.numeric3 = value.value().parse().ok();
        }
    }

    let mut products = products
        .into_values()
        .filter(|product| product.product_type.is_some())
        .collect::<Vec<_>>();
    for product in &mut products {
        product.features.sort_by(|a, b| a.as_str().cmp(b.as_str()));
        product.features.dedup();
    }
    products.sort_by(|a, b| a.iri.as_str().cmp(b.iri.as_str()));
    products
}

fn choose_incremental_bsbm_query(
    store: &Store,
    products: &[BsbmProduct],
    query_id: usize,
) -> Option<(Query, NamedNode)> {
    for product in products {
        let estimated_product_count =
            estimate_incremental_bsbm_product_count(products, product, query_id)?;
        if !(1..=3).contains(&estimated_product_count) {
            continue;
        }
        let Some(query_text) = instantiate_incremental_bsbm_query(product, products, query_id)
        else {
            continue;
        };
        let Some(query) = parse_incremental_bsbm_select_query(&query_text) else {
            continue;
        };
        if count_query_results(store, &query) > 0 {
            return Some((query, product.iri.clone()));
        }
    }
    None
}

fn estimate_incremental_bsbm_product_count(
    products: &[BsbmProduct],
    product: &BsbmProduct,
    query_id: usize,
) -> Option<usize> {
    let product_type = product.product_type.as_ref()?;
    let feature1 = product.features.first()?;
    let feature2 = product.features.get(1);
    let feature3 = product.features.get(2).or(feature2);
    let x = product.numeric1?.saturating_sub(1);
    match query_id {
        1 => {
            let feature2 = feature2?;
            Some(
                products
                    .iter()
                    .filter(|candidate| {
                        candidate.product_type.as_ref() == Some(product_type)
                            && candidate.features.contains(feature1)
                            && candidate.features.contains(feature2)
                            && candidate.numeric1.is_some_and(|value| value > x)
                    })
                    .count(),
            )
        }
        2 => Some(1),
        3 => {
            let absent_feature = products
                .iter()
                .flat_map(|p| &p.features)
                .find(|feature| !product.features.contains(feature))?;
            let y = product.numeric3?.saturating_add(1);
            Some(
                products
                    .iter()
                    .filter(|candidate| {
                        candidate.product_type.as_ref() == Some(product_type)
                            && candidate.features.contains(feature1)
                            && candidate.numeric1.is_some_and(|value| value > x)
                            && candidate.numeric3.is_some_and(|value| value < y)
                            && !candidate.features.contains(absent_feature)
                    })
                    .count(),
            )
        }
        4 => {
            let feature2 = feature2?;
            let feature3 = feature3?;
            let y = product.numeric2?.saturating_sub(1);
            Some(
                products
                    .iter()
                    .filter(|candidate| {
                        candidate.product_type.as_ref() == Some(product_type)
                            && candidate.features.contains(feature1)
                            && ((candidate.features.contains(feature2)
                                && candidate.numeric1.is_some_and(|value| value > x))
                                || (candidate.features.contains(feature3)
                                    && candidate.numeric2.is_some_and(|value| value > y)))
                    })
                    .count(),
            )
        }
        _ => None,
    }
}

fn instantiate_incremental_bsbm_query(
    product: &BsbmProduct,
    products: &[BsbmProduct],
    query_id: usize,
) -> Option<String> {
    let product_type = product.product_type.as_ref()?;
    let feature1 = product.features.first()?;
    let feature2 = product.features.get(1);
    let feature3 = product.features.get(2).or(feature2);
    let x = product.numeric1?.saturating_sub(1);

    match query_id {
        1 => Some(
            EXPLORE_QUERY_1
                .replace("%ProductType%", &product_type.to_string())
                .replace("%ProductFeature1%", &feature1.to_string())
                .replace("%ProductFeature2%", &feature2?.to_string())
                .replace("%x%", &x.to_string()),
        ),
        2 => Some(EXPLORE_QUERY_2.replace("%ProductXYZ%", &product.iri.to_string())),
        3 => {
            let absent_feature = products
                .iter()
                .flat_map(|p| &p.features)
                .find(|feature| !product.features.contains(feature))?;
            Some(
                EXPLORE_QUERY_3_MINUS
                    .replace("%ProductType%", &product_type.to_string())
                    .replace("%ProductFeature1%", &feature1.to_string())
                    .replace("%ProductFeature2%", &absent_feature.to_string())
                    .replace("%x%", &x.to_string())
                    .replace("%y%", &product.numeric3?.saturating_add(1).to_string()),
            )
        }
        4 => Some(
            EXPLORE_QUERY_4
                .replace("%ProductType%", &product_type.to_string())
                .replace("%ProductFeature1%", &feature1.to_string())
                .replace("%ProductFeature2%", &feature2?.to_string())
                .replace("%ProductFeature3%", &feature3?.to_string())
                .replace("%x%", &x.to_string())
                .replace("%y%", &product.numeric2?.saturating_sub(1).to_string()),
        ),
        _ => None,
    }
}

fn choose_irrelevant_product(
    products: &[BsbmProduct],
    query_id: usize,
    relevant_product: &NamedNode,
) -> Option<NamedNode> {
    let selected_product = products
        .iter()
        .find(|product| &product.iri == relevant_product)?;
    for product in products {
        if &product.iri == relevant_product {
            continue;
        }
        if !matches_incremental_bsbm_query(product, selected_product, products, query_id)? {
            return Some(product.iri.clone());
        }
    }
    None
}

fn matches_incremental_bsbm_query(
    candidate: &BsbmProduct,
    selected: &BsbmProduct,
    products: &[BsbmProduct],
    query_id: usize,
) -> Option<bool> {
    let product_type = selected.product_type.as_ref()?;
    let feature1 = selected.features.first()?;
    let feature2 = selected.features.get(1);
    let feature3 = selected.features.get(2).or(feature2);
    let x = selected.numeric1?.saturating_sub(1);
    match query_id {
        1 => {
            let feature2 = feature2?;
            Some(
                candidate.product_type.as_ref() == Some(product_type)
                    && candidate.features.contains(feature1)
                    && candidate.features.contains(feature2)
                    && candidate.numeric1.is_some_and(|value| value > x),
            )
        }
        2 => Some(candidate.iri == selected.iri),
        3 => {
            let absent_feature = products
                .iter()
                .flat_map(|p| &p.features)
                .find(|feature| !selected.features.contains(feature))?;
            let y = selected.numeric3?.saturating_add(1);
            Some(
                candidate.product_type.as_ref() == Some(product_type)
                    && candidate.features.contains(feature1)
                    && candidate.numeric1.is_some_and(|value| value > x)
                    && candidate.numeric3.is_some_and(|value| value < y)
                    && !candidate.features.contains(absent_feature),
            )
        }
        4 => {
            let feature2 = feature2?;
            let feature3 = feature3?;
            let y = selected.numeric2?.saturating_sub(1);
            Some(
                candidate.product_type.as_ref() == Some(product_type)
                    && candidate.features.contains(feature1)
                    && ((candidate.features.contains(feature2)
                        && candidate.numeric1.is_some_and(|value| value > x))
                        || (candidate.features.contains(feature3)
                            && candidate.numeric2.is_some_and(|value| value > y))),
            )
        }
        _ => None,
    }
}

fn triples_involving_product(store: &Store, product: &NamedNode) -> Vec<Quad> {
    let product_subject = NamedOrBlankNode::from(product.clone());
    let product_object = Term::from(product.clone());
    let mut triples = store
        .quads_for_pattern(Some(&product_subject), None, None, None)
        .map(|quad| quad.unwrap())
        .collect::<Vec<_>>();
    let mut seen = triples.iter().cloned().collect::<HashSet<_>>();
    for quad in store.quads_for_pattern(None, None, Some(&product_object), None) {
        let quad = quad.unwrap();
        if seen.insert(quad.clone()) {
            triples.push(quad);
        }
    }
    triples
}

fn initialize_incremental_workload_states_without_updates(
    data: &[u8],
    workload: &IncrementalBsbmWorkload,
) -> Vec<(
    Store,
    SparqlEvaluator,
    IncrementalQueryResultsState<'static, impl IncrementalQueryableDataset<'static>>,
    IncrementalBsbmCase,
)> {
    workload
        .cases
        .iter()
        .map(|case| {
            let store = initialize_store(data);
            apply_quads(&store, &[], &case.base_removals);
            let evaluator = SparqlEvaluator::new().without_optimizations();
            let mut state = evaluator
                .clone()
                .for_query(case.query.clone())
                .on_store(&store)
                .execute_incremental_results()
                .unwrap();
            select_results(state.results().unwrap()).count();
            (store, evaluator, state, case.clone())
        })
        .collect()
}

fn initialize_incremental_workload_states_with_updates(
    data: &[u8],
    workload: &IncrementalBsbmWorkload,
) -> Vec<(
    Store,
    SparqlEvaluator,
    IncrementalQueryResultsState<'static, impl IncrementalQueryableDataset<'static>>,
    IncrementalBsbmCase,
)> {
    let states = initialize_incremental_workload_states_without_updates(data, workload);
    states
        .into_iter()
        .map(|(store, evaluator, state, case)| {
            apply_quads(&store, &case.additions, &case.deletions);
            (store, evaluator, state, case)
        })
        .collect()
}

fn run_incremental_bsbm_workload_with_updates<'a, D: IncrementalQueryableDataset<'a>>(
    states: Vec<(
        Store,
        SparqlEvaluator,
        IncrementalQueryResultsState<'a, D>,
        IncrementalBsbmCase,
    )>,
) -> usize {
    let mut results = 0;
    for (store, _, mut state, case) in states {
        apply_quads(&store, &case.additions, &case.deletions);
        results += select_results(state.results().unwrap()).count();
    }
    results
}

fn run_incremental_bsbm_workload<'a, D: IncrementalQueryableDataset<'a>>(
    states: Vec<(
        Store,
        SparqlEvaluator,
        IncrementalQueryResultsState<'a, D>,
        IncrementalBsbmCase,
    )>,
) -> usize {
    let mut results = 0;
    for (_, _, mut state, _) in states {
        results += select_results(state.results().unwrap()).count();
    }
    results
}

fn initialize_recompute_workload_stores_without_updates(
    data: &[u8],
    workload: &IncrementalBsbmWorkload,
) -> Vec<(Store, IncrementalBsbmCase)> {
    workload
        .cases
        .iter()
        .map(|case| {
            let store = initialize_store(data);
            apply_quads(&store, &[], &case.base_removals);
            (store, case.clone())
        })
        .collect()
}

fn initialize_recompute_workload_stores_with_updates(
    data: &[u8],
    workload: &IncrementalBsbmWorkload,
) -> Vec<(Store, IncrementalBsbmCase)> {
    let states = initialize_recompute_workload_stores_without_updates(data, workload);
    states
        .into_iter()
        .map(|(store, case)| {
            apply_quads(&store, &case.additions, &case.deletions);
            (store, case)
        })
        .collect()
}

fn run_recompute_bsbm_workload_with_updates(states: Vec<(Store, IncrementalBsbmCase)>) -> usize {
    let mut results = 0;
    for (store, case) in states {
        apply_quads(&store, &case.additions, &case.deletions);
        results += count_query_results(&store, &case.query);
    }
    results
}

fn run_recompute_bsbm_workload(states: Vec<(Store, IncrementalBsbmCase)>) -> usize {
    let mut results = 0;
    for (store, case) in states {
        results += count_query_results(&store, &case.query);
    }
    results
}

fn apply_quads(store: &Store, additions: &[Quad], deletions: &[Quad]) {
    let mut transaction = store.start_transaction().unwrap();
    for quad in deletions {
        transaction.remove(quad);
    }
    for quad in additions {
        transaction.insert(quad.clone());
    }
    transaction.commit().unwrap();
}

fn count_query_results(store: &Store, query: &Query) -> usize {
    drain_query_results(
        SparqlEvaluator::new()
            .without_optimizations()
            .for_query(query.clone())
            .on_store(store)
            .execute()
            .unwrap(),
    )
}

fn named_node(iri: &str) -> NamedNode {
    NamedNode::new(iri.to_owned()).unwrap()
}

const EXPLORE_QUERY_1: &str = "
PREFIX bsbm: <http://www4.wiwiss.fu-berlin.de/bizer/bsbm/v01/vocabulary/>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

SELECT DISTINCT ?product ?label
WHERE {
    ?product rdfs:label ?label .
    ?product a %ProductType% .
    ?product bsbm:productFeature %ProductFeature1% .
    ?product bsbm:productFeature %ProductFeature2% .
    ?product bsbm:productPropertyNumeric1 ?value1 .
    FILTER (?value1 > %x%)
}
ORDER BY ?label
LIMIT 10
";

const EXPLORE_QUERY_2: &str = "
PREFIX bsbm: <http://www4.wiwiss.fu-berlin.de/bizer/bsbm/v01/vocabulary/>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
PREFIX dc: <http://purl.org/dc/elements/1.1/>

SELECT ?label ?comment ?producer ?productFeature ?propertyTextual1 ?propertyTextual2 ?propertyTextual3
 ?propertyNumeric1 ?propertyNumeric2 ?propertyTextual4 ?propertyTextual5 ?propertyNumeric4
WHERE {
    %ProductXYZ% rdfs:label ?label .
    %ProductXYZ% rdfs:comment ?comment .
    %ProductXYZ% bsbm:producer ?p .
    ?p rdfs:label ?producer .
    %ProductXYZ% dc:publisher ?p .
    %ProductXYZ% bsbm:productFeature ?f .
    ?f rdfs:label ?productFeature .
    %ProductXYZ% bsbm:productPropertyTextual1 ?propertyTextual1 .
    %ProductXYZ% bsbm:productPropertyTextual2 ?propertyTextual2 .
    %ProductXYZ% bsbm:productPropertyTextual3 ?propertyTextual3 .
    %ProductXYZ% bsbm:productPropertyNumeric1 ?propertyNumeric1 .
    %ProductXYZ% bsbm:productPropertyNumeric2 ?propertyNumeric2 .
    OPTIONAL { %ProductXYZ% bsbm:productPropertyTextual4 ?propertyTextual4 }
    OPTIONAL { %ProductXYZ% bsbm:productPropertyTextual5 ?propertyTextual5 }
    OPTIONAL { %ProductXYZ% bsbm:productPropertyNumeric4 ?propertyNumeric4 }
}
";

const EXPLORE_QUERY_3_MINUS: &str = "
PREFIX bsbm: <http://www4.wiwiss.fu-berlin.de/bizer/bsbm/v01/vocabulary/>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>

SELECT ?product ?label
WHERE {
    ?product rdfs:label ?label .
    ?product a %ProductType% .
    ?product bsbm:productFeature %ProductFeature1% .
    ?product bsbm:productPropertyNumeric1 ?p1 .
    FILTER (?p1 > %x%)
    ?product bsbm:productPropertyNumeric3 ?p3 .
    FILTER (?p3 < %y%)
    MINUS {
        ?product bsbm:productFeature %ProductFeature2% .
    }
}
ORDER BY ?label
LIMIT 10
";

const EXPLORE_QUERY_4: &str = "
PREFIX bsbm: <http://www4.wiwiss.fu-berlin.de/bizer/bsbm/v01/vocabulary/>
PREFIX rdfs: <http://www.w3.org/2000/01/rdf-schema#>
PREFIX rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#>

SELECT DISTINCT ?product ?label ?propertyTextual
WHERE {
    {
       ?product rdfs:label ?label .
       ?product rdf:type %ProductType% .
       ?product bsbm:productFeature %ProductFeature1% .
       ?product bsbm:productFeature %ProductFeature2% .
       ?product bsbm:productPropertyTextual1 ?propertyTextual .
       ?product bsbm:productPropertyNumeric1 ?p1 .
       FILTER (?p1 > %x%)
    } UNION {
       ?product rdfs:label ?label .
       ?product rdf:type %ProductType% .
       ?product bsbm:productFeature %ProductFeature1% .
       ?product bsbm:productFeature %ProductFeature3% .
       ?product bsbm:productPropertyTextual1 ?propertyTextual .
       ?product bsbm:productPropertyNumeric2 ?p2 .
       FILTER (?p2 > %y%)
    }
}
ORDER BY ?label
OFFSET 5
LIMIT 10
";

fn initialize_store(data: &[u8]) -> Store {
    let store = Store::new().unwrap();
    do_bulk_load(&store, data);
    store
}

struct IncrementalBsbmStats {
    cases: Vec<IncrementalBsbmCaseStats>,
    relevant_cases: usize,
    irrelevant_cases: usize,
    additions: usize,
    deletions: usize,
    initial_results: usize,
    expected_results: usize,
    total_changes: usize,
}

struct IncrementalBsbmCaseStats {
    query_id: usize,
    scenario: IncrementalBsbmScenario,
    additions: usize,
    deletions: usize,
    initial_results: usize,
    expected_results: usize,
    incremental_changes: usize,
}

fn collect_incremental_bsbm_stats(
    data: &[u8],
    workload: &IncrementalBsbmWorkload,
) -> IncrementalBsbmStats {
    let store = initialize_store(data);
    let evaluator = SparqlEvaluator::new().without_optimizations();
    let relevant_cases = workload
        .cases
        .iter()
        .filter(|case| {
            matches!(
                case.scenario,
                IncrementalBsbmScenario::RelevantDeletion
                    | IncrementalBsbmScenario::RelevantInsertion
            )
        })
        .count();
    let irrelevant_cases = workload.cases.len() - relevant_cases;
    let additions = workload.cases.iter().map(|case| case.additions.len()).sum();
    let deletions = workload.cases.iter().map(|case| case.deletions.len()).sum();
    let cases = workload
        .cases
        .iter()
        .map(|case| {
            apply_quads(&store, &[], &case.base_removals);
            let mut state = evaluator
                .clone()
                .for_query(case.query.clone())
                .on_store(&store)
                .execute_incremental_deltas()
                .unwrap();
            let initial_results = drain_delta_results(state.deltas().unwrap());
            apply_quads(&store, &case.additions, &case.deletions);
            let incremental_changes = drain_delta_results(state.deltas().unwrap());
            let expected_results = count_query_results(&store, &case.query);
            apply_quads(&store, &case.deletions, &case.additions);
            apply_quads(&store, &case.base_removals, &[]);
            IncrementalBsbmCaseStats {
                query_id: case.query_id,
                scenario: case.scenario,
                additions: case.additions.len(),
                deletions: case.deletions.len(),
                initial_results,
                expected_results,
                incremental_changes,
            }
        })
        .collect::<Vec<_>>();
    let initial_results = cases.iter().map(|case| case.initial_results).sum();
    let expected_results = cases.iter().map(|case| case.expected_results).sum();
    let total_changes = cases.iter().map(|case| case.incremental_changes).sum();
    IncrementalBsbmStats {
        cases,
        relevant_cases,
        irrelevant_cases,
        additions,
        deletions,
        initial_results,
        expected_results,
        total_changes,
    }
}

#[expect(clippy::print_stderr, reason = "Benchmark workload diagnostics")]
fn print_incremental_bsbm_stats(
    data_size: usize,
    workload: &IncrementalBsbmWorkload,
    stats: &IncrementalBsbmStats,
) {
    eprintln!(
        "BSBM incremental workload ({data_size}): {} cases from Explore queries 1-4",
        workload.cases.len()
    );
    eprintln!(
        "  scenarios: {} relevant, {} irrelevant; delta triples: {} additions, {} deletions",
        stats.relevant_cases, stats.irrelevant_cases, stats.additions, stats.deletions
    );
    eprintln!(
        "  result rows: {} initial, {} after update; incremental result changes: {}",
        stats.initial_results, stats.expected_results, stats.total_changes
    );
    for case in &stats.cases {
        eprintln!(
            "  q{} {:>19}: initial = {}, expected = {}, row diff = {}, incremental changes = {}, input delta = +{} -{}",
            case.query_id,
            case.scenario.name(),
            case.initial_results,
            case.expected_results,
            case.initial_results.abs_diff(case.expected_results),
            case.incremental_changes,
            case.additions,
            case.deletions
        );
    }
}

fn parse_incremental_bsbm_select_query(query: &str) -> Option<Query> {
    let mut query = Query::from_str(query).unwrap();
    let Query::Select(select) = &mut query else {
        return None;
    };
    // Temporary benchmark setup: keep the BSBM SELECT bodies but remove
    // solution modifiers that the incremental evaluator does not support yet.
    select.expression =
        remove_incrementally_unsupported_solution_modifiers(select.expression.clone());
    Some(query)
}

fn remove_incrementally_unsupported_solution_modifiers(
    pattern: QueryExpression,
) -> QueryExpression {
    match pattern {
        QueryExpression::OrderBy { inner, .. }
        | QueryExpression::Distinct { inner }
        | QueryExpression::Reduced { inner }
        | QueryExpression::Slice { inner, .. } => {
            remove_incrementally_unsupported_solution_modifiers(*inner)
        }
        QueryExpression::Join { left, right } => QueryExpression::Join {
            left: Box::new(remove_incrementally_unsupported_solution_modifiers(*left)),
            right: Box::new(remove_incrementally_unsupported_solution_modifiers(*right)),
        },
        QueryExpression::LeftJoin {
            left,
            right,
            expression,
        } => QueryExpression::LeftJoin {
            left: Box::new(remove_incrementally_unsupported_solution_modifiers(*left)),
            right: Box::new(remove_incrementally_unsupported_solution_modifiers(*right)),
            expression,
        },
        QueryExpression::Filter { expr, inner } => QueryExpression::Filter {
            expr,
            inner: Box::new(remove_incrementally_unsupported_solution_modifiers(*inner)),
        },
        QueryExpression::Union { left, right } => QueryExpression::Union {
            left: Box::new(remove_incrementally_unsupported_solution_modifiers(*left)),
            right: Box::new(remove_incrementally_unsupported_solution_modifiers(*right)),
        },
        QueryExpression::Graph { name, inner } => QueryExpression::Graph {
            name,
            inner: Box::new(remove_incrementally_unsupported_solution_modifiers(*inner)),
        },
        QueryExpression::Extend {
            inner,
            variable,
            expression,
        } => QueryExpression::Extend {
            inner: Box::new(remove_incrementally_unsupported_solution_modifiers(*inner)),
            variable,
            expression,
        },
        QueryExpression::Minus { left, right } => QueryExpression::Minus {
            left: Box::new(remove_incrementally_unsupported_solution_modifiers(*left)),
            right: Box::new(remove_incrementally_unsupported_solution_modifiers(*right)),
        },
        QueryExpression::Project { inner, variables } => QueryExpression::Project {
            inner: Box::new(remove_incrementally_unsupported_solution_modifiers(*inner)),
            variables,
        },
        QueryExpression::Group {
            inner,
            variables,
            aggregates,
        } => QueryExpression::Group {
            inner: Box::new(remove_incrementally_unsupported_solution_modifiers(*inner)),
            variables,
            aggregates,
        },
        QueryExpression::Service {
            name,
            inner,
            silent,
        } => QueryExpression::Service {
            name,
            inner: Box::new(remove_incrementally_unsupported_solution_modifiers(*inner)),
            silent,
        },
        // Keep leaf nodes and optional operators, such as LATERAL, unchanged.
        _ => pattern,
    }
}

fn drain_query_results(results: QueryResults<'_>) -> usize {
    match results {
        QueryResults::Boolean(_) => 1,
        QueryResults::Solutions(solutions) => solutions
            .inspect(|s| {
                s.as_ref().unwrap();
            })
            .count(),
        QueryResults::Graph(graph) => graph
            .inspect(|t| {
                t.as_ref().unwrap();
            })
            .count(),
    }
}

fn drain_delta_results(results: QueryResultsDelta<'_>) -> usize {
    match results {
        QueryResultsDelta::Solutions(solutions) => solutions
            .inspect(|s| {
                s.as_ref().unwrap();
            })
            .count(),
        QueryResultsDelta::Boolean(values) => values
            .inspect(|v| {
                v.as_ref().unwrap();
            })
            .count(),
        QueryResultsDelta::Graph(graph) => graph
            .inspect(|t| {
                t.as_ref().unwrap();
            })
            .count(),
    }
}

#[derive(Clone)]
struct IncrementalWatdivConfig {
    static_scale: u32,
    stream_scale: u32,
    seed: u64,
    initial_stream_fractions: Vec<f64>,
    query_count: usize,
    artifact_dir: PathBuf,
    archive_name: &'static str,
    static_data: PathBuf,
    stream_events: PathBuf,
    queries: PathBuf,
}

impl IncrementalWatdivConfig {
    fn published_workload() -> Self {
        let (static_scale, stream_scale, query_count, archive_name) =
            match std::env::var("WATDIV_INCREMENTAL_SCALE").as_deref() {
                Ok("10") | Err(_) => (
                    10,
                    10,
                    50,
                    "stream-watdiv-static-10-stream-10-seed-42-q50-m6-c1.zip",
                ),
                Ok("1") => (
                    1,
                    1,
                    10,
                    "stream-watdiv-static-1-stream-1-seed-42-q10-m6-c1.zip",
                ),
                Ok(value) => panic!("unknown WATDIV_INCREMENTAL_SCALE {value}; use 1 or 10"),
            };
        let seed = 42;
        let initial_stream_fractions =
            env_fractions("WATDIV_INCREMENTAL_INITIAL_STREAM_FRACTIONS", &[0.90]);
        let artifact_dir = repo_root().join(format!(
            "target/watdiv-incremental/static-{static_scale}-stream-{stream_scale}-seed-42-q{query_count}-m6-c1"
        ));
        let static_data = artifact_dir.join("static.nt");
        let stream_events = artifact_dir.join("stream-events.jsonl");
        let queries = artifact_dir.join("queries.sparql");
        Self {
            static_scale,
            stream_scale,
            seed,
            initial_stream_fractions,
            query_count,
            artifact_dir,
            archive_name,
            static_data,
            stream_events,
            queries,
        }
    }
}

fn ensure_published_watdiv_workload(config: &IncrementalWatdivConfig) {
    if config.static_data.exists() && config.stream_events.exists() && config.queries.exists() {
        return;
    }
    let download_dir = repo_root().join("target/watdiv-incremental/downloads");
    fs::create_dir_all(&download_dir).unwrap();
    let archive_path = download_dir.join(config.archive_name);
    if !archive_path.exists() {
        let url = format!(
            "https://zenodo.org/api/records/23016810/files/{}/content",
            config.archive_name
        );
        let client = oxhttp::Client::new()
            .with_redirection_limit(5)
            .with_user_agent(concat!("Oxigraph/", env!("CARGO_PKG_VERSION")))
            .unwrap();
        let request = Request::builder().uri(&url).body(()).unwrap();
        let response = client.request(request).unwrap();
        assert!(
            response.status().is_success(),
            "{url} returned {}",
            response.status()
        );
        let mut temp = NamedTempFile::new_in(&download_dir).unwrap();
        std::io::copy(&mut response.into_body(), &mut temp).unwrap();
        temp.persist(&archive_path).unwrap();
    }

    let mut archive = ZipArchive::new(File::open(&archive_path).unwrap()).unwrap();
    fs::create_dir_all(&config.artifact_dir).unwrap();
    for (name, target) in [
        ("static.nt", &config.static_data),
        ("stream-events.jsonl", &config.stream_events),
        ("queries.sparql", &config.queries),
    ] {
        if target.exists() {
            continue;
        }
        let mut entry = archive.by_name(name).unwrap();
        let mut temp = NamedTempFile::new_in(&config.artifact_dir).unwrap();
        std::io::copy(&mut entry, &mut temp).unwrap();
        temp.persist(target).unwrap();
    }
}

#[derive(Clone)]
struct IncrementalWatdivWorkload {
    static_data: Vec<u8>,
    static_triple_count: usize,
    events: Vec<WatdivEvent>,
    queries: Vec<Query>,
    query_texts: Vec<String>,
    cases: Vec<IncrementalWatdivCase>,
}

#[derive(Clone)]
struct WatdivEvent {
    triple_count: usize,
    quads: Vec<Quad>,
}

#[derive(Clone)]
struct IncrementalWatdivCase {
    query_id: usize,
    initial_stream_fraction: f64,
    prefix_event_count: usize,
    update_type: WatdivUpdateType,
    update_event_count: usize,
    update_triple_count: usize,
}

#[derive(Clone, Copy)]
enum WatdivUpdateType {
    Insertion,
    Deletion,
}

impl WatdivUpdateType {
    const ALL: [Self; 2] = [Self::Insertion, Self::Deletion];

    fn id(self) -> &'static str {
        match self {
            Self::Insertion => "insertion",
            Self::Deletion => "deletion",
        }
    }
}

#[derive(Default)]
struct WatdivDeltaCounts {
    additions: usize,
    deletions: usize,
}

struct WatdivMeasurement {
    query_id: usize,
    query_text: String,
    initial_stream_fraction: f64,
    update_type: WatdivUpdateType,
    initial_event_count: usize,
    update_event_count: usize,
    initial_triple_count: usize,
    update_triple_count: usize,
    final_triple_count: usize,
    initial_result_count: usize,
    result_delta_addition_count: usize,
    result_delta_deletion_count: usize,
    final_result_count: usize,
    incremental_latency_ns: u128,
    reevaluation_latency_ns: u128,
}

fn prepare_incremental_watdiv_workload(
    config: &IncrementalWatdivConfig,
) -> IncrementalWatdivWorkload {
    let static_data = fs::read(&config.static_data).unwrap();
    let events = read_watdiv_events(&config.stream_events);
    let query_texts = read_watdiv_queries(&config.queries);

    let static_triple_count = parse_ntriples_quads(&static_data).len();
    let mut queries = Vec::new();
    let mut normalized_query_texts = Vec::new();
    for query_text in query_texts {
        if let Some(query) = parse_incremental_watdiv_select_query(&query_text) {
            normalized_query_texts.push(query_text);
            queries.push(query);
        }
        if queries.len() == config.query_count {
            break;
        }
    }
    assert!(
        !queries.is_empty(),
        "could not load any parseable WatDiv SELECT query"
    );

    let mut cases = Vec::new();
    for &initial_stream_fraction in &config.initial_stream_fractions {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "Validated fractions in [0, 1] select a bounded event prefix"
        )]
        #[cfg_attr(target_pointer_width = "64", expect(clippy::cast_precision_loss))]
        let prefix_event_count =
            ((initial_stream_fraction * events.len() as f64).floor() as usize).min(events.len());
        let update_event_count = events.len() - prefix_event_count;
        let update_triple_count = events[prefix_event_count..]
            .iter()
            .map(|event| event.quads.len())
            .sum::<usize>();
        for update_type in WatdivUpdateType::ALL {
            for query_id in 1..=queries.len() {
                cases.push(IncrementalWatdivCase {
                    query_id,
                    initial_stream_fraction,
                    prefix_event_count,
                    update_type,
                    update_event_count,
                    update_triple_count,
                });
            }
        }
    }

    IncrementalWatdivWorkload {
        static_data,
        static_triple_count,
        events,
        queries,
        query_texts: normalized_query_texts,
        cases,
    }
}

fn read_stream_watdiv_timestamp_events(stream_file: &Path) -> Vec<(String, Vec<String>)> {
    let mut events: Vec<(String, Vec<String>)> = Vec::new();
    for line in BufReader::new(File::open(stream_file).unwrap()).lines() {
        let line = line.unwrap();
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let (timestamp, triple) = parse_stream_watdiv_line(line);
        if let Some((last_timestamp, triples)) = events.last_mut()
            && last_timestamp == &timestamp
        {
            triples.push(triple);
            continue;
        }
        events.push((timestamp, vec![triple]));
    }
    assert!(!events.is_empty(), "Stream WatDiv stream file is empty");
    events
}

fn parse_stream_watdiv_line(line: &str) -> (String, String) {
    let mut parts = line.split('\t').collect::<Vec<_>>();
    assert!(
        parts.len() >= 4,
        "Stream WatDiv rows should be tab-separated subject predicate object timestamp: {line}"
    );
    let timestamp = parts.pop().unwrap().trim().to_owned();
    let subject = stream_watdiv_resource(parts[0].trim());
    let predicate = stream_watdiv_resource(parts[1].trim());
    let object = if parts[2].trim_start().starts_with('"') {
        parts[2..].join("\t")
    } else {
        stream_watdiv_resource(parts[2].trim())
    };
    (timestamp, format!("{subject} {predicate} {object} ."))
}

fn stream_watdiv_resource(value: &str) -> String {
    if value.starts_with('<') || value.starts_with("_:") || value.starts_with('"') {
        value.to_owned()
    } else {
        format!("<{value}>")
    }
}

fn read_watdiv_events(path: &Path) -> Vec<WatdivEvent> {
    let first_line = BufReader::new(File::open(path).unwrap())
        .lines()
        .map(|line| line.unwrap())
        .find(|line| !line.trim().is_empty());
    if first_line.is_some_and(|line| is_stream_watdiv_line(line.trim())) {
        return read_stream_watdiv_timestamp_events(path)
            .into_iter()
            .enumerate()
            .map(|(event_id, (timestamp, triples))| {
                build_watdiv_event(event_id, Some(&timestamp), &triples)
            })
            .collect();
    }

    let file = File::open(path).unwrap();
    let reader = BufReader::new(file);
    let mut events = Vec::new();
    let mut current = Vec::new();
    for line in reader.lines() {
        let line = line.unwrap();
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('{') {
            flush_watdiv_event(&mut events, &mut current, None);
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            let event_id = value["event_id"]
                .as_u64()
                .map_or(events.len(), |id| usize::try_from(id).unwrap());
            let timestamp = value["timestamp"].as_str();
            let triples = value["triples"]
                .as_array()
                .unwrap_or_else(|| panic!("WatDiv event JSONL row should have a triples array"))
                .iter()
                .map(|value| value.as_str().unwrap().to_owned())
                .collect::<Vec<_>>();
            events.push(build_watdiv_event(event_id, timestamp, &triples));
        } else if line.starts_with("# event") {
            flush_watdiv_event(&mut events, &mut current, None);
        } else if let Some(triple) = line.strip_prefix('+') {
            current.push(triple.trim().to_owned());
        } else {
            flush_watdiv_event(&mut events, &mut current, None);
            let event_id = events.len();
            events.push(build_watdiv_event(event_id, None, &[line.to_owned()]));
        }
    }
    flush_watdiv_event(&mut events, &mut current, None);
    assert!(!events.is_empty(), "WatDiv stream event file is empty");
    events
}

fn is_stream_watdiv_line(line: &str) -> bool {
    let parts = line.split('\t').collect::<Vec<_>>();
    parts.len() >= 4
        && parts
            .last()
            .is_some_and(|timestamp| timestamp.parse::<u64>().is_ok())
}

fn flush_watdiv_event(
    events: &mut Vec<WatdivEvent>,
    current: &mut Vec<String>,
    timestamp: Option<&str>,
) {
    if !current.is_empty() {
        let event_id = events.len();
        events.push(build_watdiv_event(
            event_id,
            timestamp,
            &std::mem::take(current),
        ));
    }
}

fn build_watdiv_event(event_id: usize, timestamp: Option<&str>, triples: &[String]) -> WatdivEvent {
    let triple_count = triples.len();
    let mut data = triples.join("\n");
    data.push('\n');
    let quads = parse_ntriples_quads(data.as_bytes())
        .into_iter()
        .enumerate()
        .map(|(triple_id, quad)| {
            Quad::new(
                quad.subject,
                quad.predicate,
                quad.object,
                GraphName::NamedNode(watdiv_stream_graph_name(event_id, triple_id, timestamp)),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        triple_count,
        quads.len(),
        "WatDiv event has unparsable triples"
    );
    WatdivEvent {
        triple_count,
        quads,
    }
}

fn watdiv_stream_graph_name(
    event_id: usize,
    triple_id: usize,
    timestamp: Option<&str>,
) -> NamedNode {
    let timestamp = timestamp.map_or_else(|| "none".to_owned(), sanitize_watdiv_graph_component);
    NamedNode::new(format!(
        "urn:watdiv:stream:event:{event_id}:triple:{triple_id}:time:{timestamp}"
    ))
    .unwrap()
}

fn sanitize_watdiv_graph_component(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn read_watdiv_queries(path: &Path) -> Vec<String> {
    if path.is_dir() {
        let mut files = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| matches!(extension, "rq" | "sparql"))
            })
            .collect::<Vec<_>>();
        files.sort();
        return files
            .into_iter()
            .flat_map(|path| normalize_watdiv_queries(&fs::read_to_string(path).unwrap()))
            .collect();
    }
    normalize_watdiv_queries(&fs::read_to_string(path).unwrap())
}

fn normalize_watdiv_queries(raw: &str) -> Vec<String> {
    let mut queries = Vec::new();
    let mut query = String::new();
    let mut brace_depth = 0_i64;
    for line in raw.lines() {
        let line = normalize_watdiv_query_line(line);
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
        if brace_depth <= 0 && line.ends_with('}') {
            queries.push(query.trim().to_owned());
            query.clear();
            brace_depth = 0;
        }
    }
    if !query.trim().is_empty() {
        queries.push(query.trim().to_owned());
    }
    queries
}

fn normalize_watdiv_query_line(line: &str) -> String {
    let mut line = line.replace('\t', " ");
    while line.contains("  ") {
        line = line.replace("  ", " ");
    }
    let trimmed = line.trim();
    if trimmed.starts_with("STREAM ") || trimmed.starts_with("GRAPH ") {
        "{".to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn parse_incremental_watdiv_select_query(query: &str) -> Option<Query> {
    let mut query = Query::from_str(query).ok()?;
    let Query::Select(select) = &mut query else {
        return None;
    };
    select.expression =
        remove_incrementally_unsupported_solution_modifiers(select.expression.clone());
    Some(query)
}

fn prepare_watdiv_query(query: &Query) -> PreparedSparqlQuery {
    let mut prepared = SparqlEvaluator::new().for_query(query.clone());
    prepared.dataset_mut().set_default_graph_as_union();
    prepared
}

fn validate_incremental_watdiv_workload(
    workload: &IncrementalWatdivWorkload,
) -> Vec<WatdivMeasurement> {
    workload
        .cases
        .iter()
        .map(|case| validate_incremental_watdiv_case(workload, case))
        .collect()
}

fn validate_incremental_watdiv_case(
    workload: &IncrementalWatdivWorkload,
    case: &IncrementalWatdivCase,
) -> WatdivMeasurement {
    let (store, mut state, initial_result_count) =
        initialize_incremental_watdiv_case_with_updates(workload, case);
    let start = Instant::now();
    let delta_counts =
        run_incremental_watdiv_case_without_updates(&store, &mut state, workload, case);
    let incremental_latency_ns = start.elapsed().as_nanos();
    let incremental_results = collect_incremental_watdiv_final_result_multiset(workload, case);

    let final_store = initialize_final_watdiv_store(workload, case);
    let start = Instant::now();
    let reevaluated_results = collect_watdiv_result_multiset(
        prepare_watdiv_query(&workload.queries[case.query_id - 1])
            .on_store(&final_store)
            .execute()
            .unwrap(),
    );
    let reevaluation_latency_ns = start.elapsed().as_nanos();
    assert!(
        incremental_results == reevaluated_results,
        "WatDiv incremental mismatch for query {}, fraction {}, update {}: incremental results {}, reevaluated results {}, update triples {}\n{}",
        case.query_id,
        case.initial_stream_fraction,
        case.update_type.id(),
        count_multiset(&incremental_results),
        count_multiset(&reevaluated_results),
        case.update_triple_count,
        workload.query_texts[case.query_id - 1]
    );
    let final_result_count = count_multiset(&reevaluated_results);
    assert_eq!(
        initial_result_count + delta_counts.additions,
        final_result_count + delta_counts.deletions,
        "WatDiv delta counts do not reconcile for query {}, fraction {}, update {}",
        case.query_id,
        case.initial_stream_fraction,
        case.update_type.id()
    );

    WatdivMeasurement {
        query_id: case.query_id,
        query_text: workload.query_texts[case.query_id - 1].clone(),
        initial_stream_fraction: case.initial_stream_fraction,
        update_type: case.update_type,
        initial_event_count: initial_watdiv_event_count(workload, case),
        update_event_count: case.update_event_count,
        initial_triple_count: initial_watdiv_triple_count(workload, case),
        update_triple_count: case.update_triple_count,
        final_triple_count: final_watdiv_triple_count(workload, case),
        initial_result_count,
        result_delta_addition_count: delta_counts.additions,
        result_delta_deletion_count: delta_counts.deletions,
        final_result_count,
        incremental_latency_ns,
        reevaluation_latency_ns,
    }
}

fn collect_incremental_watdiv_final_result_multiset(
    workload: &IncrementalWatdivWorkload,
    case: &IncrementalWatdivCase,
) -> HashMap<Vec<String>, usize> {
    let store = initialize_watdiv_initial_store(workload, case);
    let mut state: IncrementalQueryResultsState<'static, _> =
        prepare_watdiv_query(&workload.queries[case.query_id - 1])
            .on_store(&store)
            .execute_incremental_results()
            .unwrap();
    select_results(state.results().unwrap()).count();
    apply_watdiv_events(
        &store,
        watdiv_suffix_events(workload, case),
        case.update_type,
    );

    let mut multiset = HashMap::new();
    for solution in select_results(state.results().unwrap()) {
        let key = solution
            .values()
            .iter()
            .map(|value| {
                value
                    .as_ref()
                    .map_or_else(|| "0".to_owned(), |term| format!("1{term}"))
            })
            .collect::<Vec<_>>();
        *multiset.entry(key).or_insert(0) += 1;
    }
    multiset
}

fn initialize_incremental_watdiv_case(
    workload: &IncrementalWatdivWorkload,
    case: &IncrementalWatdivCase,
) -> (
    Store,
    IncrementalQueryDeltasState<'static, impl IncrementalQueryableDataset<'static>>,
    usize,
) {
    let store = initialize_watdiv_initial_store(workload, case);
    let mut state = prepare_watdiv_query(&workload.queries[case.query_id - 1])
        .on_store(&store)
        .execute_incremental_deltas()
        .unwrap();
    let initial_result_count = collect_watdiv_delta_counts(state.deltas().unwrap()).additions;
    (store, state, initial_result_count)
}

fn initialize_incremental_watdiv_case_with_updates(
    workload: &IncrementalWatdivWorkload,
    case: &IncrementalWatdivCase,
) -> (
    Store,
    IncrementalQueryDeltasState<'static, impl IncrementalQueryableDataset<'static>>,
    usize,
) {
    let (store, state, initial_result_count) = initialize_incremental_watdiv_case(workload, case);
    apply_watdiv_events(
        &store,
        watdiv_suffix_events(workload, case),
        case.update_type,
    );
    (store, state, initial_result_count)
}

fn initialize_watdiv_initial_store(
    workload: &IncrementalWatdivWorkload,
    case: &IncrementalWatdivCase,
) -> Store {
    load_watdiv_store(workload, initial_watdiv_event_count(workload, case))
}

fn initialize_final_watdiv_store(
    workload: &IncrementalWatdivWorkload,
    case: &IncrementalWatdivCase,
) -> Store {
    let event_count = match case.update_type {
        WatdivUpdateType::Insertion => workload.events.len(),
        WatdivUpdateType::Deletion => case.prefix_event_count,
    };
    load_watdiv_store(workload, event_count)
}

fn load_watdiv_store(workload: &IncrementalWatdivWorkload, event_count: usize) -> Store {
    const BATCH_QUADS: usize = 50_000;
    let store = Store::new().unwrap();
    let mut loader = store
        .bulk_loader()
        .with_num_threads(1)
        .with_max_memory_size_in_megabytes(256);
    loader
        .load_from_slice(
            RdfParser::from_format(RdfFormat::NTriples).lenient(),
            &workload.static_data,
        )
        .unwrap();
    let mut batch = Vec::with_capacity(BATCH_QUADS);
    for event in workload.events.iter().take(event_count) {
        batch.extend(event.quads.iter().cloned());
        if batch.len() >= BATCH_QUADS {
            loader.load_quads(std::mem::take(&mut batch)).unwrap();
        }
    }
    if !batch.is_empty() {
        loader.load_quads(batch).unwrap();
    }
    loader.commit().unwrap();
    store
}

fn run_incremental_watdiv_case_without_updates<'a, D: IncrementalQueryableDataset<'a>>(
    _store: &Store,
    state: &mut IncrementalQueryDeltasState<'a, D>,
    _workload: &IncrementalWatdivWorkload,
    _case: &IncrementalWatdivCase,
) -> WatdivDeltaCounts {
    collect_watdiv_delta_counts(state.deltas().unwrap())
}

fn apply_watdiv_events(store: &Store, events: &[WatdivEvent], update_type: WatdivUpdateType) {
    const BATCH_QUADS: usize = 50_000;
    let mut transaction = store.start_transaction().unwrap();
    let mut batch_size = 0;
    for event in events {
        for quad in &event.quads {
            match update_type {
                WatdivUpdateType::Insertion => transaction.insert(quad.clone()),
                WatdivUpdateType::Deletion => transaction.remove(quad),
            }
            batch_size += 1;
        }
        if batch_size >= BATCH_QUADS {
            transaction.commit().unwrap();
            transaction = store.start_transaction().unwrap();
            batch_size = 0;
        }
    }
    if batch_size > 0 {
        transaction.commit().unwrap();
    }
}

fn watdiv_suffix_events<'a>(
    workload: &'a IncrementalWatdivWorkload,
    case: &IncrementalWatdivCase,
) -> &'a [WatdivEvent] {
    &workload.events[case.prefix_event_count..]
}

fn initial_watdiv_event_count(
    workload: &IncrementalWatdivWorkload,
    case: &IncrementalWatdivCase,
) -> usize {
    match case.update_type {
        WatdivUpdateType::Insertion => case.prefix_event_count,
        WatdivUpdateType::Deletion => workload.events.len(),
    }
}

fn initial_watdiv_triple_count(
    workload: &IncrementalWatdivWorkload,
    case: &IncrementalWatdivCase,
) -> usize {
    workload.static_triple_count
        + workload
            .events
            .iter()
            .take(initial_watdiv_event_count(workload, case))
            .map(|event| event.quads.len())
            .sum::<usize>()
}

fn final_watdiv_triple_count(
    workload: &IncrementalWatdivWorkload,
    case: &IncrementalWatdivCase,
) -> usize {
    match case.update_type {
        WatdivUpdateType::Insertion => {
            workload.static_triple_count
                + workload
                    .events
                    .iter()
                    .map(|event| event.quads.len())
                    .sum::<usize>()
        }
        WatdivUpdateType::Deletion => {
            workload.static_triple_count
                + workload
                    .events
                    .iter()
                    .take(case.prefix_event_count)
                    .map(|event| event.quads.len())
                    .sum::<usize>()
        }
    }
}

fn collect_watdiv_delta_counts(delta: QueryResultsDelta<'_>) -> WatdivDeltaCounts {
    let mut counts = WatdivDeltaCounts::default();
    match delta {
        QueryResultsDelta::Solutions(changes) => {
            for change in changes {
                match change.unwrap() {
                    Delta::Addition(_) => counts.additions += 1,
                    Delta::Deletion(_) => counts.deletions += 1,
                }
            }
        }
        QueryResultsDelta::Boolean(changes) => {
            counts.additions += changes
                .inspect(|value| {
                    value.as_ref().unwrap();
                })
                .count();
        }
        QueryResultsDelta::Graph(changes) => {
            for change in changes {
                match change.unwrap() {
                    Delta::Addition(_) => counts.additions += 1,
                    Delta::Deletion(_) => counts.deletions += 1,
                }
            }
        }
    }
    counts
}

fn collect_watdiv_result_multiset(results: QueryResults<'_>) -> HashMap<Vec<String>, usize> {
    let mut multiset = HashMap::new();
    match results {
        QueryResults::Solutions(solutions) => {
            for solution in solutions {
                let solution = solution.unwrap();
                let key = solution
                    .values()
                    .iter()
                    .map(|value| {
                        value
                            .as_ref()
                            .map_or_else(|| "0".to_owned(), |term| format!("1{term}"))
                    })
                    .collect::<Vec<_>>();
                *multiset.entry(key).or_insert(0) += 1;
            }
        }
        QueryResults::Boolean(value) => {
            multiset.insert(vec![value.to_string()], 1);
        }
        QueryResults::Graph(triples) => {
            for triple in triples {
                let triple = triple.unwrap();
                *multiset
                    .entry(vec![
                        triple.subject.to_string(),
                        triple.predicate.to_string(),
                        triple.object.to_string(),
                    ])
                    .or_insert(0) += 1;
            }
        }
    }
    multiset
}

fn count_watdiv_query_results(store: &Store, query: &Query) -> usize {
    drain_query_results(
        prepare_watdiv_query(query)
            .on_store(store)
            .execute()
            .unwrap(),
    )
}

fn count_multiset(multiset: &HashMap<Vec<String>, usize>) -> usize {
    multiset.values().sum()
}

fn write_incremental_watdiv_results(
    config: &IncrementalWatdivConfig,
    workload: &IncrementalWatdivWorkload,
    measurements: &[WatdivMeasurement],
) {
    fs::create_dir_all(&config.artifact_dir).unwrap();
    let mut file = File::create(config.artifact_dir.join("results.tsv")).unwrap();
    writeln!(
        file,
        "seed\tstatic_scale\tstream_scale\tinitial_stream_fraction\tquery_id\tquery_text\tupdate_type\tinitial_event_count\tupdate_event_count\tinitial_triple_count\tupdate_triple_count\tfinal_triple_count\tinitial_result_count\tresult_delta_addition_count\tresult_delta_deletion_count\tfinal_result_count\tincremental_latency_ns\treevaluation_latency_ns\tpeak_memory_bytes\tresult_change_type"
    )
    .unwrap();
    for measurement in measurements {
        let result_change_type = if measurement.result_delta_addition_count
            + measurement.result_delta_deletion_count
            == 0
        {
            "result-preserving"
        } else {
            "result-changing"
        };
        let fields = [
            config.seed.to_string(),
            config.static_scale.to_string(),
            config.stream_scale.to_string(),
            measurement.initial_stream_fraction.to_string(),
            measurement.query_id.to_string(),
            tsv_escape(&measurement.query_text),
            measurement.update_type.id().to_owned(),
            measurement.initial_event_count.to_string(),
            measurement.update_event_count.to_string(),
            measurement.initial_triple_count.to_string(),
            measurement.update_triple_count.to_string(),
            measurement.final_triple_count.to_string(),
            measurement.initial_result_count.to_string(),
            measurement.result_delta_addition_count.to_string(),
            measurement.result_delta_deletion_count.to_string(),
            measurement.final_result_count.to_string(),
            measurement.incremental_latency_ns.to_string(),
            measurement.reevaluation_latency_ns.to_string(),
            String::new(),
            result_change_type.to_owned(),
        ];
        writeln!(file, "{}", fields.join("\t")).unwrap();
    }
    assert_eq!(
        workload.cases.len(),
        measurements.len(),
        "Each workload case must have one measurement"
    );
}

#[expect(clippy::print_stderr, reason = "Benchmark workload diagnostics")]
fn print_incremental_watdiv_stats(
    config: &IncrementalWatdivConfig,
    workload: &IncrementalWatdivWorkload,
    measurements: &[WatdivMeasurement],
) {
    let stream_triples = workload
        .events
        .iter()
        .map(|event| event.quads.len())
        .sum::<usize>();
    let stream_event_triples = workload
        .events
        .iter()
        .map(|event| event.triple_count)
        .sum::<usize>();
    eprintln!(
        "WatDiv incremental workload: static scale {}, stream scale {}, seed {}, {} queries, {} cases",
        config.static_scale,
        config.stream_scale,
        config.seed,
        workload.queries.len(),
        workload.cases.len()
    );
    eprintln!("  artifacts: {}", config.artifact_dir.display());
    eprintln!(
        "  triples/events: {} static triples, {} stream events, {} stream triples ({} event text triples)",
        workload.static_triple_count,
        workload.events.len(),
        stream_triples,
        stream_event_triples
    );
    eprintln!(
        "  validated {} WatDiv insertion/deletion cases; machine-readable results: {}",
        measurements.len(),
        config.artifact_dir.join("results.tsv").display()
    );
}

fn parse_ntriples_quads(data: &[u8]) -> Vec<Quad> {
    RdfParser::from_format(RdfFormat::NTriples)
        .lenient()
        .for_slice(data)
        .map(|quad| quad.unwrap())
        .collect()
}

fn env_fractions(name: &str, default: &[f64]) -> Vec<f64> {
    std::env::var(name)
        .ok()
        .map(|value| {
            value
                .split([',', ' '])
                .filter(|value| !value.is_empty())
                .map(|value| value.parse::<f64>().unwrap())
                .inspect(|value| {
                    assert!(
                        (0.0..=1.0).contains(value),
                        "{name} fractions must be between 0 and 1"
                    )
                })
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty())
        .unwrap_or_else(|| default.to_vec())
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .to_owned()
}

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

fn fraction_id(value: f64) -> String {
    let mut value = format!("{value:.6}");
    while value.contains('.') && value.ends_with('0') {
        value.pop();
    }
    if value.ends_with('.') {
        value.pop();
    }
    value.replace('.', "p")
}

fn tsv_escape(value: &str) -> String {
    value.replace(['\t', '\n', '\r'], " ")
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

criterion_group!(
    incremental_store,
    incremental_store_bsbm,
    incremental_store_watdiv
);
criterion_main!(incremental_store);
