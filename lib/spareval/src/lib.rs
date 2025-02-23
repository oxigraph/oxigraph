#![doc = include_str!("../README.md")]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

mod dataset;
mod error;
mod eval;
mod model;
mod service;
#[cfg(feature = "rdf-star")]
pub use crate::dataset::ExpressionTriple;
pub use crate::dataset::{ExpressionTerm, InternalQuad, QueryableDataset};
pub use crate::error::QueryEvaluationError;
use crate::eval::{EvalNodeWithStats, SimpleEvaluator, Timer};
pub use crate::model::{QueryResults, QuerySolution, QuerySolutionIter, QueryTripleIter};
use crate::service::ServiceHandlerRegistry;
pub use crate::service::{DefaultServiceHandler, ServiceHandler};
use json_event_parser::{JsonEvent, WriterJsonSerializer};
use oxrdf::{NamedNode, Term, Variable};
use oxsdatatypes::{DayTimeDuration, Float};
use spargebra::Query;
use sparopt::algebra::GraphPattern;
use sparopt::Optimizer;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::{fmt, io};

/// Evaluates a query against a given [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset)
///
/// Note that this evaluator does not handle the `FROM` and `FROM NAMED` part of the query.
/// You must select the proper dataset before using this struct.
///
/// To adapt this software to work on your own RDF dataset, you need to implement the [`QueryableDataset`] trait.
///
/// ```
/// use oxrdf::{Dataset, GraphName, NamedNode, Quad};
/// use spareval::{QueryEvaluator, QueryResults};
/// use spargebra::Query;
///
/// let ex = NamedNode::new("http://example.com")?;
/// let dataset = Dataset::from_iter([Quad::new(
///     ex.clone(),
///     ex.clone(),
///     ex.clone(),
///     GraphName::DefaultGraph,
/// )]);
/// let query = Query::parse("SELECT * WHERE { ?s ?p ?o }", None)?;
/// let results = QueryEvaluator::new().execute(dataset, &query);
/// if let QueryResults::Solutions(solutions) = results? {
///     let solutions = solutions.collect::<Result<Vec<_>, _>>()?;
///     assert_eq!(solutions.len(), 1);
///     assert_eq!(solutions[0]["s"], ex.into());
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Clone, Default)]
pub struct QueryEvaluator {
    service_handler: ServiceHandlerRegistry,
    custom_functions: CustomFunctionRegistry,
    without_optimizations: bool,
    run_stats: bool,
}

impl QueryEvaluator {
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn execute(
        &self,
        dataset: impl QueryableDataset,
        query: &Query,
    ) -> Result<QueryResults, QueryEvaluationError> {
        self.explain(dataset, query).0
    }

    /// Executes a SPARQL query while substituting some variables with the given values.
    ///
    /// Substitution follows [RDF-dev SEP-0007](https://github.com/w3c/sparql-dev/blob/main/SEP/SEP-0007/sep-0007.md).
    ///
    /// ```
    /// use oxrdf::{Dataset, GraphName, NamedNode, Quad, Variable};
    /// use spareval::{QueryEvaluator, QueryResults};
    /// use spargebra::Query;
    ///
    /// let ex = NamedNode::new("http://example.com")?;
    /// let dataset = Dataset::from_iter([Quad::new(
    ///     ex.clone(),
    ///     ex.clone(),
    ///     ex.clone(),
    ///     GraphName::DefaultGraph,
    /// )]);
    /// let query = Query::parse("SELECT * WHERE { ?s ?p ?o }", None)?;
    /// let results = QueryEvaluator::new().execute_with_substituted_variables(
    ///     dataset,
    ///     &query,
    ///     [(Variable::new("s")?, ex.clone().into())],
    /// );
    /// if let QueryResults::Solutions(solutions) = results? {
    ///     let solutions = solutions.collect::<Result<Vec<_>, _>>()?;
    ///     assert_eq!(solutions.len(), 1);
    ///     assert_eq!(solutions[0]["s"], ex.into());
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn execute_with_substituted_variables(
        &self,
        dataset: impl QueryableDataset,
        query: &Query,
        substitutions: impl IntoIterator<Item = (Variable, Term)>,
    ) -> Result<QueryResults, QueryEvaluationError> {
        self.explain_with_substituted_variables(dataset, query, substitutions)
            .0
    }

