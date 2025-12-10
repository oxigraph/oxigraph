use crate::sparql::Variable;
use crate::sparql::datafusion::table::QuadTableProvider;
use crate::sparql::dataset::DatasetView;
use crate::storage::binary_encoder::{decode_term, encode_term, write_term};
use crate::storage::numeric_encoder::Decoder;
use crate::store::StorageError;
use datafusion::arrow::array::{Array, ArrayRef, BinaryArray, BinaryBuilder, NullArray};
use datafusion::arrow::datatypes::DataType;
use datafusion::common::{Result, ScalarValue, downcast_value, internal_err};
use datafusion::datasource::DefaultTableSource;
use datafusion::error::DataFusionError;
use datafusion::execution::context::SessionConfig;
use datafusion::execution::runtime_env::RuntimeEnvBuilder;
use datafusion::execution::{
    SendableRecordBatchStream, SessionState, SessionStateBuilder, TaskContext,
};
use datafusion::logical_expr::{Literal, LogicalPlan, LogicalPlanBuilder};
use datafusion::physical_plan::display::DisplayableExecutionPlan;
use datafusion::physical_plan::execute_stream;
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
        let state = SessionStateBuilder::new()
            .with_config(SessionConfig::new())
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
        let physical_plan = self
            .state
            .create_physical_plan(&logical_plan)
            .await
            .map_err(map_df_error)?;
        let variables = physical_plan
            .schema()
            .fields()
            .iter()
            .map(|f| Variable::new_unchecked(f.name()))
            .collect();
        Ok((
            variables,
            execute_stream(physical_plan, Arc::new(TaskContext::from(&self.state)))
                .map_err(map_df_error)?,
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
