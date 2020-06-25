///! Integration tests based on [SPARQL 1.1 Test Cases](https://www.w3.org/2009/sparql/docs/tests/README.html)
use oxigraph::model::vocab::rdf;
use oxigraph::model::vocab::rdfs;
use oxigraph::model::*;
use oxigraph::sparql::*;
use oxigraph::*;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fmt;
use std::fs::File;
use std::io::Read;
use std::io::{BufRead, BufReader};
use std::iter::once;
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn sparql_w3c_syntax_testsuite() -> Result<()> {
    let manifest_10_urls =
        vec!["http://www.w3.org/2001/sw/DataAccess/tests/data-r2/manifest-syntax.ttl"];
    let manifest_11_urls = vec![
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/syntax-query/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/syntax-fed/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/construct/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/grouping/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/aggregates/manifest.ttl",
    ];
    for test_result in manifest_10_urls
        .into_iter()
        .chain(manifest_11_urls.into_iter())
        .chain(once(
            "https://github.com/oxigraph/oxigraph/tests/sparql/manifest.ttl",
        ))
        .flat_map(TestManifest::new)
    {
        let test = test_result.unwrap();
        if test.kind == "PositiveSyntaxTest" || test.kind == "PositiveSyntaxTest11" {
            match Query::parse(&read_file_to_string(&test.query)?, Some(&test.query)) {
                Err(error) => panic!("Failure on {} with error: {}", test, error),
                Ok(query) => {
                    if let Err(error) = Query::parse(&query.to_string(), None) {
                        assert!(
                            false,
                            "Failure to deserialize \"{}\" of {} with error: {}",
                            query.to_string(),
                            test,
                            error
                        )
                    }
                }
            }
        } else if test.kind == "NegativeSyntaxTest" || test.kind == "NegativeSyntaxTest11" {
            //TODO
            if let Ok(result) = Query::parse(&read_file_to_string(&test.query)?, Some(&test.query))
            {
                eprintln!("Failure on {}. The output tree is: {}", test, result);
            }
        } else if test.kind != "QueryEvaluationTest" {
            panic!("Not supported test: {}", test);
        }
    }
    Ok(())
}

