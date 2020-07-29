//! [SPARQL](https://www.w3.org/TR/sparql11-overview/) implementation.

mod algebra;
mod eval;
mod json_results;
mod model;
mod parser;
mod plan;
mod plan_builder;
mod xml_results;

use crate::model::NamedNode;
use crate::sparql::algebra::QueryVariants;
use crate::sparql::eval::SimpleEvaluator;
use crate::sparql::plan::TripleTemplate;
use crate::sparql::plan::{DatasetView, PlanNode};
use crate::sparql::plan_builder::PlanBuilder;
use crate::store::ReadableEncodedStore;
use crate::Error;
use crate::Result;

pub use crate::sparql::model::QuerySolution;
pub use crate::sparql::model::QuerySolutionsIterator;
pub use crate::sparql::model::QueryTriplesIterator;
#[deprecated(note = "Please directly use QuerySolutionsIterator type instead")]
pub type BindingsIterator<'a> = QuerySolutionsIterator;
pub use crate::sparql::model::QueryResult;
pub use crate::sparql::model::QueryResultSyntax;
pub use crate::sparql::model::Variable;
pub use crate::sparql::parser::Query;
pub use crate::sparql::parser::SparqlParseError;
use std::convert::TryInto;
use std::rc::Rc;

/// A prepared [SPARQL query](https://www.w3.org/TR/sparql11-query/)
#[deprecated(
    note = "Not useful anymore. The exec method is already implemented by the different PreparedQuery structures"
)]
pub trait PreparedQuery {}

/// A prepared [SPARQL query](https://www.w3.org/TR/sparql11-query/)
pub(crate) struct SimplePreparedQuery<S: ReadableEncodedStore + 'static>(
    SimplePreparedQueryAction<S>,
);

#[derive(Clone)]
enum SimplePreparedQueryAction<S: ReadableEncodedStore + 'static> {
    Select {
        plan: Rc<PlanNode>,
        variables: Rc<Vec<Variable>>,
        evaluator: SimpleEvaluator<S>,
    },
    Ask {
        plan: Rc<PlanNode>,
        evaluator: SimpleEvaluator<S>,
    },
    Construct {
        plan: Rc<PlanNode>,
        construct: Rc<Vec<TripleTemplate>>,
        evaluator: SimpleEvaluator<S>,
    },
    Describe {
        plan: Rc<PlanNode>,
        evaluator: SimpleEvaluator<S>,
    },
}

impl<S: ReadableEncodedStore + 'static> SimplePreparedQuery<S> {
    pub(crate) fn new(
        store: S,
        query: impl TryInto<Query, Error = impl Into<Error>>,
        options: QueryOptions,
    ) -> Result<Self> {
        let dataset = Rc::new(DatasetView::new(store, options.default_graph_as_union));
        Ok(Self(match query.try_into().map_err(|e| e.into())?.0 {
            QueryVariants::Select {
                algebra, base_iri, ..
            } => {
                let (plan, variables) = PlanBuilder::build(dataset.encoder(), &algebra)?;
                SimplePreparedQueryAction::Select {
                    plan: Rc::new(plan),
                    variables: Rc::new(variables),
                    evaluator: SimpleEvaluator::new(dataset, base_iri, options.service_handler),
                }
            }
            QueryVariants::Ask {
                algebra, base_iri, ..
            } => {
                let (plan, _) = PlanBuilder::build(dataset.encoder(), &algebra)?;
                SimplePreparedQueryAction::Ask {
                    plan: Rc::new(plan),
                    evaluator: SimpleEvaluator::new(dataset, base_iri, options.service_handler),
                }
            }
            QueryVariants::Construct {
                construct,
                algebra,
                base_iri,
                ..
            } => {
                let (plan, variables) = PlanBuilder::build(dataset.encoder(), &algebra)?;
                SimplePreparedQueryAction::Construct {
                    plan: Rc::new(plan),
                    construct: Rc::new(PlanBuilder::build_graph_template(
                        dataset.encoder(),
                        &construct,
                        variables,
                    )?),
                    evaluator: SimpleEvaluator::new(dataset, base_iri, options.service_handler),
                }
            }
            QueryVariants::Describe {
                algebra, base_iri, ..
            } => {
                let (plan, _) = PlanBuilder::build(dataset.encoder(), &algebra)?;
                SimplePreparedQueryAction::Describe {
                    plan: Rc::new(plan),
                    evaluator: SimpleEvaluator::new(dataset, base_iri, options.service_handler),
                }
            }
        }))
    }

    /// Evaluates the query and returns its results
    pub fn exec(&self) -> Result<QueryResult> {
        match &self.0 {
            SimplePreparedQueryAction::Select {
                plan,
                variables,
                evaluator,
            } => evaluator.evaluate_select_plan(plan, variables.clone()),
            SimplePreparedQueryAction::Ask { plan, evaluator } => evaluator.evaluate_ask_plan(plan),
            SimplePreparedQueryAction::Construct {
                plan,
                construct,
                evaluator,
            } => evaluator.evaluate_construct_plan(plan, construct.clone()),
            SimplePreparedQueryAction::Describe { plan, evaluator } => {
                evaluator.evaluate_describe_plan(plan)
            }
        }
    }
}

