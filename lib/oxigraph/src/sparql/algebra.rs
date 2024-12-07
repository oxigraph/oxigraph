//! [SPARQL 1.1 Query Algebra](https://www.w3.org/TR/sparql11-query/#sparqlQuery)
//!
//! The root type for SPARQL queries is [`Query`] and the root type for updates is [`Update`].

use crate::model::*;
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
#[allow(clippy::field_scoped_visibility_modifiers)]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct Query {
    pub(super) inner: spargebra::Query,
    pub(super) dataset: QueryDataset,
}

impl Query {
    /// Parses a SPARQL query with an optional base IRI to resolve relative IRIs in the query.
    pub fn parse(
        query: &str,
        base_iri: Option<&str>,
    ) -> Result<Self, spargebra::SparqlSyntaxError> {
        let query = Self::from(spargebra::Query::parse(query, base_iri)?);
        Ok(Self {
            dataset: query.dataset,
            inner: query.inner,
        })
    }

    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset)
    pub fn dataset(&self) -> &QueryDataset {
        &self.dataset
    }

    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset)
    pub fn dataset_mut(&mut self) -> &mut QueryDataset {
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
            dataset: QueryDataset::from_algebra(match &query {
                spargebra::Query::Select { dataset, .. }
                | spargebra::Query::Construct { dataset, .. }
                | spargebra::Query::Describe { dataset, .. }
                | spargebra::Query::Ask { dataset, .. } => dataset,
            }),
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
#[allow(clippy::field_scoped_visibility_modifiers)]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct Update {
    pub(super) inner: spargebra::Update,
    pub(super) using_datasets: Vec<Option<QueryDataset>>,
}

impl Update {
    /// Parses a SPARQL update with an optional base IRI to resolve relative IRIs in the query.
    pub fn parse(
        update: &str,
        base_iri: Option<&str>,
    ) -> Result<Self, spargebra::SparqlSyntaxError> {
        Ok(spargebra::Update::parse(update, base_iri)?.into())
    }

    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset) in [DELETE/INSERT operations](https://www.w3.org/TR/sparql11-update/#deleteInsert).
    pub fn using_datasets(&self) -> impl Iterator<Item = &QueryDataset> {
        self.using_datasets.iter().filter_map(Option::as_ref)
    }

    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset) in [DELETE/INSERT operations](https://www.w3.org/TR/sparql11-update/#deleteInsert).
    pub fn using_datasets_mut(&mut self) -> impl Iterator<Item = &mut QueryDataset> {
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
                        Some(QueryDataset::from_algebra(using))
                    } else {
                        None
                    }
                })
                .collect(),
            inner: update,
        }
    }
}

/// A SPARQL query [dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset)
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct QueryDataset {
    default: Option<Vec<GraphName>>,
    named: Option<Vec<NamedOrBlankNode>>,
}

impl QueryDataset {
    pub(crate) fn new() -> Self {
        Self {
            default: None,
            named: None,
        }
    }

    fn from_algebra(inner: &Option<spargebra::algebra::QueryDataset>) -> Self {
        if let Some(inner) = inner {
            Self {
                default: Some(inner.default.iter().map(|g| g.clone().into()).collect()),
                named: inner
                    .named
                    .as_ref()
                    .map(|named| named.iter().map(|g| g.clone().into()).collect()),
            }
        } else {
            Self {
                default: Some(vec![GraphName::DefaultGraph]),
                named: None,
            }
        }
    }

    /// Checks if this dataset specification is the default one
    /// (i.e. the default graph is the store default graph and all the store named graphs are available)
    ///
    /// ```
    /// use oxigraph::sparql::Query;
    ///
    /// assert!(Query::parse("SELECT ?s ?p ?o WHERE { ?s ?p ?o . }", None)?
    ///     .dataset()
    ///     .is_default_dataset());
    /// assert!(!Query::parse(
    ///     "SELECT ?s ?p ?o FROM <http://example.com> WHERE { ?s ?p ?o . }",
    ///     None
    /// )?
    /// .dataset()
    /// .is_default_dataset());
    ///
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn is_default_dataset(&self) -> bool {
        self.default
            .as_ref()
            .is_some_and(|t| t == &[GraphName::DefaultGraph])
            && self.named.is_none()
    }

    /// Returns the list of the store graphs that are available to the query as the default graph or `None` if the union of all graphs is used as the default graph
    /// This list is by default only the store default graph
    pub fn default_graph_graphs(&self) -> Option<&[GraphName]> {
        self.default.as_deref()
    }

    /// Sets if the default graph for the query should be the union of all the graphs in the queried store
    pub fn set_default_graph_as_union(&mut self) {
        self.default = None;
    }

    /// Sets the list of graphs the query should consider as being part of the default graph.
    ///
    /// By default only the store default graph is considered.
    /// ```
    /// use oxigraph::model::NamedNode;
    /// use oxigraph::sparql::Query;
    ///
    /// let mut query = Query::parse("SELECT ?s ?p ?o WHERE { ?s ?p ?o . }", None)?;
    /// let default = vec![NamedNode::new("http://example.com")?.into()];
    /// query.dataset_mut().set_default_graph(default.clone());
    /// assert_eq!(
    ///     query.dataset().default_graph_graphs(),
    ///     Some(default.as_slice())
    /// );
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
    /// use oxigraph::model::NamedNode;
    /// use oxigraph::sparql::Query;
    ///
    /// let mut query = Query::parse("SELECT ?s ?p ?o WHERE { ?s ?p ?o . }", None)?;
    /// let named = vec![NamedNode::new("http://example.com")?.into()];
    /// query
    ///     .dataset_mut()
    ///     .set_available_named_graphs(named.clone());
    /// assert_eq!(
    ///     query.dataset().available_named_graphs(),
    ///     Some(named.as_slice())
    /// );
    ///
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn set_available_named_graphs(&mut self, named_graphs: Vec<NamedOrBlankNode>) {
        self.named = Some(named_graphs);
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
