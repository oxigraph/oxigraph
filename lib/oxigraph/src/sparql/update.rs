#[cfg(feature = "http-client")]
use crate::io::{RdfFormat, RdfParser};
use crate::model::{GraphName as OxGraphName, GraphNameRef, Quad as OxQuad};
use crate::sparql::algebra::QueryDataset;
#[expect(deprecated)]
use crate::sparql::algebra::Update;
use crate::sparql::dataset::DatasetView;
use crate::sparql::error::UpdateEvaluationError;
#[cfg(feature = "http-client")]
use crate::sparql::http::Client;
use crate::storage::{Storage, StorageError, StorageReadableTransaction, StorageTransaction};
use crate::store::{Store, Transaction};
use oxiri::Iri;
#[cfg(feature = "http-client")]
use oxrdfio::LoadedDocument;
use rustc_hash::FxHashMap;
use sparesults::QuerySolution;
use spareval::{QueryEvaluator, QueryResults};
use spargebra::algebra::{GraphPattern, GraphTarget};
use spargebra::term::{
    BlankNode, GraphName, GraphNamePattern, GroundQuad, GroundQuadPattern, GroundTerm,
    GroundTermPattern, NamedNode, NamedNodePattern, NamedOrBlankNode, Quad, QuadPattern, Term,
    TermPattern,
};
#[cfg(feature = "rdf-12")]
use spargebra::term::{GroundTriple, GroundTriplePattern, Triple, TriplePattern};
use spargebra::{GraphUpdateOperation, Query};
#[cfg(feature = "http-client")]
use std::io::Read;
#[cfg(feature = "http-client")]
use std::time::Duration;

/// A prepared SPARQL update.
///
/// Usage example:
/// ```
/// use oxigraph::sparql::SparqlEvaluator;
/// use oxigraph::store::Store;
///
/// let prepared_update = SparqlEvaluator::new().parse_update(
///     "INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }",
/// )?;
/// prepared_update.on_store(&Store::new()?).execute()?;
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
#[must_use]
pub struct PreparedSparqlUpdate {
    evaluator: QueryEvaluator,
    update: spargebra::Update,
    using_datasets: Vec<Option<QueryDataset>>,
    #[cfg(feature = "http-client")]
    http_timeout: Option<Duration>,
    #[cfg(feature = "http-client")]
    http_redirection_limit: usize,
}

impl PreparedSparqlUpdate {
    #[expect(deprecated)]
    pub(crate) fn new(
        evaluator: QueryEvaluator,
        update: Update,
        #[cfg(feature = "http-client")] http_timeout: Option<Duration>,
        #[cfg(feature = "http-client")] http_redirection_limit: usize,
    ) -> Self {
        Self {
            evaluator,
            update: update.inner,
            using_datasets: update.using_datasets,
            #[cfg(feature = "http-client")]
            http_timeout,
            #[cfg(feature = "http-client")]
            http_redirection_limit,
        }
    }

    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset) in [DELETE/INSERT operations](https://www.w3.org/TR/sparql11-update/#deleteInsert).
    #[inline]
    pub fn using_datasets(&self) -> impl Iterator<Item = &QueryDataset> {
        self.using_datasets.iter().filter_map(Option::as_ref)
    }

    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset) in [DELETE/INSERT operations](https://www.w3.org/TR/sparql11-update/#deleteInsert).
    #[inline]
    pub fn using_datasets_mut(&mut self) -> impl Iterator<Item = &mut QueryDataset> {
        self.using_datasets.iter_mut().filter_map(Option::as_mut)
    }

    /// Bind the prepared update to the [`Store`] it should be evaluated on.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::sparql::SparqlEvaluator;
    /// use oxigraph::store::Store;
    ///
    /// let prepared_update = SparqlEvaluator::new().parse_update(
    ///     "INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }",
    /// )?;
    /// prepared_update.on_store(&Store::new()?).execute()?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn on_store(self, store: &Store) -> BoundPreparedSparqlUpdate<'_, '_> {
        let transaction = if update_requires_read(&self.update) {
            store
                .storage()
                .start_readable_transaction()
                .map(UpdateTransaction::OwnedReadable)
        } else {
            let storage = store.storage();
            storage
                .start_transaction()
                .map(|transaction| UpdateTransaction::Owned(transaction, storage))
        };
        BoundPreparedSparqlUpdate {
            evaluator: self.evaluator,
            update: self.update,
            using_datasets: self.using_datasets,
            #[cfg(feature = "http-client")]
            http_timeout: self.http_timeout,
            #[cfg(feature = "http-client")]
            http_redirection_limit: self.http_redirection_limit,
            transaction,
        }
    }

    /// Bind the prepared update to the [`Transaction`] it should be evaluated on.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::sparql::SparqlEvaluator;
    /// use oxigraph::store::Store;
    ///
    /// let prepared_update = SparqlEvaluator::new().parse_update(
    ///     "INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }",
    /// )?;
    /// let store = Store::new()?;
    /// let mut transaction = store.start_transaction()?;
    /// prepared_update.on_transaction(&mut transaction).execute()?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn on_transaction<'a, 'b: 'a>(
        self,
        transaction: &'a mut Transaction<'b>,
    ) -> BoundPreparedSparqlUpdate<'a, 'b> {
        BoundPreparedSparqlUpdate {
            evaluator: self.evaluator,
            update: self.update,
            using_datasets: self.using_datasets,
            #[cfg(feature = "http-client")]
            http_timeout: self.http_timeout,
            #[cfg(feature = "http-client")]
            http_redirection_limit: self.http_redirection_limit,
            transaction: Ok(UpdateTransaction::BorrowedReadable(transaction.inner_mut())),
        }
    }
}

