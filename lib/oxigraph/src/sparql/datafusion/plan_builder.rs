use crate::sparql::datafusion::function::{
    agg_avg, agg_max, agg_min, agg_sum, divide, effective_boolean_value, greater_than,
    greater_than_or_equal, is_blank, lang, lang_matches, less_than, less_than_or_equal, multiply,
    order_by_collation, plus, regex, str, subtract, term_equals, to_rdf_literal, xsd_decimal,
    xsd_double, xsd_float, xsd_integer,
};
use crate::sparql::datafusion::table::QuadTableProvider;
use crate::sparql::dataset::DatasetView;
use crate::storage::binary_encoder::encode_term;
use datafusion::arrow::datatypes::Field;
use datafusion::common::alias::AliasGenerator;
use datafusion::common::{
    Column, DFSchema, DFSchemaRef, JoinType, Result, TableReference, not_impl_err,
};
use datafusion::datasource::cte_worktable::CteWorkTable;
use datafusion::datasource::{DefaultTableSource, TableProvider};
use datafusion::functions::expr_fn::coalesce;
use datafusion::functions_aggregate::count::{count, count_all, count_distinct, count_udaf};
use datafusion::logical_expr::expr::{AggregateFunction as DFAggregateFunction, Sort};
use datafusion::logical_expr::{
    Case, Expr, LogicalPlanBuilder, TableSource, and, exists, lit, not, or,
};
use datafusion::prelude::make_array;
use oxrdf::vocab::xsd;
use oxrdf::{BlankNode, Term, Variable};
use spareval::QueryableDataset;
use spargebra::algebra::{
    AggregateExpression, AggregateFunction, Expression, Function, GraphPattern, OrderExpression,
    PropertyPathExpression, QueryDataset,
};
use spargebra::term::{NamedNodePattern, TermPattern};
use std::collections::HashMap;
use std::iter::once;
use std::mem::swap;
use std::sync::Arc;

