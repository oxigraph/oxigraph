use crate::algebra::{
    AggregateExpression, AggregateFunction, Expression, Function, GraphPattern, GraphTarget,
    OrderExpression, PropertyPathExpression, QueryDataset,
};
use crate::ast;
use crate::error::AlgebraBuilderError;
use crate::query::{AskQuery, ConstructQuery, DescribeQuery, Query, SelectQuery};
#[cfg(feature = "sparql-12")]
use crate::term::GroundTriple;
use crate::term::{
    GraphName, GraphNamePattern, GroundQuad, GroundQuadPattern, GroundTerm, NamedNodePattern, Quad,
    QuadPattern, TermPattern, TriplePattern,
};
use crate::update::{
    ClearOperation, CreateOperation, DeleteDataOperation, DeleteInsertOperation, DropOperation,
    InsertDataOperation, LoadOperation, Update,
};
use chumsky::span::{SimpleSpan, Span, Spanned, WrappingSpan};
use oxiri::Iri;
#[cfg(feature = "sparql-12")]
use oxrdf::BaseDirection;
use oxrdf::vocab::{rdf, xsd};
use oxrdf::{BlankNode, Literal, NamedNode, Variable};
use rand::random;
use std::cmp::{max, min};
use std::collections::{HashMap, HashSet};
use std::mem::take;
use std::ops::RangeInclusive;

pub struct AlgebraBuilder<'a> {
    base_iri: Option<Iri<String>>,
    prefixes: HashMap<String, String>,
    custom_aggregate_functions: &'a HashSet<NamedNode>,
}

impl<'a> AlgebraBuilder<'a> {
    pub fn new(
        base_iri: Option<Iri<String>>,
        prefixes: HashMap<String, String>,
        custom_aggregate_functions: &'a HashSet<NamedNode>,
    ) -> Self {
        Self {
            base_iri,
            prefixes,
            custom_aggregate_functions,
        }
    }

    pub fn build_query(mut self, query: ast::Query<'_>) -> Result<Query, AlgebraBuilderError> {
        self.apply_prologue(query.prologue)?;
        Ok(match query.variant {
            ast::QueryQuery::Select(select) => {
                self.build_select_query(select, query.values_clause)?.into()
            }
            ast::QueryQuery::Construct(construct) => self
                .build_construct_query(construct, query.values_clause)?
                .into(),
            ast::QueryQuery::Describe(describe) => self
                .build_describe_query(describe, query.values_clause)?
                .into(),
            ast::QueryQuery::Ask(ask) => self.build_ask_query(ask, query.values_clause)?.into(),
        })
    }

    fn build_select_query(
        self,
        query: ast::SelectQuery<'_>,
        values_clause: Option<ast::ValuesClause<'_>>,
    ) -> Result<SelectQuery, AlgebraBuilderError> {
        Ok(SelectQuery {
            dataset: self.build_dataset(query.dataset_clause)?,
            pattern: self.build_select(
                query.select_clause,
                query.where_clause,
                query.solution_modifier,
                values_clause,
                true,
            )?,
            base_iri: self.base_iri,
        })
    }

    fn build_construct_query(
        self,
        query: ast::ConstructQuery<'_>,
        values_clause: Option<ast::ValuesClause<'_>>,
    ) -> Result<ConstructQuery, AlgebraBuilderError> {
        let where_clause = query.where_clause.unwrap_or_else(|| {
            ast::GraphPattern::Group(vec![
                query
                    .template
                    .span
                    .make_wrapped(ast::GraphPatternElement::Triples(
                        query
                            .template
                            .inner
                            .clone()
                            .into_iter()
                            .map(|(s, pos)| {
                                (
                                    s.into(),
                                    pos.into_iter()
                                        .map(|(p, os)| {
                                            (p.into(), os.into_iter().map(Into::into).collect())
                                        })
                                        .collect(),
                                )
                            })
                            .collect(),
                    )),
            ])
        });
        let template = self.build_triple_template(query.template.inner)?;
        Ok(ConstructQuery {
            template: template.clone(),
            dataset: self.build_dataset(query.dataset_clause)?,
            pattern: self.build_select(
                ast::SelectClause {
                    option: ast::SelectionOption::Default,
                    bindings: SimpleSpan::new((), 0..0).make_wrapped(ast::SelectVariables::Star),
                },
                where_clause,
                query.solution_modifier,
                values_clause,
                false,
            )?,
            base_iri: self.base_iri,
        })
    }

    fn build_describe_query(
        self,
        query: ast::DescribeQuery<'_>,
        values_clause: Option<ast::ValuesClause<'_>>,
    ) -> Result<DescribeQuery, AlgebraBuilderError> {
        let mut pattern = self.build_select(
            ast::SelectClause {
                option: ast::SelectionOption::Default,
                bindings: query.targets.span.make_wrapped(match &query.targets.inner {
                    ast::DescribeTargets::Star => ast::SelectVariables::Star,
                    ast::DescribeTargets::Explicit(targets) => ast::SelectVariables::Explicit(
                        targets
                            .iter()
                            .filter_map(|var_or_iri| {
                                if let ast::VarOrIri::Var(v) = var_or_iri.inner {
                                    Some(var_or_iri.span.make_wrapped((None, v)))
                                } else {
                                    None
                                }
                            })
                            .collect(),
                    ),
                }),
            },
            query
                .where_clause
                .unwrap_or_else(|| ast::GraphPattern::Group(Vec::new())),
            query.solution_modifier,
            values_clause,
            false,
        )?;
        // We add the IRIS
        let mut counter = 0;
        if let ast::DescribeTargets::Explicit(targets) = query.targets.inner {
            for target in targets {
                // We generate a variable
                let variable = loop {
                    counter += 1;
                    let variable = Variable::new_unchecked(format!("v{counter}"));
                    // We look for name conflicts
                    let mut found_conflict = false;
                    pattern.on_in_scope_variable(|v| {
                        found_conflict |= *v == variable;
                    });
                    if !found_conflict {
                        break variable;
                    }
                };
                if let ast::VarOrIri::Iri(target) = target.inner {
                    pattern = GraphPattern::Extend {
                        inner: Box::new(pattern),
                        variable,
                        expression: self.build_named_node(target)?.into(),
                    }
                }
            }
        }
        Ok(DescribeQuery {
            dataset: self.build_dataset(query.dataset_clause)?,
            pattern,
            base_iri: self.base_iri,
        })
    }

    fn build_ask_query(
        self,
        query: ast::AskQuery<'_>,
        values_clause: Option<ast::ValuesClause<'_>>,
    ) -> Result<AskQuery, AlgebraBuilderError> {
        Ok(AskQuery {
            dataset: self.build_dataset(query.dataset_clause)?,
            pattern: self.build_select(
                ast::SelectClause {
                    option: ast::SelectionOption::Default,
                    bindings: SimpleSpan::new((), 0..0).make_wrapped(ast::SelectVariables::Star),
                },
                query.where_clause,
                query.solution_modifier,
                values_clause,
                false,
            )?,
            base_iri: self.base_iri,
        })
    }

    fn apply_prologue(
        &mut self,
        prologue: Vec<ast::PrologueDecl<'_>>,
    ) -> Result<(), AlgebraBuilderError> {
        for decl in prologue {
            self.apply_prologue_decl(decl)?;
        }
        Ok(())
    }

    fn apply_prologue_decl(
        &mut self,
        decl: ast::PrologueDecl<'_>,
    ) -> Result<(), AlgebraBuilderError> {
        match decl {
            ast::PrologueDecl::Base(base_iri) => {
                self.base_iri = Some(self.build_iri(base_iri)?);
            }
            ast::PrologueDecl::Prefix(prefix, iri) => {
                self.prefixes
                    .insert(prefix.into(), self.build_iri(iri)?.into_inner());
            }
            #[cfg(feature = "sparql-12")]
            ast::PrologueDecl::Version(_) => (),
        }
        Ok(())
    }

    fn build_dataset(
        &self,
        clauses: Vec<ast::GraphClause<'_>>,
    ) -> Result<Option<QueryDataset>, AlgebraBuilderError> {
        if clauses.is_empty() {
            return Ok(None);
        }
        let mut default = Vec::new();
        let mut named = Vec::new();
        for clause in clauses {
            match clause {
                ast::GraphClause::Default(iri) => {
                    default.push(self.build_named_node(iri)?);
                }
                ast::GraphClause::Named(iri) => {
                    named.push(self.build_named_node(iri)?);
                }
            }
        }
        Ok(Some(QueryDataset {
            default,
            named: Some(named),
        }))
    }

    fn build_select(
        &self,
        select_clause: ast::SelectClause<'_>,
        where_clause: ast::GraphPattern<'_>,
        solution_modifier: ast::SolutionModifier<'_>,
        values_clause: Option<ast::ValuesClause<'_>>,
        is_select_explicit: bool,
    ) -> Result<GraphPattern, AlgebraBuilderError> {
        find_graph_pattern_blank_node_ids_and_validate_syntax_restrictions(&where_clause)?;
        let mut p = self.build_graph_pattern(where_clause)?;

        // We build some elements to collect aggregates
        let mut aggregates = Vec::new();

        let select_expressions = match select_clause.bindings.inner {
            ast::SelectVariables::Star => None,
            ast::SelectVariables::Explicit(bindings) => Some(
                bindings
                    .into_iter()
                    .map(|binding| {
                        let (expression, variable) = binding.inner;
                        let variable = Self::build_variable(variable);
                        Ok(binding.span.make_wrapped(
                            if let Some(Spanned {
                                inner: ast::Expression::Aggregate(aggregate),
                                span,
                            }) = expression
                            {
                                aggregates.push((
                                    variable.clone(),
                                    self.build_aggregate(span.make_wrapped(aggregate))?,
                                ));
                                (None, variable)
                            } else {
                                (
                                    expression
                                        .map(|e| self.build_expression(e, &mut aggregates))
                                        .transpose()?,
                                    variable,
                                )
                            },
                        ))
                    })
                    .collect::<Result<Vec<_>, AlgebraBuilderError>>()?,
            ),
        };

        let having_expression = solution_modifier
            .having_clause
            .into_iter()
            .map(|e| self.build_expression(e, &mut aggregates))
            .reduce(|a, b| Ok(Expression::And(Box::new(a?), Box::new(b?))));

        let order_expressions = solution_modifier
            .order_clause
            .into_iter()
            .map(|e| self.build_order_expression(e, &mut aggregates))
            .collect::<Result<Vec<_>, _>>()?;

        // GROUP BY
        let with_aggregate = !solution_modifier.group_clause.is_empty() || !aggregates.is_empty();
        if with_aggregate {
            let mut variables = Vec::new();
            for (expression, variable) in solution_modifier.group_clause {
                let expression_span = expression.span;
                let mut group_by_aggregates = Vec::new();
                let expression = self.build_expression(expression, &mut group_by_aggregates)?;
                if !group_by_aggregates.is_empty() {
                    return Err(AlgebraBuilderError::new(
                        expression_span,
                        "Aggregation functions cannot be used in GROUP BY",
                    ));
                }
                let variable = variable.map(Self::build_variable);
                if let Some(variable) = variable {
                    // Explicit renaming
                    p = GraphPattern::Extend {
                        inner: Box::new(p),
                        variable: variable.clone(),
                        expression,
                    };
                    variables.push(variable);
                } else if let Expression::Variable(variable) = expression {
                    // We can directly use it
                    variables.push(variable);
                } else {
                    // We have to introduce an intermediate variable
                    let variable = random_variable();
                    p = GraphPattern::Extend {
                        inner: Box::new(p),
                        variable: variable.clone(),
                        expression,
                    };
                    variables.push(variable);
                }
            }
            p = GraphPattern::Group {
                inner: Box::new(p),
                variables,
                aggregates,
            };
        }

        // HAVING
        if let Some(expr) = having_expression {
            p = GraphPattern::Filter {
                expr: expr?,
                inner: Box::new(p),
            };
        }

        // VALUES
        if let Some(values_clause) = values_clause {
            p = new_join(p, self.build_values_clause(values_clause)?);
        }

        // SELECT
        let mut projection_variables = Vec::new();
        if let Some(select_expressions) = select_expressions {
            let mut visible = HashSet::new();
            p.on_in_scope_variable(|v| {
                visible.insert(v.clone());
            });
            for binding in select_expressions {
                let (expression, variable) = binding.inner;
                if let Some(expression) = expression {
                    if visible.contains(&variable) {
                        // We disallow to override an existing variable with an expression
                        return Err(AlgebraBuilderError::new(
                            binding.span,
                            format!(
                                "The SELECT overrides {variable} using an expression even if it's already used"
                            ),
                        ));
                    }
                    if with_aggregate {
                        // We validate projection variables if there is an aggregate
                        if let Some(v) = find_unbound_variable(&expression, &visible) {
                            return Err(AlgebraBuilderError::new(
                                binding.span,
                                format!("The variable {v} is unbound in a SELECT expression"),
                            ));
                        }
                    }
                    p = GraphPattern::Extend {
                        inner: Box::new(p),
                        variable: variable.clone(),
                        expression,
                    };
                } else if with_aggregate && !visible.contains(&variable) {
                    // We validate projection variables if there is an aggregate
                    return Err(AlgebraBuilderError::new(
                        binding.span,
                        format!("The SELECT variable {variable} is unbound"),
                    ));
                }
                if projection_variables.contains(&variable) {
                    return Err(AlgebraBuilderError::new(
                        select_clause.bindings.span,
                        format!("{variable} is declared twice in SELECT"),
                    ));
                }
                projection_variables.push(variable)
            }
        } else {
            if with_aggregate && is_select_explicit {
                return Err(AlgebraBuilderError::new(
                    select_clause.bindings.span,
                    "SELECT * is not authorized with GROUP BY",
                ));
            }
            // TODO: is it really useful to always do a projection?
            p.on_in_scope_variable(|v| {
                if !projection_variables.contains(v) {
                    projection_variables.push(v.clone());
                }
            });
            projection_variables.sort();
        }

        let mut m = p;

        // ORDER BY
        if !order_expressions.is_empty() {
            m = GraphPattern::OrderBy {
                inner: Box::new(m),
                expression: order_expressions,
            };
        }

        // PROJECT
        m = GraphPattern::Project {
            inner: Box::new(m),
            variables: projection_variables,
        };
        match select_clause.option {
            ast::SelectionOption::Distinct => m = GraphPattern::Distinct { inner: Box::new(m) },
            ast::SelectionOption::Reduced => m = GraphPattern::Reduced { inner: Box::new(m) },
            ast::SelectionOption::Default => (),
        }

        // OFFSET LIMIT
        if let Some(ast::LimitOffsetClauses { limit, offset }) =
            solution_modifier.limit_offset_clauses
        {
            m = GraphPattern::Slice {
                inner: Box::new(m),
                start: offset,
                length: limit,
            }
        }
        Ok(m)
    }

