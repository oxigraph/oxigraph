use crate::sparql::datafusion::function::{
    ComparisonOperator, compare_terms, effective_boolean_value, order_by_collation, term_equals,
    to_rdf_literal,
};
use crate::sparql::datafusion::table::QuadTableProvider;
use crate::sparql::dataset::DatasetView;
use crate::storage::binary_encoder::encode_term;
use datafusion::arrow::datatypes::{DataType, Field};
use datafusion::common::alias::AliasGenerator;
use datafusion::common::{
    Column, DFSchema, DFSchemaRef, DataFusionError, JoinType, Result, TableReference,
};
use datafusion::datasource::{DefaultTableSource, TableProvider};
use datafusion::functions::expr_fn::coalesce;
use datafusion::functions_aggregate::count::{count, count_all, count_distinct, count_udaf};
use datafusion::logical_expr::expr::{AggregateFunction as DFAggregateFunction, Sort};
use datafusion::logical_expr::{
    Case, Expr, LogicalPlanBuilder, TableSource, and, exists, lit, not, or,
};
use oxrdf::{BlankNode, Term, Variable};
use spareval::QueryableDataset;
use spargebra::algebra::{
    AggregateExpression, AggregateFunction, Expression, GraphPattern, OrderExpression, QueryDataset,
};
use spargebra::term::{NamedNodePattern, TermPattern, TriplePattern};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::iter::once;
use std::sync::Arc;

const GRAPH_MAGIC_COLUMN: &str = "#graph#";

/// Builds a DataFusion `LogicalPlan` from a SPARQL `GraphPattern`
pub struct SparqlPlanBuilder<'a> {
    dataset: Arc<DatasetView<'static>>,
    quad_table_source: Arc<dyn TableSource>,
    blank_node_to_variable: HashMap<BlankNode, Variable>,
    table_name: AliasGenerator,
    dataset_spec: Option<&'a QueryDataset>,
}

impl<'a> SparqlPlanBuilder<'a> {
    pub fn new(dataset: Arc<DatasetView<'static>>, dataset_spec: Option<&'a QueryDataset>) -> Self {
        // TODO: add a graph name table
        Self {
            dataset: Arc::clone(&dataset),
            quad_table_source: table_source(QuadTableProvider::new(dataset)),
            blank_node_to_variable: HashMap::new(),
            table_name: AliasGenerator::new(),
            dataset_spec,
        }
    }

    pub fn plan(&mut self, pattern: &GraphPattern) -> Result<LogicalPlanBuilder> {
        self.plan_for_graph_pattern(pattern, true, &HashMap::new())
    }

