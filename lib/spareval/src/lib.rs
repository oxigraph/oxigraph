#![doc = include_str!("../README.md")]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

mod dataset;
mod error;
mod eval;
mod expression;
mod limits;
mod model;
mod n3_builtins;
mod service;
mod update;

#[cfg(feature = "sparql-12")]
pub use crate::dataset::ExpressionTriple;
pub use crate::dataset::{ExpressionTerm, InternalQuad, QueryableDataset};
pub use crate::error::QueryEvaluationError;
pub use crate::eval::CancellationToken;
pub use crate::limits::QueryExecutionLimits;
pub use crate::n3_builtins::{get_all_n3_builtins, N3BuiltinFn};
use crate::eval::{EvalNodeWithStats, SimpleEvaluator, Timer};
use crate::expression::{
    CustomFunctionRegistry, ExpressionEvaluatorContext, build_expression_evaluator,
};
pub use crate::model::{QueryResults, QuerySolution, QuerySolutionIter, QueryTripleIter};
use crate::service::ServiceHandlerRegistry;
pub use crate::service::{DefaultServiceHandler, ServiceHandler};
pub use crate::update::{DeleteInsertIter, DeleteInsertQuad};
use json_event_parser::{JsonEvent, WriterJsonSerializer};
use oxiri::Iri;
use oxrdf::{GraphName, Literal, NamedNode, NamedOrBlankNode, Term, Variable};
use oxsdatatypes::{DateTime, DayTimeDuration, Float};
use spargebra::Query;
use spargebra::algebra::QueryDataset;
use spargebra::term::{GroundQuadPattern, QuadPattern};
use sparopt::Optimizer;
use sparopt::algebra::GraphPattern;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::{fmt, io};

/// Evaluates a query against a given [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset)
///
/// To adapt this software to work on your own RDF dataset, you need to implement the [`QueryableDataset`] trait.
///
/// ```
/// use oxrdf::{Dataset, GraphName, NamedNode, Quad};
/// use spareval::{QueryEvaluator, QueryResults};
/// use spargebra::SparqlParser;
///
/// let ex = NamedNode::new("http://example.com")?;
/// let dataset = Dataset::from_iter([Quad::new(
///     ex.clone(),
///     ex.clone(),
///     ex.clone(),
///     GraphName::DefaultGraph,
/// )]);
/// let query = SparqlParser::new().parse_query("SELECT * WHERE { ?s ?p ?o }")?;
/// let evaluator = QueryEvaluator::new();
/// let results = evaluator.prepare(&query).execute(&dataset)?;
/// if let QueryResults::Solutions(solutions) = results {
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
    custom_aggregate_functions: CustomAggregateFunctionRegistry,
    without_optimizations: bool,
    run_stats: bool,
    cancellation_token: Option<CancellationToken>,
    limits: Option<QueryExecutionLimits>,
}

impl QueryEvaluator {
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Prepare the SPARQL query to be executed.
    pub fn prepare<'a>(&'a self, query: &'a Query) -> PreparedQuery<'a> {
        let dataset = query.dataset().cloned().map(Into::into).unwrap_or_default();
        PreparedQuery {
            evaluator: self,
            query,
            dataset,
            substitutions: HashMap::new(),
        }
    }

