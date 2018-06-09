///! Integration tests based on [RDF 1.1 Test Cases](https://www.w3.org/TR/rdf11-testcases/)

#[macro_use]
extern crate lazy_static;
extern crate reqwest;
extern crate rudf;
extern crate url;

use reqwest::Client;
use reqwest::Response;
use rudf::model::vocab::rdf;
use rudf::model::vocab::rdfs;
use rudf::model::*;
use rudf::rio::ntriples::read_ntriples;
use rudf::rio::turtle::read_turtle;
use rudf::rio::RioError;
use rudf::rio::RioResult;
use rudf::sparql::ast::Query;
use rudf::sparql::parser::read_sparql_query;
use rudf::store::isomorphism::GraphIsomorphism;
use rudf::store::memory::MemoryGraph;
use std::error::Error;
use std::fmt;
use url::Url;

#[test]
fn turtle_w3c_testsuite() {
    let manifest_url = Url::parse("http://www.w3.org/2013/TurtleTests/manifest.ttl").unwrap();
    let client = RDFClient::default();
    //TODO: make blacklist pass
    let test_blacklist = vec![
        //UTF-8 broken surrogates in BNode ids
        NamedNode::new(
            manifest_url
                .join("#prefix_with_PN_CHARS_BASE_character_boundaries")
                .unwrap(),
        ),
        NamedNode::new(
            manifest_url
                .join("#labeled_blank_node_with_PN_CHARS_BASE_character_boundaries")
                .unwrap(),
        ),
        NamedNode::new(
            manifest_url
                .join("#localName_with_assigned_nfc_PN_CHARS_BASE_character_boundaries")
                .unwrap(),
        ),
        NamedNode::new(
            manifest_url
                .join("#localName_with_nfc_PN_CHARS_BASE_character_boundaries")
                .unwrap(),
        ),
    ];

    for test_result in TestManifest::new(&client, manifest_url) {
        let test = test_result.unwrap();
        if test_blacklist.contains(&test.id) {
            return;
        }
        if test.kind == "TestTurtlePositiveSyntax" {
            if let Err(error) = client.load_turtle(test.action.clone()) {
                assert!(false, "Failure on {} with error: {}", test, error)
            }
        } else if test.kind == "TestTurtleNegativeSyntax" {
            assert!(
                client.load_turtle(test.action.clone()).is_err(),
                "Failure on {}",
                test
            );
        } else if test.kind == "TestTurtleEval" {
            match client.load_turtle(test.action.clone()) {
                Ok(action_graph) => match client.load_turtle(test.result.clone().unwrap()) {
                    Ok(result_graph) => assert!(
                        action_graph.is_isomorphic(&result_graph),
                        "Failure on {}. Expected file:\n{}\nParsed file:\n{}\n",
                        test,
                        action_graph,
                        result_graph
                    ),
                    Err(error) => assert!(
                        false,
                        "Failure to parse the Turtle result file {} of {} with error: {}",
                        test.result.clone().unwrap(),
                        test,
                        error
                    ),
                },
                Err(error) => assert!(false, "Failure to parse {} with error: {}", test, error),
            }
        } else if test.kind == "TestTurtleNegativeEval" {
            let action_graph = client.load_turtle(test.action.clone());
            let result_graph = test.result
                .clone()
                .map(|r| client.load_turtle(r))
                .unwrap_or_else(|| Ok(MemoryGraph::default()));
            assert!(
                action_graph.is_err()
                    || !action_graph.unwrap().is_isomorphic(&result_graph.unwrap()),
                "Failure on {}",
                test
            );
        } else {
            assert!(false, "Not supported test: {}", test);
        }
    }
}

#[test]
fn ntriples_w3c_testsuite() {
    let client = RDFClient::default();
    let manifest_url = Url::parse("http://www.w3.org/2013/N-TriplesTests/manifest.ttl").unwrap();

    for test_result in TestManifest::new(&client, manifest_url) {
        let test = test_result.unwrap();
        if test.kind == "TestNTriplesPositiveSyntax" {
            if let Err(error) = client.load_ntriples(test.action.clone()) {
                assert!(false, "Failure on {} with error: {}", test, error)
            }
        } else if test.kind == "TestNTriplesNegativeSyntax" {
            assert!(
                client.load_ntriples(test.action.clone()).is_err(),
                "Failure on {}",
                test
            );
        } else {
            assert!(false, "Not supported test: {}", test);
        }
    }
}

