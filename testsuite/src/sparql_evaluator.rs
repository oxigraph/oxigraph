use crate::evaluator::TestEvaluator;
use crate::files::*;
use crate::manifest::*;
use crate::report::dataset_diff;
use crate::vocab::*;
use anyhow::{anyhow, Result};
use oxigraph::model::vocab::*;
use oxigraph::model::*;
use oxigraph::sparql::*;
use oxigraph::store::Store;
use std::collections::HashMap;
use std::io::Cursor;
use std::str::FromStr;
use std::sync::Arc;
use std::{fmt, io};

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
        "https://github.com/oxigraph/oxigraph/tests#NegativeJsonResultsSyntaxTest",
        evaluate_negative_json_result_syntax_test,
    );
    evaluator.register(
        "https://github.com/oxigraph/oxigraph/tests#NegativeXmlResultsSyntaxTest",
        evaluate_negative_xml_result_syntax_test,
    );
    evaluator.register(
        "https://github.com/oxigraph/oxigraph/tests#NegativeTsvResultsSyntaxTest",
        evaluate_negative_tsv_result_syntax_test,
    );
}

fn evaluate_positive_syntax_test(test: &Test) -> Result<()> {
    let query_file = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {}", test))?;
    match Query::parse(&read_file_to_string(query_file)?, Some(query_file)) {
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
}

fn evaluate_negative_syntax_test(test: &Test) -> Result<()> {
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
}

fn evaluate_negative_json_result_syntax_test(test: &Test) -> Result<()> {
    if result_syntax_check(test, QueryResultsFormat::Json).is_ok() {
        Err(anyhow!("Oxigraph parses even if it should not {}.", test))
    } else {
        Ok(())
    }
}

fn evaluate_negative_xml_result_syntax_test(test: &Test) -> Result<()> {
    if result_syntax_check(test, QueryResultsFormat::Xml).is_ok() {
        Err(anyhow!("Oxigraph parses even if it should not {}.", test))
    } else {
        Ok(())
    }
}

fn evaluate_negative_tsv_result_syntax_test(test: &Test) -> Result<()> {
    if result_syntax_check(test, QueryResultsFormat::Tsv).is_ok() {
        Err(anyhow!("Oxigraph parses even if it should not {}.", test))
    } else {
        Ok(())
    }
}

fn result_syntax_check(test: &Test, format: QueryResultsFormat) -> Result<()> {
    let results_file = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {}", test))?;
    match QueryResults::read(Cursor::new(read_file_to_string(results_file)?), format)? {
        QueryResults::Solutions(solutions) => {
            for s in solutions {
                s?;
            }
        }
        QueryResults::Graph(triples) => {
            for t in triples {
                t?;
            }
        }
        QueryResults::Boolean(_) => (),
    }
    Ok(())
}

fn evaluate_evaluation_test(test: &Test) -> Result<()> {
    let store = Store::new()?;
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
                        return Err(anyhow!("Invalid FROM in query {} for test {}", query, test));
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
            }
        }
    }
}

