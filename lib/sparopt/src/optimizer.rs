use crate::algebra::{
    Expression, GroundTerm, JoinAlgorithm, LeftJoinAlgorithm, MinusAlgorithm, QueryExpression,
};
use crate::estimation::{Estimate, Estimator, SEEK_COST, filter_selectivity};
use crate::statistics::{NoStatistics, Statistics};
use crate::type_inference::{
    VariableType, VariableTypes, infer_expression_type, infer_query_expression_types,
};
use oxrdf::{NamedNode, Variable};
use rustc_hash::FxHashMap;
use spargebra::algebra::PropertyPathExpression;
use spargebra::term::{NamedNodePattern, TermPattern};
use spargebra::vocab::sparql;
use std::cell::{OnceCell, RefCell};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::iter::once;
use std::rc::Rc;

/// Join groups up to this number of operands are ordered with an exhaustive search of the left-deep join trees.
const MAX_OPERANDS_FOR_EXHAUSTIVE_SEARCH: usize = 10;
/// Join groups up to this number of operands are ordered with a greedy search started from each operand.
/// Bigger groups use a single greedy search started from the smallest operand.
const MAX_OPERANDS_FOR_ALL_STARTS: usize = 20;

pub struct Optimizer;

impl Optimizer {
    pub fn optimize_query_expression(query_expression: QueryExpression) -> QueryExpression {
        Self::optimize_query_expression_with(query_expression, &NoStatistics)
    }

    /// Optimizes the query expression using the given statistics about the queried dataset to estimate cardinalities.
    pub fn optimize_query_expression_with(
        query_expression: QueryExpression,
        statistics: &impl Statistics,
    ) -> QueryExpression {
        let input_types = VariableTypes::default();
        let query_expression = Self::normalize_pattern(query_expression, &input_types);
        let query_expression = Self::push_graph(query_expression, None, &input_types);
        let query_expression = Self::flatten_joins(query_expression, &input_types);
        let query_expression = JoinReorderer {
            estimator: Estimator::new(statistics),
        }
        .reorder_joins(query_expression, &input_types);
        Self::push_filters(query_expression, Vec::new(), &input_types)
    }

    /// Normalize the pattern, discarding any join ordering information
    fn normalize_pattern(
        query_expression: QueryExpression,
        input_types: &VariableTypes,
    ) -> QueryExpression {
        match query_expression {
            QueryExpression::QuadPattern {
                subject,
                predicate,
                object,
                graph_name,
            } => QueryExpression::QuadPattern {
                subject,
                predicate,
                object,
                graph_name,
            },
            QueryExpression::Path {
                subject,
                path,
                object,
            } => QueryExpression::Path {
                subject,
                path,
                object,
            },
            QueryExpression::Graph { graph_name, inner } => {
                QueryExpression::graph(Self::normalize_pattern(*inner, input_types), graph_name)
            }
            QueryExpression::Join {
                left,
                right,
                algorithm,
            } => QueryExpression::join(
                Self::normalize_pattern(*left, input_types),
                Self::normalize_pattern(*right, input_types),
                algorithm,
            ),
            QueryExpression::LeftJoin {
                left,
                right,
                expression,
                algorithm,
            } => {
                let left = Self::normalize_pattern(*left, input_types);
                let right = Self::normalize_pattern(*right, input_types);
                let mut inner_types = infer_query_expression_types(&left, input_types.clone());
                inner_types
                    .intersect_with(infer_query_expression_types(&right, input_types.clone()));
                QueryExpression::left_join(
                    left,
                    right,
                    Self::normalize_expression(expression, &inner_types),
                    algorithm,
                )
            }
            #[cfg(feature = "sep-0006")]
            QueryExpression::Lateral { left, right } => {
                let left = Self::normalize_pattern(*left, input_types);
                let left_types = infer_query_expression_types(&left, input_types.clone());
                let right = Self::normalize_pattern(*right, &left_types);
                QueryExpression::lateral(left, right)
            }
            QueryExpression::Filter { inner, expression } => {
                let inner = Self::normalize_pattern(*inner, input_types);
                let inner_types = infer_query_expression_types(&inner, input_types.clone());
                let expression = Self::normalize_expression(expression, &inner_types);
                let expression_type = infer_expression_type(&expression, &inner_types);
                if expression_type == VariableType::UNDEF {
                    QueryExpression::empty()
                } else {
                    QueryExpression::filter(inner, expression)
                }
            }
            QueryExpression::Union { inner } => QueryExpression::union_all(
                inner
                    .into_iter()
                    .map(|e| Self::normalize_pattern(e, input_types)),
            ),
            QueryExpression::Extend {
                inner,
                variable,
                expression,
            } => {
                let inner = Self::normalize_pattern(*inner, input_types);
                let inner_types = infer_query_expression_types(&inner, input_types.clone());
                let expression = Self::normalize_expression(expression, &inner_types);
                let expression_type = infer_expression_type(&expression, &inner_types);
                if expression_type == VariableType::UNDEF {
                    // TODO: valid?
                    inner
                } else {
                    QueryExpression::extend(inner, variable, expression)
                }
            }
            QueryExpression::Minus {
                left,
                right,
                algorithm,
            } => QueryExpression::minus(
                Self::normalize_pattern(*left, input_types),
                Self::normalize_pattern(*right, input_types),
                algorithm,
            ),
            QueryExpression::Values {
                variables,
                bindings,
            } => QueryExpression::values(variables, bindings),
            QueryExpression::OrderBy { inner, expression } => {
                QueryExpression::order_by(Self::normalize_pattern(*inner, input_types), expression)
            }
            QueryExpression::Project { inner, variables } => {
                QueryExpression::project(Self::normalize_pattern(*inner, input_types), variables)
            }
            QueryExpression::Distinct { inner } => {
                QueryExpression::distinct(Self::normalize_pattern(*inner, input_types))
            }
            QueryExpression::Reduced { inner } => {
                QueryExpression::reduced(Self::normalize_pattern(*inner, input_types))
            }
            QueryExpression::Slice {
                inner,
                offset,
                limit,
            } => {
                QueryExpression::slice(Self::normalize_pattern(*inner, input_types), offset, limit)
            }
            QueryExpression::Group {
                inner,
                variables,
                aggregates,
            } => {
                // TODO: min, max and sample don't care about DISTINCT
                QueryExpression::group(
                    Self::normalize_pattern(*inner, input_types),
                    variables,
                    aggregates,
                )
            }
            QueryExpression::Service { .. } => {
                // We leave this problem to the remote SPARQL endpoint
                query_expression
            }
        }
    }

    fn normalize_expression(expression: Expression, types: &VariableTypes) -> Expression {
        match expression {
            Expression::NamedNode(node) => node.into(),
            Expression::Literal(literal) => literal.into(),
            Expression::Variable(variable) => variable.into(),
            Expression::Or(inner) => Expression::or_all(
                inner
                    .into_iter()
                    .map(|e| Self::normalize_expression(e, types)),
            ),
            Expression::And(inner) => Expression::and_all(
                inner
                    .into_iter()
                    .map(|e| Self::normalize_expression(e, types)),
            ),
            Expression::FunctionCall(name, args) if name == sparql::EQUALS && args.len() == 2 => {
                let [left, right] = args.try_into().unwrap(); // TODO: collapse in if after bumping MSRV (and same below)
                let left = Self::normalize_expression(left, types);
                let left_types = infer_expression_type(&left, types);
                let right = Self::normalize_expression(right, types);
                let right_types = infer_expression_type(&right, types);
                #[cfg_attr(not(feature = "sparql-12"), expect(unused_mut))]
                let mut must_use_equal = left_types.literal && right_types.literal;
                #[cfg(feature = "sparql-12")]
                {
                    must_use_equal = must_use_equal || left_types.triple && right_types.triple;
                }
                if must_use_equal {
                    Expression::equal(left, right)
                } else {
                    Expression::same_term(left, right)
                }
            }
            Expression::FunctionCall(name, args)
                if name == sparql::NOT_EQUALS && args.len() == 2 =>
            {
                let [left, right] = args.try_into().unwrap();
                let left = Self::normalize_expression(left, types);
                let left_types = infer_expression_type(&left, types);
                let right = Self::normalize_expression(right, types);
                let right_types = infer_expression_type(&right, types);
                #[cfg_attr(not(feature = "sparql-12"), expect(unused_mut))]
                let mut must_use_equal = left_types.literal && right_types.literal;
                #[cfg(feature = "sparql-12")]
                {
                    must_use_equal = must_use_equal || left_types.triple && right_types.triple;
                }
                if must_use_equal {
                    Expression::not_equal(left, right)
                } else {
                    !Expression::same_term(left, right)
                }
            }
            Expression::FunctionCall(name, args)
                if name == sparql::LOGICAL_NOT && args.len() == 1 =>
            {
                let [arg] = args.try_into().unwrap();
                !Self::normalize_expression(arg, types)
            }
            Expression::FunctionCall(name, args)
                if name == sparql::SAME_TERM && args.len() == 2 =>
            {
                let [left, right] = args.try_into().unwrap();
                Expression::same_term(
                    Self::normalize_expression(left, types),
                    Self::normalize_expression(right, types),
                )
            }
            Expression::Exists(inner) => {
                Self::optimize_exists(Self::normalize_pattern(*inner, types), types)
            }
            Expression::Bound(variable) => {
                let t = types.get(&variable);
                if !t.undef {
                    true.into()
                } else if t == VariableType::UNDEF {
                    false.into()
                } else {
                    Expression::Bound(variable)
                }
            }
            Expression::If(cond, then, els) => Expression::if_cond(
                Self::normalize_expression(*cond, types),
                Self::normalize_expression(*then, types),
                Self::normalize_expression(*els, types),
            ),
            Expression::Coalesce(inners) => Expression::coalesce(
                inners
                    .into_iter()
                    .map(|e| Self::normalize_expression(e, types))
                    .collect(),
            ),
            Expression::FunctionCall(name, args) => Expression::call(
                name,
                args.into_iter()
                    .map(|e| Self::normalize_expression(e, types))
                    .collect(),
            ),
        }
    }

