use crate::files::*;
use crate::manifest::*;
use crate::report::{dataset_diff, TestResult};
use crate::vocab::*;
use anyhow::{anyhow, Result};
use chrono::Utc;
use oxigraph::model::vocab::*;
use oxigraph::model::*;
use oxigraph::sparql::*;
use oxigraph::MemoryStore;
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use std::{fmt, io};

pub fn evaluate_sparql_tests(
    manifest: impl Iterator<Item = Result<Test>>,
) -> Result<Vec<TestResult>> {
    manifest
        .map(|test| {
            let test = test?;
            let outcome = evaluate_sparql_test(&test);
            Ok(TestResult {
                test: test.id,
                outcome,
                date: Utc::now(),
            })
        })
        .collect()
}

fn evaluate_sparql_test(test: &Test) -> Result<()> {
    if test.kind == "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#PositiveSyntaxTest"
        || test.kind
            == "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#PositiveSyntaxTest11"
    {
        let query_file = test
            .action
            .as_deref()
            .ok_or_else(|| anyhow!("No action found for test {}", test))?;
        match Query::parse(&read_file_to_string(&query_file)?, Some(&query_file)) {
            Err(error) => Err(anyhow!("Not able to parse {} with error: {}", test, error)),
            Ok(query) => match Query::parse(&query.to_string(), None) {
                Ok(_) => Ok(()),
                Err(error) => Err(anyhow!(
                    "Failure to deserialize \"{}\" of {} with error: {}",
                    query.to_string(),
                    test,
                    error
                )),
            },
        }
    } else if test.kind
        == "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#NegativeSyntaxTest"
        || test.kind
            == "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#NegativeSyntaxTest11"
    {
        let query_file = test
            .action
            .as_deref()
            .ok_or_else(|| anyhow!("No action found for test {}", test))?;
        match Query::parse(&read_file_to_string(query_file)?, Some(query_file)) {
            Ok(result) => Err(anyhow!(
                "Oxigraph parses even if it should not {}. The output tree is: {}",
                test,
                result
            )),
            Err(_) => Ok(()),
        }
    } else if test.kind
        == "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#QueryEvaluationTest"
    {
        let store = MemoryStore::new();
        if let Some(data) = &test.data {
            load_to_store(data, &store, GraphNameRef::DefaultGraph)?;
        }
        for (name, value) in &test.graph_data {
            load_to_store(value, &store, name)?;
        }
        let query_file = test
            .query
            .as_deref()
            .ok_or_else(|| anyhow!("No action found for test {}", test))?;
        let options = QueryOptions::default()
            .with_service_handler(StaticServiceHandler::new(&test.service_data)?);
        match Query::parse(&read_file_to_string(query_file)?, Some(query_file)) {
            Err(error) => Err(anyhow!(
                "Failure to parse query of {} with error: {}",
                test,
                error
            )),
            Ok(query) => {
                // FROM and FROM NAMED support. We make sure the data is in the store
                if !query.dataset().is_default_dataset() {
                    for graph_name in query.dataset().default_graph_graphs().unwrap_or(&[]) {
                        if let GraphName::NamedNode(graph_name) = graph_name {
                            load_to_store(graph_name.as_str(), &store, graph_name.as_ref())?;
                        } else {
                            return Err(anyhow!(
                                "Invalid FROM in query {} for test {}",
                                query,
                                test
                            ));
                        }
                    }
                    for graph_name in query.dataset().available_named_graphs().unwrap_or(&[]) {
                        if let NamedOrBlankNode::NamedNode(graph_name) = graph_name {
                            load_to_store(graph_name.as_str(), &store, graph_name.as_ref())?;
                        } else {
                            return Err(anyhow!(
                                "Invalid FROM NAMED in query {} for test {}",
                                query,
                                test
                            ));
                        }
                    }
                }
                match store.query_opt(query, options) {
                    Err(error) => Err(anyhow!(
                        "Failure to execute query of {} with error: {}",
                        test,
                        error
                    )),
                    Ok(actual_results) => {
                        let expected_results = load_sparql_query_result(
                            test.result.as_ref().unwrap(),
                        )
                        .map_err(|e| {
                            anyhow!("Error constructing expected graph for {}: {}", test, e)
                        })?;
                        let with_order = if let StaticQueryResults::Solutions { ordered, .. } =
                            &expected_results
                        {
                            *ordered
                        } else {
                            false
                        };
                        let actual_results =
                            StaticQueryResults::from_query_results(actual_results, with_order)?;

                        if are_query_results_isomorphic(&expected_results, &actual_results) {
                            Ok(())
                        } else {
                            Err(anyhow!("Failure on {}.\nExpected file:\n{}\nOutput file:\n{}\nParsed query:\n{}\nData:\n{}\n",
                                               test,
                                               expected_results,
                                               actual_results,
                                               Query::parse(&read_file_to_string(query_file)?, Some(query_file)).unwrap(),
                                               store
                            ))
                        }
                    }
                }
            }
        }
    } else if test.kind
        == "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#PositiveUpdateSyntaxTest11"
    {
        let update_file = test
            .action
            .as_deref()
            .ok_or_else(|| anyhow!("No action found for test {}", test))?;
        match Update::parse(&read_file_to_string(&update_file)?, Some(&update_file)) {
            Err(error) => Err(anyhow!("Not able to parse {} with error: {}", test, error)),
            Ok(update) => match Update::parse(&update.to_string(), None) {
                Ok(_) => Ok(()),
                Err(error) => Err(anyhow!(
                    "Failure to deserialize \"{}\" of {} with error: {}",
                    update.to_string(),
                    test,
                    error
                )),
            },
        }
    } else if test.kind
        == "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#NegativeUpdateSyntaxTest11"
    {
        let update_file = test
            .action
            .as_deref()
            .ok_or_else(|| anyhow!("No action found for test {}", test))?;
        match Query::parse(&read_file_to_string(update_file)?, Some(update_file)) {
            Ok(result) => Err(anyhow!(
                "Oxigraph parses even if it should not {}. The output tree is: {}",
                test,
                result
            )),
            Err(_) => Ok(()),
        }
    } else if test.kind
        == "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#UpdateEvaluationTest"
    {
        let store = MemoryStore::new();
        if let Some(data) = &test.data {
            load_to_store(data, &store, &GraphName::DefaultGraph)?;
        }
        for (name, value) in &test.graph_data {
            load_to_store(value, &store, name)?;
        }

        let result_store = MemoryStore::new();
        if let Some(data) = &test.result {
            load_to_store(data, &result_store, &GraphName::DefaultGraph)?;
        }
        for (name, value) in &test.result_graph_data {
            load_to_store(value, &result_store, name)?;
        }

        let update_file = test
            .update
            .as_deref()
            .ok_or_else(|| anyhow!("No action found for test {}", test))?;
        match Update::parse(&read_file_to_string(update_file)?, Some(update_file)) {
            Err(error) => Err(anyhow!(
                "Failure to parse update of {} with error: {}",
                test,
                error
            )),
            Ok(update) => match store.update(update) {
                Err(error) => Err(anyhow!(
                    "Failure to execute update of {} with error: {}",
                    test,
                    error
                )),
                Ok(()) => {
                    let mut store_dataset: Dataset = store.iter().collect();
                    store_dataset.canonicalize();
                    let mut result_store_dataset: Dataset = result_store.iter().collect();
                    result_store_dataset.canonicalize();
                    if store_dataset == result_store_dataset {
                        Ok(())
                    } else {
                        Err(anyhow!(
                            "Failure on {}.\nDiff:\n{}\nParsed update:\n{}\n",
                            test,
                            dataset_diff(&result_store_dataset, &store_dataset),
                            Update::parse(&read_file_to_string(update_file)?, Some(update_file))
                                .unwrap(),
                        ))
                    }
                }
            },
        }
    } else {
        Err(anyhow!("Unsupported test type: {}", test.kind))
    }
}