#[test]
fn sparql_w3c_query_evaluation_testsuite() -> Result<()> {
    let manifest_10_urls = vec![
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/algebra/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/ask/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/basic/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/bnode-coreference/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/boolean-effective-value/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/bound/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/cast/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/construct/manifest.ttl",
        //TODO FROM and FROM NAMED "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/construct/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/distinct/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-equals/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-ops/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/graph/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/i18n/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/optional/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/optional-filter/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/reduced/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/regex/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/solution-seq/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/sort/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/triple-match/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/type-promotion/manifest.ttl",
    ];

    let manifest_11_urls = vec![
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/aggregates/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/bind/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/bindings/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/construct/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/exists/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/functions/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/grouping/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/negation/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/project-expression/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/property-path/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/service/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/subquery/manifest.ttl",
    ];

    let test_blacklist = vec![
        //Multiple writing of the same xsd:integer. Our system does strong normalization.
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/distinct/manifest#distinct-1").unwrap(),
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/distinct/manifest#distinct-9").unwrap(),
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-str-1").unwrap(),
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-str-2").unwrap(),
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-equals/manifest#eq-graph-1").unwrap(),
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-equals/manifest#eq-graph-2").unwrap(),
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#open-eq-01").unwrap(),
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#open-eq-04").unwrap(),
        //Multiple writing of the same xsd:double. Our system does strong normalization.
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm").unwrap(),
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm-simple").unwrap(),
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm-eq").unwrap(),
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm-not-eq").unwrap(),
        //Simple literal vs xsd:string. We apply RDF 1.1
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/distinct/manifest#distinct-2").unwrap(),
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#open-eq-08").unwrap(),
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#open-eq-10").unwrap(),
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#open-eq-11").unwrap(),
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#open-eq-12").unwrap(),
        //DATATYPE("foo"@en) returns rdf:langString in RDF 1.1
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-datatype-2").unwrap(),
        // FROM support
        NamedNode::parse("http://www.w3.org/2009/sparql/docs/tests/data-sparql11/construct/manifest#constructwhere04").unwrap(),
        //BNODE() scope is currently wrong
        NamedNode::parse("http://www.w3.org/2009/sparql/docs/tests/data-sparql11/functions/manifest#bnode01").unwrap(),
        //Property path with unbound graph name are not supported yet
        NamedNode::parse("http://www.w3.org/2009/sparql/docs/tests/data-sparql11/property-path/manifest#pp35").unwrap(),
        //SERVICE name from a BGP
        NamedNode::parse("http://www.w3.org/2009/sparql/docs/tests/data-sparql11/service/manifest#service5").unwrap(),
        // We use XSD 1.1 equality on dates
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/open-world/manifest#date-2").unwrap(),
        // We choose to simplify first the nested group patterns in OPTIONAL
        NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/optional-filter/manifest#dawg-optional-filter-005-not-simplified").unwrap(),
    ];

    let tests: Result<Vec<_>> = manifest_10_urls
        .into_iter()
        .chain(manifest_11_urls.into_iter())
        .flat_map(TestManifest::new)
        .collect();
    let failed: Vec<_> = tests?.into_par_iter().map(|test| {
        if test_blacklist.contains(&test.id) {
            Ok(())
        } else if test.kind == "QueryEvaluationTest" {
            let store = MemoryStore::new();
            if let Some(data) = &test.data {
                load_graph_to_store(&data, &store, None)?;
            }
            for graph_data in &test.graph_data {
                load_graph_to_store(
                    &graph_data,
                    &store,
                    Some(&NamedNode::parse(graph_data)?.into()),
                )?;
            }
            match store.prepare_query(&read_file_to_string(&test.query)?, QueryOptions::default().with_base_iri(&test.query).with_service_handler(StaticServiceHandler::new(&test.service_data)?))
                {
                    Err(error) => Err(Error::msg(format!(
                    "Failure to parse query of {} with error: {}",
                    test, error
                ))),
                    Ok(query) => match query.exec() {
                        Err(error) => Err(Error::msg(format!(
                        "Failure to execute query of {} with error: {}",
                        test, error
                    ))),
                        Ok(result) => {
                            let expected_graph =
                                load_sparql_query_result(test.result.as_ref().unwrap()).map_err(|e| Error::msg(format!("Error constructing expected graph for {}: {}", test, e)))?;
                            let with_order = expected_graph
                                .quads_for_pattern(None, Some(&rs::INDEX), None, None)
                                .next()
                                .is_some();
                            let actual_graph = to_dataset(result, with_order).map_err(|e| Error::msg(format!("Error constructing result graph for {}: {}", test, e)))?;
                            if actual_graph.is_isomorphic(&expected_graph) {
                                Ok(())
                            } else {
                                Err(Error::msg(format!("Failure on {}.\nExpected file:\n{}\nOutput file:\n{}\nParsed query:\n{}\nData:\n{}\n",
                                test,
                                expected_graph,
                                actual_graph,
                                Query::parse(&read_file_to_string(&test.query)?, Some(&test.query)).unwrap(),
                                store
                            )))
                            }
                        }
                    },
                }
        } else if test.kind != "NegativeSyntaxTest11" {
            panic!("Not supported test: {}", test)
        } else {
            Ok(())
        }
    }).filter_map(|v| v.err()).map(|e| e.to_string()).collect();
    assert!(
        failed.is_empty(),
        "{} tests failed:\n{}",
        failed.len(),
        failed.join("\n")
    );
    Ok(())
}

fn load_graph(url: &str) -> Result<MemoryStore> {
    let store = MemoryStore::new();
    load_graph_to_store(url, &store, None)?;
    Ok(store)
}

