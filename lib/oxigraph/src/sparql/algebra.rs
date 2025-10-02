//! [SPARQL 1.1 Query Algebra](https://www.w3.org/TR/sparql11-query/#sparqlQuery)
//!
//! The root type for SPARQL queries is [`Query`] and the root type for updates is [`Update`].

#![expect(deprecated)]

use spareval::QueryDatasetSpecification;
use spargebra::GraphUpdateOperation;
use std::fmt;
use std::str::FromStr;

/// A parsed [SPARQL query](https://www.w3.org/TR/sparql11-query/).
///
/// ```
/// use oxigraph::model::NamedNode;
/// use oxigraph::sparql::Query;
///
/// let query_str = "SELECT ?s ?p ?o WHERE { ?s ?p ?o . }";
/// let mut query = Query::parse(query_str, None)?;
///
/// assert_eq!(query.to_string(), query_str);
///
/// // We edit the query dataset specification
/// let default = vec![NamedNode::new("http://example.com")?.into()];
/// query.dataset_mut().set_default_graph(default.clone());
/// assert_eq!(
///     query.dataset().default_graph_graphs(),
///     Some(default.as_slice())
/// );
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
#[expect(clippy::field_scoped_visibility_modifiers)]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
#[deprecated(
    note = "Use SparqlEvaluator instead to parse the query with options or directly the spargebra::Query type",
    since = "0.5.0"
)]
pub struct Query {
    pub(super) inner: spargebra::Query,
    pub(super) dataset: QueryDatasetSpecification,
}

impl Query {
    /// Parses a SPARQL query with an optional base IRI to resolve relative IRIs in the query.
    pub fn parse(
        query: &str,
        base_iri: Option<&str>,
    ) -> Result<Self, spargebra::SparqlSyntaxError> {
        #[expect(deprecated)]
        Ok(spargebra::Query::parse(query, base_iri)?.into())
    }

    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset)
    pub fn dataset(&self) -> &QueryDatasetSpecification {
        &self.dataset
    }

    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset)
    pub fn dataset_mut(&mut self) -> &mut QueryDatasetSpecification {
        &mut self.dataset
    }
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f) // TODO: override
    }
}

impl FromStr for Query {
    type Err = spargebra::SparqlSyntaxError;

    fn from_str(query: &str) -> Result<Self, Self::Err> {
        Self::parse(query, None)
    }
}

impl TryFrom<&str> for Query {
    type Error = spargebra::SparqlSyntaxError;

    fn try_from(query: &str) -> Result<Self, Self::Error> {
        Self::from_str(query)
    }
}

impl TryFrom<&String> for Query {
    type Error = spargebra::SparqlSyntaxError;

    fn try_from(query: &String) -> Result<Self, Self::Error> {
        Self::from_str(query)
    }
}

impl From<spargebra::Query> for Query {
    fn from(query: spargebra::Query) -> Self {
        Self {
            dataset: query.dataset().cloned().map(Into::into).unwrap_or_default(),
            inner: query,
        }
    }
}

/// A parsed [SPARQL update](https://www.w3.org/TR/sparql11-update/).
///
/// ```
/// use oxigraph::sparql::Update;
///
/// let update_str = "CLEAR ALL ;";
/// let update = Update::parse(update_str, None)?;
///
/// assert_eq!(update.to_string().trim(), update_str);
/// # Ok::<_, oxigraph::sparql::SparqlSyntaxError>(())
/// ```
#[expect(clippy::field_scoped_visibility_modifiers)]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
#[deprecated(
    note = "Use SparqlEvaluator instead to parse the update with options or directly the spargebra::Update type",
    since = "0.5.0"
)]
pub struct Update {
    pub(super) inner: spargebra::Update,
    pub(super) using_datasets: Vec<Option<QueryDatasetSpecification>>,
}

impl Update {
    /// Parses a SPARQL update with an optional base IRI to resolve relative IRIs in the query.
    pub fn parse(
        update: &str,
        base_iri: Option<&str>,
    ) -> Result<Self, spargebra::SparqlSyntaxError> {
        #[expect(deprecated)]
        Ok(spargebra::Update::parse(update, base_iri)?.into())
    }

    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset) in [DELETE/INSERT operations](https://www.w3.org/TR/sparql11-update/#deleteInsert).
    pub fn using_datasets(&self) -> impl Iterator<Item = &QueryDatasetSpecification> {
        self.using_datasets.iter().filter_map(Option::as_ref)
    }

    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset) in [DELETE/INSERT operations](https://www.w3.org/TR/sparql11-update/#deleteInsert).
    pub fn using_datasets_mut(&mut self) -> impl Iterator<Item = &mut QueryDatasetSpecification> {
        self.using_datasets.iter_mut().filter_map(Option::as_mut)
    }
}

impl fmt::Display for Update {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl FromStr for Update {
    type Err = spargebra::SparqlSyntaxError;

    fn from_str(update: &str) -> Result<Self, Self::Err> {
        Self::parse(update, None)
    }
}

impl TryFrom<&str> for Update {
    type Error = spargebra::SparqlSyntaxError;

    fn try_from(update: &str) -> Result<Self, Self::Error> {
        Self::from_str(update)
    }
}

impl TryFrom<&String> for Update {
    type Error = spargebra::SparqlSyntaxError;

    fn try_from(update: &String) -> Result<Self, Self::Error> {
        Self::from_str(update)
    }
}

impl From<spargebra::Update> for Update {
    fn from(update: spargebra::Update) -> Self {
        Self {
            using_datasets: update
                .operations
                .iter()
                .map(|operation| {
                    if let GraphUpdateOperation::DeleteInsert { using, .. } = operation {
                        Some(using.clone().map(Into::into).unwrap_or_default())
                    } else {
                        None
                    }
                })
                .collect(),
            inner: update,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_send_sync() {
        fn is_send_sync<T: Send + Sync>() {}
        is_send_sync::<Query>();
        is_send_sync::<Update>();
    }
}