    fn optimize_exists(expression: QueryExpression, input_types: &VariableTypes) -> Expression {
        // We rewrite `EXISTS { A UNION B } into `EXISTS { A } || EXISTS { B }`
        if let QueryExpression::Union { inner } = expression {
            Expression::or_all(
                inner
                    .into_iter()
                    .map(|e| Self::optimize_exists(e, input_types)),
            )
        } else if let QueryExpression::Join {
            left,
            right,
            algorithm,
        } = expression
        {
            // Check if the join is actually joining anything or is just a cartesian product
            // If it's a cartesian product we can rewrite it as the && of two EXISTS
            let mut has_shared_variable_not_set_by_parent = false;
            left.lookup_used_variables(&mut |left_variable| {
                right.lookup_used_variables(&mut |right_variable| {
                    if left_variable == right_variable && input_types.get(left_variable).undef {
                        has_shared_variable_not_set_by_parent = true;
                    }
                });
            });
            if !has_shared_variable_not_set_by_parent {
                return Expression::and_all([
                    Self::optimize_exists(*left, input_types),
                    Self::optimize_exists(*right, input_types),
                ]);
            }
            Expression::exists(QueryExpression::Join {
                left,
                right,
                algorithm,
            })
        } else {
            Expression::exists(expression)
        }
    }

    fn push_filters(
        query_expression: QueryExpression,
        mut filters: Vec<Expression>,
        input_types: &VariableTypes,
    ) -> QueryExpression {
        match query_expression {
            QueryExpression::QuadPattern { .. }
            | QueryExpression::Path { .. }
            | QueryExpression::Values { .. } => {
                QueryExpression::filter(query_expression, Expression::and_all(filters))
            }
            QueryExpression::Join {
                left,
                right,
                algorithm,
            } => {
                let left_types = infer_query_expression_types(&left, input_types.clone());
                let right_types = infer_query_expression_types(&right, input_types.clone());
                let mut left_filters = Vec::new();
                let mut right_filters = Vec::new();
                let mut final_filters = Vec::new();
                for filter in filters {
                    let push_left = can_filter_be_pushed(&filter, &left_types, &right_types);
                    let push_right = can_filter_be_pushed(&filter, &right_types, &left_types);
                    if push_left {
                        if push_right {
                            left_filters.push(filter.clone());
                            right_filters.push(filter);
                        } else {
                            left_filters.push(filter);
                        }
                    } else if push_right {
                        right_filters.push(filter);
                    } else {
                        final_filters.push(filter);
                    }
                }
                QueryExpression::filter(
                    QueryExpression::join(
                        Self::push_filters(*left, left_filters, input_types),
                        Self::push_filters(*right, right_filters, input_types),
                        algorithm,
                    ),
                    Expression::and_all(final_filters),
                )
            }
            #[cfg(feature = "sep-0006")]
            QueryExpression::Lateral { left, right } => {
                let left_types = infer_query_expression_types(&left, input_types.clone());
                let right_types = infer_query_expression_types(&right, input_types.clone());
                let mut left_filters = Vec::new();
                let mut right_filters = Vec::new();
                for filter in filters {
                    let push_left = can_filter_be_pushed(&filter, &left_types, &right_types);
                    if push_left {
                        left_filters.push(filter);
                    } else {
                        right_filters.push(filter);
                    }
                }
                let left = Self::push_filters(*left, left_filters, input_types);
                let right = Self::push_filters(*right, right_filters, &left_types);
                if let QueryExpression::Filter {
                    inner: inner_right,
                    expression,
                } = right
                {
                    // We prefer to have filter out of the lateral rather than inside the right part
                    QueryExpression::filter(
                        QueryExpression::lateral(left, *inner_right),
                        expression,
                    )
                } else {
                    QueryExpression::lateral(left, right)
                }
            }
            QueryExpression::LeftJoin {
                left,
                right,
                expression,
                algorithm,
            } => {
                let left_types = infer_query_expression_types(&left, input_types.clone());
                let right_types = infer_query_expression_types(&right, input_types.clone());
                let mut left_filters = Vec::new();
                let mut right_filters = Vec::new();
                let mut final_filters = Vec::new();
                for filter in filters {
                    let push_left = can_filter_be_pushed(&filter, &left_types, &right_types);
                    if push_left {
                        left_filters.push(filter);
                    } else {
                        final_filters.push(filter);
                    }
                }
                let expression = if expression.effective_boolean_value().is_none()
                    && (are_all_expression_variables_bound(&expression, &right_types)
                        || are_no_expression_variables_bound(&expression, &left_types))
                {
                    right_filters.push(expression);
                    true.into()
                } else {
                    expression
                };
                QueryExpression::filter(
                    QueryExpression::left_join(
                        Self::push_filters(*left, left_filters, input_types),
                        Self::push_filters(*right, right_filters, input_types),
                        expression,
                        algorithm,
                    ),
                    Expression::and_all(final_filters),
                )
            }
            QueryExpression::Minus {
                left,
                right,
                algorithm,
            } => QueryExpression::minus(
                Self::push_filters(*left, filters, input_types),
                Self::push_filters(*right, Vec::new(), input_types),
                algorithm,
            ),
            QueryExpression::Graph { inner, graph_name } => {
                let mut filter_to_push = Vec::with_capacity(filters.len());
                let mut filters_to_write = Vec::with_capacity(filters.len());
                for filter in filters {
                    if !does_contain_exists(&filter)
                        && if let NamedNodePattern::Variable(v) = &graph_name {
                            !filter.used_variables().contains(v)
                        } else {
                            true
                        }
                    {
                        // The graph variable and EXISTS are not used, we can push the EXPRESSION further
                        filter_to_push.push(filter);
                    } else {
                        filters_to_write.push(filter);
                    }
                }
                let mut pattern = QueryExpression::graph(
                    Self::push_filters(*inner, filter_to_push, input_types),
                    graph_name,
                );
                if !filters_to_write.is_empty() {
                    pattern =
                        QueryExpression::filter(pattern, Expression::and_all(filters_to_write));
                }
                pattern
            }
            QueryExpression::Extend {
                inner,
                expression,
                variable,
            } => {
                let inner = if filters
                    .iter()
                    .any(|filter| filter.used_variables().contains(&variable))
                    && !does_contain_exists(&expression)
                    && is_deterministic(&expression)
                {
                    // Some filters are on the variable, we compute it as early as possible to filter early
                    match Self::sink_extend(*inner, &variable, &expression, input_types) {
                        Ok(new_inner) => {
                            return Self::push_filters(new_inner, filters, input_types);
                        }
                        Err(inner) => inner,
                    }
                } else {
                    inner
                };
                // TODO: handle the case where the filter overrides an expression variable (should not happen in SPARQL but allowed in the algebra)
                let mut inner_filters = Vec::new();
                let mut final_filters = Vec::new();
                for filter in filters {
                    let extend_variable_used =
                        filter.used_variables().into_iter().any(|v| *v == variable);
                    if extend_variable_used {
                        final_filters.push(filter);
                    } else {
                        inner_filters.push(filter);
                    }
                }
                QueryExpression::filter(
                    QueryExpression::extend(
                        Self::push_filters(*inner, inner_filters, input_types),
                        variable,
                        expression,
                    ),
                    Expression::and_all(final_filters),
                )
            }
            QueryExpression::Filter { inner, expression } => {
                if let Expression::And(expressions) = expression {
                    filters.extend(expressions)
                } else {
                    filters.push(expression)
                }
                Self::push_filters(*inner, filters, input_types)
            }
            QueryExpression::Union { inner } => QueryExpression::union_all(
                inner
                    .into_iter()
                    .map(|c| Self::push_filters(c, filters.clone(), input_types)),
            ),
            QueryExpression::Slice {
                inner,
                offset,
                limit,
            } => QueryExpression::filter(
                QueryExpression::slice(
                    Self::push_filters(*inner, Vec::new(), input_types),
                    offset,
                    limit,
                ),
                Expression::and_all(filters),
            ),
            QueryExpression::Distinct { inner } => {
                QueryExpression::distinct(Self::push_filters(*inner, filters, input_types))
            }
            QueryExpression::Reduced { inner } => {
                QueryExpression::reduced(Self::push_filters(*inner, filters, input_types))
            }
            QueryExpression::Project { inner, variables } => {
                let mut inner_filters = Vec::new();
                let mut final_filters = Vec::new();
                for filter in filters {
                    if filter
                        .used_variables()
                        .into_iter()
                        .all(|variable| variables.contains(variable))
                    {
                        inner_filters.push(filter);
                    } else {
                        // Variables not projected by a subquery are not in scope outside of it.
                        // Pushing such a filter would make the subquery's hidden bindings visible
                        // to the filter, in particular for correlated EXISTS expressions.
                        final_filters.push(filter);
                    }
                }
                QueryExpression::filter(
                    QueryExpression::project(
                        Self::push_filters(*inner, inner_filters, input_types),
                        variables,
                    ),
                    Expression::and_all(final_filters),
                )
            }
            QueryExpression::OrderBy { inner, expression } => QueryExpression::order_by(
                Self::push_filters(*inner, filters, input_types),
                expression,
            ),
            QueryExpression::Service { .. } => {
                // TODO: we can be smart and push some filters
                // But we need to check the behavior of SILENT that can transform no results into a singleton
                QueryExpression::filter(query_expression, Expression::and_all(filters))
            }
            QueryExpression::Group {
                inner,
                variables,
                aggregates,
            } => QueryExpression::filter(
                QueryExpression::group(
                    Self::push_filters(*inner, Vec::new(), input_types),
                    variables,
                    aggregates,
                ),
                Expression::and_all(filters),
            ),
        }
    }

