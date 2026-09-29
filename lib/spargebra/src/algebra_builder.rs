use crate::algebra::{
    AggregateExpression, Expression, GraphTarget, OrderExpression, PropertyPathExpression,
    QueryDatasetSpecification, QueryExpression,
};
use crate::ast;
use crate::error::AlgebraBuilderError;
use crate::query::{AskQuery, ConstructQuery, DescribeQuery, Query, SelectQuery};
#[cfg(feature = "sparql-12")]
use crate::term::GroundTriple;
use crate::term::{
    GraphName, GraphNamePattern, GroundQuad, GroundTerm, NamedNodePattern, Quad, QuadPattern,
    QuadTemplate, TermPattern, TermTemplate, TriplePattern, TripleTemplate,
};
use crate::update::{
    ClearOperation, CreateOperation, DeleteDataOperation, DeleteInsertOperation, DropOperation,
    InsertDataOperation, LoadOperation, Update,
};
use crate::vocab::sparql;
use chumsky::span::{SimpleSpan, Span, Spanned, WrappingSpan};
use oxiri::{Iri, IriRef};
#[cfg(feature = "sparql-12")]
use oxrdf::BaseDirection;
#[cfg(feature = "sparql-12")]
use oxrdf::Triple;
use oxrdf::vocab::{rdf, xsd};
use oxrdf::{BlankNode, Literal, NamedNode, NamedOrBlankNode, Term, Variable};
use oxstr::OxString;
use std::borrow::Cow;
use std::cmp::{max, min};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::mem::take;
use std::ops::RangeInclusive;

pub struct AlgebraBuilder<'a> {
    base_iri: Option<Iri<OxString>>,
    prefixes: HashMap<OxString, Iri<OxString>>,
    custom_aggregate_functions: &'a HashSet<NamedNode>,
    buffer: String,
    variable_allocator: FreshVariableAllocator<'a>,
    blank_node_allocator: FreshBlankNodeAllocator<'a>,
}

impl<'a> AlgebraBuilder<'a> {
    pub fn new(
        base_iri: Option<Iri<OxString>>,
        prefixes: HashMap<OxString, Iri<OxString>>,
        custom_aggregate_functions: &'a HashSet<NamedNode>,
    ) -> Self {
        Self {
            base_iri,
            prefixes,
            custom_aggregate_functions,
            buffer: String::new(),
            variable_allocator: FreshVariableAllocator::default(),
            blank_node_allocator: FreshBlankNodeAllocator::default(),
        }
    }

