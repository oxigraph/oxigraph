use crate::files::load_to_store;
use crate::vocab::*;
use oxigraph::model::vocab::*;
use oxigraph::model::*;
use oxigraph::{Error, MemoryStore, Result};
use std::fmt;

pub struct Test {
    pub id: NamedNode,
    pub kind: NamedNode,
    pub name: Option<String>,
    pub comment: Option<String>,
    pub action: Option<String>,
    pub query: Option<String>,
    pub data: Option<String>,
    pub graph_data: Vec<String>,
    pub service_data: Vec<(String, String)>,
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
        if let Some(action) = &self.action {
            write!(f, " on file \"{}\"", action)?;
        }
        if let Some(query) = &self.query {
            write!(f, " on query {}", &query)?;
        }
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
    graph: MemoryStore,
    tests_to_do: Vec<Term>,
    manifests_to_do: Vec<String>,
}

impl TestManifest {
    pub fn new<S: ToString>(manifest_urls: impl IntoIterator<Item = S>) -> Self {
        Self {
            graph: MemoryStore::new(),
            tests_to_do: Vec::new(),
            manifests_to_do: manifest_urls
                .into_iter()
                .map(|url| url.to_string())
                .collect(),
        }
    }
}

impl Iterator for TestManifest {
    type Item = Result<Test>;

    fn next(&mut self) -> Option<Result<Test>> {
        match self.tests_to_do.pop() {
            Some(Term::NamedNode(test_node)) => {
                let test_subject = NamedOrBlankNode::from(test_node.clone());
                let kind =
                    match object_for_subject_predicate(&self.graph, &test_subject, &rdf::TYPE) {
                        Some(Term::NamedNode(c)) => c,
                        _ => return self.next(), //We ignore the test
                    };
                let name = match object_for_subject_predicate(&self.graph, &test_subject, &mf::NAME)
                {
                    Some(Term::Literal(c)) => Some(c.value().to_string()),
                    _ => None,
                };
                let comment = match object_for_subject_predicate(
                    &self.graph,
                    &test_subject,
                    &rdfs::COMMENT,
                ) {
                    Some(Term::Literal(c)) => Some(c.value().to_string()),
                    _ => None,
                };
                let (action, query, data, graph_data, service_data) =
                    match object_for_subject_predicate(&self.graph, &test_subject, &*mf::ACTION) {
                        Some(Term::NamedNode(n)) => {
                            (Some(n.into_string()), None, None, vec![], vec![])
                        }
                        Some(Term::BlankNode(n)) => {
                            let n = n.into();
                            let query =
                                match object_for_subject_predicate(&self.graph, &n, &qt::QUERY) {
                                    Some(Term::NamedNode(q)) => Some(q.into_string()),
                                    _ => None,
                                };
                            let data =
                                match object_for_subject_predicate(&self.graph, &n, &qt::DATA) {
                                    Some(Term::NamedNode(q)) => Some(q.into_string()),
                                    _ => None,
                                };
                            let graph_data =
                                objects_for_subject_predicate(&self.graph, &n, &qt::GRAPH_DATA)
                                    .filter_map(|g| match g {
                                        Term::NamedNode(q) => Some(q.into_string()),
                                        _ => None,
                                    })
                                    .collect();
                            let service_data =
                                objects_for_subject_predicate(&self.graph, &n, &qt::SERVICE_DATA)
                                    .filter_map(|g| match g {
                                        Term::NamedNode(g) => Some(g.into()),
                                        Term::BlankNode(g) => Some(g.into()),
                                        _ => None,
                                    })
                                    .filter_map(|g| {
                                        if let (
                                            Some(Term::NamedNode(endpoint)),
                                            Some(Term::NamedNode(data)),
                                        ) = (
                                            object_for_subject_predicate(
                                                &self.graph,
                                                &g,
                                                &qt::ENDPOINT,
                                            ),
                                            object_for_subject_predicate(
                                                &self.graph,
                                                &g,
                                                &qt::DATA,
                                            ),
                                        ) {
                                            Some((endpoint.into_string(), data.into_string()))
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                            (None, query, data, graph_data, service_data)
                        }
                        Some(_) => return Some(Err(Error::msg("invalid action"))),
                        None => {
                            return Some(Err(Error::msg(format!(
                                "action not found for test {}",
                                test_subject
                            ))));
                        }
                    };
                let result =
                    match object_for_subject_predicate(&self.graph, &test_subject, &*mf::RESULT) {
                        Some(Term::NamedNode(n)) => Some(n.into_string()),
                        Some(_) => return Some(Err(Error::msg("invalid result"))),
                        None => None,
                    };
                Some(Ok(Test {
                    id: test_node,
                    kind,
                    name,
                    comment,
                    action,
                    query,
                    data,
                    graph_data,
                    service_data,
                    result,
                }))
            }
            Some(_) => self.next(),
            None => {
                match self.manifests_to_do.pop() {
                    Some(url) => {
                        let manifest = NamedOrBlankNode::from(NamedNode::new(url.clone()).unwrap());
                        if let Err(error) = load_to_store(&url, &self.graph, None) {
                            return Some(Err(error));
                        }

                        // New manifests
                        match object_for_subject_predicate(&self.graph, &manifest, &*mf::INCLUDE) {
                            Some(Term::BlankNode(list)) => {
                                self.manifests_to_do.extend(
                                    RdfListIterator::iter(&self.graph, list.into()).filter_map(
                                        |m| match m {
                                            Term::NamedNode(nm) => Some(nm.into_string()),
                                            _ => None,
                                        },
                                    ),
                                );
                            }
                            Some(_) => return Some(Err(Error::msg("invalid tests list"))),
                            None => (),
                        }

                        // New tests
                        match object_for_subject_predicate(&self.graph, &manifest, &*mf::ENTRIES) {
                            Some(Term::BlankNode(list)) => {
                                self.tests_to_do
                                    .extend(RdfListIterator::iter(&self.graph, list.into()));
                            }
                            Some(term) => {
                                return Some(Err(Error::msg(format!(
                                    "Invalid tests list. Got term {}",
                                    term
                                ))));
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

struct RdfListIterator<'a> {
    graph: &'a MemoryStore,
    current_node: Option<NamedOrBlankNode>,
}

impl<'a> RdfListIterator<'a> {
    fn iter(graph: &'a MemoryStore, root: NamedOrBlankNode) -> RdfListIterator<'a> {
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
                let result = object_for_subject_predicate(&self.graph, &current, &rdf::FIRST);
                self.current_node =
                    match object_for_subject_predicate(&self.graph, &current, &rdf::REST) {
                        Some(Term::NamedNode(ref n)) if *n == *rdf::NIL => None,
                        Some(Term::NamedNode(n)) => Some(n.into()),
                        Some(Term::BlankNode(n)) => Some(n.into()),
                        _ => None,
                    };
                result
            }
            None => None,
        }
    }
}

fn object_for_subject_predicate(
    store: &MemoryStore,
    subject: &NamedOrBlankNode,
    predicate: &NamedNode,
) -> Option<Term> {
    objects_for_subject_predicate(store, subject, predicate).next()
}

fn objects_for_subject_predicate(
    store: &MemoryStore,
    subject: &NamedOrBlankNode,
    predicate: &NamedNode,
) -> impl Iterator<Item = Term> {
    store
        .quads_for_pattern(Some(subject), Some(predicate), None, None)
        .map(|t| t.object_owned())
}