    fn push_graph(
        query_expression: QueryExpression,
        current_graph: Option<NamedNodePattern>,
        input_types: &VariableTypes,
    ) -> QueryExpression {
        match query_expression {
            QueryExpression::QuadPattern {
                subject,
                predicate,
                object,
                graph_name,
            } => {
                if graph_name.is_some() {
                    unreachable!("Already set quad pattern graph name")
                }
                QueryExpression::QuadPattern {
                    subject,
                    predicate,
                    object,
                    graph_name: current_graph,
                }
            }
            QueryExpression::Path { .. } | QueryExpression::Values { .. } => {
                wrap_in_possible_graph(query_expression, current_graph)
            }
            QueryExpression::Graph { graph_name, inner } => {
                if let Some(current_graph) = current_graph {
                    if current_graph == graph_name {
                        // Same graph name, no need to keep the outer one
                        Self::push_graph(*inner, Some(graph_name), input_types)
                    } else {
                        QueryExpression::graph(
                            Self::push_graph(*inner, Some(graph_name), input_types),
                            current_graph,
                        )
                    }
                } else {
                    Self::push_graph(*inner, Some(graph_name), input_types)
                }
            }
            QueryExpression::Join {
                left,
                right,
                algorithm,
            } => {
                if matches!(*left, QueryExpression::Values { .. }) {
                    QueryExpression::join(
                        *left,
                        Self::push_graph(*right, current_graph, input_types),
                        algorithm,
                    )
                } else if matches!(*right, QueryExpression::Values { .. }) {
                    QueryExpression::join(
                        Self::push_graph(*left, current_graph, input_types),
                        *right,
                        algorithm,
                    )
                } else {
                    QueryExpression::join(
                        Self::push_graph(*left, current_graph.clone(), input_types),
                        Self::push_graph(*right, current_graph, input_types),
                        algorithm,
                    )
                }
            }
            QueryExpression::Filter { inner, expression } => {
                if !does_contain_exists(&expression)
                    && current_graph.as_ref().is_none_or(|pattern| {
                        if let NamedNodePattern::Variable(v) = pattern {
                            !expression.used_variables().contains(v)
                        } else {
                            true
                        }
                    })
                {
                    // The graph variable is not used, we can push the GRAPH operator further
                    QueryExpression::filter(
                        Self::push_graph(*inner, current_graph, input_types),
                        expression,
                    )
                } else {
                    wrap_in_possible_graph(
                        QueryExpression::filter(
                            Self::push_graph(*inner, None, input_types),
                            expression,
                        ),
                        current_graph,
                    )
                }
            }
            QueryExpression::Union { inner } => QueryExpression::union_all(
                inner
                    .into_iter()
                    .map(|c| Self::push_graph(c, current_graph.clone(), input_types)),
            ),
            QueryExpression::LeftJoin {
                left,
                right,
                expression,
                algorithm,
            } => {
                if !does_contain_exists(&expression)
                    && current_graph.as_ref().is_none_or(|pattern| {
                        if let NamedNodePattern::Variable(v) = pattern {
                            !expression.used_variables().contains(v)
                                && infer_query_expression_types(&right, input_types.clone()).get(v)
                                    == VariableType::UNDEF
                        } else {
                            true
                        }
                    })
                {
                    // Expression is safe and the graph variable is not used in right
                    QueryExpression::left_join(
                        Self::push_graph(*left, current_graph.clone(), input_types),
                        Self::push_graph(*right, current_graph, input_types),
                        expression,
                        algorithm,
                    )
                } else {
                    wrap_in_possible_graph(
                        QueryExpression::left_join(
                            Self::push_graph(*left, None, input_types),
                            Self::push_graph(*right, None, input_types),
                            expression,
                            algorithm,
                        ),
                        current_graph,
                    )
                }
            }
            #[cfg(feature = "sep-0006")]
            QueryExpression::Lateral { left, right } => wrap_in_possible_graph(
                QueryExpression::lateral(
                    Self::push_graph(*left, None, input_types),
                    Self::push_graph(*right, None, input_types),
                ),
                current_graph,
            ),
            QueryExpression::Extend {
                inner,
                variable,
                expression,
            } => {
                if !does_contain_exists(&expression)
                    && current_graph.as_ref().is_none_or(|pattern| {
                        if let NamedNodePattern::Variable(v) = pattern {
                            variable != *v && !expression.used_variables().contains(v)
                        } else {
                            true
                        }
                    })
                {
                    // The graph variable is not used, we can push the GRAPH operator further
                    QueryExpression::extend(
                        Self::push_graph(*inner, current_graph, input_types),
                        variable,
                        expression,
                    )
                } else {
                    wrap_in_possible_graph(
                        QueryExpression::extend(
                            Self::push_graph(*inner, None, input_types),
                            variable,
                            expression,
                        ),
                        current_graph,
                    )
                }
            }
            QueryExpression::Minus {
                left,
                right,
                algorithm,
            } => {
                let left_variables = infer_query_expression_types(&left, input_types.clone());
                let right_variables = infer_query_expression_types(&right, input_types.clone());
                if left_variables
                    .iter()
                    .any(|(v, t)| !t.undef && !right_variables.get(v).undef)
                    && current_graph.as_ref().is_none_or(|pattern| {
                        if let NamedNodePattern::Variable(v) = pattern {
                            // In the right side, the graph variable is not bound to the graph name
                            right_variables.get(v) == VariableType::UNDEF
                        } else {
                            true
                        }
                    })
                {
                    // We know we are not in the disjoint case, we can propagate
                    QueryExpression::minus(
                        Self::push_graph(*left, current_graph.clone(), input_types),
                        Self::push_graph(*right, current_graph, input_types),
                        algorithm,
                    )
                } else {
                    wrap_in_possible_graph(
                        QueryExpression::minus(
                            Self::push_graph(*left, None, input_types),
                            Self::push_graph(*right, None, input_types),
                            algorithm,
                        ),
                        current_graph,
                    )
                }
            }
            QueryExpression::OrderBy { inner, expression } => wrap_in_possible_graph(
                QueryExpression::order_by(Self::push_graph(*inner, None, input_types), expression),
                current_graph,
            ),
            QueryExpression::Project { inner, variables } => wrap_in_possible_graph(
                QueryExpression::project(Self::push_graph(*inner, None, input_types), variables),
                current_graph,
            ),
            QueryExpression::Distinct { inner } => {
                QueryExpression::distinct(Self::push_graph(*inner, current_graph, input_types))
            }
            QueryExpression::Reduced { inner } => {
                QueryExpression::distinct(Self::push_graph(*inner, current_graph, input_types))
            }
            QueryExpression::Slice {
                inner,
                offset,
                limit,
            } => wrap_in_possible_graph(
                QueryExpression::slice(Self::push_graph(*inner, None, input_types), offset, limit),
                current_graph,
            ),
            QueryExpression::Group {
                inner,
                variables,
                aggregates,
            } => wrap_in_possible_graph(
                QueryExpression::group(
                    Self::push_graph(*inner, None, input_types),
                    variables,
                    aggregates,
                ),
                current_graph,
            ),
            QueryExpression::Service {
                name,
                inner,
                silent,
            } => wrap_in_possible_graph(
                QueryExpression::service(Self::push_graph(*inner, None, input_types), name, silent),
                current_graph,
            ),
        }
    }

    /// Moves BIND, FILTER and OPTIONAL out of join operands when it does not change the query semantics.
    ///
    /// This way the join reordering sees all the triple patterns of a group graph pattern as a single join,
    /// even if they are separated by BIND, FILTER or OPTIONAL.
    /// FILTERs on IRI disjunctions like `?v IN (ex:a, ex:b)` are also turned into joins with VALUES.
    fn flatten_joins(
        query_expression: QueryExpression,
        input_types: &VariableTypes,
    ) -> QueryExpression {
        match query_expression {
            QueryExpression::QuadPattern { .. }
            | QueryExpression::Path { .. }
            | QueryExpression::Values { .. }
            | QueryExpression::Service { .. } => query_expression,
            QueryExpression::Join { left, right, .. } => Self::lift_out_of_join(
                Self::flatten_joins(*left, input_types),
                Self::flatten_joins(*right, input_types),
                input_types,
            ),
            QueryExpression::LeftJoin {
                left,
                right,
                expression,
                algorithm,
            } => QueryExpression::left_join(
                Self::flatten_joins(*left, input_types),
                Self::flatten_joins(*right, input_types),
                expression,
                algorithm,
            ),
            #[cfg(feature = "sep-0006")]
            QueryExpression::Lateral { left, right } => {
                let left = Self::flatten_joins(*left, input_types);
                let left_types = infer_query_expression_types(&left, input_types.clone());
                QueryExpression::lateral(left, Self::flatten_joins(*right, &left_types))
            }
            QueryExpression::Filter { inner, expression } => Self::filter_with_values(
                Self::flatten_joins(*inner, input_types),
                expression,
                input_types,
            ),
            QueryExpression::Union { inner } => QueryExpression::union_all(
                inner
                    .into_iter()
                    .map(|e| Self::flatten_joins(e, input_types)),
            ),
            QueryExpression::Extend {
                inner,
                variable,
                expression,
            } => QueryExpression::extend(
                Self::flatten_joins(*inner, input_types),
                variable,
                expression,
            ),
            QueryExpression::Minus {
                left,
                right,
                algorithm,
            } => QueryExpression::minus(
                Self::flatten_joins(*left, input_types),
                Self::flatten_joins(*right, input_types),
                algorithm,
            ),
            QueryExpression::Graph { graph_name, inner } => {
                QueryExpression::graph(Self::flatten_joins(*inner, input_types), graph_name)
            }
            QueryExpression::OrderBy { inner, expression } => {
                QueryExpression::order_by(Self::flatten_joins(*inner, input_types), expression)
            }
            QueryExpression::Project { inner, variables } => QueryExpression::project(
                Self::flatten_joins(*inner, &input_types.restricted_to(&variables)),
                variables,
            ),
            QueryExpression::Distinct { inner } => {
                QueryExpression::distinct(Self::flatten_joins(*inner, input_types))
            }
            QueryExpression::Reduced { inner } => {
                QueryExpression::reduced(Self::flatten_joins(*inner, input_types))
            }
            QueryExpression::Slice {
                inner,
                offset,
                limit,
            } => QueryExpression::slice(Self::flatten_joins(*inner, input_types), offset, limit),
            QueryExpression::Group {
                inner,
                variables,
                aggregates,
            } => QueryExpression::group(
                Self::flatten_joins(*inner, &input_types.restricted_to(&variables)),
                variables,
                aggregates,
            ),
        }
    }

