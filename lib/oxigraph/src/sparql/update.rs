#[cfg(feature = "http-client")]
use crate::io::{RdfFormat, RdfParser};
use crate::model::{GraphName as OxGraphName, GraphNameRef, Quad as OxQuad};
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
use spareval::{DeleteInsertQuad, QueryDatasetSpecification, QueryEvaluator};
use spargebra::algebra::GraphTarget;
use spargebra::term::{BlankNode, GraphName, GroundQuad, GroundTerm, NamedOrBlankNode, Quad, Term};
#[cfg(feature = "rdf-12")]
use spargebra::term::{GroundTriple, Triple};
use spargebra::update::{
    ClearOperation, CreateOperation, DeleteDataOperation, DeleteInsertOperation, DropOperation,
    GraphUpdateOperation, InsertDataOperation, LoadOperation, Update,
};
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
    update: Update,
    using_datasets: Vec<Option<QueryDatasetSpecification>>,
    #[cfg(feature = "http-client")]
    http_timeout: Option<Duration>,
    #[cfg(feature = "http-client")]
    http_redirection_limit: usize,
}

impl PreparedSparqlUpdate {
    pub(crate) fn new(
        evaluator: QueryEvaluator,
        update: Update,
        #[cfg(feature = "http-client")] http_timeout: Option<Duration>,
        #[cfg(feature = "http-client")] http_redirection_limit: usize,
    ) -> Self {
        let using_datasets = update
            .operations
            .iter()
            .map(|operation| {
                if let GraphUpdateOperation::DeleteInsert(operation) = operation {
                    Some(operation.using.clone().map(Into::into).unwrap_or_default())
                } else {
                    None
                }
            })
            .collect();
        Self {
            evaluator,
            update,
            using_datasets,
            #[cfg(feature = "http-client")]
            http_timeout,
            #[cfg(feature = "http-client")]
            http_redirection_limit,
        }
    }

    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset) in [DELETE/INSERT operations](https://www.w3.org/TR/sparql11-update/#deleteInsert).
    #[inline]
    pub fn using_datasets(&self) -> impl Iterator<Item = &QueryDatasetSpecification> {
        self.using_datasets.iter().filter_map(Option::as_ref)
    }

    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset) in [DELETE/INSERT operations](https://www.w3.org/TR/sparql11-update/#deleteInsert).
    #[inline]
    pub fn using_datasets_mut(&mut self) -> impl Iterator<Item = &mut QueryDatasetSpecification> {
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
    update: Update,
    using_datasets: Vec<Option<QueryDatasetSpecification>>,
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
        using_datasets: &[Option<QueryDatasetSpecification>],
    ) -> Result<(), UpdateEvaluationError> {
        for (update, using_dataset) in updates.iter().zip(using_datasets) {
            self.eval(update, using_dataset)?;
        }
        Ok(())
    }

    fn eval(
        &mut self,
        update: &GraphUpdateOperation,
        using_dataset: &Option<QueryDatasetSpecification>,
    ) -> Result<(), UpdateEvaluationError> {
        match update {
            GraphUpdateOperation::InsertData(op) => {
                self.eval_insert_data(op);
                Ok(())
            }
            GraphUpdateOperation::DeleteData(op) => {
                self.eval_delete_data(op);
                Ok(())
            }
            GraphUpdateOperation::DeleteInsert(op) => self.eval_delete_insert(
                op,
                using_dataset
                    .as_ref()
                    .unwrap_or(&QueryDatasetSpecification::new()),
            ),
            GraphUpdateOperation::Load(op) => self.eval_load(op),
            GraphUpdateOperation::Clear(op) => self.eval_clear(op),
            GraphUpdateOperation::Create(op) => self.eval_create(op),
            GraphUpdateOperation::Drop(op) => self.eval_drop(op),
        }
    }

    fn eval_insert_data(&mut self, operation: &InsertDataOperation) {
        let mut bnodes = FxHashMap::default();
        for quad in &operation.data {
            let quad = convert_quad(quad, &mut bnodes);
            self.transaction.insert(quad.as_ref());
        }
    }

    fn eval_delete_data(&mut self, operation: &DeleteDataOperation) {
        for quad in &operation.data {
            let quad = convert_ground_quad(quad);
            self.transaction.remove(quad.as_ref());
        }
    }

    fn eval_delete_insert(
        &mut self,
        operation: &DeleteInsertOperation,
        using: &QueryDatasetSpecification,
    ) -> Result<(), UpdateEvaluationError> {
        let mut prepared = self.query_evaluator.prepare_delete_insert(
            operation.delete.clone(),
            operation.insert.clone(),
            self.base_iri.clone(),
            None,
            &operation.pattern,
        );
        *prepared.dataset_mut() = using.clone();
        let mutations = prepared
            .execute(DatasetView::new(self.transaction.reader()))?
            .collect::<Result<Vec<_>, _>>()?;
        for mutation in mutations {
            match mutation {
                DeleteInsertQuad::Delete(quad) => self.transaction.remove(quad.as_ref()),
                DeleteInsertQuad::Insert(quad) => self.transaction.insert(quad.as_ref()),
            }
        }
        Ok(())
    }

    fn eval_load(&mut self, operation: &LoadOperation) -> Result<(), UpdateEvaluationError> {
        if let Err(error) = eval_load(
            operation,
            #[cfg(feature = "http-client")]
            &self.client,
            |q| self.transaction.insert(q.as_ref()),
        ) {
            if operation.silent { Ok(()) } else { Err(error) }
        } else {
            Ok(())
        }
    }

    fn eval_create(&mut self, operation: &CreateOperation) -> Result<(), UpdateEvaluationError> {
        if self
            .transaction
            .reader()
            .contains_named_graph(&operation.graph.as_ref().into())?
        {
            if operation.silent {
                Ok(())
            } else {
                Err(UpdateEvaluationError::GraphAlreadyExists(
                    operation.graph.clone(),
                ))
            }
        } else {
            self.transaction
                .insert_named_graph((&operation.graph).into());
            Ok(())
        }
    }

    fn eval_clear(&mut self, operation: &ClearOperation) -> Result<(), UpdateEvaluationError> {
        match &operation.graph {
            GraphTarget::NamedNode(graph_name) => {
                if self
                    .transaction
                    .reader()
                    .contains_named_graph(&graph_name.as_ref().into())?
                {
                    Ok(self.transaction.clear_graph(graph_name.into())?)
                } else if operation.silent {
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

    fn eval_drop(&mut self, operation: &DropOperation) -> Result<(), UpdateEvaluationError> {
        match &operation.graph {
            GraphTarget::NamedNode(graph_name) => {
                if self
                    .transaction
                    .reader()
                    .contains_named_graph(&graph_name.as_ref().into())?
                {
                    self.transaction.remove_named_graph(graph_name.into())?;
                    Ok(())
                } else if operation.silent {
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

fn update_requires_read(update: &Update) -> bool {
    for (i, op) in update.operations.iter().enumerate() {
        match op {
            GraphUpdateOperation::InsertData(_)
            | GraphUpdateOperation::DeleteData(_)
            | GraphUpdateOperation::Create(CreateOperation { silent: true, .. })
            | GraphUpdateOperation::Clear(ClearOperation {
                graph: GraphTarget::DefaultGraph | GraphTarget::NamedGraphs | GraphTarget::AllGraphs,
                ..
            })
            | GraphUpdateOperation::Drop(DropOperation {
                graph: GraphTarget::DefaultGraph | GraphTarget::NamedGraphs | GraphTarget::AllGraphs,
                ..
            }) => (),
            GraphUpdateOperation::DeleteInsert(_) if i == 0 => (),
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
        using_datasets: &[Option<QueryDatasetSpecification>],
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
        using_dataset: &Option<QueryDatasetSpecification>,
    ) -> Result<(), UpdateEvaluationError> {
        match update {
            GraphUpdateOperation::InsertData(op) => {
                self.eval_insert_data(op);
                Ok(())
            }
            GraphUpdateOperation::DeleteData(op) => {
                self.eval_delete_data(op);
                Ok(())
            }
            GraphUpdateOperation::DeleteInsert(op) => self.eval_delete_insert(
                op,
                using_dataset
                    .as_ref()
                    .unwrap_or(&QueryDatasetSpecification::new()),
            ),
            GraphUpdateOperation::Load(op) => self.eval_load(op),
            GraphUpdateOperation::Clear(op) => self.eval_clear(op),
            GraphUpdateOperation::Create(op) => self.eval_create(op),
            GraphUpdateOperation::Drop(op) => self.eval_drop(op),
        }
    }

    fn eval_insert_data(&mut self, operation: &InsertDataOperation) {
        let mut bnodes = FxHashMap::default();
        for quad in &operation.data {
            let quad = convert_quad(quad, &mut bnodes);
            self.transaction.insert(quad.as_ref());
        }
    }

    fn eval_delete_data(&mut self, operation: &DeleteDataOperation) {
        for quad in &operation.data {
            let quad = convert_ground_quad(quad);
            self.transaction.remove(quad.as_ref());
        }
    }

    fn eval_delete_insert(
        &mut self,
        operation: &DeleteInsertOperation,
        using: &QueryDatasetSpecification,
    ) -> Result<(), UpdateEvaluationError> {
        let Some(storage) = self.storage_for_initial_read.take() else {
            return Err(UpdateEvaluationError::Unexpected(
                "It is not possible to evaluate delete/insert operations on a write-only transaction after other update operations".into(),
            ));
        };
        let mut prepared = self.query_evaluator.prepare_delete_insert(
            operation.delete.clone(),
            operation.insert.clone(),
            self.base_iri.clone(),
            None,
            &operation.pattern,
        );
        *prepared.dataset_mut() = using.clone();
        let mutations = prepared
            .execute(DatasetView::new(storage.snapshot()))?
            .collect::<Result<Vec<_>, _>>()?;
        for mutation in mutations {
            match mutation {
                DeleteInsertQuad::Delete(quad) => self.transaction.remove(quad.as_ref()),
                DeleteInsertQuad::Insert(quad) => self.transaction.insert(quad.as_ref()),
            }
        }
        Ok(())
    }

    fn eval_load(&mut self, operation: &LoadOperation) -> Result<(), UpdateEvaluationError> {
        if let Err(error) = eval_load(
            operation,
            #[cfg(feature = "http-client")]
            &self.client,
            |q| self.transaction.insert(q.as_ref()),
        ) {
            if operation.silent { Ok(()) } else { Err(error) }
        } else {
            Ok(())
        }
    }

    fn eval_create(&mut self, operation: &CreateOperation) -> Result<(), UpdateEvaluationError> {
        if !operation.silent {
            return Err(UpdateEvaluationError::Unexpected(
                "Not possible to create a named graph using a write-only transaction when SILENT option is not set".into(),
            ));
        }
        self.transaction
            .insert_named_graph((&operation.graph).into());
        Ok(())
    }

    fn eval_clear(&mut self, operation: &ClearOperation) -> Result<(), UpdateEvaluationError> {
        match &operation.graph {
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

    fn eval_drop(&mut self, operation: &DropOperation) -> Result<(), UpdateEvaluationError> {
        match &operation.graph {
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
    operation: &LoadOperation,
    client: &Client,
    mut insert: impl FnMut(OxQuad),
) -> Result<(), UpdateEvaluationError> {
    let (content_type, body) = client
        .get(
            operation.source.as_str(),
            "application/n-triples, text/turtle, application/rdf+xml",
        )
        .map_err(|e| UpdateEvaluationError::Service(Box::new(e)))?;
    let format = RdfFormat::from_media_type(&content_type)
        .ok_or_else(|| UpdateEvaluationError::UnsupportedContentType(content_type))?;
    let to_graph_name = match &operation.destination {
        GraphName::NamedNode(graph_name) => graph_name.into(),
        GraphName::DefaultGraph => GraphNameRef::DefaultGraph,
    };
    let client = client.clone();
    let parser = RdfParser::from_format(format)
        .rename_blank_nodes()
        .without_named_graphs()
        .with_default_graph(to_graph_name)
        .with_base_iri(operation.source.as_str())
        .map_err(|e| {
            UpdateEvaluationError::Unexpected(
                format!("Invalid URL: {}: {e}", operation.source).into(),
            )
        })?
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
    _operation: &LoadOperation,
    _insert: impl FnMut(OxQuad),
) -> Result<(), UpdateEvaluationError> {
    Err(UpdateEvaluationError::Unexpected(
        "HTTP client is not available. Enable the feature 'http-client'".into(),
    ))
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