fn load_graph_to_store(
    url: &str,
    store: &MemoryStore,
    to_graph_name: Option<&NamedOrBlankNode>,
) -> Result<()> {
    let syntax = if url.ends_with(".nt") {
        GraphSyntax::NTriples
    } else if url.ends_with(".ttl") {
        GraphSyntax::Turtle
    } else if url.ends_with(".rdf") {
        GraphSyntax::RdfXml
    } else {
        return Err(Error::msg(format!(
            "Serialization type not found for {}",
            url
        )));
    };
    store.load_graph(read_file(url)?, syntax, to_graph_name, Some(url))
}

fn load_sparql_query_result(url: &str) -> Result<MemoryStore> {
    if url.ends_with(".srx") {
        to_dataset(
            QueryResult::read(read_file(url)?, QueryResultSyntax::Xml)?,
            false,
        )
    } else {
        load_graph(url)
    }
}

fn to_relative_path(url: &str) -> Result<String> {
    if url.starts_with("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/") {
        Ok(url.replace(
            "http://www.w3.org/2001/sw/DataAccess/tests/",
            "rdf-tests/sparql11/",
        ))
    } else if url.starts_with("http://www.w3.org/2009/sparql/docs/tests/data-sparql11/") {
        Ok(url.replace(
            "http://www.w3.org/2009/sparql/docs/tests/",
            "rdf-tests/sparql11/",
        ))
    } else if url.starts_with("https://github.com/oxigraph/oxigraph/tests/") {
        Ok(url.replace(
            "https://github.com/oxigraph/oxigraph/tests/",
            "oxigraph-tests/",
        ))
    } else {
        Err(Error::msg(format!("Not supported url for file: {}", url)))
    }
}

fn read_file(url: &str) -> Result<impl BufRead> {
    let mut base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base_path.push("tests");
    base_path.push(to_relative_path(url)?);

    Ok(BufReader::new(File::open(&base_path).map_err(|e| {
        Error::msg(format!(
            "Opening file {} failed with {}",
            base_path.display(),
            e,
        ))
    })?))
}

fn read_file_to_string(url: &str) -> Result<String> {
    let mut string = String::default();
    read_file(url)?.read_to_string(&mut string)?;
    Ok(string)
}

fn to_dataset(result: QueryResult<'_>, with_order: bool) -> Result<MemoryStore> {
    match result {
        QueryResult::Graph(graph) => graph.map(|t| t.map(|t| t.in_graph(None))).collect(),
        QueryResult::Boolean(value) => {
            let store = MemoryStore::new();
            let result_set = BlankNode::default();
            store.insert(Quad::new(
                result_set,
                rdf::TYPE.clone(),
                rs::RESULT_SET.clone(),
                None,
            ));
            store.insert(Quad::new(
                result_set,
                rs::BOOLEAN.clone(),
                Literal::from(value),
                None,
            ));
            Ok(store)
        }
        QueryResult::Bindings(solutions) => {
            let store = MemoryStore::new();
            let result_set = BlankNode::default();
            store.insert(Quad::new(
                result_set,
                rdf::TYPE.clone(),
                rs::RESULT_SET.clone(),
                None,
            ));
            for variable in solutions.variables() {
                store.insert(Quad::new(
                    result_set,
                    rs::RESULT_VARIABLE.clone(),
                    Literal::new_simple_literal(variable.as_str()),
                    None,
                ));
            }
            for (i, solution) in solutions.enumerate() {
                let solution = solution?;
                let solution_id = BlankNode::default();
                store.insert(Quad::new(
                    result_set,
                    rs::SOLUTION.clone(),
                    solution_id,
                    None,
                ));
                for (variable, value) in solution.iter() {
                    let binding = BlankNode::default();
                    store.insert(Quad::new(solution_id, rs::BINDING.clone(), binding, None));
                    store.insert(Quad::new(binding, rs::VALUE.clone(), value.clone(), None));
                    store.insert(Quad::new(
                        binding,
                        rs::VARIABLE.clone(),
                        Literal::new_simple_literal(variable.as_str()),
                        None,
                    ));
                }
                if with_order {
                    store.insert(Quad::new(
                        solution_id,
                        rs::INDEX.clone(),
                        Literal::from((i + 1) as i128),
                        None,
                    ));
                }
            }
            Ok(store)
        }
    }
}