    /// Builds the join of `left` and `right`, lifting BIND, FILTER and OPTIONAL from them above the join if possible
    fn lift_out_of_join(
        left: QueryExpression,
        right: QueryExpression,
        input_types: &VariableTypes,
    ) -> QueryExpression {
        // The join is commutative, we try both sides
        match Self::lift_from_join_operand(left, right, input_types) {
            Ok(result) => result,
            Err(operands) => {
                let (left, right) = *operands;
                match Self::lift_from_join_operand(right, left, input_types) {
                    Ok(result) => result,
                    Err(operands) => {
                        let (right, left) = *operands;
                        QueryExpression::join(left, right, JoinAlgorithm::default())
                    }
                }
            }
        }
    }

    /// Lifts the BIND, FILTER or OPTIONAL at the root of `operand` above its join with `other`
    ///
    /// Returns back `operand` and `other` if it is not possible
    fn lift_from_join_operand(
        operand: QueryExpression,
        other: QueryExpression,
        input_types: &VariableTypes,
    ) -> Result<QueryExpression, Box<(QueryExpression, QueryExpression)>> {
        match operand {
            QueryExpression::Extend {
                inner,
                variable,
                expression,
            } => {
                // Join(Extend(A, v, e), B) = Extend(Join(A, B), v, e) if B does not bind v
                // and the variables of e bound by B are always bound by A
                let other_types = infer_query_expression_types(&other, input_types.clone());
                if !does_contain_exists(&expression)
                    && is_deterministic(&expression)
                    && other_types.get(&variable) == VariableType::UNDEF
                    && are_variables_unchanged_by_join(
                        expression.used_variables(),
                        &infer_query_expression_types(&inner, input_types.clone()),
                        &other_types,
                    )
                {
                    Ok(QueryExpression::extend(
                        Self::lift_out_of_join(*inner, other, input_types),
                        variable,
                        expression,
                    ))
                } else {
                    Err(Box::new((
                        QueryExpression::Extend {
                            inner,
                            variable,
                            expression,
                        },
                        other,
                    )))
                }
            }
            QueryExpression::Filter { inner, expression } => {
                // Join(Filter(A, f), B) = Filter(Join(A, B), f) if the variables of f bound by B are always bound by A
                if !does_contain_exists(&expression)
                    && is_deterministic(&expression)
                    && are_variables_unchanged_by_join(
                        expression.used_variables(),
                        &infer_query_expression_types(&inner, input_types.clone()),
                        &infer_query_expression_types(&other, input_types.clone()),
                    )
                {
                    Ok(QueryExpression::filter(
                        Self::lift_out_of_join(*inner, other, input_types),
                        expression,
                    ))
                } else {
                    Err(Box::new((
                        QueryExpression::Filter { inner, expression },
                        other,
                    )))
                }
            }
            QueryExpression::LeftJoin {
                left,
                right,
                expression,
                algorithm,
            } => {
                // Join(LeftJoin(A, B, f), C) = LeftJoin(Join(A, C), B, f)
                // if the variables of B and f bound by C are always bound by A (well-designed patterns)
                let left_types = infer_query_expression_types(&left, input_types.clone());
                let other_types = infer_query_expression_types(&other, input_types.clone());
                let right_types = infer_query_expression_types(&right, input_types.clone());
                if !does_contain_exists(&expression)
                    && is_deterministic(&expression)
                    && are_variables_unchanged_by_join(
                        right_types
                            .iter()
                            .filter(|(_, t)| **t != VariableType::UNDEF)
                            .map(|(v, _)| v),
                        &left_types,
                        &other_types,
                    )
                    && are_variables_unchanged_by_join(
                        expression.used_variables(),
                        &left_types,
                        &other_types,
                    )
                {
                    Ok(QueryExpression::left_join(
                        Self::lift_out_of_join(*left, other, input_types),
                        *right,
                        expression,
                        algorithm,
                    ))
                } else {
                    Err(Box::new((
                        QueryExpression::LeftJoin {
                            left,
                            right,
                            expression,
                            algorithm,
                        },
                        other,
                    )))
                }
            }
            _ => Err(Box::new((operand, other))),
        }
    }

    /// Rewrites `FILTER(?v IN (ex:a, ex:b))` into a join with `VALUES ?v { ex:a ex:b }`
    ///
    /// The variable must be always bound by the filtered expression.
    /// We only do it with IRIs because value equality is not the same as term equality on literals.
    fn filter_with_values(
        mut inner: QueryExpression,
        expression: Expression,
        input_types: &VariableTypes,
    ) -> QueryExpression {
        let inner_types = infer_query_expression_types(&inner, input_types.clone());
        let conjuncts = if let Expression::And(conjuncts) = expression {
            conjuncts
        } else {
            vec![expression]
        };
        let mut remaining_conjuncts = Vec::new();
        for conjunct in conjuncts {
            match iri_disjunction(&conjunct) {
                Some((variable, iris)) if !inner_types.get(&variable).undef => {
                    let values = QueryExpression::values(
                        vec![variable],
                        iris.into_iter()
                            .map(|iri| vec![Some(GroundTerm::NamedNode(iri))])
                            .collect(),
                    );
                    inner = Self::lift_out_of_join(inner, values, input_types);
                }
                _ => remaining_conjuncts.push(conjunct),
            }
        }
        QueryExpression::filter(inner, Expression::and_all(remaining_conjuncts))
    }

    /// Moves the computation of `variable` as deep as possible into `inner` i.e. just after all the variables it depends on are bound
    ///
    /// Returns back `inner` if it is not possible
    fn sink_extend(
        pattern: QueryExpression,
        variable: &Variable,
        expression: &Expression,
        input_types: &VariableTypes,
    ) -> Result<QueryExpression, Box<QueryExpression>> {
        let is_computable_from = |pattern: &QueryExpression| {
            are_all_expression_variables_bound(
                expression,
                &infer_query_expression_types(pattern, input_types.clone()),
            )
        };
        let sink_or_extend = |pattern: QueryExpression| {
            Self::sink_extend(pattern, variable, expression, input_types).unwrap_or_else(
                |pattern| QueryExpression::extend(*pattern, variable.clone(), expression.clone()),
            )
        };
        match pattern {
            #[cfg(feature = "sep-0006")]
            QueryExpression::Lateral { left, right } => {
                if !is_variable_used(&right, variable) && is_computable_from(&left) {
                    Ok(QueryExpression::lateral(sink_or_extend(*left), *right))
                } else {
                    Err(Box::new(QueryExpression::Lateral { left, right }))
                }
            }
            QueryExpression::Join {
                left,
                right,
                algorithm,
            } => {
                if !is_variable_used(&right, variable) && is_computable_from(&left) {
                    Ok(QueryExpression::join(
                        sink_or_extend(*left),
                        *right,
                        algorithm,
                    ))
                } else if !is_variable_used(&left, variable) && is_computable_from(&right) {
                    Ok(QueryExpression::join(
                        *left,
                        sink_or_extend(*right),
                        algorithm,
                    ))
                } else {
                    Err(Box::new(QueryExpression::Join {
                        left,
                        right,
                        algorithm,
                    }))
                }
            }
            QueryExpression::LeftJoin {
                left,
                right,
                expression: left_join_expression,
                algorithm,
            } => {
                if !is_variable_used(&right, variable)
                    && !left_join_expression.used_variables().contains(variable)
                    && is_computable_from(&left)
                {
                    Ok(QueryExpression::left_join(
                        sink_or_extend(*left),
                        *right,
                        left_join_expression,
                        algorithm,
                    ))
                } else {
                    Err(Box::new(QueryExpression::LeftJoin {
                        left,
                        right,
                        expression: left_join_expression,
                        algorithm,
                    }))
                }
            }
            QueryExpression::Filter {
                inner,
                expression: filter,
            } if !filter.used_variables().contains(variable) => {
                match Self::sink_extend(*inner, variable, expression, input_types) {
                    Ok(inner) => Ok(QueryExpression::filter(inner, filter)),
                    Err(inner) => Err(Box::new(QueryExpression::filter(*inner, filter))),
                }
            }
            _ => Err(Box::new(pattern)),
        }
    }
}