    /// Creates a `LogicalPlan` from a `GraphPattern`
    ///
    /// Most operator conversions are fairly straightforward. Some notes:
    /// - the output plan columns correspond to the SPARQL query variable plus a magic #graph# column that contains the output row graph name (it is used for the GRAPH operator).
    /// - in_default_graph sets if we query the default graph or a named graph, in the later case the #graph# column is set to the named graph name.
    /// - external_schema allows injecting bindings deeply in the plan. It is used for EXISTS and LATERAL.
    ///   BGPs and property paths join the bindings they generate with it.
    fn plan_for_graph_pattern(
        &mut self,
        pattern: &GraphPattern,
        in_default_graph: bool,
        external_schema: &HashMap<String, Column>,
    ) -> Result<LogicalPlanBuilder> {
        match pattern {
            GraphPattern::Values {
                variables,
                bindings,
            } => {
                if variables.is_empty() || bindings.is_empty() {
                    // Workaround empty values not allowed
                    Ok((0..bindings.len())
                        .map(|_| Ok(LogicalPlanBuilder::empty(true)))
                        .reduce(|l, r| l?.union(r?.build()?))
                        .transpose()?
                        .unwrap_or_else(|| LogicalPlanBuilder::empty(false)))
                } else {
                    LogicalPlanBuilder::values(
                        bindings
                            .iter()
                            .map(|vs| {
                                vs.iter()
                                    .map(|value| {
                                        value.as_ref().map_or_else(
                                            || Ok(Expr::default()),
                                            |v| self.term_to_expr(v.clone()),
                                        )
                                    })
                                    .collect::<Result<Vec<_>>>()
                            })
                            .collect::<Result<Vec<_>>>()?,
                    )?
                    .project(variables.iter().enumerate().map(
                        |(i, variable)| {
                            Expr::from(Column::from_name(format!("column{}", i + 1)))
                                .alias(variable.as_str())
                        },
                    ))
                }
            }
            GraphPattern::Bgp { patterns } => {
                let plans = patterns
                    .iter()
                    .map(|p| self.plan_for_triple_pattern(p, in_default_graph, external_schema))
                    .collect::<Vec<_>>();
                plans
                    .into_iter()
                    .reduce(|left, right| {
                        self.join(
                            left?,
                            JoinType::Inner,
                            right?,
                            None,
                            in_default_graph,
                            external_schema,
                        )
                    })
                    .unwrap_or_else(|| Ok(LogicalPlanBuilder::empty(true)))
            }
            GraphPattern::Path { .. } => Err(DataFusionError::NotImplemented(
                "Path patterns are not implemented yet".into(),
            )),
            GraphPattern::Graph { inner, name } => {
                let mut plan = self.plan_for_graph_pattern(inner, false, external_schema)?;
                // We join with the existing value for the GRAPH variable/constant
                let input_value = match name {
                    NamedNodePattern::NamedNode(name) => Some(self.term_to_expr(name.clone())?),
                    NamedNodePattern::Variable(variable) => {
                        schema_column(plan.schema(), variable.as_str()).map(Into::into)
                    }
                };
                if self.dataset_spec.is_some_and(|s| s.named.is_some())
                    || plan
                        .schema()
                        .fields()
                        .iter()
                        .find(|field| field.name() == GRAPH_MAGIC_COLUMN)
                        .is_none_or(|field| field.is_nullable())
                {
                    // We need to join with the list of possible graph names
                    plan = self.ensure_qualified_names(plan)?;
                    let input_graph_column = schema_column(plan.schema(), GRAPH_MAGIC_COLUMN);
                    let graph_names = self.plan_for_graph_names()?.build()?;
                    let graph_name_var =
                        schema_column(graph_names.schema(), GRAPH_MAGIC_COLUMN).unwrap();
                    let projection = plan
                        .schema()
                        .iter()
                        .map(Column::from)
                        .chain(once(graph_name_var.clone()))
                        .collect::<Vec<_>>();
                    plan = plan
                        .join_on(
                            graph_names,
                            JoinType::Inner,
                            input_graph_column.map(|c| {
                                Expr::from(c.clone())
                                    .eq(graph_name_var.into())
                                    .or(Expr::from(c).is_null())
                            }),
                        )?
                        .project(projection)?;
                    plan = self.ensure_qualified_names(plan)?;
                }
                let input_graph_column = schema_column(plan.schema(), GRAPH_MAGIC_COLUMN); // We refresh the column in case the join added it
                let output_column_name = if let NamedNodePattern::Variable(v) = name {
                    Some(v.as_str())
                } else {
                    None
                };
                if let Some(graph_column) = &input_graph_column {
                    if let Some(external_column) = output_column_name
                        .as_ref()
                        .and_then(|c| external_schema.get(*c))
                    {
                        // We apply external constraint
                        plan = plan.filter(eq_with_null_match_anything(
                            graph_column.clone(),
                            Expr::OuterReferenceColumn(DataType::Binary, external_column.clone()),
                        ))?;
                    }
                    if let Some(input_value) = &input_value {
                        // We make sure it's equal to the already filled variable
                        plan = plan.filter(eq_with_null_match_anything(
                            graph_column.clone(),
                            input_value.clone(),
                        ))?;
                    }
                }
                let projection = plan
                    .schema()
                    .iter()
                    .filter(|(_, field)| {
                        field.name() != GRAPH_MAGIC_COLUMN
                            && output_column_name
                                .as_ref()
                                .is_none_or(|v| field.name() != v)
                    })
                    .map(|e| Column::from(e).into())
                    .chain(output_column_name.map(|v| {
                        match (input_graph_column, input_value) {
                            (Some(l), Some(r)) => coalesce(vec![l.into(), r]),
                            (Some(l), None) => l.into(),
                            (None, Some(r)) => r,
                            (None, None) => Expr::default(),
                        }
                        .alias(v)
                    }))
                    .collect::<Vec<_>>();
                plan.project(projection)
            }
            GraphPattern::Join { left, right } => {
                let left = self.plan_for_graph_pattern(left, in_default_graph, external_schema)?;
                let right =
                    self.plan_for_graph_pattern(right, in_default_graph, external_schema)?;
                self.join(
                    left,
                    JoinType::Inner,
                    right,
                    None,
                    in_default_graph,
                    external_schema,
                )
            }
            GraphPattern::LeftJoin {
                left,
                right,
                expression,
            } => {
                let left = self.plan_for_graph_pattern(left, in_default_graph, external_schema)?;
                let right =
                    self.plan_for_graph_pattern(right, in_default_graph, external_schema)?;
                self.join(
                    left,
                    JoinType::Left,
                    right,
                    expression.as_ref(),
                    in_default_graph,
                    external_schema,
                )
            }
            GraphPattern::Minus { left, right } => {
                let left = self.plan_for_graph_pattern(left, in_default_graph, external_schema)?;
                let right =
                    self.plan_for_graph_pattern(right, in_default_graph, external_schema)?;
                self.join(
                    left,
                    JoinType::LeftAnti,
                    right,
                    None,
                    in_default_graph,
                    external_schema,
                )
            }
            GraphPattern::Lateral { left, right } => {
                let left = self.plan_for_graph_pattern(left, in_default_graph, external_schema)?;
                let left = self.ensure_qualified_names(left)?;
                let mut right_external_schema = external_schema.clone();
                for (table_ref, field) in left.schema().iter() {
                    right_external_schema
                        .insert(field.name().into(), Column::from((table_ref, field)));
                }
                let right =
                    self.plan_for_graph_pattern(right, in_default_graph, &right_external_schema)?;
                self.join(
                    left,
                    JoinType::Inner,
                    right,
                    None,
                    in_default_graph,
                    external_schema,
                )
            }
            GraphPattern::Union { left, right } => {
                let left = self.plan_for_graph_pattern(left, in_default_graph, external_schema)?;
                let right =
                    self.plan_for_graph_pattern(right, in_default_graph, external_schema)?;
                left.union_by_name(right.build()?)
            }
            GraphPattern::Filter { inner, expr } => {
                let plan = self.plan_for_graph_pattern(inner, in_default_graph, external_schema)?;
                let plan = self.ensure_qualified_names(plan)?;
                let filter = self.effective_boolean_value_expression(
                    expr,
                    plan.schema(),
                    in_default_graph,
                    external_schema,
                )?;
                plan.filter(filter)
            }
            GraphPattern::Extend {
                inner,
                variable,
                expression,
            } => {
                let plan = self.plan_for_graph_pattern(inner, in_default_graph, external_schema)?;
                let plan = self.ensure_qualified_names(plan)?;
                let projection = plan
                    .schema()
                    .iter()
                    .map(|field| Column::from(field).into())
                    .chain(once(
                        self.expression(
                            expression,
                            plan.schema(),
                            in_default_graph,
                            external_schema,
                        )?
                        .alias(variable.as_str()),
                    ))
                    .collect::<Vec<_>>();
                plan.project(projection)
            }
            GraphPattern::Group {
                inner,
                variables,
                aggregates,
            } => {
                let plan = self.plan_for_graph_pattern(inner, in_default_graph, external_schema)?;
                let mut projection = Vec::new();
                let group_expr = variables
                    .iter()
                    .enumerate()
                    .filter(|(i, v)| !variables[i + 1..].contains(v)) // We remove duplicates
                    .map(|(_, v)| {
                        let column = Self::variable_expression(v, plan.schema(), external_schema);
                        projection.push(column.clone());
                        column
                    })
                    .chain(schema_column(plan.schema(), GRAPH_MAGIC_COLUMN).map(Into::into))
                    .collect::<Vec<_>>();
                let aggr_expr = aggregates
                    .iter()
                    .map(|(target_var, expression)| {
                        let aggregate = match expression {
                            AggregateExpression::CountSolutions { distinct } => {
                                if *distinct {
                                    // We count the columns that are not in the group part
                                    Expr::AggregateFunction(DFAggregateFunction::new_udf(
                                        count_udaf(),
                                        plan.schema()
                                            .iter()
                                            .filter_map(|c| {
                                                let expr = Column::from(c).into();
                                                if group_expr.contains(&expr) {
                                                    None // Already in the group clause, no need to count again
                                                } else {
                                                    Some(expr)
                                                }
                                            })
                                            .collect(),
                                        true,
                                        None,
                                        Vec::new(),
                                        None,
                                    ))
                                } else {
                                    count_all()
                                }
                            }
                            AggregateExpression::FunctionCall {
                                name,
                                expr,
                                distinct,
                            } => {
                                let expression = self.expression(
                                    expr,
                                    plan.schema(),
                                    in_default_graph,
                                    external_schema,
                                )?;
                                match (name, distinct) {
                                    (AggregateFunction::Count, false) => count(expression),
                                    (AggregateFunction::Count, true) => count_distinct(expression),
                                    _ => {
                                        return Err(DataFusionError::NotImplemented(format!(
                                            "{name} is not implemented yet"
                                        )));
                                    }
                                }
                            }
                        };
                        projection.push(
                            to_rdf_literal(Column::from_name(aggregate.name_for_alias()?).into())
                                .alias(target_var.as_str()),
                        );
                        Ok(aggregate)
                    })
                    .collect::<Result<Vec<_>>>()?;
                plan.aggregate(group_expr, aggr_expr)?.project(projection)
            }
            GraphPattern::Project { inner, variables } => {
                // We only keep externals that are in the projection
                let external_schema = filter_external_schema(external_schema, variables);
                let plan =
                    self.plan_for_graph_pattern(inner, in_default_graph, &external_schema)?;
                let projection = variables
                    .iter()
                    .map(|v| Self::variable_expression(v, plan.schema(), &external_schema))
                    .chain(schema_column(plan.schema(), GRAPH_MAGIC_COLUMN).map(Into::into))
                    .collect::<Vec<_>>();
                plan.project(projection)
            }
            GraphPattern::OrderBy { inner, expression } => {
                let plan = self.plan_for_graph_pattern(inner, in_default_graph, external_schema)?;
                let sorts = expression
                    .iter()
                    .map(|e| {
                        self.order_expression(e, plan.schema(), in_default_graph, external_schema)
                    })
                    .collect::<Result<Vec<_>>>()?;
                plan.sort(sorts)
            }
            GraphPattern::Distinct { inner } => {
                if let GraphPattern::Project { inner, variables } = &**inner {
                    // We can use DISTINCT ON
                    let (inner, sort) =
                        if let GraphPattern::OrderBy { inner, expression } = &**inner {
                            (inner, Some(expression))
                        } else {
                            (inner, None)
                        };
                    // We only keep externals that are in the projection
                    let external_schema = filter_external_schema(external_schema, variables);
                    let plan =
                        self.plan_for_graph_pattern(inner, in_default_graph, &external_schema)?;
                    let projection = variables
                        .iter()
                        .map(|v| Self::variable_expression(v, plan.schema(), &external_schema))
                        .chain(schema_column(plan.schema(), GRAPH_MAGIC_COLUMN).map(Into::into))
                        .collect::<Vec<_>>();
                    let mut sort = sort
                        .map(|es| {
                            es.iter()
                                .map(|e| {
                                    self.order_expression(
                                        e,
                                        plan.schema(),
                                        in_default_graph,
                                        &external_schema,
                                    )
                                })
                                .collect::<Result<Vec<_>>>()
                        })
                        .transpose()?;
                    let on_expr = if let Some(sort) = &mut sort {
                        // We ensure that the sort expressions are a super-set of the ON expressions.
                        // For that we start the ON by the sort expressions, then add the missing variables
                        let mut on_expr = sort.iter().map(|e| e.expr.clone()).collect::<Vec<_>>();
                        for expr in &projection {
                            let expr = self.order_by_collation(expr.clone());
                            if !on_expr.contains(&expr) {
                                on_expr.push(expr.clone());
                                sort.push(expr.sort(true, true));
                            }
                        }
                        on_expr
                    } else {
                        projection.clone()
                    };
                    plan.distinct_on(on_expr, projection, sort)
                } else {
                    self.plan_for_graph_pattern(inner, in_default_graph, external_schema)?
                        .distinct()
                }
            }
            GraphPattern::Reduced { inner } => {
                self.plan_for_graph_pattern(inner, in_default_graph, external_schema)
            }
            GraphPattern::Slice {
                inner,
                start,
                length,
            } => self
                .plan_for_graph_pattern(inner, in_default_graph, external_schema)?
                .limit(*start, *length),
            GraphPattern::Service { .. } => Err(DataFusionError::NotImplemented(
                "SERVICE is not implemented yet".into(),
            )),
        }
    }