/// A prepared SPARQL query bound to a storage, ready to be executed.
///
///
/// Usage example:
/// ```
/// use oxigraph::sparql::SparqlEvaluator;
/// use oxigraph::store::Store;
///
/// let store = Store::new()?;
/// let prepared_update = SparqlEvaluator::new()
///     .parse_update(
///         "INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }",
///     )?
///     .on_store(&store);
/// prepared_update.execute()?;
/// # Ok::<_, Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub struct BoundPreparedSparqlUpdate<'a, 'b> {
    evaluator: QueryEvaluator,
    update: spargebra::Update,
    using_datasets: Vec<Option<QueryDataset>>,
    #[cfg(feature = "http-client")]
    http_timeout: Option<Duration>,
    #[cfg(feature = "http-client")]
    http_redirection_limit: usize,
    transaction: Result<UpdateTransaction<'a, 'b>, StorageError>,
}

impl BoundPreparedSparqlUpdate<'_, '_> {
    /// Evaluate the update against the given store.
    pub fn execute(self) -> Result<(), UpdateEvaluationError> {
        match self.transaction? {
            UpdateTransaction::OwnedReadable(mut transaction) => {
                ReadableUpdateEvaluator {
                    transaction: &mut transaction,
                    base_iri: self.update.base_iri.clone(),
                    query_evaluator: self.evaluator,
                    #[cfg(feature = "http-client")]
                    client: Client::new(self.http_timeout, self.http_redirection_limit),
                }
                .eval_all(&self.update.operations, &self.using_datasets)?;
                transaction.commit()?;
                Ok(())
            }
            UpdateTransaction::BorrowedReadable(transaction) => ReadableUpdateEvaluator {
                transaction,
                base_iri: self.update.base_iri.clone(),
                query_evaluator: self.evaluator,
                #[cfg(feature = "http-client")]
                client: Client::new(self.http_timeout, self.http_redirection_limit),
            }
            .eval_all(&self.update.operations, &self.using_datasets),
            UpdateTransaction::Owned(mut transaction, storage) => {
                WriteOnlyUpdateEvaluator {
                    transaction: &mut transaction,
                    storage_for_initial_read: Some(storage),
                    base_iri: self.update.base_iri.clone(),
                    query_evaluator: self.evaluator,
                    #[cfg(feature = "http-client")]
                    client: Client::new(self.http_timeout, self.http_redirection_limit),
                }
                .eval_all(&self.update.operations, &self.using_datasets)?;
                transaction.commit()?;
                Ok(())
            }
        }
    }
}

