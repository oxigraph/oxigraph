use crate::sparql::Variable;
use crate::sparql::datafusion::table::QuadTableProvider;
use crate::sparql::dataset::DatasetView;
use crate::storage::binary_encoder::{decode_term, encode_term, write_term};
use crate::storage::numeric_encoder::Decoder;
use crate::store::StorageError;
use datafusion::arrow::array::{Array, ArrayRef, BinaryArray, BinaryBuilder, NullArray};
use datafusion::arrow::datatypes::DataType;
use datafusion::arrow::record_batch::RecordBatch;
use datafusion::common::{Result, ScalarValue, downcast_value, internal_err};
use datafusion::datasource::DefaultTableSource;
use datafusion::error::DataFusionError;
use datafusion::execution::context::SessionConfig;
use datafusion::execution::runtime_env::RuntimeEnvBuilder;
use datafusion::execution::{
    SendableRecordBatchStream, SessionState, SessionStateBuilder, TaskContext,
};
use datafusion::logical_expr::{Literal, LogicalPlan, LogicalPlanBuilder};
use datafusion::physical_plan::ExecutionPlan;
use datafusion::physical_plan::display::DisplayableExecutionPlan;
use datafusion::physical_plan::execute_stream;
use datafusion::datasource::memory::MemorySourceConfig;
use datafusion::datasource::source::DataSourceExec;
use datafusion::physical_plan::collect;
use datafusion::physical_plan::recursive_query::RecursiveQueryExec;
use datafusion::physical_plan::work_table::WorkTableExec;
use futures::StreamExt;
use oxrdf::{Term, Triple};
use sparesults::QuerySolution;
use spareval::{
    ExpressionTerm, QueryEvaluationError, QueryResults, QuerySolutionIter, QueryTripleIter,
    QueryableDataset,
};
use spareval_fusion::dataset::{ExpressionTermEncoder, QueryableDatasetAccess};
use spareval_fusion::plan_builder::SparqlPlanBuilder;
use spargebra::Query;
use std::sync::Arc;
use std::vec::IntoIter;
use tokio::runtime::{Builder, Runtime};

mod table;

pub struct DatafusionEvaluator {
    state: SessionState,
    runtime: Arc<Runtime>,
}

impl DatafusionEvaluator {
    pub fn new() -> Result<Self, QueryEvaluationError> {
        // Force single-partition execution. Recursive SPARQL paths produce
        // small per-iteration batches; the parallel HashJoin plus
        // RepartitionExec layers the default optimizer adds dominate the
        // per-round cost and end up being slower than serial execution.
        let config = SessionConfig::new().with_target_partitions(1);
        let state = SessionStateBuilder::new()
            .with_config(config)
            .with_runtime_env(Arc::new(RuntimeEnvBuilder::new().build().map_err(map_df_error)?))
            .with_default_features() //TODO
            .build();
        Ok(Self {
            state,
            runtime: Arc::new(
                Builder::new_current_thread()
                    .build()
                    .map_err(|e| QueryEvaluationError::Unexpected(Box::new(e)))?,
            ),
        })
    }