    fn build_triple_template(
        &self,
        template: Vec<(ast::GraphNode<'_>, ast::PropertyList<'_>)>,
    ) -> Result<Vec<TriplePattern>, AlgebraBuilderError> {
        let mut patterns = Vec::new();
        for (subject, property_list) in template {
            self.build_property_list(
                &self.build_graph_node(subject, &mut patterns)?,
                property_list,
                &mut patterns,
            )?;
        }
        Ok(patterns)
    }

    fn build_values_clause(
        &self,
        values_clause: ast::ValuesClause<'_>,
    ) -> Result<GraphPattern, AlgebraBuilderError> {
        if let Some((vl, vr)) = values_clause
            .variables
            .iter()
            .enumerate()
            .find_map(|(i, vl)| {
                let vr = values_clause.variables[i + 1..]
                    .iter()
                    .find(|vr| vl.inner.0 == vr.inner.0)?;
                Some((vl, vr))
            })
        {
            return Err(AlgebraBuilderError::new(
                SimpleSpan::new((), vl.span.start..vr.span.end),
                format!("Variable {} is repeated, this is not allowed", vl.inner.0),
            ));
        }
        let variables = values_clause
            .variables
            .into_iter()
            .map(Self::build_variable)
            .collect::<Vec<_>>();
        let bindings = values_clause
            .values
            .inner
            .into_iter()
            .map(|binding| {
                binding
                    .into_iter()
                    .map(|value| self.build_ground_term(value))
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        if bindings.iter().any(|vs| vs.len() != variables.len()) {
            return Err(AlgebraBuilderError::new(
                values_clause.values.span,
                "The VALUES clause rows should have exactly the same number of values as there are variables. To set a value to undefined use UNDEF",
            ));
        }
        Ok(GraphPattern::Values {
            variables,
            bindings,
        })
    }

    fn build_ground_term(
        &self,
        data_block_value: ast::DataBlockValue<'_>,
    ) -> Result<Option<GroundTerm>, AlgebraBuilderError> {
        Ok(match data_block_value {
            ast::DataBlockValue::Iri(n) => Some(self.build_named_node(n)?.into()),
            ast::DataBlockValue::Literal(l) => Some(self.build_literal(l)?.into()),
            #[cfg(feature = "sparql-12")]
            ast::DataBlockValue::TripleTerm(t) => Some(self.build_triple_term_data(t)?.into()),
            ast::DataBlockValue::Undef => None,
        })
    }

    #[cfg(feature = "sparql-12")]
    pub fn build_triple_term_data(
        &self,
        t: ast::TripleTermData<'a>,
    ) -> Result<GroundTriple, AlgebraBuilderError> {
        Ok(GroundTriple {
            subject: self.build_named_node(t.subject)?,
            predicate: match t.predicate {
                ast::IriOrA::Iri(p) => self.build_named_node(p)?,
                ast::IriOrA::A => rdf::TYPE.into_owned(),
            },
            object: match t.object {
                ast::TripleTermDataObject::Iri(o) => self.build_named_node(o)?.into(),
                ast::TripleTermDataObject::Literal(o) => self.build_literal(o)?.into(),
                ast::TripleTermDataObject::TripleTerm(o) => self.build_triple_term_data(*o)?.into(),
            },
        })
    }

    fn build_graph_pattern(
        &self,
        graph_pattern: ast::GraphPattern<'_>,
    ) -> Result<GraphPattern, AlgebraBuilderError> {
        Ok(match graph_pattern {
            ast::GraphPattern::SubSelect(sub_select) => self.build_select(
                sub_select.select_clause,
                sub_select.where_clause,
                sub_select.solution_modifier,
                sub_select.values_clause,
                true,
            )?,
            ast::GraphPattern::Group(elements) => {
                let mut g = GraphPattern::default();
                let mut filter: Option<Expression> = None;
                for element in elements {
                    match element.inner {
                        ast::GraphPatternElement::Optional(p) => {
                            let p = self.build_graph_pattern(*p)?;
                            let (right, expression) =
                                if let GraphPattern::Filter { expr, inner } = p {
                                    (inner, Some(expr))
                                } else {
                                    (Box::new(p), None)
                                };
                            g = GraphPattern::LeftJoin {
                                left: Box::new(g),
                                right,
                                expression,
                            }
                        }
                        ast::GraphPatternElement::Minus(p) => {
                            g = GraphPattern::Minus {
                                left: Box::new(g),
                                right: Box::new(self.build_graph_pattern(*p)?),
                            }
                        }
                        ast::GraphPatternElement::Bind(expression, var) => {
                            let variable = Self::build_variable(var);
                            let mut is_variable_overridden = false;
                            g.on_in_scope_variable(|v| {
                                if *v == variable {
                                    is_variable_overridden = true;
                                }
                            });
                            if is_variable_overridden {
                                return Err(AlgebraBuilderError::new(
                                    element.span,
                                    format!(
                                        "{variable} is already in scoped and cannot be overridden by BIND"
                                    ),
                                ));
                            }
                            let mut aggregates = Vec::new();
                            g = GraphPattern::Extend {
                                inner: Box::new(g),
                                variable,
                                expression: self.build_expression(expression, &mut aggregates)?,
                            };
                            if !aggregates.is_empty() {
                                return Err(AlgebraBuilderError::new(
                                    element.span,
                                    "Aggregation functions cannot be used in BIND",
                                ));
                            }
                        }
                        ast::GraphPatternElement::Filter(expr) => {
                            let mut aggregates = Vec::new();
                            let expr = self.build_expression(expr, &mut aggregates)?;
                            if !aggregates.is_empty() {
                                return Err(AlgebraBuilderError::new(
                                    element.span,
                                    "Aggregation functions cannot be used in FILTER",
                                ));
                            }
                            filter = Some(if let Some(f) = filter {
                                Expression::And(Box::new(f), Box::new(expr))
                            } else {
                                expr
                            })
                        }
                        ast::GraphPatternElement::Triples(triples) => {
                            let mut patterns = Vec::new();
                            for (subject, predicate_objects) in triples {
                                self.build_property_list_path(
                                    &self.build_graph_node_path(subject, &mut patterns)?,
                                    predicate_objects,
                                    &mut patterns,
                                )?;
                            }
                            let mut bgp = Vec::new();
                            for pattern in patterns {
                                match pattern {
                                    TripleOrPathPattern::Triple(t) => {
                                        bgp.push(t);
                                    }
                                    TripleOrPathPattern::Path {
                                        subject,
                                        path,
                                        object,
                                    } => {
                                        if !bgp.is_empty() {
                                            g = new_join(
                                                g,
                                                GraphPattern::Bgp {
                                                    patterns: take(&mut bgp),
                                                },
                                            );
                                        }
                                        g = new_join(
                                            g,
                                            GraphPattern::Path {
                                                subject,
                                                path,
                                                object,
                                            },
                                        );
                                    }
                                }
                            }
                            if !bgp.is_empty() {
                                g = new_join(
                                    g,
                                    GraphPattern::Bgp {
                                        patterns: take(&mut bgp),
                                    },
                                );
                            }
                        }
                        ast::GraphPatternElement::Union(elements) => {
                            g = new_join(
                                g,
                                elements
                                    .into_iter()
                                    .map(|e| self.build_graph_pattern(e))
                                    .reduce(|l, r| {
                                        Ok(GraphPattern::Union {
                                            left: Box::new(l?),
                                            right: Box::new(r?),
                                        })
                                    })
                                    .unwrap_or_else(|| Ok(GraphPattern::default()))?,
                            );
                        }
                        ast::GraphPatternElement::Values(values) => {
                            g = new_join(g, self.build_values_clause(values)?);
                        }
                        ast::GraphPatternElement::Service {
                            silent,
                            name,
                            pattern,
                        } => {
                            g = new_join(
                                g,
                                GraphPattern::Service {
                                    name: self.build_named_node_pattern(name)?,
                                    inner: Box::new(self.build_graph_pattern(*pattern)?),
                                    silent,
                                },
                            )
                        }
                        ast::GraphPatternElement::Graph { name, pattern } => {
                            g = new_join(
                                g,
                                GraphPattern::Graph {
                                    name: self.build_named_node_pattern(name)?,
                                    inner: Box::new(self.build_graph_pattern(*pattern)?),
                                },
                            )
                        }
                        #[cfg(feature = "sep-0006")]
                        ast::GraphPatternElement::Lateral(p) => {
                            let p = self.build_graph_pattern(*p)?;
                            let mut defined_variables = HashSet::new();
                            add_defined_variables(&p, &mut defined_variables);
                            let mut overridden_variable = None;
                            g.on_in_scope_variable(|v| {
                                if defined_variables.contains(v) {
                                    overridden_variable = Some(v.clone());
                                }
                            });
                            if let Some(overridden_variable) = overridden_variable {
                                return Err(AlgebraBuilderError::new(
                                    element.span,
                                    format!(
                                        "{overridden_variable} is overridden in the right side of LATERAL"
                                    ),
                                ));
                            }
                            g = GraphPattern::Lateral {
                                left: Box::new(g),
                                right: Box::new(p),
                            }
                        }
                    }
                }

                if let Some(expr) = filter {
                    GraphPattern::Filter {
                        expr,
                        inner: Box::new(g),
                    }
                } else {
                    g
                }
            }
        })
    }

    fn build_order_expression(
        &self,
        expression: ast::OrderCondition<'_>,
        aggregates: &mut Vec<(Variable, AggregateExpression)>,
    ) -> Result<OrderExpression, AlgebraBuilderError> {
        Ok(match expression {
            ast::OrderCondition::Asc(e) => {
                OrderExpression::Asc(self.build_expression(e, aggregates)?)
            }
            ast::OrderCondition::Desc(e) => {
                OrderExpression::Desc(self.build_expression(e, aggregates)?)
            }
        })
    }

    fn build_aggregate(
        &self,
        aggregate: Spanned<ast::Aggregate<'_>>,
    ) -> Result<AggregateExpression, AlgebraBuilderError> {
        let (name, expression, distinct) = match aggregate.inner {
            ast::Aggregate::Count(distinct, expression) => {
                if let Some(expression) = expression {
                    (AggregateFunction::Count, expression, distinct)
                } else {
                    return Ok(AggregateExpression::CountSolutions { distinct });
                }
            }
            ast::Aggregate::Sum(distinct, expression) => {
                (AggregateFunction::Sum, expression, distinct)
            }
            ast::Aggregate::Min(distinct, expression) => {
                (AggregateFunction::Min, expression, distinct)
            }
            ast::Aggregate::Max(distinct, expression) => {
                (AggregateFunction::Max, expression, distinct)
            }
            ast::Aggregate::Avg(distinct, expression) => {
                (AggregateFunction::Avg, expression, distinct)
            }
            ast::Aggregate::Sample(distinct, expression) => {
                (AggregateFunction::Sample, expression, distinct)
            }
            ast::Aggregate::GroupConcat(distinct, expression, separator) => (
                AggregateFunction::GroupConcat {
                    separator: separator.map(Self::build_string).transpose()?,
                },
                expression,
                distinct,
            ),
        };
        let mut nested_aggregates = Vec::new();
        let expr = self.build_expression(*expression, &mut nested_aggregates)?;
        if !nested_aggregates.is_empty() {
            return Err(AlgebraBuilderError::new(
                aggregate.span,
                "Aggregated expressions cannot be nested",
            ));
        }
        Ok(AggregateExpression::FunctionCall {
            name,
            expr,
            distinct,
        })
    }

    fn build_expression(
        &self,
        expression: Spanned<ast::Expression<'_>>,
        aggregates: &mut Vec<(Variable, AggregateExpression)>,
    ) -> Result<Expression, AlgebraBuilderError> {
        Ok(match expression.inner {
            ast::Expression::Or(l, r) => Expression::Or(
                Box::new(self.build_expression(*l, aggregates)?),
                Box::new(self.build_expression(*r, aggregates)?),
            ),
            ast::Expression::And(l, r) => Expression::And(
                Box::new(self.build_expression(*l, aggregates)?),
                Box::new(self.build_expression(*r, aggregates)?),
            ),
            ast::Expression::Equal(l, r) => Expression::Equal(
                Box::new(self.build_expression(*l, aggregates)?),
                Box::new(self.build_expression(*r, aggregates)?),
            ),
            ast::Expression::NotEqual(l, r) => Expression::Not(Box::new(Expression::Equal(
                Box::new(self.build_expression(*l, aggregates)?),
                Box::new(self.build_expression(*r, aggregates)?),
            ))),
            ast::Expression::Less(l, r) => Expression::Less(
                Box::new(self.build_expression(*l, aggregates)?),
                Box::new(self.build_expression(*r, aggregates)?),
            ),
            ast::Expression::LessOrEqual(l, r) => Expression::LessOrEqual(
                Box::new(self.build_expression(*l, aggregates)?),
                Box::new(self.build_expression(*r, aggregates)?),
            ),
            ast::Expression::Greater(l, r) => Expression::Greater(
                Box::new(self.build_expression(*l, aggregates)?),
                Box::new(self.build_expression(*r, aggregates)?),
            ),
            ast::Expression::GreaterOrEqual(l, r) => Expression::GreaterOrEqual(
                Box::new(self.build_expression(*l, aggregates)?),
                Box::new(self.build_expression(*r, aggregates)?),
            ),
            ast::Expression::In(l, r) => Expression::In(
                Box::new(self.build_expression(*l, aggregates)?),
                r.into_iter()
                    .map(|e| self.build_expression(e, aggregates))
                    .collect::<Result<_, _>>()?,
            ),
            ast::Expression::NotIn(l, r) => Expression::Not(Box::new(Expression::In(
                Box::new(self.build_expression(*l, aggregates)?),
                r.into_iter()
                    .map(|e| self.build_expression(e, aggregates))
                    .collect::<Result<_, _>>()?,
            ))),
            ast::Expression::Add(l, r) => Expression::Add(
                Box::new(self.build_expression(*l, aggregates)?),
                Box::new(self.build_expression(*r, aggregates)?),
            ),
            ast::Expression::Subtract(l, r) => Expression::Subtract(
                Box::new(self.build_expression(*l, aggregates)?),
                Box::new(self.build_expression(*r, aggregates)?),
            ),
            ast::Expression::Multiply(l, r) => Expression::Multiply(
                Box::new(self.build_expression(*l, aggregates)?),
                Box::new(self.build_expression(*r, aggregates)?),
            ),
            ast::Expression::Divide(l, r) => Expression::Divide(
                Box::new(self.build_expression(*l, aggregates)?),
                Box::new(self.build_expression(*r, aggregates)?),
            ),
            ast::Expression::UnaryPlus(e) => {
                Expression::UnaryPlus(Box::new(self.build_expression(*e, aggregates)?))
            }
            ast::Expression::UnaryMinus(e) => {
                Expression::UnaryMinus(Box::new(self.build_expression(*e, aggregates)?))
            }
            ast::Expression::Not(e) => {
                Expression::Not(Box::new(self.build_expression(*e, aggregates)?))
            }
            ast::Expression::Bound(v) => Expression::Bound(Self::build_variable(v)),
            ast::Expression::Aggregate(aggregate) => {
                let aggregate = self.build_aggregate(expression.span.make_wrapped(aggregate))?;
                register_aggregate(aggregate, aggregates).into()
            }
            ast::Expression::Iri(n) => Expression::NamedNode(self.build_named_node(n)?),
            ast::Expression::Literal(l) => Expression::Literal(self.build_literal(l)?),
            ast::Expression::Var(v) => Expression::Variable(Self::build_variable(v)),
            #[cfg(feature = "sparql-12")]
            ast::Expression::TripleTerm(t) => self.build_expr_triple_term(t)?,
            ast::Expression::BuiltIn(name, args) => {
                let args = args
                    .into_iter()
                    .map(|e| self.build_expression(e, aggregates))
                    .collect::<Result<_, _>>()?;
                let name = match name {
                    ast::BuiltInName::Coalesce => {
                        return Ok(Expression::Coalesce(args));
                    }
                    ast::BuiltInName::If => {
                        let [a, b, c] = args.try_into().map_err(|_| {
                            AlgebraBuilderError::new(
                                expression.span,
                                "The IF function takes exactly 3 parameters",
                            )
                        })?;
                        return Ok(Expression::If(Box::new(a), Box::new(b), Box::new(c)));
                    }
                    ast::BuiltInName::SameTerm => {
                        let [l, r] = args.try_into().map_err(|_| {
                            AlgebraBuilderError::new(
                                expression.span,
                                "The sameTerm function takes exactly 2 parameters",
                            )
                        })?;
                        return Ok(Expression::SameTerm(Box::new(l), Box::new(r)));
                    }
                    ast::BuiltInName::Str => Function::Str,
                    ast::BuiltInName::Lang => Function::Lang,
                    ast::BuiltInName::LangMatches => Function::LangMatches,
                    ast::BuiltInName::Datatype => Function::Datatype,
                    ast::BuiltInName::Iri | ast::BuiltInName::Uri => Function::Iri,
                    ast::BuiltInName::BNode => Function::BNode,
                    ast::BuiltInName::Rand => Function::Rand,
                    ast::BuiltInName::Abs => Function::Abs,
                    ast::BuiltInName::Ceil => Function::Ceil,
                    ast::BuiltInName::Floor => Function::Floor,
                    ast::BuiltInName::Round => Function::Round,
                    ast::BuiltInName::Concat => Function::Concat,
                    ast::BuiltInName::SubStr => Function::SubStr,
                    ast::BuiltInName::StrLen => Function::StrLen,
                    ast::BuiltInName::Replace => Function::Replace,
                    ast::BuiltInName::UCase => Function::UCase,
                    ast::BuiltInName::LCase => Function::LCase,
                    ast::BuiltInName::EncodeForUri => Function::EncodeForUri,
                    ast::BuiltInName::Contains => Function::Contains,
                    ast::BuiltInName::StrStarts => Function::StrStarts,
                    ast::BuiltInName::StrEnds => Function::StrEnds,
                    ast::BuiltInName::StrBefore => Function::StrBefore,
                    ast::BuiltInName::StrAfter => Function::StrAfter,
                    ast::BuiltInName::Year => Function::Year,
                    ast::BuiltInName::Month => Function::Month,
                    ast::BuiltInName::Day => Function::Day,
                    ast::BuiltInName::Hours => Function::Hours,
                    ast::BuiltInName::Minutes => Function::Minutes,
                    ast::BuiltInName::Seconds => Function::Seconds,
                    ast::BuiltInName::Timezone => Function::Timezone,
                    ast::BuiltInName::Tz => Function::Tz,
                    ast::BuiltInName::Now => Function::Now,
                    ast::BuiltInName::Uuid => Function::Uuid,
                    ast::BuiltInName::StrUuid => Function::StrUuid,
                    ast::BuiltInName::Md5 => Function::Md5,
                    ast::BuiltInName::Sha1 => Function::Sha1,
                    ast::BuiltInName::Sha256 => Function::Sha256,
                    ast::BuiltInName::Sha384 => Function::Sha384,
                    ast::BuiltInName::Sha512 => Function::Sha512,
                    ast::BuiltInName::StrLang => Function::StrLang,
                    ast::BuiltInName::StrDt => Function::StrDt,
                    ast::BuiltInName::IsIri | ast::BuiltInName::IsUri => Function::IsIri,
                    ast::BuiltInName::IsBlank => Function::IsBlank,
                    ast::BuiltInName::IsLiteral => Function::IsLiteral,
                    ast::BuiltInName::IsNumeric => Function::IsNumeric,
                    ast::BuiltInName::Regex => Function::Regex,
                    #[cfg(feature = "sparql-12")]
                    ast::BuiltInName::Triple => Function::Triple,
                    #[cfg(feature = "sparql-12")]
                    ast::BuiltInName::Subject => Function::Subject,
                    #[cfg(feature = "sparql-12")]
                    ast::BuiltInName::Predicate => Function::Predicate,
                    #[cfg(feature = "sparql-12")]
                    ast::BuiltInName::Object => Function::Object,
                    #[cfg(feature = "sparql-12")]
                    ast::BuiltInName::IsTriple => Function::IsTriple,
                    #[cfg(feature = "sparql-12")]
                    ast::BuiltInName::LangDir => Function::LangDir,
                    #[cfg(feature = "sparql-12")]
                    ast::BuiltInName::HasLang => Function::HasLang,
                    #[cfg(feature = "sparql-12")]
                    ast::BuiltInName::HasLangDir => Function::HasLangDir,
                    #[cfg(feature = "sparql-12")]
                    ast::BuiltInName::StrLangDir => Function::StrLangDir,
                    #[cfg(feature = "sep-0002")]
                    ast::BuiltInName::Adjust => Function::Adjust,
                };
                let arity = function_arity(&name);
                if !arity.contains(&args.len()) {
                    return Err(AlgebraBuilderError::new(
                        expression.span,
                        if arity.start() == arity.end() {
                            format!(
                                "{name} is called with {} parameters even if it only supports between {} parameters",
                                args.len(),
                                arity.start(),
                            )
                        } else {
                            format!(
                                "{name} is called with {} parameters even if it only supports between {} and  {} parameters",
                                args.len(),
                                arity.start(),
                                arity.end()
                            )
                        },
                    ));
                }
                Expression::FunctionCall(name, args)
            }
            ast::Expression::Function(name, args) => {
                let name = self.build_named_node(name)?;
                if self.custom_aggregate_functions.contains(&name) {
                    if args.args.len() != 1 {
                        return Err(AlgebraBuilderError::new(
                            expression.span,
                            format!(
                                "Oxigraph only supports aggregate functions with 1 argument, {name} called with {}",
                                args.args.len()
                            ),
                        ));
                    }
                    let mut nested_aggregates = Vec::new();
                    let expr = self.build_expression(
                        args.args.into_iter().next().unwrap(),
                        &mut nested_aggregates,
                    )?;
                    if !nested_aggregates.is_empty() {
                        return Err(AlgebraBuilderError::new(
                            expression.span,
                            "Aggregated expressions cannot be nested",
                        ));
                    }
                    return Ok(register_aggregate(
                        AggregateExpression::FunctionCall {
                            name: AggregateFunction::Custom(name),
                            expr,
                            distinct: args.distinct,
                        },
                        aggregates,
                    )
                    .into());
                }
                if args.distinct {
                    return Err(AlgebraBuilderError::new(
                        expression.span,
                        format!(
                            "{name} is not an aggregate function, it cannot be used with the DISTINCT option"
                        ),
                    ));
                }
                Expression::FunctionCall(
                    Function::Custom(name),
                    args.args
                        .into_iter()
                        .map(|e| self.build_expression(e, aggregates))
                        .collect::<Result<_, _>>()?,
                )
            }
            ast::Expression::Exists(gp) => {
                Expression::Exists(Box::new(self.build_graph_pattern(*gp)?))
            }
            ast::Expression::NotExists(gp) => Expression::Not(Box::new(Expression::Exists(
                Box::new(self.build_graph_pattern(*gp)?),
            ))),
        })
    }

    #[cfg(feature = "sparql-12")]
    fn build_expr_triple_term(
        &self,
        t: ast::ExprTripleTerm<'_>,
    ) -> Result<Expression, AlgebraBuilderError> {
        Ok(Expression::FunctionCall(
            Function::Triple,
            vec![
                match t.subject {
                    ast::ExprTripleTermSubject::Iri(s) => self.build_named_node(s)?.into(),
                    ast::ExprTripleTermSubject::Var(s) => Self::build_variable(s).into(),
                },
                self.build_verb(t.predicate)?.into(),
                match t.object {
                    ast::ExprTripleTermObject::Iri(o) => self.build_named_node(o)?.into(),
                    ast::ExprTripleTermObject::Literal(o) => self.build_literal(o)?.into(),
                    ast::ExprTripleTermObject::Var(o) => Self::build_variable(o).into(),
                    ast::ExprTripleTermObject::TripleTerm(o) => self.build_expr_triple_term(*o)?,
                },
            ],
        ))
    }

    fn build_property_list(
        &self,
        subject: &TermPattern,
        property_list: ast::PropertyList<'_>,
        patterns: &mut Vec<TriplePattern>,
    ) -> Result<(), AlgebraBuilderError> {
        for (predicate, objects) in property_list {
            let predicate = self.build_verb(predicate)?;
            for object in objects {
                let object = self.build_object(
                    #[cfg(feature = "sparql-12")]
                    subject,
                    #[cfg(feature = "sparql-12")]
                    &predicate,
                    object,
                    patterns,
                )?;
                patterns.push(TriplePattern::new(
                    subject.clone(),
                    predicate.clone(),
                    object,
                ));
            }
        }
        Ok(())
    }

    fn build_property_list_path(
        &self,
        subject: &TermPattern,
        property_list: ast::PropertyListPath<'_>,
        patterns: &mut Vec<TripleOrPathPattern>,
    ) -> Result<(), AlgebraBuilderError> {
        for (predicate, objects) in property_list {
            let predicate = self.build_var_or_path(predicate)?;
            for object in objects {
                let object = self.build_object_path(
                    #[cfg(feature = "sparql-12")]
                    subject,
                    #[cfg(feature = "sparql-12")]
                    &predicate,
                    object,
                    patterns,
                )?;
                match predicate.clone() {
                    VarOrPath::Var(predicate) => patterns.push(TripleOrPathPattern::Triple(
                        TriplePattern::new(subject.clone(), predicate, object),
                    )),
                    VarOrPath::Path(path) => {
                        add_path_to_patterns(subject.clone(), path, object, patterns)
                    }
                }
            }
        }
        Ok(())
    }

    fn build_object(
        &self,
        #[cfg(feature = "sparql-12")] subject: &TermPattern,
        #[cfg(feature = "sparql-12")] predicate: &NamedNodePattern,
        object: ast::Object<'_>,
        patterns: &mut Vec<TriplePattern>,
    ) -> Result<TermPattern, AlgebraBuilderError> {
        let object_pattern = self.build_graph_node(object.graph_node, patterns)?;
        #[cfg(feature = "sparql-12")]
        {
            let mut current_reifier = None;
            for annotation in object.annotations {
                let reifier_to_emit = match annotation.inner {
                    ast::Annotation::Reifier(r) => {
                        let reifier_to_emit = current_reifier;
                        current_reifier = Some(if let Some(r) = r {
                            self.build_reifier_id(r)?
                        } else {
                            BlankNode::default().into()
                        });
                        reifier_to_emit
                    }
                    ast::Annotation::AnnotationBlock(a) => {
                        let reifier_to_emit = take(&mut current_reifier)
                            .unwrap_or_else(|| BlankNode::default().into());
                        self.build_property_list(&reifier_to_emit, a, patterns)?;
                        Some(reifier_to_emit)
                    }
                };
                if let Some(reifier) = reifier_to_emit {
                    patterns.push(TriplePattern::new(
                        reifier,
                        rdf::REIFIES.into_owned(),
                        TriplePattern::new(
                            subject.clone(),
                            predicate.clone(),
                            object_pattern.clone(),
                        ),
                    ));
                }
            }
        }
        Ok(object_pattern)
    }

    fn build_object_path(
        &self,
        #[cfg(feature = "sparql-12")] subject: &TermPattern,
        #[cfg(feature = "sparql-12")] predicate: &VarOrPath,
        object_path: ast::ObjectPath<'_>,
        patterns: &mut Vec<TripleOrPathPattern>,
    ) -> Result<TermPattern, AlgebraBuilderError> {
        let object = self.build_graph_node_path(object_path.graph_node, patterns)?;
        #[cfg(feature = "sparql-12")]
        {
            let mut current_reifier = None;
            for annotation in object_path.annotations {
                let reifier_to_emit = match annotation.inner {
                    ast::AnnotationPath::Reifier(r) => {
                        let reifier_to_emit = current_reifier;
                        current_reifier = Some(annotation.span.make_wrapped(if let Some(r) = r {
                            self.build_reifier_id(r)?
                        } else {
                            BlankNode::default().into()
                        }));
                        reifier_to_emit
                    }
                    ast::AnnotationPath::AnnotationBlock(a) => {
                        let reifier_to_emit = take(&mut current_reifier).unwrap_or_else(|| {
                            annotation.span.make_wrapped(BlankNode::default().into())
                        });
                        self.build_property_list_path(&reifier_to_emit.inner, a, patterns)?;
                        Some(reifier_to_emit)
                    }
                };
                if let Some(reifier) = reifier_to_emit {
                    let predicate = match predicate {
                        VarOrPath::Var(predicate) => NamedNodePattern::from(predicate.clone()),
                        VarOrPath::Path(PropertyPathExpression::NamedNode(predicate)) => {
                            predicate.clone().into()
                        }
                        VarOrPath::Path(_) => {
                            return Err(AlgebraBuilderError::new(
                                reifier.span,
                                "Reifiers can only be used on triples and not on property paths",
                            ));
                        }
                    };
                    patterns.push(TripleOrPathPattern::Triple(TriplePattern::new(
                        reifier.inner,
                        rdf::REIFIES.into_owned(),
                        TriplePattern::new(subject.clone(), predicate.clone(), object.clone()),
                    )));
                }
            }
        }
        Ok(object)
    }

    #[cfg(feature = "sparql-12")]
    fn build_reifier_id(
        &self,
        var_or_reifier_id: ast::VarOrReifierId<'_>,
    ) -> Result<TermPattern, AlgebraBuilderError> {
        Ok(match var_or_reifier_id {
            ast::VarOrReifierId::Var(v) => Self::build_variable(v).into(),
            ast::VarOrReifierId::Iri(n) => self.build_named_node(n)?.into(),
            ast::VarOrReifierId::BlankNode(n) => Self::build_blank_node(n).into(),
        })
    }

    fn build_graph_node(
        &self,
        graph_node: ast::GraphNode<'_>,
        patterns: &mut Vec<TriplePattern>,
    ) -> Result<TermPattern, AlgebraBuilderError> {
        match graph_node {
            ast::GraphNode::VarOrTerm(var_or_term) => self.build_term_pattern(var_or_term),
            ast::GraphNode::Collection(elements) => {
                let mut current_list_node = TermPattern::from(rdf::NIL.into_owned());
                for element in elements.inner.into_iter().rev() {
                    let element = self.build_graph_node(element, patterns)?;
                    let new_blank_node = TermPattern::from(BlankNode::default());
                    patterns.push(TriplePattern::new(
                        new_blank_node.clone(),
                        rdf::FIRST.into_owned(),
                        element.clone(),
                    ));
                    patterns.push(TriplePattern::new(
                        new_blank_node.clone(),
                        rdf::REST.into_owned(),
                        current_list_node,
                    ));
                    current_list_node = new_blank_node;
                }
                Ok(current_list_node)
            }
            ast::GraphNode::BlankNodePropertyList(property_list) => {
                let subject = TermPattern::from(BlankNode::default());
                self.build_property_list(&subject, property_list.inner, patterns)?;
                Ok(subject)
            }
            #[cfg(feature = "sparql-12")]
            ast::GraphNode::ReifiedTriple(t) => self.build_reified_triple(t, patterns),
        }
    }

    fn build_graph_node_path(
        &self,
        graph_node_path: ast::GraphNodePath<'_>,
        patterns: &mut Vec<TripleOrPathPattern>,
    ) -> Result<TermPattern, AlgebraBuilderError> {
        match graph_node_path {
            ast::GraphNodePath::VarOrTerm(var_or_term) => self.build_term_pattern(var_or_term),
            ast::GraphNodePath::Collection(elements) => {
                let mut current_list_node = TermPattern::from(rdf::NIL.into_owned());
                for element in elements.inner.into_iter().rev() {
                    let element = self.build_graph_node_path(element, patterns)?;
                    let new_blank_node = TermPattern::from(BlankNode::default());
                    patterns.push(TripleOrPathPattern::Triple(TriplePattern::new(
                        new_blank_node.clone(),
                        rdf::FIRST.into_owned(),
                        element.clone(),
                    )));
                    patterns.push(TripleOrPathPattern::Triple(TriplePattern::new(
                        new_blank_node.clone(),
                        rdf::REST.into_owned(),
                        current_list_node,
                    )));
                    current_list_node = new_blank_node;
                }
                Ok(current_list_node)
            }
            ast::GraphNodePath::BlankNodePropertyList(property_list) => {
                let subject = TermPattern::from(BlankNode::default());
                self.build_property_list_path(&subject, property_list.inner, patterns)?;
                Ok(subject)
            }
            #[cfg(feature = "sparql-12")]
            ast::GraphNodePath::ReifiedTriple(t) => {
                let mut extra_patterns = Vec::new();
                let term = self.build_reified_triple(t, &mut extra_patterns)?;
                patterns.extend(extra_patterns.into_iter().map(TripleOrPathPattern::Triple));
                Ok(term)
            }
        }
    }

    fn build_var_or_path(
        &self,
        var_or_path: ast::VarOrPath<'_>,
    ) -> Result<VarOrPath, AlgebraBuilderError> {
        Ok(match var_or_path {
            ast::VarOrPath::Var(v) => VarOrPath::Var(Self::build_variable(v)),
            ast::VarOrPath::Path(p) => VarOrPath::Path(self.build_path(p)?),
        })
    }

    fn build_path(
        &self,
        path: ast::Path<'_>,
    ) -> Result<PropertyPathExpression, AlgebraBuilderError> {
        Ok(match path {
            ast::Path::Alternative(l, r) => PropertyPathExpression::Alternative(
                Box::new(self.build_path(*l)?),
                Box::new(self.build_path(*r)?),
            ),
            ast::Path::Sequence(l, r) => PropertyPathExpression::Sequence(
                Box::new(self.build_path(*l)?),
                Box::new(self.build_path(*r)?),
            ),
            ast::Path::Inverse(p) => {
                PropertyPathExpression::Reverse(Box::new(self.build_path(*p)?))
            }
            ast::Path::ZeroOrOne(p) => {
                PropertyPathExpression::ZeroOrOne(Box::new(self.build_path(*p)?))
            }
            ast::Path::ZeroOrMore(p) => {
                PropertyPathExpression::ZeroOrMore(Box::new(self.build_path(*p)?))
            }
            ast::Path::OneOrMore(p) => {
                PropertyPathExpression::OneOrMore(Box::new(self.build_path(*p)?))
            }
            ast::Path::Iri(p) => PropertyPathExpression::NamedNode(self.build_named_node(p)?),
            ast::Path::A => PropertyPathExpression::NamedNode(rdf::TYPE.into_owned()),
            ast::Path::NegatedPropertySet(nps) => {
                let mut direct = Vec::new();
                let mut inverse = Vec::new();
                for p in nps {
                    match p {
                        ast::PathOneInPropertySet::Iri(p) => direct.push(self.build_named_node(p)?),
                        ast::PathOneInPropertySet::InverseIri(p) => {
                            inverse.push(self.build_named_node(p)?)
                        }
                        ast::PathOneInPropertySet::A => direct.push(rdf::TYPE.into_owned()),
                        ast::PathOneInPropertySet::InverseA => inverse.push(rdf::TYPE.into_owned()),
                    }
                }
                if inverse.is_empty() {
                    PropertyPathExpression::NegatedPropertySet(direct)
                } else if direct.is_empty() {
                    PropertyPathExpression::Reverse(Box::new(
                        PropertyPathExpression::NegatedPropertySet(inverse),
                    ))
                } else {
                    PropertyPathExpression::Alternative(
                        Box::new(PropertyPathExpression::NegatedPropertySet(direct)),
                        Box::new(PropertyPathExpression::Reverse(Box::new(
                            PropertyPathExpression::NegatedPropertySet(inverse),
                        ))),
                    )
                }
            }
        })
    }

    fn build_verb(&self, verb: ast::Verb<'_>) -> Result<NamedNodePattern, AlgebraBuilderError> {
        Ok(match verb {
            ast::Verb::Var(v) => Self::build_variable(v).into(),
            ast::Verb::Iri(n) => self.build_named_node(n)?.into(),
            ast::Verb::A => rdf::TYPE.into_owned().into(),
        })
    }

    fn build_term_pattern(
        &self,
        var_or_term: ast::VarOrTerm<'_>,
    ) -> Result<TermPattern, AlgebraBuilderError> {
        Ok(match var_or_term {
            ast::VarOrTerm::Var(v) => Self::build_variable(v).into(),
            ast::VarOrTerm::Iri(n) => self.build_named_node(n)?.into(),
            ast::VarOrTerm::BlankNode(n) => Self::build_blank_node(n).into(),
            ast::VarOrTerm::Literal(l) => self.build_literal(l)?.into(),
            ast::VarOrTerm::Nil => rdf::NIL.into_owned().into(),
            #[cfg(feature = "sparql-12")]
            ast::VarOrTerm::TripleTerm(t) => {
                TermPattern::Triple(Box::new(self.build_triple_term(*t)?))
            }
        })
    }

    #[cfg(feature = "sparql-12")]
    fn build_triple_term(
        &self,
        triple_term: ast::TripleTerm<'_>,
    ) -> Result<TriplePattern, AlgebraBuilderError> {
        Ok(TriplePattern::new(
            self.build_term_pattern(triple_term.subject)?,
            self.build_verb(triple_term.predicate)?,
            self.build_term_pattern(triple_term.object)?,
        ))
    }

    #[cfg(feature = "sparql-12")]
    fn build_reified_triple(
        &self,
        triple: ast::ReifiedTriple<'a>,
        patterns: &mut Vec<TriplePattern>,
    ) -> Result<TermPattern, AlgebraBuilderError> {
        let reifier = triple
            .reifier
            .map(|r| self.build_reifier_id(r))
            .transpose()?
            .unwrap_or_else(|| BlankNode::default().into());
        let triple = TriplePattern::new(
            self.build_reified_triple_subject_or_object(triple.subject, patterns)?,
            self.build_verb(triple.predicate)?,
            self.build_reified_triple_subject_or_object(triple.object, patterns)?,
        );
        patterns.push(TriplePattern::new(
            reifier.clone(),
            rdf::REIFIES.into_owned(),
            triple,
        ));
        Ok(reifier)
    }

    #[cfg(feature = "sparql-12")]
    fn build_reified_triple_subject_or_object(
        &self,
        triple_term: ast::ReifiedTripleSubjectOrObject<'a>,
        patterns: &mut Vec<TriplePattern>,
    ) -> Result<TermPattern, AlgebraBuilderError> {
        Ok(match triple_term {
            ast::ReifiedTripleSubjectOrObject::Var(v) => Self::build_variable(v).into(),
            ast::ReifiedTripleSubjectOrObject::Iri(n) => self.build_named_node(n)?.into(),
            ast::ReifiedTripleSubjectOrObject::BlankNode(n) => Self::build_blank_node(n).into(),
            ast::ReifiedTripleSubjectOrObject::Literal(l) => self.build_literal(l)?.into(),
            ast::ReifiedTripleSubjectOrObject::ReifiedTriple(t) => {
                self.build_reified_triple(*t, patterns)?
            }
            ast::ReifiedTripleSubjectOrObject::TripleTerm(t) => {
                TermPattern::Triple(Box::new(self.build_triple_term(*t)?))
            }
        })
    }

    fn build_named_node_pattern(
        &self,
        var_or_iri: ast::VarOrIri<'_>,
    ) -> Result<NamedNodePattern, AlgebraBuilderError> {
        Ok(match var_or_iri {
            ast::VarOrIri::Var(v) => Self::build_variable(v).into(),
            ast::VarOrIri::Iri(n) => self.build_named_node(n)?.into(),
        })
    }

    fn build_variable(var: Spanned<ast::Var<'_>>) -> Variable {
        Variable::new_unchecked(var.0)
    }

    fn build_literal(&self, literal: ast::Literal<'_>) -> Result<Literal, AlgebraBuilderError> {
        Ok(match literal {
            ast::Literal::Boolean(v) => {
                Literal::new_typed_literal(if v { "true" } else { "false" }, xsd::BOOLEAN)
            }
            ast::Literal::Integer(v) => Literal::new_typed_literal(v, xsd::INTEGER),
            ast::Literal::Decimal(v) => Literal::new_typed_literal(v, xsd::DECIMAL),
            ast::Literal::Double(v) => Literal::new_typed_literal(v, xsd::DOUBLE),
            ast::Literal::String(v) => Literal::new_simple_literal(Self::build_string(v)?),
            ast::Literal::LangString(v, l) => Literal::new_language_tagged_literal(
                Self::build_string(v)?,
                l.inner,
            )
            .map_err(|e| {
                AlgebraBuilderError::new(l.span, format!("Invalid language tag '{}': {e}", l.inner))
            })?,
            #[cfg(feature = "sparql-12")]
            ast::Literal::DirLangString(v, l) => Literal::new_directional_language_tagged_literal(
                Self::build_string(v)?,
                l.inner.0,
                match l.inner.1 {
                    "ltr" => BaseDirection::Ltr,
                    "rtl" => BaseDirection::Rtl,
                    _ => {
                        return Err(AlgebraBuilderError::new(
                            l.span,
                            format!(
                                "The only possible base directions are 'rtl' and 'ltr', found '{}'",
                                l.inner.1
                            ),
                        ));
                    }
                },
            )
            .map_err(|e| {
                AlgebraBuilderError::new(
                    l.span,
                    format!("Invalid language tag '{}': {e}", l.inner.0),
                )
            })?,
            ast::Literal::Typed(v, t) => {
                Literal::new_typed_literal(Self::build_string(v)?, self.build_named_node(t)?)
            }
        })
    }

    fn build_blank_node(blank_node: Spanned<ast::BlankNode<'_>>) -> BlankNode {
        if let Some(id) = blank_node.inner.0 {
            BlankNode::new_unchecked(id)
        } else {
            BlankNode::default()
        }
    }

    fn build_string(string: Spanned<ast::String<'_>>) -> Result<String, AlgebraBuilderError> {
        unescape_string(string.inner.0, string.span)
    }

    fn build_named_node(&self, iri: ast::Iri<'_>) -> Result<NamedNode, AlgebraBuilderError> {
        Ok(NamedNode::new_unchecked(
            match iri {
                ast::Iri::IriRef(iri) => self.build_iri(iri),
                ast::Iri::PrefixedName(pname) => self.build_prefixed_name(pname),
            }?
            .into_inner(),
        ))
    }

    fn build_prefixed_name(
        &self,
        pname: Spanned<ast::PrefixedName<'_>>,
    ) -> Result<Iri<String>, AlgebraBuilderError> {
        if let Some(base) = self.prefixes.get(pname.inner.0) {
            let mut iri = String::with_capacity(base.len() + pname.inner.1.len());
            iri.push_str(base);
            for chunk in pname.inner.1.split('\\') {
                // We remove \
                iri.push_str(chunk);
            }
            Iri::parse(iri).map_err(|e| {
                AlgebraBuilderError::new(
                    pname.span,
                    format!(
                        "Invalid IRI built from '{}:{}': {e}",
                        pname.inner.0, pname.inner.1
                    ),
                )
            })
        } else {
            Err(AlgebraBuilderError::new(
                pname.span,
                format!("The prefix '{}:' is not defined", pname.inner.0),
            ))
        }
    }

    fn build_iri(&self, iri: Spanned<ast::IriRef<'_>>) -> Result<Iri<String>, AlgebraBuilderError> {
        #[cfg(feature = "standard-unicode-escaping")]
        let iri_value = iri.inner.0;
        #[cfg(not(feature = "standard-unicode-escaping"))]
        let iri_value = unescape_iriref(iri.inner.0, iri.span)?;
        if let Some(base_iri) = &self.base_iri {
            #[cfg_attr(feature = "standard-unicode-escaping", expect(clippy::needless_borrow))]
            base_iri.resolve(&iri_value)
        } else {
            Iri::parse({
                #[cfg(feature = "standard-unicode-escaping")]
                {
                    iri_value.to_owned()
                }
                #[cfg(not(feature = "standard-unicode-escaping"))]
                {
                    iri_value.clone()
                }
            })
        }
        .map_err(|e| AlgebraBuilderError::new(iri.span, format!("Invalid IRI '{iri_value}': {e}")))
    }

    pub fn build_update(mut self, update: ast::Update<'_>) -> Result<Update, AlgebraBuilderError> {
        valid_update_operation_blank_node_id_syntax_restrictions(&update)?;

        let mut operations = Vec::new();
        for (prologue, update1) in update.operations {
            self.apply_prologue(prologue)?;
            match update1 {
                ast::Update1::Load { silent, from, to } => operations.push(
                    LoadOperation {
                        silent,
                        source: self.build_named_node(from)?,
                        destination: to
                            .map(|i| self.build_named_node(i))
                            .transpose()?
                            .map_or(GraphName::DefaultGraph, GraphName::NamedNode),
                    }
                    .into(),
                ),
                ast::Update1::Clear { silent, graph } => operations.push(
                    ClearOperation {
                        silent,
                        graph: self.build_graph_target(graph)?,
                    }
                    .into(),
                ),
                ast::Update1::Drop { silent, graph } => operations.push(
                    DropOperation {
                        silent,
                        graph: self.build_graph_target(graph)?,
                    }
                    .into(),
                ),
                ast::Update1::Create { silent, graph } => operations.push(
                    CreateOperation {
                        silent,
                        graph: self.build_named_node(graph)?,
                    }
                    .into(),
                ),
                ast::Update1::Add { from, to, .. } => {
                    // Rewriting defined by https://www.w3.org/TR/sparql11-update/#add
                    let from = self.build_graph_name(from)?;
                    let to = self.build_graph_name(to)?;
                    operations.push(copy_graph(from, to).into())
                }
                ast::Update1::Move { silent, from, to } => {
                    // Rewriting defined by https://www.w3.org/TR/sparql11-update/#move
                    let from = self.build_graph_name(from)?;
                    let to = self.build_graph_name(to)?;
                    if from != to {
                        operations.extend([
                            DropOperation {
                                silent: true,
                                graph: to.clone().into(),
                            }
                            .into(),
                            copy_graph(from.clone(), to).into(),
                            DropOperation {
                                silent,
                                graph: from.into(),
                            }
                            .into(),
                        ])
                    }
                }
                ast::Update1::Copy { from, to, .. } => {
                    // Rewriting defined by https://www.w3.org/TR/sparql11-update/#move
                    let from = self.build_graph_name(from)?;
                    let to = self.build_graph_name(to)?;
                    if from != to {
                        operations.extend([
                            DropOperation {
                                silent: true,
                                graph: to.clone().into(),
                            }
                            .into(),
                            copy_graph(from, to).into(),
                        ])
                    }
                }
                ast::Update1::DeleteWhere { pattern } => {
                    let delete = self.build_ground_quad_patterns(pattern)?;

                    let mut graph_pattern = GraphPattern::default();
                    let mut current_graph_name = &GraphNamePattern::DefaultGraph;
                    let mut current_bgp = Vec::new();
                    for pattern in &delete {
                        if *current_graph_name != pattern.graph_name {
                            graph_pattern = new_join(
                                graph_pattern,
                                wrap_bpg_in_graph(
                                    take(&mut current_bgp),
                                    current_graph_name.clone(),
                                ),
                            )
                        }
                        current_graph_name = &pattern.graph_name;
                        current_bgp.push(TriplePattern {
                            subject: pattern.subject.clone().into(),
                            predicate: pattern.predicate.clone(),
                            object: pattern.object.clone().into(),
                        });
                    }
                    graph_pattern = new_join(
                        graph_pattern,
                        wrap_bpg_in_graph(take(&mut current_bgp), current_graph_name.clone()),
                    );

                    operations.push(
                        DeleteInsertOperation {
                            delete,
                            insert: Vec::new(),
                            using: None,
                            pattern: Box::new(graph_pattern),
                        }
                        .into(),
                    )
                }
                ast::Update1::Modify {
                    with,
                    delete,
                    insert,
                    using,
                    r#where,
                } => {
                    let mut using = self.build_dataset(using)?;
                    let mut delete = self.build_ground_quad_patterns(delete)?;
                    let mut insert = self.build_quad_patterns(insert)?;

                    if let Some(with) = with {
                        let with = self.build_named_node(with)?;
                        // We inject WITH everywhere
                        for quad in &mut delete {
                            if quad.graph_name == GraphNamePattern::DefaultGraph {
                                quad.graph_name = with.clone().into();
                            }
                        }
                        for quad in &mut insert {
                            if quad.graph_name == GraphNamePattern::DefaultGraph {
                                quad.graph_name = with.clone().into();
                            }
                        }
                        if using.is_none() {
                            using = Some(QueryDataset {
                                default: vec![with],
                                named: None,
                            });
                        }
                    }

                    operations.push(
                        DeleteInsertOperation {
                            delete,
                            insert,
                            using,
                            pattern: Box::new(self.build_graph_pattern(r#where)?),
                        }
                        .into(),
                    );
                }
                ast::Update1::InsertData { quads } => operations.push(
                    InsertDataOperation {
                        data: self.build_quads(quads)?,
                    }
                    .into(),
                ),
                ast::Update1::DeleteData { quads } => operations.push(
                    DeleteDataOperation {
                        data: self.build_ground_quads(quads)?,
                    }
                    .into(),
                ),
            }
        }
        self.apply_prologue(update.trailing_prologue)?;
        Ok(Update {
            operations,
            base_iri: self.base_iri,
        })
    }

    fn build_ground_quad_patterns(
        &self,
        quads: ast::QuadPatterns<'_>,
    ) -> Result<Vec<GroundQuadPattern>, AlgebraBuilderError> {
        let mut visitor = FindBlankNodeOrVariable::default();
        visitor.visit_quads(&quads);
        if let Some(blank_node) = visitor.blank_node {
            return Err(AlgebraBuilderError::new(
                blank_node.span,
                "Blank nodes are not allowed in the DELETE part of updates",
            ));
        }
        Ok(self
            .build_quad_patterns(quads)?
            .into_iter()
            .map(|p| p.try_into().unwrap())
            .collect())
    }

    fn build_quads(&self, quads: ast::QuadPatterns<'_>) -> Result<Vec<Quad>, AlgebraBuilderError> {
        let mut visitor = FindBlankNodeOrVariable::default();
        visitor.visit_quads(&quads);
        if let Some(variable) = visitor.variable {
            return Err(AlgebraBuilderError::new(
                variable.span,
                "Variables are not allowed in INSERT DATA",
            ));
        }
        Ok(self
            .build_quad_patterns(quads)?
            .into_iter()
            .map(|p| p.try_into().unwrap())
            .collect())
    }

    fn build_ground_quads(
        &self,
        quads: ast::QuadPatterns<'_>,
    ) -> Result<Vec<GroundQuad>, AlgebraBuilderError> {
        let mut visitor = FindBlankNodeOrVariable::default();
        visitor.visit_quads(&quads);
        if let Some(variable) = visitor.variable {
            return Err(AlgebraBuilderError::new(
                variable.span,
                "Variables are not allowed in DELETE DATA",
            ));
        }
        if let Some(blank_node) = visitor.blank_node {
            return Err(AlgebraBuilderError::new(
                blank_node.span,
                "Blank nodes are not allowed in DELETE DATA",
            ));
        }
        Ok(self
            .build_quads(quads)?
            .into_iter()
            .map(|p| p.try_into().unwrap())
            .collect())
    }

    fn build_quad_patterns(
        &self,
        quads: ast::QuadPatterns<'_>,
    ) -> Result<Vec<QuadPattern>, AlgebraBuilderError> {
        let mut patterns = Vec::new();
        for (graph_name, triples) in quads {
            let graph_name = if let Some(graph_name) = graph_name {
                self.build_named_node_pattern(graph_name)?.into()
            } else {
                GraphNamePattern::DefaultGraph
            };
            for triple in self.build_triple_template(triples)? {
                patterns.push(QuadPattern {
                    subject: triple.subject,
                    predicate: triple.predicate,
                    object: triple.object,
                    graph_name: graph_name.clone(),
                });
            }
        }
        Ok(patterns)
    }

    fn build_graph_name(
        &self,
        graph_ref_all: ast::GraphOrDefault<'_>,
    ) -> Result<GraphName, AlgebraBuilderError> {
        Ok(match graph_ref_all {
            ast::GraphOrDefault::Graph(n) => self.build_named_node(n)?.into(),
            ast::GraphOrDefault::Default => GraphName::DefaultGraph,
        })
    }

    fn build_graph_target(
        &self,
        graph_ref_all: ast::GraphRefAll<'_>,
    ) -> Result<GraphTarget, AlgebraBuilderError> {
        Ok(match graph_ref_all {
            ast::GraphRefAll::Graph(n) => self.build_named_node(n)?.into(),
            ast::GraphRefAll::Default => GraphTarget::DefaultGraph,
            ast::GraphRefAll::Named => GraphTarget::NamedGraphs,
            ast::GraphRefAll::All => GraphTarget::AllGraphs,
        })
    }
}

fn register_aggregate(
    agg: AggregateExpression,
    aggregates: &mut Vec<(Variable, AggregateExpression)>,
) -> Variable {
    aggregates
        .iter()
        .find_map(|(v, a)| (*a == agg).then_some(v))
        .cloned()
        .unwrap_or_else(|| {
            let new_var = random_variable();
            aggregates.push((new_var.clone(), agg));
            new_var
        })
}

fn random_variable() -> Variable {
    Variable::new_unchecked(format!("{:x}", random::<u128>()))
}

fn find_unbound_variable<'a>(
    expression: &'a Expression,
    variables: &HashSet<Variable>,
) -> Option<&'a Variable> {
    match expression {
        Expression::NamedNode(_)
        | Expression::Literal(_)
        | Expression::Bound(_)
        | Expression::Coalesce(_)
        | Expression::Exists(_) => None,
        Expression::Variable(var) => (!variables.contains(var)).then_some(var),
        Expression::UnaryPlus(e) | Expression::UnaryMinus(e) | Expression::Not(e) => {
            find_unbound_variable(e, variables)
        }
        Expression::Or(a, b)
        | Expression::And(a, b)
        | Expression::Equal(a, b)
        | Expression::SameTerm(a, b)
        | Expression::Greater(a, b)
        | Expression::GreaterOrEqual(a, b)
        | Expression::Less(a, b)
        | Expression::LessOrEqual(a, b)
        | Expression::Add(a, b)
        | Expression::Subtract(a, b)
        | Expression::Multiply(a, b)
        | Expression::Divide(a, b) => {
            find_unbound_variable(a, variables)?;
            find_unbound_variable(b, variables)
        }
        Expression::In(a, b) => {
            find_unbound_variable(a, variables)?;
            b.iter().find_map(|b| find_unbound_variable(b, variables))
        }
        Expression::FunctionCall(_, parameters) => parameters
            .iter()
            .find_map(|p| find_unbound_variable(p, variables)),
        Expression::If(a, b, c) => {
            find_unbound_variable(a, variables)?;
            find_unbound_variable(b, variables)?;
            find_unbound_variable(c, variables)
        }
    }
}

fn new_join(l: GraphPattern, r: GraphPattern) -> GraphPattern {
    // Avoid to output empty BGPs
    if let GraphPattern::Bgp { patterns: pl } = &l {
        if pl.is_empty() {
            return r;
        }
    }
    if let GraphPattern::Bgp { patterns: pr } = &r {
        if pr.is_empty() {
            return l;
        }
    }

    match (l, r) {
        (GraphPattern::Bgp { patterns: mut pl }, GraphPattern::Bgp { patterns: pr }) => {
            pl.extend(pr);
            GraphPattern::Bgp { patterns: pl }
        }
        (GraphPattern::Bgp { patterns }, other) | (other, GraphPattern::Bgp { patterns })
            if patterns.is_empty() =>
        {
            other
        }
        (l, r) => GraphPattern::Join {
            left: Box::new(l),
            right: Box::new(r),
        },
    }
}

fn wrap_bpg_in_graph(bgp: Vec<TriplePattern>, graph_name: GraphNamePattern) -> GraphPattern {
    if bgp.is_empty() {
        return GraphPattern::default();
    }
    let bgp = GraphPattern::Bgp { patterns: bgp };
    match graph_name {
        GraphNamePattern::NamedNode(g) => GraphPattern::Graph {
            name: g.into(),
            inner: Box::new(bgp),
        },
        GraphNamePattern::DefaultGraph => bgp,
        GraphNamePattern::Variable(g) => GraphPattern::Graph {
            name: g.into(),
            inner: Box::new(bgp),
        },
    }
}

impl<'a> From<ast::GraphNode<'a>> for ast::GraphNodePath<'a> {
    fn from(node: ast::GraphNode<'a>) -> Self {
        match node {
            ast::GraphNode::VarOrTerm(n) => Self::VarOrTerm(n),
            ast::GraphNode::Collection(c) => Self::Collection(
                c.span
                    .make_wrapped(c.inner.into_iter().map(Into::into).collect()),
            ),
            ast::GraphNode::BlankNodePropertyList(pl) => Self::BlankNodePropertyList(
                pl.span.make_wrapped(
                    pl.inner
                        .into_iter()
                        .map(|(p, os)| (p.into(), os.into_iter().map(Into::into).collect()))
                        .collect(),
                ),
            ),
            #[cfg(feature = "sparql-12")]
            ast::GraphNode::ReifiedTriple(t) => Self::ReifiedTriple(t),
        }
    }
}

impl<'a> From<ast::Verb<'a>> for ast::VarOrPath<'a> {
    fn from(verb: ast::Verb<'a>) -> Self {
        match verb {
            ast::Verb::Var(v) => Self::Var(v),
            ast::Verb::Iri(v) => Self::Path(ast::Path::Iri(v)),
            ast::Verb::A => Self::Path(ast::Path::A),
        }
    }
}