    fn join(
        &mut self,
        left_plan: LogicalPlanBuilder,
        join_type: JoinType,
        right_plan: LogicalPlanBuilder,
        filter: Option<&Expression>,
        in_default_graph: bool,
        external_schema: &HashMap<String, Column>,
    ) -> Result<LogicalPlanBuilder> {
        let left_plan = self.ensure_qualified_names(left_plan)?;
        let right_plan = self.ensure_qualified_names(right_plan)?;
        let mut on_exprs = Vec::new();
        if let Some(filter) = filter {
            let joint_schema = left_plan.schema().join(right_plan.schema())?;
            on_exprs.push(self.effective_boolean_value_expression(
                filter,
                &joint_schema,
                in_default_graph,
                external_schema,
            )?);
        }
        let mut shared_variables_for_minus = (join_type == JoinType::LeftAnti).then(Vec::new);
        let projection = left_and_right_fields_by_name(left_plan.schema(), right_plan.schema())
            .into_iter()
            .filter_map(
                |(column, (left_entry, right_entry))| match (left_entry, right_entry) {
                    (None, None) => None,
                    (Some(field), None) => Some(Column::from(field).into()),
                    (None, Some(field)) => {
                        if join_type == JoinType::LeftAnti {
                            None
                        } else {
                            Some(Column::from(field).into())
                        }
                    }
                    (Some((left_table_ref, left_field)), Some((right_table_ref, right_field))) => {
                        let left_expr = Expr::from(Column::from((left_table_ref, left_field)));
                        let right_expr = Expr::from(Column::from((right_table_ref, right_field)));
                        if left_field.is_nullable() || right_field.is_nullable() {
                            on_exprs.push(eq_with_null_match_anything(
                                left_expr.clone(),
                                right_expr.clone(),
                            ));
                            if join_type == JoinType::LeftAnti {
                                if let Some(shared_variables_for_minus) =
                                    &mut shared_variables_for_minus
                                {
                                    if left_field.name() != GRAPH_MAGIC_COLUMN {
                                        shared_variables_for_minus
                                            .push((left_expr.clone(), right_expr));
                                    }
                                }
                                Some(left_expr) // We don't output right variables
                            } else {
                                Some(coalesce(vec![left_expr, right_expr]).alias(column))
                            }
                        } else {
                            on_exprs.push(left_expr.clone().eq(right_expr));
                            if join_type == JoinType::LeftAnti {
                                shared_variables_for_minus = None; // We have a shared variable
                            }
                            Some(left_expr) // it must be equal to right
                        }
                    }
                },
            )
            .collect::<Vec<_>>();
        if let Some(shared_variables_for_minus) = shared_variables_for_minus {
            // SPARQL special case: if there are no shared variables, we don't match a.k.a. we match iff there are shared variables bound
            on_exprs.push(
                shared_variables_for_minus
                    .into_iter()
                    .map(|(l, r)| l.is_not_null().and(r.is_not_null()))
                    .reduce(Expr::or)
                    .unwrap_or_else(|| lit(false)),
            )
        }
        if on_exprs.is_empty() && join_type != JoinType::Inner {
            on_exprs.push(lit(true)); // DF requires a filter if join is not an inner join
        }
        left_plan
            .join_on(right_plan.build()?, join_type, on_exprs)?
            .project(projection)
    }