mod rs {
    use lazy_static::lazy_static;
    use oxigraph::model::NamedNode;

    lazy_static! {
        pub static ref RESULT_SET: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#ResultSet")
                .unwrap();
        pub static ref RESULT_VARIABLE: NamedNode = NamedNode::parse(
            "http://www.w3.org/2001/sw/DataAccess/tests/result-set#resultVariable"
        )
        .unwrap();
        pub static ref SOLUTION: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#solution")
                .unwrap();
        pub static ref BINDING: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#binding")
                .unwrap();
        pub static ref VALUE: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#value")
                .unwrap();
        pub static ref VARIABLE: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#variable")
                .unwrap();
        pub static ref INDEX: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#index")
                .unwrap();
        pub static ref BOOLEAN: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#boolean")
                .unwrap();
    }
}

mod mf {
    use lazy_static::lazy_static;
    use oxigraph::model::NamedNode;

    lazy_static! {
        pub static ref INCLUDE: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#include")
                .unwrap();
        pub static ref ENTRIES: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#entries")
                .unwrap();
        pub static ref NAME: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#name")
                .unwrap();
        pub static ref ACTION: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#action")
                .unwrap();
        pub static ref RESULT: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#result")
                .unwrap();
    }
}

mod qt {
    use lazy_static::lazy_static;
    use oxigraph::model::NamedNode;

    lazy_static! {
        pub static ref QUERY: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-query#query")
                .unwrap();
        pub static ref DATA: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-query#data").unwrap();
        pub static ref GRAPH_DATA: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-query#graphData")
                .unwrap();
        pub static ref SERVICE_DATA: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-query#serviceData")
                .unwrap();
        pub static ref ENDPOINT: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-query#endpoint")
                .unwrap();
    }
}

struct Test {
    id: NamedNode,
    kind: String,
    name: Option<String>,
    comment: Option<String>,
    query: String,
    data: Option<String>,
    graph_data: Vec<String>,
    service_data: Vec<(String, String)>,
    result: Option<String>,
}

impl fmt::Display for Test {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)?;
        for name in &self.name {
            write!(f, " named \"{}\"", name)?;
        }
        for comment in &self.comment {
            write!(f, " with comment \"{}\"", comment)?;
        }
        write!(f, " on query {}", self.query)?;
        for data in &self.data {
            write!(f, " with data {}", data)?;
        }
        for data in &self.graph_data {
            write!(f, " and graph data {}", data)?;
        }
        for result in &self.result {
            write!(f, " and expected result {}", result)?;
        }
        Ok(())
    }
}

struct TestManifest {
    graph: MemoryStore,
    tests_to_do: Vec<Term>,
    manifests_to_do: Vec<String>,
}

impl TestManifest {
    fn new(url: impl Into<String>) -> TestManifest {
        Self {
            graph: MemoryStore::new(),
            tests_to_do: Vec::default(),
            manifests_to_do: vec![url.into()],
        }
    }
}

impl Iterator for TestManifest {
    type Item = Result<Test>;

