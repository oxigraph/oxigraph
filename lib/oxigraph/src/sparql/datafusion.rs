use crate::model::{BlankNode, Term};
use crate::sparql::Variable;
use crate::sparql::dataset::DatasetView;
use crate::storage::binary_encoder::{decode_term, encode_term, write_term};
use crate::storage::numeric_encoder::{Decoder, EncodedTerm};
use crate::store::StorageError;
use async_trait::async_trait;
use datafusion::arrow::array::{ArrayRef, AsArray, BinaryBuilder, RecordBatch};
use datafusion::arrow::datatypes::{DataType, Field, GenericBinaryType, Schema, SchemaRef};
use datafusion::catalog::Session;
use datafusion::common::{Constraint, Constraints, JoinType, ScalarValue, Statistics};
use datafusion::config::ConfigOptions;
use datafusion::datasource::source::{DataSource, DataSourceExec};
use datafusion::datasource::{DefaultTableSource, TableProvider, TableType};
use datafusion::error::{DataFusionError, Result};
use datafusion::execution::context::SessionConfig;
use datafusion::execution::runtime_env::RuntimeEnvBuilder;
use datafusion::execution::{
    SendableRecordBatchStream, SessionState, SessionStateBuilder, TaskContext,
};
use datafusion::logical_expr::{
    BinaryExpr, Expr, LogicalPlan, LogicalPlanBuilder, Operator, TableProviderFilterPushDown,
    TableSource, and, col,
};
use datafusion::physical_expr::{EquivalenceProperties, LexOrdering, Partitioning, PhysicalExpr};
use datafusion::physical_plan::display::{DisplayableExecutionPlan, ProjectSchemaDisplay};
use datafusion::physical_plan::filter_pushdown::{FilterPushdownPropagation, PushedDown};
use datafusion::physical_plan::projection::{
    ProjectionExec, all_alias_free_columns, new_projections_for_columns,
};
use datafusion::physical_plan::{
    DisplayFormatType, ExecutionPlan, RecordBatchStream, execute_stream,
};
use futures::{Stream, StreamExt, stream};
use sparesults::QuerySolution;
use spareval::{InternalQuad, QueryableDataset};
use spargebra::Query;
use spargebra::algebra::GraphPattern;
use spargebra::term::{TermPattern, TriplePattern};
use std::any::Any;
use std::cmp::min;
use std::collections::HashMap;
use std::fmt;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

const DEFAULT_BATCH_SIZE: usize = 4096;

pub enum DatafusionQueryResults {
    Boolean(bool),
    Solutions(Pin<Box<dyn Stream<Item = Result<QuerySolution>>>>),
}

pub struct DatafusionEvaluator {
    state: SessionState,
}

impl DatafusionEvaluator {
    pub fn new() -> Result<Self> {
        let state = SessionStateBuilder::new()
            .with_config(SessionConfig::new())
            .with_runtime_env(Arc::new(RuntimeEnvBuilder::new().build()?))
            .with_default_features() //TODO
            .build();
        Ok(Self { state })
    }

    pub async fn execute(
        &self,
        dataset: DatasetView<'static>,
        query: &Query,
    ) -> Result<DatafusionQueryResults> {
        let dataset = Arc::new(dataset);
        match query {
            Query::Select { pattern, .. } => Ok(DatafusionQueryResults::Solutions(Box::pin(
                self.execute_graph_pattern(pattern, None, Arc::clone(&dataset))
                    .await?
                    .flat_map(move |batch| {
                        stream::iter(match batch {
                            Ok(batch) => {
                                let variables = batch
                                    .schema()
                                    .fields
                                    .iter()
                                    .map(|f| Variable::new_unchecked(f.name()))
                                    .collect::<Arc<[_]>>();
                                let mut results = (0..batch.num_rows())
                                    .map(|_| vec![None; batch.num_columns()])
                                    .collect::<Vec<_>>();
                                for (i, column) in batch.columns().iter().enumerate() {
                                    for (j, row) in column
                                        .as_bytes::<GenericBinaryType<i32>>()
                                        .iter()
                                        .enumerate()
                                    {
                                        if let Some(value) = row {
                                            let term = match decode_term(value)
                                                .and_then(|t| dataset.decode_term(&t))
                                            {
                                                Ok(term) => term,
                                                Err(e) => {
                                                    return stream::iter(vec![Err(e.into())]);
                                                }
                                            };
                                            results[j][i] = Some(term);
                                        }
                                    }
                                }
                                results
                                    .into_iter()
                                    .map(|r| Ok(QuerySolution::from((Arc::clone(&variables), r))))
                                    .collect()
                            }
                            Err(e) => vec![Err(e)],
                        })
                    }),
            ))),
            Query::Construct { .. } => Err(DataFusionError::NotImplemented(
                "CONSTRUCT is not implemented yet".into(),
            )),
            Query::Describe { .. } => Err(DataFusionError::NotImplemented(
                "DESCRIBE is not implemented yet".into(),
            )),
            Query::Ask { pattern, .. } => {
                let result = self
                    .execute_graph_pattern(pattern, Some(1), dataset)
                    .await?
                    .next()
                    .await
                    .transpose()?
                    .is_some_and(|batch| batch.num_rows() > 0);
                Ok(DatafusionQueryResults::Boolean(result))
            }
        }
    }

