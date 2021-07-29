use crate::files::load_to_store;
use crate::vocab::*;
use anyhow::{anyhow, Result};
use oxigraph::model::vocab::*;
use oxigraph::model::*;
use oxigraph::MemoryStore;
use std::fmt;

pub struct Test {
    pub id: NamedNode,
    pub kind: NamedNode,
    pub name: Option<String>,
    pub comment: Option<String>,
    pub action: Option<String>,
    pub query: Option<String>,
    pub update: Option<String>,
    pub data: Option<String>,
    pub graph_data: Vec<(NamedNode, String)>,
    pub service_data: Vec<(String, String)>,
    pub result: Option<String>,
    pub result_graph_data: Vec<(NamedNode, String)>,
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
        for (_, data) in &self.graph_data {
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
                let kind = match object_for_subject_predicate(&self.graph, &test_node, rdf::TYPE) {
                    Some(Term::NamedNode(c)) => c,
                    _ => return self.next(), //We ignore the test
                };
                let name = match object_for_subject_predicate(&self.graph, &test_node, mf::NAME) {
                    Some(Term::Literal(c)) => Some(c.value().to_string()),
                    _ => None,
                };
                let comment =
                    match object_for_subject_predicate(&self.graph, &test_node, rdfs::COMMENT) {
                        Some(Term::Literal(c)) => Some(c.value().to_string()),
                        _ => None,
                    };
                let (action, query, update, data, graph_data, service_data) =
                    match object_for_subject_predicate(&self.graph, &test_node, mf::ACTION) {
                        Some(Term::NamedNode(n)) => {
                            (Some(n.into_string()), None, None, None, vec![], vec![])
                        }
                        Some(Term::BlankNode(n)) => {
                            let query =
                                match object_for_subject_predicate(&self.graph, &n, qt::QUERY) {
                                    Some(Term::NamedNode(q)) => Some(q.into_string()),
                                    _ => None,
                                };
                            let update =
                                match object_for_subject_predicate(&self.graph, &n, ut::REQUEST) {
                                    Some(Term::NamedNode(q)) => Some(q.into_string()),
                                    _ => None,
                                };
                            let data = match object_for_subject_predicate(&self.graph, &n, qt::DATA)
                                .or_else(|| object_for_subject_predicate(&self.graph, &n, ut::DATA))
                            {
                                Some(Term::NamedNode(q)) => Some(q.into_string()),
                                _ => None,
                            };
                            let graph_data =
                                objects_for_subject_predicate(&self.graph, &n, qt::GRAPH_DATA)
                                    .chain(objects_for_subject_predicate(
                                        &self.graph,
                                        &n,
                                        ut::GRAPH_DATA,
                                    ))
                                    .filter_map(|g| match g {
                                        Term::NamedNode(q) => Some((q.clone(), q.into_string())),
                                        Term::BlankNode(node) => {
                                            if let Some(Term::NamedNode(graph)) =
                                                object_for_subject_predicate(
                                                    &self.graph,
                                                    &node,
                                                    ut::GRAPH,
                                                )
                                            {
                                                if let Some(Term::Literal(name)) =
                                                    object_for_subject_predicate(
                                                        &self.graph,
                                                        &node,
                                                        rdfs::LABEL,
                                                    )
                                                {
                                                    Some((
                                                        NamedNode::new(name.value()).unwrap(),
                                                        graph.into_string(),
                                                    ))
                                                } else {
                                                    Some((graph.clone(), graph.into_string()))
                                                }
                                            } else {
                                                None
                                            }
                                        }
                                        _ => None,
                                    })
                                    .collect();
                            let service_data =
                                objects_for_subject_predicate(&self.graph, &n, qt::SERVICE_DATA)
                                    .filter_map(|g| match g {
                                        Term::NamedNode(g) => Some(g.into()),
                                        Term::BlankNode(g) => Some(g.into()),
                                        _ => None,
                                    })
                                    .filter_map(|g: NamedOrBlankNode| {
                                        if let (
                                            Some(Term::NamedNode(endpoint)),
                                            Some(Term::NamedNode(data)),
                                        ) = (
                                            object_for_subject_predicate(
                                                &self.graph,
                                                &g,
                                                qt::ENDPOINT,
                                            ),
                                            object_for_subject_predicate(&self.graph, &g, qt::DATA),
                                        ) {
                                            Some((endpoint.into_string(), data.into_string()))
                                        } else {
                                            None
                                        }
                                    })
                                    .collect();
                            (None, query, update, data, graph_data, service_data)
                        }
                        Some(_) => return Some(Err(anyhow!("invalid action"))),
                        None => {
                            return Some(Err(anyhow!("action not found for test {}", test_node)));
                        }
                    };
                let (result, result_graph_data) =
                    match object_for_subject_predicate(&self.graph, &test_node, mf::RESULT) {
                        Some(Term::NamedNode(n)) => (Some(n.into_string()), Vec::new()),
                        Some(Term::BlankNode(n)) => (
                            if let Some(Term::NamedNode(result)) =
                                object_for_subject_predicate(&self.graph, &n, ut::DATA)
                            {
                                Some(result.into_string())
                            } else {
                                None
                            },
                            objects_for_subject_predicate(&self.graph, &n, ut::GRAPH_DATA)
                                .filter_map(|g| match g {
                                    Term::NamedNode(q) => Some((q.clone(), q.into_string())),
                                    Term::BlankNode(node) => {
                                        if let Some(Term::NamedNode(graph)) =
                                            object_for_subject_predicate(
                                                &self.graph,
                                                &node,
                                                ut::GRAPH,
                                            )
                                        {
                                            if let Some(Term::Literal(name)) =
                                                object_for_subject_predicate(
                                                    &self.graph,
                                                    &node,
                                                    rdfs::LABEL,
                                                )
                                            {
                                                Some((
                                                    NamedNode::new(name.value()).unwrap(),
                                                    graph.into_string(),
                                                ))
                                            } else {
                                                Some((graph.clone(), graph.into_string()))
                                            }
                                        } else {
                                            None
                                        }
                                    }
                                    _ => None,
                                })
                                .collect(),
                        ),
                        Some(_) => return Some(Err(anyhow!("invalid result"))),
                        None => (None, Vec::new()),
                    };
                Some(Ok(Test {
                    id: test_node,
                    kind,
                    name,
                    comment,
                    action,
                    query,
                    update,
                    data,
                    graph_data,
                    service_data,
                    result,
                    result_graph_data,
                }))
            }
            Some(_) => self.next(),
            None => {
                match self.manifests_to_do.pop() {
                    Some(url) => {
                        let manifest =
                            NamedOrBlankNodeRef::from(NamedNodeRef::new(url.as_str()).unwrap());
                        if let Err(error) =
                            load_to_store(&url, &self.graph, GraphNameRef::DefaultGraph)
                        {
                            return Some(Err(error));
                        }

                        // New manifests
                        match object_for_subject_predicate(&self.graph, manifest, mf::INCLUDE) {
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
                            Some(_) => return Some(Err(anyhow!("invalid tests list"))),
                            None => (),
                        }

                        // New tests
                        match object_for_subject_predicate(&self.graph, manifest, mf::ENTRIES) {
                            Some(Term::BlankNode(list)) => {
                                self.tests_to_do
                                    .extend(RdfListIterator::iter(&self.graph, list.into()));
                            }
                            Some(term) => {
                                return Some(Err(anyhow!("Invalid tests list. Got term {}", term)));
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
                let result = object_for_subject_predicate(self.graph, &current, rdf::FIRST);
                self.current_node =
                    match object_for_subject_predicate(self.graph, &current, rdf::REST) {
                        Some(Term::NamedNode(n)) if n == rdf::NIL => None,
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

fn object_for_subject_predicate<'a>(
    store: &MemoryStore,
    subject: impl Into<NamedOrBlankNodeRef<'a>>,
    predicate: impl Into<NamedNodeRef<'a>>,
) -> Option<Term> {
    objects_for_subject_predicate(store, subject, predicate).next()
}

fn objects_for_subject_predicate<'a>(
    store: &MemoryStore,
    subject: impl Into<NamedOrBlankNodeRef<'a>>,
    predicate: impl Into<NamedNodeRef<'a>>,
) -> impl Iterator<Item = Term> {
    store
        .quads_for_pattern(Some(subject.into()), Some(predicate.into()), None, None)
        .map(|t| t.object)
}