enum UpdateTransaction<'a, 'b> {
    OwnedReadable(StorageReadableTransaction<'b>),
    BorrowedReadable(&'a mut StorageReadableTransaction<'b>),
    Owned(StorageTransaction<'b>, &'b Storage),
}

struct ReadableUpdateEvaluator<'a, 'b> {
    transaction: &'a mut StorageReadableTransaction<'b>,
    base_iri: Option<Iri<String>>,
    query_evaluator: QueryEvaluator,
    #[cfg(feature = "http-client")]
    client: Client,
}

impl<'a, 'b: 'a> ReadableUpdateEvaluator<'a, 'b> {
    fn eval_all(
        &mut self,
        updates: &[GraphUpdateOperation],
        using_datasets: &[Option<QueryDataset>],
    ) -> Result<(), UpdateEvaluationError> {
        for (update, using_dataset) in updates.iter().zip(using_datasets) {
            self.eval(update, using_dataset)?;
        }
        Ok(())
    }

    fn eval(
        &mut self,
        update: &GraphUpdateOperation,
        using_dataset: &Option<QueryDataset>,
    ) -> Result<(), UpdateEvaluationError> {
        match update {
            GraphUpdateOperation::InsertData { data } => {
                self.eval_insert_data(data);
                Ok(())
            }
            GraphUpdateOperation::DeleteData { data } => {
                self.eval_delete_data(data);
                Ok(())
            }
            GraphUpdateOperation::DeleteInsert {
                delete,
                insert,
                pattern,
                ..
            } => self.eval_delete_insert(
                delete,
                insert,
                using_dataset.as_ref().unwrap_or(&QueryDataset::new()),
                pattern,
            ),
            GraphUpdateOperation::Load {
                silent,
                source,
                destination,
            } => {
                if let Err(error) = self.eval_load(source, destination) {
                    if *silent { Ok(()) } else { Err(error) }
                } else {
                    Ok(())
                }
            }
            GraphUpdateOperation::Clear { graph, silent } => self.eval_clear(graph, *silent),
            GraphUpdateOperation::Create { graph, silent } => self.eval_create(graph, *silent),
            GraphUpdateOperation::Drop { graph, silent } => self.eval_drop(graph, *silent),
        }
    }

    fn eval_insert_data(&mut self, data: &[Quad]) {
        let mut bnodes = FxHashMap::default();
        for quad in data {
            let quad = convert_quad(quad, &mut bnodes);
            self.transaction.insert(quad.as_ref());
        }
    }

    fn eval_delete_data(&mut self, data: &[GroundQuad]) {
        for quad in data {
            let quad = convert_ground_quad(quad);
            self.transaction.remove(quad.as_ref());
        }
    }

    fn eval_delete_insert(
        &mut self,
        delete: &[GroundQuadPattern],
        insert: &[QuadPattern],
        using: &QueryDataset,
        algebra: &GraphPattern,
    ) -> Result<(), UpdateEvaluationError> {
        let QueryResults::Solutions(solutions) = self.query_evaluator.clone().execute(
            DatasetView::new(self.transaction.reader(), using),
            &Query::Select {
                dataset: None,
                pattern: algebra.clone(),
                base_iri: self.base_iri.clone(),
            },
        )?
        else {
            unreachable!("We provided a SELECT query, we must get back solutions")
        };

        let mut mutations = Vec::new();
        let mut bnodes = FxHashMap::default();
        for solution in solutions {
            let solution = solution?;
            for quad in delete {
                if let Some(quad) = fill_ground_quad_pattern(quad, &solution) {
                    mutations.push(InsertOrDelete::Delete(quad));
                }
            }
            for quad in insert {
                if let Some(quad) = fill_quad_pattern(quad, &solution, &mut bnodes) {
                    mutations.push(InsertOrDelete::Insert(quad));
                }
            }
            bnodes.clear();
        }

        for mutation in mutations {
            match mutation {
                InsertOrDelete::Delete(quad) => self.transaction.remove(quad.as_ref()),
                InsertOrDelete::Insert(quad) => self.transaction.insert(quad.as_ref()),
            }
        }
        Ok(())
    }

