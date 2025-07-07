use crate::sparql::datafusion::functions::{EffectiveBooleanValue, ToRdfLiteral};
use crate::sparql::datafusion::table::QuadTableProvider;
use crate::sparql::dataset::DatasetView;
use crate::storage::binary_encoder::encode_term;
use datafusion::arrow::datatypes::DataType;
use datafusion::common::{Column, DataFusionError, JoinType, Result};
use datafusion::datasource::DefaultTableSource;
use datafusion::functions_aggregate::count::{count, count_all, count_distinct};
use datafusion::logical_expr::{
    Expr, ExprSchemable, LogicalPlan, LogicalPlanBuilder, ScalarUDF, TableSource, and, lit, not, or,
};
use datafusion::prelude::coalesce;
use oxrdf::{BlankNode, Term, Variable};
use spareval::QueryableDataset;
use spargebra::algebra::{AggregateExpression, AggregateFunction, Expression, GraphPattern};
use spargebra::term::{TermPattern, TriplePattern};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::sync::Arc;

pub struct SparqlPlanBuilder {
    dataset: Arc<DatasetView<'static>>,
    quad_table_source: Arc<dyn TableSource>,
    counter_per_variable: HashMap<Variable, usize>,
    blank_node_to_variable: HashMap<BlankNode, Variable>,
    triple_table_counter: usize,
    values_table_counter: usize,
}

impl SparqlPlanBuilder {
    pub fn new(dataset: Arc<DatasetView<'static>>) -> Self {
        Self {
            dataset: Arc::clone(&dataset),
            quad_table_source: Arc::new(DefaultTableSource::new(Arc::new(QuadTableProvider::new(
                dataset,
            )))),
            counter_per_variable: HashMap::new(),
            blank_node_to_variable: HashMap::new(),
            triple_table_counter: 0,
            values_table_counter: 0,
        }
    }

    pub fn build_plan_for_graph_pattern(
        &mut self,
        pattern: &GraphPattern,
        limit: Option<usize>,
    ) -> Result<(LogicalPlan, Arc<[Variable]>)> {
        let (plan, variable_mapping) = self.plan_for_graph_pattern(pattern)?;
        let mut plan = plan.project(
            variable_mapping
                .iter()
                .map(|(to, from)| from.clone().alias(to.as_str())),
        )?;
        if let Some(limit) = limit {
            plan = plan.limit(0, Some(limit))?;
        }
        Ok((plan.build()?, variable_mapping.keys().cloned().collect()))
    }

