use crate::files::*;
use crate::manifest::*;
use crate::report::{store_diff, TestResult};
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
            Ok(query) => match store.query_opt(query, options) {
                Err(error) => Err(anyhow!(
                    "Failure to execute query of {} with error: {}",
                    test,
                    error
                )),
                Ok(actual_results) => {
                    let expected_results = load_sparql_query_result(test.result.as_ref().unwrap())
                        .map_err(|e| {
                            anyhow!("Error constructing expected graph for {}: {}", test, e)
                        })?;
                    let with_order =
                        if let StaticQueryResults::Solutions { ordered, .. } = &expected_results {
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
            },
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
                    if store.is_isomorphic(&result_store) {
                        Ok(())
                    } else {
                        Err(anyhow!(
                            "Failure on {}.\nDiff:\n{}\nParsed update:\n{}\n",
                            test,
                            store_diff(&result_store, &store),
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
        Ok(StaticQueryResults::from_dataset(load_store(url)?))
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

fn to_dataset(result: QueryResults, with_order: bool) -> Result<MemoryStore> {
    match result {
        QueryResults::Graph(graph) => Ok(graph
            .map(|t| t.map(|t| t.in_graph(None)))
            .collect::<Result<_, _>>()?),
        QueryResults::Boolean(value) => {
            let store = MemoryStore::new();
            let result_set = BlankNode::default();
            store.insert(Quad::new(
                result_set.clone(),
                rdf::TYPE,
                rs::RESULT_SET,
                None,
            ));
            store.insert(Quad::new(
                result_set,
                rs::BOOLEAN,
                Literal::from(value),
                None,
            ));
            Ok(store)
        }
        QueryResults::Solutions(solutions) => {
            let store = MemoryStore::new();
            let result_set = BlankNode::default();
            store.insert(Quad::new(
                result_set.clone(),
                rdf::TYPE,
                rs::RESULT_SET,
                None,
            ));
            for variable in solutions.variables() {
                store.insert(Quad::new(
                    result_set.clone(),
                    rs::RESULT_VARIABLE,
                    Literal::new_simple_literal(variable.as_str()),
                    None,
                ));
            }
            for (i, solution) in solutions.enumerate() {
                let solution = solution?;
                let solution_id = BlankNode::default();
                store.insert(Quad::new(
                    result_set.clone(),
                    rs::SOLUTION,
                    solution_id.clone(),
                    None,
                ));
                for (variable, value) in solution.iter() {
                    let binding = BlankNode::default();
                    store.insert(Quad::new(
                        solution_id.clone(),
                        rs::BINDING,
                        binding.clone(),
                        None,
                    ));
                    store.insert(Quad::new(binding.clone(), rs::VALUE, value.clone(), None));
                    store.insert(Quad::new(
                        binding,
                        rs::VARIABLE,
                        Literal::new_simple_literal(variable.as_str()),
                        None,
                    ));
                }
                if with_order {
                    store.insert(Quad::new(
                        solution_id,
                        rs::INDEX,
                        Literal::from((i + 1) as i128),
                        None,
                    ));
                }
            }
            Ok(store)
        }
    }
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
            expected.is_isomorphic(&actual)
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
    Graph(MemoryStore),
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
        Ok(Self::from_dataset(to_dataset(results, with_order)?))
    }

    fn from_dataset(dataset: MemoryStore) -> StaticQueryResults {
        if let Some(result_set) = dataset
            .quads_for_pattern(None, Some(rdf::TYPE), Some(rs::RESULT_SET.into()), None)
            .map(|q| q.subject)
            .next()
        {
            if let Some(bool) = object_for_subject_predicate(&dataset, &result_set, rs::BOOLEAN) {
                // Boolean query
                StaticQueryResults::Boolean(bool == Literal::from(true).into())
            } else {
                // Regular query
                let mut variables: Vec<Variable> =
                    objects_for_subject_predicate(&dataset, &result_set, rs::RESULT_VARIABLE)
                        .filter_map(|object| {
                            if let Term::Literal(l) = object {
                                Some(Variable::new_unchecked(l.value()))
                            } else {
                                None
                            }
                        })
                        .collect();
                variables.sort();

                let mut solutions: Vec<_> =
                    objects_for_subject_predicate(&dataset, &result_set, rs::SOLUTION)
                        .filter_map(|object| {
                            if let Term::BlankNode(solution) = object {
                                let mut bindings =
                                    objects_for_subject_predicate(&dataset, &solution, rs::BINDING)
                                        .filter_map(|object| {
                                            if let Term::BlankNode(binding) = object {
                                                if let (
                                                    Some(Term::Literal(variable)),
                                                    Some(value),
                                                ) = (
                                                    object_for_subject_predicate(
                                                        &dataset,
                                                        &binding,
                                                        rs::VARIABLE,
                                                    ),
                                                    object_for_subject_predicate(
                                                        &dataset,
                                                        &binding,
                                                        rs::VALUE,
                                                    ),
                                                ) {
                                                    Some((
                                                        Variable::new_unchecked(variable.value()),
                                                        value,
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
                                let index =
                                    object_for_subject_predicate(&dataset, &solution, rs::INDEX)
                                        .and_then(|object| {
                                            if let Term::Literal(l) = object {
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
            StaticQueryResults::Graph(dataset)
        }
    }
}

fn object_for_subject_predicate<'a>(
    store: &MemoryStore,
    subject: impl Into<NamedOrBlankNodeRef<'a>>,
    predicate: impl Into<NamedNodeRef<'a>>,
) -> Option<Term> {
    objects_for_subject_predicate(store, subject, predicate).next()
}

fn objects_for_subject_predicate<'a>(
    store: &MemoryStore,
    subject: impl Into<NamedOrBlankNodeRef<'a>>,
    predicate: impl Into<NamedNodeRef<'a>>,
) -> impl Iterator<Item = Term> {
    store
        .quads_for_pattern(Some(subject.into()), Some(predicate.into()), None, None)
        .map(|t| t.object)
}
