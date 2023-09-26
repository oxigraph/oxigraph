use crate::evaluator::TestEvaluator;
use crate::files::*;
use crate::manifest::*;
use crate::report::{dataset_diff, format_diff};
use crate::vocab::*;
use anyhow::{anyhow, bail, ensure, Error, Result};
use oxigraph::model::vocab::*;
use oxigraph::model::*;
use oxigraph::sparql::results::QueryResultsFormat;
use oxigraph::sparql::*;
use oxigraph::store::Store;
use sparopt::Optimizer;
use std::collections::HashMap;
use std::fmt::Write;
use std::io::{self, Cursor};
use std::ops::Deref;
use std::str::FromStr;
use std::sync::{Arc, Mutex, OnceLock};

pub fn register_sparql_tests(evaluator: &mut TestEvaluator) {
    evaluator.register(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#PositiveSyntaxTest",
        evaluate_positive_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#PositiveSyntaxTest11",
        evaluate_positive_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#NegativeSyntaxTest",
        evaluate_negative_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#NegativeSyntaxTest11",
        evaluate_negative_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#QueryEvaluationTest",
        evaluate_evaluation_test,
    );
    evaluator.register(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#PositiveUpdateSyntaxTest11",
        evaluate_positive_update_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#NegativeUpdateSyntaxTest11",
        evaluate_negative_update_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#UpdateEvaluationTest",
        evaluate_update_evaluation_test,
    );
    evaluator.register(
        "https://github.com/oxigraph/oxigraph/tests#PositiveJsonResultsSyntaxTest",
        |t| evaluate_positive_result_syntax_test(t, QueryResultsFormat::Json),
    );
    evaluator.register(
        "https://github.com/oxigraph/oxigraph/tests#NegativeJsonResultsSyntaxTest",
        |t| evaluate_negative_result_syntax_test(t, QueryResultsFormat::Json),
    );
    evaluator.register(
        "https://github.com/oxigraph/oxigraph/tests#PositiveXmlResultsSyntaxTest",
        |t| evaluate_positive_result_syntax_test(t, QueryResultsFormat::Xml),
    );
    evaluator.register(
        "https://github.com/oxigraph/oxigraph/tests#NegativeXmlResultsSyntaxTest",
        |t| evaluate_negative_result_syntax_test(t, QueryResultsFormat::Xml),
    );
    evaluator.register(
        "https://github.com/oxigraph/oxigraph/tests#NegativeTsvResultsSyntaxTest",
        |t| evaluate_negative_result_syntax_test(t, QueryResultsFormat::Tsv),
    );
    evaluator.register(
        "https://github.com/oxigraph/oxigraph/tests#QueryOptimizationTest",
        evaluate_query_optimization_test,
    );
}

fn evaluate_positive_syntax_test(test: &Test) -> Result<()> {
    let query_file = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found"))?;
    let query = Query::parse(&read_file_to_string(query_file)?, Some(query_file))
        .map_err(|e| anyhow!("Not able to parse with error: {e}"))?;
    Query::parse(&query.to_string(), None)
        .map_err(|e| anyhow!("Failure to deserialize \"{query}\" with error: {e}"))?;
    Ok(())
}

fn evaluate_negative_syntax_test(test: &Test) -> Result<()> {
    let query_file = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found"))?;
    ensure!(
        Query::parse(&read_file_to_string(query_file)?, Some(query_file)).is_err(),
        "Oxigraph parses even if it should not."
    );
    Ok(())
}

fn evaluate_positive_result_syntax_test(test: &Test, format: QueryResultsFormat) -> Result<()> {
    let action_file = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found"))?;
    let actual_results = StaticQueryResults::from_query_results(
        QueryResults::read(read_file(action_file)?, format)?,
        true,
    )?;
    if let Some(result_file) = test.result.as_deref() {
        let expected_results = StaticQueryResults::from_query_results(
            QueryResults::read(read_file(result_file)?, format)?,
            true,
        )?;
        ensure!(
            are_query_results_isomorphic(&expected_results, &actual_results),
            "Not isomorphic results:\n{}\n",
            results_diff(expected_results, actual_results),
        );
    }
    Ok(())
}

