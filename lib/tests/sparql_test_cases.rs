///! Integration tests based on [SPARQL 1.1 Test Cases](https://www.w3.org/2009/sparql/docs/tests/README.html)
use failure::format_err;
use rudf::model::vocab::rdf;
use rudf::model::vocab::rdfs;
use rudf::model::*;
use rudf::sparql::algebra::Query;
use rudf::sparql::algebra::QueryResult;
use rudf::sparql::parser::read_sparql_query;
use rudf::sparql::xml_results::read_xml_results;
use rudf::sparql::PreparedQuery;
use rudf::{GraphSyntax, MemoryRepository, Repository, RepositoryConnection, Result};
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

#[test]
fn sparql_w3c_syntax_testsuite() {
    let manifest_10_url = "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/manifest-syntax.ttl";
    let manifest_11_url =
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/syntax-query/manifest.ttl";
    let test_blacklist = vec![
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql2/manifest#syntax-form-construct02"),
        //TODO: Deserialization of the serialization failing:
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql2/manifest#syntax-form-construct04"),
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql2/manifest#syntax-function-04"),
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql1/manifest#syntax-qname-04"),
    ];

    for test_result in TestManifest::new(manifest_10_url).chain(TestManifest::new(manifest_11_url))
    {
        let test = test_result.unwrap();
        if test_blacklist.contains(&test.id) {
            continue;
        }
        if test.kind == "PositiveSyntaxTest" || test.kind == "PositiveSyntaxTest11" {
            match load_sparql_query(&test.query) {
                Err(error) => assert!(false, "Failure on {} with error: {}", test, error),
                Ok(query) => {
                    if let Err(error) = read_sparql_query(query.to_string().as_bytes(), None) {
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
            if let Ok(result) = load_sparql_query(&test.query) {
                eprintln!("Failure on {}. The output tree is: {}", test, result);
            }
        } else {
            assert!(false, "Not supported test: {}", test);
        }
    }
}

#[test]
fn sparql_w3c_query_evaluation_testsuite() {
    //TODO: dataset open-world
    let manifest_10_urls = vec![
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/algebra/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/ask/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/basic/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/bnode-coreference/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/boolean-effective-value/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/bound/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/cast/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/construct/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/distinct/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-equals/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-ops/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/graph/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/i18n/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/optional/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/reduced/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/regex/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/solution-seq/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/sort/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/triple-match/manifest.ttl",
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/type-promotion/manifest.ttl",
    ];
    let test_blacklist = vec![
        //Multiple writing of the same xsd:integer. Our system does strong normalization.
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/distinct/manifest#distinct-1"),
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/distinct/manifest#distinct-9"),
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-str-1"),
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-str-2"),
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-equals/manifest#eq-graph-1"),
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-equals/manifest#eq-graph-2"),
        //Multiple writing of the same xsd:double. Our system does strong normalization.
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm"),
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm-simple"),
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm-eq"),
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm-not-eq"),
        //Simple literal vs xsd:string. We apply RDF 1.1
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/distinct/manifest#distinct-2"),
        //URI normalization: we are not normalizing well
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/i18n/manifest#normalization-1"),
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/i18n/manifest#normalization-2"),
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/i18n/manifest#normalization-3"),
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/i18n/manifest#kanji-1"),
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/i18n/manifest#kanji-2"),
        //Test on curly brace scoping with OPTIONAL filter
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/optional-filter/manifest#dawg-optional-filter-005-not-simplified"),
        //DATATYPE("foo"@en) returns rdf:langString in SPARQL 1.1
        NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-datatype-2")
    ];

    for test_result in manifest_10_urls
        .into_iter()
        .flat_map(|manifest| TestManifest::new(manifest))
    {
        let test = test_result.unwrap();
        if test_blacklist.contains(&test.id) {
            continue;
        }
        if test.kind == "QueryEvaluationTest" {
            let repository = MemoryRepository::default();
            if let Some(data) = &test.data {
                load_graph_to_repository(&data, &repository.connection().unwrap(), None).unwrap();
            }
            for graph_data in &test.graph_data {
                load_graph_to_repository(
                    &graph_data,
                    &repository.connection().unwrap(),
                    Some(&NamedNode::new(graph_data).into()),
                )
                .unwrap();
            }
            match repository
                .connection()
                .unwrap()
                .prepare_query(read_file(&test.query).unwrap())
            {
                Err(error) => assert!(
                    false,
                    "Failure to parse query of {} with error: {}",
                    test, error
                ),
                Ok(query) => match query.exec() {
                    Err(error) => assert!(
                        false,
                        "Failure to execute query of {} with error: {}",
                        test, error
                    ),
                    Ok(result) => {
                        let expected_graph =
                            load_sparql_query_result_graph(test.result.as_ref().unwrap()).unwrap();
                        let with_order = expected_graph
                            .triples_for_predicate(&rs::INDEX)
                            .next()
                            .is_some();
                        let actual_graph = to_graph(result, with_order).unwrap();
                        assert!(
                                actual_graph.is_isomorphic(&expected_graph),
                                "Failure on {}.\nExpected file:\n{}\nOutput file:\n{}\nParsed query:\n{}\nData:\n{}\n",
                                test,
                                expected_graph,
                                actual_graph,
                                load_sparql_query(&test.query).unwrap(),
                                repository_to_string(&repository)
                            )
                    }
                },
            }
        } else {
            assert!(false, "Not supported test: {}", test);
        }
    }
}

fn repository_to_string(repository: impl Repository) -> String {
    repository
        .connection()
        .unwrap()
        .quads_for_pattern(None, None, None, None)
        .map(|q| q.unwrap().to_string() + "\n")
        .collect()
}

fn load_graph(url: &str) -> Result<SimpleGraph> {
    let repository = MemoryRepository::default();
    load_graph_to_repository(url, &repository.connection().unwrap(), None)?;
    Ok(repository
        .connection()
        .unwrap()
        .quads_for_pattern(None, None, None, Some(None))
        .map(|q| q.unwrap().into_triple())
        .collect())
}

fn load_graph_to_repository(
    url: &str,
    connection: &<&MemoryRepository as Repository>::Connection,
    to_graph_name: Option<&NamedOrBlankNode>,
) -> Result<()> {
    let syntax = if url.ends_with(".ttl") {
        GraphSyntax::Turtle
    } else if url.ends_with(".rdf") {
        GraphSyntax::RdfXml
    } else {
        return Err(format_err!("Serialization type not found for {}", url));
    };
    connection.load_graph(read_file(url)?, syntax, to_graph_name, Some(url))
}

fn load_sparql_query(url: &str) -> Result<Query> {
    read_sparql_query(read_file(url)?, Some(url))
}

fn load_sparql_query_result_graph(url: &str) -> Result<SimpleGraph> {
    if url.ends_with(".srx") {
        to_graph(read_xml_results(read_file(url)?)?, false)
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
    } else {
        Err(format_err!("Not supported url for file: {}", url))
    }
}

fn read_file(url: &str) -> Result<impl BufRead> {
    let mut base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base_path.push("tests");
    base_path.push(to_relative_path(url)?);

    Ok(BufReader::new(File::open(&base_path).map_err(|e| {
        format_err!("Opening file {} failed with {}", base_path.display(), e)
    })?))
}

mod rs {
    use lazy_static::lazy_static;
    use rudf::model::NamedNode;

    lazy_static! {
        pub static ref RESULT_SET: NamedNode =
            NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/result-set#ResultSet");
        pub static ref RESULT_VARIABLE: NamedNode =
            NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/result-set#resultVariable");
        pub static ref SOLUTION: NamedNode =
            NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/result-set#solution");
        pub static ref BINDING: NamedNode =
            NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/result-set#binding");
        pub static ref VALUE: NamedNode =
            NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/result-set#value");
        pub static ref VARIABLE: NamedNode =
            NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/result-set#variable");
        pub static ref INDEX: NamedNode =
            NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/result-set#index");
        pub static ref BOOLEAN: NamedNode =
            NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/result-set#boolean");
    }
}

fn to_graph(result: QueryResult<'_>, with_order: bool) -> Result<SimpleGraph> {
    match result {
        QueryResult::Graph(graph) => graph.collect(),
        QueryResult::Boolean(value) => {
            let mut graph = SimpleGraph::default();
            let result_set = BlankNode::default();
            graph.insert(Triple::new(
                result_set.clone(),
                rdf::TYPE.clone(),
                rs::RESULT_SET.clone(),
            ));
            graph.insert(Triple::new(
                result_set.clone(),
                rs::BOOLEAN.clone(),
                Literal::from(value),
            ));
            Ok(graph)
        }
        QueryResult::Bindings(bindings) => {
            let mut graph = SimpleGraph::default();
            let result_set = BlankNode::default();
            graph.insert(Triple::new(
                result_set.clone(),
                rdf::TYPE.clone(),
                rs::RESULT_SET.clone(),
            ));
            let (variables, iter) = bindings.destruct();
            for variable in &variables {
                graph.insert(Triple::new(
                    result_set.clone(),
                    rs::RESULT_VARIABLE.clone(),
                    Literal::new_simple_literal(variable.name()?),
                ));
            }
            for (i, binding_values) in iter.enumerate() {
                let binding_values = binding_values?;
                let solution = BlankNode::default();
                graph.insert(Triple::new(
                    result_set.clone(),
                    rs::SOLUTION.clone(),
                    solution.clone(),
                ));
                for i in 0..variables.len() {
                    if let Some(ref value) = binding_values[i] {
                        let binding = BlankNode::default();
                        graph.insert(Triple::new(
                            solution.clone(),
                            rs::BINDING.clone(),
                            binding.clone(),
                        ));
                        graph.insert(Triple::new(
                            binding.clone(),
                            rs::VALUE.clone(),
                            value.clone(),
                        ));
                        graph.insert(Triple::new(
                            binding.clone(),
                            rs::VARIABLE.clone(),
                            Literal::new_simple_literal(variables[i].name()?),
                        ));
                    }
                }
                if with_order {
                    graph.insert(Triple::new(
                        solution.clone(),
                        rs::INDEX.clone(),
                        Literal::from((i + 1) as i128),
                    ));
                }
            }
            Ok(graph)
        }
    }
}

pub struct Test {
    pub id: NamedNode,
    pub kind: String,
    pub name: Option<String>,
    pub comment: Option<String>,
    pub query: String,
    pub data: Option<String>,
    pub graph_data: Vec<String>,
    pub result: Option<String>,
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

pub struct TestManifest {
    graph: SimpleGraph,
    tests_to_do: Vec<Term>,
    manifests_to_do: Vec<String>,
}

impl TestManifest {
    pub fn new(url: impl Into<String>) -> TestManifest {
        Self {
            graph: SimpleGraph::default(),
            tests_to_do: Vec::default(),
            manifests_to_do: vec![url.into()],
        }
    }
}

pub mod mf {
    use lazy_static::lazy_static;
    use rudf::model::NamedNode;

    lazy_static! {
        pub static ref INCLUDE: NamedNode =
            NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#include");
        pub static ref ENTRIES: NamedNode =
            NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#entries");
        pub static ref NAME: NamedNode =
            NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#name");
        pub static ref ACTION: NamedNode =
            NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#action");
        pub static ref RESULT: NamedNode =
            NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#result");
    }
}

pub mod qt {
    use lazy_static::lazy_static;
    use rudf::model::NamedNode;

    lazy_static! {
        pub static ref QUERY: NamedNode =
            NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/test-query#query");
        pub static ref DATA: NamedNode =
            NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/test-query#data");
        pub static ref GRAPH_DATA: NamedNode =
            NamedNode::new("http://www.w3.org/2001/sw/DataAccess/tests/test-query#graphData");
    }
}

impl Iterator for TestManifest {
    type Item = Result<Test>;

    fn next(&mut self) -> Option<Result<Test>> {
        match self.tests_to_do.pop() {
            Some(Term::NamedNode(test_node)) => {
                let test_subject = NamedOrBlankNode::from(test_node.clone());
                let kind = match self
                    .graph
                    .object_for_subject_predicate(&test_subject, &rdf::TYPE)
                {
                    Some(Term::NamedNode(c)) => match c.as_str().split("#").last() {
                        Some(k) => k.to_string(),
                        None => return self.next(), //We ignore the test
                    },
                    _ => return self.next(), //We ignore the test
                };
                let name = match self
                    .graph
                    .object_for_subject_predicate(&test_subject, &mf::NAME)
                {
                    Some(Term::Literal(c)) => Some(c.value().to_string()),
                    _ => None,
                };
                let comment = match self
                    .graph
                    .object_for_subject_predicate(&test_subject, &rdfs::COMMENT)
                {
                    Some(Term::Literal(c)) => Some(c.value().to_string()),
                    _ => None,
                };
                let (query, data, graph_data) = match self
                    .graph
                    .object_for_subject_predicate(&test_subject, &*mf::ACTION)
                {
                    Some(Term::NamedNode(n)) => (n.as_str().to_string(), None, vec![]),
                    Some(Term::BlankNode(n)) => {
                        let n = n.clone().into();
                        let query = match self.graph.object_for_subject_predicate(&n, &qt::QUERY) {
                            Some(Term::NamedNode(q)) => q.as_str().to_string(),
                            Some(_) => return Some(Err(format_err!("invalid query"))),
                            None => return Some(Err(format_err!("query not found"))),
                        };
                        let data = match self.graph.object_for_subject_predicate(&n, &qt::DATA) {
                            Some(Term::NamedNode(q)) => Some(q.as_str().to_string()),
                            _ => None,
                        };
                        let graph_data = self
                            .graph
                            .objects_for_subject_predicate(&n, &qt::GRAPH_DATA)
                            .filter_map(|g| match g {
                                Term::NamedNode(q) => Some(q.as_str().to_string()),
                                _ => None,
                            })
                            .collect();
                        (query, data, graph_data)
                    }
                    Some(_) => return Some(Err(format_err!("invalid action"))),
                    None => {
                        return Some(Err(format_err!(
                            "action not found for test {}",
                            test_subject
                        )));
                    }
                };
                let result = match self
                    .graph
                    .object_for_subject_predicate(&test_subject, &*mf::RESULT)
                {
                    Some(Term::NamedNode(n)) => Some(n.as_str().to_string()),
                    Some(_) => return Some(Err(format_err!("invalid result"))),
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
                    result,
                }))
            }
            Some(_) => Some(Err(format_err!("invalid test list"))),
            None => {
                match self.manifests_to_do.pop() {
                    Some(url) => {
                        let manifest = NamedOrBlankNode::from(NamedNode::new(url.clone()));
                        match load_graph(&url) {
                            Ok(g) => self.graph.extend(g.into_iter()),
                            Err(e) => return Some(Err(e.into())),
                        }

                        // New manifests
                        match self
                            .graph
                            .object_for_subject_predicate(&manifest, &*mf::INCLUDE)
                        {
                            Some(Term::BlankNode(list)) => {
                                self.manifests_to_do.extend(
                                    RdfListIterator::iter(&self.graph, list.clone().into())
                                        .filter_map(|m| match m {
                                            Term::NamedNode(nm) => Some(nm.as_str().to_string()),
                                            _ => None,
                                        }),
                                );
                            }
                            Some(_) => return Some(Err(format_err!("invalid tests list"))),
                            None => (),
                        }

                        // New tests
                        match self
                            .graph
                            .object_for_subject_predicate(&manifest, &*mf::ENTRIES)
                        {
                            Some(Term::BlankNode(list)) => {
                                self.tests_to_do.extend(RdfListIterator::iter(
                                    &self.graph,
                                    list.clone().into(),
                                ));
                            }
                            Some(term) => {
                                return Some(Err(format_err!(
                                    "Invalid tests list. Got term {}",
                                    term
                                )));
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

pub struct RdfListIterator<'a> {
    graph: &'a SimpleGraph,
    current_node: Option<NamedOrBlankNode>,
}

impl<'a> RdfListIterator<'a> {
    fn iter(graph: &'a SimpleGraph, root: NamedOrBlankNode) -> RdfListIterator<'a> {
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
                let result = self
                    .graph
                    .object_for_subject_predicate(&current, &rdf::FIRST);
                self.current_node = match self
                    .graph
                    .object_for_subject_predicate(&current, &rdf::REST)
                {
                    Some(Term::NamedNode(ref n)) if *n == *rdf::NIL => None,
                    Some(Term::NamedNode(n)) => Some(n.clone().into()),
                    Some(Term::BlankNode(n)) => Some(n.clone().into()),
                    _ => None,
                };
                result.cloned()
            }
            None => None,
        }
    }
}