    fn plan_for_triple_pattern(
        &mut self,
        pattern: &TriplePattern,
        in_default_graph: bool,
        external_schema: &HashMap<String, Column>,
    ) -> Result<LogicalPlanBuilder> {
        let mut filters = Vec::new();
        let mut new_to_original_column = HashMap::new();
        let table_name = self.table_name.next("quads");
        self.term_pattern_to_filter_or_project(
            pattern.subject.clone(),
            Column::new(Some(table_name.clone()), "subject"),
            &mut filters,
            &mut new_to_original_column,
            external_schema,
        )?;
        self.term_pattern_to_filter_or_project(
            pattern.predicate.clone().into(),
            Column::new(Some(table_name.clone()), "predicate"),
            &mut filters,
            &mut new_to_original_column,
            external_schema,
        )?;
        self.term_pattern_to_filter_or_project(
            pattern.object.clone(),
            Column::new(Some(table_name.clone()), "object"),
            &mut filters,
            &mut new_to_original_column,
            external_schema,
        )?;
        self.current_graph_to_filter_or_project(
            in_default_graph,
            Column::new(Some(table_name.clone()), "graph_name"),
            &mut filters,
            &mut new_to_original_column,
        )?;
        let mut plan =
            LogicalPlanBuilder::scan(&table_name, Arc::clone(&self.quad_table_source), None)?;
        if let Some(filters) = filters.into_iter().reduce(and) {
            plan = plan.filter(filters)?;
        }
        plan.project(
            new_to_original_column
                .into_iter()
                .map(|(variable, original_column)| {
                    Expr::from(original_column).alias(variable.as_str())
                }),
        )
    }

