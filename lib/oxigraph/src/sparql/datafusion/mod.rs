use crate::sparql::Variable;
use crate::sparql::datafusion::plan_builder::SparqlPlanBuilder;
use crate::sparql::dataset::DatasetView;
use crate::storage::binary_encoder::decode_term;
use crate::storage::numeric_encoder::Decoder;
use crate::store::StorageError;
use datafusion::arrow::array::{Array, BinaryArray, NullArray};
use datafusion::error::DataFusionError;
use datafusion::execution::context::SessionConfig;
use datafusion::execution::runtime_env::RuntimeEnvBuilder;
use datafusion::execution::{
    SendableRecordBatchStream, SessionState, SessionStateBuilder, TaskContext,
};
use datafusion::physical_plan::display::DisplayableExecutionPlan;
use datafusion::physical_plan::execute_stream;
use futures::StreamExt;
use sparesults::QuerySolution;
use spareval::{QueryEvaluationError, QueryResults, QuerySolutionIter};
use spargebra::Query;
use spargebra::algebra::{GraphPattern, QueryDataset};
use std::sync::Arc;
use std::vec::IntoIter;
use tokio::runtime::{Builder, Runtime};

mod function;
mod plan_builder;
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
    ) -> Result<Option<QueryResults<'static>>, QueryEvaluationError> {
        self.runtime.block_on(async {
            let dataset = Arc::new(dataset);
            match query {
                Query::Select {
                    pattern,
                    dataset: dataset_spec,
                    ..
                } => {
                    let Some((variables, stream)) = self
                        .execute_graph_pattern(pattern, None, Arc::clone(&dataset), dataset_spec)
                        .await?
                    else {
                        return Ok(None);
                    };
                    Ok(Some(QueryResults::Solutions(QuerySolutionIter::new(
                        Arc::clone(&variables),
                        QuerySolutionStreamWrapper {
                            runtime: Arc::clone(&self.runtime),
                            stream,
                            variables,
                            dataset,
                            buffer: Vec::new().into_iter(),
                        },
                    ))))
                }
                Query::Construct { .. } | Query::Describe { .. } => Ok(None),
                Query::Ask {
                    pattern,
                    dataset: dataset_spec,
                    ..
                } => {
                    let Some((_, mut stream)) = self
                        .execute_graph_pattern(pattern, Some(1), Arc::clone(&dataset), dataset_spec)
                        .await?
                    else {
                        return Ok(None);
                    };
                    while let Some(batch) = stream.next().await {
                        if batch.map_err(map_df_error)?.num_rows() > 0 {
                            return Ok(Some(QueryResults::Boolean(true)));
                        }
                    }
                    Ok(Some(QueryResults::Boolean(false)))
                }
            }
        })
    }

    async fn execute_graph_pattern(
        &self,
        pattern: &GraphPattern,
        limit: Option<usize>,
        dataset: Arc<DatasetView<'static>>,
        dataset_spec: &Option<QueryDataset>,
    ) -> Result<Option<(Arc<[Variable]>, SendableRecordBatchStream)>, QueryEvaluationError> {
        let plan = match SparqlPlanBuilder::new(dataset, dataset_spec.as_ref())
            .plan(pattern)
            .and_then(|mut plan| {
                if let Some(limit) = limit {
                    plan = plan.limit(0, Some(limit))?;
                };
                plan.build()
            }) {
            Ok(r) => r,
            Err(DataFusionError::NotImplemented(_)) => return Ok(None),
            Err(e) => return Err(map_df_error(e)),
        };
        let plan = self
            .state
            .create_physical_plan(&plan)
            .await
            .map_err(map_df_error)?;
        let variables = plan
            .schema()
            .fields()
            .iter()
            .map(|f| Variable::new_unchecked(f.name()))
            .collect();
        Ok(Some((
            variables,
            execute_stream(plan, Arc::new(TaskContext::from(&self.state))).map_err(map_df_error)?,
        )))
    }

    pub fn explain(
        self,
        dataset: DatasetView<'static>,
        query: &Query,
    ) -> Result<Option<String>, QueryEvaluationError> {
        let dataset = Arc::new(dataset);
        let (pattern, dataset_spec, limit) = match query {
            Query::Select {
                pattern, dataset, ..
            }
            | Query::Describe {
                pattern, dataset, ..
            }
            | Query::Ask {
                pattern, dataset, ..
            } => (pattern, dataset, None),
            Query::Construct {
                pattern, dataset, ..
            } => (pattern, dataset, Some(1)),
        };
        let logical_plan = match SparqlPlanBuilder::new(dataset, dataset_spec.as_ref())
            .plan(pattern)
            .and_then(|mut plan| {
                if let Some(limit) = limit {
                    plan = plan.limit(0, Some(limit))?;
                };
                plan.build()
            }) {
            Ok(plan) => plan,
            Err(DataFusionError::NotImplemented(_)) => {
                return Ok(None);
            }
            Err(e) => return Err(map_df_error(e)),
        };
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
        Ok(Some(format!(
            "{logical_plan}\n\n{displayable_execution_plan}"
        )))
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

fn map_df_error(e: DataFusionError) -> QueryEvaluationError {
    QueryEvaluationError::Unexpected(Box::new(e))
}