const SUBJECT_MAGIC_COLUMN: &str = "#subject#";
const OBJECT_MAGIC_COLUMN: &str = "#object#";
const PREDICATE_MAGIC_COLUMN: &str = "#predicate#";
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
        // TODO: add a graph name table?
        Self {
            dataset: Arc::clone(&dataset),
            quad_table_source: table_source(QuadTableProvider::new(dataset)),
            blank_node_to_variable: HashMap::new(),
            table_name: AliasGenerator::new(),
            dataset_spec,
        }
    }

    /// Plan for a SELECT query. Returns a table with a column per variable
    pub fn select_plan(&mut self, pattern: &GraphPattern) -> Result<LogicalPlanBuilder> {
        self.plan_for_graph_pattern(pattern, true, &DFSchema::empty())
    }

    /// Plan for a DESCRIBE query. Returns a table with subject, predicate and object columns
    ///
    /// It implements more or less [Concise Bounded Description](https://www.w3.org/submissions/CBD/),
    /// by including the description of every blank node recursively.
    pub fn describe_plan(&mut self, pattern: &GraphPattern) -> Result<LogicalPlanBuilder> {
        if self.dataset_spec.is_some() {
            return not_impl_err!(
                "DESCRIBE queries are not supported with a dataset specification"
            );
        };
        let input_plan = self.select_plan(pattern)?;
        let input_columns = input_plan.schema().columns();
        let (to_describe_plan, to_describe_column) = if input_columns.len() == 1 {
            (input_plan, input_columns.into_iter().next().unwrap())
        } else {
            let table_name = self.table_name.next("describe");
            let all_columns = make_array(input_columns.into_iter().map(Expr::from).collect());
            let all_columns_col = Column::new_unqualified(all_columns.name_for_alias()?);
            (
                input_plan
                    .project(vec![all_columns])?
                    .alias(&table_name)?
                    .unnest_column(all_columns_col.clone())?,
                all_columns_col,
            )
        };
        let triple_scan =
            LogicalPlanBuilder::scan("quads", Arc::clone(&self.quad_table_source), None)?
                .filter(Expr::from(Column::new(Some("quads"), "graph_name")).is_null())?
                .build()?;
        to_describe_plan
            .join(
                triple_scan.clone(),
                JoinType::Inner,
                (
                    vec![to_describe_column],
                    vec![Column::new(Some("quads"), "subject")],
                ),
                None,
            )?
            .project(vec![
                Column::new(Some("quads"), "subject"),
                Column::new(Some("quads"), "predicate"),
                Column::new(Some("quads"), "object"),
            ])?
            .to_recursive_query(
                "cbd".into(),
                LogicalPlanBuilder::scan(
                    "cbd",
                    table_source(CteWorkTable::new(
                        "cbd",
                        Arc::clone(triple_scan.schema().inner()),
                    )),
                    None,
                )?
                .filter(is_blank(
                    Arc::clone(&self.dataset),
                    Column::new(Some("cbd"), "object").into(),
                ))?
                .join(
                    triple_scan,
                    JoinType::Inner,
                    (
                        vec![Column::new(Some("cbd"), "object")],
                        vec![Column::new(Some("quads"), "subject")],
                    ),
                    None,
                )?
                .project(vec![
                    Column::new(Some("quads"), "subject"),
                    Column::new(Some("quads"), "predicate"),
                    Column::new(Some("quads"), "object"),
                ])?
                .build()?,
                true,
            )
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
        external_schema: &DFSchema,
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
                            Expr::from(Column::new_unqualified(format!("column{}", i + 1)))
                                .alias(variable.as_str())
                        },
                    ))
                }
            }
            GraphPattern::Bgp { patterns } => {
                let mut plans = patterns
                    .iter()
                    .map(|p| {
                        let subject = self.term_or_variable(p.subject.clone())?;
                        let predicate = self.term_or_variable(p.predicate.clone().into())?;
                        let object = self.term_or_variable(p.object.clone())?;
                        self.plan_for_triple_pattern(
                            subject,
                            predicate,
                            object,
                            in_default_graph,
                            external_schema,
                        )
                    })
                    .collect::<Result<Vec<_>>>()?;
                // Join ordering: we join the first pair of plans with the largest number of shared variables
                // until there is a single plan left
                while plans.len() > 1 {
                    let mut best_number_of_shared_variables = 0;
                    let mut best_pair = (0, 1);
                    for i in 0..plans.len() {
                        for j in (i + 1)..plans.len() {
                            let current_number_of_shared_variables = plans[i]
                                .schema()
                                .fields()
                                .iter()
                                .filter(|l| {
                                    plans[j].schema().has_column_with_unqualified_name(l.name())
                                })
                                .count();
                            if current_number_of_shared_variables > best_number_of_shared_variables
                            {
                                best_number_of_shared_variables =
                                    current_number_of_shared_variables;
                                best_pair = (i, j);
                            }
                        }
                    }
                    // We merge the best pair
                    let right = plans.remove(best_pair.1); // first to avoid being shifted
                    let left = plans.remove(best_pair.0);
                    plans.push(self.join(
                        left,
                        JoinType::Inner,
                        right,
                        None,
                        in_default_graph,
                        external_schema,
                    )?);
                }
                Ok(plans
                    .into_iter()
                    .next() // We only have at most one plan left
                    .unwrap_or_else(|| LogicalPlanBuilder::empty(true)))
            }
            GraphPattern::Path {
                subject,
                path,
                object,
            } => {
                let subject = self.term_or_variable(subject.clone())?;
                let object = self.term_or_variable(object.clone())?;
                self.plan_for_property_path(
                    subject,
                    path.clone(),
                    object,
                    in_default_graph,
                    external_schema,
                )
            }
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
                    if let Some(output_column_name) = output_column_name {
                        if let Some(outer_ref_col) =
                            outer_reference_column_from_schema(external_schema, output_column_name)
                        {
                            // We apply external constraint
                            plan = plan.filter(eq_with_null_match_anything(
                                graph_column.clone(),
                                outer_ref_col,
                            ))?;
                        }
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
                let mut right_external_schema = (**left.schema()).clone();
                right_external_schema.merge(external_schema);
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
                        let (aggregate, convert_back_to_term) = match expression {
                            AggregateExpression::CountSolutions { distinct } => {
                                if *distinct {
                                    // We count the columns that are not in the group part
                                    (
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
                                        )),
                                        true,
                                    )
                                } else {
                                    (count_all(), true)
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
                                match name {
                                    AggregateFunction::Count => (
                                        if *distinct {
                                            count_distinct(expression)
                                        } else {
                                            count(expression)
                                        },
                                        true,
                                    ),
                                    AggregateFunction::Sum => (
                                        agg_sum(Arc::clone(&self.dataset), expression, *distinct),
                                        false,
                                    ),
                                    AggregateFunction::Avg => (
                                        agg_avg(Arc::clone(&self.dataset), expression, *distinct),
                                        false,
                                    ),
                                    AggregateFunction::Min => (
                                        agg_min(Arc::clone(&self.dataset), expression, *distinct),
                                        false,
                                    ),
                                    AggregateFunction::Max => (
                                        agg_max(Arc::clone(&self.dataset), expression, *distinct),
                                        false,
                                    ),
                                    _ => {
                                        return not_impl_err!("{name} is not implemented yet");
                                    }
                                }
                            }
                        };
                        let mut proj = Column::new_unqualified(aggregate.name_for_alias()?).into();
                        if convert_back_to_term {
                            proj = to_rdf_literal(proj);
                        }
                        projection.push(proj.alias(target_var.as_str()));
                        Ok(aggregate)
                    })
                    .collect::<Result<Vec<_>>>()?;
                plan.aggregate(group_expr, aggr_expr)?.project(projection)
            }
            GraphPattern::Project { inner, variables } => {
                // We only keep externals that are in the projection
                let external_schema = filter_external_schema(external_schema, variables)?;
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
                    let external_schema = filter_external_schema(external_schema, variables)?;
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
            GraphPattern::Service { .. } => not_impl_err!("SERVICE is not implemented yet"),
        }
    }

    fn join(
        &mut self,
        left_plan: LogicalPlanBuilder,
        join_type: JoinType,
        right_plan: LogicalPlanBuilder,
        filter: Option<&Expression>,
        in_default_graph: bool,
        external_schema: &DFSchema,
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
        &self,
        subject: TermOrVariable,
        predicate: TermOrVariable,
        object: TermOrVariable,
        in_default_graph: bool,
        external_schema: &DFSchema,
    ) -> Result<LogicalPlanBuilder> {
        let mut filters = Vec::new();
        let mut new_to_original_column = Vec::new();
        let table_name = self.table_name.next("quads");
        self.term_pattern_to_filter_or_project(
            subject,
            Column::new(Some(table_name.clone()), "subject"),
            &mut filters,
            &mut new_to_original_column,
            external_schema,
        )?;
        self.term_pattern_to_filter_or_project(
            predicate,
            Column::new(Some(table_name.clone()), "predicate"),
            &mut filters,
            &mut new_to_original_column,
            external_schema,
        )?;
        self.term_pattern_to_filter_or_project(
            object,
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
                .project([Expr::from(Column::new_unqualified("column1")).alias(GRAPH_MAGIC_COLUMN)])
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
        &self,
        pattern: TermOrVariable,
        column: Column,
        filters: &mut Vec<Expr>,
        new_to_original_column: &mut Vec<(String, Column)>,
        external_schema: &DFSchema,
    ) -> Result<()> {
        match pattern {
            TermOrVariable::Term(t) => {
                filters.push(Expr::from(column.clone()).eq(self.term_to_expr(t)?));
                Ok(())
            }
            TermOrVariable::Variable(v) => {
                if let Some(outer_ref_col) =
                    outer_reference_column_from_schema(external_schema, v.as_str())
                {
                    filters.push(
                        Expr::from(column.clone())
                            .eq(outer_ref_col.clone())
                            .or(outer_ref_col.is_null()),
                    );
                }
                if let Some((_, existing_column)) = new_to_original_column
                    .iter()
                    .find(|(new, _)| new == v.as_str())
                {
                    filters.push(Expr::from(column).eq(existing_column.clone().into()));
                } else {
                    new_to_original_column.push((v.as_str().into(), column));
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
        new_to_original_column: &mut Vec<(String, Column)>,
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
            new_to_original_column.push((GRAPH_MAGIC_COLUMN.into(), column));
        }
        Ok(())
    }

    /// Builds a plan for a property path. Returns a table with two columns named start and end
    fn plan_for_property_path(
        &mut self,
        mut subject: TermOrVariable,
        path: PropertyPathExpression,
        mut object: TermOrVariable,
        in_default_graph: bool,
        external_schema: &DFSchema,
    ) -> Result<LogicalPlanBuilder> {
        match path {
            PropertyPathExpression::NamedNode(predicate) => self.plan_for_triple_pattern(
                subject,
                TermOrVariable::Term(predicate.into()),
                object,
                in_default_graph,
                external_schema,
            ),
            PropertyPathExpression::Reverse(p) => {
                self.plan_for_property_path(object, *p, subject, in_default_graph, external_schema)
            }
            PropertyPathExpression::Sequence(l, r) => {
                let middle_column_name = format!("#{}#", self.table_name.next("middle"));
                let left = self.plan_for_property_path(
                    subject.clone(),
                    *l,
                    TermOrVariable::Variable(Variable::new_unchecked(&middle_column_name)),
                    in_default_graph,
                    external_schema,
                )?;
                let left = self.ensure_qualified_names(left)?;
                let right = self.plan_for_property_path(
                    TermOrVariable::Variable(Variable::new_unchecked(&middle_column_name)),
                    *r,
                    object.clone(),
                    in_default_graph,
                    external_schema,
                )?;
                let right = self.ensure_qualified_names(right)?;
                let mut projection = Vec::new();
                if let TermOrVariable::Variable(v) = &subject {
                    projection.push(schema_column(left.schema(), v.as_str()).unwrap());
                }
                let mut left_join_keys =
                    vec![schema_column(left.schema(), &middle_column_name).unwrap()];
                let mut right_join_keys =
                    vec![schema_column(right.schema(), &middle_column_name).unwrap()];
                if let TermOrVariable::Variable(v) = &object {
                    if subject == object {
                        // We ensure subject and object are equals
                        left_join_keys.push(schema_column(left.schema(), v.as_str()).unwrap());
                        right_join_keys.push(schema_column(right.schema(), v.as_str()).unwrap());
                    } else {
                        projection.push(schema_column(right.schema(), v.as_str()).unwrap());
                    }
                }
                if let (Some(left_graph), Some(right_graph)) = (
                    schema_column(left.schema(), GRAPH_MAGIC_COLUMN),
                    schema_column(right.schema(), GRAPH_MAGIC_COLUMN),
                ) {
                    projection.push(left_graph.clone());
                    left_join_keys.push(left_graph);
                    right_join_keys.push(right_graph);
                }
                left.join(
                    right.build()?,
                    JoinType::Inner,
                    (left_join_keys, right_join_keys),
                    None,
                )?
                .project(projection)
            }
            PropertyPathExpression::Alternative(a, b) => self
                .plan_for_property_path(
                    subject.clone(),
                    *a,
                    object.clone(),
                    in_default_graph,
                    external_schema,
                )?
                .union_by_name(
                    self.plan_for_property_path(
                        subject,
                        *b,
                        object,
                        in_default_graph,
                        external_schema,
                    )?
                    .build()?,
                ),
            PropertyPathExpression::ZeroOrMore(p) => {
                // p* = p+?
                self.plan_for_property_path(
                    subject,
                    PropertyPathExpression::ZeroOrOne(Box::new(PropertyPathExpression::OneOrMore(
                        p,
                    ))),
                    object,
                    in_default_graph,
                    external_schema,
                )
            }
            PropertyPathExpression::OneOrMore(p) => {
                // We swap subject and object if object is a constant and subject a variable to make the query more efficient
                let mut p = *p;
                if matches!(subject, TermOrVariable::Variable(_))
                    && !matches!(object, TermOrVariable::Variable(_))
                {
                    swap(&mut subject, &mut object);
                    p = PropertyPathExpression::Reverse(Box::new(p));
                }
                let table_name = self.table_name.next("closure");
                let middle_column_name = format!("#{}#", self.table_name.next("middle"));
                let end_column_name = format!("#{}#", self.table_name.next("end"));
                let input = self.plan_for_property_path(
                    subject.clone(),
                    p.clone(),
                    TermOrVariable::Variable(Variable::new_unchecked(end_column_name.clone())),
                    in_default_graph,
                    external_schema,
                )?;
                let schema = Arc::clone(input.schema().inner());
                let recursive_left = LogicalPlanBuilder::scan(
                    table_name.clone(),
                    table_source(CteWorkTable::new(&table_name, schema)),
                    None,
                )?;
                let mut recursive_left_projection = Vec::new();
                if let TermOrVariable::Variable(v) = &subject {
                    recursive_left_projection
                        .push(Column::new(Some(table_name.clone()), v.as_str()).into());
                }
                recursive_left_projection.push(
                    Expr::from(Column::new(Some(table_name.clone()), &end_column_name))
                        .alias(&middle_column_name),
                );
                if let Some(g) = schema_column(recursive_left.schema(), GRAPH_MAGIC_COLUMN) {
                    recursive_left_projection.push(g.into());
                }
                let recursive_left = self
                    .ensure_qualified_names(recursive_left.project(recursive_left_projection)?)?;

                let recursive_right = self.plan_for_property_path(
                    TermOrVariable::Variable(Variable::new_unchecked(middle_column_name.clone())),
                    p,
                    TermOrVariable::Variable(Variable::new_unchecked(end_column_name.clone())),
                    in_default_graph,
                    external_schema,
                )?;
                let recursive_right = self.ensure_qualified_names(recursive_right)?;
                let mut output_projection = Vec::new();
                if let TermOrVariable::Variable(v) = &subject {
                    output_projection
                        .push(schema_column(recursive_left.schema(), v.as_str()).unwrap());
                }
                let left_object_column =
                    schema_column(recursive_left.schema(), &middle_column_name).unwrap();
                let mut left_join_keys = vec![left_object_column];
                let mut right_join_keys =
                    vec![schema_column(recursive_right.schema(), &middle_column_name).unwrap()];
                output_projection
                    .push(schema_column(recursive_right.schema(), &end_column_name).unwrap());
                if let (Some(left_graph), Some(right_graph)) = (
                    schema_column(recursive_left.schema(), GRAPH_MAGIC_COLUMN),
                    schema_column(recursive_right.schema(), GRAPH_MAGIC_COLUMN),
                ) {
                    output_projection.push(left_graph.clone());
                    left_join_keys.push(left_graph);
                    right_join_keys.push(right_graph);
                }
                let recursive = recursive_left
                    .join(
                        recursive_right.build()?,
                        JoinType::Inner,
                        (left_join_keys, right_join_keys),
                        None,
                    )?
                    .project(output_projection)?;
                let mut plan = input.to_recursive_query(table_name, recursive.build()?, true)?;
                // We do a final projection to get the correct end variable
                let end_column = schema_column(plan.schema(), &end_column_name).unwrap();
                let mut projection = Vec::new();
                if let TermOrVariable::Variable(subject) = &subject {
                    projection.push(Expr::from(
                        schema_column(plan.schema(), subject.as_str()).unwrap(),
                    ));
                }
                match object {
                    TermOrVariable::Term(object) => {
                        // We make sure to filter the output
                        plan =
                            plan.filter(Expr::from(end_column).eq(self.term_to_expr(object)?))?;
                    }
                    TermOrVariable::Variable(object) => {
                        if let TermOrVariable::Variable(subject) = &subject {
                            if *subject == object {
                                let subject_column =
                                    schema_column(plan.schema(), subject.as_str()).unwrap();
                                plan =
                                    plan.filter(Expr::from(end_column).eq(subject_column.into()))?;
                            } else {
                                projection.push(Expr::from(end_column).alias(object.as_str()));
                            }
                        } else {
                            projection.push(Expr::from(end_column).alias(object.as_str()));
                        }
                    }
                }
                if let Some(graph) = schema_column(plan.schema(), GRAPH_MAGIC_COLUMN) {
                    projection.push(graph.into());
                }
                plan.project(projection)
            }
            PropertyPathExpression::ZeroOrOne(p) => {
                // TODO: binding from external schema implies the Term case
                match (subject, object) {
                    (TermOrVariable::Term(subject), TermOrVariable::Term(object)) => {
                        if subject == object {
                            Ok(LogicalPlanBuilder::empty(true))
                        } else {
                            self.plan_for_property_path(
                                TermOrVariable::Term(subject),
                                *p,
                                TermOrVariable::Term(object),
                                in_default_graph,
                                external_schema,
                            )
                        }
                    }
                    (TermOrVariable::Term(subject), TermOrVariable::Variable(object)) => self
                        .plan_for_property_path(
                            TermOrVariable::Term(subject.clone()),
                            *p,
                            TermOrVariable::Variable(object.clone()),
                            in_default_graph,
                            external_schema,
                        )?
                        .union_by_name_distinct(
                            LogicalPlanBuilder::values(vec![vec![self.term_to_expr(subject)?]])?
                                .project(vec![
                                    Expr::from(Column::new_unqualified("column1"))
                                        .alias(object.as_str()),
                                ])?
                                .build()?,
                        ),
                    (TermOrVariable::Variable(subject), TermOrVariable::Term(object)) => {
                        // We swap subject and object to move to the previous case
                        self.plan_for_property_path(
                            TermOrVariable::Term(object),
                            PropertyPathExpression::Reverse(p.clone()),
                            TermOrVariable::Variable(subject),
                            in_default_graph,
                            external_schema,
                        )
                    }
                    (TermOrVariable::Variable(subject), TermOrVariable::Variable(object)) => {
                        if external_schema.has_column_with_unqualified_name(subject.as_str())
                            || external_schema.has_column_with_unqualified_name(object.as_str())
                        {
                            return not_impl_err!(
                                "Correlated queries and property path is not implemented yet"
                            );
                        }
                        let subject_object_plan = self.plan_for_triple_pattern(
                            TermOrVariable::Variable(Variable::new_unchecked(SUBJECT_MAGIC_COLUMN)),
                            TermOrVariable::Variable(Variable::new_unchecked(
                                PREDICATE_MAGIC_COLUMN,
                            )),
                            TermOrVariable::Variable(Variable::new_unchecked(OBJECT_MAGIC_COLUMN)),
                            in_default_graph,
                            &DFSchema::empty(),
                        )?;
                        let term_column_expr = make_array(vec![
                            Expr::from(
                                schema_column(subject_object_plan.schema(), SUBJECT_MAGIC_COLUMN)
                                    .unwrap(),
                            ),
                            Expr::from(
                                schema_column(subject_object_plan.schema(), OBJECT_MAGIC_COLUMN)
                                    .unwrap(),
                            ),
                        ]);
                        let term_column =
                            Column::new_unqualified(term_column_expr.name_for_alias()?);
                        let mut projection = vec![term_column_expr];
                        if let Some(g) =
                            schema_column(subject_object_plan.schema(), GRAPH_MAGIC_COLUMN)
                        {
                            projection.push(g.clone().into());
                        }
                        let graph_terms_plan = subject_object_plan
                            .project(projection)?
                            .unnest_column(term_column.clone())?;
                        let mut projection = if subject == object {
                            vec![Expr::from(term_column.clone()).alias(subject.as_str())]
                        } else {
                            vec![
                                Expr::from(term_column.clone()).alias(subject.as_str()),
                                Expr::from(term_column.clone()).alias(object.as_str()),
                            ]
                        };
                        if let Some(g) =
                            schema_column(graph_terms_plan.schema(), GRAPH_MAGIC_COLUMN)
                        {
                            projection.push(g.clone().into());
                        }
                        let graph_terms_plan = graph_terms_plan.project(projection)?;
                        self.plan_for_property_path(
                            TermOrVariable::Variable(subject.clone()),
                            *p,
                            TermOrVariable::Variable(object.clone()),
                            in_default_graph,
                            external_schema,
                        )?
                        .union_by_name_distinct(graph_terms_plan.build()?)
                    }
                }
            }
            PropertyPathExpression::NegatedPropertySet(ps) => self
                .plan_for_triple_pattern(
                    subject.clone(),
                    TermOrVariable::Variable(Variable::new_unchecked(PREDICATE_MAGIC_COLUMN)),
                    object.clone(),
                    in_default_graph,
                    external_schema,
                )?
                .filter(
                    Expr::from(Column::new_unqualified(PREDICATE_MAGIC_COLUMN)).in_list(
                        ps.iter()
                            .map(|p| self.term_to_expr(p.clone()))
                            .collect::<Result<Vec<_>>>()?,
                        true,
                    ),
                ),
        }
    }

    fn effective_boolean_value_expression(
        &mut self,
        expression: &Expression,
        schema: &DFSchema,
        in_default_graph: bool,
        external_schema: &DFSchema,
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
                let mut new_external_schema = schema.clone();
                new_external_schema.merge(external_schema);
                exists(Arc::new(
                    self.plan_for_graph_pattern(p, in_default_graph, &new_external_schema)?
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
                less_than(Arc::clone(&self.dataset), left, right)
            }
            Expression::LessOrEqual(left, right) => {
                let left = self.expression(left, schema, in_default_graph, external_schema)?;
                let right = self.expression(right, schema, in_default_graph, external_schema)?;
                less_than_or_equal(Arc::clone(&self.dataset), left, right)
            }
            Expression::Greater(left, right) => {
                let left = self.expression(left, schema, in_default_graph, external_schema)?;
                let right = self.expression(right, schema, in_default_graph, external_schema)?;
                greater_than(Arc::clone(&self.dataset), left, right)
            }
            Expression::GreaterOrEqual(left, right) => {
                let left = self.expression(left, schema, in_default_graph, external_schema)?;
                let right = self.expression(right, schema, in_default_graph, external_schema)?;
                greater_than_or_equal(Arc::clone(&self.dataset), left, right)
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
            Expression::FunctionCall(function, args) => match function {
                Function::LangMatches => lang_matches(
                    Arc::clone(&self.dataset),
                    self.expression(&args[0], schema, in_default_graph, external_schema)?,
                    self.expression(&args[1], schema, in_default_graph, external_schema)?,
                ),
                Function::Regex => regex(
                    Arc::clone(&self.dataset),
                    self.expression(&args[0], schema, in_default_graph, external_schema)?,
                    self.expression(&args[1], schema, in_default_graph, external_schema)?,
                    if args.len() > 2 {
                        Some(self.expression(
                            &args[2],
                            schema,
                            in_default_graph,
                            external_schema,
                        )?)
                    } else {
                        None
                    },
                ),
                _ => effective_boolean_value(self.expression(
                    expression,
                    schema,
                    in_default_graph,
                    external_schema,
                )?),
            },
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
        external_schema: &DFSchema,
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
        external_schema: &DFSchema,
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
            Expression::If(condition, t, f) => {
                let condition = self.effective_boolean_value_expression(
                    condition,
                    schema,
                    in_default_graph,
                    external_schema,
                )?;
                Expr::Case(Case::new(
                    None,
                    vec![
                        (
                            Box::new(condition.clone().is_true()),
                            Box::new(self.expression(
                                t,
                                schema,
                                in_default_graph,
                                external_schema,
                            )?),
                        ),
                        (
                            Box::new(condition.is_false()),
                            Box::new(self.expression(
                                f,
                                schema,
                                in_default_graph,
                                external_schema,
                            )?),
                        ),
                    ],
                    None,
                ))
            }
            Expression::Coalesce(args) => coalesce(
                args.iter()
                    .map(|arg| self.expression(arg, schema, in_default_graph, external_schema))
                    .collect::<Result<Vec<_>>>()?,
            ),
            Expression::Add(left, right) => plus(
                Arc::clone(&self.dataset),
                self.expression(left, schema, in_default_graph, external_schema)?,
                self.expression(right, schema, in_default_graph, external_schema)?,
            ),
            Expression::Subtract(left, right) => subtract(
                Arc::clone(&self.dataset),
                self.expression(left, schema, in_default_graph, external_schema)?,
                self.expression(right, schema, in_default_graph, external_schema)?,
            ),
            Expression::Multiply(left, right) => multiply(
                Arc::clone(&self.dataset),
                self.expression(left, schema, in_default_graph, external_schema)?,
                self.expression(right, schema, in_default_graph, external_schema)?,
            ),
            Expression::Divide(left, right) => divide(
                Arc::clone(&self.dataset),
                self.expression(left, schema, in_default_graph, external_schema)?,
                self.expression(right, schema, in_default_graph, external_schema)?,
            ),
            Expression::FunctionCall(function, args) => match function {
                Function::Str => str(
                    Arc::clone(&self.dataset),
                    self.expression(&args[0], schema, in_default_graph, external_schema)?,
                ),
                Function::Lang => lang(
                    Arc::clone(&self.dataset),
                    self.expression(&args[0], schema, in_default_graph, external_schema)?,
                ),
                Function::LangMatches | Function::Regex => {
                    to_rdf_literal(self.effective_boolean_value_expression(
                        expression,
                        schema,
                        in_default_graph,
                        external_schema,
                    )?)
                }
                Function::Custom(function) => match function.as_ref() {
                    xsd::INTEGER => xsd_integer(
                        Arc::clone(&self.dataset),
                        self.expression(&args[0], schema, in_default_graph, external_schema)?,
                    ),
                    xsd::DECIMAL => xsd_decimal(
                        Arc::clone(&self.dataset),
                        self.expression(&args[0], schema, in_default_graph, external_schema)?,
                    ),
                    xsd::FLOAT => xsd_float(
                        Arc::clone(&self.dataset),
                        self.expression(&args[0], schema, in_default_graph, external_schema)?,
                    ),
                    xsd::DOUBLE => xsd_double(
                        Arc::clone(&self.dataset),
                        self.expression(&args[0], schema, in_default_graph, external_schema)?,
                    ),
                    _ => {
                        return not_impl_err!("{function} is not implemented yet");
                    }
                },
                _ => {
                    return not_impl_err!("{function} is not implemented yet");
                }
            },
            _ => {
                return not_impl_err!("{expression} is not implemented yet");
            }
        })
    }

    fn variable_expression(
        variable: &Variable,
        schema: &DFSchema,
        external_schema: &DFSchema,
    ) -> Expr {
        if let Some(col) = schema_column(schema, variable.as_str()) {
            return col.into();
        }
        if let Some(out_ref_col) =
            outer_reference_column_from_schema(external_schema, variable.as_str())
        {
            return out_ref_col;
        }
        Expr::default().alias(variable.as_str())
    }

    fn term_or_variable(&mut self, pattern: TermPattern) -> Result<TermOrVariable> {
        Ok(match pattern {
            TermPattern::NamedNode(n) => TermOrVariable::Term(n.into()),
            TermPattern::BlankNode(n) => TermOrVariable::Variable(
                self.blank_node_to_variable
                    .entry(n.clone())
                    .or_insert_with(|| Variable::new_unchecked(n.to_string()))
                    .clone(),
            ),
            TermPattern::Literal(l) => TermOrVariable::Term(l.into()),
            #[cfg(feature = "rdf-12")]
            TermPattern::Triple(_) => {
                return not_impl_err!("RDF 1.2 triple terms are not implemented yet");
            }
            TermPattern::Variable(v) => TermOrVariable::Variable(v),
        })
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
}

#[derive(Clone, PartialEq, Eq)]
enum TermOrVariable {
    Term(Term),
    Variable(Variable),
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

fn filter_external_schema(schema: &DFSchema, filter: &[Variable]) -> Result<DFSchema> {
    DFSchema::new_with_metadata(
        schema
            .iter()
            .filter(|(_, field)| filter.iter().any(|v| v.as_str() == field.name()))
            .map(|(table_ref, field)| (table_ref.cloned(), Arc::clone(field)))
            .collect(),
        schema.metadata().clone(),
    )
}

fn outer_reference_column_from_schema(schema: &DFSchema, column: &str) -> Option<Expr> {
    let (table_ref, field) = schema.iter().find(|(_, field)| field.name() == column)?;
    Some(Expr::OuterReferenceColumn(
        Arc::clone(field),
        Column::new(table_ref.cloned(), field.name()),
    ))
}