    /// Execute the SPARQL query against the given dataset.
    ///
    /// Note that this evaluator does not handle the `FROM` and `FROM NAMED` part of the query.
    /// You must select the proper dataset before using this struct.
    #[deprecated(since = "0.2.1", note = "Use prepare instead")]
    #[expect(deprecated)]
    pub fn execute<'a>(
        &self,
        dataset: impl QueryableDataset<'a>,
        query: &Query,
    ) -> Result<QueryResults<'a>, QueryEvaluationError> {
        self.execute_with_substituted_variables(dataset, query, [])
    }

    /// Executes a SPARQL query while substituting some variables with the given values.
    ///
    /// Substitution follows [RDF-dev SEP-0007](https://github.com/w3c/sparql-dev/blob/main/SEP/SEP-0007/sep-0007.md).
    ///
    /// ```
    /// # #![expect(deprecated)]
    /// use oxrdf::{Dataset, GraphName, NamedNode, Quad, Variable};
    /// use spareval::{QueryEvaluator, QueryResults};
    /// use spargebra::SparqlParser;
    ///
    /// let ex = NamedNode::new("http://example.com")?;
    /// let dataset = Dataset::from_iter([Quad::new(
    ///     ex.clone(),
    ///     ex.clone(),
    ///     ex.clone(),
    ///     GraphName::DefaultGraph,
    /// )]);
    /// let query = SparqlParser::new().parse_query("SELECT * WHERE { ?s ?p ?o }")?;
    /// let results = QueryEvaluator::new().execute_with_substituted_variables(
    ///     &dataset,
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
    #[deprecated(since = "0.2.1", note = "Use prepare instead")]
    #[expect(deprecated)]
    pub fn execute_with_substituted_variables<'a>(
        &self,
        dataset: impl QueryableDataset<'a>,
        query: &Query,
        substitutions: impl IntoIterator<Item = (Variable, Term)>,
    ) -> Result<QueryResults<'a>, QueryEvaluationError> {
        self.explain_with_substituted_variables(dataset, query, substitutions)
            .0
    }

    #[deprecated(since = "0.2.1", note = "Use prepare instead")]
    #[expect(deprecated)]
    pub fn explain<'a>(
        &self,
        dataset: impl QueryableDataset<'a>,
        query: &Query,
    ) -> (
        Result<QueryResults<'a>, QueryEvaluationError>,
        QueryExplanation,
    ) {
        self.explain_with_substituted_variables(dataset, query, [])
    }

    #[deprecated(since = "0.2.1", note = "Use prepare instead")]
    pub fn explain_with_substituted_variables<'a>(
        &self,
        dataset: impl QueryableDataset<'a>,
        query: &Query,
        substitutions: impl IntoIterator<Item = (Variable, Term)>,
    ) -> (
        Result<QueryResults<'a>, QueryEvaluationError>,
        QueryExplanation,
    ) {
        let mut prepared = PreparedQuery {
            evaluator: self,
            query,
            dataset: QueryDatasetSpecification::new(),
            substitutions: HashMap::new(),
        };
        for (variable, term) in substitutions {
            prepared = prepared.substitute_variable(variable, term);
        }
        prepared.explain(dataset)
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
    /// use spargebra::SparqlParser;
    ///
    /// let evaluator = QueryEvaluator::new().with_custom_function(
    ///     NamedNode::new("http://www.w3.org/ns/formats/N-Triples")?,
    ///     |args| args.get(0).map(|t| Literal::from(t.to_string()).into()),
    /// );
    /// let query = SparqlParser::new()
    ///     .parse_query("SELECT (<http://www.w3.org/ns/formats/N-Triples>(1) AS ?nt) WHERE {}")?;
    /// if let QueryResults::Solutions(mut solutions) =
    ///     evaluator.prepare(&query).execute(&Dataset::new())?
    /// {
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

    /// Adds a custom SPARQL evaluation aggregate function.
    ///
    /// Note that it must also be given to the SPARQL parser using [`SparqlParser::with_custom_aggregate_function`](spargebra::SparqlParser::with_custom_aggregate_function).
    ///
    /// Example with a function doing concatenation:
    /// ```
    /// use oxrdf::{Dataset, Literal, NamedNode, Term};
    /// use spareval::{AggregateFunctionAccumulator, QueryEvaluator, QueryResults};
    /// use spargebra::SparqlParser;
    /// use std::mem::take;
    ///
    /// struct ConcatAccumulator {
    ///     value: String,
    /// }
    ///
    /// impl AggregateFunctionAccumulator for ConcatAccumulator {
    ///     fn accumulate(&mut self, element: Term) {
    ///         if let Term::Literal(v) = element {
    ///             if !self.value.is_empty() {
    ///                 self.value.push(' ');
    ///             }
    ///             self.value.push_str(v.value());
    ///         }
    ///     }
    ///
    ///     fn finish(&mut self) -> Option<Term> {
    ///         Some(Literal::new_simple_literal(take(&mut self.value)).into())
    ///     }
    /// }
    ///
    /// let evaluator = QueryEvaluator::new().with_custom_aggregate_function(
    ///     NamedNode::new("http://example.com/concat")?,
    ///     || {
    ///         Box::new(ConcatAccumulator {
    ///             value: String::new(),
    ///         })
    ///     },
    /// );
    /// let query = SparqlParser::new()
    ///     .with_custom_aggregate_function(NamedNode::new("http://example.com/concat")?)
    ///     .parse_query(
    ///         "SELECT (<http://example.com/concat>(?v) AS ?r) WHERE { VALUES ?v { 1 2 3 } }",
    ///     )?;
    /// if let QueryResults::Solutions(mut solutions) =
    ///     evaluator.prepare(&query).execute(&Dataset::new())?
    /// {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("r"),
    ///         Some(&Literal::new_simple_literal("1 2 3").into())
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    #[must_use]
    pub fn with_custom_aggregate_function(
        mut self,
        name: NamedNode,
        evaluator: impl Fn() -> Box<dyn AggregateFunctionAccumulator + Send + Sync>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.custom_aggregate_functions
            .insert(name, Arc::new(evaluator));
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

    /// Inject a cancellation token to the SPARQL evaluation.
    ///
    /// Might be used to abort a query cleanly.
    ///
    /// ```
    /// use oxrdf::{Dataset, GraphName, NamedNode, Quad};
    /// use spareval::{CancellationToken, QueryEvaluationError, QueryEvaluator, QueryResults};
    /// use spargebra::SparqlParser;
    ///
    /// let ex = NamedNode::new("http://example.com")?;
    /// let dataset = Dataset::from_iter([Quad::new(
    ///     ex.clone(),
    ///     ex.clone(),
    ///     ex.clone(),
    ///     GraphName::DefaultGraph,
    /// )]);
    /// let query = SparqlParser::new().parse_query("SELECT * WHERE { ?s ?p ?o }")?;
    /// let cancellation_token = CancellationToken::new();
    /// let evaluator = QueryEvaluator::new().with_cancellation_token(cancellation_token.clone());
    /// let results = evaluator.prepare(&query).execute(&dataset)?;
    /// if let QueryResults::Solutions(mut solutions) = results {
    ///     cancellation_token.cancel(); // We cancel
    ///     assert!(matches!(
    ///         solutions.next().unwrap().unwrap_err(), // It's cancelled
    ///         QueryEvaluationError::Cancelled
    ///     ));
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[must_use]
    pub fn with_cancellation_token(mut self, cancellation_token: CancellationToken) -> Self {
        self.cancellation_token = Some(cancellation_token);
        self
    }

    /// Set resource limits for query execution.
    ///
    /// Limits help prevent denial-of-service attacks from long-running or resource-intensive queries.
    ///
    /// ```
    /// use oxrdf::Dataset;
    /// use spareval::{QueryEvaluationError, QueryEvaluator, QueryExecutionLimits};
    /// use spargebra::SparqlParser;
    /// use std::time::Duration;
    ///
    /// let evaluator = QueryEvaluator::new()
    ///     .with_limits(QueryExecutionLimits::strict());
    ///
    /// let query = SparqlParser::new().parse_query("SELECT * WHERE { ?s ?p ?o }")?;
    /// let results = evaluator.prepare(&query).execute(&Dataset::new())?;
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[must_use]
    pub fn with_limits(mut self, limits: QueryExecutionLimits) -> Self {
        self.limits = Some(limits);
        self
    }

    /// Registers all N3 built-in functions with this evaluator.
    ///
    /// This includes:
    /// - Math functions: sum, difference, product, quotient
    /// - String functions: concatenation, contains, length
    /// - Log functions: equalTo, notEqualTo
    /// - List functions: first, rest, member
    ///
    /// # Example
    ///
    /// ```
    /// use oxrdf::{Dataset, Literal};
    /// use spareval::{QueryEvaluator, QueryResults};
    /// use spargebra::SparqlParser;
    ///
    /// let evaluator = QueryEvaluator::new().with_all_n3_builtins();
    /// let query = SparqlParser::new().parse_query(
    ///     "SELECT (<http://www.w3.org/2000/10/swap/math#sum>(2, 3) AS ?result) WHERE {}"
    /// )?;
    /// if let QueryResults::Solutions(mut solutions) = evaluator.prepare(&query).execute(&Dataset::new())? {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("result"),
    ///         Some(&Literal::from(5).into())
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    #[must_use]
    pub fn with_all_n3_builtins(mut self) -> Self {
        for (name, func) in get_all_n3_builtins() {
            self.custom_functions.insert(name, func);
        }
        self
    }

    // Internal helper: evaluates a SPARQL expression to an ExpressionTerm against an empty dataset
    fn eval_expression_term_with_substitutions<'a>(
        &self,
        expression: &sparopt::algebra::Expression,
        substitutions: impl IntoIterator<Item = (&'a Variable, Term)>,
    ) -> Option<ExpressionTerm> {
        struct Context<'a> {
            now: Option<DateTime>,
            custom_functions: &'a CustomFunctionRegistry,
        }

        impl<'a> ExpressionEvaluatorContext<'a> for Context<'a> {
            type Term = Term;
            type Tuple = HashMap<&'a Variable, Term>;
            type Error = QueryEvaluationError;

            fn build_variable_lookup(
                &mut self,
                variable: &Variable,
            ) -> impl Fn(&HashMap<&'a Variable, Term>) -> Option<Term> + 'a {
                let variable = variable.clone();
                move |tuple| tuple.get(&variable).cloned()
            }

            fn build_is_variable_bound(
                &mut self,
                variable: &Variable,
            ) -> impl Fn(&HashMap<&'a Variable, Term>) -> bool + 'a {
                let variable = variable.clone();
                move |tuple| tuple.contains_key(&variable)
            }

            fn build_exists(
                &mut self,
                _: &GraphPattern,
            ) -> Result<impl Fn(&HashMap<&'a Variable, Term>) -> bool + 'a, QueryEvaluationError>
            {
                Err::<fn(&HashMap<&'a Variable, Term>) -> bool, _>(
                    QueryEvaluationError::Unexpected(
                        "EXISTS is not supported by the SPARQL expression evaluator".into(),
                    ),
                )
            }

            fn internalize_named_node(
                &mut self,
                term: &NamedNode,
            ) -> Result<Term, QueryEvaluationError> {
                Ok(term.clone().into())
            }

            fn internalize_literal(
                &mut self,
                term: &Literal,
            ) -> Result<Term, QueryEvaluationError> {
                Ok(term.clone().into())
            }

            fn build_internalize_expression_term(
                &mut self,
            ) -> impl Fn(ExpressionTerm) -> Option<Term> + 'a {
                |t| Some(t.into())
            }

            fn build_externalize_expression_term(
                &mut self,
            ) -> impl Fn(Term) -> Option<ExpressionTerm> + 'a {
                |t| Some(t.into())
            }

            fn now(&mut self) -> DateTime {
                *self.now.get_or_insert_with(DateTime::now)
            }

            fn base_iri(&mut self) -> Option<Arc<Iri<String>>> {
                None
            }

            fn custom_functions(&mut self) -> &CustomFunctionRegistry {
                self.custom_functions
            }
        }

        build_expression_evaluator(
            expression,
            &mut Context {
                now: None,
                custom_functions: &self.custom_functions,
            },
        )
        .ok()?(&substitutions.into_iter().collect::<HashMap<_, _>>())
    }

    /// Evaluates a SPARQL expression against an empty dataset with optional variable substitutions.
    ///
    /// Returns the computed term or `None` if an error occurs or the expression is invalid.
    pub fn evaluate_expression<'a>(
        &self,
        expression: &sparopt::algebra::Expression,
        substitutions: impl IntoIterator<Item = (&'a Variable, Term)>,
    ) -> Option<Term> {
        self.eval_expression_term_with_substitutions(expression, substitutions)
            .map(Into::into)
    }

    /// Evaluates a SPARQL expression effective boolean value (EBV) against an empty dataset
    /// with optional variable substitutions.
    ///
    /// Returns the EBV or `None` if an error occurs or EBV is undefined for the result type.
    pub fn evaluate_effective_boolean_value_expression<'a>(
        &self,
        expression: &sparopt::algebra::Expression,
        substitutions: impl IntoIterator<Item = (&'a Variable, Term)>,
    ) -> Option<bool> {
        self.eval_expression_term_with_substitutions(expression, substitutions)?
            .effective_boolean_value()
    }

    /// Evaluates a SPARQL UPDATE DELETE/INSERT operation.
    ///
    /// Returns the list of quads to delete or insert.
    ///
    /// ```
    /// use oxrdf::{Dataset, GraphName, Literal, NamedNode, Quad};
    /// use spareval::{DeleteInsertQuad, QueryEvaluator};
    /// use spargebra::{GraphUpdateOperation, SparqlParser};
    ///
    /// let ex = NamedNode::new("http://example.com")?;
    /// let dataset = Dataset::from_iter([Quad::new(
    ///     ex.clone(),
    ///     ex.clone(),
    ///     Literal::from(0),
    ///     GraphName::DefaultGraph,
    /// )]);
    /// let update = SparqlParser::new().parse_update(
    ///     "DELETE { ?s ?p ?o } INSERT { ?s ?p ?o2 } WHERE { ?s ?p ?o BIND(?o +1 AS ?o2) }",
    /// )?;
    /// let GraphUpdateOperation::DeleteInsert {
    ///     delete,
    ///     insert,
    ///     using: _,
    ///     pattern,
    /// } = &update.operations[0]
    /// else {
    ///     unreachable!()
    /// };
    /// let results = QueryEvaluator::new()
    ///     .prepare_delete_insert(delete.clone(), insert.clone(), None, None, pattern)
    ///     .execute(&dataset)?
    ///     .collect::<Result<Vec<_>, _>>()?;
    /// assert_eq!(
    ///     results,
    ///     vec![
    ///         DeleteInsertQuad::Delete(Quad::new(
    ///             ex.clone(),
    ///             ex.clone(),
    ///             Literal::from(0),
    ///             GraphName::DefaultGraph,
    ///         )),
    ///         DeleteInsertQuad::Insert(Quad::new(
    ///             ex.clone(),
    ///             ex.clone(),
    ///             Literal::from(1),
    ///             GraphName::DefaultGraph,
    ///         ))
    ///     ]
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn prepare_delete_insert<'a>(
        &'a self,
        delete: Vec<GroundQuadPattern>,
        insert: Vec<QuadPattern>,
        base_iri: Option<Iri<String>>,
        using: Option<QueryDataset>,
        pattern: &'a spargebra::algebra::GraphPattern,
    ) -> PreparedDeleteInsertUpdate<'a> {
        PreparedDeleteInsertUpdate {
            evaluator: self,
            pattern,
            delete,
            insert,
            base_iri,
            dataset: using.map(Into::into).unwrap_or_default(),
        }
    }

    fn simple_evaluator<'a, D: QueryableDataset<'a>>(
        &self,
        dataset: D,
        dataset_spec: QueryDatasetSpecification,
        base_iri: &Option<Iri<String>>,
    ) -> Result<SimpleEvaluator<'a, D>, QueryEvaluationError> {
        SimpleEvaluator::new(
            dataset,
            base_iri.clone().map(Arc::new),
            Rc::new(self.service_handler.clone()),
            Rc::new(self.custom_functions.clone()),
            Rc::new(self.custom_aggregate_functions.clone()),
            self.cancellation_token.clone().unwrap_or_default(),
            dataset_spec,
            self.run_stats,
        )
    }
}

