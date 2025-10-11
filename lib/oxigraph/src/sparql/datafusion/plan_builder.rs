use crate::sparql::datafusion::functions::{EffectiveBooleanValue, ToRdfLiteral};
use crate::sparql::datafusion::table::QuadTableProvider;
use crate::sparql::dataset::DatasetView;
use crate::storage::binary_encoder::encode_term;
use datafusion::arrow::datatypes::{DataType, Field};
use datafusion::common::alias::AliasGenerator;
use datafusion::common::{
    Column, DFSchema, DFSchemaRef, DataFusionError, JoinType, Result, TableReference,
};
use datafusion::datasource::DefaultTableSource;
use datafusion::functions::expr_fn::coalesce;
use datafusion::functions_aggregate::count::{count, count_all, count_distinct};
use datafusion::logical_expr::{
    Expr, LogicalPlanBuilder, ScalarUDF, TableSource, and, exists, lit, not, or,
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
    table_name: AliasGenerator,
}

impl SparqlPlanBuilder {
    pub fn new(dataset: Arc<DatasetView<'static>>) -> Self {
        Self {
            dataset: Arc::clone(&dataset),
            quad_table_source: Arc::new(DefaultTableSource::new(Arc::new(QuadTableProvider::new(
                dataset,
            )))),
            blank_node_to_variable: HashMap::new(),
            table_name: AliasGenerator::new(),
        }
    }