impl<'a> From<ast::Object<'a>> for ast::ObjectPath<'a> {
    fn from(object: ast::Object<'a>) -> Self {
        Self {
            graph_node: object.graph_node.into(),
            #[cfg(feature = "sparql-12")]
            annotations: object
                .annotations
                .into_iter()
                .map(|s| s.span.make_wrapped(s.inner.into()))
                .collect(),
        }
    }
}

#[cfg(feature = "sparql-12")]
impl<'a> From<ast::Annotation<'a>> for ast::AnnotationPath<'a> {
    fn from(annotation: ast::Annotation<'a>) -> Self {
        match annotation {
            ast::Annotation::Reifier(id) => Self::Reifier(id),
            ast::Annotation::AnnotationBlock(pl) => Self::AnnotationBlock(
                pl.into_iter()
                    .map(|(p, os)| (p.into(), os.into_iter().map(Into::into).collect()))
                    .collect(),
            ),
        }
    }
}

enum TripleOrPathPattern {
    Triple(TriplePattern),
    Path {
        subject: TermPattern,
        path: PropertyPathExpression,
        object: TermPattern,
    },
}

#[derive(Clone)]
enum VarOrPath {
    Var(Variable),
    Path(PropertyPathExpression),
}

fn add_path_to_patterns(
    subject: TermPattern,
    path: PropertyPathExpression,
    object: TermPattern,
    patterns: &mut Vec<TripleOrPathPattern>,
) {
    match path {
        PropertyPathExpression::NamedNode(predicate) => patterns.push(TripleOrPathPattern::Triple(
            TriplePattern::new(subject, predicate, object),
        )),
        PropertyPathExpression::Reverse(path) => {
            add_path_to_patterns(object, *path, subject, patterns)
        }
        PropertyPathExpression::Sequence(path1, path2) => {
            let middle = BlankNode::default();
            add_path_to_patterns(subject, *path1, middle.clone().into(), patterns);
            add_path_to_patterns(middle.into(), *path2, object, patterns)
        }
        _ => patterns.push(TripleOrPathPattern::Path {
            subject,
            path,
            object,
        }),
    }
}

