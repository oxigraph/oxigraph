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
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
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

#[derive(Eq, PartialEq, Clone)]
struct Graph(HashSet<Triple>);

impl fmt::Display for Graph {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        for triple in &self.0 {
            write!(fmt, "{}\n", triple)?;
        }
        Ok(())
    }
}

#[derive(Eq, PartialEq, Hash, Ord, PartialOrd)]
struct SubjectPredicate<'a> {
    subject: &'a NamedOrBlankNode,
    predicate: &'a NamedNode,
}

impl<'a> SubjectPredicate<'a> {
    fn new(subject: &'a NamedOrBlankNode, predicate: &'a NamedNode) -> Self {
        Self { subject, predicate }
    }
}

#[derive(Eq, PartialEq, Hash, Ord, PartialOrd)]
struct PredicateObject<'a> {
    predicate: &'a NamedNode,
    object: &'a Term,
}

impl<'a> PredicateObject<'a> {
    fn new(predicate: &'a NamedNode, object: &'a Term) -> Self {
        Self { predicate, object }
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

fn subject_predicates_for_object<'a>(
    graph: &'a HashSet<Triple>,
    object: &'a Term,
) -> impl Iterator<Item = SubjectPredicate<'a>> {
    graph
        .iter()
        .filter(move |t| t.object() == object)
        .map(|t| SubjectPredicate::new(t.subject(), t.predicate()))
}

fn predicate_objects_for_subject<'a>(
    graph: &'a HashSet<Triple>,
    subject: &'a NamedOrBlankNode,
) -> impl Iterator<Item = PredicateObject<'a>> {
    graph
        .iter()
        .filter(move |t| t.subject() == subject)
        .map(|t| PredicateObject::new(t.predicate(), t.object()))
}

fn hash_blank_nodes<'a>(
    bnodes: HashSet<&'a BlankNode>,
    graph: &'a HashSet<Triple>,
) -> HashMap<u64, Vec<&'a BlankNode>> {
    let mut bnodes_by_hash: HashMap<u64, Vec<&BlankNode>> = HashMap::default();

    // NB: we need to sort the triples to have the same hash
    for bnode in bnodes.into_iter() {
        let mut hasher = DefaultHasher::new();

        {
            let subject = NamedOrBlankNode::from(bnode.clone());
            let mut po_set: BTreeSet<PredicateObject> = BTreeSet::default();
            for po in predicate_objects_for_subject(&graph, &subject) {
                if !po.object.is_blank_node() {
                    po_set.insert(po);
                }
            }
            for po in po_set {
                po.hash(&mut hasher);
            }
        }

        {
            let object = Term::from(bnode.clone());
            let mut sp_set: BTreeSet<SubjectPredicate> = BTreeSet::default();
            for sp in subject_predicates_for_object(&graph, &object) {
                if !sp.subject.is_blank_node() {
                    sp_set.insert(sp);
                }
            }
            for sp in sp_set {
                sp.hash(&mut hasher);
            }
        }

        bnodes_by_hash
            .entry(hasher.finish())
            .or_insert_with(Vec::default)
            .push(bnode);
    }

    bnodes_by_hash
}

//TODO: use a better datastructure
fn is_isomorphic(a: &HashSet<Triple>, b: &HashSet<Triple>) -> bool {
    if a.len() != b.len() {
        return false;
    }

    let mut a_bnodes: HashSet<&BlankNode> = HashSet::default();
    let mut b_bnodes: HashSet<&BlankNode> = HashSet::default();

    for t in a {
        if let NamedOrBlankNode::BlankNode(subject) = t.subject() {
            a_bnodes.insert(subject);
            if let Term::BlankNode(object) = t.object() {
                a_bnodes.insert(object);
            }
        } else if let Term::BlankNode(object) = t.object() {
            a_bnodes.insert(object);
        } else if !b.contains(t) {
            return false;
        }
    }
    for t in b {
        if let NamedOrBlankNode::BlankNode(subject) = t.subject() {
            b_bnodes.insert(subject);
            if let Term::BlankNode(object) = t.object() {
                b_bnodes.insert(object);
            }
        } else if let Term::BlankNode(object) = t.object() {
            b_bnodes.insert(object);
        } else if !a.contains(t) {
            return false;
        }
    }

    let a_bnodes_by_hash = hash_blank_nodes(a_bnodes, &a);
    let b_bnodes_by_hash = hash_blank_nodes(b_bnodes, &b);

    if a_bnodes_by_hash.len() != b_bnodes_by_hash.len() {
        return false;
    }

    for hash in a_bnodes_by_hash.keys() {
        if a_bnodes_by_hash.get(hash).map(|l| l.len())
            != b_bnodes_by_hash.get(hash).map(|l| l.len())
        {
            return false;
        }
    }

    //TODO: proper isomorphism building

    true
}

#[test]
fn turtle_w3c_testsuite() {
    let manifest_url = Url::parse("http://www.w3.org/2013/TurtleTests/manifest.ttl").unwrap();
    let client = RDFClient::default();
    let manifest = client.load_turtle(manifest_url.clone()).unwrap();
    let mf_action = NamedNode::from_str(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#action",
    ).unwrap();
    let mf_result = NamedNode::from_str(
        "http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#result",
    ).unwrap();
    let rdfs_comment = NamedNode::from_str("http://www.w3.org/2000/01/rdf-schema#comment").unwrap();
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
    subjects_for_predicate_object(&manifest, &rdf::TYPE, &rdft_test_turtle_eval).for_each(|test| {
        if test_blacklist.contains(test) {
            return;
        }
        let comment = object_for_subject_predicate(&manifest, test, &rdfs_comment).unwrap();
        if let Some(Term::NamedNode(input)) =
            object_for_subject_predicate(&manifest, test, &mf_action)
        {
            if let Some(Term::NamedNode(result)) =
                object_for_subject_predicate(&manifest, test, &mf_result)
            {
                match client.load_turtle(input.url().clone()) {
                    Ok(action_graph) =>  match client.load_turtle(result.url().clone()) {
                        Ok(result_graph) => assert!(
                            is_isomorphic(&action_graph, &result_graph),
                            "Failure on positive evaluation test file {} against {} about {}. Expected file:\n{}\nParsed file:\n{}\n",
                            input,
                            result,
                            comment,
                            Graph(action_graph),
                            Graph(result_graph)
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
    subjects_for_predicate_object(&manifest, &rdf::TYPE, &rdft_test_turtle_negative_eval).for_each(
        |test| {
            let comment = object_for_subject_predicate(&manifest, test, &rdfs_comment).unwrap();
            if let Some(Term::NamedNode(file)) =
                object_for_subject_predicate(&manifest, test, &mf_action)
            {
                if let Some(Term::NamedNode(result)) =
                    object_for_subject_predicate(&manifest, test, &mf_result)
                {
                    let action_graph = client.load_turtle(file.url().clone());
                    let result_graph = client.load_turtle(result.url().clone());
                    assert!(
                        !is_isomorphic(&action_graph.unwrap(), &result_graph.unwrap()),
                        "Failure on positive evaluation test file {} about {}",
                        file,
                        comment
                    );
                }
            }
        },
    );
}

#[test]
fn ntriples_w3c_testsuite() {
    let client = RDFClient::default();
    let manifest = client
        .load_turtle(Url::parse("http://www.w3.org/2013/N-TriplesTests/manifest.ttl").unwrap())
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