#[test]
fn sparql_w3c_syntax_testsuite() {
    let manifest_url = Url::parse(
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/syntax-query/manifest.ttl",
    ).unwrap();
    let client = RDFClient::default();

    for test_result in TestManifest::new(&client, manifest_url) {
        let test = test_result.unwrap();
        if test.kind == "PositiveSyntaxTest11" {
            match client.load_sparql_query(test.action.clone()) {
                Err(error) => assert!(false, "Failure on {} with error: {}", test, error),
                Ok(query) => {
                    if let Err(error) = read_sparql_query(query.to_string().as_bytes(), None) {
                        assert!(
                            false,
                            "Failure tu deserialize \"{}\" of {} with error: {}",
                            query.to_string(),
                            test,
                            error
                        )
                    }
                }
            }
        } else if test.kind == "NegativeSyntaxTest11" {
            //TODO
            /*assert!(
                client.load_sparql_query(test.action.clone()).is_err(),
                "Failure on {}",
                test
            );*/
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
    pub fn load_turtle(&self, url: Url) -> RioResult<MemoryGraph> {
        Ok(read_turtle(self.get(&url)?, Some(url))?.collect())
    }

    pub fn load_ntriples(&self, url: Url) -> RioResult<MemoryGraph> {
        read_ntriples(self.get(&url)?).collect()
    }

    pub fn load_sparql_query(&self, url: Url) -> RioResult<Query> {
        read_sparql_query(self.get(&url)?, Some(url))
    }

    fn get(&self, url: &Url) -> RioResult<Response> {
        match self.client.get(url.clone()).send() {
            Ok(response) => Ok(response),
            Err(error) => if error.description() == "message is incomplete" {
                self.get(url)
            } else {
                Err(RioError::new(error))
            },
        }
    }
}

pub struct Test {
    pub id: NamedNode,
    pub kind: String,
    pub name: Option<String>,
    pub comment: Option<String>,
    pub action: Url,
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
        write!(f, " on file \"{}\"", self.action)?;
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
        pub static ref INCLUDE: NamedNode = NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#include"
        ).unwrap();
        pub static ref ENTRIES: NamedNode = NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#entries"
        ).unwrap();
        pub static ref NAME: NamedNode = NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#name"
        ).unwrap();
        pub static ref ACTION: NamedNode = NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#action"
        ).unwrap();
        pub static ref RESULT: NamedNode = NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#result"
        ).unwrap();
    }
}

impl<'a> Iterator for TestManifest<'a> {
    type Item = Result<Test, ManifestError>;

    fn next(&mut self) -> Option<Result<Test, ManifestError>> {
        match self.tests_to_do.pop() {
            Some(Term::NamedNode(test_node)) => {
                let test_subject = NamedOrBlankNode::from(test_node.clone());
                let kind = match self.graph
                    .object_for_subject_predicate(&test_subject, &rdf::TYPE)
                {
                    Some(Term::NamedNode(c)) => match c.value().split("#").last() {
                        Some(k) => k.to_string(),
                        None => return Some(Err(ManifestError::NoType)),
                    },
                    _ => return Some(Err(ManifestError::NoType)),
                };
                let name = match self.graph
                    .object_for_subject_predicate(&test_subject, &mf::NAME)
                {
                    Some(Term::Literal(c)) => Some(c.value().to_string()),
                    _ => None,
                };
                let comment = match self.graph
                    .object_for_subject_predicate(&test_subject, &rdfs::COMMENT)
                {
                    Some(Term::Literal(c)) => Some(c.value().to_string()),
                    _ => None,
                };
                let action = match self.graph
                    .object_for_subject_predicate(&test_subject, &*mf::ACTION)
                {
                    Some(Term::NamedNode(n)) => n.url().clone(),
                    Some(_) => return Some(Err(ManifestError::InvalidAction)),
                    None => return Some(Err(ManifestError::ActionNotFound)),
                };
                let result = match self.graph
                    .object_for_subject_predicate(&test_subject, &*mf::RESULT)
                {
                    Some(Term::NamedNode(n)) => Some(n.url().clone()),
                    Some(_) => return Some(Err(ManifestError::InvalidResult)),
                    None => None,
                };
                Some(Ok(Test {
                    id: test_node,
                    kind,
                    name,
                    comment,
                    action,
                    result,
                }))
            }
            Some(_) => Some(Err(ManifestError::InvalidTestsList)),
            None => {
                match self.manifests_to_do.pop() {
                    Some(url) => {
                        let manifest = NamedOrBlankNode::from(NamedNode::new(url.clone()));
                        match self.client.load_turtle(url) {
                            Ok(g) => self.graph.extend(g.into_iter()),
                            Err(e) => return Some(Err(e.into())),
                        }

                        // New manifests
                        match self.graph
                            .object_for_subject_predicate(&manifest, &*mf::INCLUDE)
                        {
                            Some(Term::BlankNode(list)) => {
                                self.manifests_to_do.extend(
                                    self.graph
                                        .values_for_list(list.clone().into())
                                        .flat_map(|m| match m {
                                            Term::NamedNode(nm) => Some(nm.url().clone()),
                                            _ => None,
                                        }),
                                );
                            }
                            Some(_) => return Some(Err(ManifestError::InvalidTestsList)),
                            None => (),
                        }

                        // New tests
                        match self.graph
                            .object_for_subject_predicate(&manifest, &*mf::ENTRIES)
                        {
                            Some(Term::BlankNode(list)) => {
                                self.tests_to_do
                                    .extend(self.graph.values_for_list(list.clone().into()));
                            }
                            Some(_) => return Some(Err(ManifestError::InvalidTestsList)),
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

#[derive(Debug)]
pub enum ManifestError {
    NoType,
    ActionNotFound,
    InvalidAction,
    InvalidResult,
    InvalidTestsList,
    RioError(RioError),
}

impl Error for ManifestError {
    fn description(&self) -> &str {
        match self {
            ManifestError::NoType => "no type found on the test case",
            ManifestError::ActionNotFound => "action not found",
            ManifestError::InvalidAction => "invalid action",
            ManifestError::InvalidResult => "invalid result",
            ManifestError::InvalidTestsList => "invalid tests list",
            ManifestError::RioError(e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&Error> {
        match self {
            ManifestError::RioError(e) => Some(e),
            _ => None,
        }
    }
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

impl From<RioError> for ManifestError {
    fn from(e: RioError) -> Self {
        ManifestError::RioError(e)
    }
}
