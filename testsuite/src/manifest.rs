use crate::files::{guess_graph_format, load_to_graph};
use crate::vocab::*;
use anyhow::{bail, Result};
use oxigraph::model::vocab::*;
use oxigraph::model::*;
use std::collections::VecDeque;
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
            write!(f, " named \"{name}\"")?;
        }
        for comment in &self.comment {
            write!(f, " with comment \"{comment}\"")?;
        }
        if let Some(action) = &self.action {
            write!(f, " on file \"{action}\"")?;
        }
        if let Some(query) = &self.query {
            write!(f, " on query {}", &query)?;
        }
        for data in &self.data {
            write!(f, " with data {data}")?;
        }
        for (_, data) in &self.graph_data {
            write!(f, " and graph data {data}")?;
        }
        for result in &self.result {
            write!(f, " and expected result {result}")?;
        }
        Ok(())
    }
}

pub struct TestManifest {
    graph: Graph,
    tests_to_do: VecDeque<Term>,
    manifests_to_do: VecDeque<String>,
}

impl Iterator for TestManifest {
    type Item = Result<Test>;

    fn next(&mut self) -> Option<Result<Test>> {
        loop {
            if let Some(next) = self.next_test().transpose() {
                return Some(next);
            }
            if let Err(e) = self.load_next_manifest().transpose()? {
                return Some(Err(e));
            }
        }
    }
}

impl TestManifest {
    pub fn new<S: ToString>(manifest_urls: impl IntoIterator<Item = S>) -> Self {
        Self {
            graph: Graph::new(),
            tests_to_do: VecDeque::new(),
            manifests_to_do: manifest_urls
                .into_iter()
                .map(|url| url.to_string())
                .collect(),
        }
    }