/// Handler for SPARQL SERVICEs.
///
/// Might be used to implement [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/)
///
/// ```
/// use oxigraph::{MemoryStore, Result};
/// use oxigraph::model::*;
/// use oxigraph::sparql::{QueryOptions, QueryResult, ServiceHandler, Query};
///
/// #[derive(Default)]
/// struct TestServiceHandler {
///     store: MemoryStore
/// }
///
/// impl ServiceHandler for TestServiceHandler {
///     fn handle(&self,service_name: NamedNode, query: Query) -> Result<QueryResult> {
///         if service_name == "http://example.com/service" {
///             self.store.query(query, QueryOptions::default())
///         } else {
///             panic!()
///         }
///     }
/// }
///
/// let store = MemoryStore::new();
/// let service = TestServiceHandler::default();
/// let ex = NamedNode::new("http://example.com")?;
/// service.store.insert(Quad::new(ex.clone(), ex.clone(), ex.clone(), None));
///
/// if let QueryResult::Solutions(mut solutions) = store.query(
///     "SELECT ?s WHERE { SERVICE <http://example.com/service> { ?s ?p ?o } }",
///     QueryOptions::default().with_service_handler(service)
/// )? {
///     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
/// }
/// # Result::Ok(())
/// ```
pub trait ServiceHandler {
    /// Evaluates a `Query` against a given service identified by a `NamedNode`.
    fn handle(&self, service_name: NamedNode, query: Query) -> Result<QueryResult>;
}

struct EmptyServiceHandler;

impl ServiceHandler for EmptyServiceHandler {
    fn handle(&self, _: NamedNode, _: Query) -> Result<QueryResult> {
        Err(Error::msg("The SERVICE feature is not implemented"))
    }
}

/// Options for SPARQL query evaluation
#[derive(Clone)]
pub struct QueryOptions {
    pub(crate) default_graph_as_union: bool,
    pub(crate) service_handler: Rc<dyn ServiceHandler>,
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            default_graph_as_union: false,
            service_handler: Rc::new(EmptyServiceHandler),
        }
    }
}

impl QueryOptions {
    /// Consider the union of all graphs in the store as the default graph
    pub const fn with_default_graph_as_union(mut self) -> Self {
        self.default_graph_as_union = true;
        self
    }

    /// Use a given `ServiceHandler` to execute SPARQL SERVICE calls
    pub fn with_service_handler(mut self, service_handler: impl ServiceHandler + 'static) -> Self {
        self.service_handler = Rc::new(service_handler);
        self
    }
}