/// Reorders the joins using cardinality and cost estimates.
///
/// For each join group, it picks the order of the operands and,
/// for each operand, between a hash join and a for-loop join (`Lateral`) where the operand is evaluated once per row with the variables already bound.
struct JoinReorderer<'a> {
    estimator: Estimator<'a>,
}

impl JoinReorderer<'_> {
    fn reorder_joins(
        &self,
        query_expression: QueryExpression,
        input_types: &VariableTypes,
    ) -> QueryExpression {
        self.reorder_joins_with_filters(query_expression, input_types, &[])
    }

    /// `filters` are applied on top of `query_expression`, they are only used to improve the estimates
    fn reorder_joins_with_filters(
        &self,
        query_expression: QueryExpression,
        input_types: &VariableTypes,
        filters: &[Expression],
    ) -> QueryExpression {
        match query_expression {
            QueryExpression::QuadPattern { .. }
            | QueryExpression::Path { .. }
            | QueryExpression::Values { .. } => query_expression,
            QueryExpression::Join { left, right, .. } => {
                // We flatten the join operation
                let mut operands = Vec::new();
                let mut todo = vec![*right, *left];
                while let Some(e) = todo.pop() {
                    if let QueryExpression::Join { left, right, .. } = e {
                        todo.push(*right);
                        todo.push(*left);
                    } else {
                        operands.push(e);
                    }
                }
                self.reorder_join_operands(operands, input_types, filters)
            }
            #[cfg(feature = "sep-0006")]
            QueryExpression::Lateral { left, right } => {
                let left_types = infer_query_expression_types(&left, input_types.clone());
                QueryExpression::lateral(
                    self.reorder_joins_with_filters(*left, input_types, filters),
                    self.reorder_joins(*right, &left_types),
                )
            }
            QueryExpression::LeftJoin {
                left,
                right,
                expression,
                ..
            } => self.reorder_left_join(*left, *right, expression, input_types, filters),
            QueryExpression::Minus { left, right, .. } => {
                let left = self.reorder_joins_with_filters(*left, input_types, filters);
                let left_types = infer_query_expression_types(&left, input_types.clone());
                let right = self.reorder_joins(*right, input_types);
                let right_types = infer_query_expression_types(&right, input_types.clone());
                QueryExpression::minus(
                    left,
                    right,
                    MinusAlgorithm::HashBuildRightProbeLeft {
                        keys: join_key_variables(&left_types, &right_types, input_types),
                    },
                )
            }
            QueryExpression::Graph { graph_name, inner } => {
                let inner_filters = filters
                    .iter()
                    .filter(|filter| {
                        !does_contain_exists(filter)
                            && if let NamedNodePattern::Variable(v) = &graph_name {
                                !filter.used_variables().contains(v)
                            } else {
                                true
                            }
                    })
                    .cloned()
                    .collect::<Vec<_>>();
                QueryExpression::graph(
                    self.reorder_joins_with_filters(*inner, input_types, &inner_filters),
                    graph_name,
                )
            }
            QueryExpression::Extend {
                inner,
                expression,
                variable,
            } => {
                let inner_filters = filters
                    .iter()
                    .filter(|filter| !filter.used_variables().contains(&variable))
                    .cloned()
                    .collect::<Vec<_>>();
                QueryExpression::extend(
                    self.reorder_joins_with_filters(*inner, input_types, &inner_filters),
                    variable,
                    expression,
                )
            }
            QueryExpression::Filter { inner, expression } => {
                let mut inner_filters = filters.to_vec();
                if let Expression::And(conjuncts) = &expression {
                    inner_filters.extend(conjuncts.iter().cloned());
                } else {
                    inner_filters.push(expression.clone());
                }
                QueryExpression::filter(
                    self.reorder_joins_with_filters(*inner, input_types, &inner_filters),
                    expression,
                )
            }
            QueryExpression::Union { inner } => QueryExpression::union_all(
                inner
                    .into_iter()
                    .map(|c| self.reorder_joins_with_filters(c, input_types, filters)),
            ),
            QueryExpression::Slice {
                inner,
                offset,
                limit,
            } => QueryExpression::slice(self.reorder_joins(*inner, input_types), offset, limit),
            QueryExpression::Distinct { inner } => QueryExpression::distinct(
                self.reorder_joins_with_filters(*inner, input_types, filters),
            ),
            QueryExpression::Reduced { inner } => QueryExpression::reduced(
                self.reorder_joins_with_filters(*inner, input_types, filters),
            ),
            QueryExpression::Project { inner, variables } => QueryExpression::project(
                // The variables that are not projected are not the same as the outer ones
                self.reorder_joins(*inner, &input_types.restricted_to(&variables)),
                variables,
            ),
            QueryExpression::OrderBy { inner, expression } => QueryExpression::order_by(
                self.reorder_joins_with_filters(*inner, input_types, filters),
                expression,
            ),
            QueryExpression::Service { .. } => {
                // We don't do join reordering inside of SERVICE calls, we don't know about cardinalities
                query_expression
            }
            QueryExpression::Group {
                inner,
                variables,
                aggregates,
            } => QueryExpression::group(
                self.reorder_joins(*inner, &input_types.restricted_to(&variables)),
                variables,
                aggregates,
            ),
        }
    }

    fn reorder_left_join(
        &self,
        left: QueryExpression,
        right: QueryExpression,
        expression: Expression,
        input_types: &VariableTypes,
        filters: &[Expression],
    ) -> QueryExpression {
        let left = self.reorder_joins_with_filters(left, input_types, filters);
        let left_types = infer_query_expression_types(&left, input_types.clone());
        #[cfg(feature = "sep-0006")]
        {
            let right_types = infer_query_expression_types(&right, input_types.clone());
            if has_common_variables(&left_types, &right_types, input_types) {
                // We plan the right side with the left side variables bound to evaluate it once per left row
                let right_for_loop = self.reorder_joins(right.clone(), &left_types);
                if is_fit_for_for_loop_join(&right_for_loop, input_types, &left_types) {
                    let right = self.reorder_joins(right, input_types);
                    let left_estimate = self.estimator.estimate(&left, input_types);
                    let for_loop_estimate = self.estimator.estimate(&right_for_loop, &left_types);
                    let hash_estimate = self.estimator.estimate(&right, input_types);
                    if is_for_loop_join_useful(
                        left_estimate,
                        matches!(left, QueryExpression::Values { .. }),
                        for_loop_estimate,
                        hash_estimate,
                    ) && for_loop_join_cost(left_estimate, for_loop_estimate)
                        <= hash_join_cost(left_estimate, hash_estimate)
                    {
                        return QueryExpression::lateral(
                            left,
                            QueryExpression::left_join(
                                QueryExpression::empty_singleton(),
                                right_for_loop,
                                expression,
                                LeftJoinAlgorithm::HashBuildRightProbeLeft { keys: Vec::new() },
                            ),
                        );
                    }
                    return Self::hash_left_join(left, &left_types, right, expression, input_types);
                }
            }
        }
        let right = self.reorder_joins(right, input_types);
        Self::hash_left_join(left, &left_types, right, expression, input_types)
    }

    fn hash_left_join(
        left: QueryExpression,
        left_types: &VariableTypes,
        right: QueryExpression,
        expression: Expression,
        input_types: &VariableTypes,
    ) -> QueryExpression {
        let right_types = infer_query_expression_types(&right, input_types.clone());
        QueryExpression::left_join(
            left,
            right,
            expression,
            LeftJoinAlgorithm::HashBuildRightProbeLeft {
                keys: join_key_variables(left_types, &right_types, input_types),
            },
        )
    }

    fn reorder_join_operands(
        &self,
        operands: Vec<QueryExpression>,
        input_types: &VariableTypes,
        filters: &[Expression],
    ) -> QueryExpression {
        let operands = operands
            .into_iter()
            .map(|operand| JoinOperand::new(operand, input_types))
            .collect::<Vec<_>>();
        let filters = filters
            .iter()
            .map(|filter| FilterSelectivity {
                variables: filter.used_variables().into_iter().cloned().collect(),
                selectivity: filter_selectivity(filter),
            })
            .collect::<Vec<_>>();
        let mut components = connected_components(&operands, input_types)
            .into_iter()
            .map(|component| {
                let order = if component.len() <= MAX_OPERANDS_FOR_EXHAUSTIVE_SEARCH {
                    self.exhaustive_join_order(&operands, &component, input_types, &filters)
                } else {
                    self.greedy_join_order(&operands, &component, input_types, &filters)
                };
                self.build_join(&operands, &order, input_types)
            })
            .collect::<Vec<_>>();
        // We join the connected components together with cartesian products, starting from the smallest one
        components.sort_by(|(_, _, a), (_, _, b)| a.cardinality.total_cmp(&b.cardinality));
        components
            .into_iter()
            .reduce(
                |(left, mut left_types, left_estimate), (right, right_types, right_estimate)| {
                    let algorithm = hash_join_algorithm(
                        join_key_variables(&left_types, &right_types, input_types),
                        left_estimate,
                        right_estimate,
                    );
                    left_types.intersect_with(right_types);
                    (
                        QueryExpression::join(left, right, algorithm),
                        left_types,
                        Estimate::new(
                            left_estimate.cardinality * right_estimate.cardinality,
                            left_estimate.cost + hash_join_cost(left_estimate, right_estimate),
                        ),
                    )
                },
            )
            .map_or_else(QueryExpression::empty_singleton, |(e, _, _)| e)
    }

    /// Finds the cheapest left-deep join tree without cartesian products by dynamic programming over the operand sets.
    fn exhaustive_join_order(
        &self,
        operands: &[JoinOperand],
        component: &[usize],
        input_types: &VariableTypes,
        filters: &[FilterSelectivity],
    ) -> JoinOrder {
        // best[set] is the cheapest way to join the operands in the bit set
        let mut best = vec![None::<JoinOrder>; 1 << component.len()];
        for (i, operand) in component.iter().enumerate() {
            best[1 << i] = Some(self.start_join(operands, *operand, input_types, filters));
        }
        // Subsets are smaller than their supersets so they are always computed before
        for set in 1..best.len() {
            if set.is_power_of_two() {
                continue;
            }
            // Only the best candidate is built, the others are compared with their last step
            let mut best_candidate = None::<(&JoinOrder, JoinStep)>;
            for (i, operand) in component.iter().enumerate() {
                if set & (1 << i) == 0 {
                    continue;
                }
                let Some(previous) = &best[set ^ (1 << i)] else {
                    continue;
                };
                if !has_common_variables(
                    &previous.state.types,
                    &operands[*operand].types,
                    input_types,
                ) {
                    continue; // No cartesian product
                }
                let step = self.join_step(operands, *operand, &previous.state, input_types);
                if best_candidate
                    .as_ref()
                    .is_none_or(|(best_previous, best_step)| {
                        is_better_extension(previous, &step, best_previous, best_step)
                    })
                {
                    best_candidate = Some((previous, step));
                }
            }
            let best_for_set = best_candidate
                .map(|(previous, step)| Self::extend_join(operands, previous, step, filters));
            best[set] = best_for_set;
        }
        best.pop().flatten().unwrap_or_else(|| {
            unreachable!("The operands of a connected component are always joinable")
        })
    }

    /// Builds a join order by adding at each step the operand that is the cheapest to join with.
    ///
    /// If the component is not too big, the search is started from each operand and the cheapest result is kept.
    fn greedy_join_order(
        &self,
        operands: &[JoinOperand],
        component: &[usize],
        input_types: &VariableTypes,
        filters: &[FilterSelectivity],
    ) -> JoinOrder {
        let starts = if component.len() <= MAX_OPERANDS_FOR_ALL_STARTS {
            component.to_vec()
        } else {
            component
                .iter()
                .copied()
                .min_by(|a, b| {
                    self.plan_operand_alone(&operands[*a], input_types)
                        .estimate
                        .cardinality
                        .total_cmp(
                            &self
                                .plan_operand_alone(&operands[*b], input_types)
                                .estimate
                                .cardinality,
                        )
                })
                .into_iter()
                .collect()
        };
        let mut best = None::<JoinOrder>;
        for start in starts {
            let mut order = self.start_join(operands, start, input_types, filters);
            let mut remaining = component
                .iter()
                .copied()
                .filter(|i| *i != start)
                .collect::<Vec<_>>();
            while !remaining.is_empty() {
                let mut best_next = None::<(usize, JoinOrder)>;
                for (position, operand) in remaining.iter().enumerate() {
                    if !has_common_variables(
                        &order.state.types,
                        &operands[*operand].types,
                        input_types,
                    ) {
                        continue; // No cartesian product
                    }
                    let candidate =
                        self.add_to_join(operands, *operand, &order, input_types, filters);
                    if best_next.as_ref().is_none_or(|(_, b)| {
                        let (Some(candidate_step), Some(best_step)) =
                            (candidate.steps.last(), b.steps.last())
                        else {
                            unreachable!()
                        };
                        match compare_costs(candidate_step.cost, best_step.cost) {
                            Ordering::Less => true,
                            Ordering::Greater => false,
                            Ordering::Equal => candidate.state.cardinality < b.state.cardinality,
                        }
                    }) {
                        best_next = Some((position, candidate));
                    }
                }
                let Some((position, next)) = best_next else {
                    unreachable!("The operands of a connected component are always joinable")
                };
                remaining.remove(position);
                order = next;
            }
            if best.as_ref().is_none_or(|b| order.is_better_than(b)) {
                best = Some(order);
            }
        }
        best.unwrap_or_else(|| unreachable!("Components are never empty"))
    }

    /// The join with only `operand`
    fn start_join(
        &self,
        operands: &[JoinOperand],
        operand: usize,
        input_types: &VariableTypes,
        filters: &[FilterSelectivity],
    ) -> JoinOrder {
        let plan = self.plan_operand_alone(&operands[operand], input_types);
        let mut state = JoinState {
            types: operands[operand].types.clone(),
            cardinality: plan.estimate.cardinality,
            applied_filters: vec![false; filters.len()],
            is_values: matches!(operands[operand].expression, QueryExpression::Values { .. }),
        };
        state.apply_filters(filters);
        JoinOrder {
            start: operand,
            start_cardinality: plan.estimate.cardinality,
            steps: Vec::new(),
            state,
            cost: plan.estimate.cost,
        }
    }

    /// Joins `operand` to the current join, picking the cheapest join algorithm
    fn add_to_join(
        &self,
        operands: &[JoinOperand],
        operand: usize,
        order: &JoinOrder,
        input_types: &VariableTypes,
        filters: &[FilterSelectivity],
    ) -> JoinOrder {
        let step = self.join_step(operands, operand, &order.state, input_types);
        Self::extend_join(operands, order, step, filters)
    }

    /// The step joining `operand` to a join in `state`, with the cheapest join algorithm
    fn join_step(
        &self,
        operands: &[JoinOperand],
        operand: usize,
        state: &JoinState,
        input_types: &VariableTypes,
    ) -> JoinStep {
        let left = Estimate::new(state.cardinality, 0.);
        let hash_plan = self.plan_operand_alone(&operands[operand], input_types);
        let for_loop_plan = self.plan_operand(&operands[operand], &state.types, input_types);
        let hash_cost = hash_join_cost(left, hash_plan.estimate);
        let for_loop_cost = for_loop_join_cost(left, for_loop_plan.estimate);
        // The result is the same whatever the join algorithm is
        let cardinality = self.estimator.join_cardinality(
            state.cardinality,
            for_loop_plan.estimate.cardinality,
            || hash_plan.estimate.cardinality,
            || join_key_variables(&state.types, &operands[operand].types, input_types).len() == 1,
        );
        if for_loop_plan.is_fit_for_for_loop_join
            && is_for_loop_join_useful(
                left,
                state.is_values,
                for_loop_plan.estimate,
                hash_plan.estimate,
            )
            && for_loop_cost <= hash_cost
        {
            JoinStep {
                operand,
                plan: for_loop_plan,
                is_for_loop: true,
                cost: for_loop_cost,
                left_cardinality: state.cardinality,
                cardinality,
            }
        } else {
            JoinStep {
                operand,
                plan: hash_plan,
                is_for_loop: false,
                cost: hash_cost,
                left_cardinality: state.cardinality,
                cardinality,
            }
        }
    }

    /// The join order `order` followed by `step`
    fn extend_join(
        operands: &[JoinOperand],
        order: &JoinOrder,
        step: JoinStep,
        filters: &[FilterSelectivity],
    ) -> JoinOrder {
        let mut state = JoinState {
            types: order.state.types.clone(),
            cardinality: step.cardinality,
            applied_filters: order.state.applied_filters.clone(),
            is_values: false,
        };
        state
            .types
            .intersect_with(operands[step.operand].types.clone());
        state.apply_filters(filters);
        let cost = order.cost + step.cost;
        let mut steps = Vec::with_capacity(order.steps.len() + 1);
        steps.extend(order.steps.iter().cloned());
        steps.push(step);
        JoinOrder {
            start: order.start,
            start_cardinality: order.start_cardinality,
            steps,
            state,
            cost,
        }
    }

    /// Plans the operand evaluated with only the input variables bound (cached)
    fn plan_operand_alone(
        &self,
        operand: &JoinOperand,
        input_types: &VariableTypes,
    ) -> Rc<OperandPlan> {
        Rc::clone(
            operand
                .plan_alone
                .get_or_init(|| self.plan_operand(operand, input_types, input_types)),
        )
    }

    /// Plans the operand evaluated with the variables of `bound_types` bound (cached)
    fn plan_operand(
        &self,
        operand: &JoinOperand,
        bound_types: &VariableTypes,
        input_types: &VariableTypes,
    ) -> Rc<OperandPlan> {
        let key = operand
            .variables
            .iter()
            .map(|v| bound_types.get(v))
            .collect::<Vec<_>>();
        if let Some(plan) = operand.plans.borrow().get(&key) {
            return Rc::clone(plan);
        }
        let expression = self.reorder_joins(operand.expression.clone(), bound_types);
        let estimate = self.estimator.estimate(&expression, bound_types);
        let is_fit_for_for_loop_join = cfg!(feature = "sep-0006")
            && is_fit_for_for_loop_join(&expression, input_types, bound_types);
        let plan = Rc::new(OperandPlan {
            expression,
            estimate,
            is_fit_for_for_loop_join,
        });
        operand.plans.borrow_mut().insert(key, Rc::clone(&plan));
        plan
    }

    fn build_join(
        &self,
        operands: &[JoinOperand],
        order: &JoinOrder,
        input_types: &VariableTypes,
    ) -> (QueryExpression, VariableTypes, Estimate) {
        let mut output = self
            .plan_operand_alone(&operands[order.start], input_types)
            .expression
            .clone();
        let mut output_types = operands[order.start].types.clone();
        for step in &order.steps {
            #[cfg(feature = "sep-0006")]
            if step.is_for_loop {
                output = QueryExpression::lateral(output, step.plan.expression.clone());
                output_types.intersect_with(operands[step.operand].types.clone());
                continue;
            }
            output = QueryExpression::join(
                output,
                step.plan.expression.clone(),
                hash_join_algorithm(
                    join_key_variables(&output_types, &operands[step.operand].types, input_types),
                    Estimate::new(step.left_cardinality, 0.),
                    step.plan.estimate,
                ),
            );
            output_types.intersect_with(operands[step.operand].types.clone());
        }
        (
            output,
            output_types,
            Estimate::new(order.state.cardinality, order.cost),
        )
    }
}