    async fn execute_graph_pattern(
        &self,
        pattern: &GraphPattern,
        limit: Option<usize>,
        dataset: Arc<DatasetView<'static>>,
    ) -> Result<SendableRecordBatchStream> {
        let plan = PlanBuilder::new(dataset).build_plan_for_graph_pattern(pattern, limit)?;
        let plan = self.state.create_physical_plan(&plan).await?;
        execute_stream(plan, Arc::new(TaskContext::from(&self.state)))
    }

    pub async fn explain(self, dataset: DatasetView<'static>, query: &Query) -> Result<String> {
        let dataset = Arc::new(dataset);
        let (pattern, limit) = match query {
            Query::Select { pattern, .. }
            | Query::Describe { pattern, .. }
            | Query::Ask { pattern, .. } => (pattern, None),
            Query::Construct { pattern, .. } => (pattern, Some(1)),
        };
        let plan = PlanBuilder::new(dataset).build_plan_for_graph_pattern(pattern, limit)?;
        let plan = self.state.create_physical_plan(&plan).await?;
        Ok(DisplayableExecutionPlan::with_full_metrics(&*plan)
            .set_show_statistics(true)
            .set_show_schema(true)
            .indent(true)
            .to_string())
    }
}

struct PlanBuilder {
    dataset: Arc<DatasetView<'static>>,
    quad_table_source: Arc<dyn TableSource>,
    table_counter: u64,
    blank_node_to_variable: HashMap<BlankNode, Variable>,
}

impl PlanBuilder {
    fn new(dataset: Arc<DatasetView<'static>>) -> Self {
        Self {
            dataset: Arc::clone(&dataset),
            quad_table_source: Arc::new(DefaultTableSource::new(Arc::new(QuadTableProvider::new(
                dataset,
            )))),
            table_counter: 0,
            blank_node_to_variable: HashMap::new(),
        }
    }

    fn build_plan_for_graph_pattern(
        &mut self,
        pattern: &GraphPattern,
        limit: Option<usize>,
    ) -> Result<LogicalPlan> {
        let (plan, variable_mapping) = self.plan_for_graph_pattern(pattern)?;
        let mut plan = plan.project(
            variable_mapping
                .into_iter()
                .map(|(to, from)| col(from).alias(to.to_string())),
        )?;
        if let Some(limit) = limit {
            plan = plan.limit(0, Some(limit))?;
        }
        plan.build()
    }