    pub fn execute(
        &self,
        dataset: DatasetView<'static>,
        query: &Query,
    ) -> Result<QueryResults<'static>, QueryEvaluationError> {
        // TODO: implement as much as possible in DataFusion
        self.runtime.block_on(async {
            let dataset = Arc::new(dataset);
            let plan = Self::query_plan(query, Arc::clone(&dataset))?;
            let (variables, mut stream) = self.execute_plan(plan).await?;
            Ok(match query {
                Query::Select { .. } => QueryResults::Solutions(QuerySolutionIter::new(
                    Arc::clone(&variables),
                    QuerySolutionStreamWrapper {
                        runtime: Arc::clone(&self.runtime),
                        stream,
                        variables,
                        dataset,
                        buffer: Vec::new().into_iter(),
                    },
                )),
                Query::Construct { .. } | Query::Describe { .. } => {
                    QueryResults::Graph(QueryTripleIter::new(QueryTripleStreamWrapper {
                        runtime: Arc::clone(&self.runtime),
                        stream,
                        dataset,
                        buffer: Vec::new().into_iter(),
                    }))
                }
                Query::Ask { .. } => {
                    while let Some(batch) = stream.next().await {
                        if batch.map_err(map_df_error)?.num_rows() > 0 {
                            return Ok(QueryResults::Boolean(true));
                        }
                    }
                    QueryResults::Boolean(false)
                }
            })
        })
    }

    fn query_plan(
        query: &Query,
        dataset: Arc<DatasetView<'static>>,
    ) -> Result<LogicalPlan, QueryEvaluationError> {
        SparqlPlanBuilder::new(OxigraphQueryableDataset::new(dataset))
            .query_plan(query)
            .map_err(map_df_error)?
            .build()
            .map_err(map_df_error)
    }

    async fn execute_plan(
        &self,
        logical_plan: LogicalPlan,
    ) -> Result<(Arc<[Variable]>, SendableRecordBatchStream), QueryEvaluationError> {
        let verbose = std::env::var("OXIGRAPH_DATAFUSION_TIMING").is_ok();
        let t0 = std::time::Instant::now();
        let physical_plan = self
            .state
            .create_physical_plan(&logical_plan)
            .await
            .map_err(map_df_error)?;
        if verbose {
            eprintln!("timing: create_physical_plan = {} ms", t0.elapsed().as_secs_f64() * 1000.0);
        }
        let task_ctx = Arc::new(TaskContext::from(&self.state));
        let t1 = std::time::Instant::now();
        let physical_plan =
            materialize_recursive_term_subtrees(physical_plan, Arc::clone(&task_ctx))
                .await
                .map_err(map_df_error)?;
        if verbose {
            eprintln!("timing: materialize = {} ms", t1.elapsed().as_secs_f64() * 1000.0);
        }
        let t2 = std::time::Instant::now();
        let physical_plan =
            precompute_recursive_closures(physical_plan, Arc::clone(&task_ctx))
                .await
                .map_err(map_df_error)?;
        if verbose {
            eprintln!("timing: precompute_closures = {} ms", t2.elapsed().as_secs_f64() * 1000.0);
        }
        let variables = physical_plan
            .schema()
            .fields()
            .iter()
            .map(|f| Variable::new_unchecked(f.name()))
            .collect();
        Ok((
            variables,
            execute_stream(physical_plan, task_ctx).map_err(map_df_error)?,
        ))
    }

    pub fn explain(
        self,
        dataset: DatasetView<'static>,
        query: &Query,
    ) -> Result<String, QueryEvaluationError> {
        let logical_plan = Self::query_plan(query, Arc::new(dataset))?;
        let logical_plan = self.state.optimize(&logical_plan).map_err(map_df_error)?;
        let physical_plan = self
            .runtime
            .block_on(
                self.state
                    .query_planner()
                    .create_physical_plan(&logical_plan, &self.state),
            )
            .map_err(map_df_error)?;
        let displayable_execution_plan =
            DisplayableExecutionPlan::with_full_metrics(&*physical_plan)
                .set_show_statistics(true)
                .set_show_schema(true)
                .indent(true);
        Ok(format!("{logical_plan}\n\n{displayable_execution_plan}"))
    }
}

impl From<StorageError> for DataFusionError {
    fn from(e: StorageError) -> Self {
        DataFusionError::External(Box::new(e))
    }
}

