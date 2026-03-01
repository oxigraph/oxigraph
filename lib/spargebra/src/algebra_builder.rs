use crate::algebra::{
    AggregateExpression, Expression, Function, GraphPattern, OrderExpression,
    PropertyPathExpression, QueryDataset,
};
use crate::ast;
use crate::query::Query;
use crate::term::{NamedNodePattern, TermPattern, TriplePattern};
use oxiri::Iri;
use oxrdf::vocab::{rdf, xsd};
use oxrdf::{BaseDirection, BlankNode, Literal, NamedNode, Variable};
use rand::random;
use std::collections::{HashMap, HashSet};
use std::mem::take;

pub struct AlgebraBuilder {
    base_iri: Option<Iri<String>>,
    prefixes: HashMap<String, String>,
    custom_aggregate_functions: HashSet<NamedNode>,
    used_bnodes: HashSet<BlankNode>,
    currently_used_bnodes: HashSet<BlankNode>,
    aggregates: Vec<Vec<(Variable, AggregateExpression)>>,
}

impl AlgebraBuilder {
    pub fn new(
        base_iri: Option<Iri<String>>,
        prefixes: HashMap<String, String>,
        custom_aggregate_functions: HashSet<NamedNode>,
    ) -> Self {
        Self {
            base_iri,
            prefixes,
            custom_aggregate_functions,
            used_bnodes: HashSet::new(),
            currently_used_bnodes: HashSet::new(),
            aggregates: Vec::new(),
        }
    }

    pub fn build_query(mut self, query: ast::Query<'_>) -> Result<Query, String> {
        self.apply_prologue(query.prologue)?;
        match query.query {
            ast::QueryQuery::Select(select) => self.build_select_query(select, query.values_clause),
            ast::QueryQuery::Construct(construct) => {
                self.build_construct_query(construct, query.values_clause)
            }
            ast::QueryQuery::Describe(describe) => {
                self.build_describe_query(describe, query.values_clause)
            }
            ast::QueryQuery::Ask(ask) => self.build_ask_query(ask, query.values_clause),
        }
    }

    fn build_select_query(
        mut self,
        query: ast::SelectQuery<'_>,
        values_clause: Option<ast::ValuesClause<'_>>,
    ) -> Result<Query, String> {
        Ok(Query::Select {
            dataset: self.build_dataset(query.dataset_clause)?,
            pattern: self.build_select(
                query.select_clause,
                query.where_clause,
                query.solution_modifier,
                values_clause,
            )?,
            base_iri: self.base_iri,
        })
    }

    fn build_construct_query(
        mut self,
        query: ast::ConstructQuery<'_>,
        values_clause: Option<ast::ValuesClause<'_>>,
    ) -> Result<Query, String> {
        let where_clause = query.where_clause.unwrap_or_else(|| {
            ast::GraphPattern::Group(vec![ast::GraphPatternElement::Triples(
                query
                    .template
                    .clone()
                    .into_iter()
                    .map(|(s, pos)| {
                        (
                            s.into(),
                            pos.into_iter()
                                .map(|(p, os)| (p.into(), os.into_iter().map(Into::into).collect()))
                                .collect(),
                        )
                    })
                    .collect(),
            )])
        });
        let template = self.build_triple_template(query.template)?;
        Ok(Query::Construct {
            template: template.clone(),
            dataset: self.build_dataset(query.dataset_clause)?,
            pattern: self.build_select(
                ast::SelectClause {
                    option: ast::SelectionOption::Default,
                    bindings: Vec::new(),
                },
                where_clause,
                query.solution_modifier,
                values_clause,
            )?,
            base_iri: self.base_iri,
        })
    }

    fn build_describe_query(
        mut self,
        query: ast::DescribeQuery<'_>,
        values_clause: Option<ast::ValuesClause<'_>>,
    ) -> Result<Query, String> {
        let mut pattern = self.build_select(
            ast::SelectClause {
                option: ast::SelectionOption::Default,
                bindings: query
                    .targets
                    .iter()
                    .filter_map(|var_or_iri| {
                        if let ast::VarOrIri::Var(v) = var_or_iri {
                            Some((None, v.clone()))
                        } else {
                            None
                        }
                    })
                    .collect(),
            },
            query
                .where_clause
                .unwrap_or_else(|| ast::GraphPattern::Group(Vec::new())),
            query.solution_modifier,
            values_clause,
        )?;
        // We add the IRIS
        for target in query.targets {
            if let ast::VarOrIri::Iri(target) = target {
                pattern = GraphPattern::Extend {
                    inner: Box::new(pattern),
                    variable: random_variable(),
                    expression: self.build_named_node(target)?.into(),
                }
            }
        }
        Ok(Query::Describe {
            dataset: self.build_dataset(query.dataset_clause)?,
            pattern,
            base_iri: self.base_iri,
        })
    }

