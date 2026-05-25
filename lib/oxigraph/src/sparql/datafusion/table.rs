use crate::sparql::dataset::{DatasetQuadIterator, DatasetView};
use crate::storage::binary_encoder::{decode_term, write_term};
use crate::storage::numeric_encoder::EncodedTerm;
use async_trait::async_trait;
use datafusion::arrow::array::{ArrayRef, BinaryBuilder, RecordBatch, RecordBatchOptions};
use datafusion::arrow::datatypes::{DataType, Field, Schema, SchemaRef};
use datafusion::catalog::Session;
use datafusion::common::stats::Precision;
use datafusion::common::{Constraint, Constraints, ScalarValue, Statistics, internal_err};
use datafusion::config::ConfigOptions;
use datafusion::datasource::source::{DataSource, DataSourceExec};
use datafusion::datasource::{TableProvider, TableType};
use datafusion::error::{DataFusionError, Result};
use datafusion::execution::{SendableRecordBatchStream, TaskContext};
use datafusion::logical_expr::{BinaryExpr, Expr, Operator, TableProviderFilterPushDown};
use datafusion::physical_expr::{EquivalenceProperties, LexOrdering, Partitioning, PhysicalExpr};
use datafusion::physical_plan::display::ProjectSchemaDisplay;
use datafusion::physical_plan::filter_pushdown::{FilterPushdownPropagation, PushedDown};
use datafusion::physical_plan::projection::{
    ProjectionExprs, all_alias_free_columns, new_projections_for_columns,
};
use datafusion::physical_plan::{DisplayFormatType, ExecutionPlan, RecordBatchStream};
use futures::Stream;
use spareval::QueryableDataset;
use std::any::Any;
use std::cmp::min;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

const DEFAULT_BATCH_SIZE: usize = 4096;

pub struct QuadTableProvider {
    dataset: Arc<DatasetView<'static>>,
    schema: SchemaRef,
    constraints: Constraints,
}

impl QuadTableProvider {
    pub fn new(dataset: Arc<DatasetView<'static>>) -> Self {
        Self {
            dataset,
            schema: Arc::new(Schema::new(vec![
                Field::new("subject", DataType::Binary, false),
                Field::new("predicate", DataType::Binary, false),
                Field::new("object", DataType::Binary, false),
                Field::new("graph_name", DataType::Binary, true),
            ])),
            constraints: Constraints::new_unverified(vec![Constraint::PrimaryKey(vec![
                0, 1, 2, 3,
            ])]),
        }
    }
}

impl fmt::Debug for QuadTableProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QuadTableProvider").finish()
    }
}