    fn eval_load(&mut self, from: &NamedNode, to: &GraphName) -> Result<(), UpdateEvaluationError> {
        eval_load(
            from,
            to,
            #[cfg(feature = "http-client")]
            &self.client,
            |q| self.transaction.insert(q.as_ref()),
        )
    }

    fn eval_create(
        &mut self,
        graph_name: &NamedNode,
        silent: bool,
    ) -> Result<(), UpdateEvaluationError> {
        if self
            .transaction
            .reader()
            .contains_named_graph(&graph_name.as_ref().into())?
        {
            if silent {
                Ok(())
            } else {
                Err(UpdateEvaluationError::GraphAlreadyExists(
                    graph_name.clone(),
                ))
            }
        } else {
            self.transaction.insert_named_graph(graph_name.into());
            Ok(())
        }
    }

    fn eval_clear(
        &mut self,
        graph: &GraphTarget,
        silent: bool,
    ) -> Result<(), UpdateEvaluationError> {
        match graph {
            GraphTarget::NamedNode(graph_name) => {
                if self
                    .transaction
                    .reader()
                    .contains_named_graph(&graph_name.as_ref().into())?
                {
                    Ok(self.transaction.clear_graph(graph_name.into())?)
                } else if silent {
                    Ok(())
                } else {
                    Err(UpdateEvaluationError::GraphDoesNotExist(graph_name.clone()))
                }
            }
            GraphTarget::DefaultGraph => {
                self.transaction.clear_graph(GraphNameRef::DefaultGraph)?;
                Ok(())
            }
            GraphTarget::NamedGraphs => Ok(self.transaction.clear_all_named_graphs()?),
            GraphTarget::AllGraphs => Ok(self.transaction.clear_all_graphs()?),
        }
    }

    fn eval_drop(
        &mut self,
        graph: &GraphTarget,
        silent: bool,
    ) -> Result<(), UpdateEvaluationError> {
        match graph {
            GraphTarget::NamedNode(graph_name) => {
                if self
                    .transaction
                    .reader()
                    .contains_named_graph(&graph_name.as_ref().into())?
                {
                    self.transaction.remove_named_graph(graph_name.into())?;
                    Ok(())
                } else if silent {
                    Ok(())
                } else {
                    Err(UpdateEvaluationError::GraphDoesNotExist(graph_name.clone()))
                }
            }
            GraphTarget::DefaultGraph => {
                Ok(self.transaction.clear_graph(GraphNameRef::DefaultGraph)?)
            }
            GraphTarget::NamedGraphs => Ok(self.transaction.remove_all_named_graphs()?),
            GraphTarget::AllGraphs => Ok(self.transaction.clear()?),
        }
    }
}

fn update_requires_read(update: &spargebra::Update) -> bool {
    for (i, op) in update.operations.iter().enumerate() {
        match op {
            GraphUpdateOperation::InsertData { .. }
            | GraphUpdateOperation::DeleteData { .. }
            | GraphUpdateOperation::Create { silent: true, .. }
            | GraphUpdateOperation::Clear {
                graph: GraphTarget::DefaultGraph | GraphTarget::NamedGraphs | GraphTarget::AllGraphs,
                ..
            }
            | GraphUpdateOperation::Drop {
                graph: GraphTarget::DefaultGraph | GraphTarget::NamedGraphs | GraphTarget::AllGraphs,
                ..
            } => (),
            GraphUpdateOperation::DeleteInsert { .. } if i == 0 => (),
            _ => return true,
        }
    }
    false
}

struct WriteOnlyUpdateEvaluator<'a, 'b> {
    transaction: &'a mut StorageTransaction<'b>,
    storage_for_initial_read: Option<&'b Storage>,
    base_iri: Option<Iri<String>>,
    query_evaluator: QueryEvaluator,
    #[cfg(feature = "http-client")]
    client: Client,
}

