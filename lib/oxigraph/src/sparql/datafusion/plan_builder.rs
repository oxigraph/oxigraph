use crate::sparql::datafusion::functions::{EffectiveBooleanValue, ToRdfLiteral};
use crate::sparql::datafusion::table::QuadTableProvider;
use crate::sparql::dataset::DatasetView;
use crate::storage::binary_encoder::encode_term;
use datafusion::arrow::datatypes::Field;
use datafusion::common::{
    Column, DFSchema, DFSchemaRef, DataFusionError, JoinType, Result, TableReference,
};
use datafusion::datasource::DefaultTableSource;
use datafusion::functions::expr_fn::coalesce;
use datafusion::functions_aggregate::count::{count, count_all, count_distinct};
use datafusion::logical_expr::{
    Expr, LogicalPlanBuilder, ScalarUDF, TableSource, and, lit, not, or,
};
use oxrdf::{BlankNode, Term, Variable};
use spareval::QueryableDataset;
use spargebra::algebra::{AggregateExpression, AggregateFunction, Expression, GraphPattern};
use spargebra::term::{TermPattern, TriplePattern};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::iter::once;
use std::sync::Arc;

pub struct SparqlPlanBuilder {
    dataset: Arc<DatasetView<'static>>,
    quad_table_source: Arc<dyn TableSource>,
    blank_node_to_variable: HashMap<BlankNode, Variable>,
    table_counter: usize,
}

impl SparqlPlanBuilder {
    pub fn new(dataset: Arc<DatasetView<'static>>) -> Self {
        Self {
            dataset: Arc::clone(&dataset),
            quad_table_source: Arc::new(DefaultTableSource::new(Arc::new(QuadTableProvider::new(
                dataset,
            )))),
            blank_node_to_variable: HashMap::new(),
            table_counter: 0,
        }
    }

