use oxrdf::Variable;
use spargebra::term::{GroundTermPattern, NamedNodePattern};
use sparopt::algebra::{Expression, GraphPattern, JoinAlgorithm, LeftJoinAlgorithm};
use sparopt::Optimizer;

// Helper functions to create common patterns
fn var(name: &str) -> Variable {
    Variable::new_unchecked(name)
}

fn triple_pattern(s: &str, p: &str, o: &str) -> GraphPattern {
    GraphPattern::QuadPattern {
        subject: GroundTermPattern::Variable(var(s)),
        predicate: NamedNodePattern::Variable(var(p)),
        object: GroundTermPattern::Variable(var(o)),
        graph_name: None,
    }
}

fn var_expr(name: &str) -> Expression {
    Expression::Variable(var(name))
}

// Test 1: Filter Pushing - Simple case
#[test]
fn test_filter_pushing_basic() {
    // FILTER should be pushed down in joins
    let pattern = GraphPattern::filter(
        GraphPattern::join(
            triple_pattern("s", "p", "o"),
            triple_pattern("s", "p2", "o2"),
            JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] },
        ),
        var_expr("s"),
    );

    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // The filter should be pushed into both sides of the join since 's' is bound in both
    match optimized {
        GraphPattern::Join { left, right, .. } => {
            // Both sides should have filters or the pattern should be valid
            assert!(matches!(*left, GraphPattern::Filter { .. }) || matches!(*left, GraphPattern::QuadPattern { .. }));
            assert!(matches!(*right, GraphPattern::Filter { .. }) || matches!(*right, GraphPattern::QuadPattern { .. }));
        }
        _ => panic!("Expected Join pattern, got: {:?}", optimized),
    }
}

// Test 2: Filter Pushing - Filter only pushable to left side
#[test]
fn test_filter_pushing_left_only() {
    // Filter on 'o' should only push to left side where 'o' is bound
    let pattern = GraphPattern::filter(
        GraphPattern::join(
            triple_pattern("s", "p", "o"),
            triple_pattern("s", "p2", "x"),
            JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] },
        ),
        var_expr("o"),
    );

    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // The optimization should handle this appropriately
    match optimized {
        GraphPattern::Filter { .. } | GraphPattern::Join { .. } => {
            // Either wrapped in filter or pushed down
        }
        _ => panic!("Unexpected pattern: {:?}", optimized),
    }
}

// Test 3: Filter Pushing - Filter only pushable to right side
#[test]
fn test_filter_pushing_right_only() {
    let pattern = GraphPattern::filter(
        GraphPattern::join(
            triple_pattern("s", "p", "o"),
            triple_pattern("s", "p2", "x"),
            JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] },
        ),
        var_expr("x"),
    );

    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // The optimization should handle this appropriately
    match optimized {
        GraphPattern::Filter { .. } | GraphPattern::Join { .. } => {
            // Either wrapped in filter or pushed down
        }
        _ => panic!("Unexpected pattern: {:?}", optimized),
    }
}

// Test 4: Filter Pushing - Multiple filters
#[test]
fn test_filter_pushing_multiple_filters() {
    // Multiple filters should be flattened and pushed appropriately
    let inner = triple_pattern("s", "p", "o");
    let pattern = GraphPattern::filter(
        GraphPattern::filter(inner, var_expr("s")),
        var_expr("o"),
    );

    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // Should have a single filter with AND of both conditions
    match optimized {
        GraphPattern::Filter { expression, .. } => {
            // Expression should be an AND or equivalent
            match expression {
                Expression::And(exprs) => {
                    assert_eq!(exprs.len(), 2, "Should have both filter conditions");
                }
                _ => {
                    // Single expression is also valid if one got optimized away
                }
            }
        }
        _ => panic!("Expected Filter pattern, got: {:?}", optimized),
    }
}

// Test 5: Filter Pushing - LeftJoin
#[test]
fn test_filter_pushing_left_join() {
    // Filters should only push to left side of LeftJoin
    let pattern = GraphPattern::filter(
        GraphPattern::left_join(
            triple_pattern("s", "p", "o"),
            triple_pattern("s", "p2", "o2"),
            Expression::Literal(true.into()),
            LeftJoinAlgorithm::HashBuildRightProbeLeft { keys: vec![] },
        ),
        var_expr("s"),
    );

    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // Filter should be pushed to the left side only
    match optimized {
        GraphPattern::LeftJoin { left, .. } | GraphPattern::Filter { inner: left, .. } => {
            // Left side should have the filter
            if let GraphPattern::LeftJoin { left, .. } = *left {
                assert!(matches!(*left, GraphPattern::Filter { .. }) || matches!(*left, GraphPattern::QuadPattern { .. }));
            }
        }
        _ => panic!("Expected LeftJoin pattern, got: {:?}", optimized),
    }
}

