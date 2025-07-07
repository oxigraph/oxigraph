use crate::sparql::datafusion::NULL;
use crate::sparql::datafusion::functions::EffectiveBooleanValue;
use crate::sparql::datafusion::table::QuadTableProvider;
use crate::sparql::dataset::DatasetView;
use crate::storage::binary_encoder::encode_term;
use datafusion::common::{Column, DataFusionError, JoinType};
use datafusion::datasource::DefaultTableSource;
use datafusion::logical_expr::{
    Expr, LogicalPlan, LogicalPlanBuilder, ScalarUDF, TableSource, and, lit, not, or,
};
use oxrdf::{BlankNode, Term, Variable};
use spareval::QueryableDataset;
use spargebra::algebra::{Expression, GraphPattern};
use spargebra::term::{TermPattern, TriplePattern};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::iter::once;
use std::sync::Arc;

pub struct PlanBuilder {
    dataset: Arc<DatasetView<'static>>,
    quad_table_source: Arc<dyn TableSource>,
    variable_space_counter: u64,
    blank_node_to_variable: HashMap<BlankNode, Variable>,
}

impl PlanBuilder {
    pub fn new(dataset: Arc<DatasetView<'static>>) -> Self {
        Self {
            dataset: Arc::clone(&dataset),
            quad_table_source: Arc::new(DefaultTableSource::new(Arc::new(QuadTableProvider::new(
                dataset,
            )))),
            variable_space_counter: 0,
            blank_node_to_variable: HashMap::new(),
        }
    }

    pub fn build_plan_for_graph_pattern(
        &mut self,
        pattern: &GraphPattern,
        limit: Option<usize>,
    ) -> datafusion::common::Result<LogicalPlan> {
        let (plan, variable_mapping) = self.plan_for_graph_pattern(pattern)?;
        let mut plan = plan.project(
            variable_mapping
                .into_iter()
                .map(|(to, from)| col(from).alias(to.as_str())),
        )?;
        if let Some(limit) = limit {
            plan = plan.limit(0, Some(limit))?;
        }
        plan.build()
    }

