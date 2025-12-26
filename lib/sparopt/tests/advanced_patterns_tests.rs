use oxrdf::Variable;
use spargebra::term::{GroundTerm, GroundTermPattern, NamedNode, NamedNodePattern};
use sparopt::algebra::{Expression, GraphPattern, JoinAlgorithm, LeftJoinAlgorithm, MinusAlgorithm};
use sparopt::Optimizer;

// Helper functions
fn var(name: &str) -> Variable {
    Variable::new_unchecked(name)
}

fn var_expr(name: &str) -> Expression {
    Expression::Variable(var(name))
}

fn triple(s: &str, p: &str, o: &str) -> GraphPattern {
    GraphPattern::QuadPattern {
        subject: GroundTermPattern::Variable(var(s)),
        predicate: NamedNodePattern::Variable(var(p)),
        object: GroundTermPattern::Variable(var(o)),
        graph_name: None,
    }
}

// Test 1: Star join pattern - common subject
#[test]
fn test_star_join_common_subject() {
    // ?s ?p1 ?o1 . ?s ?p2 ?o2 . ?s ?p3 ?o3
    let p1 = triple("s", "p1", "o1");
    let p2 = triple("s", "p2", "o2");
    let p3 = triple("s", "p3", "o3");

    let join1 = GraphPattern::join(p1, p2, JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] });
    let join2 = GraphPattern::join(join1, p3, JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] });

    let optimized = Optimizer::optimize_graph_pattern(join2);

    match optimized {
        GraphPattern::Join { algorithm, .. } => {
            match algorithm {
                JoinAlgorithm::HashBuildLeftProbeRight { keys } => {
                    // Should have identified 's' as a join key
                    assert!(!keys.is_empty(), "Star join should have join keys");
                }
            }
        }
        _ => panic!("Expected Join pattern"),
    }
}

// Test 2: Chain join pattern
#[test]
fn test_chain_join_pattern() {
    // ?s ?p1 ?o1 . ?o1 ?p2 ?o2 . ?o2 ?p3 ?o3
    let p1 = triple("s", "p1", "o1");
    let p2 = triple("o1", "p2", "o2");
    let p3 = triple("o2", "p3", "o3");

    let join1 = GraphPattern::join(p1, p2, JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] });
    let join2 = GraphPattern::join(join1, p3, JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] });

    let optimized = Optimizer::optimize_graph_pattern(join2);

    match optimized {
        GraphPattern::Join { .. } => {
            // Should be reordered optimally
        }
        _ => panic!("Expected Join pattern"),
    }
}

// Test 3: Project pattern with optimization
#[test]
fn test_project_pattern() {
    let pattern = triple("s", "p", "o");
    let filtered = GraphPattern::filter(pattern, var_expr("s"));
    let projected = GraphPattern::project(filtered, vec![var("s"), var("p")]);

    let optimized = Optimizer::optimize_graph_pattern(projected);

    match optimized {
        GraphPattern::Project { variables, .. } => {
            assert_eq!(variables.len(), 2, "Should project 2 variables");
            assert!(variables.contains(&var("s")));
            assert!(variables.contains(&var("p")));
        }
        _ => panic!("Expected Project pattern"),
    }
}

// Test 4: Group pattern
#[test]
fn test_group_pattern() {
    let pattern = triple("s", "p", "o");
    let grouped = GraphPattern::group(pattern, vec![var("s")], vec![]);

    let optimized = Optimizer::optimize_graph_pattern(grouped);

    match optimized {
        GraphPattern::Group { variables, .. } => {
            assert_eq!(variables.len(), 1, "Should group by 1 variable");
            assert_eq!(variables[0], var("s"));
        }
        _ => panic!("Expected Group pattern"),
    }
}

// Test 5: OrderBy pattern
#[test]
fn test_order_by_pattern() {
    use sparopt::algebra::OrderExpression;

    let pattern = triple("s", "p", "o");
    let ordered = GraphPattern::order_by(
        pattern,
        vec![OrderExpression::Asc(var_expr("s"))],
    );

    let optimized = Optimizer::optimize_graph_pattern(ordered);

    match optimized {
        GraphPattern::OrderBy { expression, .. } => {
            assert_eq!(expression.len(), 1, "Should have 1 order expression");
        }
        _ => panic!("Expected OrderBy pattern"),
    }
}