    fn plan_for_graph_pattern(
        &mut self,
        pattern: &GraphPattern,
    ) -> Result<(LogicalPlanBuilder, HashMap<Variable, Expr>)> {
        match pattern {
            GraphPattern::Bgp { patterns } => {
                let plans = patterns
                    .iter()
                    .map(|p| self.plan_for_triple_pattern(p))
                    .collect::<Vec<_>>();
                plans
                    .into_iter()
                    .reduce(|l, r| {
                        let (left_plan, left_variable_to_expr) = l?;
                        let (right_plan, right_variable_to_expr) = r?;
                        self.join(
                            left_plan,
                            left_variable_to_expr,
                            JoinType::Inner,
                            right_plan,
                            right_variable_to_expr,
                            None,
                        )
                    })
                    .unwrap_or_else(|| Ok((LogicalPlanBuilder::empty(true), HashMap::new())))
            }
            GraphPattern::Path { .. } => Err(DataFusionError::NotImplemented(
                "Path patterns are not implemented yet".into(),
            )),
            GraphPattern::Join { left, right } => {
                let (left_plan, left_variable_to_expr) = self.plan_for_graph_pattern(left)?;
                let (right_plan, right_variable_to_expr) = self.plan_for_graph_pattern(right)?;
                self.join(
                    left_plan,
                    left_variable_to_expr,
                    JoinType::Inner,
                    right_plan,
                    right_variable_to_expr,
                    None,
                )
            }
            GraphPattern::Lateral { .. } => Err(DataFusionError::NotImplemented(
                "LATERAL is not implemented yet".into(),
            )),
            GraphPattern::LeftJoin {
                left,
                right,
                expression,
            } => {
                let (left_plan, left_variable_to_expr) = self.plan_for_graph_pattern(left)?;
                let (right_plan, right_variable_to_expr) = self.plan_for_graph_pattern(right)?;
                self.join(
                    left_plan,
                    left_variable_to_expr,
                    JoinType::Left,
                    right_plan,
                    right_variable_to_expr,
                    expression.as_ref(),
                )
            }
            GraphPattern::Filter { inner, expr } => {
                let (inner, variable_to_expr) = self.plan_for_graph_pattern(inner)?;
                Ok((
                    inner.filter(
                        self.effective_boolean_value_expression(expr, &variable_to_expr)?,
                    )?,
                    variable_to_expr,
                ))
            }
            GraphPattern::Union { left, right } => {
                let (left, left_variable_to_expr) = self.plan_for_graph_pattern(left)?;
                let (right, right_variable_to_expr) = self.plan_for_graph_pattern(right)?;
                let mut left_projection = Vec::new();
                let mut right_projection = Vec::new();
                let new_variable_to_expr =
                    left_and_right_expr_for_variable(left_variable_to_expr, right_variable_to_expr)
                        .into_iter()
                        .map(|(variable, (left_expr, right_expr))| {
                            let column_name = column_name(&left_expr)
                                .or_else(|| column_name(&right_expr))
                                .map_or_else(|| self.new_column_name(&variable), ToOwned::to_owned);
                            left_projection.push(
                                alias_if_changed(left_expr.clone(), &column_name)
                                    .cast_to(&DataType::Binary, &left.schema())?,
                            );
                            right_projection.push(
                                alias_if_changed(right_expr.clone(), &column_name)
                                    .cast_to(&DataType::Binary, &right.schema())?,
                            );
                            Ok((variable, col(column_name)))
                        })
                        .collect::<Result<_>>()?;
                // TODO: union_by_name after ensuring all variables are projected
                Ok((
                    left.project(left_projection)?
                        .union(right.project(right_projection)?.build()?)?,
                    new_variable_to_expr,
                ))
            }
            GraphPattern::Graph { .. } => Err(DataFusionError::NotImplemented(
                "GRAPH is not implemented yet".into(),
            )),
            GraphPattern::Extend {
                inner,
                variable,
                expression,
            } => {
                let (plan, mut variable_to_expr) = self.plan_for_graph_pattern(inner)?;
                variable_to_expr.insert(
                    variable.clone(),
                    self.expression(expression, &variable_to_expr)?,
                );
                Ok((plan, variable_to_expr))
            }
            GraphPattern::Minus { left, right } => {
                let (left_plan, left_variable_to_expr) = self.plan_for_graph_pattern(left)?;
                let (right_plan, right_variable_to_expr) = self.plan_for_graph_pattern(right)?;
                self.join(
                    left_plan,
                    left_variable_to_expr,
                    JoinType::LeftAnti,
                    right_plan,
                    right_variable_to_expr,
                    None,
                )
            }
            GraphPattern::Values {
                variables,
                bindings,
            } => {
                let mut variable_to_expr = HashMap::new();
                let plan = if variables.is_empty() || bindings.is_empty() {
                    // Workaround empty values not allowed
                    (0..bindings.len())
                        .map(|_| Ok(LogicalPlanBuilder::empty(true)))
                        .reduce(|l, r| l?.union(r?.build()?))
                        .transpose()?
                        .unwrap_or_else(|| LogicalPlanBuilder::empty(false))
                } else {
                    self.values_table_counter += 1;
                    let table_name = format!("values-{}", self.triple_table_counter);
                    for (i, variable) in variables.iter().enumerate() {
                        variable_to_expr.insert(
                            variable.clone(),
                            Column::new(Some(table_name.clone()), format!("column{}", i + 1))
                                .into(),
                        );
                    }
                    LogicalPlanBuilder::values(
                        bindings
                            .iter()
                            .map(|vs| {
                                vs.iter()
                                    .map(|value| {
                                        value.as_ref().map_or_else(
                                            || Ok(Expr::default()),
                                            |v| self.term_to_expression(v.clone().into()),
                                        )
                                    })
                                    .collect::<Result<Vec<_>>>()
                            })
                            .collect::<Result<Vec<_>>>()?,
                    )?
                    .alias(table_name)?
                };
                Ok((plan, variable_to_expr))
            }
            GraphPattern::OrderBy { .. } => Err(DataFusionError::NotImplemented(
                "ORDER BY is not implemented yet".into(),
            )),
            GraphPattern::Project { inner, variables } => {
                let (inner, variable_to_expr) = self.plan_for_graph_pattern(inner)?;
                let mut new_variable_to_expr = HashMap::new();
                let mut projection = Vec::new();
                for variable in variables {
                    let expr = variable_to_expr.get(variable).cloned().unwrap_or_default();
                    let column = column_name(&expr)
                        .map_or_else(|| self.new_column_name(variable), ToOwned::to_owned);
                    projection.push(alias_if_changed(expr, &column));
                    new_variable_to_expr.insert(variable.clone(), col(column));
                }
                Ok((inner.project(projection)?, new_variable_to_expr))
            }
            GraphPattern::Distinct { inner } => {
                let (inner, variable_to_expr) = self.plan_for_graph_pattern(inner)?;
                if variable_to_expr.is_empty() {
                    // TODO: fix this
                    // Error: Context("Optimizer rule 'replace_distinct_aggregate' failed", Plan("Aggregate requires at least one grouping or aggregate expression"))
                    Err(DataFusionError::NotImplemented(
                        "DISTINCT without variables is not working in DataFusion yet".into(),
                    ))
                } else {
                    Ok((inner.distinct()?, variable_to_expr))
                }
            }
            GraphPattern::Reduced { inner } => self.plan_for_graph_pattern(inner),
            GraphPattern::Slice {
                inner,
                start,
                length,
            } => {
                let (inner, variable_to_expr) = self.plan_for_graph_pattern(inner)?;
                Ok((inner.limit(*start, *length)?, variable_to_expr))
            }
            GraphPattern::Group {
                inner,
                variables,
                aggregates,
            } => {
                let (inner, variable_to_expr) = self.plan_for_graph_pattern(inner)?;
                let mut new_variable_to_expr = HashMap::new();
                let group_expr = variables
                    .iter()
                    .map(|v| {
                        let expr = variable_to_expr.get(v).cloned().unwrap_or_default();
                        new_variable_to_expr.insert(v.clone(), expr.clone());
                        expr.clone()
                    })
                    .collect::<Vec<_>>();
                let aggr_expr = aggregates
                    .iter()
                    .map(|(target_var, expression)| {
                        let intermediate_column = self.new_column_name(target_var);
                        let (aggregate, output_expr) = match expression {
                            AggregateExpression::CountSolutions { distinct } => {
                                if *distinct {
                                    return Err(DataFusionError::NotImplemented(
                                        "COUNT(DISTINCT *) is not implemented yet".into(),
                                    ));
                                }
                                (count_all(), to_rdf_literal(col(&intermediate_column)))
                            }
                            AggregateExpression::FunctionCall {
                                name,
                                expr,
                                distinct,
                            } => {
                                let expression = self.expression(expr, &variable_to_expr)?;
                                match (name, distinct) {
                                    (AggregateFunction::Count, false) => (
                                        count(expression),
                                        to_rdf_literal(col(&intermediate_column)),
                                    ),
                                    (AggregateFunction::Count, true) => (
                                        count_distinct(expression),
                                        to_rdf_literal(col(&intermediate_column)),
                                    ),
                                    _ => {
                                        return Err(DataFusionError::NotImplemented(format!(
                                            "{name} is not implemented yet"
                                        )));
                                    }
                                }
                            }
                        };
                        new_variable_to_expr.insert(target_var.clone(), output_expr);
                        Ok(aggregate.alias(intermediate_column))
                    })
                    .collect::<Result<Vec<_>>>()?;
                Ok((
                    inner.aggregate(group_expr, aggr_expr)?,
                    new_variable_to_expr,
                ))
            }
            GraphPattern::Service { .. } => Err(DataFusionError::NotImplemented(
                "SERVICE is not implemented yet".into(),
            )),
        }
    }