/// Called on every variable defined using "AS" or "VALUES"
#[cfg(feature = "sep-0006")]
fn add_defined_variables<'a>(pattern: &'a GraphPattern, set: &mut HashSet<&'a Variable>) {
    match pattern {
        GraphPattern::Bgp { .. } | GraphPattern::Path { .. } => {}
        GraphPattern::Join { left, right }
        | GraphPattern::LeftJoin { left, right, .. }
        | GraphPattern::Lateral { left, right }
        | GraphPattern::Union { left, right }
        | GraphPattern::Minus { left, right } => {
            add_defined_variables(left, set);
            add_defined_variables(right, set);
        }
        GraphPattern::Graph { inner, .. } => {
            add_defined_variables(inner, set);
        }
        GraphPattern::Extend {
            inner, variable, ..
        } => {
            set.insert(variable);
            add_defined_variables(inner, set);
        }
        GraphPattern::Group {
            variables,
            aggregates,
            inner,
        } => {
            for (v, _) in aggregates {
                set.insert(v);
            }
            let mut inner_variables = HashSet::new();
            add_defined_variables(inner, &mut inner_variables);
            for v in inner_variables {
                if variables.contains(v) {
                    set.insert(v);
                }
            }
        }
        GraphPattern::Values { variables, .. } => {
            for v in variables {
                set.insert(v);
            }
        }
        GraphPattern::Project { variables, inner } => {
            let mut inner_variables = HashSet::new();
            add_defined_variables(inner, &mut inner_variables);
            for v in inner_variables {
                if variables.contains(v) {
                    set.insert(v);
                }
            }
        }
        GraphPattern::Service { inner, .. }
        | GraphPattern::Filter { inner, .. }
        | GraphPattern::OrderBy { inner, .. }
        | GraphPattern::Distinct { inner }
        | GraphPattern::Reduced { inner }
        | GraphPattern::Slice { inner, .. } => add_defined_variables(inner, set),
    }
}