fn evaluate_negative_result_syntax_test(test: &Test, format: QueryResultsFormat) -> Result<()> {
    let action_file = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found"))?;
    ensure!(
        QueryResults::read(Cursor::new(read_file_to_string(action_file)?), format)
            .map_err(Error::from)
            .and_then(|r| { StaticQueryResults::from_query_results(r, true) })
            .is_err(),
        "Oxigraph parses even if it should not."
    );
    Ok(())
}

fn evaluate_evaluation_test(test: &Test) -> Result<()> {
    let store = get_store()?;
    if let Some(data) = &test.data {
        load_dataset_to_store(data, &store)?;
    }
    for (name, value) in &test.graph_data {
        load_graph_to_store(value, &store, name.clone())?;
    }
    let query_file = test
        .query
        .as_deref()
        .ok_or_else(|| anyhow!("No action found"))?;
    let options = QueryOptions::default()
        .with_service_handler(StaticServiceHandler::new(&test.service_data)?);
    let query = Query::parse(&read_file_to_string(query_file)?, Some(query_file))
        .map_err(|e| anyhow!("Failure to parse query with error: {e}"))?;

    // We check parsing roundtrip
    Query::parse(&query.to_string(), None)
        .map_err(|e| anyhow!("Failure to deserialize \"{query}\" with error: {e}"))?;

    // FROM and FROM NAMED support. We make sure the data is in the store
    if !query.dataset().is_default_dataset() {
        for graph_name in query.dataset().default_graph_graphs().unwrap_or(&[]) {
            let GraphName::NamedNode(graph_name) = graph_name else {
                bail!("Invalid FROM in query {query}");
            };
            load_graph_to_store(graph_name.as_str(), &store, graph_name.as_ref())?;
        }
        for graph_name in query.dataset().available_named_graphs().unwrap_or(&[]) {
            let NamedOrBlankNode::NamedNode(graph_name) = graph_name else {
                bail!("Invalid FROM NAMED in query {query}");
            };
            load_graph_to_store(graph_name.as_str(), &store, graph_name.as_ref())?;
        }
    }

    let expected_results = load_sparql_query_result(test.result.as_ref().unwrap())
        .map_err(|e| anyhow!("Error constructing expected graph: {e}"))?;
    let with_order = if let StaticQueryResults::Solutions { ordered, .. } = &expected_results {
        *ordered
    } else {
        false
    };

    for with_query_optimizer in [true, false] {
        let mut options = options.clone();
        if !with_query_optimizer {
            options = options.without_optimizations();
        }
        let actual_results = store
            .query_opt(query.clone(), options)
            .map_err(|e| anyhow!("Failure to execute query with error: {e}"))?;
        let actual_results = StaticQueryResults::from_query_results(actual_results, with_order)?;

        ensure!(
            are_query_results_isomorphic(&expected_results, &actual_results),
            "Not isomorphic results.\n{}\nParsed query:\n{}\nData:\n{}\n",
            results_diff(expected_results, actual_results),
            Query::parse(&read_file_to_string(query_file)?, Some(query_file)).unwrap(),
            &*store
        );
    }
    Ok(())
}

fn evaluate_positive_update_syntax_test(test: &Test) -> Result<()> {
    let update_file = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found"))?;
    let update = Update::parse(&read_file_to_string(update_file)?, Some(update_file))
        .map_err(|e| anyhow!("Not able to parse with error: {e}"))?;
    Update::parse(&update.to_string(), None)
        .map_err(|e| anyhow!("Failure to deserialize \"{update}\" with error: {e}"))?;
    Ok(())
}

fn evaluate_negative_update_syntax_test(test: &Test) -> Result<()> {
    let update_file = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found"))?;
    ensure!(
        Update::parse(&read_file_to_string(update_file)?, Some(update_file)).is_err(),
        "Oxigraph parses even if it should not."
    );
    Ok(())
}