/// An operand of a join group
struct JoinOperand {
    /// The expression as written in the query, it is planned again for each set of bound variables
    expression: QueryExpression,
    types: VariableTypes,
    variables: Vec<Variable>,
    /// The plans of the expression, keyed by the types of `variables` when it is evaluated
    plans: RefCell<FxHashMap<Vec<VariableType>, Rc<OperandPlan>>>,
    /// The plan with only the input variables bound, it is the most used
    plan_alone: OnceCell<Rc<OperandPlan>>,
}

impl JoinOperand {
    fn new(expression: QueryExpression, input_types: &VariableTypes) -> Self {
        let types = infer_query_expression_types(&expression, input_types.clone());
        let mut variables = Vec::new();
        expression.lookup_used_variables(&mut |v| {
            if !variables.contains(v) {
                variables.push(v.clone());
            }
        });
        Self {
            expression,
            types,
            variables,
            plans: RefCell::default(),
            plan_alone: OnceCell::new(),
        }
    }
}

struct OperandPlan {
    expression: QueryExpression,
    estimate: Estimate,
    is_fit_for_for_loop_join: bool,
}

struct FilterSelectivity {
    variables: Vec<Variable>,
    selectivity: f64,
}

#[derive(Clone)]
struct JoinState {
    types: VariableTypes,
    cardinality: f64,
    /// The filters whose variables are all bound and that are taken into account in the cardinality
    applied_filters: Vec<bool>,
    is_values: bool,
}