#[async_trait]
impl TableProvider for QuadTableProvider {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }

    fn constraints(&self) -> Option<&Constraints> {
        Some(&self.constraints)
    }

    fn table_type(&self) -> TableType {
        TableType::Base
    }

    async fn scan(
        &self,
        _state: &dyn Session,
        projection: Option<&Vec<usize>>,
        filters: &[Expr],
        limit: Option<usize>,
    ) -> Result<Arc<dyn ExecutionPlan>> {
        let schema = if let Some(projection) = projection {
            Arc::new(self.schema.project(projection)?)
        } else {
            Arc::clone(&self.schema)
        };
        let mut subject = None;
        let mut predicate = None;
        let mut object = None;
        let mut graph_name = None;
        for filter in filters {
            match filter {
                Expr::BinaryExpr(BinaryExpr {
                    left,
                    op: Operator::Eq,
                    right,
                }) => match (&**left, &**right) {
                    (Expr::Column(c), Expr::Literal(ScalarValue::Binary(b), _))
                    | (Expr::Literal(ScalarValue::Binary(b), _), Expr::Column(c)) => {
                        let value = b.as_deref().map(decode_term).transpose()?;
                        match c.name.as_str() {
                            "subject" => {
                                subject = Some(value.ok_or_else(|| {
                                    DataFusionError::Internal(
                                        "subject column is not nullable".into(),
                                    )
                                })?);
                            }
                            "predicate" => {
                                predicate = Some(value.ok_or_else(|| {
                                    DataFusionError::Internal(
                                        "predicate column is not nullable".into(),
                                    )
                                })?);
                            }
                            "object" => {
                                object = Some(value.ok_or_else(|| {
                                    DataFusionError::Internal(
                                        "object column is not nullable".into(),
                                    )
                                })?);
                            }
                            "graph_name" => {
                                graph_name = Some(value);
                            }
                            _ => {
                                return internal_err!("Unsupported filer pushed in the scan");
                            }
                        }
                    }
                    _ => {
                        return internal_err!("Unsupported filer pushed in the scan");
                    }
                },
                Expr::IsNull(e) => match &**e {
                    Expr::Column(c) => {
                        if c.name() == "graph_name" {
                            graph_name = Some(None);
                        } else {
                            return internal_err!("graph_name is the only nullable column");
                        }
                    }
                    _ => {
                        return internal_err!("Unsupported filer pushed in the scan");
                    }
                },
                _ => {
                    return internal_err!("Unsupported filer pushed in the scan");
                }
            }
        }
        Ok(DataSourceExec::from_data_source(QuadDataSource {
            dataset: Arc::clone(&self.dataset),
            subject,
            predicate,
            object,
            graph_name,
            limit,
            schema,
        }))
    }

    fn supports_filters_pushdown(
        &self,
        filters: &[&Expr],
    ) -> Result<Vec<TableProviderFilterPushDown>> {
        Ok(filters
            .iter()
            .map(|f| match f {
                Expr::BinaryExpr(BinaryExpr {
                    left,
                    op: Operator::Eq,
                    right,
                }) => match (&**left, &**right) {
                    (Expr::Column(_), Expr::Literal(ScalarValue::Binary(_), _))
                    | (Expr::Literal(ScalarValue::Binary(_), _), Expr::Column(_)) => {
                        TableProviderFilterPushDown::Exact
                    }
                    _ => TableProviderFilterPushDown::Unsupported,
                },
                Expr::IsNull(e) => match &**e {
                    Expr::Column(_) => TableProviderFilterPushDown::Exact,
                    _ => TableProviderFilterPushDown::Unsupported,
                },
                _ => TableProviderFilterPushDown::Unsupported,
            })
            .collect())
    }

    fn statistics(&self) -> Option<Statistics> {
        None
    }
}

struct QuadDataSource {
    dataset: Arc<DatasetView<'static>>,
    schema: SchemaRef,
    subject: Option<EncodedTerm>,
    predicate: Option<EncodedTerm>,
    object: Option<EncodedTerm>,
    graph_name: Option<Option<EncodedTerm>>,
    limit: Option<usize>,
}

impl fmt::Debug for QuadDataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut s = f.debug_struct("QuadDataSource");
        s.field(
            "projection",
            &ProjectSchemaDisplay(&self.schema).to_string(),
        );
        if let Some(subject) = self.subject.clone() {
            s.field(
                "subject",
                &self
                    .dataset
                    .externalize_term(subject)
                    .map_err(|_| fmt::Error)?,
            );
        }
        if let Some(predicate) = self.predicate.clone() {
            s.field(
                "predicate",
                &self
                    .dataset
                    .externalize_term(predicate)
                    .map_err(|_| fmt::Error)?,
            );
        }
        if let Some(object) = self.object.clone() {
            s.field(
                "object",
                &self
                    .dataset
                    .externalize_term(object)
                    .map_err(|_| fmt::Error)?,
            );
        }
        if let Some(graph_name) = self.graph_name.clone() {
            if let Some(graph_name) = graph_name {
                s.field(
                    "graph_name",
                    &self
                        .dataset
                        .externalize_term(graph_name)
                        .map_err(|_| fmt::Error)?,
                );
            } else {
                s.field("graph_name", &"default");
            }
        }
        if let Some(limit) = self.limit {
            s.field("limit", &limit);
        }
        s.finish()
    }
}