/// A prepared SPARQL query.
///
/// Allows customizing things like the evaluation dataset and substituting variables.
///
/// Usage example:
/// ```
/// use oxrdf::{Dataset, Literal, Variable};
/// use spareval::{QueryEvaluator, QueryResults};
/// use spargebra::SparqlParser;
///
/// let query = SparqlParser::new().parse_query("SELECT ?v WHERE {}")?;
/// let evaluator = QueryEvaluator::new();
/// let prepared_query = evaluator
///     .prepare(&query)
///     .substitute_variable(Variable::new("v")?, Literal::from(1));
///
/// if let QueryResults::Solutions(mut solutions) = prepared_query.execute(&Dataset::new())? {
///     assert_eq!(
///         solutions.next().unwrap()?.get("v"),
///         Some(&Literal::from(1).into())
///     );
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Clone)]
#[must_use]
pub struct PreparedQuery<'a> {
    evaluator: &'a QueryEvaluator,
    query: &'a Query,
    dataset: QueryDatasetSpecification,
    substitutions: HashMap<Variable, Term>,
}

impl PreparedQuery<'_> {
    /// Substitute a variable with a given RDF term in the SPARQL query.
    ///
    /// Usage example:
    /// ```
    /// use oxrdf::{Dataset, Literal, Variable};
    /// use spareval::{QueryEvaluator, QueryResults};
    /// use spargebra::SparqlParser;
    ///
    /// let query = SparqlParser::new().parse_query("SELECT ?v WHERE {}")?;
    /// let evaluator = QueryEvaluator::new();
    /// let prepared_query = evaluator
    ///     .prepare(&query)
    ///     .substitute_variable(Variable::new("v")?, Literal::from(1));
    ///
    /// if let QueryResults::Solutions(mut solutions) = prepared_query.execute(&Dataset::new())? {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("v"),
    ///         Some(&Literal::from(1).into())
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn substitute_variable(
        mut self,
        variable: impl Into<Variable>,
        term: impl Into<Term>,
    ) -> Self {
        self.substitutions.insert(variable.into(), term.into());
        self
    }

    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset) of this prepared query.
    #[inline]
    pub fn dataset(&self) -> &QueryDatasetSpecification {
        &self.dataset
    }
    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset) of this prepared query.
    #[inline]
    pub fn dataset_mut(&mut self) -> &mut QueryDatasetSpecification {
        &mut self.dataset
    }

    /// Execute the SPARQL query against the given [`QueryableDataset`].
    pub fn execute<'b>(
        self,
        dataset: impl QueryableDataset<'b>,
    ) -> Result<QueryResults<'b>, QueryEvaluationError> {
        self.explain(dataset).0
    }

    pub fn explain<'b>(
        self,
        dataset: impl QueryableDataset<'b>,
    ) -> (
        Result<QueryResults<'b>, QueryEvaluationError>,
        QueryExplanation,
    ) {
        let start_planning = Timer::now();
        let (results, plan_node_with_stats, planning_duration) = match self.query {
            Query::Select {
                pattern, base_iri, ..
            } => {
                let mut pattern = GraphPattern::from(pattern);
                if !self.evaluator.without_optimizations {
                    pattern = Optimizer::optimize_graph_pattern(pattern);
                }
                let planning_duration = start_planning.elapsed();
                let (results, explanation) =
                    match self
                        .evaluator
                        .simple_evaluator(dataset, self.dataset, base_iri)
                    {
                        Ok(evaluator) => evaluator.evaluate_select(&pattern, self.substitutions),
                        Err(e) => (Err(e), Rc::new(EvalNodeWithStats::empty())),
                    };
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
                if !self.evaluator.without_optimizations {
                    pattern = Optimizer::optimize_graph_pattern(pattern);
                }
                let planning_duration = start_planning.elapsed();
                let (results, explanation) =
                    match self
                        .evaluator
                        .simple_evaluator(dataset, self.dataset, base_iri)
                    {
                        Ok(evaluator) => evaluator.evaluate_ask(&pattern, self.substitutions),
                        Err(e) => (Err(e), Rc::new(EvalNodeWithStats::empty())),
                    };
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
                if !self.evaluator.without_optimizations {
                    pattern = Optimizer::optimize_graph_pattern(pattern);
                }
                let planning_duration = start_planning.elapsed();
                let (results, explanation) =
                    match self
                        .evaluator
                        .simple_evaluator(dataset, self.dataset, base_iri)
                    {
                        Ok(evaluator) => {
                            evaluator.evaluate_construct(&pattern, template, self.substitutions)
                        }
                        Err(e) => (Err(e), Rc::new(EvalNodeWithStats::empty())),
                    };
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
                if !self.evaluator.without_optimizations {
                    pattern = Optimizer::optimize_graph_pattern(pattern);
                }
                let planning_duration = start_planning.elapsed();
                let (results, explanation) =
                    match self
                        .evaluator
                        .simple_evaluator(dataset, self.dataset, base_iri)
                    {
                        Ok(evaluator) => evaluator.evaluate_describe(&pattern, self.substitutions),
                        Err(e) => (Err(e), Rc::new(EvalNodeWithStats::empty())),
                    };
                (
                    results.map(QueryResults::Graph),
                    explanation,
                    planning_duration,
                )
            }
        };
        let explanation = QueryExplanation {
            inner: plan_node_with_stats,
            with_stats: self.evaluator.run_stats,
            planning_duration,
        };
        (results, explanation)
    }
}