    fn next(&mut self) -> Option<Result<Test>> {
        match self.tests_to_do.pop() {
            Some(Term::NamedNode(test_node)) => {
                let test_subject = NamedOrBlankNode::from(test_node.clone());
                let kind =
                    match object_for_subject_predicate(&self.graph, &test_subject, &rdf::TYPE) {
                        Some(Term::NamedNode(c)) => match c.as_str().split('#').last() {
                            Some(k) => k.to_string(),
                            None => return self.next(), //We ignore the test
                        },
                        _ => return self.next(), //We ignore the test
                    };
                let name = match object_for_subject_predicate(&self.graph, &test_subject, &mf::NAME)
                {
                    Some(Term::Literal(c)) => Some(c.value().to_string()),
                    _ => None,
                };
                let comment = match object_for_subject_predicate(
                    &self.graph,
                    &test_subject,
                    &rdfs::COMMENT,
                ) {
                    Some(Term::Literal(c)) => Some(c.value().to_string()),
                    _ => None,
                };
                let (query, data, graph_data, service_data) =
                    match object_for_subject_predicate(&self.graph, &test_subject, &*mf::ACTION) {
                        Some(Term::NamedNode(n)) => (n.as_str().to_owned(), None, vec![], vec![]),
                        Some(Term::BlankNode(n)) => {
                            let n = n.clone().into();
                            let query =
                                match object_for_subject_predicate(&self.graph, &n, &qt::QUERY) {
                                    Some(Term::NamedNode(q)) => q.as_str().to_owned(),
                                    Some(_) => return Some(Err(Error::msg("invalid query"))),
                                    None => return Some(Err(Error::msg("query not found"))),
                                };
                            let data =
                                match object_for_subject_predicate(&self.graph, &n, &qt::DATA) {
                                    Some(Term::NamedNode(q)) => Some(q.as_str().to_owned()),
                                    _ => None,
                                };
                            let graph_data =
                                objects_for_subject_predicate(&self.graph, &n, &qt::GRAPH_DATA)
                                    .filter_map(|g| match g {
                                        Term::NamedNode(q) => Some(q.as_str().to_owned()),
                                        _ => None,
                                    })
                                    .collect();
                            let service_data =
                                objects_for_subject_predicate(&self.graph, &n, &qt::SERVICE_DATA)
                                    .filter_map(|g| match g {
                                        Term::NamedNode(g) => Some(g.into()),
                                        Term::BlankNode(g) => Some(g.into()),
                                        _ => None,
                                    })
                                    .filter_map(|g| {
                                        if let (
                                            Some(Term::NamedNode(endpoint)),
                                            Some(Term::NamedNode(data)),
                                        ) = (
                                            object_for_subject_predicate(
                                                &self.graph,
                                                &g,
                                                &qt::ENDPOINT,
                                            ),
                                            object_for_subject_predicate(
                                                &self.graph,
                                                &g,
                                                &qt::DATA,
                                            ),
                                        ) {
                                            Some((
                                                endpoint.as_str().to_owned(),
                                                data.as_str().to_owned(),
                                            ))
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                            (query, data, graph_data, service_data)
                        }
                        Some(_) => return Some(Err(Error::msg("invalid action"))),
                        None => {
                            return Some(Err(Error::msg(format!(
                                "action not found for test {}",
                                test_subject
                            ))));
                        }
                    };
                let result =
                    match object_for_subject_predicate(&self.graph, &test_subject, &*mf::RESULT) {
                        Some(Term::NamedNode(n)) => Some(n.as_str().to_owned()),
                        Some(_) => return Some(Err(Error::msg("invalid result"))),
                        None => None,
                    };
                Some(Ok(Test {
                    id: test_node,
                    kind,
                    name,
                    comment,
                    query,
                    data,
                    graph_data,
                    service_data,
                    result,
                }))
            }
            Some(_) => Some(Err(Error::msg("invalid test list"))),
            None => {
                match self.manifests_to_do.pop() {
                    Some(url) => {
                        let manifest =
                            NamedOrBlankNode::from(NamedNode::parse(url.clone()).unwrap());
                        if let Err(e) = load_graph_to_store(&url, &self.graph, None) {
                            return Some(Err(e));
                        }
                        // New manifests
                        match object_for_subject_predicate(&self.graph, &manifest, &*mf::INCLUDE) {
                            Some(Term::BlankNode(list)) => {
                                self.manifests_to_do.extend(
                                    RdfListIterator::iter(&self.graph, list.clone().into())
                                        .filter_map(|m| match m {
                                            Term::NamedNode(nm) => Some(nm.into_string()),
                                            _ => None,
                                        }),
                                );
                            }
                            Some(_) => return Some(Err(Error::msg("invalid tests list"))),
                            None => (),
                        }

                        // New tests
                        match object_for_subject_predicate(&self.graph, &manifest, &*mf::ENTRIES) {
                            Some(Term::BlankNode(list)) => {
                                self.tests_to_do.extend(RdfListIterator::iter(
                                    &self.graph,
                                    list.clone().into(),
                                ));
                            }
                            Some(term) => {
                                return Some(Err(Error::msg(format!(
                                    "Invalid tests list. Got term {}",
                                    term
                                ))));
                            }
                            None => (),
                        }
                    }
                    None => return None,
                }
                self.next()
            }
        }
    }
}

struct RdfListIterator<'a> {
    graph: &'a MemoryStore,
    current_node: Option<NamedOrBlankNode>,
}

impl<'a> RdfListIterator<'a> {
    fn iter(graph: &'a MemoryStore, root: NamedOrBlankNode) -> RdfListIterator<'a> {
        RdfListIterator {
            graph,
            current_node: Some(root),
        }
    }
}

impl<'a> Iterator for RdfListIterator<'a> {
    type Item = Term;

    fn next(&mut self) -> Option<Term> {
        match self.current_node.clone() {
            Some(current) => {
                let result = object_for_subject_predicate(&self.graph, &current, &rdf::FIRST);
                self.current_node =
                    match object_for_subject_predicate(&self.graph, &current, &rdf::REST) {
                        Some(Term::NamedNode(ref n)) if *n == *rdf::NIL => None,
                        Some(Term::NamedNode(n)) => Some(n.into()),
                        Some(Term::BlankNode(n)) => Some(n.into()),
                        _ => None,
                    };
                result
            }
            None => None,
        }
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
                        let name = NamedNode::parse(name)?;
                        let store = MemoryStore::new();
                        load_graph_to_store(&data, &store, None)?;
                        Ok((name, store))
                    })
                    .collect::<Result<_>>()?,
            ),
        })
    }
}