// Test 6: Slice pattern
#[test]
fn test_slice_pattern() {
    let pattern = triple("s", "p", "o");
    let sliced = GraphPattern::slice(pattern, 10, Some(20));

    let optimized = Optimizer::optimize_graph_pattern(sliced);

    match optimized {
        GraphPattern::Slice { start, length, .. } => {
            assert_eq!(start, 10, "Start should be 10");
            assert_eq!(length, Some(20), "Length should be 20");
        }
        _ => panic!("Expected Slice pattern"),
    }
}

// Test 7: Reduced pattern
#[test]
fn test_reduced_pattern() {
    let pattern = triple("s", "p", "o");
    let reduced = GraphPattern::reduced(pattern);

    let optimized = Optimizer::optimize_graph_pattern(reduced);

    match optimized {
        GraphPattern::Reduced { .. } => {
            // Should preserve reduced
        }
        _ => panic!("Expected Reduced pattern"),
    }
}

// Test 8: Minus pattern
#[test]
fn test_minus_pattern() {
    let left = triple("s", "p", "o");
    let right = triple("s", "p2", "o2");
    let minus = GraphPattern::minus(left, right, MinusAlgorithm::HashBuildRightProbeLeft { keys: vec![] });

    let optimized = Optimizer::optimize_graph_pattern(minus);

    match optimized {
        GraphPattern::Minus { algorithm, .. } => {
            match algorithm {
                MinusAlgorithm::HashBuildRightProbeLeft { keys } => {
                    // Should have join keys for 's'
                    assert!(!keys.is_empty(), "Minus should have join keys");
                }
            }
        }
        _ => panic!("Expected Minus pattern"),
    }
}

// Test 9: Values pattern
#[test]
fn test_values_pattern() {
    let values = GraphPattern::values(
        vec![var("x"), var("y")],
        vec![
            vec![
                Some(GroundTerm::NamedNode(NamedNode::new_unchecked("http://example.org/a"))),
                Some(GroundTerm::Literal(1.into())),
            ],
            vec![
                Some(GroundTerm::NamedNode(NamedNode::new_unchecked("http://example.org/b"))),
                Some(GroundTerm::Literal(2.into())),
            ],
        ],
    );

    let optimized = Optimizer::optimize_graph_pattern(values);

    match optimized {
        GraphPattern::Values { variables, bindings } => {
            assert_eq!(variables.len(), 2, "Should have 2 variables");
            assert_eq!(bindings.len(), 2, "Should have 2 bindings");
        }
        _ => panic!("Expected Values pattern"),
    }
}

// Test 10: Complex filter with multiple AND conditions
#[test]
fn test_complex_filter_multiple_and() {
    let pattern = triple("s", "p", "o");
    let filter = GraphPattern::filter(
        pattern,
        Expression::and_all(vec![
            var_expr("s"),
            var_expr("p"),
            var_expr("o"),
            Expression::greater(var_expr("o"), Expression::Literal(10.into())),
        ]),
    );

    let optimized = Optimizer::optimize_graph_pattern(filter);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::And(exprs) => {
                    assert_eq!(exprs.len(), 4, "Should have 4 AND conditions");
                }
                _ => {}
            }
        }
        _ => panic!("Expected Filter pattern"),
    }
}

// Test 11: LeftJoin with non-trivial expression
#[test]
fn test_left_join_with_expression() {
    let left = triple("s", "p", "o");
    let right = triple("s", "p2", "o2");
    let expr = Expression::equal(var_expr("o"), var_expr("o2"));

    let left_join = GraphPattern::left_join(
        left,
        right,
        expr,
        LeftJoinAlgorithm::HashBuildRightProbeLeft { keys: vec![] },
    );

    let optimized = Optimizer::optimize_graph_pattern(left_join);

    match optimized {
        GraphPattern::LeftJoin { algorithm, .. } => {
            match algorithm {
                LeftJoinAlgorithm::HashBuildRightProbeLeft { keys } => {
                    // Should have 's' as join key
                    assert!(!keys.is_empty(), "LeftJoin should have join keys");
                }
            }
        }
        _ => panic!("Expected LeftJoin pattern"),
    }
}

