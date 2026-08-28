use crate::algebra::{
    Expression, JoinAlgorithm, LeftJoinAlgorithm, MinusAlgorithm, QueryExpression,
};
use crate::type_inference::{
    VariableType, VariableTypes, infer_expression_type, infer_query_expression_types,
};
use oxrdf::Variable;
use oxrdf::vocab::rdf;
use spargebra::algebra::PropertyPathExpression;
use spargebra::term::{GroundTermPattern, NamedNodePattern};
use spargebra::vocab::sparql;
use std::cmp::{max, min};

pub struct Optimizer;

impl Optimizer {
    pub fn optimize_query_expression(query_expression: QueryExpression) -> QueryExpression {
        let input_types = VariableTypes::default();
        let query_expression = Self::normalize_pattern(query_expression, &input_types);
        let query_expression = Self::push_graph(query_expression, None, &input_types);
        let query_expression = Self::reorder_joins(query_expression, &input_types);
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
                    let push_left = are_all_expression_variables_bound(&filter, &left_types);
                    let push_right = are_all_expression_variables_bound(&filter, &right_types);
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
                let mut left_filters = Vec::new();
                let mut right_filters = Vec::new();
                for filter in filters {
                    let push_left = are_all_expression_variables_bound(&filter, &left_types);
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
                    let push_left = are_all_expression_variables_bound(&filter, &left_types);
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
            QueryExpression::Project { inner, variables } => QueryExpression::project(
                Self::push_filters(*inner, filters, input_types),
                variables,
            ),
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

    fn reorder_joins(
        query_expression: QueryExpression,
        input_types: &VariableTypes,
    ) -> QueryExpression {
        match query_expression {
            QueryExpression::QuadPattern { .. }
            | QueryExpression::Path { .. }
            | QueryExpression::Values { .. } => query_expression,
            QueryExpression::Join { left, right, .. } => {
                // We flatten the join operation
                let mut to_reorder = Vec::new();
                let mut todo = vec![*right, *left];
                while let Some(e) = todo.pop() {
                    if let QueryExpression::Join { left, right, .. } = e {
                        todo.push(*right);
                        todo.push(*left);
                    } else {
                        to_reorder.push(Self::reorder_joins(e, input_types));
                    }
                }

                // We do first type inference
                let to_reorder_types = to_reorder
                    .iter()
                    .map(|p| infer_query_expression_types(p, input_types.clone()))
                    .collect::<Vec<_>>();

                // We do greedy join reordering
                let mut output_cartesian_product_joins = Vec::new();
                let mut not_yet_reordered_ids = vec![true; to_reorder.len()];
                // We look for the next connected component to reorder and pick the smallest element
                while let Some(next_entry_id) = not_yet_reordered_ids
                    .iter()
                    .enumerate()
                    .filter(|(_, v)| **v)
                    .map(|(i, _)| i)
                    .min_by_key(|i| estimate_query_expression_size(&to_reorder[*i], input_types))
                {
                    not_yet_reordered_ids[next_entry_id] = false; // It's now done
                    let mut output = to_reorder[next_entry_id].clone();
                    let mut output_types = to_reorder_types[next_entry_id].clone();
                    // We look for an other child to join with that does not blow up the join cost
                    while let Some(next_id) = not_yet_reordered_ids
                        .iter()
                        .enumerate()
                        .filter(|(_, v)| **v)
                        .map(|(i, _)| i)
                        .filter(|i| {
                            has_common_variables(&output_types, &to_reorder_types[*i], input_types)
                        })
                        .min_by_key(|i| {
                            // Estimation of the join cost
                            if cfg!(feature = "sep-0006")
                                && is_fit_for_for_loop_join(
                                    &to_reorder[*i],
                                    input_types,
                                    &output_types,
                                )
                            {
                                estimate_lateral_cost(
                                    &output,
                                    &output_types,
                                    &to_reorder[*i],
                                    input_types,
                                )
                            } else {
                                estimate_join_cost(
                                    &output,
                                    &to_reorder[*i],
                                    &JoinAlgorithm::HashBuildLeftProbeRight {
                                        keys: join_key_variables(
                                            &output_types,
                                            &to_reorder_types[*i],
                                            input_types,
                                        ),
                                    },
                                    input_types,
                                )
                            }
                        })
                    {
                        not_yet_reordered_ids[next_id] = false; // It's now done
                        let next = to_reorder[next_id].clone();
                        #[cfg(feature = "sep-0006")]
                        {
                            output = if is_fit_for_for_loop_join(&next, input_types, &output_types)
                            {
                                QueryExpression::lateral(output, next)
                            } else {
                                QueryExpression::join(
                                    output,
                                    next,
                                    JoinAlgorithm::HashBuildLeftProbeRight {
                                        keys: join_key_variables(
                                            &output_types,
                                            &to_reorder_types[next_id],
                                            input_types,
                                        ),
                                    },
                                )
                            };
                        }
                        #[cfg(not(feature = "sep-0006"))]
                        {
                            output = QueryExpression::join(
                                output,
                                next,
                                JoinAlgorithm::HashBuildLeftProbeRight {
                                    keys: join_key_variables(
                                        &output_types,
                                        &to_reorder_types[next_id],
                                        input_types,
                                    ),
                                },
                            );
                        }
                        output_types.intersect_with(to_reorder_types[next_id].clone());
                    }
                    output_cartesian_product_joins.push(output);
                }
                output_cartesian_product_joins
                    .into_iter()
                    .reduce(|left, right| {
                        let keys = join_key_variables(
                            &infer_query_expression_types(&left, input_types.clone()),
                            &infer_query_expression_types(&right, input_types.clone()),
                            input_types,
                        );
                        if estimate_query_expression_size(&left, input_types)
                            <= estimate_query_expression_size(&right, input_types)
                        {
                            QueryExpression::join(
                                left,
                                right,
                                JoinAlgorithm::HashBuildLeftProbeRight { keys },
                            )
                        } else {
                            QueryExpression::join(
                                right,
                                left,
                                JoinAlgorithm::HashBuildLeftProbeRight { keys },
                            )
                        }
                    })
                    .unwrap()
            }
            #[cfg(feature = "sep-0006")]
            QueryExpression::Lateral { left, right } => {
                let left_types = infer_query_expression_types(&left, input_types.clone());
                QueryExpression::lateral(
                    Self::reorder_joins(*left, input_types),
                    Self::reorder_joins(*right, &left_types),
                )
            }
            QueryExpression::LeftJoin {
                left,
                right,
                expression,
                ..
            } => {
                let left = Self::reorder_joins(*left, input_types);
                let left_types = infer_query_expression_types(&left, input_types.clone());
                #[cfg(feature = "sep-0006")]
                {
                    let initial_right_types =
                        infer_query_expression_types(&right, input_types.clone());
                    if has_common_variables(&left_types, &initial_right_types, input_types) {
                        let lateral_cost = estimate_query_expression_size(&left, input_types)
                            .saturating_mul(estimate_query_expression_size(&right, &left_types));
                        let keys =
                            join_key_variables(&left_types, &initial_right_types, input_types);
                        let join_cost = estimate_query_expression_size(&left, input_types)
                            .saturating_mul(estimate_query_expression_size(&right, input_types))
                            .saturating_div(
                                1_000_u64.saturating_pow(keys.len().try_into().unwrap()),
                            );

                        if lateral_cost <= join_cost.saturating_mul(100) {
                            let right_for_lateral =
                                Self::reorder_joins((*right).clone(), &left_types);
                            if is_fit_for_for_loop_join(
                                &right_for_lateral,
                                input_types,
                                &left_types,
                            ) {
                                return QueryExpression::lateral(
                                    left,
                                    QueryExpression::left_join(
                                        QueryExpression::empty_singleton(),
                                        right_for_lateral,
                                        expression,
                                        LeftJoinAlgorithm::HashBuildRightProbeLeft {
                                            keys: Vec::new(),
                                        },
                                    ),
                                );
                            }
                        }
                    }
                }
                let right = Self::reorder_joins(*right, input_types);
                let right_types = infer_query_expression_types(&right, input_types.clone());
                QueryExpression::left_join(
                    left,
                    right,
                    expression,
                    LeftJoinAlgorithm::HashBuildRightProbeLeft {
                        keys: join_key_variables(&left_types, &right_types, input_types),
                    },
                )
            }
            QueryExpression::Minus { left, right, .. } => {
                let left = Self::reorder_joins(*left, input_types);
                let left_types = infer_query_expression_types(&left, input_types.clone());
                let right = Self::reorder_joins(*right, input_types);
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
                QueryExpression::graph(Self::reorder_joins(*inner, input_types), graph_name)
            }
            QueryExpression::Extend {
                inner,
                expression,
                variable,
            } => QueryExpression::extend(
                Self::reorder_joins(*inner, input_types),
                variable,
                expression,
            ),
            QueryExpression::Filter { inner, expression } => {
                QueryExpression::filter(Self::reorder_joins(*inner, input_types), expression)
            }
            QueryExpression::Union { inner } => QueryExpression::union_all(
                inner
                    .into_iter()
                    .map(|c| Self::reorder_joins(c, input_types)),
            ),
            QueryExpression::Slice {
                inner,
                offset,
                limit,
            } => QueryExpression::slice(Self::reorder_joins(*inner, input_types), offset, limit),
            QueryExpression::Distinct { inner } => {
                QueryExpression::distinct(Self::reorder_joins(*inner, input_types))
            }
            QueryExpression::Reduced { inner } => {
                QueryExpression::reduced(Self::reorder_joins(*inner, input_types))
            }
            QueryExpression::Project { inner, variables } => {
                QueryExpression::project(Self::reorder_joins(*inner, input_types), variables)
            }
            QueryExpression::OrderBy { inner, expression } => {
                QueryExpression::order_by(Self::reorder_joins(*inner, input_types), expression)
            }
            QueryExpression::Service { .. } => {
                // We don't do join reordering inside of SERVICE calls, we don't know about cardinalities
                query_expression
            }
            QueryExpression::Group {
                inner,
                variables,
                aggregates,
            } => QueryExpression::group(
                Self::reorder_joins(*inner, input_types),
                variables,
                aggregates,
            ),
        }
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
            let right_types = infer_query_expression_types(right, global_input_types.clone());
            if right_types.iter().any(|(variable, t)| {
                *t != VariableType::UNDEF
                    && left_types.get(variable).undef
                    && entry_types.get(variable) != VariableType::UNDEF
            }) {
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
        QueryExpression::Join { .. }
        | QueryExpression::Minus { .. }
        | QueryExpression::Service { .. }
        | QueryExpression::OrderBy { .. }
        | QueryExpression::Distinct { .. }
        | QueryExpression::Reduced { .. }
        | QueryExpression::Slice { .. }
        | QueryExpression::Project { .. }
        | QueryExpression::Group { .. } => false,
    }
}

fn is_path_fit_for_for_loop_join(
    subject: &GroundTermPattern,
    path: &PropertyPathExpression,
    object: &GroundTermPattern,
    entry_types: &VariableTypes,
) -> bool {
    match path {
        PropertyPathExpression::Link(_)
        | PropertyPathExpression::OneOrMorePath(_)
        | PropertyPathExpression::Nps(_) => true,
        PropertyPathExpression::Inv(path) => {
            is_path_fit_for_for_loop_join(object, path, subject, entry_types)
        }
        PropertyPathExpression::Seq(l, r) => {
            let whatever = Variable::new_unchecked("#intermediate#").into();
            is_path_fit_for_for_loop_join(subject, l, &whatever, entry_types)
                || is_path_fit_for_for_loop_join(&whatever, r, subject, entry_types)
        }
        PropertyPathExpression::Alt(l, r) => {
            is_path_fit_for_for_loop_join(subject, l, object, entry_types)
                && is_path_fit_for_for_loop_join(subject, r, object, entry_types)
        }
        PropertyPathExpression::ZeroOrMorePath(_) | PropertyPathExpression::ZeroOrOnePath(_) => {
            // We don't want to set the left or right side of the zero or ... path because it could be returned in the result set even if it is not supported in the graph
            if let (GroundTermPattern::Variable(subject), GroundTermPattern::Variable(object)) =
                (subject, object)
            {
                entry_types.get(subject) == VariableType::UNDEF
                    && entry_types.get(object) == VariableType::UNDEF
            } else {
                true
            }
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

fn estimate_query_expression_size(
    expression: &QueryExpression,
    input_types: &VariableTypes,
) -> u64 {
    match expression {
        QueryExpression::Values { bindings, .. } => bindings.len().try_into().unwrap(),
        QueryExpression::QuadPattern {
            subject,
            predicate,
            object,
            ..
        } => {
            let mut size = estimate_triple_pattern_size(
                is_term_pattern_bound(subject, input_types),
                is_named_node_pattern_bound(predicate, input_types),
                is_term_pattern_bound(object, input_types),
            );
            if let NamedNodePattern::NamedNode(predicate) = predicate {
                if *predicate == rdf::TYPE {
                    size = size.saturating_add(1);
                }
            }
            size
        }
        QueryExpression::Path {
            subject,
            path,
            object,
            ..
        } => estimate_path_size(
            is_term_pattern_bound(subject, input_types),
            path,
            is_term_pattern_bound(object, input_types),
        ),
        QueryExpression::Graph { graph_name, inner } => {
            (if is_named_node_pattern_bound(graph_name, input_types) {
                1_u64
            } else {
                100
            })
            .saturating_mul(estimate_query_expression_size(inner, input_types))
        }
        QueryExpression::Join {
            left,
            right,
            algorithm,
        } => estimate_join_cost(left, right, algorithm, input_types),
        QueryExpression::LeftJoin {
            left,
            right,
            algorithm,
            ..
        } => match algorithm {
            LeftJoinAlgorithm::HashBuildRightProbeLeft { keys } => {
                let left_size = estimate_query_expression_size(left, input_types);
                max(
                    left_size,
                    left_size
                        .saturating_mul(estimate_query_expression_size(
                            right,
                            &infer_query_expression_types(right, input_types.clone()),
                        ))
                        .saturating_div(1_000_u64.saturating_pow(keys.len().try_into().unwrap())),
                )
            }
        },
        #[cfg(feature = "sep-0006")]
        QueryExpression::Lateral { left, right } => estimate_lateral_cost(
            left,
            &infer_query_expression_types(left, input_types.clone()),
            right,
            input_types,
        ),
        QueryExpression::Union { inner } => inner
            .iter()
            .map(|inner| estimate_query_expression_size(inner, input_types))
            .fold(0, u64::saturating_add),
        QueryExpression::Minus { left, .. } => estimate_query_expression_size(left, input_types),
        QueryExpression::Filter { inner, .. }
        | QueryExpression::Extend { inner, .. }
        | QueryExpression::OrderBy { inner, .. }
        | QueryExpression::Project { inner, .. }
        | QueryExpression::Distinct { inner, .. }
        | QueryExpression::Reduced { inner, .. }
        | QueryExpression::Group { inner, .. }
        | QueryExpression::Service { inner, .. } => {
            estimate_query_expression_size(inner, input_types)
        }
        QueryExpression::Slice {
            inner,
            offset,
            limit,
        } => {
            let inner = estimate_query_expression_size(inner, input_types).saturating_sub(*offset);
            if let Some(limit) = limit {
                min(inner, *limit)
            } else {
                inner
            }
        }
    }
}

fn estimate_join_cost(
    left: &QueryExpression,
    right: &QueryExpression,
    algorithm: &JoinAlgorithm,
    input_types: &VariableTypes,
) -> u64 {
    match algorithm {
        JoinAlgorithm::HashBuildLeftProbeRight { keys } => {
            estimate_query_expression_size(left, input_types)
                .saturating_mul(estimate_query_expression_size(right, input_types))
                .saturating_div(1_000_u64.saturating_pow(keys.len().try_into().unwrap()))
        }
    }
}

fn estimate_lateral_cost(
    left: &QueryExpression,
    left_types: &VariableTypes,
    right: &QueryExpression,
    input_types: &VariableTypes,
) -> u64 {
    estimate_query_expression_size(left, input_types)
        .saturating_mul(estimate_query_expression_size(right, left_types))
}

fn estimate_triple_pattern_size(
    subject_bound: bool,
    predicate_bound: bool,
    object_bound: bool,
) -> u64 {
    match (subject_bound, predicate_bound, object_bound) {
        (true, true, true) => 1,
        (true, true, false) => 10,
        (true, false, true) => 2,
        (false, true, true) => 1_000,
        (true, false, false) => 100,
        (false, false, false) => 1_000_000_000,
        (false, true, false) => 1_000_000,
        (false, false, true) => 10_000,
    }
}

fn estimate_path_size(start_bound: bool, path: &PropertyPathExpression, end_bound: bool) -> u64 {
    match path {
        PropertyPathExpression::Link(_) => {
            estimate_triple_pattern_size(start_bound, true, end_bound)
        }
        PropertyPathExpression::Inv(p) => estimate_path_size(end_bound, p, start_bound),
        PropertyPathExpression::Seq(a, b) => {
            // We do a for loop join in the best direction
            min(
                estimate_path_size(start_bound, a, false)
                    .saturating_mul(estimate_path_size(true, b, end_bound)),
                estimate_path_size(start_bound, a, true)
                    .saturating_mul(estimate_path_size(false, b, end_bound)),
            )
        }
        PropertyPathExpression::Alt(a, b) => estimate_path_size(start_bound, a, end_bound)
            .saturating_add(estimate_path_size(start_bound, b, end_bound)),
        PropertyPathExpression::ZeroOrMorePath(p) => {
            if start_bound && end_bound {
                1
            } else if start_bound || end_bound {
                estimate_path_size(start_bound, p, end_bound).saturating_mul(1000)
            } else {
                1_000_000_000
            }
        }
        PropertyPathExpression::OneOrMorePath(p) => {
            if start_bound && end_bound {
                1
            } else {
                estimate_path_size(start_bound, p, end_bound).saturating_mul(1000)
            }
        }
        PropertyPathExpression::ZeroOrOnePath(p) => {
            if start_bound && end_bound {
                1
            } else if start_bound || end_bound {
                estimate_path_size(start_bound, p, end_bound)
            } else {
                1_000_000_000
            }
        }
        PropertyPathExpression::Nps(_) => {
            estimate_triple_pattern_size(start_bound, false, end_bound)
        }
    }
}

fn is_term_pattern_bound(pattern: &GroundTermPattern, input_types: &VariableTypes) -> bool {
    match pattern {
        GroundTermPattern::NamedNode(_) | GroundTermPattern::Literal(_) => true,
        GroundTermPattern::Variable(v) => !input_types.get(v).undef,
        #[cfg(feature = "sparql-12")]
        GroundTermPattern::Triple(t) => {
            is_term_pattern_bound(&t.subject, input_types)
                && is_named_node_pattern_bound(&t.predicate, input_types)
                && is_term_pattern_bound(&t.object, input_types)
        }
    }
}

fn is_named_node_pattern_bound(pattern: &NamedNodePattern, input_types: &VariableTypes) -> bool {
    match pattern {
        NamedNodePattern::NamedNode(_) => true,
        NamedNodePattern::Variable(v) => !input_types.get(v).undef,
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn estimate_slice_size(offset: u64, limit: Option<u64>) -> u64 {
        estimate_query_expression_size(
            &QueryExpression::slice(
                QueryExpression::values(Vec::new(), vec![Vec::new(); 5]),
                offset,
                limit,
            ),
            &VariableTypes::default(),
        )
    }

    #[test]
    fn slice_size_applies_offset_before_limit() {
        assert_eq!(estimate_slice_size(2, Some(1)), 1);
        assert_eq!(estimate_slice_size(6, Some(1)), 0);
        assert_eq!(estimate_slice_size(2, None), 3);
    }
}