impl ServiceHandler for StaticServiceHandler {
    fn handle<'a>(
        &'a self,
        service_name: &NamedNode,
        graph_pattern: &'a GraphPattern,
    ) -> Result<QuerySolutionsIterator<'a>> {
        if let QueryResult::Bindings(iterator) = self
            .services
            .get(service_name)
            .ok_or_else(|| Error::msg(format!("Service {} not found", service_name)))?
            .prepare_query_from_pattern(
                &graph_pattern,
                QueryOptions::default().with_service_handler(self.clone()),
            )?
            .exec()?
        {
            //TODO: very ugly
            let (variables, iter) = iterator.destruct();
            let collected = iter.collect::<Vec<_>>();
            Ok(QuerySolutionsIterator::new(
                variables,
                Box::new(collected.into_iter()),
            ))
        } else {
            Err(Error::msg("Expected bindings but got another QueryResult"))
        }
    }
}

fn object_for_subject_predicate(
    store: &MemoryStore,
    subject: &NamedOrBlankNode,
    predicate: &NamedNode,
) -> Option<Term> {
    objects_for_subject_predicate(store, subject, predicate).next()
}

fn objects_for_subject_predicate(
    store: &MemoryStore,
    subject: &NamedOrBlankNode,
    predicate: &NamedNode,
) -> impl Iterator<Item = Term> {
    store
        .quads_for_pattern(Some(subject), Some(predicate), None, None)
        .map(|t| t.object_owned())
}
