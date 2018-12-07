///! Integration tests based on [SPARQL 1.1 Test Cases](https://www.w3.org/2009/sparql/docs/tests/README.html)
#[macro_use]
extern crate lazy_static;
extern crate reqwest;
extern crate rudf;
extern crate url;
#[macro_use]
extern crate failure;

use reqwest::Client;
use reqwest::Response;
use rudf::model::vocab::rdf;
use rudf::model::vocab::rdfs;
use rudf::model::*;
use rudf::rio::turtle::read_turtle;
use rudf::rio::xml::read_rdf_xml;
use rudf::sparql::algebra::Query;
use rudf::sparql::algebra::QueryResult;
use rudf::sparql::parser::read_sparql_query;
use rudf::sparql::xml_results::read_xml_results;
use rudf::sparql::PreparedQuery;
use rudf::sparql::SparqlDataset;
use rudf::store::isomorphism::GraphIsomorphism;
use rudf::store::MemoryDataset;
use rudf::store::MemoryGraph;
use rudf::Result;
use std::error::Error;
use std::fmt;
use std::io::BufReader;
use std::str::FromStr;
use url::Url;

#[test]
fn sparql_w3c_syntax_testsuite() {
    let manifest_10_url =
        Url::parse("https://www.w3.org/2001/sw/DataAccess/tests/data-r2/manifest-syntax.ttl")
            .unwrap();
    let manifest_11_url = Url::parse(
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/syntax-query/manifest.ttl",
    )
    .unwrap();
    let test_blacklist = vec![
        NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql2/manifest#syntax-form-construct02").unwrap(),
        //TODO: Deserialization of the serialization failing:
        NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql2/manifest#syntax-form-construct04").unwrap(),
        NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql2/manifest#syntax-function-04").unwrap(),
        NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql1/manifest#syntax-qname-04").unwrap(),
    ];
    let client = RDFClient::default();

    for test_result in TestManifest::new(&client, manifest_10_url)
        .chain(TestManifest::new(&client, manifest_11_url))
    {
        let test = test_result.unwrap();
        if test_blacklist.contains(&test.id) {
            continue;
        }
        if test.kind == "PositiveSyntaxTest" || test.kind == "PositiveSyntaxTest11" {
            match client.load_sparql_query(test.query.clone()) {
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
            if let Ok(result) = client.load_sparql_query(test.query.clone()) {
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
        Url::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/algebra/manifest.ttl")
            .unwrap(),
        Url::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/ask/manifest.ttl")
            .unwrap(),
        Url::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/basic/manifest.ttl")
            .unwrap(),
        Url::parse(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/bnode-coreference/manifest.ttl",
        ).unwrap(),
        Url::parse(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/boolean-effective-value/manifest.ttl",
        ).unwrap(),
        Url::parse(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/bound/manifest.ttl",
        ).unwrap(),
        Url::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/cast/manifest.ttl").unwrap(),
        Url::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/construct/manifest.ttl")
            .unwrap(),
        Url::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/distinct/manifest.ttl")
            .unwrap(),
        Url::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest.ttl")
            .unwrap(),
        Url::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-equals/manifest.ttl")
            .unwrap(),
        Url::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-ops/manifest.ttl")
            .unwrap(),
        Url::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/graph/manifest.ttl")
            .unwrap(),
        Url::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/i18n/manifest.ttl").unwrap(),
        Url::parse(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/optional-filter/manifest.ttl",
        ).unwrap(),
        Url::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/optional/manifest.ttl")
            .unwrap(),
        Url::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/reduced/manifest.ttl")
            .unwrap(),
        Url::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/regex/manifest.ttl")
            .unwrap(),
        Url::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/solution-seq/manifest.ttl")
            .unwrap(),
        Url::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/sort/manifest.ttl").unwrap(),
        Url::parse("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/triple-match/manifest.ttl")
            .unwrap(),
        Url::parse(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/type-promotion/manifest.ttl",
        ).unwrap(),
    ];
    let test_blacklist = vec![
        //Multiple writing of the same xsd:integer. Our system does strong normalization.
        NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/distinct/manifest#distinct-1",
        ).unwrap(),
        NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/distinct/manifest#distinct-9",
        ).unwrap(),
        NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-str-1",
        ).unwrap(),
        NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-str-2",
        ).unwrap(),
        NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-equals/manifest#eq-graph-1",
        ).unwrap(),
        NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-equals/manifest#eq-graph-2",
        ).unwrap(),
        //Multiple writing of the same xsd:double. Our system does strong normalization.
        NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm",
        ).unwrap(),
        NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm-simple",
        ).unwrap(),
        NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm-eq",
        ).unwrap(),
        NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#sameTerm-not-eq",
        ).unwrap(),
        //URI normalization: we are normalizing more strongly
        NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/i18n/manifest#normalization-3",
        ).unwrap(),
        //Test on curly brace scoping with OPTIONAL filter
        NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/optional-filter/manifest#dawg-optional-filter-005-not-simplified",
        ).unwrap(),
        //Case insensitive language tag comparison
        NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#lang-case-insensitive-eq",
        ).unwrap(),
        NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#lang-case-insensitive-ne",
        ).unwrap(),
        NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-lang-3",
        ).unwrap(),
        //Difference in language matching
        NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-langMatches-basic",
        ).unwrap(),
        NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-langMatches-basic",
        ).unwrap(),
        //DATATYPE("foo"@en) returns rdf:langString in SPARQL 1.1
        NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/expr-builtin/manifest#dawg-datatype-2",
        ).unwrap(),
    ];
    let client = RDFClient::default();

    for test_result in manifest_10_urls
        .into_iter()
        .flat_map(|manifest| TestManifest::new(&client, manifest))
    {
        let test = test_result.unwrap();
        if test_blacklist.contains(&test.id) {
            continue;
        }
        if test.kind == "QueryEvaluationTest" {
            let data = match &test.data {
                Some(data) => {
                    let dataset = MemoryDataset::default();
                    let dataset_default = dataset.default_graph();
                    client
                        .load_graph(data.clone())
                        .unwrap()
                        .iter()
                        .unwrap()
                        .for_each(|triple| dataset_default.insert(&triple.unwrap()).unwrap());
                    dataset
                }
                None => MemoryDataset::default(),
            };
            for graph_data in &test.graph_data {
                let named_graph = data
                    .named_graph(&NamedNode::from(graph_data.clone()).into())
                    .unwrap();
                client
                    .load_graph(graph_data.clone())
                    .unwrap()
                    .iter()
                    .unwrap()
                    .for_each(|triple| named_graph.insert(&triple.unwrap()).unwrap());
            }
            match data.prepare_query(client.get(&test.query).unwrap()) {
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
                        let expected_graph = client
                            .load_sparql_query_result_graph(test.result.clone().unwrap())
                            .unwrap();
                        let with_order = expected_graph
                            .triples_for_predicate(&rs::INDEX)
                            .unwrap()
                            .next()
                            .is_some();
                        let actual_graph = to_graph(result, with_order).unwrap();
                        assert!(
                            actual_graph.is_isomorphic(&expected_graph).unwrap(),
                            "Failure on {}.\nExpected file:\n{}\nOutput file:\n{}\nParsed query:\n{}\nData:\n{}\n",
                            test,
                            expected_graph,
                            actual_graph,
                            client.load_sparql_query(test.query.clone()).unwrap(),
                            data
                        )
                    }
                },
            }
        } else {
            assert!(false, "Not supported test: {}", test);
        }
    }
}