impl WriteOnlyUpdateEvaluator<'_, '_> {
    fn eval_all(
        &mut self,
        updates: &[GraphUpdateOperation],
        using_datasets: &[Option<QueryDataset>],
    ) -> Result<(), UpdateEvaluationError> {
        for (update, using_dataset) in updates.iter().zip(using_datasets) {
            self.eval(update, using_dataset)?;
            self.storage_for_initial_read.take(); // We unset the initial reader because we have likely mutated the store state.
        }
        Ok(())
    }

    fn eval(
        &mut self,
        update: &GraphUpdateOperation,
        using_dataset: &Option<QueryDataset>,
    ) -> Result<(), UpdateEvaluationError> {
        match update {
            GraphUpdateOperation::InsertData { data } => {
                self.eval_insert_data(data);
                Ok(())
            }
            GraphUpdateOperation::DeleteData { data } => {
                self.eval_delete_data(data);
                Ok(())
            }
            GraphUpdateOperation::DeleteInsert {
                delete,
                insert,
                pattern,
                ..
            } => self.eval_delete_insert(
                delete,
                insert,
                using_dataset.as_ref().unwrap_or(&QueryDataset::new()),
                pattern,
            ),
            GraphUpdateOperation::Load {
                silent,
                source,
                destination,
            } => {
                if let Err(error) = self.eval_load(source, destination) {
                    if *silent { Ok(()) } else { Err(error) }
                } else {
                    Ok(())
                }
            }
            GraphUpdateOperation::Clear { graph, silent } => self.eval_clear(graph, *silent),
            GraphUpdateOperation::Create { graph, silent } => self.eval_create(graph, *silent),
            GraphUpdateOperation::Drop { graph, silent } => self.eval_drop(graph, *silent),
        }
    }

    fn eval_insert_data(&mut self, data: &[Quad]) {
        let mut bnodes = FxHashMap::default();
        for quad in data {
            let quad = convert_quad(quad, &mut bnodes);
            self.transaction.insert(quad.as_ref());
        }
    }

    fn eval_delete_data(&mut self, data: &[GroundQuad]) {
        for quad in data {
            let quad = convert_ground_quad(quad);
            self.transaction.remove(quad.as_ref());
        }
    }

    fn eval_delete_insert(
        &mut self,
        delete: &[GroundQuadPattern],
        insert: &[QuadPattern],
        using: &QueryDataset,
        algebra: &GraphPattern,
    ) -> Result<(), UpdateEvaluationError> {
        let Some(storage) = self.storage_for_initial_read.take() else {
            return Err(UpdateEvaluationError::Unexpected(
                "It is not possible to evaluate delete/insert operations on a write-only transaction after other update operations".into(),
            ));
        };
        let QueryResults::Solutions(solutions) = self.query_evaluator.clone().execute(
            DatasetView::new(storage.snapshot(), using),
            &Query::Select {
                dataset: None,
                pattern: algebra.clone(),
                base_iri: self.base_iri.clone(),
            },
        )?
        else {
            unreachable!("We provided a SELECT query, we must get back solutions")
        };

        let mut bnodes = FxHashMap::default();
        for solution in solutions {
            let solution = solution?;
            for quad in delete {
                if let Some(quad) = fill_ground_quad_pattern(quad, &solution) {
                    self.transaction.remove(quad.as_ref());
                }
            }
            for quad in insert {
                if let Some(quad) = fill_quad_pattern(quad, &solution, &mut bnodes) {
                    self.transaction.insert(quad.as_ref());
                }
            }
            bnodes.clear();
        }
        Ok(())
    }

    fn eval_load(&mut self, from: &NamedNode, to: &GraphName) -> Result<(), UpdateEvaluationError> {
        eval_load(
            from,
            to,
            #[cfg(feature = "http-client")]
            &self.client,
            |q| self.transaction.insert(q.as_ref()),
        )
    }

