///! Integration tests based on [RDF 1.1 Test Cases](https://www.w3.org/TR/rdf11-testcases/)
use failure::format_err;
use rudf::model::vocab::rdf;
use rudf::model::vocab::rdfs;
use rudf::model::*;
use rudf::rio::ntriples::read_ntriples;
use rudf::rio::turtle::read_turtle;
use rudf::rio::xml::read_rdf_xml;
use rudf::store::isomorphism::GraphIsomorphism;
use rudf::store::MemoryGraph;
use rudf::Result;
use std::fmt;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use url::Url;

#[test]
fn turtle_w3c_testsuite() {
    let manifest_url = Url::parse("http://www.w3.org/2013/TurtleTests/manifest.ttl").unwrap();
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
        NamedNode::new(manifest_url.join("#IRI-resolution-01").unwrap()),
        NamedNode::new(manifest_url.join("#IRI-resolution-02").unwrap()),
        NamedNode::new(manifest_url.join("#IRI-resolution-07").unwrap()),
        NamedNode::new(manifest_url.join("#turtle-subm-01").unwrap()),
        NamedNode::new(manifest_url.join("#turtle-subm-27").unwrap()),
    ];

    for test_result in TestManifest::new(manifest_url) {
        let test = test_result.unwrap();
        if test_blacklist.contains(&test.id) {
            continue;
        }
        if test.kind == "TestTurtlePositiveSyntax" {
            if let Err(error) = load_turtle(test.action.clone()) {
                assert!(false, "Failure on {} with error: {}", test, error)
            }
        } else if test.kind == "TestTurtleNegativeSyntax" {
            assert!(
                load_turtle(test.action.clone()).is_err(),
                "Failure on {}",
                test
            );
        } else if test.kind == "TestTurtleEval" {
            match load_turtle(test.action.clone()) {
                Ok(action_graph) => match load_turtle(test.result.clone().unwrap()) {
                    Ok(result_graph) => assert!(
                        action_graph.is_isomorphic(&result_graph).unwrap(),
                        "Failure on {}. Expected file:\n{}\nParsed file:\n{}\n",
                        test,
                        result_graph,
                        action_graph
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
            let action_graph = load_turtle(test.action.clone());
            let result_graph = test
                .result
                .clone()
                .map(|r| load_turtle(r))
                .unwrap_or_else(|| Ok(MemoryGraph::default()));
            assert!(
                action_graph.is_err()
                    || !action_graph
                        .unwrap()
                        .is_isomorphic(&result_graph.unwrap())
                        .unwrap(),
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
    let manifest_url = Url::parse("http://www.w3.org/2013/N-TriplesTests/manifest.ttl").unwrap();

    for test_result in TestManifest::new(manifest_url) {
        let test = test_result.unwrap();
        if test.kind == "TestNTriplesPositiveSyntax" {
            if let Err(error) = load_ntriples(test.action.clone()) {
                assert!(false, "Failure on {} with error: {}", test, error)
            }
        } else if test.kind == "TestNTriplesNegativeSyntax" {
            assert!(
                load_ntriples(test.action.clone()).is_err(),
                "Failure on {}",
                test
            );
        } else {
            assert!(false, "Not supported test: {}", test);
        }
    }
}

#[test]
fn rdf_xml_w3c_testsuite() -> Result<()> {
    let manifest_url = Url::parse("http://www.w3.org/2013/RDFXMLTests/manifest.ttl")?;
    //TODO: make blacklist pass
    let test_blacklist = vec![
        NamedNode::new(manifest_url.join("#xml-canon-test001")?),
        NamedNode::new(manifest_url.join("#rdfms-seq-representation-test001")?),
        NamedNode::new(manifest_url.join("#rdf-containers-syntax-vs-schema-test004")?),
    ];

    for test_result in TestManifest::new(manifest_url) {
        let test = test_result?;
        if test_blacklist.contains(&test.id) {
            continue;
        }

        if test.kind == "TestXMLNegativeSyntax" {
            /*TODO assert!(
                load_rdf_xml(test.action.clone()).is_err(),
                "Failure on {}",
                test
            );*/
        } else if test.kind == "TestXMLEval" {
            match load_rdf_xml(test.action.clone()) {
                Ok(action_graph) => match load_ntriples(test.result.clone().unwrap()) {
                    Ok(result_graph) => assert!(
                        action_graph.is_isomorphic(&result_graph)?,
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

fn load_turtle(url: Url) -> Result<MemoryGraph> {
    Ok(read_turtle(read_file(&url)?, Some(url))?.collect())
}

fn load_ntriples(url: Url) -> Result<MemoryGraph> {
    read_ntriples(read_file(&url)?).collect()
}

fn load_rdf_xml(url: Url) -> Result<MemoryGraph> {
    read_rdf_xml(read_file(&url)?, Some(url)).collect()
}

fn to_relative_path(url: &Url) -> Result<String> {
    let url = url.as_str();
    if url.starts_with("http://www.w3.org/2013/N-TriplesTests") {
        Ok(url.replace(
            "http://www.w3.org/2013/N-TriplesTests",
            "rdf-tests/ntriples/",
        ))
    } else if url.starts_with("http://www.w3.org/2013/TurtleTests/") {
        Ok(url.replace("http://www.w3.org/2013/TurtleTests/", "rdf-tests/turtle/"))
    } else if url.starts_with("http://www.w3.org/2013/RDFXMLTests/") {
        Ok(url.replace("http://www.w3.org/2013/RDFXMLTests/", "rdf-tests/rdf-xml/"))
    } else {
        Err(format_err!("Not supported url for file: {}", url))
    }
}

fn read_file(url: &Url) -> Result<impl BufRead> {
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
    pub action: Url,
    pub result: Option<Url>,
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
    graph: MemoryGraph,
    tests_to_do: Vec<Term>,
    manifests_to_do: Vec<Url>,
}

impl TestManifest {
    pub fn new(url: Url) -> TestManifest {
        Self {
            graph: MemoryGraph::default(),
            tests_to_do: Vec::default(),
            manifests_to_do: vec![url],
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
                let action = match self
                    .graph
                    .object_for_subject_predicate(&test_subject, &*mf::ACTION)
                    .unwrap()
                {
                    Some(Term::NamedNode(n)) => n.as_url().clone(),
                    Some(_) => return Some(Err(format_err!("invalid action"))),
                    None => return Some(Err(format_err!("action not found"))),
                };
                let result = match self
                    .graph
                    .object_for_subject_predicate(&test_subject, &*mf::RESULT)
                    .unwrap()
                {
                    Some(Term::NamedNode(n)) => Some(n.as_url().clone()),
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
                        let manifest = NamedOrBlankNode::from(NamedNode::new(url.clone()));
                        match load_turtle(url) {
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

pub struct RdfListIterator<'a, G: Graph> {
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