    pub fn explain(
        &self,
        dataset: impl QueryableDataset,
        query: &Query,
    ) -> (Result<QueryResults, QueryEvaluationError>, QueryExplanation) {
        self.explain_with_substituted_variables(dataset, query, [])
    }

    pub fn explain_with_substituted_variables(
        &self,
        dataset: impl QueryableDataset,
        query: &Query,
        substitutions: impl IntoIterator<Item = (Variable, Term)>,
    ) -> (Result<QueryResults, QueryEvaluationError>, QueryExplanation) {
        let start_planning = Timer::now();
        let (results, plan_node_with_stats, planning_duration) = match query {
            Query::Select {
                pattern, base_iri, ..
            } => {
                let mut pattern = GraphPattern::from(pattern);
                if !self.without_optimizations {
                    pattern = Optimizer::optimize_graph_pattern(pattern);
                }
                let planning_duration = start_planning.elapsed();
                let (results, explanation) = SimpleEvaluator::new(
                    dataset,
                    base_iri.clone().map(Rc::new),
                    Rc::new(self.service_handler.clone()),
                    Rc::new(self.custom_functions.clone()),
                    self.run_stats,
                )
                .evaluate_select(&pattern, substitutions);
                (
                    results.map(QueryResults::Solutions),
                    explanation,
                    planning_duration,
                )
            }
            Query::Ask {
                pattern, base_iri, ..
            } => {
                let mut pattern = GraphPattern::from(pattern);
                if !self.without_optimizations {
                    pattern = Optimizer::optimize_graph_pattern(pattern);
                }
                let planning_duration = start_planning.elapsed();
                let (results, explanation) = SimpleEvaluator::new(
                    dataset,
                    base_iri.clone().map(Rc::new),
                    Rc::new(self.service_handler.clone()),
                    Rc::new(self.custom_functions.clone()),
                    self.run_stats,
                )
                .evaluate_ask(&pattern, substitutions);
                (
                    results.map(QueryResults::Boolean),
                    explanation,
                    planning_duration,
                )
            }
            Query::Construct {
                template,
                pattern,
                base_iri,
                ..
            } => {
                let mut pattern = GraphPattern::from(pattern);
                if !self.without_optimizations {
                    pattern = Optimizer::optimize_graph_pattern(pattern);
                }
                let planning_duration = start_planning.elapsed();
                let (results, explanation) = SimpleEvaluator::new(
                    dataset,
                    base_iri.clone().map(Rc::new),
                    Rc::new(self.service_handler.clone()),
                    Rc::new(self.custom_functions.clone()),
                    self.run_stats,
                )
                .evaluate_construct(&pattern, template, substitutions);
                (
                    results.map(QueryResults::Graph),
                    explanation,
                    planning_duration,
                )
            }
            Query::Describe {
                pattern, base_iri, ..
            } => {
                let mut pattern = GraphPattern::from(pattern);
                if !self.without_optimizations {
                    pattern = Optimizer::optimize_graph_pattern(pattern);
                }
                let planning_duration = start_planning.elapsed();
                let (results, explanation) = SimpleEvaluator::new(
                    dataset,
                    base_iri.clone().map(Rc::new),
                    Rc::new(self.service_handler.clone()),
                    Rc::new(self.custom_functions.clone()),
                    self.run_stats,
                )
                .evaluate_describe(&pattern, substitutions);
                (
                    results.map(QueryResults::Graph),
                    explanation,
                    planning_duration,
                )
            }
        };
        let explanation = QueryExplanation {
            inner: plan_node_with_stats,
            with_stats: self.run_stats,
            planning_duration,
        };
        (results, explanation)
    }