    fn eval_create(
        &mut self,
        graph_name: &NamedNode,
        silent: bool,
    ) -> Result<(), UpdateEvaluationError> {
        if !silent {
            return Err(UpdateEvaluationError::Unexpected(
                "Not possible to create a named graph using a write-only transaction when SILENT option is not set".into(),
            ));
        }
        self.transaction.insert_named_graph(graph_name.into());
        Ok(())
    }

    fn eval_clear(
        &mut self,
        graph: &GraphTarget,
        _silent: bool,
    ) -> Result<(), UpdateEvaluationError> {
        match graph {
            GraphTarget::NamedNode(_) => Err(UpdateEvaluationError::Unexpected(
                "Not possible to clear a named graph using a write-only transaction".into(),
            )),
            GraphTarget::DefaultGraph => {
                self.transaction.clear_default_graph();
                Ok(())
            }
            GraphTarget::NamedGraphs => {
                self.transaction.clear_all_named_graphs();
                Ok(())
            }
            GraphTarget::AllGraphs => {
                self.transaction.clear_all_graphs();
                Ok(())
            }
        }
    }

    fn eval_drop(
        &mut self,
        graph: &GraphTarget,
        _silent: bool,
    ) -> Result<(), UpdateEvaluationError> {
        match graph {
            GraphTarget::NamedNode(_) => Err(UpdateEvaluationError::Unexpected(
                "Not possible to drop a named graph using a write-only transaction".into(),
            )),
            GraphTarget::DefaultGraph => {
                self.transaction.clear_default_graph();
                Ok(())
            }
            GraphTarget::NamedGraphs => {
                self.transaction.remove_all_named_graphs();
                Ok(())
            }
            GraphTarget::AllGraphs => {
                self.transaction.clear();
                Ok(())
            }
        }
    }
}

#[cfg(feature = "http-client")]
fn eval_load(
    from: &NamedNode,
    to: &GraphName,
    client: &Client,
    mut insert: impl FnMut(OxQuad),
) -> Result<(), UpdateEvaluationError> {
    let (content_type, body) = client
        .get(
            from.as_str(),
            "application/n-triples, text/turtle, application/rdf+xml",
        )
        .map_err(|e| UpdateEvaluationError::Service(Box::new(e)))?;
    let format = RdfFormat::from_media_type(&content_type)
        .ok_or_else(|| UpdateEvaluationError::UnsupportedContentType(content_type))?;
    let to_graph_name = match to {
        GraphName::NamedNode(graph_name) => graph_name.into(),
        GraphName::DefaultGraph => GraphNameRef::DefaultGraph,
    };
    let client = client.clone();
    let parser = RdfParser::from_format(format)
        .rename_blank_nodes()
        .without_named_graphs()
        .with_default_graph(to_graph_name)
        .with_base_iri(from.as_str())
        .map_err(|e| UpdateEvaluationError::Unexpected(format!("Invalid URL: {from}: {e}").into()))?
        .for_reader(body)
        .with_document_loader(move |url| {
            let (content_type, mut body) = client.get(
                url,
                "application/n-triples, text/turtle, application/rdf+xml, application/ld+json",
            )?;
            let mut content = Vec::new();
            body.read_to_end(&mut content)?;
            Ok(LoadedDocument {
                url: url.into(),
                content,
                format: RdfFormat::from_media_type(&content_type)
                    .ok_or_else(|| UpdateEvaluationError::UnsupportedContentType(content_type))?,
            })
        });
    for q in parser {
        insert(q?);
    }
    Ok(())
}

#[cfg(not(feature = "http-client"))]
fn eval_load(
    _from: &NamedNode,
    _to: &GraphName,
    _insert: impl FnMut(OxQuad),
) -> Result<(), UpdateEvaluationError> {
    Err(UpdateEvaluationError::Unexpected(
        "HTTP client is not available. Enable the feature 'http-client'".into(),
    ))
}

enum InsertOrDelete {
    Insert(OxQuad),
    Delete(OxQuad),
}