pub struct RDFClient {
    client: Client,
}

impl Default for RDFClient {
    fn default() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl RDFClient {
    fn load_graph(&self, url: Url) -> Result<MemoryGraph> {
        if url.as_str().ends_with(".ttl") {
            Ok(read_turtle(self.get(&url)?, Some(url))?.collect())
        } else if url.as_str().ends_with(".rdf") {
            read_rdf_xml(BufReader::new(self.get(&url)?), Some(url)).collect()
        } else {
            Err(format_err!("Serialization type not found for {}", url))
        }
    }

    fn load_sparql_query(&self, url: Url) -> Result<Query> {
        read_sparql_query(self.get(&url)?, Some(url))
    }

    fn load_sparql_query_result_graph(&self, url: Url) -> Result<MemoryGraph> {
        if url.as_str().ends_with(".srx") {
            to_graph(read_xml_results(BufReader::new(self.get(&url)?))?, false)
        } else {
            self.load_graph(url)
        }
    }

    fn get(&self, url: &Url) -> Result<Response> {
        match self.client.get(url.clone()).send() {
            Ok(response) => Ok(response),
            Err(error) => {
                if error.description() == "parsed HTTP message from remote is incomplete" {
                    self.get(url)
                } else {
                    Err(format_err!("HTTP request error: {}", error.description()))
                }
            }
        }
    }
}

mod rs {
    use rudf::model::NamedNode;
    use std::str::FromStr;