fn evaluate_positive_update_syntax_test(test: &Test) -> Result<()> {
    let update_file = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {}", test))?;
    match Update::parse(&read_file_to_string(update_file)?, Some(update_file)) {
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
}

fn evaluate_negative_update_syntax_test(test: &Test) -> Result<()> {
    let update_file = test
        .action
        .as_deref()
        .ok_or_else(|| anyhow!("No action found for test {}", test))?;
    match Update::parse(&read_file_to_string(update_file)?, Some(update_file)) {
        Ok(result) => Err(anyhow!(
            "Oxigraph parses even if it should not {}. The output tree is: {}",
            test,
            result
        )),
        Err(_) => Ok(()),
    }
}

fn evaluate_update_evaluation_test(test: &Test) -> Result<()> {
    let store = Store::new()?;
    if let Some(data) = &test.data {
        load_to_store(data, &store, GraphNameRef::DefaultGraph)?;
    }
    for (name, value) in &test.graph_data {
        load_to_store(value, &store, name)?;
    }

    let result_store = Store::new()?;
    if let Some(data) = &test.result {
        load_to_store(data, &result_store, GraphNameRef::DefaultGraph)?;
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
                let mut store_dataset: Dataset = store.iter().collect::<Result<_, _>>()?;
                store_dataset.canonicalize();
                let mut result_store_dataset: Dataset =
                    result_store.iter().collect::<Result<_, _>>()?;
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
        StaticQueryResults::from_graph(load_graph(url)?)
    }
}

#[derive(Clone)]
struct StaticServiceHandler {
    services: Arc<HashMap<NamedNode, Store>>,
}

impl StaticServiceHandler {
    fn new(services: &[(String, String)]) -> Result<Self> {
        Ok(Self {
            services: Arc::new(
                services
                    .iter()
                    .map(|(name, data)| {
                        let name = NamedNode::new(name)?;
                        let store = Store::new()?;
                        load_to_store(data, &store, GraphNameRef::DefaultGraph)?;
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
    expected.iter().zip(actual).all(
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
        Self::from_graph(to_graph(results, with_order)?)
    }

    fn from_graph(graph: Graph) -> Result<StaticQueryResults> {
        // Hack to normalize literals
        let store = Store::new().unwrap();
        for t in graph.iter() {
            store
                .insert(t.in_graph(GraphNameRef::DefaultGraph))
                .unwrap();
        }
        let mut graph: Graph = store.iter().map(|q| Triple::from(q.unwrap())).collect();

        if let Some(result_set) = graph.subject_for_predicate_object(rdf::TYPE, rs::RESULT_SET) {
            if let Some(bool) = graph.object_for_subject_predicate(result_set, rs::BOOLEAN) {
                // Boolean query
                Ok(StaticQueryResults::Boolean(
                    bool == Literal::from(true).as_ref().into(),
                ))
            } else {
                // Regular query
                let mut variables: Vec<Variable> = graph
                    .objects_for_subject_predicate(result_set, rs::RESULT_VARIABLE)
                    .map(|object| {
                        if let TermRef::Literal(l) = object {
                            Ok(Variable::new_unchecked(l.value()))
                        } else {
                            Err(anyhow!("Invalid rs:resultVariable: {}", object))
                        }
                    })
                    .collect::<Result<Vec<_>>>()?;
                variables.sort();

                let mut solutions = graph
                    .objects_for_subject_predicate(result_set, rs::SOLUTION)
                    .map(|object| {
                        if let TermRef::BlankNode(solution) = object {
                            let mut bindings = graph
                                .objects_for_subject_predicate(solution, rs::BINDING)
                                .map(|object| {
                                    if let TermRef::BlankNode(binding) = object {
                                        if let (Some(TermRef::Literal(variable)), Some(value)) = (
                                            graph.object_for_subject_predicate(
                                                binding,
                                                rs::VARIABLE,
                                            ),
                                            graph.object_for_subject_predicate(binding, rs::VALUE),
                                        ) {
                                            Ok((
                                                Variable::new_unchecked(variable.value()),
                                                value.into_owned(),
                                            ))
                                        } else {
                                            Err(anyhow!("Invalid rs:binding: {}", binding))
                                        }
                                    } else {
                                        Err(anyhow!("Invalid rs:binding: {}", object))
                                    }
                                })
                                .collect::<Result<Vec<_>>>()?;
                            bindings.sort_by(|(a, _), (b, _)| a.cmp(b));
                            let index = graph
                                .object_for_subject_predicate(solution, rs::INDEX)
                                .map(|object| {
                                    if let TermRef::Literal(l) = object {
                                        Ok(u64::from_str(l.value())?)
                                    } else {
                                        Err(anyhow!("Invalid rs:index: {}", object))
                                    }
                                })
                                .transpose()?;
                            Ok((bindings, index))
                        } else {
                            Err(anyhow!("Invalid rs:solution: {}", object))
                        }
                    })
                    .collect::<Result<Vec<_>>>()?;
                solutions.sort_by(|(_, index_a), (_, index_b)| index_a.cmp(index_b));

                let ordered = solutions.iter().all(|(_, index)| index.is_some());

                Ok(StaticQueryResults::Solutions {
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
            Ok(StaticQueryResults::Graph(graph))
        }
    }
}