#[cfg(not(feature = "standard-unicode-escaping"))]
fn unescape_iriref(mut input: &str, span: SimpleSpan) -> Result<String, AlgebraBuilderError> {
    let mut output = String::with_capacity(input.len());
    while let Some((before, after)) = input.split_once('\\') {
        output.push_str(before);
        let mut after = after.chars();
        let (escape, after) = match after.next() {
            Some('u') => read_hex_char::<4>(after.as_str(), span)?,
            Some('U') => read_hex_char::<8>(after.as_str(), span)?,
            Some(c) => {
                unreachable!(
                    "IRIs are only allowed to contain escape sequences \\uXXXX and \\UXXXXXXXX, found \\{c}"
                );
            }
            None => {
                unreachable!("IRIs are not allowed to end with a '\\'");
            }
        };
        output.push(escape);
        input = after;
    }
    output.push_str(input);
    Ok(output)
}

#[cfg_attr(
    feature = "standard-unicode-escaping",
    expect(unused_variables, clippy::unnecessary_wraps)
)]
fn unescape_string(mut input: &str, span: SimpleSpan) -> Result<String, AlgebraBuilderError> {
    let mut output = String::with_capacity(input.len());
    while let Some((before, after)) = input.split_once('\\') {
        output.push_str(before);
        let mut after = after.chars();
        let (escape, after) = match after.next() {
            Some('t') => ('\u{0009}', after.as_str()),
            Some('b') => ('\u{0008}', after.as_str()),
            Some('n') => ('\u{000A}', after.as_str()),
            Some('r') => ('\u{000D}', after.as_str()),
            Some('f') => ('\u{000C}', after.as_str()),
            Some('"') => ('\u{0022}', after.as_str()),
            Some('\'') => ('\u{0027}', after.as_str()),
            Some('\\') => ('\u{005C}', after.as_str()),
            #[cfg(not(feature = "standard-unicode-escaping"))]
            Some('u') => read_hex_char::<4>(after.as_str(), span)?,
            #[cfg(not(feature = "standard-unicode-escaping"))]
            Some('U') => read_hex_char::<8>(after.as_str(), span)?,
            Some(c) => {
                unreachable!("\\{c} is not an allowed escaping in strings");
            }
            None => {
                unreachable!("strings are not allowed to end with a '\\'");
            }
        };
        output.push(escape);
        input = after;
    }
    output.push_str(input);
    Ok(output)
}