/// Walks the physical plan and, for every `RecursiveQueryExec`, materialises
/// the parts of the recursive term that do not reference the working table
/// into an in-memory data source. This avoids re-executing the (constant)
/// edge relation on every iteration, which is the dominant cost for property
/// path closures on chain shaped or hierarchical data.
async fn materialize_recursive_term_subtrees(
    plan: Arc<dyn ExecutionPlan>,
    task_ctx: Arc<TaskContext>,
) -> Result<Arc<dyn ExecutionPlan>, DataFusionError> {
    if plan.as_any().is::<RecursiveQueryExec>() {
        // children() returns the static term then the recursive term.
        let children = plan.children();
        if children.len() != 2 {
            return Ok(plan);
        }
        let static_term = Arc::clone(children[0]);
        let recursive_term = Arc::clone(children[1]);
        // Recurse into the static term first (in case it itself contains
        // recursion), then materialise non-working-table subtrees of the
        // recursive term.
        let static_term = Box::pin(materialize_recursive_term_subtrees(
            static_term,
            Arc::clone(&task_ctx),
        ))
        .await?;
        let recursive_term =
            materialize_non_worktable_subtrees(recursive_term, Arc::clone(&task_ctx)).await?;
        return plan.with_new_children(vec![static_term, recursive_term]);
    }
    // Non-recursive node: descend into children unchanged.
    let mut new_children = Vec::with_capacity(plan.children().len());
    for child in plan.children() {
        new_children.push(
            Box::pin(materialize_recursive_term_subtrees(
                Arc::clone(child),
                Arc::clone(&task_ctx),
            ))
            .await?,
        );
    }
    if new_children.is_empty() {
        Ok(plan)
    } else {
        plan.with_new_children(new_children)
    }
}

async fn materialize_non_worktable_subtrees(
    plan: Arc<dyn ExecutionPlan>,
    task_ctx: Arc<TaskContext>,
) -> Result<Arc<dyn ExecutionPlan>, DataFusionError> {
    let single_partition = plan.properties().partitioning.partition_count() == 1;
    if single_partition && !contains_work_table(plan.as_ref()) {
        let schema = plan.schema();
        let batches = collect(Arc::clone(&plan), Arc::clone(&task_ctx)).await?;
        let mem = MemorySourceConfig::try_new(&[batches], schema, None)?;
        return Ok(DataSourceExec::from_data_source(mem));
    }
    // Either contains the working table, or has non-trivial partitioning:
    // recurse into children rather than replacing this node.
    let mut new_children = Vec::with_capacity(plan.children().len());
    for child in plan.children() {
        new_children.push(
            Box::pin(materialize_non_worktable_subtrees(
                Arc::clone(child),
                Arc::clone(&task_ctx),
            ))
            .await?,
        );
    }
    if new_children.is_empty() {
        Ok(plan)
    } else {
        plan.with_new_children(new_children)
    }
}

fn contains_work_table(plan: &dyn ExecutionPlan) -> bool {
    if plan.as_any().is::<WorkTableExec>() {
        return true;
    }
    plan.children().iter().any(|c| contains_work_table(c.as_ref()))
}