    fn join(
        &self,
        left_plan: LogicalPlanBuilder,
        left_variable_to_expr: HashMap<Variable, Expr>,
        join_type: JoinType,
        right_plan: LogicalPlanBuilder,
        right_variable_to_expr: HashMap<Variable, Expr>,
        filter: Option<&Expression>,
    ) -> Result<(LogicalPlanBuilder, HashMap<Variable, Expr>)> {
        let mut on_exprs = Vec::new();
        if let Some(filter) = filter {
            on_exprs.push(self.effective_boolean_value_expression(filter, &left_variable_to_expr)?);
        }
        let mut new_variable_to_expr = HashMap::new();
        let mut shared_variables_for_minus = Vec::new();
        for (variable, (left_expr, right_expr)) in
            left_and_right_expr_for_variable(left_variable_to_expr, right_variable_to_expr)
        {
            let (left_type, left_nullable) =
                left_expr.data_type_and_nullable(&left_plan.schema())?;
            let (right_type, right_nullable) =
                right_expr.data_type_and_nullable(&right_plan.schema())?;
            let result_expr = if right_type.is_null() {
                left_expr
            } else if left_type.is_null() {
                if join_type == JoinType::LeftAnti {
                    continue; // We don't output right variables
                };
                right_expr
            } else if left_nullable || right_nullable {
                on_exprs.push(
                    left_expr
                        .clone()
                        .eq(right_expr.clone())
                        .or(left_expr.clone().is_null().or(right_expr.clone().is_null())),
                );
                if join_type == JoinType::LeftAnti {
                    shared_variables_for_minus.push((left_expr.clone(), right_expr));
                    left_expr // We don't output right variables
                } else {
                    coalesce(vec![left_expr, right_expr])
                }
            } else {
                if join_type == JoinType::LeftAnti {
                    shared_variables_for_minus.push((left_expr.clone(), right_expr.clone()));
                }
                on_exprs.push(left_expr.clone().eq(right_expr));
                left_expr
            };
            new_variable_to_expr.insert(variable, result_expr);
        }
        if join_type == JoinType::LeftAnti {
            // SPARQL special case: if there is no shared variables we don't match aka we match iff there are shared variables bound
            on_exprs.push(
                shared_variables_for_minus
                    .into_iter()
                    .map(|(l, r)| l.is_not_null().and(r.is_not_null()))
                    .reduce(|l, r| l.or(r))
                    .unwrap_or_else(|| lit(false)),
            )
        }
        if on_exprs.is_empty() && join_type != JoinType::Inner {
            on_exprs.push(lit(true)); // DF requires a filter if join is not an inner join
        }
        Ok((
            left_plan.join_on(right_plan.build()?, join_type, on_exprs)?,
            new_variable_to_expr,
        ))
    }

