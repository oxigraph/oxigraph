use crate::expression::ExpressionEvaluationError;
use oxrdf::{NamedNode, Term, Variable};
use spargebra::SparqlSyntaxError;
use std::convert::Infallible;
use std::error::Error;
use std::ops::RangeInclusive;

/// A SPARQL evaluation error
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum QueryEvaluationError {
    /// Error from the underlying RDF dataset
    #[error(transparent)]
    Dataset(Box<dyn Error + Send + Sync>),
    /// Error during `SERVICE` evaluation
    #[error("{0}")]
    Service(#[source] Box<dyn Error + Send + Sync>),
    /// If a variable present in the given initial substitution is not present in the `SELECT` part of the query
    #[error("The SPARQL query does not contains variable {0} in its SELECT projection")]
    NotExistingSubstitutedVariable(Variable),
    /// Error if the dataset returns the default graph even if a named graph is expected
    #[error("The SPARQL dataset returned the default graph even if a named graph is expected")]
    UnexpectedDefaultGraph,
    /// The given custom function is not supported
    #[error("The custom function {0} is not supported")]
    UnsupportedCustomFunction(NamedNode),
    /// The given custom function arity is not supported
    #[error("The custom function {name} requires between {} and {} arguments, but {actual} were given", .expected.start(), .expected.end())]
    UnsupportedCustomFunctionArity {
        name: NamedNode,
        expected: RangeInclusive<usize>,
        actual: usize,
    },
    /// The variable storing the `SERVICE` name is unbound
    #[error("The variable encoding the service name is unbound")]
    UnboundService,
    /// Invalid service name
    #[error("{0} is not a valid service name")]
    InvalidServiceName(Term),
    /// The given `SERVICE` is not supported
    #[error("The service {0} is not supported")]
    UnsupportedService(NamedNode),
    #[cfg(feature = "sparql-12")]
    #[error("The SPARQL dataset returned a triple term that is not a valid RDF 1.2 term")]
    InvalidStorageTripleTerm,
    #[error("The SPARQL operation has been cancelled")]
    Cancelled,
    /// Query execution exceeded the configured timeout limit
    #[error("Query execution exceeded the timeout limit of {0:?}")]
    Timeout(std::time::Duration),
    /// Query result set exceeded the maximum allowed number of rows
    #[error("Query result set exceeded the maximum allowed {0} rows")]
    ResultLimitExceeded(usize),
    /// Query GROUP BY exceeded the maximum number of groups
    #[error("Query GROUP BY exceeded the maximum allowed {0} groups")]
    GroupLimitExceeded(usize),
    /// Property path evaluation exceeded the maximum depth
    #[error("Property path evaluation exceeded the maximum depth of {0}")]
    PropertyPathDepthExceeded(usize),
    /// Query execution exceeded the maximum allowed memory
    #[error("Query execution exceeded the maximum allowed memory of {0} bytes")]
    MemoryLimitExceeded(usize),
    #[doc(hidden)]
    #[error(transparent)]
    Unexpected(Box<dyn Error + Send + Sync>),
}

impl From<Infallible> for QueryEvaluationError {
    #[inline]
    fn from(error: Infallible) -> Self {
        match error {}
    }
}

// TODO: remove when removing the Store::update method
#[doc(hidden)]
impl From<SparqlSyntaxError> for QueryEvaluationError {
    #[inline]
    fn from(error: SparqlSyntaxError) -> Self {
        Self::Unexpected(Box::new(error))
    }
}

impl From<ExpressionEvaluationError<Self>> for QueryEvaluationError {
    #[inline]
    fn from(error: ExpressionEvaluationError<Self>) -> Self {
        match error {
            ExpressionEvaluationError::Context(e) => e,
            ExpressionEvaluationError::UnsupportedCustomFunction(name) => {
                Self::UnsupportedCustomFunction(name)
            }
            ExpressionEvaluationError::UnsupportedCustomFunctionArity {
                name,
                expected,
                actual,
            } => Self::UnsupportedCustomFunctionArity {
                name,
                expected,
                actual,
            },
        }
    }
}