/// Walks the physical plan and, for every `RecursiveQueryExec` matching our
/// OneOrMore property path lowering pattern (two binary columns, output is
/// single partition, edge data already materialised by the previous pass),
/// pre-executes the seed and edges and computes the transitive closure in
/// pure Rust. The whole `RecursiveQueryExec` is replaced with a
/// `DataSourceExec` over the precomputed result.
///
/// This bypasses DataFusion's per-iteration `reset_plan_states` + execute()
/// overhead, which dominates the cost on chain-shaped graphs at medium and
/// larger sizes.
async fn precompute_recursive_closures(
    plan: Arc<dyn ExecutionPlan>,
    task_ctx: Arc<TaskContext>,
) -> Result<Arc<dyn ExecutionPlan>, DataFusionError> {
    // Recurse into children first so any nested recursive queries are also
    // rewritten before we look at this node.
    let mut new_children = Vec::with_capacity(plan.children().len());
    for child in plan.children() {
        new_children.push(
            Box::pin(precompute_recursive_closures(
                Arc::clone(child),
                Arc::clone(&task_ctx),
            ))
            .await?,
        );
    }
    let plan = if new_children.is_empty() {
        plan
    } else {
        plan.with_new_children(new_children)?
    };

    if !plan.as_any().is::<RecursiveQueryExec>() {
        return Ok(plan);
    }
    let output_schema = plan.schema();
    let verbose = std::env::var("OXIGRAPH_DATAFUSION_CLOSURE_LOG").is_ok();
    if verbose {
        eprintln!(
            "closure: hit RecursiveQueryExec fields={} types=[{}] partitions={}",
            output_schema.fields().len(),
            output_schema
                .fields()
                .iter()
                .map(|f| format!("{}:{}", f.name(), f.data_type()))
                .collect::<Vec<_>>()
                .join(", "),
            plan.properties().partitioning.partition_count(),
        );
    }

    if output_schema.fields().len() != 2 {
        if verbose { eprintln!("closure: skip, not 2 columns"); }
        return Ok(plan);
    }
    for f in output_schema.fields() {
        if f.data_type() != &DataType::Binary {
            if verbose { eprintln!("closure: skip, non-binary column {}", f.name()); }
            return Ok(plan);
        }
    }
    if plan.properties().partitioning.partition_count() != 1 {
        if verbose { eprintln!("closure: skip, multi-partition"); }
        return Ok(plan);
    }

    let children = plan.children();
    if children.len() != 2 {
        return Ok(plan);
    }
    let static_term = Arc::clone(children[0]);
    let recursive_term = Arc::clone(children[1]);

    let Some(edge_plan) = find_unique_non_worktable_subtree(&recursive_term) else {
        if verbose { eprintln!("closure: skip, no unique edge subtree"); }
        return Ok(plan);
    };

    let seed_batches = collect(static_term, Arc::clone(&task_ctx)).await?;
    let edge_batches = collect(edge_plan, Arc::clone(&task_ctx)).await?;

    if verbose {
        let seed_rows: usize = seed_batches.iter().map(|b| b.num_rows()).sum();
        let edge_rows: usize = edge_batches.iter().map(|b| b.num_rows()).sum();
        eprintln!(
            "closure: precomputing seed_rows={} edge_rows={}",
            seed_rows, edge_rows
        );
    }

    let result_batches = match compute_closure(&seed_batches, &edge_batches, &output_schema) {
        Ok(b) => b,
        Err(e) => {
            if verbose { eprintln!("closure: compute_closure failed: {e}"); }
            return Ok(plan);
        }
    };

    if verbose {
        let result_rows: usize = result_batches.iter().map(|b| b.num_rows()).sum();
        eprintln!("closure: result_rows={}", result_rows);
    }

    let mem = MemorySourceConfig::try_new(&[result_batches], output_schema, None)?;
    Ok(DataSourceExec::from_data_source(mem))
}

/// Returns Some(subtree) if there is exactly one subtree without a
/// WorkTableExec inside `plan`. The "top-most" rule means: walk down until
/// we either find a WorkTableExec-free node (return it) or hit children
/// where some have a WorkTable and some don't. In the latter case, return
/// the unique WorkTable-free child (if there's exactly one).
fn find_unique_non_worktable_subtree(
    plan: &Arc<dyn ExecutionPlan>,
) -> Option<Arc<dyn ExecutionPlan>> {
    if !contains_work_table(plan.as_ref()) {
        return Some(Arc::clone(plan));
    }
    let children = plan.children();
    let mut found: Option<Arc<dyn ExecutionPlan>> = None;
    for child in &children {
        if !contains_work_table(child.as_ref()) {
            if found.is_some() {
                // Two non-worktable children at the same level: ambiguous.
                return None;
            }
            found = Some(Arc::clone(child));
        }
    }
    if let Some(edge) = found {
        return Some(edge);
    }
    // All children contain WorkTable. Descend into them and find inside.
    for child in &children {
        if let Some(edge) = find_unique_non_worktable_subtree(child) {
            return Some(edge);
        }
    }
    None
}

