use crate::sparql::Variable;
use crate::sparql::datafusion::plan_builder::PlanBuilder;
use crate::sparql::dataset::DatasetView;
use crate::storage::binary_encoder::decode_term;
use crate::storage::numeric_encoder::Decoder;
use crate::store::StorageError;
use datafusion::arrow::array::AsArray;
use datafusion::arrow::datatypes::GenericBinaryType;
use datafusion::common::ScalarValue;
use datafusion::error::{DataFusionError, Result};
use datafusion::execution::context::SessionConfig;
use datafusion::execution::runtime_env::RuntimeEnvBuilder;
use datafusion::execution::{
    SendableRecordBatchStream, SessionState, SessionStateBuilder, TaskContext,
};
use datafusion::logical_expr::Expr;
use datafusion::physical_plan::display::DisplayableExecutionPlan;
use datafusion::physical_plan::execute_stream;
use futures::{Stream, StreamExt, stream};
use sparesults::QuerySolution;
use spargebra::Query;
use spargebra::algebra::GraphPattern;
use std::pin::Pin;
use std::sync::Arc;

mod functions;
mod plan_builder;
mod table;

const NULL: Expr = Expr::Literal(ScalarValue::Null, None);

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
                                    let Some(column) =
                                        column.as_bytes_opt::<GenericBinaryType<i32>>()
                                    else {
                                        continue;
                                    };
                                    for (j, row) in column.iter().enumerate() {
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

impl From<StorageError> for DataFusionError {
    fn from(e: StorageError) -> Self {
        DataFusionError::External(Box::new(e))
    }
}
