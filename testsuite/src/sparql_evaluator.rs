use crate::evaluator::TestEvaluator;
use crate::files::*;
use crate::manifest::*;
use crate::report::{dataset_diff, format_diff};
use crate::vocab::*;
use anyhow::{Context, Error, Result, bail, ensure};
use oxigraph::io::RdfParser;
use oxigraph::model::dataset::CanonicalizationAlgorithm;
use oxigraph::model::vocab::rdf;
use oxigraph::model::{
    BlankNode, BlankNodeRef, Dataset, Graph, GraphName, GraphNameRef, Literal, LiteralRef,
    NamedNode, Term, TermRef, Triple, TripleRef, Variable,
};
use oxigraph::sparql::QueryResults;
use oxigraph::sparql::results::QueryResultsFormat;
use oxigraph::store::Store;
use oxiri::Iri;
use spareval::{DefaultServiceHandler, QueryEvaluationError, QueryEvaluator, QuerySolutionIter};
use spargebra::algebra::GraphPattern;
use spargebra::{Query, SparqlParser};
use spargeo::add_geosparql_functions;
use sparopt::Optimizer;
use std::collections::HashMap;
use std::fmt::Write;
use std::io::{self, Cursor};
use std::str::FromStr;
use std::sync::Arc;

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
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#PositiveUpdateSyntaxTest",
        evaluate_positive_update_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#PositiveUpdateSyntaxTest11",
        evaluate_positive_update_syntax_test,
    );
    evaluator.register(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#NegativeUpdateSyntaxTest",
        evaluate_negative_update_syntax_test,
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
    let query_file = test.action.as_deref().context("No action found")?;
    let query = SparqlParser::new()
        .with_base_iri(query_file)?
        .parse_query(&read_file_to_string(query_file)?)
        .context("Not able to parse")?;
    SparqlParser::new()
        .parse_query(&query.to_string())
        .with_context(|| format!("Failure to deserialize \"{query}\""))?;
    Ok(())
}

fn evaluate_negative_syntax_test(test: &Test) -> Result<()> {
    let query_file = test.action.as_deref().context("No action found")?;
    ensure!(
        SparqlParser::new()
            .with_base_iri(query_file)?
            .parse_query(&read_file_to_string(query_file)?)
            .is_err(),
        "Oxigraph parses even if it should not."
    );
    Ok(())
}

fn evaluate_positive_result_syntax_test(test: &Test, format: QueryResultsFormat) -> Result<()> {
    let action_file = test.action.as_deref().context("No action found")?;
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
    let action_file = test.action.as_deref().context("No action found")?;
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
    let mut dataset = Dataset::new();
    if let Some(data) = &test.data {
        load_to_dataset(data, &mut dataset, GraphName::DefaultGraph)?;
    }
    for (name, value) in &test.graph_data {
        load_to_dataset(value, &mut dataset, name.clone())?;
    }
    let query_file = test.query.as_deref().context("No action found")?;
    let query = SparqlParser::new()
        .with_base_iri(query_file)?
        .parse_query(&read_file_to_string(query_file)?)
        .context("Failure to parse query")?;

    // We check parsing roundtrip
    SparqlParser::new()
        .parse_query(&query.to_string())
        .with_context(|| format!("Failure to deserialize \"{query}\""))?;

    let evaluator = QueryEvaluator::new()
        .with_default_service_handler(StaticServiceHandler::new(&test.service_data)?);
    let evaluator = add_geosparql_functions(evaluator);

    // FROM and FROM NAMED support. We make sure the data is in the store
    if let Some(query_dataset) = query.dataset() {
        for graph_name in &query_dataset.default {
            load_to_dataset(graph_name.as_str(), &mut dataset, GraphName::DefaultGraph)?;
        }
        if let Some(named_graphs) = &query_dataset.named {
            for graph_name in named_graphs {
                load_to_dataset(graph_name.as_str(), &mut dataset, graph_name.clone())?;
            }
        }
    }

    let expected_results = load_sparql_query_result(test.result.as_ref().unwrap())
        .context("Error constructing expected graph")?;
    let with_order = if let StaticQueryResults::Solutions { ordered, .. } = &expected_results {
        *ordered
    } else {
        false
    };

    for with_query_optimizer in [true, false] {
        let mut evaluator = evaluator.clone();
        if !with_query_optimizer {
            evaluator = evaluator.without_optimizations();
        }
        let actual_results = evaluator.execute(dataset.clone(), &query)?;
        let actual_results =
            StaticQueryResults::from_query_results(actual_results.into(), with_order)
                .with_context(|| format!("Error when executing {query}"))?;

        ensure!(
            are_query_results_isomorphic(&expected_results, &actual_results),
            "Not isomorphic results.\n{}\nParsed query:\n{query}\nData:\n{dataset}\n",
            results_diff(expected_results, actual_results),
        );
    }
    Ok(())
}

