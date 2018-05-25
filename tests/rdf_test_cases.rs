extern crate reqwest;
extern crate rudf;
extern crate url;

use reqwest::Client;
use rudf::model::data::*;
use rudf::model::vocab::rdf;
use rudf::rio::RioError;
use rudf::rio::RioResult;
use rudf::rio::ntriples::read_ntriples;
use rudf::rio::turtle::read_turtle;
use std::collections::HashSet;
use std::iter::FromIterator;
use std::str::FromStr;
use url::Url;

struct RDFClient {
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
    fn load_turtle(&self, uri: Url) -> RioResult<HashSet<Triple>> {
        match self.client.get(uri.clone()).send() {
            Ok(response) => Ok(HashSet::from_iter(read_turtle(response, Some(uri))?)),
            Err(error) => Err(RioError::new(error)),
        }
    }

    fn load_ntriples(&self, uri: Url) -> RioResult<HashSet<Triple>> {
        match self.client.get(uri).send() {
            Ok(response) => read_ntriples(response).collect(),
            Err(error) => Err(RioError::new(error)),
        }
    }
}

fn objects_for_subject_predicate<'a>(
    graph: &'a HashSet<Triple>,
    subject: &'a NamedOrBlankNode,
    predicate: &'a NamedNode,
) -> impl Iterator<Item = &'a Term> {
    graph
        .iter()
        .filter(move |t| t.subject() == subject && t.predicate() == predicate)
        .map(|t| t.object())
}

fn object_for_subject_predicate<'a>(
    graph: &'a HashSet<Triple>,
    subject: &'a NamedOrBlankNode,
    predicate: &'a NamedNode,
) -> Option<&'a Term> {
    objects_for_subject_predicate(graph, subject, predicate).nth(0)
}

fn subjects_for_predicate_object<'a>(
    graph: &'a HashSet<Triple>,
    predicate: &'a NamedNode,
    object: &'a Term,
) -> impl Iterator<Item = &'a NamedOrBlankNode> {
    graph
        .iter()
        .filter(move |t| t.predicate() == predicate && t.object() == object)
        .map(|t| t.subject())
}

fn subject_for_predicate_object<'a>(
    graph: &'a HashSet<Triple>,
    predicate: &'a NamedNode,
    object: &'a Term,
) -> Option<&'a NamedOrBlankNode> {
    subjects_for_predicate_object(graph, predicate, object).nth(0)
}

#[test]
fn turtle_w3c_testsuite() {
    let client = RDFClient::default();
    let manifest = client
        .load_turtle(Url::parse("https://www.w3.org/2013/TurtleTests/manifest.ttl").unwrap())
        .unwrap();
    let mf_action = NamedNode::from_str(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#action",
    ).unwrap();
    let rdfs_comment = NamedNode::from_str("http://www.w3.org/2000/01/rdf-schema#comment").unwrap();
    let rdft_test_turtle_positive_syntax = Term::from(
        NamedNode::from_str("http://www.w3.org/ns/rdftest#TestTurtlePositiveSyntax").unwrap(),
    );
    let rdft_test_turtle_negative_syntax = Term::from(
        NamedNode::from_str("http://www.w3.org/ns/rdftest#TestTurtleNegativeSyntax").unwrap(),
    );

    subjects_for_predicate_object(&manifest, &rdf::TYPE, &rdft_test_turtle_positive_syntax)
        .for_each(|test| {
            let comment = object_for_subject_predicate(&manifest, test, &rdfs_comment).unwrap();
            if let Some(Term::NamedNode(file)) =
                object_for_subject_predicate(&manifest, test, &mf_action)
            {
                if let Err(error) = client.load_turtle(file.url().clone()) {
                    assert!(
                        false,
                        "Failure on positive syntax file {} about {} with error: {}",
                        file, comment, error
                    )
                }
            }
        });
    subjects_for_predicate_object(&manifest, &rdf::TYPE, &rdft_test_turtle_negative_syntax)
        .for_each(|test| {
            let comment = object_for_subject_predicate(&manifest, test, &rdfs_comment).unwrap();
            if let Some(Term::NamedNode(file)) =
                object_for_subject_predicate(&manifest, test, &mf_action)
            {
                assert!(
                    client.load_turtle(file.url().clone()).is_err(),
                    "Failure on negative syntax test file {} about {}",
                    file,
                    comment
                );
            }
        });
}

#[test]
fn ntriples_w3c_testsuite() {
    let client = RDFClient::default();
    let manifest = client
        .load_turtle(Url::parse("https://www.w3.org/2013/N-TriplesTests/manifest.ttl").unwrap())
        .unwrap();
    let mf_action = NamedNode::from_str(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#action",
    ).unwrap();
    let rdfs_comment = NamedNode::from_str("http://www.w3.org/2000/01/rdf-schema#comment").unwrap();
    let rdft_test_ntriples_positive_syntax = Term::from(
        NamedNode::from_str("http://www.w3.org/ns/rdftest#TestNTriplesPositiveSyntax").unwrap(),
    );
    let rdft_test_ntriples_negative_syntax = Term::from(
        NamedNode::from_str("http://www.w3.org/ns/rdftest#TestNTriplesNegativeSyntax").unwrap(),
    );

    subjects_for_predicate_object(&manifest, &rdf::TYPE, &rdft_test_ntriples_positive_syntax)
        .for_each(|test| {
            let comment = object_for_subject_predicate(&manifest, test, &rdfs_comment).unwrap();
            if let Some(Term::NamedNode(file)) =
                object_for_subject_predicate(&manifest, test, &mf_action)
            {
                if let Err(error) = client.load_ntriples(file.url().clone()) {
                    assert!(
                        false,
                        "Failure on positive syntax file {} about {} with error: {}",
                        file, comment, error
                    )
                }
            }
        });
    subjects_for_predicate_object(&manifest, &rdf::TYPE, &rdft_test_ntriples_negative_syntax)
        .for_each(|test| {
            let comment = object_for_subject_predicate(&manifest, test, &rdfs_comment).unwrap();
            if let Some(Term::NamedNode(file)) =
                object_for_subject_predicate(&manifest, test, &mf_action)
            {
                assert!(
                    client.load_ntriples(file.url().clone()).is_err(),
                    "Failure on negative syntax test file {} about {}",
                    file,
                    comment
                );
            }
        });
}