    fn plan_for_graph_names(&self) -> Result<LogicalPlanBuilder> {
        let out_table_name = self.table_name.next("g");
        if let Some(spec) = self.dataset_spec.and_then(|spec| spec.named.as_ref()) {
            if spec.is_empty() {
                // Workaround empty values not allowed
                LogicalPlanBuilder::empty(false)
                    .project([Expr::default().alias(GRAPH_MAGIC_COLUMN)])
            } else {
                LogicalPlanBuilder::values(
                    spec.iter()
                        .map(|name| Ok(vec![self.term_to_expr(name.clone())?]))
                        .collect::<Result<Vec<_>>>()?,
                )?
                .project([Expr::from(Column::from_name("column1")).alias(GRAPH_MAGIC_COLUMN)])
            }
        } else {
            let scan_table_name = self.table_name.next("quads");
            let graph_name_column = Column::new(Some(scan_table_name.clone()), "graph_name");
            LogicalPlanBuilder::scan(&scan_table_name, Arc::clone(&self.quad_table_source), None)?
                .filter(Expr::from(graph_name_column.clone()).is_not_null())?
                .project([Expr::from(graph_name_column.clone()).alias(GRAPH_MAGIC_COLUMN)])?
                .distinct()
        }?
        .alias(out_table_name.clone())
    }