/// A prepared SPARQL query.
///
/// Allows customizing things like the evaluation dataset and substituting variables.
///
/// Usage example:
/// ```
/// use oxrdf::{Dataset, GraphName, Literal, NamedNode, Quad};
/// use spareval::{DeleteInsertQuad, QueryEvaluator};
/// use spargebra::{GraphUpdateOperation, SparqlParser};
///
/// let ex = NamedNode::new("http://example.com")?;
/// let dataset = Dataset::from_iter([Quad::new(
///     ex.clone(),
///     ex.clone(),
///     Literal::from(0),
///     GraphName::DefaultGraph,
/// )]);
/// let update = SparqlParser::new().parse_update(
///     "DELETE { ?s ?p ?o } INSERT { ?s ?p ?o2 } WHERE { ?s ?p ?o BIND(?o +1 AS ?o2) }",
/// )?;
/// let GraphUpdateOperation::DeleteInsert {
///     delete,
///     insert,
///     using: _,
///     pattern,
/// } = &update.operations[0]
/// else {
///     unreachable!()
/// };
/// let results = QueryEvaluator::new()
///     .prepare_delete_insert(delete.clone(), insert.clone(), None, None, pattern)
///     .execute(&dataset)?
///     .collect::<Result<Vec<_>, _>>()?;
/// assert_eq!(
///     results,
///     vec![
///         DeleteInsertQuad::Delete(Quad::new(
///             ex.clone(),
///             ex.clone(),
///             Literal::from(0),
///             GraphName::DefaultGraph,
///         )),
///         DeleteInsertQuad::Insert(Quad::new(
///             ex.clone(),
///             ex.clone(),
///             Literal::from(1),
///             GraphName::DefaultGraph,
///         ))
///     ]
/// );
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Clone)]
#[must_use]
pub struct PreparedDeleteInsertUpdate<'a> {
    evaluator: &'a QueryEvaluator,
    pattern: &'a spargebra::algebra::GraphPattern,
    delete: Vec<GroundQuadPattern>,
    insert: Vec<QuadPattern>,
    base_iri: Option<Iri<String>>,
    dataset: QueryDatasetSpecification,
}