fn convert_quad(quad: &Quad, bnodes: &mut FxHashMap<BlankNode, BlankNode>) -> OxQuad {
    OxQuad {
        subject: match &quad.subject {
            NamedOrBlankNode::NamedNode(subject) => subject.clone().into(),
            NamedOrBlankNode::BlankNode(subject) => convert_blank_node(subject, bnodes).into(),
        },
        predicate: quad.predicate.clone(),
        object: match &quad.object {
            Term::NamedNode(object) => object.clone().into(),
            Term::BlankNode(object) => convert_blank_node(object, bnodes).into(),
            Term::Literal(object) => object.clone().into(),
            #[cfg(feature = "rdf-12")]
            Term::Triple(subject) => convert_triple(subject, bnodes).into(),
        },
        graph_name: match &quad.graph_name {
            GraphName::NamedNode(graph_name) => graph_name.clone().into(),
            GraphName::DefaultGraph => OxGraphName::DefaultGraph,
        },
    }
}

#[cfg(feature = "rdf-12")]
fn convert_triple(triple: &Triple, bnodes: &mut FxHashMap<BlankNode, BlankNode>) -> Triple {
    Triple {
        subject: match &triple.subject {
            NamedOrBlankNode::NamedNode(subject) => subject.clone().into(),
            NamedOrBlankNode::BlankNode(subject) => convert_blank_node(subject, bnodes).into(),
        },
        predicate: triple.predicate.clone(),
        object: match &triple.object {
            Term::NamedNode(object) => object.clone().into(),
            Term::BlankNode(object) => convert_blank_node(object, bnodes).into(),
            Term::Literal(object) => object.clone().into(),
            #[cfg(feature = "rdf-12")]
            Term::Triple(subject) => convert_triple(subject, bnodes).into(),
        },
    }
}

fn convert_blank_node(node: &BlankNode, bnodes: &mut FxHashMap<BlankNode, BlankNode>) -> BlankNode {
    bnodes.entry(node.clone()).or_default().clone()
}

fn convert_ground_quad(quad: &GroundQuad) -> OxQuad {
    OxQuad {
        subject: quad.subject.clone().into(),
        predicate: quad.predicate.clone(),
        object: match &quad.object {
            GroundTerm::NamedNode(object) => object.clone().into(),
            GroundTerm::Literal(object) => object.clone().into(),
            #[cfg(feature = "rdf-12")]
            GroundTerm::Triple(subject) => convert_ground_triple(subject).into(),
        },
        graph_name: match &quad.graph_name {
            GraphName::NamedNode(graph_name) => graph_name.clone().into(),
            GraphName::DefaultGraph => OxGraphName::DefaultGraph,
        },
    }
}

#[cfg(feature = "rdf-12")]
fn convert_ground_triple(triple: &GroundTriple) -> Triple {
    Triple {
        subject: triple.subject.clone().into(),
        predicate: triple.predicate.clone(),
        object: match &triple.object {
            GroundTerm::NamedNode(object) => object.clone().into(),
            GroundTerm::Literal(object) => object.clone().into(),
            #[cfg(feature = "rdf-12")]
            GroundTerm::Triple(subject) => convert_ground_triple(subject).into(),
        },
    }
}

fn fill_quad_pattern(
    quad: &QuadPattern,
    solution: &QuerySolution,
    bnodes: &mut FxHashMap<BlankNode, BlankNode>,
) -> Option<OxQuad> {
    Some(OxQuad {
        subject: match fill_term_or_var(&quad.subject, solution, bnodes)? {
            Term::NamedNode(node) => node.into(),
            Term::BlankNode(node) => node.into(),
            #[cfg(feature = "rdf-12")]
            Term::Triple(_) => return None,
            Term::Literal(_) => return None,
        },
        predicate: fill_named_node_or_var(&quad.predicate, solution)?,
        object: fill_term_or_var(&quad.object, solution, bnodes)?,
        graph_name: fill_graph_name_or_var(&quad.graph_name, solution)?,
    })
}