    fn build_ask_query(
        mut self,
        query: ast::AskQuery<'_>,
        values_clause: Option<ast::ValuesClause<'_>>,
    ) -> Result<Query, String> {
        Ok(Query::Ask {
            dataset: self.build_dataset(query.dataset_clause)?,
            pattern: self.build_select(
                ast::SelectClause {
                    option: ast::SelectionOption::Default,
                    bindings: Vec::new(),
                },
                query.where_clause,
                query.solution_modifier,
                values_clause,
            )?,
            base_iri: self.base_iri,
        })
    }

    fn apply_prologue(&mut self, prologue: Vec<ast::PrologueDecl<'_>>) -> Result<(), String> {
        for decl in prologue {
            self.apply_prologue_decl(decl)?;
        }
        Ok(())
    }

    fn apply_prologue_decl(&mut self, decl: ast::PrologueDecl<'_>) -> Result<(), String> {
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
    ) -> Result<Option<QueryDataset>, String> {
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
        &mut self,
        select_clause: ast::SelectClause<'_>,
        where_clause: ast::GraphPattern<'_>,
        solution_modifier: ast::SolutionModifier<'_>,
        values_clause: Option<ast::ValuesClause<'_>>,
    ) -> Result<GraphPattern, String> {
        let mut p = self.build_graph_pattern(where_clause)?;

        // GROUP BY
        let aggregates = self.aggregates.pop().unwrap_or_default();
        let with_aggregate = !solution_modifier.group_clause.is_empty() || !aggregates.is_empty();
        if with_aggregate {
            let mut variables = Vec::new();
            for (expression, variable) in solution_modifier.group_clause {
                let expression = self.build_expression(expression)?;
                let variable = variable.map(|v| self.build_variable(v));
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
        if let Some(expr) = solution_modifier
            .having_clause
            .into_iter()
            .map(|e| self.build_expression(e))
            .reduce(|a, b| Ok(Expression::And(Box::new(a?), Box::new(b?))))
        {
            p = GraphPattern::Filter {
                expr: expr?,
                inner: Box::new(p),
            };
        }

        // VALUES
        if let Some(values_clause) = values_clause {
            p = new_join(p, self.build_values_clause(values_clause));
        }

        // SELECT
        let mut projection_variables = Vec::new();
        let with_project = if select_clause.bindings.is_empty() {
            if with_aggregate {
                return Err("SELECT * is not authorized with GROUP BY".into());
            }
            // TODO: is it really useful to always do a projection?
            p.on_in_scope_variable(|v| {
                if !projection_variables.contains(v) {
                    projection_variables.push(v.clone());
                }
            });
            projection_variables.sort();
            true
        } else {
            let mut visible = HashSet::new();
            p.on_in_scope_variable(|v| {
                visible.insert(v.clone());
            });
            for (expression, variable) in select_clause.bindings {
                let variable = self.build_variable(variable);
                if let Some(expression) = expression {
                    let expression = self.build_expression(expression)?;
                    if visible.contains(&variable) {
                        // We disallow to override an existing variable with an expression
                        return Err(format!(
                            "The SELECT overrides {variable} using an expression even if it's already used"
                        ));
                    }
                    if with_aggregate {
                        // We validate projection variables if there is an aggregate
                        if let Some(v) = find_unbound_variable(&expression, &visible) {
                            return Err(format!(
                                "The variable {v} is unbound in a SELECT expression",
                            ));
                        }
                    }
                    p = GraphPattern::Extend {
                        inner: Box::new(p),
                        variable: variable.clone(),
                        expression,
                    };
                } else {
                    if with_aggregate && !visible.contains(&variable) {
                        // We validate projection variables if there is an aggregate
                        return Err(format!("The SELECT variable {variable} is unbound"));
                    }
                }
                if projection_variables.contains(&variable) {
                    return Err(format!("{variable} is declared twice in SELECT"));
                }
                projection_variables.push(variable)
            }
            true
        };

        let mut m = p;

        // ORDER BY
        if !solution_modifier.order_clause.is_empty() {
            m = GraphPattern::OrderBy {
                inner: Box::new(m),
                expression: solution_modifier
                    .order_clause
                    .into_iter()
                    .map(|e| self.build_order_expression(e))
                    .collect::<Result<_, _>>()?,
            };
        }

        // PROJECT
        if with_project {
            m = GraphPattern::Project {
                inner: Box::new(m),
                variables: projection_variables,
            };
        }
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
        template: Vec<(
            ast::GraphNode<'_>,
            Vec<(ast::Verb<'_>, Vec<ast::Object<'_>>)>,
        )>,
    ) -> Result<Vec<TriplePattern>, String> {
        let mut patterns = Vec::new();
        for (subject, predicate_objects) in template {
            for (predicate, objects) in predicate_objects {
                for object in objects {
                    unimplemented!()
                }
            }
        }
        Ok(patterns)
    }

    fn build_values_clause(&self, values_clause: ast::ValuesClause<'_>) -> GraphPattern {
        unimplemented!()
    }

    fn build_graph_pattern(
        &mut self,
        graph_pattern: ast::GraphPattern<'_>,
    ) -> Result<GraphPattern, String> {
        Ok(match graph_pattern {
            ast::GraphPattern::Group(elements) => {
                let mut g = GraphPattern::default();
                let mut filter: Option<Expression> = None;
                for element in elements {
                    match element {
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
                            let variable = self.build_variable(var);
                            let mut is_variable_overridden = false;
                            g.on_in_scope_variable(|v| {
                                if *v == variable {
                                    is_variable_overridden = true;
                                }
                            });
                            if is_variable_overridden {
                                return Err(format!(
                                    "{variable} is already in scoped and cannot be overridden by BIND"
                                ));
                            }
                            g = GraphPattern::Extend {
                                inner: Box::new(g),
                                variable,
                                expression: self.build_expression(expression)?,
                            }
                        }
                        ast::GraphPatternElement::Filter(expr) => {
                            let expr = self.build_expression(expr)?;
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
                        ast::GraphPatternElement::Values { .. } => unimplemented!(),
                        ast::GraphPatternElement::Service { .. } => unimplemented!(),
                        ast::GraphPatternElement::Graph { .. } => unimplemented!(),
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
                                return Err(format!(
                                    "{overridden_variable} is overridden in the right side of LATERAL"
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
        &mut self,
        expression: ast::OrderCondition<'_>,
    ) -> Result<OrderExpression, String> {
        Ok(match expression {
            ast::OrderCondition::Asc(e) => OrderExpression::Asc(self.build_expression(e)?),
            ast::OrderCondition::Desc(e) => OrderExpression::Desc(self.build_expression(e)?),
        })
    }

    fn build_expression(&mut self, expression: ast::Expression<'_>) -> Result<Expression, String> {
        Ok(match expression {
            ast::Expression::Or(l, r) => Expression::Or(
                Box::new(self.build_expression(*l)?),
                Box::new(self.build_expression(*r)?),
            ),
            ast::Expression::And(l, r) => Expression::And(
                Box::new(self.build_expression(*l)?),
                Box::new(self.build_expression(*r)?),
            ),
            ast::Expression::Equal(l, r) => Expression::Equal(
                Box::new(self.build_expression(*l)?),
                Box::new(self.build_expression(*r)?),
            ),
            ast::Expression::NotEqual(l, r) => Expression::Not(Box::new(Expression::Equal(
                Box::new(self.build_expression(*l)?),
                Box::new(self.build_expression(*r)?),
            ))),
            ast::Expression::Less(l, r) => Expression::Less(
                Box::new(self.build_expression(*l)?),
                Box::new(self.build_expression(*r)?),
            ),
            ast::Expression::LessOrEqual(l, r) => Expression::LessOrEqual(
                Box::new(self.build_expression(*l)?),
                Box::new(self.build_expression(*r)?),
            ),
            ast::Expression::Greater(l, r) => Expression::Greater(
                Box::new(self.build_expression(*l)?),
                Box::new(self.build_expression(*r)?),
            ),
            ast::Expression::GreaterOrEqual(l, r) => Expression::GreaterOrEqual(
                Box::new(self.build_expression(*l)?),
                Box::new(self.build_expression(*r)?),
            ),
            ast::Expression::In(l, r) => Expression::In(
                Box::new(self.build_expression(*l)?),
                r.into_iter()
                    .map(|e| self.build_expression(e))
                    .collect::<Result<_, _>>()?,
            ),
            ast::Expression::NotIn(l, r) => Expression::Not(Box::new(Expression::In(
                Box::new(self.build_expression(*l)?),
                r.into_iter()
                    .map(|e| self.build_expression(e))
                    .collect::<Result<_, _>>()?,
            ))),
            ast::Expression::Add(l, r) => Expression::Add(
                Box::new(self.build_expression(*l)?),
                Box::new(self.build_expression(*r)?),
            ),
            ast::Expression::Subtract(l, r) => Expression::Subtract(
                Box::new(self.build_expression(*l)?),
                Box::new(self.build_expression(*r)?),
            ),
            ast::Expression::Multiply(l, r) => Expression::Multiply(
                Box::new(self.build_expression(*l)?),
                Box::new(self.build_expression(*r)?),
            ),
            ast::Expression::Divide(l, r) => Expression::Divide(
                Box::new(self.build_expression(*l)?),
                Box::new(self.build_expression(*r)?),
            ),
            ast::Expression::UnaryPlus(e) => {
                Expression::UnaryPlus(Box::new(self.build_expression(*e)?))
            }
            ast::Expression::UnaryMinus(e) => {
                Expression::UnaryMinus(Box::new(self.build_expression(*e)?))
            }
            ast::Expression::Not(e) => Expression::Not(Box::new(self.build_expression(*e)?)),
            ast::Expression::Bound(v) => Expression::Bound(self.build_variable(v)),
            ast::Expression::Aggregate(_) => unimplemented!(),
            ast::Expression::Iri(n) => Expression::NamedNode(self.build_named_node(n)?),
            ast::Expression::Literal(l) => Expression::Literal(self.build_literal(l)?),
            ast::Expression::Var(v) => Expression::Variable(self.build_variable(v)),
            ast::Expression::BuiltIn(name, args) => {
                let args = args
                    .into_iter()
                    .map(|e| self.build_expression(e))
                    .collect::<Result<_, _>>()?;
                Expression::FunctionCall(
                    match name {
                        ast::BuiltInName::Coalesce => {
                            return Ok(Expression::Coalesce(args));
                        }
                        ast::BuiltInName::If => {
                            let [a, b, c] = args
                                .try_into()
                                .map_err(|_| "IF() takes exactly 3 parameters")?;
                            return Ok(Expression::If(Box::new(a), Box::new(b), Box::new(c)));
                        }
                        ast::BuiltInName::SameTerm => {
                            let [l, r] = args
                                .try_into()
                                .map_err(|_| "sameTerm() takes exactly 3 parameters")?;
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
                    },
                    args,
                )
            }
            ast::Expression::Function(name, args) => {
                let name = self.build_named_node(name)?;
                if self.custom_aggregate_functions.contains(&name) {
                    unimplemented!();
                }
                Expression::FunctionCall(
                    Function::Custom(name),
                    args.args
                        .into_iter()
                        .map(|e| self.build_expression(e))
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

    fn build_property_list_path(
        &self,
        subject: &TermPattern,
        property_list: ast::PropertyListPath<'_>,
        patterns: &mut Vec<TripleOrPathPattern>,
    ) -> Result<(), String> {
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
                patterns.push(match predicate.clone() {
                    VarOrPath::Var(predicate) => TripleOrPathPattern::Triple(TriplePattern::new(
                        subject.clone(),
                        predicate,
                        object,
                    )),
                    VarOrPath::Path(path) => TripleOrPathPattern::Path {
                        // TODO: build TermPattern if possible
                        subject: subject.clone(),
                        path,
                        object,
                    },
                })
            }
        }
        Ok(())
    }

    fn build_object_path(
        &self,
        #[cfg(feature = "sparql-12")] subject: &TermPattern,
        #[cfg(feature = "sparql-12")] predicate: &VarOrPath,
        object_path: ast::ObjectPath<'_>,
        patterns: &mut Vec<TripleOrPathPattern>,
    ) -> Result<TermPattern, String> {
        let object = self.build_graph_node_path(object_path.graph_node, patterns)?;
        #[cfg(feature = "sparql-12")]
        {
            let mut current_reifier = None;
            for annotation in object_path.annotation {
                let reifier_to_emit = match annotation {
                    ast::AnnotationPath::Reifier(r) => {
                        let reifier_to_emit = current_reifier;
                        current_reifier = Some(if let Some(r) = r {
                            self.build_reifier_id(r)?
                        } else {
                            BlankNode::default().into()
                        });
                        reifier_to_emit
                    }
                    ast::AnnotationPath::AnnotationBlock(a) => {
                        let reifier_to_emit = take(&mut current_reifier)
                            .unwrap_or_else(|| BlankNode::default().into());
                        self.build_property_list_path(&reifier_to_emit, a, patterns)?;
                        Some(reifier_to_emit)
                    }
                };
                if let Some(reifier) = reifier_to_emit {
                    let predicate =
                        match predicate {
                            VarOrPath::Var(predicate) => NamedNodePattern::from(predicate.clone()),
                            VarOrPath::Path(PropertyPathExpression::NamedNode(predicate)) => {
                                predicate.clone().into()
                            }
                            VarOrPath::Path(_) => return Err(
                                "Reifiers can only be used on triples and not on property paths"
                                    .into(),
                            ),
                        };
                    patterns.push(TripleOrPathPattern::Triple(TriplePattern::new(
                        subject.clone(),
                        predicate,
                        reifier,
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
    ) -> Result<TermPattern, String> {
        Ok(match var_or_reifier_id {
            ast::VarOrReifierId::Var(v) => self.build_variable(v).into(),
            ast::VarOrReifierId::Iri(n) => self.build_named_node(n)?.into(),
            ast::VarOrReifierId::BlankNode(n) => self.build_blank_node(n).into(),
        })
    }

    fn build_graph_node_path(
        &self,
        graph_node_path: ast::GraphNodePath<'_>,
        patterns: &mut Vec<TripleOrPathPattern>,
    ) -> Result<TermPattern, String> {
        match graph_node_path {
            ast::GraphNodePath::VarOrTerm(var_or_term) => self.build_term_pattern(var_or_term),
            ast::GraphNodePath::Collection(elements) => {
                let mut patterns = Vec::new();
                let mut current_list_node = TermPattern::from(rdf::NIL.into_owned());
                for element in elements.into_iter().rev() {
                    let element = self.build_graph_node_path(element, &mut patterns)?;
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
                self.build_property_list_path(&subject, property_list, patterns)?;
                Ok(subject)
            }
        }
    }

    fn build_var_or_path(&self, var_or_path: ast::VarOrPath<'_>) -> Result<VarOrPath, String> {
        Ok(match var_or_path {
            ast::VarOrPath::Var(v) => VarOrPath::Var(self.build_variable(v)),
            ast::VarOrPath::Path(p) => VarOrPath::Path(self.build_path(p)?),
        })
    }

    fn build_path(&self, path: ast::Path<'_>) -> Result<PropertyPathExpression, String> {
        Ok(match path {
            ast::Path::Alternative(l, r) => PropertyPathExpression::Alternative(
                Box::new(self.build_path(*l)?),
                Box::new(self.build_path(*r)?),
            ),
            ast::Path::Sequence(l, r) => PropertyPathExpression::Alternative(
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

    fn build_term_pattern(&self, var_or_term: ast::VarOrTerm<'_>) -> Result<TermPattern, String> {
        Ok(match var_or_term {
            ast::VarOrTerm::Var(v) => self.build_variable(v).into(),
            ast::VarOrTerm::Iri(n) => self.build_named_node(n)?.into(),
            ast::VarOrTerm::BlankNode(n) => self.build_blank_node(n).into(),
            ast::VarOrTerm::Literal(l) => self.build_literal(l)?.into(),
            ast::VarOrTerm::Nil => rdf::NIL.into_owned().into(),
        })
    }

    fn build_variable(&self, var: ast::Var<'_>) -> Variable {
        Variable::new_unchecked(var.0)
    }

    fn build_literal(&self, literal: ast::Literal<'_>) -> Result<Literal, String> {
        Ok(match literal {
            ast::Literal::Boolean(v) => {
                Literal::new_typed_literal(if v { "true" } else { "false" }, xsd::BOOLEAN)
            }
            ast::Literal::Integer(v) => Literal::new_typed_literal(v, xsd::INTEGER),
            ast::Literal::Decimal(v) => Literal::new_typed_literal(v, xsd::DECIMAL),
            ast::Literal::Double(v) => Literal::new_typed_literal(v, xsd::DOUBLE),
            ast::Literal::String(v) => Literal::new_simple_literal(self.build_string(v)),
            ast::Literal::LangString(v, l) => {
                Literal::new_language_tagged_literal(self.build_string(v), l)
                    .map_err(|e| format!("Invalid language tag '{l}': {e}"))?
            }
            #[cfg(feature = "sparql-12")]
            ast::Literal::DirLangString(v, l, d) => {
                Literal::new_directional_language_tagged_literal(
                    self.build_string(v),
                    l,
                    match d {
                        "ltr" => BaseDirection::Ltr,
                        "rtl" => BaseDirection::Rtl,
                        _ => {
                            return Err(format!(
                                "The only possible base directions are 'rtl' and 'ltr', found '{d}'"
                            ));
                        }
                    },
                )
                .map_err(|e| format!("Invalid language tag '{l}': {e}"))?
            }
            ast::Literal::Typed(v, t) => {
                Literal::new_typed_literal(self.build_string(v), self.build_named_node(t)?)
            }
        })
    }

    fn build_blank_node(&self, blank_node: ast::BlankNode<'_>) -> BlankNode {
        if let Some(id) = blank_node.0 {
            BlankNode::new_unchecked(id)
        } else {
            BlankNode::default()
        }
    }

    fn build_string(&self, string: ast::String<'_>) -> String {
        string.0.into()
    }

    fn build_named_node(&self, iri: ast::Iri<'_>) -> Result<NamedNode, String> {
        Ok(NamedNode::new_unchecked(
            match iri {
                ast::Iri::IriRef(iri) => self.build_iri(iri),
                ast::Iri::PrefixedName(pname) => self.build_prefixed_name(pname),
            }?
            .into_inner(),
        ))
    }

    fn build_prefixed_name(&self, pname: ast::PrefixedName<'_>) -> Result<Iri<String>, String> {
        if let Some(base) = self.prefixes.get(pname.0) {
            let mut iri = String::with_capacity(base.len() + pname.1.len());
            iri.push_str(base);
            for chunk in pname.1.split('\\') {
                // We remove \
                iri.push_str(chunk);
            }
            Iri::parse(iri)
                .map_err(|e| format!("Invalid IRI built from '{}:{}': {e}", pname.0, pname.1))
        } else {
            Err(format!("The prefix '{}:' is not defined", pname.0))
        }
    }

    fn build_iri(&self, iri: ast::IriRef<'_>) -> Result<Iri<String>, String> {
        if let Some(base_iri) = &self.base_iri {
            base_iri.resolve(&iri.0)
        } else {
            Iri::parse(iri.0.into())
        }
        .map_err(|e| format!("Invalid IRI '{}': {e}", iri.0))
    }

    fn new_aggregation(&mut self, agg: AggregateExpression) -> Result<Variable, &'static str> {
        let aggregates = self.aggregates.last_mut().ok_or("Unexpected aggregate")?;
        Ok(aggregates
            .iter()
            .find_map(|(v, a)| (a == &agg).then_some(v))
            .cloned()
            .unwrap_or_else(|| {
                let new_var = random_variable();
                aggregates.push((new_var.clone(), agg));
                new_var
            }))
    }
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
        Expression::Variable(var) => variables.contains(var).then_some(var),
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

impl<'a> From<ast::GraphNode<'a>> for ast::GraphNodePath<'a> {
    fn from(node: ast::GraphNode<'a>) -> Self {
        match node {
            ast::GraphNode::VarOrTerm(n) => Self::VarOrTerm(n),
            ast::GraphNode::Collection(c) => {
                Self::Collection(c.into_iter().map(Into::into).collect())
            }
            ast::GraphNode::BlankNodePropertyList(pl) => Self::BlankNodePropertyList(
                pl.into_iter()
                    .map(|(p, os)| (p.into(), os.into_iter().map(Into::into).collect()))
                    .collect(),
            ),
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
            annotation: object.annotation.into_iter().map(Into::into).collect(),
        }
    }
}

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