impl PreparedDeleteInsertUpdate<'_> {
    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset) of this prepared update.
    #[inline]
    pub fn dataset(&self) -> &QueryDatasetSpecification {
        &self.dataset
    }
    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset) of this prepared update.
    #[inline]
    pub fn dataset_mut(&mut self) -> &mut QueryDatasetSpecification {
        &mut self.dataset
    }

    /// Execute the SPARQL query against the given [`QueryableDataset`].
    pub fn execute<'b>(
        self,
        dataset: impl QueryableDataset<'b>,
    ) -> Result<DeleteInsertIter<'b>, QueryEvaluationError> {
        let mut pattern = GraphPattern::from(self.pattern);
        if !self.evaluator.without_optimizations {
            pattern = Optimizer::optimize_graph_pattern(pattern);
        }
        let (solutions, _) = self
            .evaluator
            .simple_evaluator(dataset, self.dataset, &self.base_iri)?
            .evaluate_select(&pattern, []);
        Ok(DeleteInsertIter::new(solutions?, self.delete, self.insert))
    }
}

pub(crate) type CustomAggregateFunctionRegistry = HashMap<
    NamedNode,
    Arc<dyn (Fn() -> Box<dyn AggregateFunctionAccumulator + Send + Sync>) + Send + Sync>,