fn evaluate_update_evaluation_test(test: &Test) -> Result<()> {
    let store = get_store()?;
    if let Some(data) = &test.data {
        load_dataset_to_store(data, &store)?;
    }
    for (name, value) in &test.graph_data {
        load_graph_to_store(value, &store, name.clone())?;
    }

    let result_store = get_store()?;
    if let Some(data) = &test.result {
        load_dataset_to_store(data, &result_store)?;
    }
    for (name, value) in &test.result_graph_data {
        load_graph_to_store(value, &result_store, name.clone())?;
    }

    let update_file = test
        .update
        .as_deref()
        .ok_or_else(|| anyhow!("No action found"))?;
    let update = Update::parse(&read_file_to_string(update_file)?, Some(update_file))
        .map_err(|e| anyhow!("Failure to parse update with error: {e}"))?;

    // We check parsing roundtrip
    Update::parse(&update.to_string(), None)
        .map_err(|e| anyhow!("Failure to deserialize \"{update}\" with error: {e}"))?;

    store
        .update(update)
        .map_err(|e| anyhow!("Failure to execute update with error: {e}"))?;
    let mut store_dataset: Dataset = store.iter().collect::<Result<_, _>>()?;
    store_dataset.canonicalize();
    let mut result_store_dataset: Dataset = result_store.iter().collect::<Result<_, _>>()?;
    result_store_dataset.canonicalize();
    ensure!(
        store_dataset == result_store_dataset,
        "Not isomorphic result dataset.\nDiff:\n{}\nParsed update:\n{}\n",
        dataset_diff(&result_store_dataset, &store_dataset),
        Update::parse(&read_file_to_string(update_file)?, Some(update_file)).unwrap(),
    );
    Ok(())
}

fn load_sparql_query_result(url: &str) -> Result<StaticQueryResults> {
    if let Some(format) = url
        .rsplit_once('.')
        .and_then(|(_, extension)| QueryResultsFormat::from_extension(extension))
    {
        StaticQueryResults::from_query_results(QueryResults::read(read_file(url)?, format)?, false)
    } else {
        StaticQueryResults::from_graph(&load_graph(url, guess_rdf_format(url)?, false)?)
    }
}

#[derive(Clone)]
struct StaticServiceHandler {
    services: Arc<HashMap<NamedNode, StoreRef>>,
}

impl StaticServiceHandler {
    fn new(services: &[(String, String)]) -> Result<Self> {
        Ok(Self {
            services: Arc::new(
                services
                    .iter()
                    .map(|(name, data)| {
                        let name = NamedNode::new(name)?;
                        let store = get_store()?;
                        load_dataset_to_store(data, &store)?;
                        Ok((name, store))
                    })
                    .collect::<Result<_>>()?,
            ),
        })
    }
}

impl ServiceHandler for StaticServiceHandler {
    type Error = EvaluationError;

    fn handle(
        &self,
        service_name: NamedNode,
        query: Query,
    ) -> std::result::Result<QueryResults, EvaluationError> {
        self.services
            .get(&service_name)
            .ok_or_else(|| {
                EvaluationError::Service(Box::new(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Service {service_name} not found"),
                )))
            })?
            .query_opt(
                query,
                QueryOptions::default().with_service_handler(self.clone()),
            )
    }
}

fn to_graph(result: QueryResults, with_order: bool) -> Result<Graph> {
    Ok(match result {
        QueryResults::Graph(graph) => graph.collect::<Result<Graph, _>>()?,
        QueryResults::Boolean(value) => {
            let mut graph = Graph::new();
            let result_set = BlankNode::default();
            graph.insert(TripleRef::new(&result_set, rdf::TYPE, rs::RESULT_SET));
            graph.insert(TripleRef::new(
                &result_set,
                rs::BOOLEAN,
                &Literal::from(value),
            ));
            graph
        }
        QueryResults::Solutions(solutions) => {
            let mut graph = Graph::new();
            let result_set = BlankNode::default();
            graph.insert(TripleRef::new(&result_set, rdf::TYPE, rs::RESULT_SET));
            for variable in solutions.variables() {
                graph.insert(TripleRef::new(
                    &result_set,
                    rs::RESULT_VARIABLE,
                    LiteralRef::new_simple_literal(variable.as_str()),
                ));
            }
            for (i, solution) in solutions.enumerate() {
                let solution = solution?;
                let solution_id = BlankNode::default();
                graph.insert(TripleRef::new(&result_set, rs::SOLUTION, &solution_id));
                for (variable, value) in solution.iter() {
                    let binding = BlankNode::default();
                    graph.insert(TripleRef::new(&solution_id, rs::BINDING, &binding));
                    graph.insert(TripleRef::new(&binding, rs::VALUE, value));
                    graph.insert(TripleRef::new(
                        &binding,
                        rs::VARIABLE,
                        LiteralRef::new_simple_literal(variable.as_str()),
                    ));
                }
                if with_order {
                    graph.insert(TripleRef::new(
                        &solution_id,
                        rs::INDEX,
                        &Literal::from((i + 1) as i128),
                    ));
                }
            }
            graph
        }
    })
}