fn load_sparql_query_result(url: &str) -> Result<StaticQueryResults> {
    if url.ends_with(".srx") {
        StaticQueryResults::from_query_results(
            QueryResults::read(read_file(url)?, QueryResultsFormat::Xml)?,
            false,
        )
    } else if url.ends_with(".srj") {
        StaticQueryResults::from_query_results(
            QueryResults::read(read_file(url)?, QueryResultsFormat::Json)?,
            false,
        )
    } else if url.ends_with(".tsv") {
        StaticQueryResults::from_query_results(
            QueryResults::read(read_file(url)?, QueryResultsFormat::Tsv)?,
            false,
        )
    } else {
        Ok(StaticQueryResults::from_graph(load_graph(url)?))
    }
}

#[derive(Clone)]
struct StaticServiceHandler {
    services: Arc<HashMap<NamedNode, MemoryStore>>,
}

impl StaticServiceHandler {
    fn new(services: &[(String, String)]) -> Result<Self> {
        Ok(Self {
            services: Arc::new(
                services
                    .iter()
                    .map(|(name, data)| {
                        let name = NamedNode::new(name)?;
                        let store = MemoryStore::new();
                        load_to_store(&data, &store, &GraphName::DefaultGraph)?;
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
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Service {} not found", service_name),
                )
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
    expected.iter().zip(actual).all(
        move |((expected_variable, expected_value), (actual_variable, actual_value))| {
            expected_variable == actual_variable
                && expected_value
                    == if let Term::BlankNode(actual_value) = actual_value {
                        bnode_map.entry(actual_value).or_insert(expected_value)
                    } else {
                        actual_value
                    }
        },
    )
}

enum StaticQueryResults {
    Graph(Graph),
    Solutions {
        variables: Vec<Variable>,
        solutions: Vec<Vec<(Variable, Term)>>,
        ordered: bool,
    },
    Boolean(bool),
}

impl fmt::Display for StaticQueryResults {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StaticQueryResults::Graph(g) => g.fmt(f),
            StaticQueryResults::Solutions {
                variables,
                solutions,
                ..
            } => {
                write!(f, "Variables:")?;
                for v in variables {
                    write!(f, " {}", v)?;
                }
                for solution in solutions {
                    write!(f, "\n{{")?;
                    for (k, v) in solution {
                        write!(f, "{} = {} ", k, v)?;
                    }
                    write!(f, "}}")?;
                }
                Ok(())
            }
            StaticQueryResults::Boolean(b) => b.fmt(f),
        }
    }
}

impl StaticQueryResults {
    fn from_query_results(results: QueryResults, with_order: bool) -> Result<StaticQueryResults> {
        Ok(Self::from_graph(to_graph(results, with_order)?))
    }

    fn from_graph(graph: Graph) -> StaticQueryResults {
        // Hack to normalize literals
        let mut graph: Graph = graph
            .iter()
            .map(|t| t.into_owned().in_graph(GraphName::DefaultGraph))
            .collect::<MemoryStore>()
            .into_iter()
            .map(Triple::from)
            .collect();

        if let Some(result_set) = graph.subject_for_predicate_object(rdf::TYPE, rs::RESULT_SET) {
            if let Some(bool) = graph.object_for_subject_predicate(result_set, rs::BOOLEAN) {
                // Boolean query
                StaticQueryResults::Boolean(bool == Literal::from(true).as_ref().into())
            } else {
                // Regular query
                let mut variables: Vec<Variable> = graph
                    .objects_for_subject_predicate(result_set, rs::RESULT_VARIABLE)
                    .filter_map(|object| {
                        if let TermRef::Literal(l) = object {
                            Some(Variable::new_unchecked(l.value()))
                        } else {
                            None
                        }
                    })
                    .collect();
                variables.sort();

                let mut solutions: Vec<_> = graph
                    .objects_for_subject_predicate(result_set, rs::SOLUTION)
                    .filter_map(|object| {
                        if let TermRef::BlankNode(solution) = object {
                            let mut bindings = graph
                                .objects_for_subject_predicate(solution, rs::BINDING)
                                .filter_map(|object| {
                                    if let TermRef::BlankNode(binding) = object {
                                        if let (Some(TermRef::Literal(variable)), Some(value)) = (
                                            graph.object_for_subject_predicate(
                                                binding,
                                                rs::VARIABLE,
                                            ),
                                            graph.object_for_subject_predicate(binding, rs::VALUE),
                                        ) {
                                            Some((
                                                Variable::new_unchecked(variable.value()),
                                                value.into_owned(),
                                            ))
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                })
                                .collect::<Vec<_>>();
                            bindings.sort_by(|(a, _), (b, _)| a.cmp(&b));
                            let index = graph
                                .object_for_subject_predicate(solution, rs::INDEX)
                                .and_then(|object| {
                                    if let TermRef::Literal(l) = object {
                                        u64::from_str(l.value()).ok()
                                    } else {
                                        None
                                    }
                                });
                            Some((bindings, index))
                        } else {
                            None
                        }
                    })
                    .collect();
                solutions.sort_by(|(_, index_a), (_, index_b)| index_a.cmp(index_b));

                let ordered = solutions.iter().all(|(_, index)| index.is_some());

                StaticQueryResults::Solutions {
                    variables,
                    solutions: solutions
                        .into_iter()
                        .map(|(solution, _)| solution)
                        .collect(),
                    ordered,
                }
            }
        } else {
            graph.canonicalize();
            StaticQueryResults::Graph(graph)
        }
    }
}
