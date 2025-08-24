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
#[cfg(feature = "sparql-12")]
pub use crate::dataset::ExpressionTriple;
pub use crate::dataset::{ExpressionTerm, InternalQuad, QueryableDataset};
pub use crate::error::QueryEvaluationError;
pub use crate::eval::CancellationToken;
use crate::eval::{EvalNodeWithStats, SimpleEvaluator, Timer};
pub use crate::model::{QueryResults, QuerySolution, QuerySolutionIter, QueryTripleIter};
use crate::service::ServiceHandlerRegistry;
pub use crate::service::{DefaultServiceHandler, ServiceHandler};
use json_event_parser::{JsonEvent, WriterJsonSerializer};
use oxrdf::{Dataset, NamedNode, Term, Variable};
use oxsdatatypes::{DayTimeDuration, Float};
use spargebra::Query;
use sparopt::Optimizer;
use sparopt::algebra::GraphPattern;
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
/// let results = QueryEvaluator::new().execute(&dataset, &query);
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
    custom_aggregate_functions: CustomAggregateFunctionRegistry,
    without_optimizations: bool,
    run_stats: bool,
    cancellation_token: Option<CancellationToken>,
}

impl QueryEvaluator {
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn execute<'a>(
        &self,
        dataset: impl QueryableDataset<'a>,
        query: &Query,
    ) -> Result<QueryResults<'a>, QueryEvaluationError> {
        self.explain(dataset, query).0
    }

    /// Executes a SPARQL query while substituting some variables with the given values.
    ///
    /// Substitution follows [RDF-dev SEP-0007](https://github.com/w3c/sparql-dev/blob/main/SEP/SEP-0007/sep-0007.md).
    ///
    /// ```
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
    pub fn execute_with_substituted_variables<'a>(
        &self,
        dataset: impl QueryableDataset<'a>,
        query: &Query,
        substitutions: impl IntoIterator<Item = (Variable, Term)>,
    ) -> Result<QueryResults<'a>, QueryEvaluationError> {
        self.explain_with_substituted_variables(dataset, query, substitutions)
            .0
    }

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

    pub fn explain_with_substituted_variables<'a>(
        &self,
        dataset: impl QueryableDataset<'a>,
        query: &Query,
        substitutions: impl IntoIterator<Item = (Variable, Term)>,
    ) -> (
        Result<QueryResults<'a>, QueryEvaluationError>,
        QueryExplanation,
    ) {
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
                    Rc::new(self.custom_aggregate_functions.clone()),
                    self.cancellation_token.clone().unwrap_or_default(),
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
                    Rc::new(self.custom_aggregate_functions.clone()),
                    self.cancellation_token.clone().unwrap_or_default(),
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
                    Rc::new(self.custom_aggregate_functions.clone()),
                    self.cancellation_token.clone().unwrap_or_default(),
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
                    Rc::new(self.custom_aggregate_functions.clone()),
                    self.cancellation_token.clone().unwrap_or_default(),
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
    /// use spargebra::SparqlParser;
    ///
    /// let evaluator = QueryEvaluator::new().with_custom_function(
    ///     NamedNode::new("http://www.w3.org/ns/formats/N-Triples")?,
    ///     |args| args.get(0).map(|t| Literal::from(t.to_string()).into()),
    /// );
    /// let query = SparqlParser::new()
    ///     .parse_query("SELECT (<http://www.w3.org/ns/formats/N-Triples>(1) AS ?nt) WHERE {}")?;
    /// if let QueryResults::Solutions(mut solutions) = evaluator.execute(&Dataset::new(), &query)? {
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
    /// if let QueryResults::Solutions(mut solutions) = evaluator.execute(&Dataset::new(), &query)? {
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
    /// let results = QueryEvaluator::new()
    ///     .with_cancellation_token(cancellation_token.clone())
    ///     .execute(&dataset, &query);
    /// if let QueryResults::Solutions(mut solutions) = results? {
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

    // Internal helper: evaluates a SPARQL expression to an ExpressionTerm against an empty dataset
    fn eval_expression_term_with_substitutions<'a>(
        &self,
        expression: &sparopt::algebra::Expression,
        substitutions: impl IntoIterator<Item = (&'a Variable, Term)>,
    ) -> Option<ExpressionTerm> {
        // Empty dataset to support EXISTS evaluation without accessing data
        let dataset = Dataset::new();
        let evaluator = SimpleEvaluator::new(
            &dataset,
            None,
            Rc::new(self.service_handler.clone()),
            Rc::new(self.custom_functions.clone()),
            Rc::new(self.custom_aggregate_functions.clone()),
            self.cancellation_token.clone().unwrap_or_default(),
            false,
        );

        let mut encoded_variables = Vec::new();
        let mut stat_children = Vec::new();
        let eval =
            evaluator.expression_evaluator(expression, &mut encoded_variables, &mut stat_children);

        // Build the input tuple with provided substitutions (ignore unknown variables)
        let mut tuple = eval::InternalTuple::with_capacity(encoded_variables.len());
        for (var, term) in substitutions {
            if let Some(pos) = encoded_variables.iter().position(|v| v == var) {
                let internal = (&dataset).internalize_term(term).ok()?;
                tuple.set(pos, internal);
            }
        }

        eval(&tuple)
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
}

pub(crate) type CustomFunctionRegistry =
    HashMap<NamedNode, Arc<dyn (Fn(&[Term]) -> Option<Term>) + Send + Sync>>;
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