fn are_query_results_isomorphic(
    expected: &StaticQueryResults,
    actual: &StaticQueryResults,
) -> bool {
    match (expected, actual) {
        (
            StaticQueryResults::Solutions {
                variables: expected_variables,
                solutions: expected_solutions,
                ordered,
            },
            StaticQueryResults::Solutions {
                variables: actual_variables,
                solutions: actual_solutions,
                ..
            },
        ) => {
            expected_variables == actual_variables
                && expected_solutions.len() == actual_solutions.len()
                && if *ordered {
                    expected_solutions.iter().zip(actual_solutions).all(
                        |(expected_solution, actual_solution)| {
                            compare_solutions(expected_solution, actual_solution)
                        },
                    )
                } else {
                    expected_solutions.iter().all(|expected_solution| {
                        actual_solutions.iter().any(|actual_solution| {
                            compare_solutions(expected_solution, actual_solution)
                        })
                    })
                }
        }
        (StaticQueryResults::Boolean(expected), StaticQueryResults::Boolean(actual)) => {
            expected == actual
        }
        (StaticQueryResults::Graph(expected), StaticQueryResults::Graph(actual)) => {
            expected == actual
        }
        _ => false,
    }
}

fn compare_solutions(expected: &[(Variable, Term)], actual: &[(Variable, Term)]) -> bool {
    let mut bnode_map = HashMap::new();
    expected.len() == actual.len()
        && expected.iter().zip(actual).all(
            move |((expected_variable, expected_value), (actual_variable, actual_value))| {
                expected_variable == actual_variable
                    && compare_terms(
                        expected_value.as_ref(),
                        actual_value.as_ref(),
                        &mut bnode_map,
                    )
            },
        )
}

fn compare_terms<'a>(
    expected: TermRef<'a>,
    actual: TermRef<'a>,
    bnode_map: &mut HashMap<BlankNodeRef<'a>, BlankNodeRef<'a>>,
) -> bool {
    match (expected, actual) {
        (TermRef::BlankNode(expected), TermRef::BlankNode(actual)) => {
            expected == *bnode_map.entry(actual).or_insert(expected)
        }
        (TermRef::Triple(expected), TermRef::Triple(actual)) => {
            compare_terms(
                expected.subject.as_ref().into(),
                actual.subject.as_ref().into(),
                bnode_map,
            ) && compare_terms(
                expected.predicate.as_ref().into(),
                actual.predicate.as_ref().into(),
                bnode_map,
            ) && compare_terms(expected.object.as_ref(), actual.object.as_ref(), bnode_map)
        }
        (expected, actual) => expected == actual,
    }
}

#[allow(clippy::large_enum_variant)]
enum StaticQueryResults {
    Graph(Graph),
    Solutions {
        variables: Vec<Variable>,
        solutions: Vec<Vec<(Variable, Term)>>,
        ordered: bool,
    },
    Boolean(bool),
}

impl StaticQueryResults {
    fn from_query_results(results: QueryResults, with_order: bool) -> Result<Self> {
        Self::from_graph(&to_graph(results, with_order)?)
    }