impl DataSource for QuadDataSource {
    fn open(
        &self,
        partition: usize,
        _context: Arc<TaskContext>,
    ) -> Result<SendableRecordBatchStream> {
        debug_assert_eq!(partition, 0, "Only a single partition is supported");
        Ok(Box::pin(QuadStream {
            iter: QuadIter {
                iter: self.dataset.internal_quads_for_pattern(
                    self.subject.as_ref(),
                    self.predicate.as_ref(),
                    self.object.as_ref(),
                    self.graph_name.as_ref().map(|g| g.as_ref()),
                ),
                // Let's build a first batch of limit size if small enough
                batch_size: self
                    .limit
                    .map_or(DEFAULT_BATCH_SIZE, |l| min(l, DEFAULT_BATCH_SIZE)),
                schema: Arc::clone(&self.schema),
            },
            schema: Arc::clone(&self.schema),
        }))
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn fmt_as(&self, _: DisplayFormatType, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "projection={}", ProjectSchemaDisplay(&self.schema))?;
        if let Some(subject) = self.subject.clone() {
            write!(
                f,
                ", subject={}",
                self.dataset
                    .externalize_term(subject)
                    .map_err(|_| fmt::Error)?
            )?;
        }
        if let Some(predicate) = self.predicate.clone() {
            write!(
                f,
                ", predicate={}",
                self.dataset
                    .externalize_term(predicate)
                    .map_err(|_| fmt::Error)?
            )?;
        }
        if let Some(object) = self.object.clone() {
            write!(
                f,
                ", object={}",
                self.dataset
                    .externalize_term(object)
                    .map_err(|_| fmt::Error)?
            )?;
        }
        if let Some(graph_name) = self.graph_name.clone() {
            if let Some(graph_name) = graph_name {
                write!(
                    f,
                    ", graph_name={}",
                    self.dataset
                        .externalize_term(graph_name)
                        .map_err(|_| fmt::Error)?
                )?;
            } else {
                write!(f, ", graph_name=default")?;
            }
        }
        if let Some(limit) = self.limit {
            write!(f, ", limit={limit}")?;
        }
        Ok(())
    }

    /// If possible, redistribute batches across partitions according to their size.
    ///
    /// Returns `Ok(None)` if unable to repartition. Preserve output ordering if exists.
    /// Refer to [`DataSource::repartitioned`] for further details.
    fn repartitioned(
        &self,
        _target_partitions: usize,
        _repartition_file_min_size: usize,
        _output_ordering: Option<LexOrdering>,
    ) -> Result<Option<Arc<dyn DataSource>>> {
        Ok(None)
    }

    fn output_partitioning(&self) -> Partitioning {
        Partitioning::UnknownPartitioning(1)
    }

    fn eq_properties(&self) -> EquivalenceProperties {
        EquivalenceProperties::new(Arc::clone(&self.schema))
    }

    fn partition_statistics(&self, partition: Option<usize>) -> Result<Statistics> {
        if !matches!(partition, None | Some(0)) {
            return internal_err!("Only a single partition is supported by QuadDataSource");
        }
        let size = estimate_triple_pattern_size(
            self.subject.is_some(),
            self.predicate.is_some(),
            self.object.is_some(),
        );
        Ok(Statistics::new_unknown(&self.schema)
            .with_num_rows(Precision::Inexact(size))
            .with_total_byte_size(Precision::Inexact(size * 17 * 3)))
    }

    fn with_fetch(&self, _limit: Option<usize>) -> Option<Arc<dyn DataSource>> {
        None
    }

    fn fetch(&self) -> Option<usize> {
        None
    }

    fn try_swapping_with_projection(
        &self,
        projection: &ProjectionExprs,
    ) -> Result<Option<Arc<dyn DataSource>>> {
        if !all_alias_free_columns(projection.as_ref()) {
            // We only handle dropping columns
            return Ok(None);
        }
        let new_projections = new_projections_for_columns(
            projection.as_ref(),
            &(0..self.schema.fields().len()).collect::<Vec<_>>(),
        );
        Ok(Some(Arc::new(QuadDataSource {
            dataset: Arc::clone(&self.dataset),
            subject: self.subject.clone(),
            predicate: self.predicate.clone(),
            object: self.object.clone(),
            graph_name: self.graph_name.clone(),
            limit: self.limit,
            schema: Arc::new(self.schema.project(&new_projections)?),
        })))
    }