impl JoinState {
    fn apply_filters(&mut self, filters: &[FilterSelectivity]) {
        for (filter, applied) in filters.iter().zip(&mut self.applied_filters) {
            if !*applied && filter.variables.iter().all(|v| !self.types.get(v).undef) {
                *applied = true;
                self.cardinality *= filter.selectivity;
            }
        }
    }
}

#[derive(Clone)]
struct JoinStep {
    operand: usize,
    plan: Rc<OperandPlan>,
    #[cfg_attr(not(feature = "sep-0006"), expect(dead_code))]
    is_for_loop: bool,
    cost: f64,
    left_cardinality: f64,
    /// The cardinality of the join after this step, without the filters
    cardinality: f64,
}

/// A left-deep join tree
#[derive(Clone)]
struct JoinOrder {
    start: usize,
    start_cardinality: f64,
    steps: Vec<JoinStep>,
    /// The state after the last step
    state: JoinState,
    /// The cost of all the steps
    cost: f64,
}

impl JoinOrder {
    fn operands(&self) -> impl Iterator<Item = usize> + '_ {
        once(self.start).chain(self.steps.iter().map(|step| step.operand))
    }

    /// Lower cost wins, then the one starting with the smallest operand, then the order closest to the written one
    fn is_better_than(&self, other: &Self) -> bool {
        match compare_costs(self.cost, other.cost)
            .then_with(|| compare_costs(self.start_cardinality, other.start_cardinality))
        {
            Ordering::Less => true,
            Ordering::Greater => false,
            Ordering::Equal => self.operands().lt(other.operands()),
        }
    }
}

/// Compares `a` followed by `a_step` with `b` followed by `b_step` like [`JoinOrder::is_better_than`]
fn is_better_extension(a: &JoinOrder, a_step: &JoinStep, b: &JoinOrder, b_step: &JoinStep) -> bool {
    match compare_costs(a.cost + a_step.cost, b.cost + b_step.cost)
        .then_with(|| compare_costs(a.start_cardinality, b.start_cardinality))
    {
        Ordering::Less => true,
        Ordering::Greater => false,
        Ordering::Equal => a
            .operands()
            .chain(once(a_step.operand))
            .lt(b.operands().chain(once(b_step.operand))),
    }
}

/// Compares costs, ignoring the rounding errors
fn compare_costs(a: f64, b: f64) -> Ordering {
    if (a - b).abs() <= 1e-9 * a.abs().max(b.abs()) {
        Ordering::Equal
    } else {
        a.total_cmp(&b)
    }
}

/// Groups the join operands into sets connected by shared variables
fn connected_components(operands: &[JoinOperand], input_types: &VariableTypes) -> Vec<Vec<usize>> {
    /// Union-find root lookup with path compression
    fn root(component_of: &mut [usize], mut i: usize) -> usize {
        while component_of[i] != i {
            component_of[i] = component_of[component_of[i]];
            i = component_of[i];
        }
        i
    }

    let mut component_of = (0..operands.len()).collect::<Vec<_>>();
    for i in 0..operands.len() {
        for j in (i + 1)..operands.len() {
            if has_common_variables(&operands[i].types, &operands[j].types, input_types) {
                let (a, b) = (root(&mut component_of, i), root(&mut component_of, j));
                component_of[a.max(b)] = a.min(b);
            }
        }
    }
    let mut components = Vec::<Vec<usize>>::new();
    let mut component_index = HashMap::new();
    for i in 0..operands.len() {
        let root = root(&mut component_of, i);
        let index = *component_index.entry(root).or_insert_with(|| {
            components.push(Vec::new());
            components.len() - 1
        });
        components[index].push(i);
    }
    components
}

fn hash_join_cost(left: Estimate, right: Estimate) -> f64 {
    right.cost + left.cardinality + right.cardinality
}

fn for_loop_join_cost(left: Estimate, right: Estimate) -> f64 {
    left.cardinality * (SEEK_COST + right.cost)
}

/// A for-loop join is only worth it if the bindings from the left side make the right side more selective
///
/// Otherwise we would evaluate the same thing for each left row.
fn is_for_loop_join_useful(
    left: Estimate,
    is_left_values: bool,
    for_loop_right: Estimate,
    hash_right: Estimate,
) -> bool {
    for_loop_right.cardinality < hash_right.cardinality || left.cardinality <= 1. || is_left_values
}

/// The hash join builds its hash table from the smallest side
fn hash_join_algorithm(keys: Vec<Variable>, left: Estimate, right: Estimate) -> JoinAlgorithm {
    if right.cardinality < left.cardinality {
        JoinAlgorithm::HashBuildRightProbeLeft { keys }
    } else {
        JoinAlgorithm::HashBuildLeftProbeRight { keys }
    }
}

fn is_fit_for_for_loop_join(
    query_expression: &QueryExpression,
    global_input_types: &VariableTypes,
    entry_types: &VariableTypes,
) -> bool {
    // TODO: think more about it
    match query_expression {
        QueryExpression::Values { .. } | QueryExpression::QuadPattern { .. } => true,
        QueryExpression::Path {
            subject,
            path,
            object,
        } => is_path_fit_for_for_loop_join(subject, path, object, entry_types),
        #[cfg(feature = "sep-0006")]
        QueryExpression::Lateral { left, right } => {
            is_fit_for_for_loop_join(left, global_input_types, entry_types)
                && is_fit_for_for_loop_join(right, global_input_types, entry_types)
        }
        QueryExpression::LeftJoin {
            left,
            right,
            expression,
            ..
        } => {
            if !is_fit_for_for_loop_join(left, global_input_types, entry_types) {
                return false;
            }

            // It is not ok to transform into for loop join if right binds a variable also bound by the entry part of the for loop join
            let mut left_types = infer_query_expression_types(left, global_input_types.clone());
            // Keep the global input types for expressions such as BIND(?input AS ?output).
            // Only check variables used by right: other global input bindings pass through unchanged.
            let right_types = infer_query_expression_types(right, global_input_types.clone());
            let mut has_conflicting_variable = false;
            right.lookup_used_variables(&mut |variable| {
                has_conflicting_variable |= right_types.get(variable) != VariableType::UNDEF
                    && left_types.get(variable).undef
                    && entry_types.get(variable) != VariableType::UNDEF;
            });
            if has_conflicting_variable {
                return false;
            }

            // We don't forget the final expression
            left_types.intersect_with(right_types);
            is_expression_fit_for_for_loop_join(expression, &left_types, entry_types)
        }
        QueryExpression::Union { inner } => inner
            .iter()
            .all(|i| is_fit_for_for_loop_join(i, global_input_types, entry_types)),
        QueryExpression::Filter { inner, expression } => {
            is_fit_for_for_loop_join(inner, global_input_types, entry_types)
                && is_expression_fit_for_for_loop_join(
                    expression,
                    &infer_query_expression_types(inner, global_input_types.clone()),
                    entry_types,
                )
        }
        QueryExpression::Extend {
            inner,
            expression,
            variable,
        } => {
            is_fit_for_for_loop_join(inner, global_input_types, entry_types)
                && entry_types.get(variable) == VariableType::UNDEF
                && is_expression_fit_for_for_loop_join(
                    expression,
                    &infer_query_expression_types(inner, global_input_types.clone()),
                    entry_types,
                )
        }
        QueryExpression::Graph { inner, graph_name } => {
            is_fit_for_for_loop_join(inner, global_input_types, entry_types)
                && if let NamedNodePattern::Variable(variable) = graph_name {
                    entry_types.get(variable) == VariableType::UNDEF
                } else {
                    true
                }
        }
        QueryExpression::Project { inner, variables } => {
            // Only the projected variables are given to the sub-query
            is_sub_query_fit_for_for_loop_join(
                inner,
                &global_input_types.restricted_to(variables),
                &entry_types.restricted_to(variables),
            )
        }
        QueryExpression::Join { .. }
        | QueryExpression::Minus { .. }
        | QueryExpression::Service { .. }
        | QueryExpression::OrderBy { .. }
        | QueryExpression::Distinct { .. }
        | QueryExpression::Reduced { .. }
        | QueryExpression::Slice { .. }
        | QueryExpression::Group { .. } => false,
    }
}