fn evaluate_positive_update_syntax_test(test: &Test) -> Result<()> {
    let update_file = test.action.as_deref().context("No action found")?;
    let update = SparqlParser::new()
        .with_base_iri(update_file)?
        .parse_update(&read_file_to_string(update_file)?)
        .context("Not able to parse")?;
    SparqlParser::new()
        .parse_update(&update.to_string())
        .with_context(|| format!("Failure to deserialize \"{update}\""))?;
    Ok(())
}

fn evaluate_negative_update_syntax_test(test: &Test) -> Result<()> {
    let update_file = test.action.as_deref().context("No action found")?;
    ensure!(
        SparqlParser::new()
            .with_base_iri(update_file)?
            .parse_update(&read_file_to_string(update_file)?)
            .is_err(),
        "Oxigraph parses even if it should not."
    );
    Ok(())
}

fn evaluate_update_evaluation_test(test: &Test) -> Result<()> {
    let store = Store::new()?;
    if let Some(data) = &test.data {
        load_to_store(data, &store, GraphName::DefaultGraph)?;
    }
    for (name, value) in &test.graph_data {
        load_to_store(value, &store, name.clone())?;
    }

    let result_store = Store::new()?;
    if let Some(data) = &test.result {
        load_to_store(data, &result_store, GraphName::DefaultGraph)?;
    }
    for (name, value) in &test.result_graph_data {
        load_to_store(value, &result_store, name.clone())?;
    }

    let update_file = test.update.as_deref().context("No action found")?;
    let update = SparqlParser::new()
        .with_base_iri(update_file)?
        .parse_update(&read_file_to_string(update_file)?)
        .context("Failure to parse update")?;

    // We check parsing roundtrip
    SparqlParser::new()
        .parse_update(&update.to_string())
        .with_context(|| format!("Failure to deserialize \"{update}\""))?;

    store
        .update(update.clone())
        .context("Failure to execute update")?;
    let mut store_dataset: Dataset = store.iter().collect::<Result<_, _>>()?;
    store_dataset.canonicalize(CanonicalizationAlgorithm::Unstable);
    let mut result_store_dataset: Dataset = result_store.iter().collect::<Result<_, _>>()?;
    result_store_dataset.canonicalize(CanonicalizationAlgorithm::Unstable);
    ensure!(
        store_dataset == result_store_dataset,
        "Not isomorphic result dataset.\nDiff:\n{}\nParsed update:\n{}\n",
        dataset_diff(&result_store_dataset, &store_dataset),
        update
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
    services: Arc<HashMap<NamedNode, Dataset>>,
}

impl StaticServiceHandler {
    fn new(services: &[(String, String)]) -> Result<Self> {
        Ok(Self {
            services: Arc::new(
                services
                    .iter()
                    .map(|(name, data)| {
                        let name = NamedNode::new(name)?;
                        let dataset = load_dataset(data, guess_rdf_format(data)?, false, false)?;
                        Ok((name, dataset))
                    })
                    .collect::<Result<_>>()?,
            ),
        })
    }
}

impl DefaultServiceHandler for StaticServiceHandler {
    type Error = QueryEvaluationError;

    fn handle(
        &self,
        service_name: NamedNode,
        pattern: GraphPattern,
        base_iri: Option<String>,
    ) -> Result<QuerySolutionIter, QueryEvaluationError> {
        let dataset = self.services.get(&service_name).ok_or_else(|| {
            QueryEvaluationError::Service(Box::new(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("Service {service_name} not found"),
            )))
        })?;

        let evaluator = QueryEvaluator::new().with_default_service_handler(StaticServiceHandler {
            services: Arc::clone(&self.services),
        });
        let spareval::QueryResults::Solutions(iter) = evaluator.execute(
            dataset.clone(),
            &Query::Select {
                dataset: None,
                pattern,
                base_iri: base_iri.map(Iri::parse).transpose().map_err(|e| {
                    QueryEvaluationError::Service(Box::new(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        format!("Invalid base IRI: {e}"),
                    )))
                })?,
            },
        )?
        else {
            return Err(QueryEvaluationError::Service(Box::new(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Expecting solutions",
            ))));
        };
        Ok(iter)
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
                for (variable, value) in &solution {
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

enum StaticQueryResults {
    Graph(Box<Graph>),
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
        let store = Store::new()?;
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
            graph.canonicalize(CanonicalizationAlgorithm::Unstable);
            Ok(Self::Graph(Box::new(graph)))
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
            out.write_str("{").unwrap();
            s.sort_unstable_by(|(v1, _), (v2, _)| v1.cmp(v2));
            for (variable, value) in s {
                write!(&mut out, "{variable} = {value} ").unwrap();
            }
            out.write_str("}").unwrap();
            out
        })
        .collect::<Vec<_>>();
    if !ordered {
        lines.sort_unstable();
    }
    lines.join("\n")
}

