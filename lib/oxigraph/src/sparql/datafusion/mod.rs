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
use datafusion::logical_expr::{LogicalPlan, LogicalPlanBuilder};
use datafusion::physical_plan::display::DisplayableExecutionPlan;
use datafusion::physical_plan::execute_stream;
use futures::StreamExt;
use oxrdf::{BlankNode, NamedOrBlankNode, Term, Triple};
use rustc_hash::{FxHashMap, FxHashSet};
use sparesults::QuerySolution;
use spareval::{QueryEvaluationError, QueryResults, QuerySolutionIter, QueryTripleIter};
use spargebra::Query;
use spargebra::algebra::{GraphPattern, QueryDataset};
use spargebra::term::{NamedNodePattern, TermPattern, TriplePattern};
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
    ) -> Result<QueryResults<'static>, QueryEvaluationError> {
        // TODO: implement as much as possible in DataFusion
        self.runtime.block_on(async {
            let dataset = Arc::new(dataset);
            match query {
                Query::Select {
                    pattern,
                    dataset: dataset_spec,
                    ..
                } => {
                    let plan = Self::select_graph_pattern_plan(
                        pattern,
                        Arc::clone(&dataset),
                        dataset_spec,
                    )?;
                    let (variables, stream) = self.execute_plan(plan).await?;
                    Ok(QueryResults::Solutions(QuerySolutionIter::new(
                        Arc::clone(&variables),
                        QuerySolutionStreamWrapper {
                            runtime: Arc::clone(&self.runtime),
                            stream,
                            variables,
                            dataset,
                            buffer: Vec::new().into_iter(),
                        },
                    )))
                }
                Query::Construct {
                    template,
                    dataset: dataset_spec,
                    pattern,
                    ..
                } => {
                    let plan = Self::select_graph_pattern_plan(
                        pattern,
                        Arc::clone(&dataset),
                        dataset_spec,
                    )?;
                    let (variables, stream) = self.execute_plan(plan).await?;
                    Ok(QueryResults::Graph(QueryTripleIter::new(
                        ConstructIterator {
                            solutions: QuerySolutionStreamWrapper {
                                runtime: Arc::clone(&self.runtime),
                                stream,
                                variables,
                                dataset,
                                buffer: Vec::new().into_iter(),
                            },
                            template: template.clone(),
                            buffered_results: Vec::new(),
                            already_emitted_results: FxHashSet::default(),
                            bnodes: FxHashMap::default(),
                        },
                    )))
                }
                Query::Describe {
                    pattern,
                    dataset: dataset_spec,
                    ..
                } => {
                    let plan = Self::describe_graph_pattern_plan(
                        pattern,
                        Arc::clone(&dataset),
                        dataset_spec,
                    )?;
                    let (variables, stream) = self.execute_plan(plan).await?;
                    Ok(QueryResults::Graph(QueryTripleIter::new(
                        ConstructIterator {
                            solutions: QuerySolutionStreamWrapper {
                                runtime: Arc::clone(&self.runtime),
                                stream,
                                variables,
                                dataset,
                                buffer: Vec::new().into_iter(),
                            },
                            template: vec![TriplePattern {
                                subject: TermPattern::Variable(Variable::new_unchecked("subject")),
                                predicate: NamedNodePattern::Variable(Variable::new_unchecked(
                                    "predicate",
                                )),
                                object: TermPattern::Variable(Variable::new_unchecked("object")),
                            }],
                            buffered_results: Vec::new(),
                            already_emitted_results: FxHashSet::default(),
                            bnodes: FxHashMap::default(),
                        },
                    )))
                }
                Query::Ask {
                    pattern,
                    dataset: dataset_spec,
                    ..
                } => {
                    let plan = Self::select_graph_pattern_plan(
                        pattern,
                        Arc::clone(&dataset),
                        dataset_spec,
                    )?;
                    // No need to load more than a row
                    let plan = LogicalPlanBuilder::new(plan)
                        .limit(0, Some(1))
                        .map_err(map_df_error)?
                        .build()
                        .map_err(map_df_error)?;
                    let (_, mut stream) = self.execute_plan(plan).await?;
                    while let Some(batch) = stream.next().await {
                        if batch.map_err(map_df_error)?.num_rows() > 0 {
                            return Ok(QueryResults::Boolean(true));
                        }
                    }
                    Ok(QueryResults::Boolean(false))
                }
            }
        })
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

    fn select_graph_pattern_plan(
        pattern: &GraphPattern,
        dataset: Arc<DatasetView<'static>>,
        dataset_spec: &Option<QueryDataset>,
    ) -> Result<LogicalPlan, QueryEvaluationError> {
        SparqlPlanBuilder::new(dataset, dataset_spec.as_ref())
            .select_plan(pattern)
            .map_err(map_df_error)?
            .build()
            .map_err(map_df_error)
    }

    fn describe_graph_pattern_plan(
        pattern: &GraphPattern,
        dataset: Arc<DatasetView<'static>>,
        dataset_spec: &Option<QueryDataset>,
    ) -> Result<LogicalPlan, QueryEvaluationError> {
        SparqlPlanBuilder::new(dataset, dataset_spec.as_ref())
            .describe_plan(pattern)
            .map_err(map_df_error)?
            .build()
            .map_err(map_df_error)
    }

    pub fn explain(
        self,
        dataset: DatasetView<'static>,
        query: &Query,
    ) -> Result<String, QueryEvaluationError> {
        let dataset = Arc::new(dataset);
        let logical_plan = match query {
            Query::Select {
                pattern,
                dataset: dataset_spec,
                ..
            }
            | Query::Ask {
                pattern,
                dataset: dataset_spec,
                ..
            }
            | Query::Construct {
                pattern,
                dataset: dataset_spec,
                ..
            } => Self::select_graph_pattern_plan(pattern, dataset, dataset_spec),
            Query::Describe {
                pattern,
                dataset: dataset_spec,
                ..
            } => Self::describe_graph_pattern_plan(pattern, dataset, dataset_spec),
        }?;
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

struct ConstructIterator {
    solutions: QuerySolutionStreamWrapper,
    template: Vec<TriplePattern>,
    buffered_results: Vec<Triple>,
    already_emitted_results: FxHashSet<Triple>,
    bnodes: FxHashMap<BlankNode, BlankNode>,
}

impl Iterator for ConstructIterator {
    type Item = Result<Triple, QueryEvaluationError>;

    fn next(&mut self) -> Option<Result<Triple, QueryEvaluationError>> {
        loop {
            if let Some(r) = self.buffered_results.pop() {
                return Some(Ok(r));
            }
            let solution = match self.solutions.next()? {
                Ok(solution) => solution,
                Err(e) => return Some(Err(e)),
            };
            for template in &self.template {
                let Some(triple) = substitute_triple_pattern(template, &solution, &mut self.bnodes)
                else {
                    continue;
                };
                // We allocate new blank nodes for each solution,
                // triples with blank nodes are likely to be new.
                #[cfg(feature = "rdf-12")]
                let new_triple = triple.subject.is_blank_node()
                    || triple.object.is_blank_node()
                    || triple.object.is_triple()
                    || self.already_emitted_results.insert(triple.clone());
                #[cfg(not(feature = "rdf-12"))]
                let new_triple = triple.subject.is_blank_node()
                    || triple.object.is_blank_node()
                    || self.already_emitted_results.insert(triple.clone());
                if new_triple {
                    self.buffered_results.push(triple);
                    if self.already_emitted_results.len() > 1024 * 1024 {
                        // We don't want to have a too big memory impact
                        self.already_emitted_results.clear();
                    }
                }
            }
            self.bnodes.clear();
        }
    }
}

fn substitute_triple_pattern(
    pattern: &TriplePattern,
    solution: &QuerySolution,
    bnodes: &mut FxHashMap<BlankNode, BlankNode>,
) -> Option<Triple> {
    Some(Triple::new(
        match substitute_term_pattern(&pattern.subject, solution, bnodes)? {
            Term::NamedNode(node) => NamedOrBlankNode::from(node),
            Term::BlankNode(node) => node.into(),
            Term::Literal(_) => return None,
            #[cfg(feature = "rdf-12")]
            Term::Triple(_) => return None,
        },
        match &pattern.predicate {
            NamedNodePattern::NamedNode(node) => node.clone(),
            NamedNodePattern::Variable(v) => {
                if let Term::NamedNode(node) = solution.get(v)? {
                    node.clone()
                } else {
                    return None;
                }
            }
        },
        substitute_term_pattern(&pattern.object, solution, bnodes)?,
    ))
}

fn substitute_term_pattern(
    pattern: &TermPattern,
    solution: &QuerySolution,
    bnodes: &mut FxHashMap<BlankNode, BlankNode>,
) -> Option<Term> {
    Some(match pattern {
        TermPattern::NamedNode(node) => node.clone().into(),
        TermPattern::BlankNode(node) => bnodes.entry(node.clone()).or_default().clone().into(),
        TermPattern::Literal(node) => node.clone().into(),
        #[cfg(feature = "rdf-12")]
        TermPattern::Triple(triple) => substitute_triple_pattern(triple, solution, bnodes)?.into(),
        TermPattern::Variable(v) => solution.get(v)?.clone(),
    })
}

fn map_df_error(e: DataFusionError) -> QueryEvaluationError {
    QueryEvaluationError::Unexpected(Box::new(e))
}