/// Same as [`is_fit_for_for_loop_join`] for the inside of a sub-query projection, where aggregations are allowed
fn is_sub_query_fit_for_for_loop_join(
    query_expression: &QueryExpression,
    global_input_types: &VariableTypes,
    entry_types: &VariableTypes,
) -> bool {
    match query_expression {
        QueryExpression::Extend {
            inner,
            expression,
            variable,
        } => {
            is_sub_query_fit_for_for_loop_join(inner, global_input_types, entry_types)
                && entry_types.get(variable) == VariableType::UNDEF
                && is_expression_fit_for_for_loop_join(
                    expression,
                    &infer_query_expression_types(inner, global_input_types.clone()),
                    entry_types,
                )
        }
        QueryExpression::Filter { inner, expression } => {
            is_sub_query_fit_for_for_loop_join(inner, global_input_types, entry_types)
                && is_expression_fit_for_for_loop_join(
                    expression,
                    &infer_query_expression_types(inner, global_input_types.clone()),
                    entry_types,
                )
        }
        QueryExpression::Group {
            inner,
            variables,
            aggregates,
        } => {
            if aggregates
                .iter()
                .any(|(v, _)| entry_types.get(v) != VariableType::UNDEF)
            {
                return false;
            }
            // The given variables must be group keys always bound by the inner pattern.
            // This way the given values select whole groups and the other groups are not changed.
            let inner_types = infer_query_expression_types(inner, global_input_types.clone());
            if entry_types.iter().any(|(v, t)| {
                *t != VariableType::UNDEF && (!variables.contains(v) || inner_types.get(v).undef)
            }) {
                return false;
            }
            is_fit_for_for_loop_join(
                inner,
                &global_input_types.restricted_to(variables),
                &entry_types.restricted_to(variables),
            )
        }
        _ => is_fit_for_for_loop_join(query_expression, global_input_types, entry_types),
    }
}

fn is_path_fit_for_for_loop_join(
    subject: &TermPattern,
    path: &PropertyPathExpression,
    object: &TermPattern,
    entry_types: &VariableTypes,
) -> bool {
    // We don't want to set the left or right side of a zero-length path from
    // the input because it could be returned even if it is not a node in the
    // active graph.
    if let (TermPattern::Variable(subject), TermPattern::Variable(object)) = (subject, object) {
        can_path_match_zero_length(path)
            && entry_types.get(subject) == VariableType::UNDEF
            && entry_types.get(object) == VariableType::UNDEF
    } else {
        true
    }
}

fn can_path_match_zero_length(path: &PropertyPathExpression) -> bool {
    match path {
        PropertyPathExpression::Link(_) | PropertyPathExpression::Nps(_) => false,
        PropertyPathExpression::Inv(path) | PropertyPathExpression::OneOrMorePath(path) => {
            can_path_match_zero_length(path)
        }
        PropertyPathExpression::Seq(left, right) => {
            can_path_match_zero_length(left) && can_path_match_zero_length(right)
        }
        PropertyPathExpression::Alt(left, right) => {
            can_path_match_zero_length(left) || can_path_match_zero_length(right)
        }
        PropertyPathExpression::ZeroOrMorePath(_) | PropertyPathExpression::ZeroOrOnePath(_) => {
            true
        }
    }
}

fn are_all_expression_variables_bound(
    expression: &Expression,
    variable_types: &VariableTypes,
) -> bool {
    expression
        .used_variables()
        .into_iter()
        .all(|v| !variable_types.get(v).undef)
}

fn are_no_expression_variables_bound(
    expression: &Expression,
    variable_types: &VariableTypes,
) -> bool {
    expression
        .used_variables()
        .into_iter()
        .all(|v| variable_types.get(v) == VariableType::UNDEF)
}

fn is_expression_fit_for_for_loop_join(
    expression: &Expression,
    input_types: &VariableTypes,
    entry_types: &VariableTypes,
) -> bool {
    match expression {
        Expression::NamedNode(_) | Expression::Literal(_) => true,
        Expression::Variable(v) | Expression::Bound(v) => {
            !input_types.get(v).undef || entry_types.get(v) == VariableType::UNDEF
        }
        Expression::Or(inner)
        | Expression::And(inner)
        | Expression::Coalesce(inner)
        | Expression::FunctionCall(_, inner) => inner
            .iter()
            .all(|e| is_expression_fit_for_for_loop_join(e, input_types, entry_types)),
        Expression::If(a, b, c) => {
            is_expression_fit_for_for_loop_join(a, input_types, entry_types)
                && is_expression_fit_for_for_loop_join(b, input_types, entry_types)
                && is_expression_fit_for_for_loop_join(c, input_types, entry_types)
        }
        Expression::Exists(inner) => is_fit_for_for_loop_join(inner, input_types, entry_types),
    }
}

fn has_common_variables(
    left: &VariableTypes,
    right: &VariableTypes,
    input_types: &VariableTypes,
) -> bool {
    // TODO: we should be smart and count as shared variables FILTER(?a = ?b)
    left.iter().any(|(variable, left_type)| {
        !left_type.undef && !right.get(variable).undef && input_types.get(variable).undef
    })
}

fn join_key_variables(
    left: &VariableTypes,
    right: &VariableTypes,
    input_types: &VariableTypes,
) -> Vec<Variable> {
    left.iter()
        .filter(|(variable, left_type)| {
            !left_type.undef && !right.get(variable).undef && input_types.get(variable).undef
        })
        .map(|(variable, _)| variable.clone())
        .collect()
}

/// Checks that the given variables have the same value in the join of `side` with `other` as in `side` alone
///
/// It is the case if they are always bound in `side` or never bound in `other`.
fn are_variables_unchanged_by_join<'a>(
    variables: impl IntoIterator<Item = &'a Variable>,
    side_types: &VariableTypes,
    other_types: &VariableTypes,
) -> bool {
    variables
        .into_iter()
        .all(|v| !side_types.get(v).undef || other_types.get(v) == VariableType::UNDEF)
}

/// Checks that evaluating the filter on the rows of a join side gives the same result as on the join output
fn can_filter_be_pushed(
    filter: &Expression,
    side_types: &VariableTypes,
    other_side_types: &VariableTypes,
) -> bool {
    are_variables_unchanged_by_join(filter.used_variables(), side_types, other_side_types)
}

fn is_variable_used(query_expression: &QueryExpression, variable: &Variable) -> bool {
    let mut found = false;
    query_expression.lookup_used_variables(&mut |v| {
        if v == variable {
            found = true;
        }
    });
    found
}

/// Checks that the expression returns the same value each time it is evaluated on the same solution
fn is_deterministic(expression: &Expression) -> bool {
    match expression {
        Expression::NamedNode(_)
        | Expression::Literal(_)
        | Expression::Variable(_)
        | Expression::Bound(_)
        | Expression::Exists(_) => true,
        Expression::Or(inner) | Expression::And(inner) | Expression::Coalesce(inner) => {
            inner.iter().all(is_deterministic)
        }
        Expression::FunctionCall(name, args) => {
            ![sparql::RAND, sparql::UUID, sparql::STRUUID, sparql::BNODE].contains(name)
                && args.iter().all(is_deterministic)
        }
        Expression::If(a, b, c) => {
            is_deterministic(a) && is_deterministic(b) && is_deterministic(c)
        }
    }
}

/// Returns `(?v, [a, b...])` if the expression is `sameTerm(?v, a) || sameTerm(?v, b) || ...` with at least two IRIs
fn iri_disjunction(expression: &Expression) -> Option<(Variable, Vec<NamedNode>)> {
    let Expression::Or(disjuncts) = expression else {
        return None;
    };
    let mut variable = None::<&Variable>;
    let mut iris = Vec::with_capacity(disjuncts.len());
    for disjunct in disjuncts {
        let Expression::FunctionCall(name, args) = disjunct else {
            return None;
        };
        if *name != sparql::SAME_TERM {
            return None;
        }
        let ([Expression::Variable(v), Expression::NamedNode(iri)]
        | [Expression::NamedNode(iri), Expression::Variable(v)]) = args.as_slice()
        else {
            return None;
        };
        if variable.is_some_and(|variable| variable != v) {
            return None;
        }
        variable = Some(v);
        if !iris.contains(iri) {
            iris.push(iri.clone());
        }
    }
    if iris.len() < 2 {
        return None;
    }
    Some((variable?.clone(), iris))
}

fn wrap_in_possible_graph(
    expression: QueryExpression,
    graph_name: Option<NamedNodePattern>,
) -> QueryExpression {
    if let Some(graph_name) = graph_name {
        QueryExpression::graph(expression, graph_name)
    } else {
        expression
    }
}

fn does_contain_exists(expression: &Expression) -> bool {
    match expression {
        Expression::Exists(_) => true,
        Expression::NamedNode(_)
        | Expression::Literal(_)
        | Expression::Variable(_)
        | Expression::Bound(_) => false,
        Expression::Or(e)
        | Expression::And(e)
        | Expression::Coalesce(e)
        | Expression::FunctionCall(_, e) => e.iter().any(does_contain_exists),
        Expression::If(a, b, c) => {
            does_contain_exists(a) || does_contain_exists(b) || does_contain_exists(c)
        }
    }
}