    /// Use a given [`ServiceHandler`] to execute [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE calls.
    ///
    /// See [`ServiceHandler`] for an example.
    #[inline]
    #[must_use]
    pub fn with_service_handler(
        mut self,
        service_name: impl Into<NamedNode>,
        handler: impl ServiceHandler + 'static,
    ) -> Self {
        self.service_handler = self
            .service_handler
            .with_handler(service_name.into(), handler);
        self
    }

    /// Use a given [`DefaultServiceHandler`] to execute [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE calls if no explicit service handler is defined for the service.
    ///
    /// See [`DefaultServiceHandler`] for an example.
    #[inline]
    #[must_use]
    pub fn with_default_service_handler(
        mut self,
        handler: impl DefaultServiceHandler + 'static,
    ) -> Self {
        self.service_handler = self.service_handler.with_default_handler(handler);
        self
    }

    #[inline]
    #[must_use]
    pub fn has_default_service_handler(&self) -> bool {
        self.service_handler.has_default_handler()
    }

    /// Adds a custom SPARQL evaluation function.
    ///
    /// Example with a function serializing terms to N-Triples:
    /// ```
    /// use oxrdf::{Dataset, Literal, NamedNode};
    /// use spareval::{QueryEvaluator, QueryResults};
    /// use spargebra::Query;
    ///
    /// let evaluator = QueryEvaluator::new().with_custom_function(
    ///     NamedNode::new("http://www.w3.org/ns/formats/N-Triples")?,
    ///     |args| args.get(0).map(|t| Literal::from(t.to_string()).into()),
    /// );
    /// let query = Query::parse(
    ///     "SELECT (<http://www.w3.org/ns/formats/N-Triples>(1) AS ?nt) WHERE {}",
    ///     None,
    /// )?;
    /// if let QueryResults::Solutions(mut solutions) = evaluator.execute(Dataset::new(), &query)? {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("nt"),
    ///         Some(&Literal::from("\"1\"^^<http://www.w3.org/2001/XMLSchema#integer>").into())
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    #[must_use]
    pub fn with_custom_function(
        mut self,
        name: NamedNode,
        evaluator: impl Fn(&[Term]) -> Option<Term> + Send + Sync + 'static,
    ) -> Self {
        self.custom_functions.insert(name, Arc::new(evaluator));
        self
    }

    /// Disables query optimizations and runs the query as it is.
    #[inline]
    #[must_use]
    pub fn without_optimizations(mut self) -> Self {
        self.without_optimizations = true;
        self
    }

    /// Compute statistics during evaluation and fills them in the explanation tree.
    #[inline]
    #[must_use]
    pub fn compute_statistics(mut self) -> Self {
        self.run_stats = true;
        self
    }
}

pub(crate) type CustomFunctionRegistry =
    HashMap<NamedNode, Arc<dyn (Fn(&[Term]) -> Option<Term>) + Send + Sync>>;

/// The explanation of a query.
#[derive(Clone)]
pub struct QueryExplanation {
    inner: Rc<EvalNodeWithStats>,
    with_stats: bool,
    planning_duration: Option<DayTimeDuration>,
}

impl QueryExplanation {
    /// Writes the explanation as JSON.
    pub fn write_in_json(&self, writer: impl io::Write) -> io::Result<()> {
        let mut serializer = WriterJsonSerializer::new(writer);
        serializer.serialize_event(JsonEvent::StartObject)?;
        if let Some(planning_duration) = self.planning_duration {
            serializer
                .serialize_event(JsonEvent::ObjectKey("planning duration in seconds".into()))?;
            serializer.serialize_event(JsonEvent::Number(
                planning_duration.as_seconds().to_string().into(),
            ))?;
        }
        serializer.serialize_event(JsonEvent::ObjectKey("plan".into()))?;
        self.inner.json_node(&mut serializer, self.with_stats)?;
        serializer.serialize_event(JsonEvent::EndObject)
    }
}

impl fmt::Debug for QueryExplanation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut obj = f.debug_struct("QueryExplanation");
        if let Some(planning_duration) = self.planning_duration {
            obj.field(
                "planning duration in seconds",
                &f32::from(Float::from(planning_duration.as_seconds())),
            );
        }
        obj.field("tree", &self.inner);
        obj.finish_non_exhaustive()
    }
}