    fn next_test(&mut self) -> Result<Option<Test>> {
        loop {
            let test_node = if let Some(test_node) = self.tests_to_do.pop_front() {
                test_node
            } else {
                return Ok(None);
            };
            let test_node = if let Term::NamedNode(test_node) = test_node {
                test_node
            } else {
                bail!("Invalid test identifier. Got {test_node}");
            };

            if self
                .graph
                .contains(TripleRef::new(&test_node, rdft::APPROVAL, rdft::REJECTED))
            {
                continue; // We do not run rejected tests
            }
            let name = if let Some(TermRef::Literal(c)) = self
                .graph
                .object_for_subject_predicate(&test_node, mf::NAME)
            {
                Some(c.value().to_owned())
            } else {
                None
            };
            let kind = if let Some(TermRef::NamedNode(c)) = self
                .graph
                .object_for_subject_predicate(&test_node, rdf::TYPE)
            {
                c.into_owned()
            } else {
                bail!(
                    "The test {test_node} named {} has no rdf:type",
                    name.as_deref().unwrap_or("")
                );
            };
            let comment = if let Some(TermRef::Literal(c)) = self
                .graph
                .object_for_subject_predicate(&test_node, rdfs::COMMENT)
            {
                Some(c.value().to_owned())
            } else {
                None
            };
            let (action, query, update, data, graph_data, service_data) = match self
                .graph
                .object_for_subject_predicate(&test_node, mf::ACTION)
            {
                Some(TermRef::NamedNode(n)) => (
                    Some(n.as_str().to_owned()),
                    None,
                    None,
                    None,
                    vec![],
                    vec![],
                ),
                Some(TermRef::BlankNode(n)) => {
                    let query = match self.graph.object_for_subject_predicate(n, qt::QUERY) {
                        Some(TermRef::NamedNode(q)) => Some(q.as_str().to_owned()),
                        _ => None,
                    };
                    let update = match self.graph.object_for_subject_predicate(n, ut::REQUEST) {
                        Some(TermRef::NamedNode(q)) => Some(q.as_str().to_owned()),
                        _ => None,
                    };
                    let data = match self
                        .graph
                        .object_for_subject_predicate(n, qt::DATA)
                        .or_else(|| self.graph.object_for_subject_predicate(n, ut::DATA))
                    {
                        Some(TermRef::NamedNode(q)) => Some(q.as_str().to_owned()),
                        _ => None,
                    };
                    let graph_data = self
                        .graph
                        .objects_for_subject_predicate(n, qt::GRAPH_DATA)
                        .chain(self.graph.objects_for_subject_predicate(n, ut::GRAPH_DATA))
                        .filter_map(|g| match g {
                            TermRef::NamedNode(q) => Some((q.into_owned(), q.as_str().to_owned())),
                            TermRef::BlankNode(node) => {
                                if let Some(TermRef::NamedNode(graph)) =
                                    self.graph.object_for_subject_predicate(node, ut::GRAPH)
                                {
                                    if let Some(TermRef::Literal(name)) =
                                        self.graph.object_for_subject_predicate(node, rdfs::LABEL)
                                    {
                                        Some((
                                            NamedNode::new(name.value()).unwrap(),
                                            graph.as_str().to_owned(),
                                        ))
                                    } else {
                                        Some((graph.into_owned(), graph.as_str().to_owned()))
                                    }
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        })
                        .collect();
                    let service_data = self
                        .graph
                        .objects_for_subject_predicate(n, qt::SERVICE_DATA)
                        .filter_map(|g| match g {
                            TermRef::NamedNode(g) => Some(g.into()),
                            TermRef::BlankNode(g) => Some(g.into()),
                            _ => None,
                        })
                        .filter_map(|g: SubjectRef<'_>| {
                            if let (
                                Some(TermRef::NamedNode(endpoint)),
                                Some(TermRef::NamedNode(data)),
                            ) = (
                                self.graph.object_for_subject_predicate(g, qt::ENDPOINT),
                                self.graph.object_for_subject_predicate(g, qt::DATA),
                            ) {
                                Some((endpoint.as_str().to_owned(), data.as_str().to_owned()))
                            } else {
                                None
                            }
                        })
                        .collect();
                    (None, query, update, data, graph_data, service_data)
                }
                Some(_) => bail!("invalid action"),
                None => {
                    bail!("action not found for test {test_node}");
                }
            };
            let (result, result_graph_data) = match self
                .graph
                .object_for_subject_predicate(&test_node, mf::RESULT)
            {
                Some(TermRef::NamedNode(n)) => (Some(n.as_str().to_owned()), Vec::new()),
                Some(TermRef::BlankNode(n)) => (
                    if let Some(TermRef::NamedNode(result)) =
                        self.graph.object_for_subject_predicate(n, ut::DATA)
                    {
                        Some(result.as_str().to_owned())
                    } else {
                        None
                    },
                    self.graph
                        .objects_for_subject_predicate(n, ut::GRAPH_DATA)
                        .filter_map(|g| match g {
                            TermRef::NamedNode(q) => Some((q.into_owned(), q.as_str().to_owned())),
                            TermRef::BlankNode(node) => {
                                if let Some(TermRef::NamedNode(graph)) =
                                    self.graph.object_for_subject_predicate(node, ut::GRAPH)
                                {
                                    if let Some(TermRef::Literal(name)) =
                                        self.graph.object_for_subject_predicate(node, rdfs::LABEL)
                                    {
                                        Some((
                                            NamedNode::new(name.value()).unwrap(),
                                            graph.as_str().to_owned(),
                                        ))
                                    } else {
                                        Some((graph.into_owned(), graph.as_str().to_owned()))
                                    }
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        })
                        .collect(),
                ),
                Some(_) => bail!("invalid result"),
                None => (None, Vec::new()),
            };
            return Ok(Some(Test {
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
            }));
        }
    }

    fn load_next_manifest(&mut self) -> Result<Option<()>> {
        let url = if let Some(url) = self.manifests_to_do.pop_front() {
            url
        } else {
            return Ok(None);
        };
        self.graph.clear();
        load_to_graph(&url, &mut self.graph, guess_graph_format(&url)?)?;

        let manifests = self
            .graph
            .subjects_for_predicate_object(rdf::TYPE, mf::MANIFEST)
            .collect::<Vec<_>>();
        if manifests.len() != 1 {
            bail!("The file {url} should contain a single manifest");
        }
        for manifest in manifests {
            match self
                .graph
                .object_for_subject_predicate(manifest, mf::INCLUDE)
            {
                Some(TermRef::BlankNode(list)) => {
                    self.manifests_to_do.extend(
                        RdfListIterator::iter(&self.graph, list.into()).filter_map(|m| match m {
                            Term::NamedNode(nm) => Some(nm.into_string()),
                            _ => None,
                        }),
                    );
                }
                Some(_) => bail!("invalid tests list"),
                None => (),
            }

            // New tests
            match self
                .graph
                .object_for_subject_predicate(manifest, mf::ENTRIES)
            {
                Some(TermRef::BlankNode(list)) => {
                    self.tests_to_do
                        .extend(RdfListIterator::iter(&self.graph, list.into()));
                }
                Some(term) => {
                    bail!("Invalid tests list. Got term {term}");
                }
                None => (),
            }
        }
        Ok(Some(()))
    }
}

struct RdfListIterator<'a> {
    graph: &'a Graph,
    current_node: Option<SubjectRef<'a>>,
}

impl<'a> RdfListIterator<'a> {
    fn iter(graph: &'a Graph, root: SubjectRef<'a>) -> RdfListIterator<'a> {
        RdfListIterator {
            graph,
            current_node: Some(root),
        }
    }
}

impl<'a> Iterator for RdfListIterator<'a> {
    type Item = Term;

    fn next(&mut self) -> Option<Term> {
        match self.current_node {
            Some(current) => {
                let result = self
                    .graph
                    .object_for_subject_predicate(current, rdf::FIRST)
                    .map(|v| v.into_owned());
                self.current_node =
                    match self.graph.object_for_subject_predicate(current, rdf::REST) {
                        Some(TermRef::NamedNode(n)) if n == rdf::NIL => None,
                        Some(TermRef::NamedNode(n)) => Some(n.into()),
                        Some(TermRef::BlankNode(n)) => Some(n.into()),
                        _ => None,
                    };
                result
            }
            None => None,
        }
    }
}