/// Compute the transitive closure of a binary edge relation in pure Rust.
///
/// Schema convention:
/// - `seed_batches`: two Binary columns. Column 0 is the "carry" (subject),
///   column 1 is the "step" (the current end value, joined against the edge
///   relation's first column).
/// - `edge_batches`: two Binary columns. Column 0 is the "from" side
///   (matched against the step), column 1 is the "to" side (becomes the
///   new step).
fn compute_closure(
    seed_batches: &[RecordBatch],
    edge_batches: &[RecordBatch],
    output_schema: &Arc<datafusion::arrow::datatypes::Schema>,
) -> Result<Vec<RecordBatch>, DataFusionError> {
    // Build edge index: HashMap<from_bytes, Vec<to_bytes>>
    let mut edge_index: std::collections::HashMap<Vec<u8>, Vec<Vec<u8>>> =
        std::collections::HashMap::new();
    for batch in edge_batches {
        if batch.num_columns() < 2 {
            return Err(DataFusionError::Internal(
                "edge batch must have at least 2 columns".to_string(),
            ));
        }
        let from = batch
            .column(0)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or_else(|| {
                DataFusionError::Internal("edge column 0 must be Binary".to_string())
            })?;
        let to = batch
            .column(1)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or_else(|| {
                DataFusionError::Internal("edge column 1 must be Binary".to_string())
            })?;
        for i in 0..batch.num_rows() {
            if from.is_null(i) || to.is_null(i) {
                continue;
            }
            edge_index
                .entry(from.value(i).to_vec())
                .or_default()
                .push(to.value(i).to_vec());
        }
    }

    // Seed -> seen and delta. Pairs are (carry, step).
    let mut seen: std::collections::HashSet<(Vec<u8>, Vec<u8>)> =
        std::collections::HashSet::new();
    let mut delta: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
    for batch in seed_batches {
        if batch.num_columns() < 2 {
            return Err(DataFusionError::Internal(
                "seed batch must have at least 2 columns".to_string(),
            ));
        }
        let carry = batch
            .column(0)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or_else(|| {
                DataFusionError::Internal("seed column 0 must be Binary".to_string())
            })?;
        let step = batch
            .column(1)
            .as_any()
            .downcast_ref::<BinaryArray>()
            .ok_or_else(|| {
                DataFusionError::Internal("seed column 1 must be Binary".to_string())
            })?;
        for i in 0..batch.num_rows() {
            if carry.is_null(i) || step.is_null(i) {
                continue;
            }
            let pair = (carry.value(i).to_vec(), step.value(i).to_vec());
            if seen.insert(pair.clone()) {
                delta.push(pair);
            }
        }
    }

    // Iterate to fixpoint.
    while !delta.is_empty() {
        let mut new_delta: Vec<(Vec<u8>, Vec<u8>)> = Vec::new();
        for (c, s) in &delta {
            if let Some(nexts) = edge_index.get(s) {
                for next in nexts {
                    let pair = (c.clone(), next.clone());
                    if seen.insert(pair.clone()) {
                        new_delta.push(pair);
                    }
                }
            }
        }
        delta = new_delta;
    }

    // Build a single output RecordBatch.
    let mut col0 = BinaryBuilder::with_capacity(seen.len(), seen.len() * 8);
    let mut col1 = BinaryBuilder::with_capacity(seen.len(), seen.len() * 8);
    for (c, s) in seen {
        col0.append_value(&c);
        col1.append_value(&s);
    }
    let col0: ArrayRef = Arc::new(col0.finish());
    let col1: ArrayRef = Arc::new(col1.finish());
    let batch = RecordBatch::try_new(Arc::clone(output_schema), vec![col0, col1])
        .map_err(DataFusionError::from)?;
    Ok(vec![batch])
}

struct QuerySolutionStreamWrapper {
    runtime: Arc<Runtime>,
    stream: SendableRecordBatchStream,
    variables: Arc<[Variable]>,
    dataset: Arc<DatasetView<'static>>,
    buffer: IntoIter<Result<QuerySolution, QueryEvaluationError>>,
}