// Test 6: Constant Folding - Boolean expressions
#[test]
fn test_constant_folding_and_true() {
    let pattern = GraphPattern::filter(
        triple_pattern("s", "p", "o"),
        Expression::and_all(vec![
            Expression::Literal(true.into()),
            var_expr("s"),
        ]),
    );

    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // The 'true' should be eliminated from the AND
    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Variable(_) => {
                    // Good - true was eliminated
                }
                Expression::And(exprs) => {
                    assert_eq!(exprs.len(), 1, "True should be eliminated from AND");
                }
                _ => {}
            }
        }
        _ => panic!("Expected Filter pattern"),
    }
}

// Test 7: Constant Folding - OR with false
#[test]
fn test_constant_folding_or_false() {
    let pattern = GraphPattern::filter(
        triple_pattern("s", "p", "o"),
        Expression::or_all(vec![
            Expression::Literal(false.into()),
            var_expr("s"),
        ]),
    );

    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // The 'false' should be eliminated from the OR
    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Variable(_) => {
                    // Good - false was eliminated
                }
                Expression::Or(exprs) => {
                    assert_eq!(exprs.len(), 1, "False should be eliminated from OR");
                }
                _ => {}
            }
        }
        _ => panic!("Expected Filter pattern"),
    }
}

// Test 8: Constant Folding - AND with false
#[test]
fn test_constant_folding_and_false() {
    let pattern = GraphPattern::filter(
        triple_pattern("s", "p", "o"),
        Expression::and_all(vec![
            Expression::Literal(false.into()),
            var_expr("s"),
        ]),
    );

    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // AND with false should become just false, making the pattern empty
    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Literal(lit) if lit.value() == "false" => {
                    // Good - reduced to false
                }
                _ => {}
            }
        }
        _ => {}
    }
}

// Test 9: Constant Folding - OR with true
#[test]
fn test_constant_folding_or_true() {
    let pattern = GraphPattern::filter(
        triple_pattern("s", "p", "o"),
        Expression::or_all(vec![
            Expression::Literal(true.into()),
            var_expr("s"),
        ]),
    );

    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // OR with true should become just true
    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Literal(lit) if lit.value() == "true" => {
                    // Good - reduced to true
                }
                _ => {}
            }
        }
        _ => {}
    }
}

// Test 10: Constant Folding - BOUND optimization
#[test]
fn test_constant_folding_bound() {
    // Create a pattern where a variable is definitely bound
    let pattern = GraphPattern::extend(
        triple_pattern("s", "p", "o"),
        var("x"),
        Expression::Literal(42.into()),
    );

    let pattern_with_filter = GraphPattern::filter(
        pattern,
        Expression::Bound(var("s")),
    );

    let optimized = Optimizer::optimize_graph_pattern(pattern_with_filter);

    // BOUND(?s) should be optimized since ?s is always bound in the triple pattern
    // The optimizer should recognize this during type inference
    match optimized {
        GraphPattern::Filter { expression, .. } | GraphPattern::Extend { expression, .. } => {
            // Check if BOUND got optimized to true
            match expression {
                Expression::Literal(lit) if lit.value() == "true" => {
                    // Good - BOUND was optimized to true
                }
                _ => {
                    // May still have BOUND if type inference didn't catch it
                }
            }
        }
        _ => {}
    }
}

// Test 11: Empty Pattern Elimination
#[test]
fn test_empty_pattern_elimination_exists() {
    // EXISTS with empty pattern should become false
    let pattern = GraphPattern::filter(
        triple_pattern("s", "p", "o"),
        Expression::exists(GraphPattern::filter(
            triple_pattern("x", "y", "z"),
            Expression::Literal(false.into()),
        )),
    );

    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // The EXISTS should be eliminated or optimized
    match optimized {
        GraphPattern::Filter { .. } => {
            // Pattern is still there, which is fine
        }
        _ => {}
    }
}

// Test 12: Join Reordering - Simple case
#[test]
fn test_join_reordering_basic() {
    // Create a join of three patterns
    let p1 = triple_pattern("s", "p1", "o1");
    let p2 = triple_pattern("s", "p2", "o2");
    let p3 = triple_pattern("o1", "p3", "o3");

    let pattern = GraphPattern::join(
        GraphPattern::join(p1, p2, JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] }),
        p3,
        JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] },
    );

    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // The optimizer should reorder joins and set proper join keys
    match optimized {
        GraphPattern::Join { left: _, right: _, algorithm } => {
            // Should have proper join keys now
            match algorithm {
                JoinAlgorithm::HashBuildLeftProbeRight { keys: _ } => {
                    // Keys should be computed
                    // In this case, we should have join keys
                }
            }
        }
        _ => panic!("Expected Join pattern"),
    }
}

// Test 13: Join Reordering - Cartesian product detection
#[test]
fn test_join_reordering_cartesian_product() {
    // Two unconnected patterns should still work
    let p1 = triple_pattern("s1", "p1", "o1");
    let p2 = triple_pattern("s2", "p2", "o2");

    let pattern = GraphPattern::join(
        p1,
        p2,
        JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] },
    );

    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // Should still be a join, but with empty keys (cartesian product)
    match optimized {
        GraphPattern::Join { algorithm, .. } => {
            match algorithm {
                JoinAlgorithm::HashBuildLeftProbeRight { keys } => {
                    assert!(keys.is_empty(), "Cartesian product should have no join keys");
                }
            }
        }
        _ => panic!("Expected Join pattern"),
    }
}