    pub fn build_query(mut self, query: ast::Query<'a>) -> Result<Query, AlgebraBuilderError> {
        self.variable_allocator.reset_with_query(&query);
        self.blank_node_allocator.reset_with_query(&query);
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
        mut self,
        query: ast::SelectQuery<'a>,
        values_clause: Option<ast::ValuesClause<'a>>,
    ) -> Result<SelectQuery, AlgebraBuilderError> {
        Ok(SelectQuery {
            dataset: self.build_dataset(query.dataset_clause)?,
            expression: self.build_select(
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
        mut self,
        query: ast::ConstructQuery<'a>,
        values_clause: Option<ast::ValuesClause<'a>>,
    ) -> Result<ConstructQuery, AlgebraBuilderError> {
        let where_clause = query.where_clause.unwrap_or_else(|| {
            ast::GraphPattern::Group(vec![query.template.span.make_wrapped(
                ast::GraphPatternElement::Triples(query.template.inner.clone()),
            )])
        });
        let template = self.build_triple_template(query.template.inner)?;
        Ok(ConstructQuery {
            template: template.clone(),
            dataset: self.build_dataset(query.dataset_clause)?,
            expression: self.build_select(
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
        mut self,
        query: ast::DescribeQuery<'a>,
        values_clause: Option<ast::ValuesClause<'a>>,
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
                    let variable =
                        Variable::new_unchecked(OxString::new_owned(&format!("v{counter}")));
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
                    pattern = QueryExpression::Extend {
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
        mut self,
        query: ast::AskQuery<'a>,
        values_clause: Option<ast::ValuesClause<'a>>,
    ) -> Result<AskQuery, AlgebraBuilderError> {
        Ok(AskQuery {
            dataset: self.build_dataset(query.dataset_clause)?,
            expression: self.build_select(
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
        prologue: Vec<ast::PrologueDecl<'a>>,
    ) -> Result<(), AlgebraBuilderError> {
        for decl in prologue {
            self.apply_prologue_decl(decl)?;
        }
        Ok(())
    }

    fn apply_prologue_decl(
        &mut self,
        decl: ast::PrologueDecl<'a>,
    ) -> Result<(), AlgebraBuilderError> {
        match decl {
            ast::PrologueDecl::Base(base_iri) => {
                self.base_iri = Some(Iri::parse_unchecked(self.build_iri(base_iri)?));
            }
            ast::PrologueDecl::Prefix(prefix, iri) => {
                let iri = Iri::parse_unchecked(self.build_iri(iri)?);
                self.prefixes.insert(OxString::new_owned(prefix), iri);
            }
            #[cfg(feature = "sparql-12")]
            ast::PrologueDecl::Version(_) => (),
        }
        Ok(())
    }

    fn build_dataset(
        &mut self,
        clauses: Vec<ast::GraphClause<'a>>,
    ) -> Result<Option<QueryDatasetSpecification>, AlgebraBuilderError> {
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
        Ok(Some(QueryDatasetSpecification {
            default,
            named: Some(named),
        }))
    }

    fn build_select(
        &mut self,
        select_clause: ast::SelectClause<'a>,
        where_clause: ast::GraphPattern<'a>,
        solution_modifier: ast::SolutionModifier<'a>,
        values_clause: Option<ast::ValuesClause<'a>>,
        is_select_explicit: bool,
    ) -> Result<QueryExpression, AlgebraBuilderError> {
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
                let expression = self.build_expression_without_aggregates(
                    expression,
                    "Aggregation functions cannot be used in GROUP BY",
                )?;
                let variable = variable.map(Self::build_variable);
                if let Some(variable) = variable {
                    // Explicit renaming
                    p = QueryExpression::Extend {
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
                    let variable = self.variable_allocator.fresh_variable("g");
                    p = QueryExpression::Extend {
                        inner: Box::new(p),
                        variable: variable.clone(),
                        expression,
                    };
                    variables.push(variable);
                }
            }
            p = QueryExpression::Group {
                inner: Box::new(p),
                variables,
                aggregates,
            };
        }

        // HAVING
        if let Some(expr) = having_expression {
            p = QueryExpression::Filter {
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
                    p = QueryExpression::Extend {
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
                if !projection_variables.contains(v)
                    && !self
                        .variable_allocator
                        .allocated_variable_names
                        .contains(v.as_str())
                {
                    projection_variables.push(v.clone());
                }
            });
            projection_variables.sort();
        }

        let mut m = p;

        // ORDER BY
        if !order_expressions.is_empty() {
            m = QueryExpression::OrderBy {
                inner: Box::new(m),
                expression: order_expressions,
            };
        }

        // PROJECT
        m = QueryExpression::Project {
            inner: Box::new(m),
            variables: projection_variables,
        };
        match select_clause.option {
            ast::SelectionOption::Distinct => m = QueryExpression::Distinct { inner: Box::new(m) },
            ast::SelectionOption::Reduced => m = QueryExpression::Reduced { inner: Box::new(m) },
            ast::SelectionOption::Default => (),
        }

        // OFFSET LIMIT
        if let Some(ast::LimitOffsetClauses { limit, offset }) =
            solution_modifier.limit_offset_clauses
        {
            m = QueryExpression::Slice {
                inner: Box::new(m),
                offset: if let Some(offset) = offset {
                    offset.inner.parse().map_err(|_| {
                        AlgebraBuilderError::new(
                            offset.span,
                            format!("OFFSET must be an integer, found '{}'", offset.inner),
                        )
                    })?
                } else {
                    0
                },
                limit: if let Some(limit) = limit {
                    Some(limit.inner.parse().map_err(|_| {
                        AlgebraBuilderError::new(
                            limit.span,
                            format!("LIMIT must be an integer, found '{}'", limit.inner),
                        )
                    })?)
                } else {
                    None
                },
            }
        }
        Ok(m)
    }

    fn build_triple_template(
        &mut self,
        template: Vec<(ast::GraphNodePath<'a>, ast::PropertyListPath<'a>)>,
    ) -> Result<Vec<TripleTemplate>, AlgebraBuilderError> {
        self.build_triple_patterns(template)?
            .into_iter()
            .map(|pattern| {
                Ok(convert_to_triple_template(
                    convert_to_spanned_triple_pattern(pattern)?,
                ))
            })
            .collect()
    }

    fn build_values_clause(
        &mut self,
        values_clause: ast::ValuesClause<'a>,
    ) -> Result<QueryExpression, AlgebraBuilderError> {
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
        Ok(QueryExpression::Values {
            variables,
            bindings,
        })
    }

    fn build_ground_term(
        &mut self,
        data_block_value: ast::DataBlockValue<'a>,
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
        &mut self,
        t: ast::TripleTermData<'a>,
    ) -> Result<GroundTriple, AlgebraBuilderError> {
        Ok(GroundTriple {
            subject: self.build_named_node(t.subject)?,
            predicate: match t.predicate {
                ast::IriOrA::Iri(p) => self.build_named_node(p)?,
                ast::IriOrA::A => rdf::TYPE,
            },
            object: match t.object {
                ast::TripleTermDataObject::Iri(o) => self.build_named_node(o)?.into(),
                ast::TripleTermDataObject::Literal(o) => self.build_literal(o)?.into(),
                ast::TripleTermDataObject::TripleTerm(o) => self.build_triple_term_data(*o)?.into(),
            },
        })
    }

    fn build_graph_pattern(
        &mut self,
        graph_pattern: ast::GraphPattern<'a>,
    ) -> Result<QueryExpression, AlgebraBuilderError> {
        Ok(match graph_pattern {
            ast::GraphPattern::SubSelect(sub_select) => self.build_select(
                sub_select.select_clause,
                sub_select.where_clause,
                sub_select.solution_modifier,
                sub_select.values_clause,
                true,
            )?,
            ast::GraphPattern::Group(elements) => {
                let mut g = QueryExpression::default();
                let mut filter: Option<Expression> = None;
                for element in elements {
                    match element.inner {
                        ast::GraphPatternElement::Optional(p) => {
                            // We remove filters from the inner
                            #[expect(clippy::shadow_same)]
                            let mut p = *p;
                            let mut filters = Vec::new();
                            if let ast::GraphPattern::Group(group) = p {
                                p = ast::GraphPattern::Group(
                                    group
                                        .into_iter()
                                        .filter_map(|element| {
                                            if let ast::GraphPatternElement::Filter(expression) =
                                                element.inner
                                            {
                                                filters.push(expression);
                                                None
                                            } else {
                                                Some(element)
                                            }
                                        })
                                        .collect(),
                                );
                            }
                            g = QueryExpression::LeftJoin {
                                left: Box::new(g),
                                right: Box::new(self.build_graph_pattern(p)?),
                                expression: filters
                                    .into_iter()
                                    .map(|expr| {
                                        self.build_expression_without_aggregates(
                                            expr,
                                            "Aggregation functions cannot be used in FILTER",
                                        )
                                    })
                                    .reduce(|l, r| Ok(Expression::And(Box::new(l?), Box::new(r?))))
                                    .transpose()?,
                            }
                        }
                        ast::GraphPatternElement::Minus(p) => {
                            g = QueryExpression::Minus {
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
                            g = QueryExpression::Extend {
                                inner: Box::new(g),
                                variable,
                                expression: self.build_expression_without_aggregates(
                                    expression,
                                    "Aggregation functions cannot be used in BIND",
                                )?,
                            };
                        }
                        ast::GraphPatternElement::Filter(expr) => {
                            let expr = self.build_expression_without_aggregates(
                                expr,
                                "Aggregation functions cannot be used in FILTER",
                            )?;
                            filter = Some(if let Some(f) = filter {
                                Expression::And(Box::new(f), Box::new(expr))
                            } else {
                                expr
                            })
                        }
                        ast::GraphPatternElement::Triples(triples) => {
                            let mut translated_triple_patterns = Vec::new();
                            for pattern in self.build_triple_patterns(triples)? {
                                match pattern {
                                    SpannedTripleOrPathPattern::Triple(t) => {
                                        translated_triple_patterns.push(
                                            TripleOrPathPattern::Triple(
                                                self.convert_triple_pattern(t),
                                            ),
                                        )
                                    }
                                    SpannedTripleOrPathPattern::Path {
                                        subject,
                                        path,
                                        object,
                                    } => {
                                        let subject = self.convert_term_pattern(subject);
                                        let object = self.convert_term_pattern(object);
                                        self.add_path_to_patterns(
                                            subject,
                                            path.inner,
                                            object,
                                            &mut translated_triple_patterns,
                                        );
                                    }
                                }
                            }
                            let mut bgp = Vec::new();
                            for pattern in translated_triple_patterns {
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
                                                QueryExpression::Bgp {
                                                    patterns: take(&mut bgp),
                                                },
                                            );
                                        }
                                        g = new_join(
                                            g,
                                            QueryExpression::Path {
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
                                    QueryExpression::Bgp {
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
                                        Ok(QueryExpression::Union {
                                            left: Box::new(l?),
                                            right: Box::new(r?),
                                        })
                                    })
                                    .unwrap_or_else(|| Ok(QueryExpression::default()))?,
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
                                QueryExpression::Service {
                                    name: self.build_named_node_pattern(name)?,
                                    inner: Box::new(self.build_graph_pattern(*pattern)?),
                                    silent,
                                },
                            )
                        }
                        ast::GraphPatternElement::Graph { name, pattern } => {
                            g = new_join(
                                g,
                                QueryExpression::Graph {
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
                            g = QueryExpression::Lateral {
                                left: Box::new(g),
                                right: Box::new(p),
                            }
                        }
                    }
                }

                if let Some(expr) = filter {
                    QueryExpression::Filter {
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
        &mut self,
        expression: ast::OrderCondition<'a>,
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
        &mut self,
        aggregate: Spanned<ast::Aggregate<'a>>,
    ) -> Result<AggregateExpression, AlgebraBuilderError> {
        let (name, expression, distinct, scalarvals) = match aggregate.inner {
            ast::Aggregate::Count(distinct, expression) => {
                if let Some(expression) = expression {
                    (sparql::AGG_COUNT, expression, distinct, BTreeMap::new())
                } else {
                    return Ok(AggregateExpression::CountSolutions { distinct });
                }
            }
            ast::Aggregate::Sum(distinct, expression) => {
                (sparql::AGG_SUM, expression, distinct, BTreeMap::new())
            }
            ast::Aggregate::Min(distinct, expression) => {
                (sparql::AGG_MIN, expression, distinct, BTreeMap::new())
            }
            ast::Aggregate::Max(distinct, expression) => {
                (sparql::AGG_MAX, expression, distinct, BTreeMap::new())
            }
            ast::Aggregate::Avg(distinct, expression) => {
                (sparql::AGG_AVG, expression, distinct, BTreeMap::new())
            }
            ast::Aggregate::Sample(distinct, expression) => {
                (sparql::AGG_SAMPLE, expression, distinct, BTreeMap::new())
            }
            ast::Aggregate::GroupConcat(distinct, expression, separator) => {
                let mut scalarvals = BTreeMap::new();
                if let Some(separator) = separator {
                    scalarvals.insert("separator".into(), Self::build_string(separator)?);
                }
                (sparql::AGG_GROUP_CONCAT, expression, distinct, scalarvals)
            }
        };
        let expr = self.build_expression_without_aggregates(
            *expression,
            "Aggregated expressions cannot be nested",
        )?;
        Ok(AggregateExpression::FunctionCall {
            name,
            expr,
            distinct,
            scalarvals,
        })
    }

    fn build_expression_without_aggregates(
        &mut self,
        expression: Spanned<ast::Expression<'a>>,
        error_message: &'static str,
    ) -> Result<Expression, AlgebraBuilderError> {
        let mut aggregates = Vec::new();
        let span = expression.span;
        let expression = self.build_expression(expression, &mut aggregates)?;
        if !aggregates.is_empty() {
            return Err(AlgebraBuilderError::new(span, error_message));
        }
        Ok(expression)
    }

    fn build_expression(
        &mut self,
        expression: Spanned<ast::Expression<'a>>,
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
            ast::Expression::Equal(l, r) => Expression::FunctionCall(
                sparql::EQUALS,
                vec![
                    self.build_expression(*l, aggregates)?,
                    self.build_expression(*r, aggregates)?,
                ],
            ),
            ast::Expression::NotEqual(l, r) => Expression::FunctionCall(
                sparql::NOT_EQUALS,
                vec![
                    self.build_expression(*l, aggregates)?,
                    self.build_expression(*r, aggregates)?,
                ],
            ),
            ast::Expression::Less(l, r) => Expression::FunctionCall(
                sparql::LESS_THAN,
                vec![
                    self.build_expression(*l, aggregates)?,
                    self.build_expression(*r, aggregates)?,
                ],
            ),
            ast::Expression::LessOrEqual(l, r) => Expression::FunctionCall(
                sparql::LESS_THAN_OR_EQUAL,
                vec![
                    self.build_expression(*l, aggregates)?,
                    self.build_expression(*r, aggregates)?,
                ],
            ),
            ast::Expression::Greater(l, r) => Expression::FunctionCall(
                sparql::GREATER_THAN,
                vec![
                    self.build_expression(*l, aggregates)?,
                    self.build_expression(*r, aggregates)?,
                ],
            ),
            ast::Expression::GreaterOrEqual(l, r) => Expression::FunctionCall(
                sparql::GREATER_THAN_OR_EQUAL,
                vec![
                    self.build_expression(*l, aggregates)?,
                    self.build_expression(*r, aggregates)?,
                ],
            ),
            ast::Expression::In(l, r) => Expression::In(
                Box::new(self.build_expression(*l, aggregates)?),
                r.into_iter()
                    .map(|e| self.build_expression(e, aggregates))
                    .collect::<Result<_, _>>()?,
            ),
            ast::Expression::NotIn(l, r) => Expression::FunctionCall(
                sparql::LOGICAL_NOT,
                vec![Expression::In(
                    Box::new(self.build_expression(*l, aggregates)?),
                    r.into_iter()
                        .map(|e| self.build_expression(e, aggregates))
                        .collect::<Result<_, _>>()?,
                )],
            ),
            ast::Expression::Add(l, r) => Expression::FunctionCall(
                sparql::ADD,
                vec![
                    self.build_expression(*l, aggregates)?,
                    self.build_expression(*r, aggregates)?,
                ],
            ),
            ast::Expression::Subtract(l, r) => Expression::FunctionCall(
                sparql::SUBTRACT,
                vec![
                    self.build_expression(*l, aggregates)?,
                    self.build_expression(*r, aggregates)?,
                ],
            ),
            ast::Expression::Multiply(l, r) => Expression::FunctionCall(
                sparql::MULTIPLY,
                vec![
                    self.build_expression(*l, aggregates)?,
                    self.build_expression(*r, aggregates)?,
                ],
            ),
            ast::Expression::Divide(l, r) => Expression::FunctionCall(
                sparql::DIVIDE,
                vec![
                    self.build_expression(*l, aggregates)?,
                    self.build_expression(*r, aggregates)?,
                ],
            ),
            ast::Expression::UnaryPlus(e) => Expression::FunctionCall(
                sparql::UNARY_PLUS,
                vec![self.build_expression(*e, aggregates)?],
            ),
            ast::Expression::UnaryMinus(e) => Expression::FunctionCall(
                sparql::UNARY_MINUS,
                vec![self.build_expression(*e, aggregates)?],
            ),
            ast::Expression::Not(e) => Expression::FunctionCall(
                sparql::LOGICAL_NOT,
                vec![self.build_expression(*e, aggregates)?],
            ),
            ast::Expression::Bound(v) => Expression::Bound(Self::build_variable(v)),
            ast::Expression::Aggregate(aggregate) => {
                let aggregate = self.build_aggregate(expression.span.make_wrapped(aggregate))?;
                self.register_aggregate(aggregate, aggregates).into()
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
                let arity = function_arity(name);
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
                    ast::BuiltInName::SameTerm => sparql::SAME_TERM,
                    ast::BuiltInName::Str => sparql::STR,
                    ast::BuiltInName::Lang => sparql::LANG,
                    ast::BuiltInName::LangMatches => sparql::LANG_MATCHES,
                    ast::BuiltInName::Datatype => sparql::DATATYPE,
                    ast::BuiltInName::Iri => sparql::IRI,
                    ast::BuiltInName::Uri => sparql::URI,
                    ast::BuiltInName::BNode => sparql::BNODE,
                    ast::BuiltInName::Rand => sparql::RAND,
                    ast::BuiltInName::Abs => sparql::ABS,
                    ast::BuiltInName::Ceil => sparql::CEIL,
                    ast::BuiltInName::Floor => sparql::FLOOR,
                    ast::BuiltInName::Round => sparql::ROUND,
                    ast::BuiltInName::Concat => sparql::CONCAT,
                    ast::BuiltInName::SubStr => sparql::SUBSTR,
                    ast::BuiltInName::StrLen => sparql::STRLEN,
                    ast::BuiltInName::Replace => sparql::REPLACE,
                    ast::BuiltInName::UCase => sparql::UCASE,
                    ast::BuiltInName::LCase => sparql::LCASE,
                    ast::BuiltInName::EncodeForUri => sparql::ENCODE_FOR_URI,
                    ast::BuiltInName::Contains => sparql::CONTAINS,
                    ast::BuiltInName::StrStarts => sparql::STRSTARTS,
                    ast::BuiltInName::StrEnds => sparql::STRENDS,
                    ast::BuiltInName::StrBefore => sparql::STRBEFORE,
                    ast::BuiltInName::StrAfter => sparql::STRAFTER,
                    ast::BuiltInName::Year => sparql::YEAR,
                    ast::BuiltInName::Month => sparql::MONTH,
                    ast::BuiltInName::Day => sparql::DAY,
                    ast::BuiltInName::Hours => sparql::HOURS,
                    ast::BuiltInName::Minutes => sparql::MINUTES,
                    ast::BuiltInName::Seconds => sparql::SECONDS,
                    ast::BuiltInName::Timezone => sparql::TIMEZONE,
                    ast::BuiltInName::Tz => sparql::TZ,
                    ast::BuiltInName::Now => sparql::NOW,
                    ast::BuiltInName::Uuid => sparql::UUID,
                    ast::BuiltInName::StrUuid => sparql::STRUUID,
                    ast::BuiltInName::Md5 => sparql::MD5,
                    ast::BuiltInName::Sha1 => sparql::SHA1,
                    ast::BuiltInName::Sha256 => sparql::SHA256,
                    ast::BuiltInName::Sha384 => sparql::SHA384,
                    ast::BuiltInName::Sha512 => sparql::SHA512,
                    ast::BuiltInName::StrLang => sparql::STRLANG,
                    ast::BuiltInName::StrDt => sparql::STRDT,
                    ast::BuiltInName::IsIri => sparql::IS_IRI,
                    ast::BuiltInName::IsUri => sparql::IS_URI,
                    ast::BuiltInName::IsBlank => sparql::IS_BLANK,
                    ast::BuiltInName::IsLiteral => sparql::IS_LITERAL,
                    ast::BuiltInName::IsNumeric => sparql::IS_NUMERIC,
                    ast::BuiltInName::Regex => sparql::REGEX,
                    #[cfg(feature = "sparql-12")]
                    ast::BuiltInName::Triple => sparql::TRIPLE,
                    #[cfg(feature = "sparql-12")]
                    ast::BuiltInName::Subject => sparql::SUBJECT,
                    #[cfg(feature = "sparql-12")]
                    ast::BuiltInName::Predicate => sparql::PREDICATE,
                    #[cfg(feature = "sparql-12")]
                    ast::BuiltInName::Object => sparql::OBJECT,
                    #[cfg(feature = "sparql-12")]
                    ast::BuiltInName::IsTriple => sparql::IS_TRIPLE,
                    #[cfg(feature = "sparql-12")]
                    ast::BuiltInName::LangDir => sparql::LANGDIR,
                    #[cfg(feature = "sparql-12")]
                    ast::BuiltInName::HasLang => sparql::HAS_LANG,
                    #[cfg(feature = "sparql-12")]
                    ast::BuiltInName::HasLangDir => sparql::HAS_LANGDIR,
                    #[cfg(feature = "sparql-12")]
                    ast::BuiltInName::StrLangDir => sparql::STRLANGDIR,
                    #[cfg(feature = "sep-0002")]
                    ast::BuiltInName::Adjust => sparql::ADJUST,
                };
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
                    let expr = self.build_expression_without_aggregates(
                        args.args.into_iter().next().unwrap(),
                        "Aggregated expressions cannot be nested",
                    )?;
                    return Ok(self
                        .register_aggregate(
                            AggregateExpression::FunctionCall {
                                name,
                                expr,
                                distinct: args.distinct,
                                scalarvals: BTreeMap::new(),
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
                    name,
                    args.args
                        .into_iter()
                        .map(|e| self.build_expression(e, aggregates))
                        .collect::<Result<_, _>>()?,
                )
            }
            ast::Expression::Exists(gp) => {
                Expression::Exists(Box::new(self.build_graph_pattern(*gp)?))
            }
            ast::Expression::NotExists(gp) => Expression::FunctionCall(
                sparql::LOGICAL_NOT,
                vec![Expression::Exists(Box::new(self.build_graph_pattern(*gp)?))],
            ),
        })
    }

    #[cfg(feature = "sparql-12")]
    fn build_expr_triple_term(
        &mut self,
        t: ast::ExprTripleTerm<'a>,
    ) -> Result<Expression, AlgebraBuilderError> {
        Ok(Expression::FunctionCall(
            sparql::TRIPLE,
            vec![
                match t.subject {
                    ast::ExprTripleTermSubject::Iri(s) => self.build_named_node(s)?.into(),
                    ast::ExprTripleTermSubject::Var(s) => Self::build_variable(s).into(),
                },
                match self.build_verb(t.predicate)? {
                    SpannedNamedNodePattern::NamedNode(s) => s.into(),
                    SpannedNamedNodePattern::Variable(v) => v.inner.into(),
                },
                match t.object {
                    ast::ExprTripleTermObject::Iri(o) => self.build_named_node(o)?.into(),
                    ast::ExprTripleTermObject::Literal(o) => self.build_literal(o)?.into(),
                    ast::ExprTripleTermObject::Var(o) => Self::build_variable(o).into(),
                    ast::ExprTripleTermObject::TripleTerm(o) => self.build_expr_triple_term(*o)?,
                },
            ],
        ))
    }

    fn build_triple_patterns(
        &mut self,
        triples: Vec<(ast::GraphNodePath<'a>, ast::PropertyListPath<'a>)>,
    ) -> Result<Vec<SpannedTripleOrPathPattern>, AlgebraBuilderError> {
        let mut patterns = Vec::new();
        for (subject, predicate_objects) in triples {
            let subject = self.build_graph_node_path(subject, &mut patterns)?;
            self.build_property_list_path(&subject, predicate_objects, &mut patterns)?;
        }
        Ok(patterns)
    }

    fn build_property_list_path(
        &mut self,
        subject: &SpannedTermPattern,
        property_list: ast::PropertyListPath<'a>,
        patterns: &mut Vec<SpannedTripleOrPathPattern>,
    ) -> Result<(), AlgebraBuilderError> {
        for (predicate, objects) in property_list {
            let predicate = self.build_verb_or_path(predicate)?;
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
                    VerbOrPath::Verb(predicate) => {
                        patterns.push(SpannedTripleOrPathPattern::Triple(SpannedTriplePattern {
                            subject: subject.clone(),
                            predicate,
                            object,
                        }))
                    }
                    VerbOrPath::Path(path) => patterns.push(SpannedTripleOrPathPattern::Path {
                        subject: subject.clone(),
                        path,
                        object,
                    }),
                }
            }
        }
        Ok(())
    }

    fn build_object_path(
        &mut self,
        #[cfg(feature = "sparql-12")] subject: &SpannedTermPattern,
        #[cfg(feature = "sparql-12")] predicate: &VerbOrPath,
        object_path: ast::ObjectPath<'a>,
        patterns: &mut Vec<SpannedTripleOrPathPattern>,
    ) -> Result<SpannedTermPattern, AlgebraBuilderError> {
        let object = self.build_graph_node_path(object_path.graph_node, patterns)?;
        #[cfg(feature = "sparql-12")]
        {
            let mut current_reifier = None;
            for annotation in object_path.annotations {
                let reifier_to_emit = match annotation.inner {
                    ast::AnnotationPath::Reifier(r) => {
                        let reifier_to_emit = current_reifier;
                        current_reifier = Some(annotation.span.make_wrapped(if let Some(r) = r {
                            self.build_reifier_id_path(r)?
                        } else {
                            SpannedTermPattern::BlankNode(
                                self.blank_node_allocator
                                    .fresh_spanned_blank_node("r", annotation.span),
                            )
                        }));
                        reifier_to_emit
                    }
                    ast::AnnotationPath::AnnotationBlock(a) => {
                        let reifier_to_emit = take(&mut current_reifier).unwrap_or_else(|| {
                            annotation.span.make_wrapped(SpannedTermPattern::BlankNode(
                                self.blank_node_allocator
                                    .fresh_spanned_blank_node("r", annotation.span),
                            ))
                        });
                        self.build_property_list_path(&reifier_to_emit.inner, a, patterns)?;
                        Some(reifier_to_emit)
                    }
                };
                if let Some(reifier) = reifier_to_emit {
                    patterns.push(SpannedTripleOrPathPattern::Triple(build_reifier_pattern(
                        reifier, subject, predicate, &object,
                    )?));
                }
            }
            if let Some(reifier) = current_reifier {
                patterns.push(SpannedTripleOrPathPattern::Triple(build_reifier_pattern(
                    reifier, subject, predicate, &object,
                )?));
            }
        }
        Ok(object)
    }

    #[cfg(feature = "sparql-12")]
    fn build_reifier_id_path(
        &mut self,
        var_or_reifier_id: ast::VarOrReifierId<'a>,
    ) -> Result<SpannedTermPattern, AlgebraBuilderError> {
        Ok(match var_or_reifier_id {
            ast::VarOrReifierId::Var(v) => {
                SpannedTermPattern::Variable(v.span.make_wrapped(Self::build_variable(v)))
            }
            ast::VarOrReifierId::Iri(n) => SpannedTermPattern::NamedNode(self.build_named_node(n)?),
            ast::VarOrReifierId::BlankNode(n) => {
                SpannedTermPattern::BlankNode(self.build_spanned_blank_node(n))
            }
        })
    }

    fn build_graph_node_path(
        &mut self,
        graph_node_path: ast::GraphNodePath<'a>,
        patterns: &mut Vec<SpannedTripleOrPathPattern>,
    ) -> Result<SpannedTermPattern, AlgebraBuilderError> {
        match graph_node_path {
            ast::GraphNodePath::VarOrTerm(var_or_term) => self.build_term_pattern_path(var_or_term),
            ast::GraphNodePath::Collection(elements) => {
                let mut current_list_node = SpannedTermPattern::NamedNode(rdf::NIL);
                for element in elements.inner.into_iter().rev() {
                    let element = self.build_graph_node_path(element, patterns)?;
                    let new_blank_node = SpannedTermPattern::BlankNode(
                        self.blank_node_allocator
                            .fresh_spanned_blank_node("c", elements.span),
                    );
                    patterns.push(SpannedTripleOrPathPattern::Triple(SpannedTriplePattern {
                        subject: new_blank_node.clone(),
                        predicate: SpannedNamedNodePattern::NamedNode(rdf::FIRST),
                        object: element,
                    }));
                    patterns.push(SpannedTripleOrPathPattern::Triple(SpannedTriplePattern {
                        subject: new_blank_node.clone(),
                        predicate: SpannedNamedNodePattern::NamedNode(rdf::REST),
                        object: current_list_node,
                    }));
                    current_list_node = new_blank_node;
                }
                Ok(current_list_node)
            }
            ast::GraphNodePath::BlankNodePropertyList(property_list) => {
                let subject = SpannedTermPattern::BlankNode(
                    self.blank_node_allocator
                        .fresh_spanned_blank_node("b", property_list.span),
                );
                self.build_property_list_path(&subject, property_list.inner, patterns)?;
                Ok(subject)
            }
            #[cfg(feature = "sparql-12")]
            ast::GraphNodePath::ReifiedTriple(t) => {
                let mut extra_patterns = Vec::new();
                let term = self.build_reified_triple_path(t, &mut extra_patterns)?;
                patterns.extend(
                    extra_patterns
                        .into_iter()
                        .map(SpannedTripleOrPathPattern::Triple),
                );
                Ok(term)
            }
        }
    }

    fn build_verb_or_path(
        &mut self,
        var_or_path: ast::VarOrPath<'a>,
    ) -> Result<VerbOrPath, AlgebraBuilderError> {
        Ok(match var_or_path {
            ast::VarOrPath::Var(v) => VerbOrPath::Verb(SpannedNamedNodePattern::Variable(
                v.span.make_wrapped(Self::build_variable(v)),
            )),
            ast::VarOrPath::Path(p) => match p.inner {
                ast::Path::Iri(s) => VerbOrPath::Verb(SpannedNamedNodePattern::NamedNode(
                    self.build_named_node(s)?,
                )),
                ast::Path::A => VerbOrPath::Verb(SpannedNamedNodePattern::NamedNode(rdf::TYPE)),
                path => VerbOrPath::Path(p.span.make_wrapped(self.build_path(path)?)),
            },
        })
    }

    fn build_path(
        &mut self,
        path: ast::Path<'a>,
    ) -> Result<PropertyPathExpression, AlgebraBuilderError> {
        Ok(match path {
            ast::Path::Alternative(l, r) => PropertyPathExpression::Alt(
                Box::new(self.build_path(*l)?),
                Box::new(self.build_path(*r)?),
            ),
            ast::Path::Sequence(l, r) => PropertyPathExpression::Seq(
                Box::new(self.build_path(*l)?),
                Box::new(self.build_path(*r)?),
            ),
            ast::Path::Inverse(p) => PropertyPathExpression::Inv(Box::new(self.build_path(*p)?)),
            ast::Path::ZeroOrOne(p) => {
                PropertyPathExpression::ZeroOrOnePath(Box::new(self.build_path(*p)?))
            }
            ast::Path::ZeroOrMore(p) => {
                PropertyPathExpression::ZeroOrMorePath(Box::new(self.build_path(*p)?))
            }
            ast::Path::OneOrMore(p) => {
                PropertyPathExpression::OneOrMorePath(Box::new(self.build_path(*p)?))
            }
            ast::Path::Iri(p) => PropertyPathExpression::Link(self.build_named_node(p)?),
            ast::Path::A => PropertyPathExpression::Link(rdf::TYPE),
            ast::Path::NegatedPropertySet(nps) => {
                let mut direct = Vec::new();
                let mut inverse = Vec::new();
                for p in nps {
                    match p {
                        ast::PathOneInPropertySet::Iri(p) => direct.push(self.build_named_node(p)?),
                        ast::PathOneInPropertySet::InverseIri(p) => {
                            inverse.push(self.build_named_node(p)?)
                        }
                        ast::PathOneInPropertySet::A => direct.push(rdf::TYPE),
                        ast::PathOneInPropertySet::InverseA => inverse.push(rdf::TYPE),
                    }
                }
                if inverse.is_empty() {
                    PropertyPathExpression::Nps(direct)
                } else if direct.is_empty() {
                    PropertyPathExpression::Inv(Box::new(PropertyPathExpression::Nps(inverse)))
                } else {
                    PropertyPathExpression::Alt(
                        Box::new(PropertyPathExpression::Nps(direct)),
                        Box::new(PropertyPathExpression::Inv(Box::new(
                            PropertyPathExpression::Nps(inverse),
                        ))),
                    )
                }
            }
            ast::Path::Nested(p) => self.build_path(*p)?,
        })
    }

    #[cfg(feature = "sparql-12")]
    fn build_verb(
        &mut self,
        verb: ast::Verb<'a>,
    ) -> Result<SpannedNamedNodePattern, AlgebraBuilderError> {
        Ok(match verb {
            ast::Verb::Var(v) => {
                SpannedNamedNodePattern::Variable(v.span.make_wrapped(Self::build_variable(v)))
            }
            ast::Verb::Iri(n) => SpannedNamedNodePattern::NamedNode(self.build_named_node(n)?),
            ast::Verb::A => SpannedNamedNodePattern::NamedNode(rdf::TYPE),
        })
    }

    #[cfg(feature = "sparql-12")]
    fn build_verb_path(
        &mut self,
        verb: ast::Verb<'a>,
    ) -> Result<SpannedNamedNodePattern, AlgebraBuilderError> {
        Ok(match verb {
            ast::Verb::Var(v) => {
                SpannedNamedNodePattern::Variable(v.span.make_wrapped(Self::build_variable(v)))
            }
            ast::Verb::Iri(n) => SpannedNamedNodePattern::NamedNode(self.build_named_node(n)?),
            ast::Verb::A => SpannedNamedNodePattern::NamedNode(rdf::TYPE),
        })
    }

    fn build_term_pattern_path(
        &mut self,
        var_or_term: ast::VarOrTerm<'a>,
    ) -> Result<SpannedTermPattern, AlgebraBuilderError> {
        Ok(match var_or_term {
            ast::VarOrTerm::Var(v) => {
                SpannedTermPattern::Variable(v.span.make_wrapped(Self::build_variable(v)))
            }
            ast::VarOrTerm::Iri(n) => SpannedTermPattern::NamedNode(self.build_named_node(n)?),
            ast::VarOrTerm::BlankNode(n) => {
                SpannedTermPattern::BlankNode(self.build_spanned_blank_node(n))
            }
            ast::VarOrTerm::Literal(l) => {
                SpannedTermPattern::Literal(l.span.make_wrapped(self.build_literal(l.inner)?))
            }
            ast::VarOrTerm::Nil => SpannedTermPattern::NamedNode(rdf::NIL),
            #[cfg(feature = "sparql-12")]
            ast::VarOrTerm::TripleTerm(t) => SpannedTermPattern::Triple(
                t.span
                    .make_wrapped(Box::new(self.build_triple_term_path(t.inner)?)),
            ),
        })
    }

    #[cfg(feature = "sparql-12")]
    fn build_triple_term_path(
        &mut self,
        triple_term: ast::TripleTerm<'a>,
    ) -> Result<SpannedTriplePattern, AlgebraBuilderError> {
        Ok(SpannedTriplePattern {
            subject: self.build_term_pattern_path(triple_term.subject)?,
            predicate: self.build_verb_path(triple_term.predicate)?,
            object: self.build_term_pattern_path(triple_term.object)?,
        })
    }

    #[cfg(feature = "sparql-12")]
    fn build_reified_triple_path(
        &mut self,
        triple: Spanned<ast::ReifiedTriple<'a>>,
        patterns: &mut Vec<SpannedTriplePattern>,
    ) -> Result<SpannedTermPattern, AlgebraBuilderError> {
        let span = triple.span;
        let triple = triple.inner;
        let reifier = triple
            .reifier
            .map(|r| self.build_reifier_id_path(r))
            .transpose()?
            .unwrap_or_else(|| {
                SpannedTermPattern::BlankNode(
                    self.blank_node_allocator
                        .fresh_spanned_blank_node("r", span),
                )
            });
        let triple = SpannedTriplePattern {
            subject: self.build_reified_triple_subject_or_object_path(triple.subject, patterns)?,
            predicate: self.build_verb_path(triple.predicate)?,
            object: self.build_reified_triple_subject_or_object_path(triple.object, patterns)?,
        };
        patterns.push(SpannedTriplePattern {
            subject: reifier.clone(),
            predicate: SpannedNamedNodePattern::NamedNode(rdf::REIFIES),
            object: SpannedTermPattern::Triple(span.make_wrapped(Box::new(triple))),
        });
        Ok(reifier)
    }

    #[cfg(feature = "sparql-12")]
    fn build_reified_triple_subject_or_object_path(
        &mut self,
        triple_term: ast::ReifiedTripleSubjectOrObject<'a>,
        patterns: &mut Vec<SpannedTriplePattern>,
    ) -> Result<SpannedTermPattern, AlgebraBuilderError> {
        Ok(match triple_term {
            ast::ReifiedTripleSubjectOrObject::Var(v) => {
                SpannedTermPattern::Variable(v.span.make_wrapped(Self::build_variable(v)))
            }
            ast::ReifiedTripleSubjectOrObject::Iri(n) => {
                SpannedTermPattern::NamedNode(self.build_named_node(n)?)
            }
            ast::ReifiedTripleSubjectOrObject::BlankNode(n) => {
                SpannedTermPattern::BlankNode(self.build_spanned_blank_node(n))
            }
            ast::ReifiedTripleSubjectOrObject::Literal(l) => {
                SpannedTermPattern::Literal(l.span.make_wrapped(self.build_literal(l.inner)?))
            }
            ast::ReifiedTripleSubjectOrObject::ReifiedTriple(t) => {
                self.build_reified_triple_path(*t, patterns)?
            }
            ast::ReifiedTripleSubjectOrObject::TripleTerm(t) => SpannedTermPattern::Triple(
                t.span
                    .make_wrapped(Box::new(self.build_triple_term_path(t.inner)?)),
            ),
        })
    }

    fn build_named_node_pattern(
        &mut self,
        var_or_iri: ast::VarOrIri<'a>,
    ) -> Result<NamedNodePattern, AlgebraBuilderError> {
        Ok(match var_or_iri {
            ast::VarOrIri::Var(v) => Self::build_variable(v).into(),
            ast::VarOrIri::Iri(n) => self.build_named_node(n)?.into(),
        })
    }

    fn build_variable(var: Spanned<ast::Var<'a>>) -> Variable {
        Variable::new_unchecked(OxString::new_owned(var.0))
    }

    fn build_literal(&mut self, literal: ast::Literal<'a>) -> Result<Literal, AlgebraBuilderError> {
        Ok(match literal {
            ast::Literal::Boolean(v) => {
                Literal::new_typed_literal(if v { "true" } else { "false" }, xsd::BOOLEAN)
            }
            ast::Literal::Integer(v) => {
                Literal::new_typed_literal(OxString::new_owned(v), xsd::INTEGER)
            }
            ast::Literal::Decimal(v) => {
                Literal::new_typed_literal(OxString::new_owned(v), xsd::DECIMAL)
            }
            ast::Literal::Double(v) => {
                Literal::new_typed_literal(OxString::new_owned(v), xsd::DOUBLE)
            }
            ast::Literal::String(v) => Literal::new_simple_literal(Self::build_string(v)?),
            ast::Literal::LangString(v, l) => Literal::new_language_tagged_literal(
                Self::build_string(v)?,
                OxString::new_owned(l.inner),
            )
            .map_err(|e| {
                AlgebraBuilderError::new(l.span, format!("Invalid language tag '{}': {e}", l.inner))
            })?,
            #[cfg(feature = "sparql-12")]
            ast::Literal::DirLangString(v, l) => Literal::new_directional_language_tagged_literal(
                Self::build_string(v)?,
                OxString::new_owned(l.inner.0),
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

    fn build_spanned_blank_node(
        &mut self,
        blank_node: Spanned<ast::BlankNode<'a>>,
    ) -> Spanned<BlankNode> {
        if let Some(id) = blank_node.inner.0 {
            blank_node
                .span
                .make_wrapped(BlankNode::new_unchecked(OxString::new_owned(id)))
        } else {
            self.blank_node_allocator
                .fresh_spanned_blank_node("a", blank_node.span)
        }
    }

    fn build_string(string: Spanned<ast::String<'a>>) -> Result<OxString, AlgebraBuilderError> {
        unescape_string(string.inner.0, string.span)
    }

    fn build_named_node(&mut self, iri: ast::Iri<'a>) -> Result<NamedNode, AlgebraBuilderError> {
        Ok(NamedNode::new_unchecked(match iri {
            ast::Iri::IriRef(iri) => self.build_iri(iri),
            ast::Iri::PrefixedName(pname) => self.build_prefixed_name(pname),
        }?))
    }

    fn build_prefixed_name(
        &mut self,
        pname: Spanned<ast::PrefixedName<'a>>,
    ) -> Result<OxString, AlgebraBuilderError> {
        if let Some(base) = self.prefixes.get(pname.inner.0) {
            let (pname_local, might_be_invalid_iri) = unescape_local_name(pname.inner.1);
            let iri = OxString::concat([base.as_str(), pname_local.as_ref()]);
            if might_be_invalid_iri || base.path().is_empty() {
                // We validate again. We always validate if the local part might be the IRI authority.
                Iri::parse(iri.as_str()).map_err(|e| {
                    AlgebraBuilderError::new(
                        pname.span,
                        format!(
                            "Invalid IRI built from '{}:{}': {e}",
                            pname.inner.0, pname.inner.1
                        ),
                    )
                })?;
            }
            Ok(iri)
        } else {
            Err(AlgebraBuilderError::new(
                pname.span,
                format!("The prefix '{}:' is not defined", pname.inner.0),
            ))
        }
    }

    fn build_iri(
        &mut self,
        iri: Spanned<ast::IriRef<'a>>,
    ) -> Result<OxString, AlgebraBuilderError> {
        let iri_value = unescape_iriref(iri.inner.0, iri.span)?;
        let iri_ref = IriRef::parse(iri_value.clone()).map_err(|e| {
            AlgebraBuilderError::new(iri.span, format!("Invalid IRI '{iri_value}': {e}"))
        })?;
        if iri_ref.is_absolute() {
            Ok(OxString::new_owned(&iri_ref.into_inner()))
        } else if let Some(base_iri) = &self.base_iri {
            self.buffer.clear();
            base_iri
                .resolve_into(&iri_ref, &mut self.buffer)
                .map_err(|e| {
                    AlgebraBuilderError::new(iri.span, format!("Invalid IRI '{iri_value}': {e}"))
                })?;
            Ok(OxString::new_owned(&self.buffer))
        } else {
            Err(AlgebraBuilderError::new(
                iri.span,
                format!("Found a relative IRI '{iri_value}' but no BASE is provided"),
            ))
        }
    }

    pub fn build_update(mut self, update: ast::Update<'a>) -> Result<Update, AlgebraBuilderError> {
        valid_update_operation_blank_node_id_syntax_restrictions(&update)?;
        self.blank_node_allocator.reset_with_update(&update);

        let mut operations = Vec::new();
        for (prologue, update1) in update.operations {
            self.variable_allocator.reset_with_update1(&update1);
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

                    let mut graph_pattern = QueryExpression::default();
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
                            subject: pattern.subject.clone(),
                            predicate: pattern.predicate.clone(),
                            object: pattern.object.clone(),
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
                            using = Some(QueryDatasetSpecification {
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
        &mut self,
        quads: ast::QuadPatterns<'a>,
    ) -> Result<Vec<QuadPattern>, AlgebraBuilderError> {
        let mut patterns = Vec::new();
        for (graph_name, triples) in quads {
            let graph_name = if let Some(graph_name) = graph_name {
                self.build_named_node_pattern(graph_name)?.into()
            } else {
                GraphNamePattern::DefaultGraph
            };
            for pattern in self.build_triple_patterns(triples)? {
                let triple =
                    convert_to_ground_triple_pattern(convert_to_spanned_triple_pattern(pattern)?)?;
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

    fn build_quads(
        &mut self,
        quads: ast::QuadPatterns<'a>,
    ) -> Result<Vec<Quad>, AlgebraBuilderError> {
        let mut result = Vec::new();
        for (graph_name, triples) in quads {
            let graph_name = if let Some(graph_name) = graph_name {
                match graph_name {
                    ast::VarOrIri::Iri(n) => self.build_named_node(n)?.into(),
                    ast::VarOrIri::Var(v) => {
                        return Err(AlgebraBuilderError::new(
                            v.span,
                            "Variables are not allowed in INSERT DATA",
                        ));
                    }
                }
            } else {
                GraphName::DefaultGraph
            };
            for pattern in self.build_triple_patterns(triples)? {
                let triple = convert_to_spanned_triple_pattern(pattern)?;
                result.push(Quad {
                    subject: convert_to_named_or_blank_node(triple.subject)?,
                    predicate: convert_to_named_node(triple.predicate)?,
                    object: convert_to_term(triple.object)?,
                    graph_name: graph_name.clone(),
                });
            }
        }
        Ok(result)
    }

    fn build_ground_quads(
        &mut self,
        quads: ast::QuadPatterns<'a>,
    ) -> Result<Vec<GroundQuad>, AlgebraBuilderError> {
        let mut result = Vec::new();
        for (graph_name, triples) in quads {
            let graph_name = if let Some(graph_name) = graph_name {
                match graph_name {
                    ast::VarOrIri::Iri(n) => self.build_named_node(n)?.into(),
                    ast::VarOrIri::Var(v) => {
                        return Err(AlgebraBuilderError::new(
                            v.span,
                            "Variables are not allowed in DELETE DATA",
                        ));
                    }
                }
            } else {
                GraphName::DefaultGraph
            };
            for pattern in self.build_triple_patterns(triples)? {
                let triple = convert_to_spanned_triple_pattern(pattern)?;
                result.push(GroundQuad {
                    subject: convert_to_ground_named_node(triple.subject)?,
                    predicate: convert_to_ground_predicate(triple.predicate)?,
                    object: convert_to_ground_term(triple.object)?,
                    graph_name: graph_name.clone(),
                });
            }
        }
        Ok(result)
    }

    fn build_quad_patterns(
        &mut self,
        quads: ast::QuadPatterns<'a>,
    ) -> Result<Vec<QuadTemplate>, AlgebraBuilderError> {
        let mut patterns = Vec::new();
        for (graph_name, triples) in quads {
            let graph_name = if let Some(graph_name) = graph_name {
                self.build_named_node_pattern(graph_name)?.into()
            } else {
                GraphNamePattern::DefaultGraph
            };
            for triple in self.build_triple_template(triples)? {
                patterns.push(QuadTemplate {
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
        &mut self,
        graph_ref_all: ast::GraphOrDefault<'a>,
    ) -> Result<GraphName, AlgebraBuilderError> {
        Ok(match graph_ref_all {
            ast::GraphOrDefault::Graph(n) => self.build_named_node(n)?.into(),
            ast::GraphOrDefault::Default => GraphName::DefaultGraph,
        })
    }

    fn build_graph_target(
        &mut self,
        graph_ref_all: ast::GraphRefAll<'a>,
    ) -> Result<GraphTarget, AlgebraBuilderError> {
        Ok(match graph_ref_all {
            ast::GraphRefAll::Graph(n) => self.build_named_node(n)?.into(),
            ast::GraphRefAll::Default => GraphTarget::DefaultGraph,
            ast::GraphRefAll::Named => GraphTarget::NamedGraphs,
            ast::GraphRefAll::All => GraphTarget::AllGraphs,
        })
    }

    fn register_aggregate(
        &mut self,
        agg: AggregateExpression,
        aggregates: &mut Vec<(Variable, AggregateExpression)>,
    ) -> Variable {
        aggregates
            .iter()
            .find_map(|(v, a)| (*a == agg).then_some(v))
            .cloned()
            .unwrap_or_else(|| {
                let new_var = self.variable_allocator.fresh_variable("agg");
                aggregates.push((new_var.clone(), agg));
                new_var
            })
    }

    fn convert_triple_pattern(&mut self, triple: SpannedTriplePattern) -> TriplePattern {
        TriplePattern::new(
            self.convert_term_pattern(triple.subject),
            match triple.predicate {
                SpannedNamedNodePattern::NamedNode(n) => NamedNodePattern::NamedNode(n),
                SpannedNamedNodePattern::Variable(v) => NamedNodePattern::Variable(v.inner),
            },
            self.convert_term_pattern(triple.object),
        )
    }

    fn convert_term_pattern(&mut self, term: SpannedTermPattern) -> TermPattern {
        match term {
            SpannedTermPattern::NamedNode(n) => TermPattern::NamedNode(n),
            SpannedTermPattern::BlankNode(n) => self
                .variable_allocator
                .map_blank_node_id(n.inner.as_str())
                .into(),
            SpannedTermPattern::Literal(l) => TermPattern::Literal(l.inner),
            SpannedTermPattern::Variable(v) => TermPattern::Variable(v.inner),
            #[cfg(feature = "sparql-12")]
            SpannedTermPattern::Triple(t) => {
                TermPattern::Triple(Box::new(self.convert_triple_pattern(*t.inner)))
            }
        }
    }

    fn add_path_to_patterns(
        &mut self,
        subject: TermPattern,
        path: PropertyPathExpression,
        object: TermPattern,
        patterns: &mut Vec<TripleOrPathPattern>,
    ) {
        match path {
            PropertyPathExpression::Link(predicate) => patterns.push(TripleOrPathPattern::Triple(
                TriplePattern::new(subject, predicate, object),
            )),
            PropertyPathExpression::Inv(path) => {
                self.add_path_to_patterns(object, *path, subject, patterns)
            }
            PropertyPathExpression::Seq(path1, path2) => {
                let middle = self.variable_allocator.fresh_variable("m");
                self.add_path_to_patterns(subject, *path1, middle.clone().into(), patterns);
                self.add_path_to_patterns(middle.into(), *path2, object, patterns)
            }
            _ => patterns.push(TripleOrPathPattern::Path {
                subject,
                path,
                object,
            }),
        }
    }
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
        Expression::Or(a, b) | Expression::And(a, b) => {
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

fn new_join(l: QueryExpression, r: QueryExpression) -> QueryExpression {
    // Avoid to output empty BGPs
    if let QueryExpression::Bgp { patterns: pl } = &l {
        if pl.is_empty() {
            return r;
        }
    }
    if let QueryExpression::Bgp { patterns: pr } = &r {
        if pr.is_empty() {
            return l;
        }
    }

    match (l, r) {
        (QueryExpression::Bgp { patterns: mut pl }, QueryExpression::Bgp { patterns: pr }) => {
            pl.extend(pr);
            QueryExpression::Bgp { patterns: pl }
        }
        (QueryExpression::Bgp { patterns }, other) | (other, QueryExpression::Bgp { patterns })
            if patterns.is_empty() =>
        {
            other
        }
        (l, r) => QueryExpression::Join {
            left: Box::new(l),
            right: Box::new(r),
        },
    }
}

fn wrap_bpg_in_graph(bgp: Vec<TriplePattern>, graph_name: GraphNamePattern) -> QueryExpression {
    if bgp.is_empty() {
        return QueryExpression::default();
    }
    let bgp = QueryExpression::Bgp { patterns: bgp };
    match graph_name {
        GraphNamePattern::NamedNode(g) => QueryExpression::Graph {
            name: g.into(),
            inner: Box::new(bgp),
        },
        GraphNamePattern::DefaultGraph => bgp,
        GraphNamePattern::Variable(g) => QueryExpression::Graph {
            name: g.into(),
            inner: Box::new(bgp),
        },
    }
}

enum SpannedTripleOrPathPattern {
    Triple(SpannedTriplePattern),
    Path {
        subject: SpannedTermPattern,
        path: Spanned<PropertyPathExpression>,
        object: SpannedTermPattern,
    },
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
struct SpannedTriplePattern {
    subject: SpannedTermPattern,
    predicate: SpannedNamedNodePattern,
    object: SpannedTermPattern,
}

#[derive(Clone)]
enum SpannedNamedNodePattern {
    NamedNode(NamedNode),
    Variable(Spanned<Variable>),
}

#[derive(Clone)]
enum SpannedTermPattern {
    NamedNode(NamedNode),
    BlankNode(Spanned<BlankNode>),
    Literal(Spanned<Literal>),
    Variable(Spanned<Variable>),
    #[cfg(feature = "sparql-12")]
    Triple(Spanned<Box<SpannedTriplePattern>>),
}

#[derive(Clone)]
enum VerbOrPath {
    Verb(SpannedNamedNodePattern),
    Path(Spanned<PropertyPathExpression>),
}

#[cfg(feature = "sparql-12")]
fn build_reifier_pattern(
    reifier: Spanned<SpannedTermPattern>,
    subject: &SpannedTermPattern,
    predicate: &VerbOrPath,
    object: &SpannedTermPattern,
) -> Result<SpannedTriplePattern, AlgebraBuilderError> {
    let predicate = match predicate {
        VerbOrPath::Verb(predicate) => predicate.clone(),
        VerbOrPath::Path(_) => {
            return Err(AlgebraBuilderError::new(
                reifier.span,
                "Reifiers can only be used on triples and not on property paths",
            ));
        }
    };
    Ok(SpannedTriplePattern {
        subject: reifier.inner,
        predicate: SpannedNamedNodePattern::NamedNode(rdf::REIFIES),
        object: SpannedTermPattern::Triple(reifier.span.make_wrapped(Box::new(
            SpannedTriplePattern {
                subject: subject.clone(),
                predicate,
                object: object.clone(),
            },
        ))),
    })
}

fn convert_to_spanned_triple_pattern(
    pattern: SpannedTripleOrPathPattern,
) -> Result<SpannedTriplePattern, AlgebraBuilderError> {
    Ok(match pattern {
        SpannedTripleOrPathPattern::Triple(triple) => triple,
        SpannedTripleOrPathPattern::Path { path, .. } => {
            return Err(AlgebraBuilderError::new(
                path.span,
                "Property paths are not allowed in CONSTRUCT, INSERT or DELETE",
            ));
        }
    })
}

fn convert_to_ground_triple_pattern(
    triple: SpannedTriplePattern,
) -> Result<TriplePattern, AlgebraBuilderError> {
    Ok(TriplePattern::new(
        convert_to_ground_term_pattern(triple.subject)?,
        match triple.predicate {
            SpannedNamedNodePattern::NamedNode(n) => NamedNodePattern::NamedNode(n),
            SpannedNamedNodePattern::Variable(v) => NamedNodePattern::Variable(v.inner),
        },
        convert_to_ground_term_pattern(triple.object)?,
    ))
}

fn convert_to_ground_term_pattern(
    term: SpannedTermPattern,
) -> Result<TermPattern, AlgebraBuilderError> {
    Ok(match term {
        SpannedTermPattern::NamedNode(n) => n.into(),
        SpannedTermPattern::BlankNode(n) => {
            return Err(AlgebraBuilderError::new(
                n.span,
                "Blank nodes are not allowed in DELETE",
            ));
        }
        SpannedTermPattern::Literal(l) => l.inner.into(),
        SpannedTermPattern::Variable(v) => v.inner.into(),
        #[cfg(feature = "sparql-12")]
        SpannedTermPattern::Triple(t) => convert_to_ground_triple_pattern(*t.inner)?.into(),
    })
}

fn convert_to_named_node(node: SpannedNamedNodePattern) -> Result<NamedNode, AlgebraBuilderError> {
    match node {
        SpannedNamedNodePattern::NamedNode(n) => Ok(n),
        SpannedNamedNodePattern::Variable(v) => Err(AlgebraBuilderError::new(
            v.span,
            "Variables are not allowed in INSERT DATA",
        )),
    }
}

fn convert_to_named_or_blank_node(
    term: SpannedTermPattern,
) -> Result<NamedOrBlankNode, AlgebraBuilderError> {
    match term {
        SpannedTermPattern::NamedNode(n) => Ok(n.into()),
        SpannedTermPattern::BlankNode(n) => Ok(n.inner.into()),
        SpannedTermPattern::Variable(v) => Err(AlgebraBuilderError::new(
            v.span,
            "Variables are not allowed in INSERT DATA",
        )),
        SpannedTermPattern::Literal(l) => Err(AlgebraBuilderError::new(
            l.span,
            "Literals are not allowed as subjects in INSERT DATA",
        )),
        #[cfg(feature = "sparql-12")]
        SpannedTermPattern::Triple(t) => Err(AlgebraBuilderError::new(
            t.span,
            "Triple terms are not allowed as subjects in INSERT DATA",
        )),
    }
}

fn convert_to_term(term: SpannedTermPattern) -> Result<Term, AlgebraBuilderError> {
    Ok(match term {
        SpannedTermPattern::NamedNode(n) => n.into(),
        SpannedTermPattern::BlankNode(n) => n.inner.into(),
        SpannedTermPattern::Literal(l) => l.inner.into(),
        SpannedTermPattern::Variable(v) => {
            return Err(AlgebraBuilderError::new(
                v.span,
                "Variables are not allowed in INSERT DATA",
            ));
        }
        #[cfg(feature = "sparql-12")]
        SpannedTermPattern::Triple(t) => {
            let triple = *t.inner;
            Triple::new(
                convert_to_named_or_blank_node(triple.subject)?,
                convert_to_named_node(triple.predicate)?,
                convert_to_term(triple.object)?,
            )
            .into()
        }
    })
}

fn convert_to_ground_predicate(
    node: SpannedNamedNodePattern,
) -> Result<NamedNode, AlgebraBuilderError> {
    match node {
        SpannedNamedNodePattern::NamedNode(n) => Ok(n),
        SpannedNamedNodePattern::Variable(v) => Err(AlgebraBuilderError::new(
            v.span,
            "Variables are not allowed in DELETE DATA",
        )),
    }
}

fn convert_to_ground_named_node(
    term: SpannedTermPattern,
) -> Result<NamedNode, AlgebraBuilderError> {
    match term {
        SpannedTermPattern::NamedNode(n) => Ok(n),
        SpannedTermPattern::BlankNode(n) => Err(AlgebraBuilderError::new(
            n.span,
            "Blank nodes are not allowed in DELETE DATA",
        )),
        SpannedTermPattern::Variable(v) => Err(AlgebraBuilderError::new(
            v.span,
            "Variables are not allowed in DELETE DATA",
        )),
        SpannedTermPattern::Literal(l) => Err(AlgebraBuilderError::new(
            l.span,
            "Literals are not allowed as subjects in DELETE DATA",
        )),
        #[cfg(feature = "sparql-12")]
        SpannedTermPattern::Triple(t) => Err(AlgebraBuilderError::new(
            t.span,
            "Triple terms are not allowed as subjects in DELETE DATA",
        )),
    }
}

fn convert_to_ground_term(term: SpannedTermPattern) -> Result<GroundTerm, AlgebraBuilderError> {
    Ok(match term {
        SpannedTermPattern::NamedNode(n) => n.into(),
        SpannedTermPattern::Literal(l) => l.inner.into(),
        SpannedTermPattern::BlankNode(n) => {
            return Err(AlgebraBuilderError::new(
                n.span,
                "Blank nodes are not allowed in DELETE DATA",
            ));
        }
        SpannedTermPattern::Variable(v) => {
            return Err(AlgebraBuilderError::new(
                v.span,
                "Variables are not allowed in DELETE DATA",
            ));
        }
        #[cfg(feature = "sparql-12")]
        SpannedTermPattern::Triple(t) => {
            let triple = *t.inner;
            GroundTriple {
                subject: convert_to_ground_named_node(triple.subject)?,
                predicate: convert_to_ground_predicate(triple.predicate)?,
                object: convert_to_ground_term(triple.object)?,
            }
            .into()
        }
    })
}

fn convert_to_triple_template(triple: SpannedTriplePattern) -> TripleTemplate {
    TripleTemplate::new(
        convert_to_term_template(triple.subject),
        match triple.predicate {
            SpannedNamedNodePattern::NamedNode(n) => NamedNodePattern::NamedNode(n),
            SpannedNamedNodePattern::Variable(v) => NamedNodePattern::Variable(v.inner),
        },
        convert_to_term_template(triple.object),
    )
}

fn convert_to_term_template(term: SpannedTermPattern) -> TermTemplate {
    match term {
        SpannedTermPattern::NamedNode(n) => n.into(),
        SpannedTermPattern::BlankNode(n) => n.inner.into(),
        SpannedTermPattern::Literal(l) => l.inner.into(),
        SpannedTermPattern::Variable(v) => v.inner.into(),
        #[cfg(feature = "sparql-12")]
        SpannedTermPattern::Triple(t) => convert_to_triple_template(*t.inner).into(),
    }
}

/// Called on every variable defined using "AS" or "VALUES"
#[cfg(feature = "sep-0006")]
fn add_defined_variables<'a>(pattern: &'a QueryExpression, set: &mut HashSet<&'a Variable>) {
    match pattern {
        QueryExpression::Bgp { .. } | QueryExpression::Path { .. } => {}
        QueryExpression::Join { left, right }
        | QueryExpression::LeftJoin { left, right, .. }
        | QueryExpression::Lateral { left, right }
        | QueryExpression::Union { left, right }
        | QueryExpression::Minus { left, right } => {
            add_defined_variables(left, set);
            add_defined_variables(right, set);
        }
        QueryExpression::Graph { inner, .. } => {
            add_defined_variables(inner, set);
        }
        QueryExpression::Extend {
            inner, variable, ..
        } => {
            set.insert(variable);
            add_defined_variables(inner, set);
        }
        QueryExpression::Group {
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
        QueryExpression::Values { variables, .. } => {
            for v in variables {
                set.insert(v);
            }
        }
        QueryExpression::Project { variables, inner } => {
            let mut inner_variables = HashSet::new();
            add_defined_variables(inner, &mut inner_variables);
            for v in inner_variables {
                if variables.contains(v) {
                    set.insert(v);
                }
            }
        }
        QueryExpression::Service { inner, .. }
        | QueryExpression::Filter { inner, .. }
        | QueryExpression::OrderBy { inner, .. }
        | QueryExpression::Distinct { inner }
        | QueryExpression::Reduced { inner }
        | QueryExpression::Slice { inner, .. } => add_defined_variables(inner, set),
    }
}

fn unescape_iriref(mut input: &str, span: SimpleSpan) -> Result<Cow<'_, str>, AlgebraBuilderError> {
    let mut output = None;
    while let Some((before, after)) = input.split_once('\\') {
        let output: &mut String = output.get_or_insert_default();
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
    Ok(if let Some(mut output) = output {
        output.push_str(input);
        output.into()
    } else {
        input.into()
    })
}

fn unescape_local_name(mut input: &str) -> (Cow<'_, str>, bool) {
    let mut output = None;
    let mut might_be_invalid_iri = false;
    while let Some((before, after)) = input.split_once('\\') {
        let output: &mut String = output.get_or_insert_default();
        output.push_str(before);
        let Some(escape) = after.chars().next() else {
            unreachable!("PNAME_LOCAL is not allowed to end with a '\\'");
        };
        output.push(escape);
        if matches!(escape, '/' | '?' | '#' | '@' | '%') {
            might_be_invalid_iri = true;
        }
        input = after;
    }
    (
        if let Some(mut output) = output {
            output.push_str(input);
            output.into()
        } else {
            input.into()
        },
        might_be_invalid_iri,
    )
}

fn unescape_string(mut input: &str, span: SimpleSpan) -> Result<OxString, AlgebraBuilderError> {
    let mut output = None;
    while let Some((before, after)) = input.split_once('\\') {
        let output: &mut String = output.get_or_insert_default();
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
            Some('u') => read_hex_char::<4>(after.as_str(), span)?,
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
    Ok(if let Some(mut output) = output {
        output.push_str(input);
        OxString::new_owned(&output)
    } else {
        OxString::new_owned(input)
    })
}

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

fn function_arity(name: ast::BuiltInName) -> RangeInclusive<usize> {
    match name {
        ast::BuiltInName::Coalesce | ast::BuiltInName::Concat => 0..=usize::MAX,
        ast::BuiltInName::If => 3..=3,
        ast::BuiltInName::SameTerm | ast::BuiltInName::LangMatches => 2..=2,
        ast::BuiltInName::Str
        | ast::BuiltInName::Lang
        | ast::BuiltInName::Datatype
        | ast::BuiltInName::Iri
        | ast::BuiltInName::Uri
        | ast::BuiltInName::Abs
        | ast::BuiltInName::Ceil
        | ast::BuiltInName::Floor
        | ast::BuiltInName::Round
        | ast::BuiltInName::StrLen
        | ast::BuiltInName::UCase
        | ast::BuiltInName::LCase
        | ast::BuiltInName::EncodeForUri
        | ast::BuiltInName::Year
        | ast::BuiltInName::Month
        | ast::BuiltInName::Day
        | ast::BuiltInName::Hours
        | ast::BuiltInName::Minutes
        | ast::BuiltInName::Seconds
        | ast::BuiltInName::Timezone
        | ast::BuiltInName::Tz
        | ast::BuiltInName::Md5
        | ast::BuiltInName::Sha1
        | ast::BuiltInName::Sha256
        | ast::BuiltInName::Sha384
        | ast::BuiltInName::Sha512
        | ast::BuiltInName::IsIri
        | ast::BuiltInName::IsUri
        | ast::BuiltInName::IsBlank
        | ast::BuiltInName::IsLiteral
        | ast::BuiltInName::IsNumeric => 1..=1,
        ast::BuiltInName::BNode => 0..=1,
        ast::BuiltInName::Rand
        | ast::BuiltInName::Now
        | ast::BuiltInName::Uuid
        | ast::BuiltInName::StrUuid => 0..=0,
        ast::BuiltInName::SubStr | ast::BuiltInName::Regex => 2..=3,
        ast::BuiltInName::Replace => 3..=4,
        ast::BuiltInName::Contains
        | ast::BuiltInName::StrStarts
        | ast::BuiltInName::StrEnds
        | ast::BuiltInName::StrBefore
        | ast::BuiltInName::StrAfter
        | ast::BuiltInName::StrLang
        | ast::BuiltInName::StrDt => 2..=2,
        #[cfg(feature = "sparql-12")]
        ast::BuiltInName::Triple | ast::BuiltInName::StrLangDir => 3..=3,
        #[cfg(feature = "sparql-12")]
        ast::BuiltInName::Subject
        | ast::BuiltInName::Predicate
        | ast::BuiltInName::Object
        | ast::BuiltInName::IsTriple
        | ast::BuiltInName::LangDir
        | ast::BuiltInName::HasLang
        | ast::BuiltInName::HasLangDir => 1..=1,
        #[cfg(feature = "sep-0002")]
        ast::BuiltInName::Adjust => 2..=2,
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
            blank_nodes.visit_quads(quads);
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

    fn visit_query(&mut self, query: &ast::Query<'a>) {
        match &query.variant {
            ast::QueryQuery::Select(query) => {
                self.visit_select_clause(&query.select_clause);
                self.visit_graph_pattern(&query.where_clause);
                self.visit_solution_modifier(&query.solution_modifier);
            }
            ast::QueryQuery::Construct(query) => {
                for (subject, property_list) in &query.template.inner {
                    self.visit_graph_node_path(subject);
                    self.visit_property_list_path(property_list);
                }
                if let Some(where_clause) = &query.where_clause {
                    self.visit_graph_pattern(where_clause);
                }
                self.visit_solution_modifier(&query.solution_modifier);
            }
            ast::QueryQuery::Describe(query) => {
                if let ast::DescribeTargets::Explicit(targets) = &query.targets.inner {
                    for target in targets {
                        self.visit_var_or_iri(&target.inner);
                    }
                }
                if let Some(where_clause) = &query.where_clause {
                    self.visit_graph_pattern(where_clause);
                }
                self.visit_solution_modifier(&query.solution_modifier);
            }
            ast::QueryQuery::Ask(query) => {
                self.visit_graph_pattern(&query.where_clause);
                self.visit_solution_modifier(&query.solution_modifier);
            }
        }
        if let Some(values_clause) = &query.values_clause {
            self.visit_values_clause(values_clause);
        }
    }

    fn visit_update(&mut self, update: &ast::Update<'a>) {
        for (_, update) in &update.operations {
            self.visit_update1(update);
        }
    }

    fn visit_update1(&mut self, update: &ast::Update1<'a>) {
        match update {
            ast::Update1::DeleteWhere { pattern }
            | ast::Update1::InsertData { quads: pattern }
            | ast::Update1::DeleteData { quads: pattern } => self.visit_quads(pattern),
            ast::Update1::Modify {
                delete,
                insert,
                r#where,
                ..
            } => {
                self.visit_quads(delete);
                self.visit_quads(insert);
                self.visit_graph_pattern(r#where);
            }
            ast::Update1::Load { .. }
            | ast::Update1::Clear { .. }
            | ast::Update1::Drop { .. }
            | ast::Update1::Create { .. }
            | ast::Update1::Add { .. }
            | ast::Update1::Move { .. }
            | ast::Update1::Copy { .. } => {}
        }
    }

    fn visit_select_clause(&mut self, select_clause: &ast::SelectClause<'a>) {
        if let ast::SelectVariables::Explicit(bindings) = &select_clause.bindings.inner {
            for binding in bindings {
                let (expression, variable) = &binding.inner;
                if let Some(expression) = expression {
                    self.visit_expression(&expression.inner);
                }
                self.on_variable(variable);
            }
        }
    }

    fn visit_solution_modifier(&mut self, solution_modifier: &ast::SolutionModifier<'a>) {
        for (expression, variable) in &solution_modifier.group_clause {
            self.visit_expression(&expression.inner);
            if let Some(variable) = variable {
                self.on_variable(variable);
            }
        }
        for expression in &solution_modifier.having_clause {
            self.visit_expression(&expression.inner);
        }
        for condition in &solution_modifier.order_clause {
            match condition {
                ast::OrderCondition::Asc(expression) | ast::OrderCondition::Desc(expression) => {
                    self.visit_expression(&expression.inner);
                }
            }
        }
    }

    fn visit_values_clause(&mut self, values_clause: &ast::ValuesClause<'a>) {
        for variable in &values_clause.variables {
            self.on_variable(variable);
        }
    }

    fn visit_graph_pattern(&mut self, graph_pattern: &ast::GraphPattern<'a>) {
        match graph_pattern {
            ast::GraphPattern::Group(elements) => {
                for element in elements {
                    match &element.inner {
                        ast::GraphPatternElement::Filter(expression) => {
                            self.visit_expression(&expression.inner);
                        }
                        ast::GraphPatternElement::Union(patterns) => {
                            for pattern in patterns {
                                self.visit_graph_pattern(pattern);
                            }
                        }
                        ast::GraphPatternElement::Minus(pattern)
                        | ast::GraphPatternElement::Optional(pattern) => {
                            self.visit_graph_pattern(pattern);
                        }
                        ast::GraphPatternElement::Values(values_clause) => {
                            self.visit_values_clause(values_clause);
                        }
                        ast::GraphPatternElement::Bind(expression, variable) => {
                            self.visit_expression(&expression.inner);
                            self.on_variable(variable);
                        }
                        ast::GraphPatternElement::Service { name, pattern, .. }
                        | ast::GraphPatternElement::Graph { name, pattern } => {
                            self.visit_var_or_iri(name);
                            self.visit_graph_pattern(pattern);
                        }
                        ast::GraphPatternElement::Triples(triples) => {
                            for (subject, property_list) in triples {
                                self.visit_graph_node_path(subject);
                                self.visit_property_list_path(property_list);
                            }
                        }
                        #[cfg(feature = "sep-0006")]
                        ast::GraphPatternElement::Lateral(pattern) => {
                            self.visit_graph_pattern(pattern);
                        }
                    }
                }
            }
            ast::GraphPattern::SubSelect(select) => {
                self.visit_select_clause(&select.select_clause);
                self.visit_graph_pattern(&select.where_clause);
                self.visit_solution_modifier(&select.solution_modifier);
                if let Some(values_clause) = &select.values_clause {
                    self.visit_values_clause(values_clause);
                }
            }
        }
    }

    fn visit_expression(&mut self, expression: &ast::Expression<'a>) {
        match expression {
            ast::Expression::Or(left, right)
            | ast::Expression::And(left, right)
            | ast::Expression::Equal(left, right)
            | ast::Expression::NotEqual(left, right)
            | ast::Expression::Less(left, right)
            | ast::Expression::LessOrEqual(left, right)
            | ast::Expression::Greater(left, right)
            | ast::Expression::GreaterOrEqual(left, right)
            | ast::Expression::Add(left, right)
            | ast::Expression::Subtract(left, right)
            | ast::Expression::Multiply(left, right)
            | ast::Expression::Divide(left, right) => {
                self.visit_expression(&left.inner);
                self.visit_expression(&right.inner);
            }
            ast::Expression::In(expression, expressions)
            | ast::Expression::NotIn(expression, expressions) => {
                self.visit_expression(&expression.inner);
                for expression in expressions {
                    self.visit_expression(&expression.inner);
                }
            }
            ast::Expression::UnaryPlus(expression)
            | ast::Expression::UnaryMinus(expression)
            | ast::Expression::Not(expression) => self.visit_expression(&expression.inner),
            ast::Expression::Bound(variable) | ast::Expression::Var(variable) => {
                self.on_variable(variable);
            }
            ast::Expression::Aggregate(aggregate) => self.visit_aggregate(aggregate),
            ast::Expression::BuiltIn(_, arguments) => {
                for argument in arguments {
                    self.visit_expression(&argument.inner);
                }
            }
            ast::Expression::Function(_, arguments) => {
                for argument in &arguments.args {
                    self.visit_expression(&argument.inner);
                }
            }
            ast::Expression::Exists(pattern) | ast::Expression::NotExists(pattern) => {
                self.visit_graph_pattern(pattern);
            }
            ast::Expression::Iri(_) | ast::Expression::Literal(_) => {}
            #[cfg(feature = "sparql-12")]
            ast::Expression::TripleTerm(triple) => self.visit_expression_triple_term(triple),
        }
    }

    fn visit_aggregate(&mut self, aggregate: &ast::Aggregate<'a>) {
        match aggregate {
            ast::Aggregate::Count(_, expression) => {
                if let Some(expression) = expression {
                    self.visit_expression(&expression.inner);
                }
            }
            ast::Aggregate::Sum(_, expression)
            | ast::Aggregate::Min(_, expression)
            | ast::Aggregate::Max(_, expression)
            | ast::Aggregate::Avg(_, expression)
            | ast::Aggregate::Sample(_, expression)
            | ast::Aggregate::GroupConcat(_, expression, _) => {
                self.visit_expression(&expression.inner);
            }
        }
    }

    #[cfg(feature = "sparql-12")]
    fn visit_expression_triple_term(&mut self, triple: &ast::ExprTripleTerm<'a>) {
        if let ast::ExprTripleTermSubject::Var(variable) = &triple.subject {
            self.on_variable(variable);
        }
        self.visit_verb(&triple.predicate);
        match &triple.object {
            ast::ExprTripleTermObject::Var(variable) => self.on_variable(variable),
            ast::ExprTripleTermObject::TripleTerm(triple) => {
                self.visit_expression_triple_term(triple);
            }
            ast::ExprTripleTermObject::Iri(_) | ast::ExprTripleTermObject::Literal(_) => {}
        }
    }

    fn visit_quads(&mut self, quads: &ast::QuadPatterns<'a>) {
        for (graph_name, triples) in quads {
            if let Some(graph_name) = graph_name {
                self.visit_var_or_iri(graph_name);
            }
            for (subject, predicate_object) in triples {
                self.visit_graph_node_path(subject);
                self.visit_property_list_path(predicate_object);
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
    fn visit_reified_triple(&mut self, reified_triple: &Spanned<ast::ReifiedTriple<'a>>) {
        let reified_triple = &reified_triple.inner;
        self.visit_reified_triple_term_subject_or_object(&reified_triple.subject);
        self.visit_verb(&reified_triple.predicate);
        self.visit_reified_triple_term_subject_or_object(&reified_triple.object);
        if let Some(reifier) = &reified_triple.reifier {
            self.visit_var_or_reifier_id(reifier);
        }
    }

    #[cfg(feature = "sparql-12")]
    fn visit_triple_term(&mut self, triple_term: &Spanned<ast::TripleTerm<'a>>) {
        let triple_term = &triple_term.inner;
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

    #[cfg(feature = "sparql-12")]
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

struct FindAllVariables<'a, 'b> {
    variables: &'b mut HashSet<&'a str>,
}

impl<'a> TermVisitor<'a> for FindAllVariables<'a, '_> {
    fn on_variable(&mut self, variable: &Spanned<ast::Var<'a>>) {
        self.variables.insert(variable.inner.0);
    }
}

#[derive(Default)]
struct FreshVariableAllocator<'a> {
    used_variable_names: HashSet<&'a str>,
    allocated_variable_names: HashSet<OxString>,
    blank_node_mapping: HashMap<OxString, Variable>,
    counter_per_prefix: HashMap<&'a str, usize>,
}

impl<'a> FreshVariableAllocator<'a> {
    fn reset_with_query(&mut self, query: &ast::Query<'a>) {
        self.reset();
        let mut visitor = FindAllVariables {
            variables: &mut self.used_variable_names,
        };
        visitor.visit_query(query);
    }

    fn reset_with_update1(&mut self, update1: &ast::Update1<'a>) {
        self.reset();
        let mut visitor = FindAllVariables {
            variables: &mut self.used_variable_names,
        };
        visitor.visit_update1(update1);
    }

    fn reset(&mut self) {
        self.used_variable_names.clear();
        self.allocated_variable_names.clear();
        self.blank_node_mapping.clear();
        self.counter_per_prefix.clear();
    }

    fn fresh_variable(&mut self, prefix: &'a str) -> Variable {
        let counter = self.counter_per_prefix.entry(prefix).or_default();
        loop {
            *counter += 1;
            let name = format!("{prefix}{}", *counter);
            if !self.used_variable_names.contains(name.as_str())
                && !self.allocated_variable_names.contains(name.as_str())
            {
                // Not used, we can emit
                let name = OxString::new_owned(&name);
                self.allocated_variable_names.insert(name.clone());
                return Variable::new_unchecked(name);
            }
        }
    }

    fn map_blank_node_id(&mut self, name: &str) -> Variable {
        if let Some(var) = self.blank_node_mapping.get(name) {
            return var.clone();
        }
        let var = if !self.used_variable_names.contains(name)
            && !self.allocated_variable_names.contains(name)
        {
            // Not used, we can emit, but need to validate (blank node id grammar is more relaxed than variable names)
            let name = OxString::new_owned(name);
            if let Ok(variable) = Variable::new(name.clone()) {
                self.allocated_variable_names.insert(name);
                variable
            } else {
                self.fresh_variable("bn")
            }
        } else {
            self.fresh_variable("bn")
        };
        self.blank_node_mapping
            .insert(OxString::new_owned(name), var.clone());
        var
    }
}

struct FindAllBlankNodes<'a, 'b> {
    ids: &'b mut HashSet<&'a str>,
}

impl<'a> TermVisitor<'a> for FindAllBlankNodes<'a, '_> {
    fn on_blank_node(&mut self, blank_node: &Spanned<ast::BlankNode<'a>>) {
        if let Some(id) = blank_node.inner.0 {
            self.ids.insert(id);
        }
    }
}

#[derive(Default)]
struct FreshBlankNodeAllocator<'a> {
    used_blank_node_ids: HashSet<&'a str>,
    counter_per_prefix: HashMap<&'a str, usize>,
}

impl<'a> FreshBlankNodeAllocator<'a> {
    fn reset_with_query(&mut self, query: &ast::Query<'a>) {
        self.reset();
        let mut visitor = FindAllBlankNodes {
            ids: &mut self.used_blank_node_ids,
        };
        visitor.visit_query(query);
    }

    fn reset_with_update(&mut self, update: &ast::Update<'a>) {
        self.reset();
        let mut visitor = FindAllBlankNodes {
            ids: &mut self.used_blank_node_ids,
        };
        visitor.visit_update(update);
    }

    fn reset(&mut self) {
        self.used_blank_node_ids.clear();
        self.counter_per_prefix.clear();
    }

    fn fresh_spanned_blank_node(
        &mut self,
        prefix: &'a str,
        span: SimpleSpan,
    ) -> Spanned<BlankNode> {
        span.make_wrapped(self.fresh_blank_node(prefix))
    }

    fn fresh_blank_node(&mut self, prefix: &'a str) -> BlankNode {
        let counter = self.counter_per_prefix.entry(prefix).or_default();
        loop {
            *counter += 1;
            let id = format!("{prefix}{}", *counter);
            if !self.used_blank_node_ids.contains(id.as_str()) {
                // Not used, we can emit
                return BlankNode::new_unchecked(id);
            }
        }
    }
}

fn copy_graph(
    from: impl Into<GraphName>,
    to: impl Into<GraphNamePattern>,
) -> DeleteInsertOperation {
    let bgp = QueryExpression::Bgp {
        patterns: vec![TriplePattern::new(
            Variable::new_unchecked("s"),
            Variable::new_unchecked("p"),
            Variable::new_unchecked("o"),
        )],
    };
    DeleteInsertOperation {
        delete: Vec::new(),
        insert: vec![QuadTemplate::new(
            Variable::new_unchecked("s"),
            Variable::new_unchecked("p"),
            Variable::new_unchecked("o"),
            to,
        )],
        using: None,
        pattern: Box::new(match from.into() {
            GraphName::NamedNode(from) => QueryExpression::Graph {
                name: from.into(),
                inner: Box::new(bgp),
            },
            GraphName::DefaultGraph => bgp,
        }),
    }
}