impl Iterator for QuerySolutionStreamWrapper {
    type Item = Result<QuerySolution, QueryEvaluationError>;

    fn next(&mut self) -> Option<Result<QuerySolution, QueryEvaluationError>> {
        loop {
            if let Some(r) = self.buffer.next() {
                return Some(r);
            }
            let mut buffer = Vec::new();
            match self.runtime.block_on(self.stream.next())? {
                Ok(batch) => {
                    let mut results = (0..batch.num_rows())
                        .map(|_| vec![None; batch.num_columns()])
                        .collect::<Vec<_>>();
                    for (i, column) in batch.columns().iter().enumerate() {
                        if column.as_any().is::<NullArray>() {
                            continue;
                        }
                        let Some(array) = column.as_any().downcast_ref::<BinaryArray>() else {
                            buffer.push(Err(QueryEvaluationError::Unexpected(
                                format!("Column {} is not a binary column", self.variables[i])
                                    .into(),
                            )));
                            continue;
                        };
                        for (j, row) in array.iter().enumerate() {
                            if let Some(value) = row {
                                let term = match decode_term(value)
                                    .and_then(|t| self.dataset.decode_term(&t))
                                {
                                    Ok(term) => term,
                                    Err(e) => {
                                        buffer.push(Err(QueryEvaluationError::Unexpected(
                                            Box::new(e),
                                        )));
                                        continue;
                                    }
                                };
                                results[j][i] = Some(term);
                            }
                        }
                    }
                    buffer.extend(
                        results
                            .into_iter()
                            .map(|r| Ok(QuerySolution::from((Arc::clone(&self.variables), r)))),
                    )
                }
                Err(e) => {
                    buffer.push(Err(map_df_error(e)));
                }
            }
            self.buffer = buffer.into_iter();
        }
    }
}

struct QueryTripleStreamWrapper {
    runtime: Arc<Runtime>,
    stream: SendableRecordBatchStream,
    dataset: Arc<DatasetView<'static>>,
    buffer: IntoIter<Result<Triple, QueryEvaluationError>>,
}

impl Iterator for QueryTripleStreamWrapper {
    type Item = Result<Triple, QueryEvaluationError>;

    fn next(&mut self) -> Option<Result<Triple, QueryEvaluationError>> {
        loop {
            if let Some(r) = self.buffer.next() {
                return Some(r);
            }
            let mut buffer = Vec::new();
            match self.runtime.block_on(self.stream.next())? {
                Ok(batch) => {
                    let mut results = (0..batch.num_rows())
                        .map(|_| [const { None }; 3])
                        .collect::<Vec<_>>();
                    for (i, column) in batch.columns().iter().enumerate() {
                        if column.as_any().is::<NullArray>() {
                            continue;
                        }
                        let Some(array) = column.as_any().downcast_ref::<BinaryArray>() else {
                            buffer.push(Err(QueryEvaluationError::Unexpected(
                                format!("Column {i} is not a binary column").into(),
                            )));
                            continue;
                        };
                        for (j, row) in array.iter().enumerate() {
                            if let Some(value) = row {
                                let term = match decode_term(value)
                                    .and_then(|t| self.dataset.decode_term(&t))
                                {
                                    Ok(term) => term,
                                    Err(e) => {
                                        buffer.push(Err(QueryEvaluationError::Unexpected(
                                            Box::new(e),
                                        )));
                                        continue;
                                    }
                                };
                                results[j][i] = Some(term);
                            }
                        }
                    }
                    buffer.extend(results.into_iter().filter_map(|[s, p, o]| {
                        Some(Ok(Triple {
                            subject: match s? {
                                Term::NamedNode(s) => s.into(),
                                Term::BlankNode(s) => s.into(),
                                Term::Literal(_) => return None,
                                #[cfg(feature = "rdf-12")]
                                Term::Triple(_) => return None,
                            },
                            predicate: match p? {
                                Term::NamedNode(p) => p,
                                Term::BlankNode(_) | Term::Literal(_) => return None,
                                #[cfg(feature = "rdf-12")]
                                Term::Triple(_) => return None,
                            },
                            object: o?,
                        }))
                    }))
                }
                Err(e) => {
                    buffer.push(Err(map_df_error(e)));
                }
            }
            self.buffer = buffer.into_iter();
        }
    }
}

