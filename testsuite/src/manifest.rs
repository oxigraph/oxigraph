use crate::files::{guess_rdf_format, load_to_graph, JsonLdLoader};
use crate::vocab::*;
use anyhow::{bail, Context, Result};
use json_ld::rdf_types::generator::Blank;
use json_ld::rdf_types::{LiteralType, Quad};
use json_ld::{IriBuf, JsonLdProcessor, RemoteDocumentReference};
use oxigraph::io::RdfFormat;
use oxigraph::model::vocab::*;
use oxigraph::model::*;
use std::collections::VecDeque;
use std::fmt;
use std::future::Future;
use std::pin::pin;
use std::task::{Poll, Waker};

pub struct Test {
    pub id: NamedNode,
    pub kinds: Vec<NamedNode>,
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
            let Some(test_node) = self.tests_to_do.pop_front() else {
                return Ok(None);
            };
            let Term::NamedNode(test_node) = test_node else {
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
            let kinds = self
                .graph
                .objects_for_subject_predicate(&test_node, rdf::TYPE)
                .map(|c| {
                    if let TermRef::NamedNode(c) = c {
                        Ok(c.into_owned())
                    } else {
                        bail!(
                            "The test {test_node} named {} has no rdf:type",
                            name.as_deref().unwrap_or("")
                        )
                    }
                })
                .collect::<Result<Vec<_>>>()?;
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
                            TermRef::NamedNode(q) => {
                                Some(Ok((q.into_owned(), q.as_str().to_owned())))
                            }
                            TermRef::BlankNode(node) => {
                                if let Some(TermRef::NamedNode(graph)) =
                                    self.graph.object_for_subject_predicate(node, ut::GRAPH)
                                {
                                    Some(Ok(
                                        if let Some(TermRef::Literal(name)) = self
                                            .graph
                                            .object_for_subject_predicate(node, rdfs::LABEL)
                                        {
                                            (
                                                match NamedNode::new(name.value()) {
                                                    Ok(graph) => graph,
                                                    Err(e) => return Some(Err(e)),
                                                },
                                                graph.as_str().to_owned(),
                                            )
                                        } else {
                                            (graph.into_owned(), graph.as_str().to_owned())
                                        },
                                    ))
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        })
                        .collect::<Result<_, _>>()?;
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
                            TermRef::NamedNode(q) => {
                                Some(Ok((q.into_owned(), q.as_str().to_owned())))
                            }
                            TermRef::BlankNode(node) => {
                                if let Some(TermRef::NamedNode(graph)) =
                                    self.graph.object_for_subject_predicate(node, ut::GRAPH)
                                {
                                    Some(Ok(
                                        if let Some(TermRef::Literal(name)) = self
                                            .graph
                                            .object_for_subject_predicate(node, rdfs::LABEL)
                                        {
                                            (
                                                match NamedNode::new(name.value()) {
                                                    Ok(graph) => graph,
                                                    Err(e) => return Some(Err(e)),
                                                },
                                                graph.as_str().to_owned(),
                                            )
                                        } else {
                                            (graph.into_owned(), graph.as_str().to_owned())
                                        },
                                    ))
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        })
                        .collect::<Result<_, _>>()?,
                ),
                Some(TermRef::Literal(l)) => (Some(l.value().to_owned()), Vec::new()),
                Some(_) => bail!("invalid result"),
                None => (None, Vec::new()),
            };
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
            }));
        }
    }

    fn load_next_manifest(&mut self) -> Result<Option<()>> {
        let Some(url) = self.manifests_to_do.pop_front() else {
            return Ok(None);
        };
        self.graph.clear();
        let format = guess_rdf_format(&url)?;
        if format == RdfFormat::JsonLd {
            // TODO: hack to support JSON-Ld manifests
            let mut generator = Blank::new();
            let document = RemoteDocumentReference::iri(IriBuf::new(url.clone())?);
            let Poll::Ready(mut rdf) = pin!(document.to_rdf(&mut generator, &JsonLdLoader))
                .poll(&mut std::task::Context::from_waker(Waker::noop()))?
            else {
                bail!("Not ready future when parsing JSON-LD")
            };

            for Quad(s, p, o, _) in rdf.quads() {
                self.graph.insert(TripleRef {
                    subject: if s.is_iri() {
                        NamedNodeRef::new_unchecked(s.as_str()).into()
                    } else {
                        BlankNodeRef::new_unchecked(s.as_str()).into()
                    },
                    predicate: NamedNodeRef::new_unchecked(p.as_str()),
                    object: if let Some(o) = o.as_iri() {
                        NamedNodeRef::new_unchecked(o.as_str()).into()
                    } else if let Some(o) = o.as_blank() {
                        BlankNodeRef::new_unchecked(o.as_str()).into()
                    } else if let Some(o) = o.as_literal() {
                        match &o.type_ {
                            LiteralType::Any(t) => LiteralRef::new_typed_literal(
                                o.as_str(),
                                NamedNodeRef::new_unchecked(t.as_str()),
                            ),
                            LiteralType::LangString(l) => {
                                LiteralRef::new_language_tagged_literal_unchecked(
                                    o.as_value(),
                                    l.as_str(),
                                )
                            }
                        }
                        .into()
                    } else {
                        unreachable!()
                    },
                });
            }
        } else {
            load_to_graph(&url, &mut self.graph, guess_rdf_format(&url)?, None, false)?;
        }

        let manifests = self
            .graph
            .subjects_for_predicate_object(rdf::TYPE, mf::MANIFEST)
            .collect::<Vec<_>>();
        if manifests.len() != 1 {
            bail!("The file should contain a single manifest");
        }
        let mut manifest = manifests[0];
        if let Some(base_iri) = self
            .graph
            .object_for_subject_predicate(manifest, mf::ASSUMED_TEST_BASE)
        {
            let Term::NamedNode(base_iri) = base_iri.into_owned() else {
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
                .subject_for_predicate_object(rdf::TYPE, mf::MANIFEST)
                .context("no manifest found")?;
        }

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

impl Iterator for RdfListIterator<'_> {
    type Item = Term;

    fn next(&mut self) -> Option<Self::Item> {
        match self.current_node {
            Some(current) => {
                let result = self
                    .graph
                    .object_for_subject_predicate(current, rdf::FIRST)
                    .map(TermRef::into_owned);
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