fn fill_term_or_var(
    term: &TermPattern,
    solution: &QuerySolution,
    bnodes: &mut FxHashMap<BlankNode, BlankNode>,
) -> Option<Term> {
    Some(match term {
        TermPattern::NamedNode(term) => term.clone().into(),
        TermPattern::BlankNode(bnode) => convert_blank_node(bnode, bnodes).into(),
        TermPattern::Literal(term) => term.clone().into(),
        #[cfg(feature = "rdf-12")]
        TermPattern::Triple(triple) => fill_triple_pattern(triple, solution, bnodes)?.into(),
        TermPattern::Variable(v) => solution.get(v)?.clone(),
    })
}

fn fill_named_node_or_var(term: &NamedNodePattern, solution: &QuerySolution) -> Option<NamedNode> {
    Some(match term {
        NamedNodePattern::NamedNode(term) => term.clone(),
        NamedNodePattern::Variable(v) => {
            if let Term::NamedNode(s) = solution.get(v)? {
                s.clone()
            } else {
                return None;
            }
        }
    })
}

fn fill_graph_name_or_var(
    term: &GraphNamePattern,
    solution: &QuerySolution,
) -> Option<OxGraphName> {
    Some(match term {
        GraphNamePattern::NamedNode(term) => term.clone().into(),
        GraphNamePattern::DefaultGraph => OxGraphName::DefaultGraph,
        GraphNamePattern::Variable(v) => match solution.get(v)? {
            Term::NamedNode(node) => node.clone().into(),
            Term::BlankNode(node) => node.clone().into(),
            Term::Literal(_) => return None,
            #[cfg(feature = "rdf-12")]
            Term::Triple(_) => return None,
        },
    })
}

#[cfg(feature = "rdf-12")]
fn fill_triple_pattern(
    triple: &TriplePattern,
    solution: &QuerySolution,
    bnodes: &mut FxHashMap<BlankNode, BlankNode>,
) -> Option<Triple> {
    Some(Triple {
        subject: match fill_term_or_var(&triple.subject, solution, bnodes)? {
            Term::NamedNode(node) => node.into(),
            Term::BlankNode(node) => node.into(),
            #[cfg(feature = "rdf-12")]
            Term::Triple(_) => return None,
            Term::Literal(_) => return None,
        },
        predicate: fill_named_node_or_var(&triple.predicate, solution)?,
        object: fill_term_or_var(&triple.object, solution, bnodes)?,
    })
}
fn fill_ground_quad_pattern(quad: &GroundQuadPattern, solution: &QuerySolution) -> Option<OxQuad> {
    Some(OxQuad {
        subject: match fill_ground_term_or_var(&quad.subject, solution)? {
            Term::NamedNode(node) => node.into(),
            Term::BlankNode(node) => node.into(),
            #[cfg(feature = "rdf-12")]
            Term::Triple(_) => return None,
            Term::Literal(_) => return None,
        },
        predicate: fill_named_node_or_var(&quad.predicate, solution)?,
        object: fill_ground_term_or_var(&quad.object, solution)?,
        graph_name: fill_graph_name_or_var(&quad.graph_name, solution)?,
    })
}

fn fill_ground_term_or_var(term: &GroundTermPattern, solution: &QuerySolution) -> Option<Term> {
    Some(match term {
        GroundTermPattern::NamedNode(term) => term.clone().into(),
        GroundTermPattern::Literal(term) => term.clone().into(),
        #[cfg(feature = "rdf-12")]
        GroundTermPattern::Triple(triple) => fill_ground_triple_pattern(triple, solution)?.into(),
        GroundTermPattern::Variable(v) => solution.get(v)?.clone(),
    })
}

#[cfg(feature = "rdf-12")]
fn fill_ground_triple_pattern(
    triple: &GroundTriplePattern,
    solution: &QuerySolution,
) -> Option<Triple> {
    Some(Triple {
        subject: match fill_ground_term_or_var(&triple.subject, solution)? {
            Term::NamedNode(node) => node.into(),
            Term::BlankNode(node) => node.into(),
            #[cfg(feature = "rdf-12")]
            Term::Triple(_) => return None,
            Term::Literal(_) => return None,
        },
        predicate: fill_named_node_or_var(&triple.predicate, solution)?,
        object: fill_ground_term_or_var(&triple.object, solution)?,
    })
}