fn map_df_error(e: DataFusionError) -> QueryEvaluationError {
    QueryEvaluationError::Unexpected(Box::new(e))
}

struct OxigraphQueryableDataset {
    quads_table_plan: LogicalPlanBuilder,
    term_encoder: OxigraphTermEncoder,
}

impl OxigraphQueryableDataset {
    fn new(dataset: Arc<DatasetView<'static>>) -> Self {
        Self {
            quads_table_plan: LogicalPlanBuilder::scan(
                "quads",
                Arc::new(DefaultTableSource::new(Arc::new(QuadTableProvider::new(
                    Arc::clone(&dataset),
                )))),
                None,
            )
            .unwrap(),
            term_encoder: OxigraphTermEncoder { dataset },
        }
    }
}

impl QueryableDatasetAccess for OxigraphQueryableDataset {
    fn quads_table_plan(&mut self) -> Result<LogicalPlanBuilder> {
        Ok(self.quads_table_plan.clone())
    }

    fn expression_term_encoder(&mut self) -> impl ExpressionTermEncoder {
        self.term_encoder.clone()
    }

    fn internalize_term(&mut self, term: Term) -> Result<impl Literal> {
        Ok(encode_term(
            &self.term_encoder.dataset.internalize_term(term)?,
        ))
    }
}

#[derive(Clone)]
struct OxigraphTermEncoder {
    dataset: Arc<DatasetView<'static>>,
}

impl ExpressionTermEncoder for OxigraphTermEncoder {
    fn internal_type(&self) -> &DataType {
        &DataType::Binary
    }

    fn internalize_expression_term(&self, term: ExpressionTerm) -> Result<ScalarValue> {
        Ok(ScalarValue::Binary(Some(encode_term(
            &self.dataset.internalize_expression_term(term)?,
        ))))
    }

    fn internalize_expression_terms(
        &self,
        terms: impl Iterator<Item = Option<ExpressionTerm>>,
    ) -> Result<ArrayRef> {
        let mut output =
            BinaryBuilder::with_capacity(terms.size_hint().0, terms.size_hint().0 * 17);
        let mut buffer = Vec::with_capacity(17);
        for term in terms {
            if let Some(term) = term {
                buffer.clear();
                write_term(
                    &mut buffer,
                    &self.dataset.internalize_expression_term(term)?,
                );
                output.append_value(&buffer);
            } else {
                output.append_null();
            }
        }
        Ok(Arc::new(output.finish()))
    }

    fn externalize_expression_term(&self, term: ScalarValue) -> Result<Option<ExpressionTerm>> {
        let term = match term {
            ScalarValue::Binary(t) | ScalarValue::BinaryView(t) => t,
            ScalarValue::Null => None,
            _ => return internal_err!("Unexpected term encoding in expression: {term:?}"),
        };
        Ok(term
            .map(|t| self.dataset.externalize_expression_term(decode_term(&t)?))
            .transpose()?)
    }

    fn externalize_expression_terms(
        &self,
        terms: ArrayRef,
    ) -> Result<impl IntoIterator<Item = Result<Option<ExpressionTerm>>>> {
        Ok((0..terms.len()).map(move |i| {
            if terms.is_null(i) {
                return Ok(None);
            }
            Ok(Some(self.dataset.externalize_expression_term(
                decode_term(downcast_value!(terms, BinaryArray).value(i))?,
            )?))
        }))
    }
}