>;

/// A trait for custom aggregate function implementation.
///
/// The accumulator accumulates values using the [`accumulate`](Self::accumulate) method
/// and returns a final aggregated value (or an error) using [`finish`](Self::finish).
///
/// See [`QueryEvaluator::with_custom_aggregate_function`] for an example.
pub trait AggregateFunctionAccumulator {
    fn accumulate(&mut self, element: Term);
    fn finish(&mut self) -> Option<Term>;
}

/// An extended SPARQL query [dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset).
///
/// Allows setting blank node graph names and that the default graph is the union of all named graphs.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct QueryDatasetSpecification {
    default: Option<Vec<GraphName>>,
    named: Option<Vec<NamedOrBlankNode>>,
}

impl QueryDatasetSpecification {
    pub fn new() -> Self {
        Self {
            default: Some(vec![GraphName::DefaultGraph]),
            named: None,
        }
    }

    /// Checks if this dataset specification is the default one
    /// (i.e., the default graph is the store default graph, and all named graphs included in the queried store are available)
    pub fn is_default_dataset(&self) -> bool {
        // TODO: rename to is_default?
        self.default
            .as_ref()
            .is_some_and(|t| t == &[GraphName::DefaultGraph])
            && self.named.is_none()
    }

    /// Returns the list of the store graphs that are available to the query as the default graph or `None` if the union of all graphs is used as the default graph.
    /// This list is by default only the store default graph.
    pub fn default_graph_graphs(&self) -> Option<&[GraphName]> {
        self.default.as_deref()
    }

    /// Sets the default graph of the query to be the union of all the graphs in the queried store.
    ///
    /// ```
    /// use oxrdf::{Dataset, NamedNode, Quad};
    /// use spareval::{QueryEvaluator, QueryResults};
    /// use spargebra::SparqlParser;
    ///
    /// let dataset = Dataset::from_iter([Quad::new(
    ///     NamedNode::new("http://example.com/s")?,
    ///     NamedNode::new("http://example.com/p")?,
    ///     NamedNode::new("http://example.com/o")?,
    ///     NamedNode::new("http://example.com/g")?,
    /// )]);
    /// let query = SparqlParser::new().parse_query("SELECT * WHERE { ?s ?p ?o }")?;
    /// let evaluator = QueryEvaluator::new();
    /// let mut prepared = evaluator.prepare(&query);
    /// prepared
    ///     .dataset_mut()
    ///     .set_default_graph(vec![NamedNode::new("http://example.com/g")?.into()]);
    /// if let QueryResults::Solutions(mut solutions) = prepared.execute(&dataset)? {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("s"),
    ///         Some(&NamedNode::new("http://example.com/s")?.into())
    ///     );
    /// }
    ///
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn set_default_graph_as_union(&mut self) {
        self.default = None;
    }