    pub fn build_plan_for_graph_pattern(
        &mut self,
        pattern: &GraphPattern,
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
                                            |v| self.term_to_expression(v.clone().into()),
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
                    .map(|p| self.plan_for_triple_pattern(p))
                    .collect::<Vec<_>>();
                plans
                    .into_iter()
                    .reduce(|left, right| self.new_join(left?, JoinType::Inner, right?, None))
                    .unwrap_or_else(|| Ok(LogicalPlanBuilder::empty(true)))
            }
            GraphPattern::Path { .. } => Err(DataFusionError::NotImplemented(
                "Path patterns are not implemented yet".into(),
            )),
            GraphPattern::Graph { .. } => Err(DataFusionError::NotImplemented(
                "GRAPH is not implemented yet".into(),
            )),
            GraphPattern::Join { left, right } => {
                let left = self.build_plan_for_graph_pattern(left)?;
                let right = self.build_plan_for_graph_pattern(right)?;
                self.new_join(left, JoinType::Inner, right, None)
            }
            GraphPattern::LeftJoin {
                left,
                right,
                expression,
            } => {
                let left = self.build_plan_for_graph_pattern(left)?;
                let right = self.build_plan_for_graph_pattern(right)?;
                self.new_join(left, JoinType::Left, right, expression.as_ref())
            }
            GraphPattern::Minus { left, right } => {
                let left = self.build_plan_for_graph_pattern(left)?;
                let right = self.build_plan_for_graph_pattern(right)?;
                self.new_join(left, JoinType::LeftAnti, right, None)
            }
            GraphPattern::Union { left, right } => {
                let left = self.build_plan_for_graph_pattern(left)?;
                let right = self.build_plan_for_graph_pattern(right)?;
                left.union_by_name(right.build()?)
            }
            GraphPattern::Filter { inner, expr } => {
                let plan = self.build_plan_for_graph_pattern(inner)?;
                let filter = self.effective_boolean_value_expression(expr, plan.schema())?;
                plan.filter(filter)
            }
            GraphPattern::Extend {
                inner,
                variable,
                expression,
            } => {
                let plan = self.build_plan_for_graph_pattern(inner)?;
                let projection = plan
                    .schema()
                    .iter()
                    .map(|field| Column::from(field).into())
                    .chain(once(
                        self.expression(expression, plan.schema())?
                            .alias(variable.as_str()),
                    ))
                    .collect::<Vec<_>>();
                plan.project(projection)
            }
            GraphPattern::Lateral { .. } => Err(DataFusionError::NotImplemented(
                "LATERAL is not implemented yet".into(),
            )),
            GraphPattern::Group {
                inner,
                variables,
                aggregates,
            } => {
                let plan = self.build_plan_for_graph_pattern(inner)?;
                let mut projection = Vec::new();
                let group_expr = variables
                    .iter()
                    .map(|v| {
                        let column = Column::from_name(v.as_str());
                        projection.push(column.clone().into());
                        column
                    })
                    .collect::<Vec<_>>();
                let aggr_expr = aggregates
                    .iter()
                    .map(|(target_var, expression)| {
                        let aggregate = match expression {
                            AggregateExpression::CountSolutions { distinct } => {
                                if *distinct {
                                    return Err(DataFusionError::NotImplemented(
                                        "COUNT(DISTINCT *) is not implemented yet".into(),
                                    ));
                                }
                                count_all()
                            }
                            AggregateExpression::FunctionCall {
                                name,
                                expr,
                                distinct,
                            } => {
                                let expression = self.expression(expr, plan.schema())?;
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
                let plan = self.build_plan_for_graph_pattern(inner)?;
                plan.project(variables.iter().map(|v| Column::from_name(v.as_str())))
            }
            GraphPattern::OrderBy { .. } => Err(DataFusionError::NotImplemented(
                "ORDER BY is not implemented yet".into(),
            )),
            GraphPattern::Distinct { inner } => {
                let plan = self.build_plan_for_graph_pattern(inner)?;
                if plan.schema().fields().is_empty() {
                    // TODO: fix this
                    // Error: Context("Optimizer rule 'replace_distinct_aggregate' failed", Plan("Aggregate requires at least one grouping or aggregate expression"))
                    return Err(DataFusionError::NotImplemented(
                        "DISTINCT without variables is not working in DataFusion yet".into(),
                    ));
                }
                self.build_plan_for_graph_pattern(inner)?.distinct()
            }
            GraphPattern::Reduced { inner } => self.build_plan_for_graph_pattern(inner),
            GraphPattern::Slice {
                inner,
                start,
                length,
            } => self
                .build_plan_for_graph_pattern(inner)?
                .limit(*start, *length),
            GraphPattern::Service { .. } => Err(DataFusionError::NotImplemented(
                "SERVICE is not implemented yet".into(),
            )),
        }
    }

    fn new_join(
        &mut self,
        left_plan: LogicalPlanBuilder,
        join_type: JoinType,
        right_plan: LogicalPlanBuilder,
        filter: Option<&Expression>,
    ) -> Result<LogicalPlanBuilder> {
        let left_plan = left_plan.alias(self.new_table_name())?;
        let right_plan = right_plan.alias(self.new_table_name())?;
        let mut on_exprs = Vec::new();
        if let Some(filter) = filter {
            let joint_schema = left_plan.schema().join(right_plan.schema())?;
            on_exprs.push(self.effective_boolean_value_expression(filter, &joint_schema)?);
        }
        let mut shared_variables_for_minus = (join_type == JoinType::LeftAnti).then(Vec::new);
        let projection = left_and_right_fields_by_name(left_plan.schema(), right_plan.schema())
            .into_iter()
            .filter_map(
                |(column, (left_entry, right_entry))| match (left_entry, right_entry) {
                    (None, None) => None,
                    (Some(field), None) | (None, Some(field)) => Some(Column::from(field).into()),
                    (Some((left_table_ref, left_field)), Some((right_table_ref, right_field))) => {
                        let left_expr = Expr::from(Column::from((left_table_ref, left_field)));
                        let right_expr = Expr::from(Column::from((right_table_ref, right_field)));
                        on_exprs.push(
                            left_expr
                                .clone()
                                .eq(right_expr.clone())
                                .or(left_expr.clone().is_null().or(right_expr.clone().is_null())),
                        );
                        if join_type == JoinType::LeftAnti {
                            if let Some(shared_variables_for_minus) =
                                &mut shared_variables_for_minus
                            {
                                shared_variables_for_minus.push((left_expr.clone(), right_expr));
                            }
                            Some(left_expr) // We don't output right variables
                        } else {
                            Some(coalesce(vec![left_expr, right_expr]).alias(column))
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

    fn plan_for_triple_pattern(&mut self, pattern: &TriplePattern) -> Result<LogicalPlanBuilder> {
        let mut filters = Vec::new();
        let mut variable_to_original_column = HashMap::new();
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
            LogicalPlanBuilder::scan("quads", Arc::clone(&self.quad_table_source), None)?;
        if let Some(filters) = filters.into_iter().reduce(and) {
            plan = plan.filter(filters)?;
        }
        plan.project(
            variable_to_original_column
                .into_iter()
                .map(|(variable, original_column)| {
                    let column = variable.as_str().to_owned();
                    Expr::from(Column::from_name(original_column)).alias(&column)
                }),
        )
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
                filters.push(Expr::from(Column::from_name(column)).eq(self.term_to_expression(t)?));
                Ok(())
            }
            ConstantOrVariable::Variable(v) => {
                match variable_to_original_column.entry(v.clone()) {
                    Entry::Vacant(entry) => {
                        entry.insert(column);
                    }
                    Entry::Occupied(entry) => {
                        filters.push(
                            Expr::from(Column::from_name(column))
                                .eq(Expr::from(Column::from_name(*entry.get()))),
                        );
                    }
                }
                Ok(())
            }
        }
    }

    fn effective_boolean_value_expression(
        &self,
        expression: &Expression,
        schema: &DFSchema,
    ) -> Result<Expr> {
        Ok(match expression {
            Expression::And(left, right) => and(
                self.effective_boolean_value_expression(left, schema)?,
                self.effective_boolean_value_expression(right, schema)?,
            ),
            Expression::Or(left, right) => or(
                self.effective_boolean_value_expression(left, schema)?,
                self.effective_boolean_value_expression(right, schema)?,
            ),
            Expression::Not(inner) => not(self.effective_boolean_value_expression(inner, schema)?),
            Expression::Bound(v) => variable_expr(v, schema).is_not_null(),
            Expression::SameTerm(l, r) => {
                self.expression(l, schema)?.eq(self.expression(r, schema)?)
            }
            _ => ebv(self.expression(expression, schema)?),
        })
    }

    fn expression(&self, expression: &Expression, schema: &DFSchema) -> Result<Expr> {
        Ok(match expression {
            Expression::Variable(v) => variable_expr(v, schema),
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

    fn new_table_name(&mut self) -> String {
        self.table_counter += 1;
        format!("t{}", self.table_counter)
    }

    fn term_to_expression(&self, term: Term) -> Result<Expr> {
        Ok(lit(encode_term(&self.dataset.internalize_term(term)?)))
    }
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

fn ebv(expr: Expr) -> Expr {
    ScalarUDF::new_from_impl(EffectiveBooleanValue::new()).call(vec![expr])
}

fn to_rdf_literal(expr: Expr) -> Expr {
    ScalarUDF::new_from_impl(ToRdfLiteral::new()).call(vec![expr])
}

fn variable_expr(variable: &Variable, schema: &DFSchema) -> Expr {
    schema
        .iter()
        .find(|(_, field)| field.name() == variable.as_str())
        .map_or_else(Expr::default, |e| Column::from(e).into())
}