#[cfg(not(feature = "standard-unicode-escaping"))]
#[expect(clippy::expect_used, clippy::unwrap_in_result)]
fn read_hex_char<const SIZE: usize>(
    input: &str,
    span: SimpleSpan,
) -> Result<(char, &str), AlgebraBuilderError> {
    let escape = input
        .get(..SIZE)
        .expect("\\u escape sequence must contain 4 characters");
    let char = u32::from_str_radix(escape, 16)
        .expect("\\u escape sequence must be followed by hexadecimal digits");
    let char = char::from_u32(char).ok_or_else(|| {
        AlgebraBuilderError::new(
            span,
            format!("{char:#X} is not a valid unicode codepoint (surrogates are not supported"),
        )
    })?;
    Ok((char, &input[SIZE..]))
}

fn function_arity(name: &Function) -> RangeInclusive<usize> {
    match name {
        Function::Str => 1..=1,
        Function::Lang => 1..=1,
        Function::LangMatches => 2..=2,
        Function::Datatype => 1..=1,
        Function::Iri => 1..=1,
        Function::BNode => 0..=1,
        Function::Rand => 0..=0,
        Function::Abs => 1..=1,
        Function::Ceil => 1..=1,
        Function::Floor => 1..=1,
        Function::Round => 1..=1,
        Function::Concat => 0..=usize::MAX,
        Function::SubStr => 2..=3,
        Function::StrLen => 1..=1,
        Function::Replace => 3..=4,
        Function::UCase => 1..=1,
        Function::LCase => 1..=1,
        Function::EncodeForUri => 1..=1,
        Function::Contains => 2..=2,
        Function::StrStarts => 2..=2,
        Function::StrEnds => 2..=2,
        Function::StrBefore => 2..=2,
        Function::StrAfter => 2..=2,
        Function::Year => 1..=1,
        Function::Month => 1..=1,
        Function::Day => 1..=1,
        Function::Hours => 1..=1,
        Function::Minutes => 1..=1,
        Function::Seconds => 1..=1,
        Function::Timezone => 1..=1,
        Function::Tz => 1..=1,
        Function::Now => 0..=0,
        Function::Uuid => 0..=0,
        Function::StrUuid => 0..=0,
        Function::Md5 => 1..=1,
        Function::Sha1 => 1..=1,
        Function::Sha256 => 1..=1,
        Function::Sha384 => 1..=1,
        Function::Sha512 => 1..=1,
        Function::StrLang => 2..=2,
        Function::StrDt => 2..=2,
        Function::IsIri => 1..=1,
        Function::IsBlank => 1..=1,
        Function::IsLiteral => 1..=1,
        Function::IsNumeric => 1..=1,
        Function::Regex => 2..=3,
        #[cfg(feature = "sparql-12")]
        Function::Triple => 3..=3,
        #[cfg(feature = "sparql-12")]
        Function::Subject => 1..=1,
        #[cfg(feature = "sparql-12")]
        Function::Predicate => 1..=1,
        #[cfg(feature = "sparql-12")]
        Function::Object => 1..=1,
        #[cfg(feature = "sparql-12")]
        Function::IsTriple => 1..=1,
        #[cfg(feature = "sparql-12")]
        Function::LangDir => 1..=1,
        #[cfg(feature = "sparql-12")]
        Function::HasLang => 1..=1,
        #[cfg(feature = "sparql-12")]
        Function::HasLangDir => 1..=1,
        #[cfg(feature = "sparql-12")]
        Function::StrLangDir => 3..=3,
        #[cfg(feature = "sep-0002")]
        Function::Adjust => 2..=2,
        Function::Custom(_) => 0..=usize::MAX,
    }
}

