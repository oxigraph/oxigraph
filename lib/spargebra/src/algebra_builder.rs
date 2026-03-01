use crate::algebra::{
    AggregateExpression, Expression, GraphPattern, OrderExpression, QueryDataset,
};
use crate::ast;
use crate::ast::VarOrIri;
use crate::query::Query;
use oxiri::Iri;
use oxrdf::{BlankNode, NamedNode, Variable};
use rand::random;
use std::collections::{HashMap, HashSet};

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

    fn build_describe_query(
        mut self,
        query: ast::DescribeQuery<'_>,
        values_clause: Option<ast::ValuesClause<'_>>,
    ) -> Result<Query, String> {
        Ok(Query::Describe {
            dataset: self.build_dataset(query.dataset_clause)?,
            pattern: self.build_select(
                ast::SelectClause {
                    option: ast::SelectionOption::Default,
                    bindings: query
                        .targets
                        .into_iter()
                        .map(|var_or_iri| match var_or_iri {
                            VarOrIri::Iri(n) => (Some(ast::Expression::Iri(n)), unimplemented!()),
                            VarOrIri::Var(v) => (None, v),
                        })
                        .collect(),
                },
                query
                    .where_clause
                    .unwrap_or_else(|| ast::GraphPattern::Triples(Vec::new())),
                query.solution_modifier,
                values_clause,
            )?,
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
                    variable
                } else {
                    if with_aggregate && !visible.contains(&variable) {
                        // We validate projection variables if there is an aggregate
                        return Err(format!("The SELECT variable {variable} is unbound"));
                    }
                    variable
                };
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

    fn build_values_clause(&self, values_clause: ast::ValuesClause<'_>) -> GraphPattern {
        unimplemented!()
    }

    fn build_graph_pattern(
        &mut self,
        graph_pattern: ast::GraphPattern<'_>,
    ) -> Result<GraphPattern, String> {
        unimplemented!()
    }

    fn build_order_expression(
        &mut self,
        expression: ast::OrderCondition<'_>,
    ) -> Result<OrderExpression, String> {
        unimplemented!()
    }

    fn build_expression(&mut self, expression: ast::Expression<'_>) -> Result<Expression, String> {
        unimplemented!()
    }

    fn build_variable(&self, var: ast::Var<'_>) -> Variable {
        Variable::new_unchecked(var.0)
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
            b.iter()
                .filter_map(|b| find_unbound_variable(b, variables))
                .next()
        }
        Expression::FunctionCall(_, parameters) => parameters
            .iter()
            .filter_map(|p| find_unbound_variable(p, variables))
            .next(),
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
