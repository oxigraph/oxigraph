use crate::QueryDatasetSpecification;
use crate::dataset::QueryableDataset;
use oxrdf::{GraphName, NamedNodeRef, Term, TermRef};
use rustc_hash::FxHashMap;
use sparopt::{GraphScope, PredicateStatistics, Statistics};
use std::cell::{OnceCell, RefCell};
use std::marker::PhantomData;

type QuadCountKey = (Option<Term>, Option<Term>, Option<Term>, GraphKey);

/// Exposes the statistics of a [`QueryableDataset`] to the optimizer, in the scope of the query dataset
pub struct DatasetStatistics<'b, 'a, D: QueryableDataset<'a>> {
    dataset: &'b D,
    /// The graphs of the query default graph, `None` for the union of all the graphs
    default_graph: Option<Vec<Option<D::InternalTerm>>>,
    /// The named graphs of the query, `None` for all the named graphs
    named_graphs: Option<Vec<D::InternalTerm>>,
    /// If the query dataset failed to be encoded
    is_broken: bool,
    // The optimizer asks many times the same thing
    quad_counts: RefCell<FxHashMap<QuadCountKey, Option<u64>>>,
    predicate_statistics: RefCell<FxHashMap<(Term, GraphKey), Option<PredicateStatistics>>>,
    // Internalizing a term might need a lookup in the dataset
    internal_terms: RefCell<FxHashMap<Term, Option<D::InternalTerm>>>,
    named_graph_count: OnceCell<Option<u64>>,
    _lifetime: PhantomData<&'a ()>,
}