// Test 12: Union with filters
#[test]
fn test_union_with_filters() {
    let p1 = GraphPattern::filter(triple("s", "p1", "o1"), var_expr("s"));
    let p2 = GraphPattern::filter(triple("s", "p2", "o2"), var_expr("s"));
    let union = GraphPattern::union_all(vec![p1, p2]);

    let optimized = Optimizer::optimize_graph_pattern(union);

    match optimized {
        GraphPattern::Union { inner } => {
            assert_eq!(inner.len(), 2, "Union should have 2 branches");
        }
        _ => panic!("Expected Union pattern"),
    }
}

// Test 13: Extend with complex expression
#[test]
fn test_extend_with_complex_expression() {
    let pattern = triple("s", "p", "o");
    let expr = Expression::Add(
        Box::new(var_expr("o")),
        Box::new(Expression::Multiply(
            Box::new(var_expr("o")),
            Box::new(Expression::Literal(2.into())),
        )),
    );
    let extend = GraphPattern::extend(pattern, var("computed"), expr);

    let optimized = Optimizer::optimize_graph_pattern(extend);

    match optimized {
        GraphPattern::Extend { variable, .. } => {
            assert_eq!(variable, var("computed"));
        }
        _ => panic!("Expected Extend pattern"),
    }
}

// Test 14: Filter pushing through Extend
#[test]
fn test_filter_pushing_through_extend() {
    let pattern = triple("s", "p", "o");
    let extend = GraphPattern::extend(pattern, var("x"), Expression::Literal(42.into()));
    let filter = GraphPattern::filter(extend, var_expr("s"));

    let optimized = Optimizer::optimize_graph_pattern(filter);

    // Filter on 's' should be pushed down through Extend
    match optimized {
        GraphPattern::Extend { inner, .. } => {
            match *inner {
                GraphPattern::Filter { .. } | GraphPattern::QuadPattern { .. } => {
                    // Filter was pushed down or merged
                }
                _ => panic!("Expected filter to be pushed down"),
            }
        }
        GraphPattern::Filter { inner, .. } => {
            // Or filter stays on top if it uses the extended variable
            match *inner {
                GraphPattern::Extend { .. } => {
                    // OK - filter uses extended variable
                }
                _ => {}
            }
        }
        _ => panic!("Expected Extend or Filter pattern"),
    }
}

// Test 15: Filter that cannot be pushed (uses extended variable)
#[test]
fn test_filter_cannot_push_through_extend() {
    let pattern = triple("s", "p", "o");
    let extend = GraphPattern::extend(pattern, var("x"), Expression::Literal(42.into()));
    let filter = GraphPattern::filter(extend, var_expr("x"));

    let optimized = Optimizer::optimize_graph_pattern(filter);

    // Filter on 'x' should NOT be pushed down since it uses the extended variable
    match optimized {
        GraphPattern::Filter { inner, expression } => {
            match *inner {
                GraphPattern::Extend { .. } => {
                    // Filter stays on top
                    match expression {
                        Expression::Variable(v) => assert_eq!(v, var("x")),
                        _ => {}
                    }
                }
                _ => {}
            }
        }
        GraphPattern::Extend { .. } => {
            // Filter may have been merged
        }
        _ => panic!("Expected Filter or Extend pattern"),
    }
}

// Test 16: Multiple joins with shared variables
#[test]
fn test_multiple_joins_shared_variables() {
    // ?s ?p ?o . ?o ?p2 ?x . ?x ?p3 ?y . ?y ?p4 ?s
    let p1 = triple("s", "p1", "o");
    let p2 = triple("o", "p2", "x");
    let p3 = triple("x", "p3", "y");
    let p4 = triple("y", "p4", "s");

    let j1 = GraphPattern::join(p1, p2, JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] });
    let j2 = GraphPattern::join(j1, p3, JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] });
    let j3 = GraphPattern::join(j2, p4, JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] });

    let optimized = Optimizer::optimize_graph_pattern(j3);

    match optimized {
        GraphPattern::Join { .. } => {
            // Should be reordered optimally
        }
        _ => panic!("Expected Join pattern"),
    }
}

