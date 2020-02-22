use criterion::{criterion_group, criterion_main, Criterion};
use failure::format_err;
use oxigraph::model::vocab::rdf;
use oxigraph::model::*;
use oxigraph::sparql::*;
use oxigraph::*;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;

criterion_group!(sparql, sparql_w3c_syntax_bench);

criterion_main!(sparql);

fn sparql_w3c_syntax_bench(c: &mut Criterion) {
    let manifest_urls = vec![
        "http://www.w3.org/2001/sw/DataAccess/tests/data-r2/manifest-syntax.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/syntax-query/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/syntax-fed/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/construct/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/grouping/manifest.ttl",
        "http://www.w3.org/2009/sparql/docs/tests/data-sparql11/aggregates/manifest.ttl",
    ];
    let queries: Vec<_> = manifest_urls
        .into_iter()
        .flat_map(TestManifest::new)
        .flat_map(|test| {
            let test = test.unwrap();
            if test.kind == "PositiveSyntaxTest" || test.kind == "PositiveSyntaxTest11" {
                Some((read_file_to_string(&test.query).unwrap(), test.query))
            } else {
                None
            }
        })
        .collect();

    c.bench_function("query parser", |b| {
        b.iter(|| {
            for (query, base) in &queries {
                Query::parse(query, Some(base)).unwrap();
            }
        })
    });
}

fn to_relative_path(url: &str) -> Result<String> {
    if url.starts_with("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/") {
        Ok(url.replace(
            "http://www.w3.org/2001/sw/DataAccess/tests/",
            "rdf-tests/sparql11/",
        ))
    } else if url.starts_with("http://www.w3.org/2009/sparql/docs/tests/data-sparql11/") {
        Ok(url.replace(
            "http://www.w3.org/2009/sparql/docs/tests/",
            "rdf-tests/sparql11/",
        ))
    } else {
        Err(format_err!("Not supported url for file: {}", url))
    }
}

fn read_file(url: &str) -> Result<impl BufRead> {
    let mut base_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    base_path.push("tests");
    base_path.push(to_relative_path(url)?);

    Ok(BufReader::new(File::open(&base_path).map_err(|e| {
        format_err!("Opening file {} failed with {}", base_path.display(), e)
    })?))
}

fn read_file_to_string(url: &str) -> Result<String> {
    let mut string = String::default();
    read_file(url)?.read_to_string(&mut string)?;
    Ok(string)
}

fn load_graph(url: &str) -> Result<SimpleGraph> {
    let repository = MemoryRepository::default();
    load_graph_to_repository(url, &mut repository.connection().unwrap(), None)?;
    Ok(repository
        .connection()
        .unwrap()
        .quads_for_pattern(None, None, None, Some(None))
        .map(|q| q.unwrap().into_triple())
        .collect())
}

fn load_graph_to_repository(
    url: &str,
    connection: &mut <&MemoryRepository as Repository>::Connection,
    to_graph_name: Option<&NamedOrBlankNode>,
) -> Result<()> {
    let syntax = if url.ends_with(".nt") {
        GraphSyntax::NTriples
    } else if url.ends_with(".ttl") {
        GraphSyntax::Turtle
    } else if url.ends_with(".rdf") {
        GraphSyntax::RdfXml
    } else {
        return Err(format_err!("Serialization type not found for {}", url));
    };
    connection.load_graph(read_file(url)?, syntax, to_graph_name, Some(url))
}

mod mf {
    use lazy_static::lazy_static;
    use oxigraph::model::NamedNode;

    lazy_static! {
        pub static ref INCLUDE: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#include")
                .unwrap();
        pub static ref ENTRIES: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#entries")
                .unwrap();
        pub static ref ACTION: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#action")
                .unwrap();
        pub static ref RESULT: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-manifest#result")
                .unwrap();
    }
}

mod qt {
    use lazy_static::lazy_static;
    use oxigraph::model::NamedNode;

    lazy_static! {
        pub static ref QUERY: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/test-query#query")
                .unwrap();
    }
}

mod rs {
    use lazy_static::lazy_static;
    use oxigraph::model::NamedNode;

