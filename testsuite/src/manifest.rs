use crate::files::{guess_rdf_format, load_to_graph};
use crate::vocab::*;
use anyhow::{Context, Result, bail};
use oxigraph::model::vocab::*;
use oxigraph::model::*;
use std::collections::{HashMap, VecDeque};
use std::fmt;

pub struct Test {
    pub id: NamedNode,
    pub kinds: Vec<NamedNode>,
    pub name: Option<OxString>,
    pub comment: Option<OxString>,
    pub action: Option<OxString>,
    pub query: Option<OxString>,
    pub update: Option<OxString>,
    pub data: Option<OxString>,
    pub graph_data: Vec<(NamedNode, OxString)>,
    pub service_data: Vec<(OxString, OxString)>,
    pub result: Option<OxString>,
    pub result_graph_data: Vec<(NamedNode, OxString)>,
    pub option: HashMap<NamedNode, Term>,
}

impl fmt::Display for Test {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for kind in &self.kinds {
            write!(f, "{kind}")?;
        }
        if let Some(name) = &self.name {
            write!(f, " named \"{name}\"")?;
        }
        if let Some(comment) = &self.comment {
            write!(f, " with comment \"{comment}\"")?;
        }
        if let Some(action) = &self.action {
            write!(f, " on file \"{action}\"")?;
        }
        if let Some(query) = &self.query {
            write!(f, " on query {query}")?;
        }
        if let Some(data) = &self.data {
            write!(f, " with data {data}")?;
        }
        for (_, data) in &self.graph_data {
            write!(f, " and graph data {data}")?;
        }
        if let Some(result) = &self.result {
            write!(f, " and expected result {result}")?;
        }
        for (k, v) in &self.option {
            write!(f, " and option {k} set to {v}")?;
        }
        Ok(())
    }
}

pub struct TestManifest {
    graph: Graph,
    tests_to_do: VecDeque<Term>,
    manifests_to_do: VecDeque<OxString>,
}

impl Iterator for TestManifest {
    type Item = Result<Test>;