    /// Sets the list of graphs the query should consider as being part of the default graph.
    ///
    /// By default, only the store default graph is considered.
    /// ```
    /// use oxrdf::{Dataset, NamedNode, Quad};
    /// use spareval::{QueryEvaluator, QueryResults};
    /// use spargebra::SparqlParser;
    ///
    /// let dataset = Dataset::from_iter([Quad::new(
    ///     NamedNode::new("http://example.com/s")?,
    ///     NamedNode::new("http://example.com/p")?,
    ///     NamedNode::new("http://example.com/o")?,
    ///     NamedNode::new("http://example.com/g")?,
    /// )]);
    /// let query = SparqlParser::new().parse_query("SELECT * WHERE { ?s ?p ?o }")?;
    /// let evaluator = QueryEvaluator::new();
    /// let mut prepared = evaluator.prepare(&query);
    /// prepared
    ///     .dataset_mut()
    ///     .set_default_graph(vec![NamedNode::new("http://example.com/g")?.into()]);
    /// if let QueryResults::Solutions(mut solutions) = prepared.execute(&dataset)? {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("s"),
    ///         Some(&NamedNode::new("http://example.com/s")?.into())
    ///     );
    /// }
    ///
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn set_default_graph(&mut self, graphs: Vec<GraphName>) {
        self.default = Some(graphs)
    }

    /// Returns the list of the available named graphs for the query or `None` if all graphs are available
    pub fn available_named_graphs(&self) -> Option<&[NamedOrBlankNode]> {
        self.named.as_deref()
    }

    /// Sets the list of allowed named graphs in the query.
    ///
    /// ```
    /// use oxrdf::{Dataset, NamedNode, Quad};
    /// use spareval::{QueryEvaluator, QueryResults};
    /// use spargebra::SparqlParser;
    ///
    /// let dataset = Dataset::from_iter([Quad::new(
    ///     NamedNode::new("http://example.com/s")?,
    ///     NamedNode::new("http://example.com/p")?,
    ///     NamedNode::new("http://example.com/o")?,
    ///     NamedNode::new("http://example.com/g")?,
    /// )]);
    /// let query = SparqlParser::new().parse_query("SELECT * WHERE { ?s ?p ?o }")?;
    /// let evaluator = QueryEvaluator::new();
    /// let mut prepared = evaluator.prepare(&query);
    /// prepared
    ///     .dataset_mut()
    ///     .set_available_named_graphs(Vec::new());
    /// if let QueryResults::Solutions(mut solutions) = prepared.execute(&dataset)? {
    ///     assert!(solutions.next().is_none(),);
    /// }
    ///
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn set_available_named_graphs(&mut self, named_graphs: Vec<NamedOrBlankNode>) {
        self.named = Some(named_graphs);
    }
}

impl Default for QueryDatasetSpecification {
    fn default() -> Self {
        Self::new()
    }
}