fn valid_update_operation_blank_node_id_syntax_restrictions(
    update: &ast::Update<'_>,
) -> Result<(), AlgebraBuilderError> {
    let mut all_blank_nodes = HashMap::new();
    for (_, operation) in &update.operations {
        extend_blank_node_ids_if_not_overlapping(
            &mut all_blank_nodes,
            find_update_operation_blank_node_ids_and_validate_syntax_restrictions(operation)?,
        )?;
    }
    Ok(())
}

fn find_update_operation_blank_node_ids_and_validate_syntax_restrictions<'a>(
    update: &ast::Update1<'a>,
) -> Result<HashMap<&'a str, SimpleSpan>, AlgebraBuilderError> {
    match update {
        ast::Update1::Load { .. }
        | ast::Update1::Clear { .. }
        | ast::Update1::Drop { .. }
        | ast::Update1::Create { .. }
        | ast::Update1::Add { .. }
        | ast::Update1::Move { .. }
        | ast::Update1::Copy { .. } => Ok(HashMap::new()),
        ast::Update1::Modify { r#where, .. } => {
            find_graph_pattern_blank_node_ids_and_validate_syntax_restrictions(r#where)
        }
        ast::Update1::DeleteWhere { pattern: quads }
        | ast::Update1::InsertData { quads }
        | ast::Update1::DeleteData { quads } => {
            let mut blank_nodes = HashMap::new();
            for (_, triples) in quads {
                for (subject, property_list) in triples {
                    blank_nodes.visit_graph_node(subject);
                    blank_nodes.visit_property_list(property_list);
                }
            }
            Ok(blank_nodes)
        }
    }
}