    lazy_static! {
        pub static ref RESULT_SET: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#ResultSet")
                .unwrap();
        pub static ref RESULT_VARIABLE: NamedNode = NamedNode::parse(
            "http://www.w3.org/2001/sw/DataAccess/tests/result-set#resultVariable"
        )
        .unwrap();
        pub static ref SOLUTION: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#solution")
                .unwrap();
        pub static ref BINDING: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#binding")
                .unwrap();
        pub static ref VALUE: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#value")
                .unwrap();
        pub static ref VARIABLE: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#variable")
                .unwrap();
        pub static ref INDEX: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#index")
                .unwrap();
        pub static ref BOOLEAN: NamedNode =
            NamedNode::parse("http://www.w3.org/2001/sw/DataAccess/tests/result-set#boolean")
                .unwrap();
    }
}

struct Test {
    kind: String,
    query: String,
}

struct TestManifest {
    graph: SimpleGraph,
    tests_to_do: Vec<Term>,
    manifests_to_do: Vec<String>,
}

impl TestManifest {
    fn new(url: impl Into<String>) -> TestManifest {
        Self {
            graph: SimpleGraph::default(),
            tests_to_do: Vec::default(),
            manifests_to_do: vec![url.into()],
        }
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
                {
                    Some(Term::NamedNode(c)) => match c.as_str().split('#').last() {
                        Some(k) => k.to_string(),
                        None => return self.next(), //We ignore the test
                    },
                    _ => return self.next(), //We ignore the test
                };
                let query = match self
                    .graph
                    .object_for_subject_predicate(&test_subject, &*mf::ACTION)
                {
                    Some(Term::NamedNode(n)) => n.as_str().to_owned(),
                    Some(Term::BlankNode(n)) => {
                        let n = n.clone().into();
                        match self.graph.object_for_subject_predicate(&n, &qt::QUERY) {
                            Some(Term::NamedNode(q)) => q.as_str().to_owned(),
                            Some(_) => return Some(Err(format_err!("invalid query"))),
                            None => return Some(Err(format_err!("query not found"))),
                        }
                    }
                    Some(_) => return Some(Err(format_err!("invalid action"))),
                    None => {
                        return Some(Err(format_err!(
                            "action not found for test {}",
                            test_subject
                        )));
                    }
                };
                Some(Ok(Test { kind, query }))
            }
            Some(_) => Some(Err(format_err!("invalid test list"))),
            None => {
                match self.manifests_to_do.pop() {
                    Some(url) => {
                        let manifest =
                            NamedOrBlankNode::from(NamedNode::parse(url.clone()).unwrap());
                        match load_graph(&url) {
                            Ok(g) => self.graph.extend(g.into_iter()),
                            Err(e) => return Some(Err(e)),
                        }

                        // New manifests
                        match self
                            .graph
                            .object_for_subject_predicate(&manifest, &*mf::INCLUDE)
                        {
                            Some(Term::BlankNode(list)) => {
                                self.manifests_to_do.extend(
                                    RdfListIterator::iter(&self.graph, list.clone().into())
                                        .filter_map(|m| match m {
                                            Term::NamedNode(nm) => Some(nm.into_string()),
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

struct RdfListIterator<'a> {
    graph: &'a SimpleGraph,
    current_node: Option<NamedOrBlankNode>,
}

impl<'a> RdfListIterator<'a> {
    fn iter(graph: &'a SimpleGraph, root: NamedOrBlankNode) -> RdfListIterator<'a> {
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
                let result = self
                    .graph
                    .object_for_subject_predicate(&current, &rdf::FIRST);
                self.current_node = match self
                    .graph
                    .object_for_subject_predicate(&current, &rdf::REST)
                {
                    Some(Term::NamedNode(ref n)) if *n == *rdf::NIL => None,
                    Some(Term::NamedNode(n)) => Some(n.clone().into()),
                    Some(Term::BlankNode(n)) => Some(n.clone().into()),
                    _ => None,
                };
                result.cloned()
            }
            None => None,
        }
    }
}
