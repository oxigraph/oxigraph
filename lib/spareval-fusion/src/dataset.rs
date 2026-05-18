use datafusion::arrow::array::ArrayRef;
use datafusion::arrow::datatypes::DataType;
use datafusion::common::{Result, ScalarValue};
use datafusion::logical_expr::{Literal, LogicalPlanBuilder};
use oxrdf::Term;
use spareval::ExpressionTerm;

/// Provides methods to access a dataset during SPARQL query evaluation.
///
/// Allows defining an internal encoding of RDF term in Arrow.
/// The major requirement is that RDF terms must be the same if, and only if, their Arrow representation is equal.
pub trait QueryableDatasetAccess {
    /// Returns a logical plan for the RDF quads of the dataset.
    ///
    /// It must contain 4 columns named "subject", "predicate", "object" and "graph_name".
    ///
    /// The first 3 columns must not be NULL, the graph_name column is NULL if, and only if, the triple is in the default graph.
    fn quads_table_plan(&mut self) -> Result<LogicalPlanBuilder>;

    /// Returns a term encoder for the current dataset.
    ///
    /// Used for expression evaluation.
    fn expression_term_encoder(&mut self) -> impl ExpressionTermEncoder;

    /// Encode an RDF term as a query plan constant.
    ///
    /// Used to build constants in expressions.
    fn internalize_term(&mut self, term: Term) -> Result<impl Literal>;
}

/// Trait to define the encoding of RDF terms in Arrow.
pub trait ExpressionTermEncoder: Send + Sync + Clone + 'static {
    /// Return the Arrow [`DataType`] in which terms are encoded
    fn internal_type(&self) -> &DataType;

    /// Encode an RDF term as used in expressions in a [`ScalarValue`].
    fn internalize_expression_term(&self, term: ExpressionTerm) -> Result<ScalarValue>;

    /// Encode RDF terms as used in expressions in a nullable arrow array.
    fn internalize_expression_terms(
        &self,
        terms: impl Iterator<Item = Option<ExpressionTerm>>,
    ) -> Result<ArrayRef>;

    /// Decode a [`ScalarValue`] into RDF term.
    fn externalize_expression_term(&self, term: ScalarValue) -> Result<Option<ExpressionTerm>>;

    /// Decode a nullable arrow array into RDF terms.
    ///
    /// Used to build constants in expressions.
    fn externalize_expression_terms(
        &self,
        terms: ArrayRef,
    ) -> Result<impl IntoIterator<Item = Result<Option<ExpressionTerm>>>>;
}