    fn term_pattern_to_filter_or_project(
        &mut self,
        pattern: TermPattern,
        column: Column,
        filters: &mut Vec<Expr>,
        new_to_original_column: &mut HashMap<String, Column>,
        external_schema: &HashMap<String, Column>,
    ) -> Result<()> {
        enum ConstantOrVariable {
            Constant(Term),
            Variable(Variable),
        }
        let pattern = match pattern {
            TermPattern::NamedNode(n) => ConstantOrVariable::Constant(n.into()),
            TermPattern::BlankNode(n) => ConstantOrVariable::Variable(
                self.blank_node_to_variable
                    .entry(n.clone())
                    .or_insert_with(|| Variable::new_unchecked(n.to_string()))
                    .clone(),
            ),
            TermPattern::Literal(l) => ConstantOrVariable::Constant(l.into()),
            #[cfg(feature = "rdf-12")]
            TermPattern::Triple(_) => {
                return Err(DataFusionError::NotImplemented(
                    "RDF 1.2 triple terms are not implemented yet".into(),
                ));
            }
            TermPattern::Variable(v) => ConstantOrVariable::Variable(v),
        };
        match pattern {
            ConstantOrVariable::Constant(t) => {
                filters.push(Expr::from(column.clone()).eq(self.term_to_expr(t)?));
                Ok(())
            }
            ConstantOrVariable::Variable(v) => {
                if let Some(external_column) = external_schema.get(v.as_str()) {
                    filters.push(
                        Expr::from(column.clone())
                            .eq(Expr::OuterReferenceColumn(
                                DataType::Binary,
                                external_column.clone(),
                            ))
                            .or(Expr::OuterReferenceColumn(
                                DataType::Binary,
                                external_column.clone(),
                            )
                            .is_null()),
                    );
                }
                match new_to_original_column.entry(v.as_str().into()) {
                    Entry::Vacant(entry) => {
                        entry.insert(column);
                    }
                    Entry::Occupied(entry) => {
                        filters.push(Expr::from(column).eq(entry.get().clone().into()));
                    }
                }
                Ok(())
            }
        }
    }

    fn current_graph_to_filter_or_project(
        &self,
        in_default_graph: bool,
        column: Column,
        filters: &mut Vec<Expr>,
        new_to_original_column: &mut HashMap<String, Column>,
    ) -> Result<()> {
        if in_default_graph {
            filters.push(if let Some(spec) = self.dataset_spec {
                Expr::from(column.clone()).in_list(
                    spec.default
                        .iter()
                        .map(|e| self.term_to_expr(e.clone()))
                        .collect::<Result<Vec<_>>>()?,
                    false,
                )
            } else {
                Expr::from(column.clone()).is_null()
            });
        } else {
            filters.push(Expr::from(column.clone()).is_not_null());
            new_to_original_column.insert(GRAPH_MAGIC_COLUMN.into(), column);
        }
        Ok(())
    }