    lazy_static! {
        pub static ref RESULT_SET: NamedNode =
            NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/result-set#ResultSet")
                .unwrap();
        pub static ref RESULT_VARIABLE: NamedNode = NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/result-set#resultVariable"
        )
        .unwrap();
        pub static ref SOLUTION: NamedNode =
            NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/result-set#solution")
                .unwrap();
        pub static ref BINDING: NamedNode =
            NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/result-set#binding")
                .unwrap();
        pub static ref VALUE: NamedNode =
            NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/result-set#value")
                .unwrap();
        pub static ref VARIABLE: NamedNode =
            NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/result-set#variable")
                .unwrap();
        pub static ref INDEX: NamedNode =
            NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/result-set#index")
                .unwrap();
        pub static ref BOOLEAN: NamedNode =
            NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/result-set#boolean")
                .unwrap();
    }
}

fn to_graph(result: QueryResult, with_order: bool) -> Result<MemoryGraph> {
    match result {
        QueryResult::Graph(graph) => graph.collect(),
        QueryResult::Boolean(value) => {
            let graph = MemoryGraph::default();
            let result_set = BlankNode::default();
            graph.insert(&Triple::new(
                result_set.clone(),
                rdf::TYPE.clone(),
                rs::RESULT_SET.clone(),
            ))?;
            graph.insert(&Triple::new(
                result_set.clone(),
                rs::BOOLEAN.clone(),
                Literal::from(value),
            ))?;
            Ok(graph)
        }
        QueryResult::Bindings(bindings) => {
            let graph = MemoryGraph::default();
            let result_set = BlankNode::default();
            graph.insert(&Triple::new(
                result_set.clone(),
                rdf::TYPE.clone(),
                rs::RESULT_SET.clone(),
            ))?;
            let (variables, iter) = bindings.destruct();
            for variable in &variables {
                graph.insert(&Triple::new(
                    result_set.clone(),
                    rs::RESULT_VARIABLE.clone(),
                    Literal::new_simple_literal(variable.name()?),
                ))?;
            }
            for (i, binding_values) in iter.enumerate() {
                let binding_values = binding_values?;
                let solution = BlankNode::default();
                graph.insert(&Triple::new(
                    result_set.clone(),
                    rs::SOLUTION.clone(),
                    solution.clone(),
                ))?;
                for i in 0..variables.len() {
                    if let Some(ref value) = binding_values[i] {
                        let binding = BlankNode::default();
                        graph.insert(&Triple::new(
                            solution.clone(),
                            rs::BINDING.clone(),
                            binding.clone(),
                        ))?;
                        graph.insert(&Triple::new(
                            binding.clone(),
                            rs::VALUE.clone(),
                            value.clone(),
                        ))?;
                        graph.insert(&Triple::new(
                            binding.clone(),
                            rs::VARIABLE.clone(),
                            Literal::new_simple_literal(variables[i].name()?),
                        ))?;
                    }
                }
                if with_order {
                    graph.insert(&Triple::new(
                        solution.clone(),
                        rs::INDEX.clone(),
                        Literal::from((i + 1) as i128),
                    ))?;
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
    pub query: Url,
    pub data: Option<Url>,
    pub graph_data: Vec<Url>,
    pub result: Option<Url>,
}

impl fmt::Display for Test {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
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

pub struct TestManifest<'a> {
    client: &'a RDFClient,
    graph: MemoryGraph,
    tests_to_do: Vec<Term>,
    manifests_to_do: Vec<Url>,
}

impl<'a> TestManifest<'a> {
    pub fn new(client: &'a RDFClient, url: Url) -> TestManifest<'a> {
        Self {
            client,
            graph: MemoryGraph::default(),
            tests_to_do: Vec::default(),
            manifests_to_do: vec![url],
        }
    }
}

pub mod mf {
    use rudf::model::NamedNode;
    use std::str::FromStr;

    lazy_static! {
        pub static ref INCLUDE: NamedNode =
            NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#include")
                .unwrap();
        pub static ref ENTRIES: NamedNode =
            NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#entries")
                .unwrap();
        pub static ref NAME: NamedNode =
            NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#name")
                .unwrap();
        pub static ref ACTION: NamedNode =
            NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#action")
                .unwrap();
        pub static ref RESULT: NamedNode =
            NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#result")
                .unwrap();
    }
}

pub mod qt {
    use rudf::model::NamedNode;
    use std::str::FromStr;

    lazy_static! {
        pub static ref QUERY: NamedNode =
            NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/test-query#query")
                .unwrap();
        pub static ref DATA: NamedNode =
            NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/test-query#data")
                .unwrap();
        pub static ref GRAPH_DATA: NamedNode =
            NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/test-query#graphData")
                .unwrap();
    }
}

impl<'a> Iterator for TestManifest<'a> {
    type Item = Result<Test>;

    fn next(&mut self) -> Option<Result<Test>> {
        match self.tests_to_do.pop() {
            Some(Term::NamedNode(test_node)) => {
                let test_subject = NamedOrBlankNode::from(test_node.clone());
                let kind = match self
                    .graph
                    .object_for_subject_predicate(&test_subject, &rdf::TYPE)
                    .unwrap()
                {
                    Some(Term::NamedNode(c)) => match c.as_str().split("#").last() {
                        Some(k) => k.to_string(),
                        None => return Some(Err(format_err!("no type"))),
                    },
                    _ => return Some(Err(format_err!("no type"))),
                };
                let name = match self
                    .graph
                    .object_for_subject_predicate(&test_subject, &mf::NAME)
                    .unwrap()
                {
                    Some(Term::Literal(c)) => Some(c.value().to_string()),
                    _ => None,
                };
                let comment = match self
                    .graph
                    .object_for_subject_predicate(&test_subject, &rdfs::COMMENT)
                    .unwrap()
                {
                    Some(Term::Literal(c)) => Some(c.value().to_string()),
                    _ => None,
                };
                let (query, data, graph_data) = match self
                    .graph
                    .object_for_subject_predicate(&test_subject, &*mf::ACTION)
                    .unwrap()
                {
                    Some(Term::NamedNode(n)) => (n.into(), None, vec![]),
                    Some(Term::BlankNode(n)) => {
                        let n = n.into();
                        let query = match self
                            .graph
                            .object_for_subject_predicate(&n, &qt::QUERY)
                            .unwrap()
                        {
                            Some(Term::NamedNode(q)) => q.into(),
                            Some(_) => return Some(Err(format_err!("invalid query"))),
                            None => return Some(Err(format_err!("query not found"))),
                        };
                        let data = match self
                            .graph
                            .object_for_subject_predicate(&n, &qt::DATA)
                            .unwrap()
                        {
                            Some(Term::NamedNode(q)) => Some(q.into()),
                            _ => None,
                        };
                        let graph_data = self
                            .graph
                            .objects_for_subject_predicate(&n, &qt::GRAPH_DATA)
                            .unwrap()
                            .filter_map(|g| match g {
                                Ok(Term::NamedNode(q)) => Some(q.into()),
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
                        )))
                    }
                };
                let result = match self
                    .graph
                    .object_for_subject_predicate(&test_subject, &*mf::RESULT)
                    .unwrap()
                {
                    Some(Term::NamedNode(n)) => Some(n.into()),
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
                        match self.client.load_graph(url) {
                            Ok(g) => g
                                .iter()
                                .unwrap()
                                .for_each(|g| self.graph.insert(&g.unwrap()).unwrap()),
                            Err(e) => return Some(Err(e.into())),
                        }

                        // New manifests
                        match self
                            .graph
                            .object_for_subject_predicate(&manifest, &*mf::INCLUDE)
                            .unwrap()
                        {
                            Some(Term::BlankNode(list)) => {
                                self.manifests_to_do.extend(
                                    RdfListIterator::iter(&self.graph, list.clone().into())
                                        .filter_map(|m| match m {
                                            Term::NamedNode(nm) => Some(nm.as_url().clone()),
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
                            .unwrap()
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

pub struct RdfListIterator<'a, G: 'a + Graph> {
    graph: &'a G,
    current_node: Option<NamedOrBlankNode>,
}

impl<'a, G: 'a + Graph> RdfListIterator<'a, G> {
    fn iter(graph: &'a G, root: NamedOrBlankNode) -> RdfListIterator<'a, G> {
        RdfListIterator {
            graph,
            current_node: Some(root),
        }
    }
}

impl<'a, G: 'a + Graph> Iterator for RdfListIterator<'a, G> {
    type Item = Term;

    fn next(&mut self) -> Option<Term> {
        match self.current_node.clone() {
            Some(current) => {
                let result = self
                    .graph
                    .object_for_subject_predicate(&current, &rdf::FIRST)
                    .unwrap()?
                    .clone();
                self.current_node = match self
                    .graph
                    .object_for_subject_predicate(&current, &rdf::REST)
                    .unwrap()
                {
                    Some(Term::NamedNode(ref n)) if *n == *rdf::NIL => None,
                    Some(Term::NamedNode(n)) => Some(n.clone().into()),
                    Some(Term::BlankNode(n)) => Some(n.clone().into()),
                    _ => None,
                };
                Some(result)
            }
            None => None,
        }
    }
}