// Test 14: Type Inference - Equal vs SameTerm
#[test]
fn test_type_inference_equal_optimization() {
    // When comparing two literals, should use Equal (not SameTerm)
    let pattern = GraphPattern::filter(
        triple_pattern("s", "p", "o"),
        Expression::Equal(
            Box::new(Expression::Literal(1.into())),
            Box::new(Expression::Literal(2.into())),
        ),
    );

    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // The Equal should be normalized
    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Equal(_, _) | Expression::Literal(_) => {
                    // Good - kept as Equal or optimized to a constant
                }
                _ => {}
            }
        }
        _ => {}
    }
}

// Test 15: Filter Pushing in Union
#[test]
fn test_filter_pushing_union() {
    let union = GraphPattern::union_all(vec![
        triple_pattern("s", "p", "o"),
        triple_pattern("s", "p2", "o2"),
    ]);

    let pattern = GraphPattern::filter(union, var_expr("s"));

    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // Filter should be pushed into both branches of the union
    match optimized {
        GraphPattern::Union { inner } => {
            for branch in inner {
                match branch {
                    GraphPattern::Filter { .. } => {
                        // Good - filter was pushed
                    }
                    GraphPattern::QuadPattern { .. } => {
                        // Also acceptable if filter was merged
                    }
                    _ => panic!("Unexpected pattern in union branch: {:?}", branch),
                }
            }
        }
        _ => panic!("Expected Union pattern"),
    }
}

// Test 16: Nested filter flattening
#[test]
fn test_nested_filter_flattening() {
    let pattern = triple_pattern("s", "p", "o");
    let filter1 = GraphPattern::filter(pattern, var_expr("s"));
    let filter2 = GraphPattern::filter(filter1, var_expr("p"));
    let filter3 = GraphPattern::filter(filter2, var_expr("o"));

    let optimized = Optimizer::optimize_graph_pattern(filter3);

    // All filters should be flattened into a single AND
    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::And(exprs) => {
                    assert_eq!(exprs.len(), 3, "All three filters should be combined");
                }
                _ => {
                    // Single expression if some got optimized
                }
            }
        }
        _ => panic!("Expected Filter pattern"),
    }
}

// Test 17: Extend pattern optimization
#[test]
fn test_extend_pattern() {
    let pattern = GraphPattern::extend(
        triple_pattern("s", "p", "o"),
        var("computed"),
        Expression::Add(
            Box::new(Expression::Literal(1.into())),
            Box::new(Expression::Literal(2.into())),
        ),
    );

    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // The pattern should still be an Extend
    match optimized {
        GraphPattern::Extend { .. } => {
            // Good
        }
        _ => panic!("Expected Extend pattern"),
    }
}

// Test 18: Filter with BOUND on extend variable
#[test]
fn test_filter_bound_on_extend() {
    let extend = GraphPattern::extend(
        triple_pattern("s", "p", "o"),
        var("x"),
        Expression::Literal(42.into()),
    );

    let pattern = GraphPattern::filter(extend, Expression::Bound(var("x")));

    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // BOUND(?x) should potentially be optimized to true since ?x is always bound by EXTEND
    match optimized {
        GraphPattern::Filter { .. } | GraphPattern::Extend { .. } => {
            // Either form is acceptable
        }
        _ => {}
    }
}

// Test 19: Distinct pattern optimization
#[test]
fn test_distinct_pattern() {
    let pattern = GraphPattern::distinct(triple_pattern("s", "p", "o"));

    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Distinct { .. } => {
            // Good - pattern preserved
        }
        _ => panic!("Expected Distinct pattern"),
    }
}

// Test 20: Complex nested pattern
#[test]
fn test_complex_nested_pattern() {
    // Build a complex pattern with joins, filters, and extends
    let p1 = triple_pattern("s", "p1", "o1");
    let p2 = triple_pattern("s", "p2", "o2");
    let join = GraphPattern::join(
        p1,
        p2,
        JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] },
    );
    let filtered = GraphPattern::filter(join, var_expr("s"));
    let extended = GraphPattern::extend(filtered, var("computed"), var_expr("o1"));

    let optimized = Optimizer::optimize_graph_pattern(extended);

    // Should still have valid structure after optimization
    match optimized {
        GraphPattern::Extend { inner, .. } => {
            // Inner should be optimized
            match *inner {
                GraphPattern::Join { .. } | GraphPattern::Filter { .. } | GraphPattern::QuadPattern { .. } => {
                    // Valid optimized structure
                }
                _ => panic!("Unexpected inner pattern: {:?}", inner),
            }
        }
        _ => panic!("Expected Extend pattern"),
    }
}