    fn from_graph(graph: &Graph) -> Result<Self> {
        // Hack to normalize literals
        let store = get_store()?;
        for t in graph {
            store.insert(t.in_graph(GraphNameRef::DefaultGraph))?;
        }
        let mut graph = store
            .iter()
            .map(|q| Ok(Triple::from(q?)))
            .collect::<Result<Graph>>()?;

        if let Some(result_set) = graph.subject_for_predicate_object(rdf::TYPE, rs::RESULT_SET) {
            if let Some(bool) = graph.object_for_subject_predicate(result_set, rs::BOOLEAN) {
                // Boolean query
                Ok(Self::Boolean(bool == Literal::from(true).as_ref().into()))
            } else {
                // Regular query
                let mut variables: Vec<Variable> = graph
                    .objects_for_subject_predicate(result_set, rs::RESULT_VARIABLE)
                    .map(|object| {
                        let TermRef::Literal(l) = object else {
                            bail!("Invalid rs:resultVariable: {object}")
                        };
                        Ok(Variable::new_unchecked(l.value()))
                    })
                    .collect::<Result<Vec<_>>>()?;
                variables.sort();

                let mut solutions = graph
                    .objects_for_subject_predicate(result_set, rs::SOLUTION)
                    .map(|object| {
                        let TermRef::BlankNode(solution) = object else {
                            bail!("Invalid rs:solution: {object}")
                        };
                        let mut bindings = graph
                            .objects_for_subject_predicate(solution, rs::BINDING)
                            .map(|object| {
                                let TermRef::BlankNode(binding) = object else {
                                    bail!("Invalid rs:binding: {object}")
                                };
                                let (Some(TermRef::Literal(variable)), Some(value)) = (
                                    graph.object_for_subject_predicate(binding, rs::VARIABLE),
                                    graph.object_for_subject_predicate(binding, rs::VALUE),
                                ) else {
                                    bail!("Invalid rs:binding: {binding}")
                                };
                                Ok((
                                    Variable::new_unchecked(variable.value()),
                                    value.into_owned(),
                                ))
                            })
                            .collect::<Result<Vec<_>>>()?;
                        bindings.sort_by(|(a, _), (b, _)| a.cmp(b));
                        let index = graph
                            .object_for_subject_predicate(solution, rs::INDEX)
                            .map(|object| {
                                let TermRef::Literal(l) = object else {
                                    bail!("Invalid rs:index: {object}")
                                };
                                Ok(u64::from_str(l.value())?)
                            })
                            .transpose()?;
                        Ok((bindings, index))
                    })
                    .collect::<Result<Vec<_>>>()?;
                solutions.sort_by(|(_, index_a), (_, index_b)| index_a.cmp(index_b));

                let ordered = solutions.iter().all(|(_, index)| index.is_some());

                Ok(Self::Solutions {
                    variables,
                    solutions: solutions
                        .into_iter()
                        .map(|(solution, _)| solution)
                        .collect(),
                    ordered,
                })
            }
        } else {
            graph.canonicalize();
            Ok(Self::Graph(graph))
        }
    }
}

fn results_diff(expected: StaticQueryResults, actual: StaticQueryResults) -> String {
    match expected {
        StaticQueryResults::Solutions {
            variables: mut expected_variables,
            solutions: expected_solutions,
            ordered,
        } => match actual {
            StaticQueryResults::Solutions {
                variables: mut actual_variables,
                solutions: actual_solutions,
                ..
            } => {
                let mut out = String::new();
                expected_variables.sort_unstable();
                actual_variables.sort_unstable();
                if expected_variables != actual_variables {
                    write!(
                        &mut out,
                        "Variables diff:\n{}",
                        format_diff(
                            &expected_variables
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join("\n"),
                            &actual_variables
                                .iter()
                                .map(ToString::to_string)
                                .collect::<Vec<_>>()
                                .join("\n"),
                            "variables",
                        )
                    )
                    .unwrap();
                }
                write!(
                    &mut out,
                    "Solutions diff:\n{}",
                    format_diff(
                        &solutions_to_string(expected_solutions, ordered),
                        &solutions_to_string(actual_solutions, ordered),
                        "solutions",
                    )
                )
                .unwrap();
                out
            }
            StaticQueryResults::Boolean(actual) => {
                format!("Expecting solutions but found the boolean {actual}")
            }
            StaticQueryResults::Graph(actual) => {
                format!("Expecting solutions but found the graph:\n{actual}")
            }
        },
        StaticQueryResults::Graph(expected) => match actual {
            StaticQueryResults::Solutions { .. } => "Expecting a graph but found solutions".into(),
            StaticQueryResults::Boolean(actual) => {
                format!("Expecting a graph but found the boolean {actual}")
            }
            StaticQueryResults::Graph(actual) => {
                let expected = expected
                    .into_iter()
                    .map(|t| t.in_graph(GraphNameRef::DefaultGraph))
                    .collect();
                let actual = actual
                    .into_iter()
                    .map(|t| t.in_graph(GraphNameRef::DefaultGraph))
                    .collect();
                dataset_diff(&expected, &actual)
            }
        },
        StaticQueryResults::Boolean(expected) => match actual {
            StaticQueryResults::Solutions { .. } => {
                "Expecting a boolean but found solutions".into()
            }
            StaticQueryResults::Boolean(actual) => {
                format!("Expecting {expected} but found {actual}")
            }
            StaticQueryResults::Graph(actual) => {
                format!("Expecting solutions but found the graph:\n{actual}")
            }
        },
    }
}