fn load_to_store(url: &str, store: &Store, to_graph_name: impl Into<GraphName>) -> Result<()> {
    store.load_from_reader(
        RdfParser::from_format(guess_rdf_format(url)?)
            .with_base_iri(url)?
            .with_default_graph(to_graph_name),
        read_file(url)?,
    )?;
    Ok(())
}

fn load_to_dataset(
    url: &str,
    dataset: &mut Dataset,
    to_graph_name: impl Into<GraphName>,
) -> Result<()> {
    dataset.extend(
        &RdfParser::from_format(guess_rdf_format(url)?)
            .with_base_iri(url)?
            .with_default_graph(to_graph_name)
            .rename_blank_nodes()
            .for_reader(read_file(url)?)
            .collect::<Result<Vec<_>, _>>()?,
    );
    Ok(())
}

fn evaluate_query_optimization_test(test: &Test) -> Result<()> {
    let action = test.action.as_deref().context("No action found")?;
    let actual = (&Optimizer::optimize_graph_pattern(
        (&if let Query::Select { pattern, .. } = SparqlParser::new()
            .with_base_iri(action)?
            .parse_query(&read_file_to_string(action)?)?
        {
            pattern
        } else {
            bail!("Only SELECT queries are supported in query sparql-optimization tests")
        })
            .into(),
    ))
        .into();
    let result = test.result.as_ref().context("No tests result found")?;
    let Query::Select {
        pattern: expected, ..
    } = SparqlParser::new()
        .with_base_iri(result)?
        .parse_query(&read_file_to_string(result)?)?
    else {
        bail!("Only SELECT queries are supported in query sparql-optimization tests")
    };
    ensure!(
        expected == actual,
        "Not equal queries.\nDiff:\n{}\n",
        format_diff(
            &Query::Select {
                pattern: expected,
                dataset: None,
                base_iri: None
            }
            .to_sse(),
            &Query::Select {
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