    fn effective_boolean_value_expression(
        &mut self,
        expression: &Expression,
        schema: &DFSchema,
        in_default_graph: bool,
        external_schema: &HashMap<String, Column>,
    ) -> Result<Expr> {
        Ok(match expression {
            Expression::And(left, right) => and(
                self.effective_boolean_value_expression(
                    left,
                    schema,
                    in_default_graph,
                    external_schema,
                )?,
                self.effective_boolean_value_expression(
                    right,
                    schema,
                    in_default_graph,
                    external_schema,
                )?,
            ),
            Expression::Or(left, right) => or(
                self.effective_boolean_value_expression(
                    left,
                    schema,
                    in_default_graph,
                    external_schema,
                )?,
                self.effective_boolean_value_expression(
                    right,
                    schema,
                    in_default_graph,
                    external_schema,
                )?,
            ),
            Expression::Not(inner) => not(self.effective_boolean_value_expression(
                inner,
                schema,
                in_default_graph,
                external_schema,
            )?),
            Expression::Bound(v) => {
                Self::variable_expression(v, schema, external_schema).is_not_null()
            }
            Expression::SameTerm(l, r) => self
                .expression(l, schema, in_default_graph, external_schema)?
                .eq(self.expression(r, schema, in_default_graph, external_schema)?),
            Expression::Exists(p) => {
                let mut external_schema = external_schema.clone();
                for (table_ref, field) in schema.iter() {
                    external_schema.insert(field.name().into(), Column::from((table_ref, field)));
                }
                exists(Arc::new(
                    self.plan_for_graph_pattern(p, in_default_graph, &external_schema)?
                        .build()?,
                ))
            }
            Expression::Equal(left, right) => term_equals(
                Arc::clone(&self.dataset),
                self.expression(left, schema, in_default_graph, external_schema)?,
                self.expression(right, schema, in_default_graph, external_schema)?,
            ),
            Expression::Less(left, right) => {
                let left = self.expression(left, schema, in_default_graph, external_schema)?;
                let right = self.expression(right, schema, in_default_graph, external_schema)?;
                self.compare_terms(left, ComparisonOperator::Less, right)
            }
            Expression::LessOrEqual(left, right) => {
                let left = self.expression(left, schema, in_default_graph, external_schema)?;
                let right = self.expression(right, schema, in_default_graph, external_schema)?;
                self.compare_terms(left, ComparisonOperator::LessOrEqual, right)
            }
            Expression::Greater(left, right) => {
                let left = self.expression(left, schema, in_default_graph, external_schema)?;
                let right = self.expression(right, schema, in_default_graph, external_schema)?;
                self.compare_terms(left, ComparisonOperator::Greater, right)
            }
            Expression::GreaterOrEqual(left, right) => {
                let left = self.expression(left, schema, in_default_graph, external_schema)?;
                let right = self.expression(right, schema, in_default_graph, external_schema)?;
                self.compare_terms(left, ComparisonOperator::GreaterOrEqual, right)
            }
            Expression::In(left, right) => {
                let left = self.expression(left, schema, in_default_graph, external_schema)?;
                right
                    .iter()
                    .map(|right| {
                        Ok(term_equals(
                            Arc::clone(&self.dataset),
                            left.clone(),
                            self.expression(right, schema, in_default_graph, external_schema)?,
                        ))
                    })
                    .reduce(|l: Result<_>, r| Ok(or(l?, r?)))
                    .unwrap_or_else(|| Ok(lit(false)))?
            }
            _ => effective_boolean_value(self.expression(
                expression,
                schema,
                in_default_graph,
                external_schema,
            )?),
        })
    }

    fn order_expression(
        &mut self,
        expression: &OrderExpression,
        schema: &DFSchema,
        in_default_graph: bool,
        external_schema: &HashMap<String, Column>,
    ) -> Result<Sort> {
        Ok(match expression {
            OrderExpression::Asc(e) => {
                let e = self.expression(e, schema, in_default_graph, external_schema)?;
                self.order_by_collation(e).sort(true, true)
            }
            OrderExpression::Desc(e) => {
                let e = self.expression(e, schema, in_default_graph, external_schema)?;
                self.order_by_collation(e).sort(false, true)
            }
        })
    }

