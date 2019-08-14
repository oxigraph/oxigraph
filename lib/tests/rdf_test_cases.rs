///! Integration tests based on [RDF 1.1 Test Cases](https://www.w3.org/TR/rdf11-testcases/)
use failure::format_err;
use rudf::model::vocab::rdf;
use rudf::model::vocab::rdfs;
use rudf::model::*;
use rudf::rio::read_ntriples;
use rudf::rio::read_rdf_xml;
use rudf::rio::read_turtle;
use rudf::Result;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

#[test]
fn turtle_w3c_testsuite() {
    let manifest_url = "http://w3c.github.io/rdf-tests/turtle/manifest.ttl";
    for test_result in TestManifest::new(manifest_url) {
        let test = test_result.unwrap();
        if test.kind == "TestTurtlePositiveSyntax" {
            if let Err(error) = load_turtle(test.action.as_str()) {
                assert!(false, "Failure on {} with error: {}", test, error)
            }
        } else if test.kind == "TestTurtleNegativeSyntax" {
            assert!(
                load_turtle(test.action.as_str()).is_err(),
                "Failure on {}",
                test
            );
        } else if test.kind == "TestTurtleEval" {
            match load_turtle(test.action.as_str()) {
                Ok(action_graph) => match load_turtle(test.result.as_ref().unwrap()) {
                    Ok(result_graph) => assert!(
                        action_graph.is_isomorphic(&result_graph),
                        "Failure on {}. Expected file:\n{}\nParsed file:\n{}\n",
                        test,
                        result_graph,
                        action_graph
                    ),
                    Err(error) => assert!(
                        false,
                        "Failure to parse the Turtle result file {} of {} with error: {}",
                        test.result.as_ref().unwrap(),
                        test,
                        error
                    ),
                },
                Err(error) => assert!(false, "Failure to parse {} with error: {}", test, error),
            }
        } else if test.kind == "TestTurtleNegativeEval" {
            let action_graph = load_turtle(test.action.as_str());
            let result_graph = test
                .result
                .clone()
                .map(|r| load_turtle(r.as_str()))
                .unwrap_or_else(|| Ok(SimpleGraph::default()));
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
    let manifest_url = "http://w3c.github.io/rdf-tests/ntriples/manifest.ttl";

    for test_result in TestManifest::new(manifest_url) {
        let test = test_result.unwrap();
        if test.kind == "TestNTriplesPositiveSyntax" {
            if let Err(error) = load_ntriples(test.action.as_str()) {
                assert!(false, "Failure on {} with error: {}", test, error)
            }
        } else if test.kind == "TestNTriplesNegativeSyntax" {
            if let Ok(graph) = load_ntriples(test.action.as_str()) {
                assert!(false, "Failure on {}, found:\n{}", test, graph);
            }
        } else {
            assert!(false, "Not supported test: {}", test);
        }
    }
}

#[test]
fn rdf_xml_w3c_testsuite() -> Result<()> {
    let manifest_url = "http://www.w3.org/2013/RDFXMLTests/manifest.ttl";

    for test_result in TestManifest::new(manifest_url) {
        let test = test_result?;

        if test.kind == "TestXMLNegativeSyntax" {
            assert!(
                load_rdf_xml(test.action.as_str()).is_err(),
                "Failure on {}",
                test
            );
        } else if test.kind == "TestXMLEval" {
            match load_rdf_xml(test.action.as_str()) {
                Ok(action_graph) => match load_ntriples(test.result.as_ref().unwrap()) {
                    Ok(result_graph) => assert!(
                        action_graph.is_isomorphic(&result_graph),
                        "Failure on {}. Expected file:\n{}\nParsed file:\n{}\n",
                        test,
                        result_graph,
                        action_graph
                    ),
                    Err(error) => assert!(
                        false,
                        "Failure to parse the RDF XML result file {} of {} with error: {}",
                        test.result.clone().unwrap(),
                        test,
                        error
                    ),
                },
                Err(error) => assert!(false, "Failure to parse {} with error: {}", test, error),
            }
        } else {
            assert!(false, "Not supported test: {}", test);
        }
    }
    Ok(())
}

fn load_turtle(url: &str) -> Result<SimpleGraph> {
    read_turtle(read_file(url)?, Some(url))?.collect()
}

fn load_ntriples(url: &str) -> Result<SimpleGraph> {
    read_ntriples(read_file(url)?)?.collect()
}

fn load_rdf_xml(url: &str) -> Result<SimpleGraph> {
    read_rdf_xml(read_file(url)?, Some(url))?.collect()
}

fn to_relative_path(url: &str) -> Result<String> {
    if url.starts_with("http://w3c.github.io/rdf-tests/") {
        Ok(url.replace("http://w3c.github.io/", ""))
    } else if url.starts_with("http://www.w3.org/2013/RDFXMLTests/") {
        Ok(url.replace("http://www.w3.org/2013/RDFXMLTests/", "rdf-tests/rdf-xml/"))
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

pub struct Test {
    pub id: NamedNode,
    pub kind: String,
    pub name: Option<String>,
    pub comment: Option<String>,
    pub action: String,
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
        write!(f, " on file \"{}\"", self.action)?;
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
                        None => return Some(Err(format_err!("no type"))),
                    },
                    _ => return Some(Err(format_err!("no type"))),
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
                let action = match self
                    .graph
                    .object_for_subject_predicate(&test_subject, &*mf::ACTION)
                {
                    Some(Term::NamedNode(n)) => n.as_str().to_string(),
                    Some(_) => return Some(Err(format_err!("invalid action"))),
                    None => return Some(Err(format_err!("action not found"))),
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
                    action,
                    result,
                }))
            }
            Some(_) => Some(Err(format_err!("invalid test list"))),
            None => {
                match self.manifests_to_do.pop() {
                    Some(url) => {
                        let manifest =
                            NamedOrBlankNode::from(NamedNode::new(url.as_str().to_string()));
                        match load_turtle(&url) {
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