    fn next(&mut self) -> Option<Self::Item> {
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
    pub fn new<S: AsRef<str>>(manifest_urls: impl IntoIterator<Item = S>) -> Self {
        Self {
            graph: Graph::new(),
            tests_to_do: VecDeque::new(),
            manifests_to_do: manifest_urls
                .into_iter()
                .map(|url| OxString::new_owned(url.as_ref()))
                .collect(),
        }
    }

    fn next_test(&mut self) -> Result<Option<Test>> {
        loop {
            let Some(test_node) = self.tests_to_do.pop_front() else {
                return Ok(None);
            };
            let Term::NamedNode(test_node) = test_node else {
                bail!("Invalid test identifier. Got {test_node}");
            };

            if self
                .graph
                .contains(TripleRef::new(&test_node, &rdft::APPROVAL, &rdft::REJECTED))
            {
                continue; // We do not run rejected tests
            }
            let name = if let Some(Term::Literal(c)) = self
                .graph
                .object_for_subject_predicate(&test_node, &mf::NAME)
            {
                Some(c.into_value())
            } else {
                None
            };
            let kinds = self
                .graph
                .objects_for_subject_predicate(&test_node, &rdf::TYPE)
                .map(|c| {
                    if let Term::NamedNode(c) = c {
                        Ok(c)
                    } else {
                        bail!(
                            "The test {test_node} named {} has no rdf:type",
                            name.as_deref().unwrap_or("")
                        )
                    }
                })
                .collect::<Result<Vec<_>>>()?;
            let comment = if let Some(Term::Literal(c)) = self
                .graph
                .object_for_subject_predicate(&test_node, &rdfs::COMMENT)
            {
                Some(c.into_value())
            } else {
                None
            };
            let (action, query, update, data, graph_data, service_data) = match self
                .graph
                .object_for_subject_predicate(&test_node, &mf::ACTION)
            {
                Some(Term::NamedNode(n)) => {
                    (Some(n.into_string()), None, None, None, vec![], vec![])
                }
                Some(Term::BlankNode(n)) => {
                    let query = match self.graph.object_for_subject_predicate(&n, &qt::QUERY) {
                        Some(Term::NamedNode(q)) => Some(q.into_string()),
                        _ => None,
                    };
                    let update = match self.graph.object_for_subject_predicate(&n, &ut::REQUEST) {
                        Some(Term::NamedNode(q)) => Some(q.into_string()),
                        _ => None,
                    };
                    let data = match self
                        .graph
                        .object_for_subject_predicate(&n, &qt::DATA)
                        .or_else(|| self.graph.object_for_subject_predicate(&n, &ut::DATA))
                    {
                        Some(Term::NamedNode(q)) => Some(q.into_string()),
                        _ => None,
                    };
                    let graph_data = self
                        .graph
                        .objects_for_subject_predicate(&n, &qt::GRAPH_DATA)
                        .chain(
                            self.graph
                                .objects_for_subject_predicate(&n, &ut::GRAPH_DATA),
                        )
                        .filter_map(|g| match g {
                            Term::NamedNode(q) => {
                                let q_str = q.clone().into_string();
                                Some(Ok((q, q_str)))
                            }
                            Term::BlankNode(node) => {
                                if let Some(Term::NamedNode(graph)) =
                                    self.graph.object_for_subject_predicate(&node, &ut::GRAPH)
                                {
                                    Some(Ok(
                                        if let Some(Term::Literal(name)) = self
                                            .graph
                                            .object_for_subject_predicate(&node, &rdfs::LABEL)
                                        {
                                            (
                                                match NamedNode::new(name.into_value()) {
                                                    Ok(graph) => graph,
                                                    Err(e) => return Some(Err(e)),
                                                },
                                                graph.clone().into_string(),
                                            )
                                        } else {
                                            let graph_str = graph.clone().into_string();
                                            (graph, graph_str)
                                        },
                                    ))
                                } else {
                                    None
                                }
                            }
                            Term::Literal(_) => None,
                            #[cfg(feature = "rdf-12")]
                            Term::Triple(_) => None,
                        })
                        .collect::<Result<_, _>>()?;
                    let service_data =
                        self.graph
                            .objects_for_subject_predicate(&n, &qt::SERVICE_DATA)
                            .filter_map(|g| match g {
                                Term::NamedNode(g) => Some(g.into()),
                                Term::BlankNode(g) => Some(g.into()),
                                Term::Literal(_) => None,
                                #[cfg(feature = "rdf-12")]
                                Term::Triple(_) => None,
                            })
                            .filter_map(|g: NamedOrBlankNode| {
                                if let (
                                    Some(Term::NamedNode(endpoint)),
                                    Some(Term::NamedNode(data)),
                                ) = (
                                    self.graph.object_for_subject_predicate(&g, &qt::ENDPOINT),
                                    self.graph.object_for_subject_predicate(&g, &qt::DATA),
                                ) {
                                    Some((endpoint.into_string(), data.into_string()))
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
                .object_for_subject_predicate(&test_node, &mf::RESULT)
            {
                Some(Term::NamedNode(n)) => (Some(n.into_string()), Vec::new()),
                Some(Term::BlankNode(n)) => (
                    if let Some(Term::NamedNode(result)) =
                        self.graph.object_for_subject_predicate(&n, &ut::DATA)
                    {
                        Some(result.into_string())
                    } else {
                        None
                    },
                    self.graph
                        .objects_for_subject_predicate(&n, &ut::GRAPH_DATA)
                        .filter_map(|g| match g {
                            Term::NamedNode(q) => {
                                let q_str = q.clone().into_string();
                                Some(Ok((q, q_str)))
                            }
                            Term::BlankNode(node) => {
                                if let Some(Term::NamedNode(graph)) =
                                    self.graph.object_for_subject_predicate(&node, &ut::GRAPH)
                                {
                                    Some(Ok(
                                        if let Some(Term::Literal(name)) = self
                                            .graph
                                            .object_for_subject_predicate(&node, &rdfs::LABEL)
                                        {
                                            (
                                                match NamedNode::new(name.into_value()) {
                                                    Ok(graph) => graph,
                                                    Err(e) => return Some(Err(e)),
                                                },
                                                graph.clone().into_string(),
                                            )
                                        } else {
                                            let graph_str = graph.clone().into_string();
                                            (graph, graph_str)
                                        },
                                    ))
                                } else {
                                    None
                                }
                            }
                            Term::Literal(_) => None,
                            #[cfg(feature = "rdf-12")]
                            Term::Triple(_) => None,
                        })
                        .collect::<Result<_, _>>()?,
                ),
                Some(Term::Literal(l)) => (Some(l.into_value()), Vec::new()),
                #[cfg(feature = "rdf-12")]
                Some(Term::Triple(_)) => bail!("invalid result"),
                None => (None, Vec::new()),
            };
            let mut option = match self
                .graph
                .object_for_subject_predicate(&test_node, &jld::OPTION)
            {
                Some(Term::BlankNode(option)) => self
                    .graph
                    .triples_for_subject(&option)
                    .map(|t| (t.predicate, t.object))
                    .collect(),
                Some(_) => bail!("invalid option"),
                None => HashMap::new(),
            };
            if let Some(hash_algorithm) = self
                .graph
                .object_for_subject_predicate(&test_node, &rdfc::HASH_ALGORITHM)
            {
                option.insert(rdfc::HASH_ALGORITHM, hash_algorithm);
            }
            return Ok(Some(Test {
                id: test_node,
                kinds,
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
                option,
            }));
        }
    }

    fn load_next_manifest(&mut self) -> Result<Option<()>> {
        let Some(url) = self.manifests_to_do.pop_front() else {
            return Ok(None);
        };
        self.graph.clear();
        load_to_graph(&url, &mut self.graph, guess_rdf_format(&url)?, None, false)?;

        let manifests = self
            .graph
            .subjects_for_predicate_object(&rdf::TYPE, &mf::MANIFEST)
            .collect::<Vec<_>>();
        if manifests.len() != 1 {
            bail!("The file should contain a single manifest");
        }
        let mut manifest = manifests.into_iter().next().unwrap();
        if let Some(base_iri) = self
            .graph
            .object_for_subject_predicate(&manifest, &mf::ASSUMED_TEST_BASE)
        {
            let Term::NamedNode(base_iri) = base_iri else {
                bail!("Invalid base IRI: {base_iri}");
            };
            self.graph.clear();
            load_to_graph(
                &url,
                &mut self.graph,
                guess_rdf_format(&url)?,
                Some(base_iri.as_str()),
                false,
            )?;
            manifest = self
                .graph
                .subject_for_predicate_object(&rdf::TYPE, &mf::MANIFEST)
                .context("no manifest found")?;
        }

        match self
            .graph
            .object_for_subject_predicate(&manifest, &mf::INCLUDE)
        {
            Some(Term::BlankNode(list)) => {
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
            .object_for_subject_predicate(&manifest, &mf::ENTRIES)
        {
            Some(Term::BlankNode(list)) => {
                self.tests_to_do
                    .extend(RdfListIterator::iter(&self.graph, list.into()));
            }
            Some(term) => {
                bail!("Invalid tests list. Got term {term}");
            }
            None => (),
        }
        Ok(Some(()))
    }
}

struct RdfListIterator<'a> {
    graph: &'a Graph,
    current_node: Option<NamedOrBlankNode>,
}

impl<'a> RdfListIterator<'a> {
    fn iter(graph: &'a Graph, root: NamedOrBlankNode) -> RdfListIterator<'a> {
        RdfListIterator {
            graph,
            current_node: Some(root),
        }
    }
}

impl Iterator for RdfListIterator<'_> {
    type Item = Term;

    fn next(&mut self) -> Option<Self::Item> {
        match &self.current_node {
            Some(current) => {
                let result = self
                    .graph
                    .object_for_subject_predicate(current, &rdf::FIRST);
                self.current_node =
                    match self.graph.object_for_subject_predicate(current, &rdf::REST) {
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