    pub fn plan_for_graph_pattern(
        &mut self,
        pattern: &GraphPattern,
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
                    .map(|p| self.plan_for_triple_pattern(p, external_schema))
                    .collect::<Vec<_>>();
                plans
                    .into_iter()
                    .reduce(|left, right| {
                        self.join(left?, JoinType::Inner, right?, None, external_schema)
                    })
                    .unwrap_or_else(|| Ok(LogicalPlanBuilder::empty(true)))
            }
            GraphPattern::Path { .. } => Err(DataFusionError::NotImplemented(
                "Path patterns are not implemented yet".into(),
            )),
            GraphPattern::Graph { .. } => Err(DataFusionError::NotImplemented(
                "GRAPH is not implemented yet".into(),
            )),
            GraphPattern::Join { left, right } => {
                let left = self.plan_for_graph_pattern(left, external_schema)?;
                let right = self.plan_for_graph_pattern(right, external_schema)?;
                self.join(left, JoinType::Inner, right, None, external_schema)
            }
            GraphPattern::LeftJoin {
                left,
                right,
                expression,
            } => {
                let left = self.plan_for_graph_pattern(left, external_schema)?;
                let right = self.plan_for_graph_pattern(right, external_schema)?;
                self.join(
                    left,
                    JoinType::Left,
                    right,
                    expression.as_ref(),
                    external_schema,
                )
            }
            GraphPattern::Minus { left, right } => {
                let left = self.plan_for_graph_pattern(left, external_schema)?;
                let right = self.plan_for_graph_pattern(right, external_schema)?;
                self.join(left, JoinType::LeftAnti, right, None, external_schema)
            }
            GraphPattern::Lateral { left, right } => {
                let left = self.plan_for_graph_pattern(left, external_schema)?;
                let left = self.ensure_qualified_names(left)?;
                let mut right_external_schema = external_schema.clone();
                for (table_ref, field) in left.schema().iter() {
                    right_external_schema
                        .insert(field.name().into(), Column::from((table_ref, field)));
                }
                let right = self.plan_for_graph_pattern(right, &right_external_schema)?;
                self.join(left, JoinType::Inner, right, None, external_schema)
            }
            GraphPattern::Union { left, right } => {
                let left = self.plan_for_graph_pattern(left, external_schema)?;
                let right = self.plan_for_graph_pattern(right, external_schema)?;
                left.union_by_name(right.build()?)
            }
            GraphPattern::Filter { inner, expr } => {
                let plan = self.plan_for_graph_pattern(inner, external_schema)?;
                let plan = self.ensure_qualified_names(plan)?;
                let filter =
                    self.effective_boolean_value_expression(expr, plan.schema(), external_schema)?;
                plan.filter(filter)
            }
            GraphPattern::Extend {
                inner,
                variable,
                expression,
            } => {
                let plan = self.plan_for_graph_pattern(inner, external_schema)?;
                let plan = self.ensure_qualified_names(plan)?;
                let projection = plan
                    .schema()
                    .iter()
                    .map(|field| Column::from(field).into())
                    .chain(once(
                        self.expression(expression, plan.schema(), external_schema)?
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
                let plan = self.plan_for_graph_pattern(inner, external_schema)?;
                let mut projection = Vec::new();
                let group_expr = variables
                    .iter()
                    .enumerate()
                    .filter(|(i, v)| !variables[i+1..].contains(v)) // We remove duplicates
                    .map(|(_, v)| {
                        let column = Self::variable_expression(v, plan.schema(), external_schema);
                        projection.push(column.clone());
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
                                let expression =
                                    self.expression(expr, plan.schema(), external_schema)?;
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
                let external_schema = external_schema
                    .iter()
                    .filter(|(column, _)| {
                        variables
                            .iter()
                            .any(|variable| *column == variable.as_str())
                    })
                    .map(|(column, expr)| (column.clone(), expr.clone()))
                    .collect();
                let plan = self.plan_for_graph_pattern(inner, &external_schema)?;
                let projection = variables
                    .iter()
                    .map(|v| Self::variable_expression(v, plan.schema(), &external_schema))
                    .collect::<Vec<_>>();
                plan.project(projection)
            }
            GraphPattern::OrderBy { .. } => Err(DataFusionError::NotImplemented(
                "ORDER BY is not implemented yet".into(),
            )),
            GraphPattern::Distinct { inner } => {
                let plan = self.plan_for_graph_pattern(inner, external_schema)?;
                if plan.schema().fields().is_empty() {
                    // TODO: fix this
                    // Error: Context("Optimizer rule 'replace_distinct_aggregate' failed", Plan("Aggregate requires at least one grouping or aggregate expression"))
                    return Err(DataFusionError::NotImplemented(
                        "DISTINCT without variables is not working in DataFusion yet".into(),
                    ));
                }
                self.plan_for_graph_pattern(inner, external_schema)?
                    .distinct()
            }
            GraphPattern::Reduced { inner } => self.plan_for_graph_pattern(inner, external_schema),
            GraphPattern::Slice {
                inner,
                start,
                length,
            } => self
                .plan_for_graph_pattern(inner, external_schema)?
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
                            on_exprs.push(
                                left_expr.clone().eq(right_expr.clone()).or(left_expr
                                    .clone()
                                    .is_null()
                                    .or(right_expr.clone().is_null())),
                            );
                            if join_type == JoinType::LeftAnti {
                                if let Some(shared_variables_for_minus) =
                                    &mut shared_variables_for_minus
                                {
                                    shared_variables_for_minus
                                        .push((left_expr.clone(), right_expr));
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
        external_schema: &HashMap<String, Column>,
    ) -> Result<LogicalPlanBuilder> {
        let mut filters = Vec::new();
        let mut variable_to_original_column = HashMap::new();
        let table_name = self.table_name.next("quads");
        self.term_pattern_to_filter_or_project(
            pattern.subject.clone(),
            Column::new(Some(table_name.clone()), "subject"),
            &mut filters,
            &mut variable_to_original_column,
            external_schema,
        )?;
        self.term_pattern_to_filter_or_project(
            pattern.predicate.clone().into(),
            Column::new(Some(table_name.clone()), "predicate"),
            &mut filters,
            &mut variable_to_original_column,
            external_schema,
        )?;
        self.term_pattern_to_filter_or_project(
            pattern.object.clone(),
            Column::new(Some(table_name.clone()), "object"),
            &mut filters,
            &mut variable_to_original_column,
            external_schema,
        )?;
        let mut plan =
            LogicalPlanBuilder::scan(&table_name, Arc::clone(&self.quad_table_source), None)?;
        if let Some(filters) = filters.into_iter().reduce(and) {
            plan = plan.filter(filters)?;
        }
        plan.project(
            variable_to_original_column
                .into_iter()
                .map(|(variable, original_column)| {
                    Expr::from(original_column).alias(variable.as_str())
                }),
        )
    }

    fn term_pattern_to_filter_or_project(
        &mut self,
        pattern: TermPattern,
        column: Column,
        filters: &mut Vec<Expr>,
        variable_to_original_column: &mut HashMap<Variable, Column>,
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
                filters.push(Expr::from(column.clone()).eq(self.term_to_expression(t)?));
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
                match variable_to_original_column.entry(v.clone()) {
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

    fn effective_boolean_value_expression(
        &mut self,
        expression: &Expression,
        schema: &DFSchema,
        external_schema: &HashMap<String, Column>,
    ) -> Result<Expr> {
        Ok(match expression {
            Expression::And(left, right) => and(
                self.effective_boolean_value_expression(left, schema, external_schema)?,
                self.effective_boolean_value_expression(right, schema, external_schema)?,
            ),
            Expression::Or(left, right) => or(
                self.effective_boolean_value_expression(left, schema, external_schema)?,
                self.effective_boolean_value_expression(right, schema, external_schema)?,
            ),
            Expression::Not(inner) => {
                not(self.effective_boolean_value_expression(inner, schema, external_schema)?)
            }
            Expression::Bound(v) => {
                Self::variable_expression(v, schema, external_schema).is_not_null()
            }
            Expression::SameTerm(l, r) => self
                .expression(l, schema, external_schema)?
                .eq(self.expression(r, schema, external_schema)?),
            Expression::Exists(p) => {
                let mut external_schema = external_schema.clone();
                for (table_ref, field) in schema.iter() {
                    external_schema.insert(field.name().into(), Column::from((table_ref, field)));
                }
                exists(Arc::new(
                    self.plan_for_graph_pattern(p, &external_schema)?.build()?,
                ))
            }
            _ => ebv(self.expression(expression, schema, external_schema)?),
        })
    }

    fn expression(
        &self,
        expression: &Expression,
        schema: &DFSchema,
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

    fn term_to_expression(&self, term: Term) -> Result<Expr> {
        Ok(lit(encode_term(&self.dataset.internalize_term(term)?)))
    }
}

fn schema_column(schema: &DFSchema, column: &str) -> Option<Column> {
    Some(Column::from(
        schema.iter().find(|(_, field)| field.name() == column)?,
    ))
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