    fn plan_for_graph_pattern(
        &mut self,
        pattern: &GraphPattern,
    ) -> Result<(LogicalPlanBuilder, HashMap<Variable, String>)> {
        match pattern {
            GraphPattern::Bgp { patterns } => patterns
                .iter()
                .map(|p| self.plan_for_triple_pattern(p))
                .reduce(|l, r| {
                    let (left_plan, left_variables_mapping) = l?;
                    let (right_plan, right_variables_mapping) = r?;
                    Self::join(
                        left_plan,
                        left_variables_mapping,
                        right_plan,
                        right_variables_mapping,
                    )
                })
                .unwrap_or_else(|| Ok((LogicalPlanBuilder::empty(true), HashMap::new()))),
            GraphPattern::Path { .. } => Err(DataFusionError::NotImplemented(
                "Path patterns are not implemented yet".into(),
            )),
            GraphPattern::Join { left, right } => {
                let (left_plan, left_variables_mapping) = self.plan_for_graph_pattern(left)?;
                let (right_plan, right_variables_mapping) = self.plan_for_graph_pattern(right)?;
                Self::join(
                    left_plan,
                    left_variables_mapping,
                    right_plan,
                    right_variables_mapping,
                )
            }
            GraphPattern::Lateral { .. } => Err(DataFusionError::NotImplemented(
                "LATERAL is not implemented yet".into(),
            )),
            GraphPattern::LeftJoin { .. } => Err(DataFusionError::NotImplemented(
                "OPTIONAL is not implemented yet".into(),
            )),
            GraphPattern::Filter { .. } => Err(DataFusionError::NotImplemented(
                "FILTER is not implemented yet".into(),
            )),
            GraphPattern::Union { left, right } => {
                let (left, mut left_variables_mapping) = self.plan_for_graph_pattern(left)?;
                let (right, right_variables_mapping) = self.plan_for_graph_pattern(right)?;
                let mut right_projection = Vec::new();
                for (variable, right_var) in right_variables_mapping {
                    if let Some(left_var) = left_variables_mapping.get(&variable).cloned() {
                        right_projection.push(col(right_var).alias(left_var));
                    } else {
                        left_variables_mapping.insert(variable, right_var);
                    }
                }
                Ok((
                    left.union(right.project(right_projection)?.build()?)?,
                    left_variables_mapping,
                ))
            }
            GraphPattern::Graph { .. } => Err(DataFusionError::NotImplemented(
                "GRAPH is not implemented yet".into(),
            )),
            GraphPattern::Extend { .. } => Err(DataFusionError::NotImplemented(
                "BIND is not implemented yet".into(),
            )),
            GraphPattern::Minus { .. } => Err(DataFusionError::NotImplemented(
                "MINUS is not implemented yet".into(),
            )),
            GraphPattern::Values { .. } => Err(DataFusionError::NotImplemented(
                "VALUES is not implemented yet".into(),
            )),
            GraphPattern::OrderBy { .. } => Err(DataFusionError::NotImplemented(
                "ORDER BY is not implemented yet".into(),
            )),
            GraphPattern::Project { inner, variables } => {
                let (inner, variables_mapping) = self.plan_for_graph_pattern(inner)?;
                let plan = inner.project(variables.iter().map(|v| col(&variables_mapping[v])))?;
                let variables_mapping = variables_mapping
                    .into_iter()
                    .filter(|(v, _)| variables.contains(v))
                    .collect();
                Ok((plan, variables_mapping))
            }
            GraphPattern::Distinct { inner } => {
                let (inner, variables_mapping) = self.plan_for_graph_pattern(inner)?;
                Ok((inner.distinct()?, variables_mapping))
            }
            GraphPattern::Reduced { inner } => self.plan_for_graph_pattern(inner), // TODO
            GraphPattern::Slice {
                inner,
                start,
                length,
            } => {
                let (inner, variables_mapping) = self.plan_for_graph_pattern(inner)?;
                Ok((inner.limit(*start, *length)?, variables_mapping))
            }
            GraphPattern::Group { .. } => Err(DataFusionError::NotImplemented(
                "GROUP BY is not implemented yet".into(),
            )),
            GraphPattern::Service { .. } => Err(DataFusionError::NotImplemented(
                "SERVICE is not implemented yet".into(),
            )),
        }
    }

    fn join(
        left_plan: LogicalPlanBuilder,
        mut left_variables_mapping: HashMap<Variable, String>,
        right_plan: LogicalPlanBuilder,
        right_variables_mapping: HashMap<Variable, String>,
    ) -> Result<(LogicalPlanBuilder, HashMap<Variable, String>)> {
        let mut left_keys = Vec::new();
        let mut right_keys = Vec::new();
        for (variable, right_var) in right_variables_mapping {
            if let Some(left_var) = left_variables_mapping.get(&variable).cloned() {
                left_keys.push(left_var);
                right_keys.push(right_var);
            } else {
                left_variables_mapping.insert(variable, right_var);
            }
        }
        Ok((
            left_plan.join(
                right_plan.build()?,
                JoinType::Inner,
                (left_keys, right_keys),
                None,
            )?,
            left_variables_mapping,
        ))
    }