    fn plan_for_triple_pattern(
        &mut self,
        pattern: &TriplePattern,
    ) -> Result<(LogicalPlanBuilder, HashMap<Variable, Expr>)> {
        let mut filters = Vec::new();
        let mut variable_to_original_column = HashMap::new();
        self.triple_table_counter += 1;
        let table_name = format!("triples-{}", self.triple_table_counter);
        self.term_pattern_to_filter_or_project(
            pattern.subject.clone(),
            "subject",
            &mut filters,
            &mut variable_to_original_column,
        )?;
        self.term_pattern_to_filter_or_project(
            pattern.predicate.clone().into(),
            "predicate",
            &mut filters,
            &mut variable_to_original_column,
        )?;
        self.term_pattern_to_filter_or_project(
            pattern.object.clone(),
            "object",
            &mut filters,
            &mut variable_to_original_column,
        )?;
        let mut plan =
            LogicalPlanBuilder::scan("quads", Arc::clone(&self.quad_table_source), None)?
                .alias(table_name)?;
        if let Some(filters) = filters.into_iter().reduce(and) {
            plan = plan.filter(filters)?;
        }
        let mut projection = Vec::new();
        let variable_to_expr = variable_to_original_column
            .into_iter()
            .map(|(variable, original_column)| {
                let column = self.new_column_name(&variable);
                projection.push(col(original_column).alias(&column));
                (variable, col(column))
            })
            .collect();
        if !projection.is_empty() {
            plan = plan.project(projection)?;
        }
        Ok((plan, variable_to_expr))
    }