    fn expression(
        &mut self,
        expression: &Expression,
        schema: &DFSchema,
        in_default_graph: bool,
        external_schema: &HashMap<String, Column>,
    ) -> Result<Expr> {
        Ok(match expression {
            Expression::Variable(v) => Self::variable_expression(v, schema, external_schema),
            Expression::NamedNode(l) => lit(encode_term(
                &self.dataset.internalize_term(l.clone().into())?,
            )),
            Expression::Literal(l) => lit(encode_term(
                &self.dataset.internalize_term(l.clone().into())?,
            )),
            Expression::And(_, _)
            | Expression::Or(_, _)
            | Expression::Not(_)
            | Expression::Bound(_)
            | Expression::SameTerm(_, _)
            | Expression::Exists(_)
            | Expression::Equal(_, _)
            | Expression::Less(_, _)
            | Expression::LessOrEqual(_, _)
            | Expression::Greater(_, _)
            | Expression::GreaterOrEqual(_, _)
            | Expression::In(_, _) => to_rdf_literal(self.effective_boolean_value_expression(
                expression,
                schema,
                in_default_graph,
                external_schema,
            )?),
            Expression::If(condition, t, f) => Expr::Case(Case::new(
                None,
                vec![(
                    Box::new(self.effective_boolean_value_expression(
                        condition,
                        schema,
                        in_default_graph,
                        external_schema,
                    )?),
                    Box::new(self.expression(t, schema, in_default_graph, external_schema)?),
                )],
                Some(Box::new(self.expression(
                    f,
                    schema,
                    in_default_graph,
                    external_schema,
                )?)),
            )),
            Expression::Coalesce(args) => coalesce(
                args.iter()
                    .map(|arg| self.expression(arg, schema, in_default_graph, external_schema))
                    .collect::<Result<Vec<_>>>()?,
            ),
            _ => {
                return Err(DataFusionError::NotImplemented(
                    "All expressions are not implemented yet".into(),
                ));
            }
        })
    }

    fn variable_expression(
        variable: &Variable,
        schema: &DFSchema,
        external_schema: &HashMap<String, Column>,
    ) -> Expr {
        if let Some(col) = schema_column(schema, variable.as_str()) {
            return col.into();
        }
        if let Some(col) = external_schema.get(variable.as_str()) {
            return Expr::OuterReferenceColumn(DataType::Binary, col.clone());
        }
        Expr::default().alias(variable.as_str())
    }

    fn ensure_qualified_names(&self, plan: LogicalPlanBuilder) -> Result<LogicalPlanBuilder> {
        if plan
            .schema()
            .iter()
            .all(|(table_ref, _)| table_ref.is_some())
        {
            // There are already table references everywhere
            return Ok(plan);
        }
        plan.alias(self.table_name.next("t"))
    }

    fn term_to_expr(&self, term: impl Into<Term>) -> Result<Expr> {
        Ok(lit(encode_term(
            &self.dataset.internalize_term(term.into())?,
        )))
    }

    fn order_by_collation(&self, expr: Expr) -> Expr {
        order_by_collation(Arc::clone(&self.dataset), expr)
    }

    fn compare_terms(&self, left: Expr, op: ComparisonOperator, right: Expr) -> Expr {
        compare_terms(Arc::clone(&self.dataset), left, op, right)
    }
}

fn schema_column(schema: &DFSchema, column: &str) -> Option<Column> {
    Some(Column::from(
        schema.iter().find(|(_, field)| field.name() == column)?,
    ))
}

fn eq_with_null_match_anything(left: impl Into<Expr>, right: impl Into<Expr>) -> Expr {
    let left = left.into();
    let right = right.into();
    left.clone()
        .eq(right.clone())
        .or(left.is_null().or(right.is_null()))
}

type ReferencedField<'a> = (Option<&'a TableReference>, &'a Field);

fn left_and_right_fields_by_name<'a>(
    left_schema: &'a DFSchemaRef,
    right_schema: &'a DFSchemaRef,
) -> HashMap<&'a str, (Option<ReferencedField<'a>>, Option<ReferencedField<'a>>)> {
    let mut result =
        HashMap::<&'a str, (Option<ReferencedField<'a>>, Option<ReferencedField<'a>>)>::new();
    for (table_ref, field) in left_schema.iter() {
        if !field.data_type().is_null() {
            result.entry(field.name()).or_default().0 = Some((table_ref, field));
        }
    }
    for (table_ref, field) in right_schema.iter() {
        if !field.data_type().is_null() {
            result.entry(field.name()).or_default().1 = Some((table_ref, field));
        }
    }
    result
}

fn table_source(provider: impl TableProvider + 'static) -> Arc<dyn TableSource> {
    Arc::new(DefaultTableSource::new(Arc::new(provider)))
}

fn filter_external_schema(
    schema: &HashMap<String, Column>,
    filter: &[Variable],
) -> HashMap<String, Column> {
    schema
        .iter()
        .filter(|(column, _)| filter.iter().any(|variable| *column == variable.as_str()))
        .map(|(column, expr)| (column.clone(), expr.clone()))
        .collect()
}