    fn plan_for_triple_pattern(
        &mut self,
        pattern: &TriplePattern,
    ) -> Result<(LogicalPlanBuilder, HashMap<Variable, String>)> {
        let mut filters = Vec::new();
        let mut projects = Vec::new();
        let mut variables_mapping = HashMap::new();
        self.table_counter += 1;
        let table_name = format!("triples-{}", self.table_counter);
        self.term_pattern_to_filter_or_project(
            pattern.subject.clone(),
            "subject",
            &mut filters,
            &mut projects,
            &mut variables_mapping,
        )?;
        self.term_pattern_to_filter_or_project(
            pattern.predicate.clone().into(),
            "predicate",
            &mut filters,
            &mut projects,
            &mut variables_mapping,
        )?;
        self.term_pattern_to_filter_or_project(
            pattern.object.clone(),
            "object",
            &mut filters,
            &mut projects,
            &mut variables_mapping,
        )?;
        let mut plan =
            LogicalPlanBuilder::scan("quads", Arc::clone(&self.quad_table_source), None)?
                .alias(table_name)?;
        if let Some(filters) = filters.into_iter().reduce(and) {
            plan = plan.filter(filters)?;
        }
        if !projects.is_empty() {
            plan = plan.project(projects)?;
        }
        Ok((plan, variables_mapping))
    }

    fn term_pattern_to_filter_or_project(
        &mut self,
        pattern: TermPattern,
        column: &'static str,
        filters: &mut Vec<Expr>,
        projects: &mut Vec<Expr>,
        variables_mapping: &mut HashMap<Variable, String>,
    ) -> Result<()> {
        match pattern {
            TermPattern::NamedNode(n) => {
                filters.push(self.column_with_term_eq("subject", n));
                Ok(())
            }
            TermPattern::BlankNode(n) => {
                projects.push(Self::column_as_var(
                    column,
                    self.blank_node_to_variable
                        .entry(n.clone())
                        .or_insert_with(|| Variable::new_unchecked(n.to_string()))
                        .clone(),
                    self.table_counter,
                    variables_mapping,
                ));
                Ok(())
            }
            TermPattern::Literal(l) => {
                filters.push(self.column_with_term_eq(column, l));
                Ok(())
            }
            #[cfg(feature = "rdf-12")]
            TermPattern::Triple(_) => Err(DataFusionError::NotImplemented(
                "RDF 1.2 triple terms are not implemented yet".into(),
            )),
            TermPattern::Variable(v) => {
                projects.push(Self::column_as_var(
                    column,
                    v,
                    self.table_counter,
                    variables_mapping,
                ));
                Ok(())
            }
        }
    }

    fn column_with_term_eq(&self, column: &'static str, term: impl Into<Term>) -> Expr {
        col(column).eq(Expr::Literal(
            ScalarValue::Binary(Some(encode_term(
                &self.dataset.internalize_term(term.into()).unwrap(),
            ))),
            None,
        ))
    }

    fn column_as_var(
        column: &'static str,
        variable: Variable,
        table_counter: u64,
        variables_mapping: &mut HashMap<Variable, String>,
    ) -> Expr {
        let var = format!("{variable}-{table_counter}");
        variables_mapping.insert(variable, var.clone());
        col(column).alias(var)
    }
}

struct QuadTableProvider {
    dataset: Arc<DatasetView<'static>>,
    schema: SchemaRef,
    constraints: Constraints,
}