fn find_graph_pattern_blank_node_ids_and_validate_syntax_restrictions<'a>(
    graph_pattern: &ast::GraphPattern<'a>,
) -> Result<HashMap<&'a str, SimpleSpan>, AlgebraBuilderError> {
    match graph_pattern {
        ast::GraphPattern::Group(elements) => {
            let mut all_blank_nodes = HashMap::new();
            let mut current_bgp_blank_nodes = HashMap::new();
            for element in elements {
                match &element.inner {
                    ast::GraphPatternElement::Filter(_) => (),
                    ast::GraphPatternElement::Values(_) | ast::GraphPatternElement::Bind(_, _) => {
                        extend_blank_node_ids_if_not_overlapping(
                            &mut all_blank_nodes,
                            take(&mut current_bgp_blank_nodes),
                        )?;
                    }
                    ast::GraphPatternElement::Union(children) => {
                        extend_blank_node_ids_if_not_overlapping(
                            &mut all_blank_nodes,
                            take(&mut current_bgp_blank_nodes),
                        )?;
                        for child in children {
                            let new_blank_nodes =
                                find_graph_pattern_blank_node_ids_and_validate_syntax_restrictions(
                                    child,
                                )?;
                            extend_blank_node_ids_if_not_overlapping(
                                &mut all_blank_nodes,
                                new_blank_nodes,
                            )?;
                        }
                    }
                    ast::GraphPatternElement::Minus(pattern)
                    | ast::GraphPatternElement::Optional(pattern)
                    | ast::GraphPatternElement::Graph { pattern, .. }
                    | ast::GraphPatternElement::Service { pattern, .. } => {
                        extend_blank_node_ids_if_not_overlapping(
                            &mut all_blank_nodes,
                            take(&mut current_bgp_blank_nodes),
                        )?;
                        extend_blank_node_ids_if_not_overlapping(
                            &mut all_blank_nodes,
                            find_graph_pattern_blank_node_ids_and_validate_syntax_restrictions(
                                pattern,
                            )?,
                        )?;
                    }
                    ast::GraphPatternElement::Triples(triples) => {
                        for (subject, predicate_objects) in triples {
                            current_bgp_blank_nodes.visit_graph_node_path(subject);
                            current_bgp_blank_nodes.visit_property_list_path(predicate_objects);
                        }
                    }
                    #[cfg(feature = "sep-0006")]
                    ast::GraphPatternElement::Lateral(pattern) => {
                        extend_blank_node_ids_if_not_overlapping(
                            &mut all_blank_nodes,
                            take(&mut current_bgp_blank_nodes),
                        )?;
                        extend_blank_node_ids_if_not_overlapping(
                            &mut all_blank_nodes,
                            find_graph_pattern_blank_node_ids_and_validate_syntax_restrictions(
                                pattern,
                            )?,
                        )?;
                    }
                }
            }
            extend_blank_node_ids_if_not_overlapping(
                &mut all_blank_nodes,
                current_bgp_blank_nodes,
            )?;
            Ok(all_blank_nodes)
        }
        ast::GraphPattern::SubSelect(select) => {
            find_graph_pattern_blank_node_ids_and_validate_syntax_restrictions(&select.where_clause)
        }
    }
}

fn extend_blank_node_ids_if_not_overlapping<'a>(
    all_blank_nodes: &mut HashMap<&'a str, SimpleSpan>,
    new_blank_nodes: HashMap<&'a str, SimpleSpan>,
) -> Result<(), AlgebraBuilderError> {
    if let Some(blank_node) = new_blank_nodes.iter().find_map(|(name, span1)| {
        let span2 = all_blank_nodes.get(name)?;
        Some(
            SimpleSpan::new((), min(span1.start, span2.start)..max(span1.end, span2.end))
                .make_wrapped(*name),
        )
    }) {
        return Err(AlgebraBuilderError::new(
            blank_node.span,
            format!(
                "_:{} is already used in an other graph pattern, this is not allowed for blank nodes",
                blank_node.inner
            ),
        ));
    }
    all_blank_nodes.extend(new_blank_nodes);
    Ok(())
}

trait TermVisitor<'a> {
    fn on_blank_node(&mut self, _blank_node: &Spanned<ast::BlankNode<'a>>) {}
    fn on_variable(&mut self, _variable: &Spanned<ast::Var<'a>>) {}

    fn visit_quads(&mut self, quads: &ast::QuadPatterns<'a>) {
        for (graph_name, triples) in quads {
            if let Some(graph_name) = graph_name {
                self.visit_var_or_iri(graph_name);
            }
            for (subject, predicate_object) in triples {
                self.visit_graph_node(subject);
                self.visit_property_list(predicate_object);
            }
        }
    }

    fn visit_graph_node_path(&mut self, graph_node_path: &ast::GraphNodePath<'a>) {
        match graph_node_path {
            ast::GraphNodePath::VarOrTerm(var_or_term) => self.visit_var_or_term(var_or_term),
            ast::GraphNodePath::Collection(nodes) => {
                // We use blank nodes for collections
                self.on_blank_node(&nodes.span.make_wrapped(ast::BlankNode(None)));
                for node in &nodes.inner {
                    self.visit_graph_node_path(node);
                }
            }
            ast::GraphNodePath::BlankNodePropertyList(property_list) => {
                // This is an anonymous blank node
                self.on_blank_node(&property_list.span.make_wrapped(ast::BlankNode(None)));
                self.visit_property_list_path(&property_list.inner)
            }
            #[cfg(feature = "sparql-12")]
            ast::GraphNodePath::ReifiedTriple(triple) => self.visit_reified_triple(triple),
        }
    }

    fn visit_graph_node(&mut self, graph_node: &ast::GraphNode<'a>) {
        match graph_node {
            ast::GraphNode::VarOrTerm(var_or_term) => self.visit_var_or_term(var_or_term),
            ast::GraphNode::Collection(nodes) => {
                // We use blank nodes for collections
                self.on_blank_node(&nodes.span.make_wrapped(ast::BlankNode(None)));
                for node in &nodes.inner {
                    self.visit_graph_node(node);
                }
            }
            ast::GraphNode::BlankNodePropertyList(property_list) => {
                // This is an anonymous blank node
                self.on_blank_node(&property_list.span.make_wrapped(ast::BlankNode(None)));
                self.visit_property_list(&property_list.inner)
            }
            #[cfg(feature = "sparql-12")]
            ast::GraphNode::ReifiedTriple(triple) => self.visit_reified_triple(triple),
        }
    }

    fn visit_property_list_path(&mut self, property_list_path: &ast::PropertyListPath<'a>) {
        for (predicate, objects) in property_list_path {
            self.visit_var_or_path(predicate);
            for object in objects {
                self.visit_graph_node_path(&object.graph_node);
                #[cfg(feature = "sparql-12")]
                {
                    let mut with_explicit_reifier = false;
                    for annotation in &object.annotations {
                        match &annotation.inner {
                            ast::AnnotationPath::Reifier(reifier) => {
                                if let Some(reifier) = reifier {
                                    self.visit_var_or_reifier_id(reifier)
                                } else {
                                    // This is an anonymous blank node
                                    self.on_blank_node(
                                        &annotation.span.make_wrapped(ast::BlankNode(None)),
                                    );
                                }
                                with_explicit_reifier = true;
                            }
                            ast::AnnotationPath::AnnotationBlock(property_list) => {
                                if !with_explicit_reifier {
                                    // We use an anonymous blank node
                                    self.on_blank_node(
                                        &annotation.span.make_wrapped(ast::BlankNode(None)),
                                    );
                                }
                                self.visit_property_list_path(property_list);
                                with_explicit_reifier = false;
                            }
                        }
                    }
                }
            }
        }
    }

    fn visit_property_list(&mut self, property_list: &ast::PropertyList<'a>) {
        for (predicate, objects) in property_list {
            self.visit_verb(predicate);
            for object in objects {
                self.visit_graph_node(&object.graph_node);
                #[cfg(feature = "sparql-12")]
                {
                    let mut with_explicit_reifier = false;
                    for annotation in &object.annotations {
                        match &annotation.inner {
                            ast::Annotation::Reifier(reifier) => {
                                if let Some(reifier) = reifier {
                                    self.visit_var_or_reifier_id(reifier);
                                } else {
                                    // This is an anonymous blank node
                                    self.on_blank_node(
                                        &annotation.span.make_wrapped(ast::BlankNode(None)),
                                    );
                                }
                                with_explicit_reifier = true;
                            }
                            ast::Annotation::AnnotationBlock(property_list) => {
                                if !with_explicit_reifier {
                                    // We use an anonymous blank node
                                    self.on_blank_node(
                                        &annotation.span.make_wrapped(ast::BlankNode(None)),
                                    );
                                }
                                self.visit_property_list(property_list);
                                with_explicit_reifier = false;
                            }
                        }
                    }
                }
            }
        }
    }

    fn visit_var_or_term(&mut self, var_or_term: &ast::VarOrTerm<'a>) {
        match var_or_term {
            ast::VarOrTerm::BlankNode(bnode) => self.on_blank_node(bnode),
            ast::VarOrTerm::Var(v) => self.on_variable(v),
            ast::VarOrTerm::Iri(_) | ast::VarOrTerm::Literal(_) | ast::VarOrTerm::Nil => (),
            #[cfg(feature = "sparql-12")]
            ast::VarOrTerm::TripleTerm(triple_term) => self.visit_triple_term(triple_term),
        }
    }

    #[cfg(feature = "sparql-12")]
    fn visit_reified_triple(&mut self, reified_triple: &ast::ReifiedTriple<'a>) {
        self.visit_reified_triple_term_subject_or_object(&reified_triple.subject);
        self.visit_verb(&reified_triple.predicate);
        self.visit_reified_triple_term_subject_or_object(&reified_triple.object);
    }

    #[cfg(feature = "sparql-12")]
    fn visit_triple_term(&mut self, triple_term: &ast::TripleTerm<'a>) {
        self.visit_var_or_term(&triple_term.subject);
        self.visit_verb(&triple_term.predicate);
        self.visit_var_or_term(&triple_term.object)
    }

    #[cfg(feature = "sparql-12")]
    fn visit_reified_triple_term_subject_or_object(
        &mut self,
        var_or_term: &ast::ReifiedTripleSubjectOrObject<'a>,
    ) {
        match var_or_term {
            ast::ReifiedTripleSubjectOrObject::BlankNode(bnode) => self.on_blank_node(bnode),
            ast::ReifiedTripleSubjectOrObject::Var(v) => self.on_variable(v),
            ast::ReifiedTripleSubjectOrObject::Iri(_)
            | ast::ReifiedTripleSubjectOrObject::Literal(_) => (),
            ast::ReifiedTripleSubjectOrObject::TripleTerm(triple_term) => {
                self.visit_triple_term(triple_term)
            }
            ast::ReifiedTripleSubjectOrObject::ReifiedTriple(triple) => {
                self.visit_reified_triple(triple)
            }
        }
    }

    #[cfg(feature = "sparql-12")]
    fn visit_var_or_reifier_id(&mut self, var_or_reifier_id: &ast::VarOrReifierId<'a>) {
        match var_or_reifier_id {
            ast::VarOrReifierId::Var(v) => self.on_variable(v),
            ast::VarOrReifierId::Iri(_) => {}
            ast::VarOrReifierId::BlankNode(bnode) => self.on_blank_node(bnode),
        }
    }

    fn visit_verb(&mut self, verb: &ast::Verb<'a>) {
        match verb {
            ast::Verb::Var(v) => self.on_variable(v),
            ast::Verb::Iri(_) | ast::Verb::A => {}
        }
    }

    fn visit_var_or_path(&mut self, var_or_path: &ast::VarOrPath<'a>) {
        match var_or_path {
            ast::VarOrPath::Var(v) => self.on_variable(v),
            ast::VarOrPath::Path(_) => {}
        }
    }

    fn visit_var_or_iri(&mut self, var_or_iri: &ast::VarOrIri<'a>) {
        match var_or_iri {
            ast::VarOrIri::Var(v) => self.on_variable(v),
            ast::VarOrIri::Iri(_) => {}
        }
    }
}

impl<'a> TermVisitor<'a> for HashMap<&'a str, SimpleSpan> {
    fn on_blank_node(&mut self, bnode: &Spanned<ast::BlankNode<'a>>) {
        if let Some(id) = &bnode.inner.0 {
            self.insert(id, bnode.span);
        }
    }
}

#[derive(Default)]
struct FindBlankNodeOrVariable<'a> {
    blank_node: Option<Spanned<ast::BlankNode<'a>>>,
    variable: Option<Spanned<ast::Var<'a>>>,
}

impl<'a> TermVisitor<'a> for FindBlankNodeOrVariable<'a> {
    fn on_blank_node(&mut self, blank_node: &Spanned<ast::BlankNode<'a>>) {
        self.blank_node.get_or_insert(*blank_node);
    }

    fn on_variable(&mut self, variable: &Spanned<ast::Var<'a>>) {
        self.variable.get_or_insert(*variable);
    }
}

fn copy_graph(
    from: impl Into<GraphName>,
    to: impl Into<GraphNamePattern>,
) -> DeleteInsertOperation {
    let bgp = GraphPattern::Bgp {
        patterns: vec![TriplePattern::new(
            Variable::new_unchecked("s"),
            Variable::new_unchecked("p"),
            Variable::new_unchecked("o"),
        )],
    };
    DeleteInsertOperation {
        delete: Vec::new(),
        insert: vec![QuadPattern::new(
            Variable::new_unchecked("s"),
            Variable::new_unchecked("p"),
            Variable::new_unchecked("o"),
            to,
        )],
        using: None,
        pattern: Box::new(match from.into() {
            GraphName::NamedNode(from) => GraphPattern::Graph {
                name: from.into(),
                inner: Box::new(bgp),
            },
            GraphName::DefaultGraph => bgp,
        }),
    }
}