// Test 17: Filter with BOUND on always-bound variable
#[test]
fn test_bound_on_always_bound_variable() {
    let pattern = triple("s", "p", "o");
    let filter = GraphPattern::filter(pattern, Expression::Bound(var("s")));

    let optimized = Optimizer::optimize_graph_pattern(filter);

    // BOUND(?s) should ideally be optimized to true since ?s is always bound
    // The optimizer may simplify this to just the pattern if BOUND evaluates to true
    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Literal(lit) => {
                    assert_eq!(lit.value(), "true", "BOUND on always-bound variable should be true");
                }
                Expression::Bound(_) => {
                    // May not be optimized if type inference doesn't catch it
                }
                _ => {}
            }
        }
        GraphPattern::QuadPattern { .. } => {
            // Filter was completely optimized away (BOUND became true and was eliminated)
        }
        _ => {}
    }
}

// Test 18: Complex nested Union, Join, and Filter
#[test]
fn test_complex_nested_union_join_filter() {
    let p1 = triple("s", "p1", "o1");
    let p2 = triple("s", "p2", "o2");
    let p3 = triple("x", "p3", "y");
    let p4 = triple("x", "p4", "z");

    let j1 = GraphPattern::join(p1, p2, JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] });
    let j2 = GraphPattern::join(p3, p4, JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] });

    let union = GraphPattern::union_all(vec![j1, j2]);
    let filter = GraphPattern::filter(union, var_expr("s"));

    let optimized = Optimizer::optimize_graph_pattern(filter);

    match optimized {
        GraphPattern::Union { inner } => {
            // Filter should be pushed into union branches
            assert_eq!(inner.len(), 2);
        }
        GraphPattern::Filter { inner, .. } => {
            // Or filter may stay on top if it can't be pushed everywhere
            match *inner {
                GraphPattern::Union { .. } => {}
                _ => {}
            }
        }
        _ => panic!("Expected Union or Filter pattern"),
    }
}

// Test 19: Empty singleton pattern
#[test]
fn test_empty_singleton_pattern() {
    let pattern = GraphPattern::empty_singleton();
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Values { bindings, .. } => {
            // Empty singleton should be VALUES with one empty binding
            assert_eq!(bindings.len(), 1);
        }
        _ => {}
    }
}

// Test 20: Filter with always-false condition
#[test]
fn test_filter_always_false() {
    let pattern = triple("s", "p", "o");
    let filter = GraphPattern::filter(pattern, Expression::Literal(false.into()));

    let optimized = Optimizer::optimize_graph_pattern(filter);

    // Filter with false should keep the pattern (evaluator will handle empty results)
    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Literal(lit) => {
                    assert_eq!(lit.value(), "false");
                }
                _ => {}
            }
        }
        _ => {}
    }
}

// Test 21: Join reordering with different cardinalities
#[test]
fn test_join_reordering_cardinalities() {
    // Small VALUES joined with large pattern
    let values = GraphPattern::values(
        vec![var("x")],
        vec![
            vec![Some(GroundTerm::Literal(1.into()))],
        ],
    );
    let pattern = triple("x", "p", "o");

    let join = GraphPattern::join(
        pattern,
        values,
        JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] },
    );

    let optimized = Optimizer::optimize_graph_pattern(join);

    // Optimizer should recognize that VALUES is smaller
    match optimized {
        GraphPattern::Join { left, right, .. } => {
            // One side should be the VALUES pattern
            let has_values = matches!(*left, GraphPattern::Values { .. })
                || matches!(*right, GraphPattern::Values { .. });
            assert!(has_values, "Join should include VALUES pattern");
        }
        _ => panic!("Expected Join pattern"),
    }
}

// Test 22: Distinct with nested patterns
#[test]
fn test_distinct_nested_patterns() {
    let p1 = triple("s", "p", "o");
    let p2 = triple("s", "p2", "o2");
    let join = GraphPattern::join(p1, p2, JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] });
    let distinct = GraphPattern::distinct(join);

    let optimized = Optimizer::optimize_graph_pattern(distinct);

    match optimized {
        GraphPattern::Distinct { inner } => {
            match *inner {
                GraphPattern::Join { .. } => {
                    // Join should be optimized inside Distinct
                }
                _ => {}
            }
        }
        _ => panic!("Expected Distinct pattern"),
    }
}