impl QuadTableProvider {
    fn new(dataset: Arc<DatasetView<'static>>) -> Self {
        Self {
            dataset,
            schema: Arc::new(Schema::new(vec![
                Field::new("subject", DataType::Binary, false),
                Field::new("predicate", DataType::Binary, false),
                Field::new("object", DataType::Binary, false),
            ])),
            constraints: Constraints::new_unverified(vec![Constraint::PrimaryKey(vec![0, 1, 2])]), /* TODO: graph_name */
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
        for filter in filters {
            match filter {
                Expr::BinaryExpr(BinaryExpr {
                    left,
                    op: Operator::Eq,
                    right,
                }) => match (&**left, &**right) {
                    (Expr::Column(c), Expr::Literal(ScalarValue::Binary(Some(b)), _))
                    | (Expr::Literal(ScalarValue::Binary(Some(b)), _), Expr::Column(c)) => {
                        let value = decode_term(b)?;
                        match c.name.as_str() {
                            "subject" => {
                                subject = Some(value);
                            }
                            "predicate" => {
                                predicate = Some(value);
                            }
                            "object" => {
                                object = Some(value);
                            }
                            _ => {
                                return Err(DataFusionError::Internal(
                                    "Unsupported filer pushed in the scan".into(),
                                ));
                            }
                        }
                    }
                    _ => {
                        return Err(DataFusionError::Internal(
                            "Unsupported filer pushed in the scan".into(),
                        ));
                    }
                },
                _ => {
                    return Err(DataFusionError::Internal(
                        "Unsupported filer pushed in the scan".into(),
                    ));
                }
            }
        }
        Ok(DataSourceExec::from_data_source(QuadDataSource {
            dataset: Arc::clone(&self.dataset),
            subject,
            predicate,
            object,
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
                }) if matches!(**left, Expr::Column(_))
                    && matches!(**right, Expr::Literal(ScalarValue::Binary(Some(_)), _))
                    || matches!(**left, Expr::Literal(ScalarValue::Binary(Some(_)), _))
                        && matches!(**right, Expr::Column(_)) =>
                {
                    TableProviderFilterPushDown::Exact
                }
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
    limit: Option<usize>,
}

impl fmt::Debug for QuadDataSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("QuadDataSource").finish()
    }
}

impl DataSource for QuadDataSource {
    fn open(
        &self,
        partition: usize,
        _context: Arc<TaskContext>,
    ) -> Result<SendableRecordBatchStream> {
        assert_eq!(partition, 0, "Only a single partition is supported");
        Ok(Box::pin(QuadStream {
            iter: self.dataset.internal_quads_for_pattern(
                self.subject.as_ref(),
                self.predicate.as_ref(),
                self.object.as_ref(),
                Some(None),
            ),
            // Let's build a first batch of limit size if small enough
            batch_size: self
                .limit
                .map_or(DEFAULT_BATCH_SIZE, |l| min(l, DEFAULT_BATCH_SIZE)),
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

    fn statistics(&self) -> Result<Statistics> {
        Ok(Statistics::new_unknown(&self.schema)) // TODO
    }

    fn with_fetch(&self, _limit: Option<usize>) -> Option<Arc<dyn DataSource>> {
        None
    }

    fn fetch(&self) -> Option<usize> {
        None
    }

    fn try_swapping_with_projection(
        &self,
        projection: &ProjectionExec,
    ) -> Result<Option<Arc<dyn ExecutionPlan>>> {
        if !all_alias_free_columns(projection.expr()) {
            // We only handle dropping columns
            return Ok(None);
        }
        let new_projections = new_projections_for_columns(
            projection,
            &(0..self.schema.fields().len()).collect::<Vec<_>>(),
        );
        Ok(Some(DataSourceExec::from_data_source(QuadDataSource {
            dataset: Arc::clone(&self.dataset),
            subject: self.subject.clone(),
            predicate: self.predicate.clone(),
            object: self.object.clone(),
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

struct QuadStream {
    iter: Box<dyn Iterator<Item = Result<InternalQuad<EncodedTerm>, StorageError>> + Send>,
    schema: SchemaRef,
    batch_size: usize,
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
        let batch_size = self.batch_size;
        Poll::Ready(
            match (&mut self.iter)
                .take(batch_size)
                .collect::<Result<Vec<_>, _>>()
            {
                Ok(batch) => {
                    if batch.is_empty() {
                        None
                    } else {
                        let mut columns = Vec::new();
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
                                    columns.push(Arc::new(subjects.finish()) as ArrayRef);
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
                                _ => unreachable!(),
                            }
                        }
                        Some(Ok(RecordBatch::try_new(Arc::clone(&self.schema), columns)?))
                    }
                }
                Err(e) => Some(Err(e.into())),
            },
        )
    }
}

impl From<StorageError> for DataFusionError {
    fn from(e: StorageError) -> Self {
        DataFusionError::External(Box::new(e))
    }
}