    fn try_pushdown_filters(
        &self,
        filters: Vec<Arc<dyn PhysicalExpr>>,
        _config: &ConfigOptions,
    ) -> Result<FilterPushdownPropagation<Arc<dyn DataSource>>> {
        // TODO: might be relevant
        Ok(FilterPushdownPropagation::with_parent_pushdown_result(
            vec![PushedDown::No; filters.len()],
        ))
    }
}

fn estimate_triple_pattern_size(
    subject_bound: bool,
    predicate_bound: bool,
    object_bound: bool,
) -> usize {
    match (subject_bound, predicate_bound, object_bound) {
        (true, true, true) => 1,
        (true, true, false) => 10,
        (true, false, true) => 2,
        (false, true, true) => 10_000,
        (true, false, false) => 100,
        (false, false, false) => 1_000_000_000,
        (false, true, false) => 1_000_000,
        (false, false, true) => 100_000,
    }
}

struct QuadStream {
    iter: QuadIter,
    schema: SchemaRef,
}

impl RecordBatchStream for QuadStream {
    fn schema(&self) -> SchemaRef {
        Arc::clone(&self.schema)
    }
}

impl Stream for QuadStream {
    type Item = Result<RecordBatch>;

    fn poll_next(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
    ) -> Poll<Option<Result<RecordBatch>>> {
        Poll::Ready(self.iter.next())
    }
}

struct QuadIter {
    iter: DatasetQuadIterator<'static>,
    schema: SchemaRef,
    batch_size: usize,
}

impl Iterator for QuadIter {
    type Item = Result<RecordBatch>;
    fn next(&mut self) -> Option<Result<RecordBatch>> {
        let batch = match (&mut self.iter)
            .take(self.batch_size)
            .collect::<Result<Vec<_>, _>>()
        {
            Ok(batch) => batch,
            Err(e) => {
                return Some(Err(e.into()));
            }
        };
        if batch.is_empty() {
            None
        } else {
            let mut columns = Vec::<ArrayRef>::new();
            let mut buffer = Vec::with_capacity(33);
            for field in self.schema.fields() {
                match field.name().as_str() {
                    "subject" => {
                        let mut subjects =
                            BinaryBuilder::with_capacity(batch.len(), batch.len() * 17);
                        for quad in &batch {
                            buffer.clear();
                            write_term(&mut buffer, &quad.subject);
                            subjects.append_value(&buffer);
                        }
                        columns.push(Arc::new(subjects.finish()));
                    }
                    "predicate" => {
                        let mut predicates =
                            BinaryBuilder::with_capacity(batch.len(), batch.len() * 17);
                        for quad in &batch {
                            buffer.clear();
                            write_term(&mut buffer, &quad.predicate);
                            predicates.append_value(&buffer);
                        }
                        columns.push(Arc::new(predicates.finish()));
                    }
                    "object" => {
                        let mut objects =
                            BinaryBuilder::with_capacity(batch.len(), batch.len() * 17);
                        for quad in &batch {
                            buffer.clear();
                            write_term(&mut buffer, &quad.object);
                            objects.append_value(&buffer);
                        }
                        columns.push(Arc::new(objects.finish()));
                    }
                    "graph_name" => {
                        let mut graph_names =
                            BinaryBuilder::with_capacity(batch.len(), batch.len() * 17);
                        for quad in &batch {
                            if let Some(graph_name) = &quad.graph_name {
                                buffer.clear();
                                write_term(&mut buffer, graph_name);
                                graph_names.append_value(&buffer);
                            } else {
                                graph_names.append_null();
                            }
                        }
                        columns.push(Arc::new(graph_names.finish()));
                    }
                    _ => unreachable!(),
                }
            }
            Some(
                RecordBatch::try_new_with_options(
                    Arc::clone(&self.schema),
                    columns,
                    &RecordBatchOptions::new().with_row_count(Some(batch.len())),
                )
                .map_err(Into::into),
            )
        }
    }
}
