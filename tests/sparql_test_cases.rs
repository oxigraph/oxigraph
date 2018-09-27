///! Integration tests based on [SPARQL 1.1 Test Cases](https://www.w3.org/2009/sparql/docs/tests/README.html)

#[macro_use]
extern crate lazy_static;
extern crate reqwest;
extern crate rudf;
extern crate url;

use reqwest::Client;
use reqwest::Response;
use rudf::errors::*;
use rudf::model::vocab::rdf;
use rudf::model::vocab::rdfs;
use rudf::model::*;
use rudf::rio::ntriples::read_ntriples;
use rudf::rio::turtle::read_turtle;
use rudf::rio::xml::read_rdf_xml;
use rudf::sparql::algebra::Query;
use rudf::sparql::parser::read_sparql_query;
use rudf::store::MemoryGraph;
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
    ).unwrap();
    let test_blacklist = vec![
        NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql2/manifest#syntax-form-construct02").unwrap(),
        //TODO: Deserialization of the serialization failing:
        NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql2/manifest#syntax-form-construct04").unwrap(),
        NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql2/manifest#syntax-function-04").unwrap(),
        NamedNode::from_str("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/syntax-sparql1/manifest#syntax-lit-08").unwrap(),
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
            match client.load_sparql_query(test.action.clone()) {
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
            if let Ok(result) = client.load_sparql_query(test.action.clone()) {
                eprintln!("Failure on {}. The output tree is: {}", test, result);
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
    pub fn load_turtle(&self, url: Url) -> Result<MemoryGraph> {
        Ok(read_turtle(self.get(&url)?, Some(url))?.collect())
    }

    pub fn load_ntriples(&self, url: Url) -> Result<MemoryGraph> {
        read_ntriples(self.get(&url)?).collect()
    }

    pub fn load_rdf_xml(&self, url: Url) -> Result<MemoryGraph> {
        read_rdf_xml(BufReader::new(self.get(&url)?), Some(url)).collect()
    }

    pub fn load_sparql_query(&self, url: Url) -> Result<Query> {
        read_sparql_query(self.get(&url)?, Some(url))
    }

    fn get(&self, url: &Url) -> Result<Response> {
        match self.client.get(url.clone()).send() {
            Ok(response) => Ok(response),
            Err(error) => if error.description() == "parsed HTTP message from remote is incomplete"
            {
                self.get(url)
            } else {
                Err(format!("HTTP request error: {}", error.description()).into())
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
                    Some(Term::NamedNode(c)) => match c.value().split("#").last() {
                        Some(k) => k.to_string(),
                        None => return Some(Err("no type".into())),
                    },
                    _ => return Some(Err("no type".into())),
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
                let action = match self
                    .graph
                    .object_for_subject_predicate(&test_subject, &*mf::ACTION)
                    .unwrap()
                {
                    Some(Term::NamedNode(n)) => n.url().clone(),
                    Some(_) => return Some(Err("invalid action".into())),
                    None => return Some(Err("action not found".into())),
                };
                let result = match self
                    .graph
                    .object_for_subject_predicate(&test_subject, &*mf::RESULT)
                    .unwrap()
                {
                    Some(Term::NamedNode(n)) => Some(n.url().clone()),
                    Some(_) => return Some(Err("invalid result".into())),
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
            Some(_) => Some(Err("invalid test list".into())),
            None => {
                match self.manifests_to_do.pop() {
                    Some(url) => {
                        let manifest = NamedOrBlankNode::from(NamedNode::new(url.clone()));
                        match self.client.load_turtle(url) {
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
                                        .flat_map(|m| match m {
                                            Term::NamedNode(nm) => Some(nm.url().clone()),
                                            _ => None,
                                        }),
                                );
                            }
                            Some(_) => return Some(Err("invalid tests list".into())),
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
                                return Some(Err(
                                    format!("Invalid tests list. Got term {}", term).into()
                                ))
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