    fn term_pattern_to_filter_or_project(
        &mut self,
        pattern: TermPattern,
        column: &'static str,
        filters: &mut Vec<Expr>,
        variable_to_original_column: &mut HashMap<Variable, &'static str>,
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
                filters.push(col(column).eq(self.term_to_expression(t)?));
                Ok(())
            }
            ConstantOrVariable::Variable(v) => {
                match variable_to_original_column.entry(v.clone()) {
                    Entry::Vacant(entry) => {
                        entry.insert(column);
                    }
                    Entry::Occupied(entry) => {
                        filters.push(col(column).eq(col(*entry.get())));
                    }
                }
                Ok(())
            }
        }
    }

    fn effective_boolean_value_expression(
        &self,
        expression: &Expression,
        variable_to_expr: &HashMap<Variable, Expr>,
    ) -> Result<Expr> {
        Ok(match expression {
            Expression::And(left, right) => and(
                self.effective_boolean_value_expression(left, variable_to_expr)?,
                self.effective_boolean_value_expression(right, variable_to_expr)?,
            ),
            Expression::Or(left, right) => or(
                self.effective_boolean_value_expression(left, variable_to_expr)?,
                self.effective_boolean_value_expression(right, variable_to_expr)?,
            ),
            Expression::Not(inner) => {
                not(self.effective_boolean_value_expression(inner, variable_to_expr)?)
            }
            Expression::Bound(v) => variable_to_expr
                .get(v)
                .cloned()
                .unwrap_or_default()
                .is_not_null(),
            Expression::SameTerm(l, r) => self
                .expression(l, variable_to_expr)?
                .eq(self.expression(r, variable_to_expr)?),
            _ => ebv(self.expression(expression, variable_to_expr)?),
        })
    }

    fn expression(
        &self,
        expression: &Expression,
        variable_to_expr: &HashMap<Variable, Expr>,
    ) -> Result<Expr> {
        Ok(match expression {
            Expression::Variable(v) => variable_to_expr.get(v).cloned().unwrap_or_default(),
            Expression::NamedNode(l) => lit(encode_term(
                &self.dataset.internalize_term(l.clone().into())?,
            )),
            Expression::Literal(l) => lit(encode_term(
                &self.dataset.internalize_term(l.clone().into())?,
            )),
            _ => {
                return Err(DataFusionError::NotImplemented(
                    "All expressions are not implemented yet".into(),
                ));
            }
        })
    }

    fn new_column_name(&mut self, variable: &Variable) -> String {
        let counter = self
            .counter_per_variable
            .entry(variable.clone())
            .or_default();
        *counter += 1;
        format!("{}-{}", variable.as_str(), counter)
    }

    fn term_to_expression(&self, term: Term) -> Result<Expr> {
        Ok(lit(encode_term(&self.dataset.internalize_term(term)?)))
    }
}

fn left_and_right_expr_for_variable(
    left_variable_to_expr: HashMap<Variable, Expr>,
    right_variable_to_expr: HashMap<Variable, Expr>,
) -> HashMap<Variable, (Expr, Expr)> {
    let mut result = HashMap::<Variable, (Expr, Expr)>::new();
    for (variable, expr) in left_variable_to_expr {
        result.entry(variable).or_default().0 = expr;
    }
    for (variable, expr) in right_variable_to_expr {
        result.entry(variable).or_default().1 = expr;
    }
    result
}

/// Copy of DataFusion col() function but making sure the relation is None
fn col(name: impl Into<String>) -> Expr {
    Column::from_name(name).into()
}

fn ebv(expr: Expr) -> Expr {
    ScalarUDF::new_from_impl(EffectiveBooleanValue::new()).call(vec![expr])
}

fn to_rdf_literal(expr: Expr) -> Expr {
    ScalarUDF::new_from_impl(ToRdfLiteral::new()).call(vec![expr])
}

fn column_name(expr: &Expr) -> Option<&str> {
    match expr {
        Expr::Column(c) if c.relation.is_none() => Some(&c.name),
        Expr::Alias(a) if a.relation.is_none() => Some(&a.name),
        _ => None,
    }
}

fn alias_if_changed(expr: Expr, alias: impl Into<String>) -> Expr {
    let alias = alias.into();
    if column_name(&expr) == Some(&alias) {
        expr
    } else {
        expr.alias(alias)
    }
}