fn solutions_to_string(solutions: Vec<Vec<(Variable, Term)>>, ordered: bool) -> String {
    let mut lines = solutions
        .into_iter()
        .map(|mut s| {
            let mut out = String::new();
            write!(&mut out, "{{").unwrap();
            s.sort_unstable_by(|(v1, _), (v2, _)| v1.cmp(v2));
            for (variable, value) in s {
                write!(&mut out, "{variable} = {value} ").unwrap();
            }
            write!(&mut out, "}}").unwrap();
            out
        })
        .collect::<Vec<_>>();
    if !ordered {
        lines.sort_unstable();
    }
    lines.join("\n")
}

fn load_graph_to_store(
    url: &str,
    store: &Store,
    to_graph_name: impl Into<GraphName>,
) -> Result<()> {
    store.load_graph(
        read_file(url)?,
        guess_rdf_format(url)?,
        to_graph_name,
        Some(url),
    )?;
    Ok(())
}

fn load_dataset_to_store(url: &str, store: &Store) -> Result<()> {
    store.load_dataset(read_file(url)?, guess_rdf_format(url)?, Some(url))?;
    Ok(())
}

fn evaluate_query_optimization_test(test: &Test) -> Result<()> {
    let action = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found"))?;
    let actual = (&Optimizer::optimize_graph_pattern(
        (&if let spargebra::Query::Select { pattern, .. } =
            spargebra::Query::parse(&read_file_to_string(action)?, Some(action))?
        {
            pattern
        } else {
            bail!("Only SELECT queries are supported in query sparql-optimization tests")
        })
            .into(),
    ))
        .into();
    let result = test
        .result
        .as_ref()
        .ok_or_else(|| anyhow!("No tests result found"))?;
    let spargebra::Query::Select {
        pattern: expected, ..
    } = spargebra::Query::parse(&read_file_to_string(result)?, Some(result))?
    else {
        bail!("Only SELECT queries are supported in query sparql-optimization tests")
    };
    ensure!(
        expected == actual,
        "Not equal queries.\nDiff:\n{}\n",
        format_diff(
            &spargebra::Query::Select {
                pattern: expected,
                dataset: None,
                base_iri: None
            }
            .to_sse(),
            &spargebra::Query::Select {
                pattern: actual,
                dataset: None,
                base_iri: None
            }
            .to_sse(),
            "query"
        )
    );
    Ok(())
}

// Pool of stores to avoid allocating/deallocating them a lot
static STORE_POOL: OnceLock<Mutex<Vec<Store>>> = OnceLock::new();

fn get_store() -> Result<StoreRef> {
    let store = if let Some(store) = STORE_POOL.get_or_init(Mutex::default).lock().unwrap().pop() {
        store
    } else {
        Store::new()?
    };
    Ok(StoreRef { store })
}

struct StoreRef {
    store: Store,
}

impl Drop for StoreRef {
    fn drop(&mut self) {
        if self.store.clear().is_ok() {
            STORE_POOL
                .get_or_init(Mutex::default)
                .lock()
                .unwrap()
                .push(self.store.clone())
        }
    }
}

impl Deref for StoreRef {
    type Target = Store;

    fn deref(&self) -> &Store {
        &self.store
    }
}