    fn plan_for_graph_pattern(
        &mut self,
        pattern: &GraphPattern,
    ) -> datafusion::common::Result<(LogicalPlanBuilder, HashMap<Variable, String>)> {
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
            GraphPattern::Filter { inner, expr } => {
                let (inner, variables_mapping) = self.plan_for_graph_pattern(inner)?;
                Ok((
                    inner.filter(
                        self.effective_boolean_value_expression(expr, &variables_mapping)?,
                    )?,
                    variables_mapping,
                ))
            }
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
            GraphPattern::Extend {
                inner,
                variable,
                expression,
            } => {
                let (inner, mut variables_mapping) = self.plan_for_graph_pattern(inner)?;
                let expr = self.expression(expression, &variables_mapping)?;
                self.variable_space_counter += 1;
                let var = format!("{}-{}", variable.as_str(), self.variable_space_counter);
                let plan = inner.project(
                    variables_mapping
                        .values()
                        .map(col)
                        .chain(once(expr.alias(&var))),
                )?;
                debug_assert!(
                    !variables_mapping.contains_key(variable),
                    "BIND variable already in scope"
                );
                variables_mapping.insert(variable.clone(), var);
                Ok((plan, variables_mapping))
            }
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
                let (inner, mut variables_mapping) = self.plan_for_graph_pattern(inner)?;
                self.variable_space_counter += 1; // In case we generate variable names
                let plan =
                    inner.project(variables.iter().map(
                        |v| match variables_mapping.entry(v.clone()) {
                            Entry::Vacant(entry) => {
                                let var = format!("{}-{}", v.as_str(), self.variable_space_counter);
                                entry.insert_entry(var.clone());
                                NULL.alias(var)
                            }
                            Entry::Occupied(entry) => col(entry.get()),
                        },
                    ))?;
                let variables_mapping = variables_mapping
                    .into_iter()
                    .filter(|(v, _)| variables.contains(v))
                    .collect();
                Ok((plan, variables_mapping))
            }
            GraphPattern::Distinct { inner } => {
                let (inner, variables_mapping) = self.plan_for_graph_pattern(inner)?;
                if variables_mapping.is_empty() {
                    // TODO: fix this
                    // Error: Context("Optimizer rule 'replace_distinct_aggregate' failed", Plan("Aggregate requires at least one grouping or aggregate expression"))
                    Err(DataFusionError::NotImplemented(
                        "DISTINCT without variables is not working in DataFusion yet".into(),
                    ))
                } else {
                    Ok((inner.distinct()?, variables_mapping))
                }
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
    ) -> datafusion::common::Result<(LogicalPlanBuilder, HashMap<Variable, String>)> {
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
    ) -> datafusion::common::Result<(LogicalPlanBuilder, HashMap<Variable, String>)> {
        let mut filters = Vec::new();
        let mut projects = Vec::new();
        let mut variables_mapping = HashMap::new();
        self.variable_space_counter += 1;
        let table_name = format!("triples-{}", self.variable_space_counter);
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
    ) -> datafusion::common::Result<()> {
        match pattern {
            TermPattern::NamedNode(n) => {
                filters.push(self.column_with_term_eq(column, n)?);
                Ok(())
            }
            TermPattern::BlankNode(n) => {
                let v = self
                    .blank_node_to_variable
                    .entry(n.clone())
                    .or_insert_with(|| Variable::new_unchecked(n.to_string()))
                    .clone();
                projects.push(self.column_as_var(column, v, variables_mapping));
                Ok(())
            }
            TermPattern::Literal(l) => {
                filters.push(self.column_with_term_eq(column, l)?);
                Ok(())
            }
            #[cfg(feature = "rdf-12")]
            TermPattern::Triple(_) => Err(DataFusionError::NotImplemented(
                "RDF 1.2 triple terms are not implemented yet".into(),
            )),
            TermPattern::Variable(v) => {
                projects.push(self.column_as_var(column, v, variables_mapping));
                Ok(())
            }
        }
    }

    fn column_with_term_eq(
        &self,
        column: &'static str,
        term: impl Into<Term>,
    ) -> datafusion::common::Result<Expr> {
        Ok(col(column).eq(lit(encode_term(
            &self.dataset.internalize_term(term.into())?,
        ))))
    }

    fn column_as_var(
        &self,
        column: &'static str,
        variable: Variable,
        variables_mapping: &mut HashMap<Variable, String>,
    ) -> Expr {
        let var = format!("{}-{}", variable.as_str(), self.variable_space_counter);
        variables_mapping.insert(variable, var.clone());
        col(column).alias(var)
    }

    fn effective_boolean_value_expression(
        &self,
        expression: &Expression,
        variables_mapping: &HashMap<Variable, String>,
    ) -> datafusion::common::Result<Expr> {
        Ok(match expression {
            Expression::And(left, right) => and(
                self.effective_boolean_value_expression(left, variables_mapping)?,
                self.effective_boolean_value_expression(right, variables_mapping)?,
            ),
            Expression::Or(left, right) => or(
                self.effective_boolean_value_expression(left, variables_mapping)?,
                self.effective_boolean_value_expression(right, variables_mapping)?,
            ),
            Expression::Not(inner) => {
                not(self.effective_boolean_value_expression(inner, variables_mapping)?)
            }
            _ => ScalarUDF::new_from_impl(EffectiveBooleanValue::new())
                .call(vec![self.expression(expression, variables_mapping)?]),
        })
    }

    fn expression(
        &self,
        expression: &Expression,
        variables_mapping: &HashMap<Variable, String>,
    ) -> datafusion::common::Result<Expr> {
        Ok(match expression {
            Expression::Variable(v) => variables_mapping.get(v).map_or(NULL, col),
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
}

/// Copy of DataFusion col() function but making sure the relation is None
fn col(name: impl Into<String>) -> Expr {
    Column::from_name(name).into()
}