impl From<QueryDataset> for QueryDatasetSpecification {
    fn from(dataset: QueryDataset) -> Self {
        Self {
            default: Some(dataset.default.into_iter().map(Into::into).collect()),
            named: dataset
                .named
                .map(|named| named.into_iter().map(Into::into).collect()),
        }
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::vocab::xsd;
    use oxrdf::{Literal, Term};
    use sparopt::algebra::{Expression, GraphPattern};

    #[test]
    fn evaluate_expression_literal_and_arithmetic() {
        let evaluator = QueryEvaluator::new();

        // Simple literal
        let expr = Expression::from(Literal::from(3_i32));
        let term = evaluator.evaluate_expression(&expr, std::iter::empty());
        assert_eq!(term, Some(Term::from(Literal::from(3_i32))));

        // 1 + 2 = 3
        let add = Expression::Add(
            Box::new(Expression::from(Literal::from(1_i32))),
            Box::new(Expression::from(Literal::from(2_i32))),
        );
        let term = evaluator.evaluate_expression(&add, std::iter::empty());
        assert_eq!(term, Some(Term::from(Literal::from(3_i32))));
    }

    #[test]
    fn evaluate_expression_with_variable_substitution() {
        let evaluator = QueryEvaluator::new();
        let x = Variable::new("x").unwrap();

        // ?x + 2 with ?x = 1 => 3
        let expr = Expression::Add(
            Box::new(Expression::from(x.clone())),
            Box::new(Expression::from(Literal::from(2_i32))),
        );
        let one: Term = Literal::from(1_i32).into();
        let result = evaluator.evaluate_expression(&expr, [(&x, one)]);
        assert_eq!(result, Some(Term::from(Literal::from(3_i32))));
    }

    #[test]
    fn evaluate_expression_with_unbound_variable_returns_none() {
        let evaluator = QueryEvaluator::new();
        let x = Variable::new("x").unwrap();
        let expr = Expression::from(x);
        let result = evaluator.evaluate_expression(&expr, std::iter::empty());
        assert!(result.is_none());
    }

    #[test]
    fn evaluate_effective_boolean_value_expression_basic() {
        let evaluator = QueryEvaluator::new();

        // Numeric EBV: 0 -> false, non-zero -> true
        let zero = Expression::from(Literal::from(0_i32));
        let five = Expression::from(Literal::from(5_i32));
        assert_eq!(
            evaluator.evaluate_effective_boolean_value_expression(&zero, std::iter::empty()),
            Some(false)
        );
        assert_eq!(
            evaluator.evaluate_effective_boolean_value_expression(&five, std::iter::empty()),
            Some(true)
        );

        // String EBV: empty -> false, non-empty -> true
        let empty_str = Expression::from(Literal::from(""));
        let non_empty_str = Expression::from(Literal::from("a"));
        assert_eq!(
            evaluator.evaluate_effective_boolean_value_expression(&empty_str, std::iter::empty()),
            Some(false)
        );
        assert_eq!(
            evaluator
                .evaluate_effective_boolean_value_expression(&non_empty_str, std::iter::empty()),
            Some(true)
        );
    }

    #[test]
    fn evaluate_effective_boolean_value_expression_exists() {
        let evaluator = QueryEvaluator::new();

        // EXISTS {} (empty) -> false
        let exists_empty = Expression::exists(GraphPattern::empty());
        assert_eq!(
            evaluator
                .evaluate_effective_boolean_value_expression(&exists_empty, std::iter::empty()),
            Some(false)
        );

        // EXISTS { VALUES () {} } (empty singleton) -> true
        let exists_unit = Expression::exists(GraphPattern::empty_singleton());
        assert_eq!(
            evaluator.evaluate_effective_boolean_value_expression(&exists_unit, std::iter::empty()),
            Some(true)
        );
    }

    #[test]
    fn evaluate_effective_boolean_value_expression_non_boolean_term() {
        let evaluator = QueryEvaluator::new();

        // NamedNode has no EBV
        let iri = NamedNode::new("http://example.com/").unwrap();
        let nn = Expression::from(iri.clone());
        assert_eq!(
            evaluator.evaluate_effective_boolean_value_expression(&nn, std::iter::empty()),
            None
        );

        // dateTime literal has no EBV
        let dt = Literal::new_typed_literal("2020-01-01T00:00:00Z", xsd::DATE_TIME);
        let expr = Expression::from(dt);
        assert_eq!(
            evaluator.evaluate_effective_boolean_value_expression(&expr, std::iter::empty()),
            None
        );
    }

    #[test]
    fn evaluate_effective_boolean_value_expression_boolean_lexical_forms() {
        let evaluator = QueryEvaluator::new();
        let one = Expression::from(Literal::new_typed_literal("1", xsd::BOOLEAN));
        let zero = Expression::from(Literal::new_typed_literal("0", xsd::BOOLEAN));
        assert_eq!(
            evaluator.evaluate_effective_boolean_value_expression(&one, std::iter::empty()),
            Some(true)
        );
        assert_eq!(
            evaluator.evaluate_effective_boolean_value_expression(&zero, std::iter::empty()),
            Some(false)
        );
    }

    #[test]
    fn evaluate_effective_boolean_value_expression_logic_with_errors() {
        let evaluator = QueryEvaluator::new();

        // OR(error, false) => error (None)
        let errorish = Expression::from(NamedNode::new("http://e/iri").unwrap());
        let or_expr = Expression::or_all([errorish, Expression::from(Literal::from(false))]);
        assert_eq!(
            evaluator.evaluate_effective_boolean_value_expression(&or_expr, std::iter::empty()),
            None
        );

        // AND(false, error) => false
        let errorish = Expression::from(NamedNode::new("http://e/iri2").unwrap());
        let and_expr = Expression::and_all([Expression::from(Literal::from(false)), errorish]);
        assert_eq!(
            evaluator.evaluate_effective_boolean_value_expression(&and_expr, std::iter::empty()),
            Some(false)
        );

        // AND(true, error) => error (None)
        let errorish = Expression::from(NamedNode::new("http://e/iri3").unwrap());
        let and_expr = Expression::and_all([Expression::from(Literal::from(true)), errorish]);
        assert_eq!(
            evaluator.evaluate_effective_boolean_value_expression(&and_expr, std::iter::empty()),
            None
        );
    }

    #[test]
    fn evaluate_expression_equality_returns_boolean_literal() {
        let evaluator = QueryEvaluator::new();
        let eq = Expression::equal(
            Expression::from(Literal::from(1_i32)),
            Expression::from(Literal::from(1_i32)),
        );
        let term = evaluator.evaluate_expression(&eq, std::iter::empty());
        assert_eq!(term, Some(Term::from(Literal::from(true))));
    }

    #[test]
    fn evaluate_expression_arithmetic_with_unbound_variable_is_none() {
        let evaluator = QueryEvaluator::new();
        let x = Variable::new("x").unwrap();
        let expr = Expression::Add(
            Box::new(Expression::from(Literal::from(2_i32))),
            Box::new(Expression::from(x)),
        );
        let result = evaluator.evaluate_expression(&expr, std::iter::empty());
        assert!(result.is_none());
    }
}