impl<'b, 'a, D: QueryableDataset<'a>> DatasetStatistics<'b, 'a, D> {
    pub fn new(dataset: &'b D, specification: &QueryDatasetSpecification) -> Self {
        let default_graph = specification
            .default
            .as_ref()
            .map(|graph_names| {
                graph_names
                    .iter()
                    .map(|graph_name| match graph_name {
                        GraphName::NamedNode(n) => {
                            dataset.internalize_term(n.clone().into()).map(Some)
                        }
                        GraphName::BlankNode(n) => {
                            dataset.internalize_term(n.clone().into()).map(Some)
                        }
                        GraphName::DefaultGraph => Ok(None),
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose();
        let named_graphs = specification
            .named
            .as_ref()
            .map(|graph_names| {
                graph_names
                    .iter()
                    .map(|graph_name| dataset.internalize_term(graph_name.clone().into()))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose();
        let (default_graph, named_graphs, is_broken) = match (default_graph, named_graphs) {
            (Ok(default_graph), Ok(named_graphs)) => (default_graph, named_graphs, false),
            _ => (None, None, true),
        };
        Self {
            dataset,
            default_graph,
            named_graphs,
            is_broken,
            quad_counts: RefCell::default(),
            predicate_statistics: RefCell::default(),
            internal_terms: RefCell::default(),
            named_graph_count: OnceCell::new(),
            _lifetime: PhantomData,
        }
    }

    fn internalize_term(&self, term: TermRef<'_>) -> Option<D::InternalTerm> {
        if let Some(internal) = self.internal_terms.borrow().get(&term.into_owned()) {
            return internal.clone();
        }
        let internal = self.dataset.internalize_term(term.into_owned()).ok();
        self.internal_terms
            .borrow_mut()
            .insert(term.into_owned(), internal.clone());
        internal
    }

    /// The dataset graphs matching the scope, in the format of [`QueryableDataset::internal_quad_count_estimate`]
    fn graphs(&self, graph: GraphScope<'_>) -> Option<Vec<Option<Option<D::InternalTerm>>>> {
        if self.is_broken {
            return None;
        }
        Some(match graph {
            GraphScope::DefaultGraph => {
                if let Some(default_graph) = &self.default_graph {
                    default_graph.iter().map(|g| Some(g.clone())).collect()
                } else {
                    // Union of all the graphs
                    vec![Some(None), None]
                }
            }
            GraphScope::NamedGraph(graph_name) => {
                let graph_name = self.internalize_term(graph_name.into())?;
                if self
                    .named_graphs
                    .as_ref()
                    .is_some_and(|named_graphs| !named_graphs.contains(&graph_name))
                {
                    Vec::new()
                } else {
                    vec![Some(Some(graph_name))]
                }
            }
            GraphScope::AllNamedGraphs => {
                if let Some(named_graphs) = &self.named_graphs {
                    named_graphs.iter().map(|g| Some(Some(g.clone()))).collect()
                } else {
                    vec![None]
                }
            }
        })
    }

    fn compute_quad_count(
        &self,
        subject: Option<TermRef<'_>>,
        predicate: Option<NamedNodeRef<'_>>,
        object: Option<TermRef<'_>>,
        graph: GraphScope<'_>,
    ) -> Option<u64> {
        let subject = match subject {
            Some(subject) => Some(self.internalize_term(subject)?),
            None => None,
        };
        let predicate = match predicate {
            Some(predicate) => Some(self.internalize_term(predicate.into())?),
            None => None,
        };
        let object = match object {
            Some(object) => Some(self.internalize_term(object)?),
            None => None,
        };
        let mut count = 0_u64;
        for graph_name in self.graphs(graph)? {
            count = count.saturating_add(
                self.dataset
                    .internal_quad_count_estimate(
                        subject.as_ref(),
                        predicate.as_ref(),
                        object.as_ref(),
                        graph_name.as_ref().map(Option::as_ref),
                    )
                    .ok()??,
            );
        }
        Some(count)
    }

    fn compute_predicate_statistics(
        &self,
        predicate: NamedNodeRef<'_>,
        graph: GraphScope<'_>,
    ) -> Option<PredicateStatistics> {
        let predicate = self.internalize_term(predicate.into())?;
        let mut statistics = PredicateStatistics {
            triples: 0,
            distinct_subjects: 0,
            distinct_objects: 0,
        };
        for graph_name in self.graphs(graph)? {
            let graph_statistics = self
                .dataset
                .internal_predicate_statistics(&predicate, graph_name.as_ref().map(Option::as_ref))
                .ok()??;
            // The sum of the distinct counts is an upper bound
            statistics.triples = statistics.triples.saturating_add(graph_statistics.triples);
            statistics.distinct_subjects = statistics
                .distinct_subjects
                .saturating_add(graph_statistics.distinct_subjects);
            statistics.distinct_objects = statistics
                .distinct_objects
                .saturating_add(graph_statistics.distinct_objects);
        }
        Some(statistics)
    }
}

impl<'a, D: QueryableDataset<'a>> Statistics for DatasetStatistics<'_, 'a, D> {
    fn quad_count(
        &self,
        subject: Option<TermRef<'_>>,
        predicate: Option<NamedNodeRef<'_>>,
        object: Option<TermRef<'_>>,
        graph: GraphScope<'_>,
    ) -> Option<u64> {
        let key = (
            subject.map(TermRef::into_owned),
            predicate.map(|p| p.into_owned().into()),
            object.map(TermRef::into_owned),
            GraphKey::from(graph),
        );
        if let Some(count) = self.quad_counts.borrow().get(&key) {
            return *count;
        }
        let count = self.compute_quad_count(subject, predicate, object, graph);
        self.quad_counts.borrow_mut().insert(key, count);
        count
    }

    fn predicate_statistics(
        &self,
        predicate: NamedNodeRef<'_>,
        graph: GraphScope<'_>,
    ) -> Option<PredicateStatistics> {
        let key = (predicate.into_owned().into(), GraphKey::from(graph));
        if let Some(statistics) = self.predicate_statistics.borrow().get(&key) {
            return *statistics;
        }
        let statistics = self.compute_predicate_statistics(predicate, graph);
        self.predicate_statistics
            .borrow_mut()
            .insert(key, statistics);
        statistics
    }

    fn distinct_predicate_count(&self, graph: GraphScope<'_>) -> Option<u64> {
        // The maximum is a lower bound
        let mut count = 0;
        for graph_name in self.graphs(graph)? {
            count = count.max(
                self.dataset
                    .internal_distinct_predicate_count_estimate(
                        graph_name.as_ref().map(Option::as_ref),
                    )
                    .ok()??,
            );
        }
        Some(count)
    }

    fn named_graph_count(&self) -> Option<u64> {
        if self.is_broken {
            return None;
        }
        if let Some(named_graphs) = &self.named_graphs {
            return named_graphs.len().try_into().ok();
        }
        *self.named_graph_count.get_or_init(|| {
            self.dataset
                .internal_named_graph_count_estimate()
                .ok()
                .flatten()
        })
    }
}

#[derive(Eq, PartialEq, Hash)]
enum GraphKey {
    DefaultGraph,
    NamedGraph(Term),
    AllNamedGraphs,
}

impl From<GraphScope<'_>> for GraphKey {
    fn from(graph: GraphScope<'_>) -> Self {
        match graph {
            GraphScope::DefaultGraph => Self::DefaultGraph,
            GraphScope::NamedGraph(graph_name) => Self::NamedGraph(graph_name.into_owned().into()),
            GraphScope::AllNamedGraphs => Self::AllNamedGraphs,
        }
    }
}
