#[macro_use]
extern crate lazy_static;
extern crate reqwest;
extern crate rudf;
extern crate url;

use reqwest::Client;
use reqwest::Response;
use rudf::model::data::*;
use rudf::model::vocab::rdf;
use rudf::model::vocab::rdfs;
use rudf::rio::ntriples::read_ntriples;
use rudf::rio::turtle::read_turtle;
use rudf::rio::RioError;
use rudf::rio::RioResult;
use rudf::store::isomorphism::GraphIsomorphism;
use rudf::store::memory::MemoryGraph;
use std::error::Error;
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
    fn load_turtle(&self, url: Url) -> RioResult<MemoryGraph> {
        Ok(read_turtle(self.get(&url)?, Some(url))?.collect())
    }

    fn load_ntriples(&self, url: Url) -> RioResult<MemoryGraph> {
        read_ntriples(self.get(&url)?).collect()
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

mod mf {
    use rudf::model::data::NamedNode;
    use std::str::FromStr;

    lazy_static! {
        pub static ref ACTION: NamedNode = NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#action"
        ).unwrap();
        pub static ref RESULT: NamedNode = NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#result"
        ).unwrap();
    }
}

#[test]
fn turtle_w3c_testsuite() {
    let manifest_url = Url::parse("http://www.w3.org/2013/TurtleTests/manifest.ttl").unwrap();
    let client = RDFClient::default();
    let manifest = client.load_turtle(manifest_url.clone()).unwrap();
    let rdft_test_turtle_positive_syntax = Term::from(
        NamedNode::from_str("http://www.w3.org/ns/rdftest#TestTurtlePositiveSyntax").unwrap(),
    );
    let rdft_test_turtle_negative_syntax = Term::from(
        NamedNode::from_str("http://www.w3.org/ns/rdftest#TestTurtleNegativeSyntax").unwrap(),
    );
    let rdft_test_turtle_eval =
        Term::from(NamedNode::from_str("http://www.w3.org/ns/rdftest#TestTurtleEval").unwrap());
    let rdft_test_turtle_negative_eval = Term::from(
        NamedNode::from_str("http://www.w3.org/ns/rdftest#TestTurtleNegativeEval").unwrap(),
    );
    //TODO: make blacklist pass
    let test_blacklist: Vec<NamedOrBlankNode> = vec![
        //UTF-8 broken surrogates in BNode ids
        NamedNode::new(
            manifest_url
                .join("#prefix_with_PN_CHARS_BASE_character_boundaries")
                .unwrap(),
        ).into(),
        NamedNode::new(
            manifest_url
                .join("#labeled_blank_node_with_PN_CHARS_BASE_character_boundaries")
                .unwrap(),
        ).into(),
        NamedNode::new(
            manifest_url
                .join("#localName_with_assigned_nfc_PN_CHARS_BASE_character_boundaries")
                .unwrap(),
        ).into(),
        NamedNode::new(
            manifest_url
                .join("#localName_with_nfc_PN_CHARS_BASE_character_boundaries")
                .unwrap(),
        ).into(),
    ];

    manifest
        .subjects_for_predicate_object(&rdf::TYPE, &rdft_test_turtle_positive_syntax)
        .for_each(|test| {
            let comment = manifest
                .object_for_subject_predicate(test, &rdfs::COMMENT)
                .unwrap();
            if let Some(Term::NamedNode(file)) =
                manifest.object_for_subject_predicate(test, &mf::ACTION)
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
    manifest
        .subjects_for_predicate_object(&rdf::TYPE, &rdft_test_turtle_negative_syntax)
        .for_each(|test| {
            let comment = manifest
                .object_for_subject_predicate(test, &rdfs::COMMENT)
                .unwrap();
            if let Some(Term::NamedNode(file)) =
                manifest.object_for_subject_predicate(test, &mf::ACTION)
            {
                assert!(
                    client.load_turtle(file.url().clone()).is_err(),
                    "Failure on negative syntax test file {} about {}",
                    file,
                    comment
                );
            }
        });
    manifest
        .subjects_for_predicate_object(&rdf::TYPE, &rdft_test_turtle_eval)
        .for_each(|test| {
            if test_blacklist.contains(test) {
                return;
            }
            let comment = manifest
                .object_for_subject_predicate(test, &rdfs::COMMENT)
                .unwrap();
            if let Some(Term::NamedNode(input)) =
                manifest.object_for_subject_predicate(test, &mf::ACTION)
            {
                if let Some(Term::NamedNode(result)) =
                    manifest.object_for_subject_predicate(test, &mf::RESULT)
                {
                    match client.load_turtle(input.url().clone()) {
                    Ok(action_graph) =>  match client.load_turtle(result.url().clone()) {
                        Ok(result_graph) => assert!(
                            action_graph.is_isomorphic(&result_graph),
                            "Failure on positive evaluation test file {} against {} about {}. Expected file:\n{}\nParsed file:\n{}\n",
                            input,
                            result,
                            comment,
                            action_graph,
                            result_graph
                        ),
                        Err(error) => assert!(
                            false,
                            "Failure to parse the Turtle result file {} about {} with error: {}",
                            result, comment, error
                        )
                    },
                    Err(error) => assert!(
                        false,
                        "Failure to parse the Turtle input file {} about {} with error: {}",
                        input, comment, error
                    )
                }
                }
            }
        });
    manifest
        .subjects_for_predicate_object(&rdf::TYPE, &rdft_test_turtle_negative_eval)
        .for_each(|test| {
            let comment = manifest
                .object_for_subject_predicate(test, &rdfs::COMMENT)
                .unwrap();
            if let Some(Term::NamedNode(file)) =
                manifest.object_for_subject_predicate(test, &mf::ACTION)
            {
                if let Some(Term::NamedNode(result)) =
                    manifest.object_for_subject_predicate(test, &mf::RESULT)
                {
                    let action_graph = client.load_turtle(file.url().clone());
                    let result_graph = client.load_turtle(result.url().clone());
                    assert!(
                        !action_graph.unwrap().is_isomorphic(&result_graph.unwrap()),
                        "Failure on positive evaluation test file {} about {}",
                        file,
                        comment
                    );
                }
            }
        });
}

#[test]
fn ntriples_w3c_testsuite() {
    let client = RDFClient::default();
    let manifest = client
        .load_turtle(Url::parse("http://www.w3.org/2013/N-TriplesTests/manifest.ttl").unwrap())
        .unwrap();
    let rdft_test_ntriples_positive_syntax = Term::from(
        NamedNode::from_str("http://www.w3.org/ns/rdftest#TestNTriplesPositiveSyntax").unwrap(),
    );
    let rdft_test_ntriples_negative_syntax = Term::from(
        NamedNode::from_str("http://www.w3.org/ns/rdftest#TestNTriplesNegativeSyntax").unwrap(),
    );

    manifest
        .subjects_for_predicate_object(&rdf::TYPE, &rdft_test_ntriples_positive_syntax)
        .for_each(|test| {
            let comment = manifest
                .object_for_subject_predicate(test, &rdfs::COMMENT)
                .unwrap();
            if let Some(Term::NamedNode(file)) =
                manifest.object_for_subject_predicate(test, &mf::ACTION)
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
    manifest
        .subjects_for_predicate_object(&rdf::TYPE, &rdft_test_ntriples_negative_syntax)
        .for_each(|test| {
            let comment = manifest
                .object_for_subject_predicate(test, &rdfs::COMMENT)
                .unwrap();
            if let Some(Term::NamedNode(file)) =
                manifest.object_for_subject_predicate(test, &mf::ACTION)
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
