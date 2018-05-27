///! Integration tests based on [SPARQL 1.1 Test Cases](https://www.w3.org/2009/sparql/docs/tests/)

#[macro_use]
extern crate lazy_static;
extern crate reqwest;
extern crate rudf;
extern crate url;

mod client;

use client::RDFClient;
use rudf::model::data::*;
use rudf::model::vocab::rdf;
use rudf::sparql::parser::read_sparql_query;
use url::Url;

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
        pub static ref POSITIVE_SYNTAX_TEST_11: NamedNode = NamedNode::from_str(
            "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#PositiveSyntaxTest11"
        ).unwrap();
    }
}

#[test]
fn sparql_w3c_syntax_testsuite() {
    let manifest_url = Url::parse(
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/syntax-query/manifest.ttl",
    ).unwrap();
    let client = RDFClient::default();
    let manifest = client.load_turtle(manifest_url.clone()).unwrap();
    let mf_positive_syntax_test = Term::from(mf::POSITIVE_SYNTAX_TEST_11.clone());

    manifest
        .subjects_for_predicate_object(&rdf::TYPE, &mf_positive_syntax_test)
        .for_each(|test| {
            if let Some(Term::NamedNode(file)) =
                manifest.object_for_subject_predicate(test, &mf::ACTION)
            {
                match client.load_sparql_query(file.url().clone()) {
                    Err(error) => assert!(
                        false,
                        "Failure on positive syntax file {} with error: {}",
                        file, error
                    ),
                    Ok(query) => {
                        if let Err(error) = read_sparql_query(query.to_string().as_bytes(), None) {
                            assert!(
                                false,
                                "Failure tu deserialize \"{}\" of file {} with error: {}",
                                query.to_string(),
                                file,
                                error
                            )
                        }
                    }
                }
            }
        });
}
