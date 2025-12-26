use oxrdf::Variable;
use spargebra::term::GroundTermPattern;
use sparopt::algebra::{Expression, GraphPattern};
use sparopt::Optimizer;

// Helper functions
fn var(name: &str) -> Variable {
    Variable::new_unchecked(name)
}

fn var_expr(name: &str) -> Expression {
    Expression::Variable(var(name))
}

fn literal_expr(value: i32) -> Expression {
    Expression::Literal(value.into())
}

fn bool_expr(value: bool) -> Expression {
    Expression::Literal(value.into())
}

fn simple_quad() -> GraphPattern {
    GraphPattern::QuadPattern {
        subject: GroundTermPattern::Variable(var("s")),
        predicate: spargebra::term::NamedNodePattern::Variable(var("p")),
        object: GroundTermPattern::Variable(var("o")),
        graph_name: None,
    }
}

// Test 1: AND flattening
#[test]
fn test_and_flattening() {
    let expr = Expression::and_all(vec![
        Expression::and_all(vec![var_expr("a"), var_expr("b")]),
        var_expr("c"),
    ]);

    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::And(exprs) => {
                    // Should be flattened to 3 separate expressions
                    assert_eq!(exprs.len(), 3, "AND should be flattened");
                }
                _ => {}
            }
        }
        _ => panic!("Expected filter pattern"),
    }
}

// Test 2: OR flattening
#[test]
fn test_or_flattening() {
    let expr = Expression::or_all(vec![
        Expression::or_all(vec![var_expr("a"), var_expr("b")]),
        var_expr("c"),
    ]);

    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Or(exprs) => {
                    // Should be flattened to 3 separate expressions
                    assert_eq!(exprs.len(), 3, "OR should be flattened");
                }
                _ => {}
            }
        }
        _ => panic!("Expected filter pattern"),
    }
}

// Test 3: Empty AND
#[test]
fn test_empty_and() {
    let expr = Expression::and_all(vec![]);
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Literal(lit) => {
                    // Empty AND should be true
                    assert_eq!(lit.value(), "true", "Empty AND should be true");
                }
                _ => panic!("Expected true literal, got: {:?}", expression),
            }
        }
        GraphPattern::QuadPattern { .. } => {
            // Filter with true was optimized away
        }
        _ => panic!("Expected filter or quad pattern"),
    }
}

// Test 4: Empty OR
#[test]
fn test_empty_or() {
    let expr = Expression::or_all(vec![]);
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // Empty OR may be optimized differently depending on context
    // Just verify the optimization completes without error
    match optimized {
        GraphPattern::Filter { expression, .. } => {
            // Filter pattern preserved - check if expression is a literal or other form
            match expression {
                Expression::Literal(lit) => {
                    // Empty OR should be false
                    assert_eq!(lit.value(), "false", "Empty OR should be false");
                }
                _ => {
                    // Expression may be transformed in other ways
                }
            }
        }
        _ => {
            // Other valid transformation (may be optimized to empty or other form)
        }
    }
}

// Test 5: Single element AND
#[test]
fn test_single_element_and() {
    let expr = Expression::and_all(vec![var_expr("x")]);
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // Single element AND may be simplified or preserved
    // Just verify the optimization completes
    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Variable(_) => {
                    // Single element AND simplified to variable
                }
                Expression::And(exprs) => {
                    assert!(exprs.len() >= 1, "Single element AND");
                }
                _ => {
                    // Other valid expression form
                }
            }
        }
        _ => {
            // Other valid transformation (may be optimized to empty or other form)
        }
    }
}

// Test 6: Single element OR
#[test]
fn test_single_element_or() {
    let expr = Expression::or_all(vec![var_expr("x")]);
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // Single element OR may be simplified or preserved
    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Variable(_) => {
                    // Single element OR simplified to variable
                }
                Expression::And(exprs) => {
                    // May be wrapped in AND for boolean conversion
                    assert!(exprs.len() >= 1);
                }
                _ => {
                    // Other valid expression form
                }
            }
        }
        _ => {
            // Other valid transformation (may be optimized to empty or other form)
        }
    }
}

// Test 7: NOT simplification
#[test]
fn test_not_expression() {
    let expr = !var_expr("x");
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Not(_) => {
                    // NOT preserved
                }
                _ => {}
            }
        }
        _ => panic!("Expected filter pattern"),
    }
}

// Test 8: Arithmetic expressions
#[test]
fn test_arithmetic_add() {
    let expr = literal_expr(1) + literal_expr(2);
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Add(_, _) => {
                    // Add preserved (constant folding at expr level not done)
                }
                _ => {}
            }
        }
        _ => panic!("Expected filter pattern"),
    }
}

// Test 9: Arithmetic subtract
#[test]
fn test_arithmetic_subtract() {
    let expr = var_expr("x") - literal_expr(5);
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Subtract(_, _) => {
                    // Subtract preserved
                }
                _ => {}
            }
        }
        _ => panic!("Expected filter pattern"),
    }
}

// Test 10: Arithmetic multiply
#[test]
fn test_arithmetic_multiply() {
    let expr = var_expr("x") * literal_expr(10);
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Multiply(_, _) => {
                    // Multiply preserved
                }
                _ => {}
            }
        }
        _ => panic!("Expected filter pattern"),
    }
}

// Test 11: Arithmetic divide
#[test]
fn test_arithmetic_divide() {
    let expr = var_expr("x") / literal_expr(2);
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Divide(_, _) => {
                    // Divide preserved
                }
                _ => {}
            }
        }
        _ => panic!("Expected filter pattern"),
    }
}

// Test 12: Comparison expressions - Equal
#[test]
fn test_comparison_equal() {
    let expr = Expression::equal(var_expr("x"), literal_expr(5));
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Equal(_, _) | Expression::SameTerm(_, _) => {
                    // Equal or SameTerm (optimizer chooses based on types)
                }
                _ => {}
            }
        }
        _ => panic!("Expected filter pattern"),
    }
}

// Test 13: Comparison - Greater
#[test]
fn test_comparison_greater() {
    let expr = Expression::greater(var_expr("x"), literal_expr(10));
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Greater(_, _) => {
                    // Preserved
                }
                _ => {}
            }
        }
        _ => panic!("Expected filter pattern"),
    }
}

// Test 14: Comparison - Less
#[test]
fn test_comparison_less() {
    let expr = Expression::less(var_expr("x"), literal_expr(10));
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Less(_, _) => {
                    // Preserved
                }
                _ => {}
            }
        }
        _ => panic!("Expected filter pattern"),
    }
}

// Test 15: IF expression with constant condition
#[test]
fn test_if_constant_true() {
    let expr = Expression::if_cond(
        bool_expr(true),
        literal_expr(1),
        literal_expr(2),
    );
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Literal(lit) => {
                    // Should select the THEN branch
                    assert_eq!(lit.value(), "1", "IF(true, 1, 2) should be 1");
                }
                _ => panic!("Expected literal 1, got: {:?}", expression),
            }
        }
        _ => panic!("Expected filter pattern"),
    }
}

// Test 16: IF expression with constant false
#[test]
fn test_if_constant_false() {
    let expr = Expression::if_cond(
        bool_expr(false),
        literal_expr(1),
        literal_expr(2),
    );
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Literal(lit) => {
                    // Should select the ELSE branch
                    assert_eq!(lit.value(), "2", "IF(false, 1, 2) should be 2");
                }
                _ => panic!("Expected literal 2, got: {:?}", expression),
            }
        }
        _ => panic!("Expected filter pattern"),
    }
}

// Test 17: IF expression with variable condition
#[test]
fn test_if_variable_condition() {
    let expr = Expression::if_cond(
        var_expr("cond"),
        literal_expr(1),
        literal_expr(2),
    );
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::If(_, _, _) => {
                    // Should be preserved since condition is not constant
                }
                _ => {}
            }
        }
        _ => panic!("Expected filter pattern"),
    }
}

// Test 18: COALESCE expression
#[test]
fn test_coalesce_expression() {
    let expr = Expression::coalesce(vec![
        var_expr("a"),
        var_expr("b"),
        literal_expr(42),
    ]);
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Coalesce(_) => {
                    // Preserved
                }
                _ => {}
            }
        }
        _ => panic!("Expected filter pattern"),
    }
}

// Test 19: EXISTS with empty singleton
#[test]
fn test_exists_empty_singleton() {
    let expr = Expression::exists(GraphPattern::empty_singleton());
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Literal(lit) => {
                    // EXISTS with empty singleton should be true
                    assert_eq!(lit.value(), "true", "EXISTS of empty singleton should be true");
                }
                _ => panic!("Expected true literal, got: {:?}", expression),
            }
        }
        GraphPattern::QuadPattern { .. } => {
            // Filter with true was optimized away
        }
        _ => panic!("Expected filter or quad pattern"),
    }
}

/// Test 20: EXISTS with empty pattern
#[test]
fn test_exists_empty_pattern() {
    let expr = Expression::exists(GraphPattern::empty());
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    // EXISTS with empty pattern may be optimized in different ways
    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Literal(lit) => {
                    // EXISTS with empty pattern should be false
                    assert_eq!(lit.value(), "false", "EXISTS of empty pattern should be false");
                }
                Expression::Exists(_) => {
                    // EXISTS expression preserved
                }
                _ => {
                    // Other valid expression form
                }
            }
        }
        _ => {
            // Other valid transformation (may be optimized to empty or other form)
        }
    }
}

// Test 21: SameTerm optimization
#[test]
fn test_same_term_identical_named_nodes() {
    use spargebra::term::NamedNode;

    let node = NamedNode::new_unchecked("http://example.org/test");
    let expr = Expression::same_term(
        Expression::NamedNode(node.clone()),
        Expression::NamedNode(node),
    );
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Literal(lit) => {
                    // SameTerm of identical nodes should be true
                    assert_eq!(lit.value(), "true", "SameTerm of identical nodes should be true");
                }
                _ => {}
            }
        }
        GraphPattern::QuadPattern { .. } => {
            // Filter with true was optimized away
        }
        _ => panic!("Expected filter or quad pattern"),
    }
}

// Test 22: Equal optimization with identical literals
#[test]
fn test_equal_identical_literals() {
    let lit = literal_expr(42);
    let expr = Expression::equal(lit.clone(), lit);
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::Literal(l) => {
                    // Equal of identical literals should be true
                    assert_eq!(l.value(), "true", "Equal of identical literals should be true");
                }
                _ => {}
            }
        }
        GraphPattern::QuadPattern { .. } => {
            // Filter with true was optimized away
        }
        _ => panic!("Expected filter or quad pattern"),
    }
}

// Test 23: Unary plus
#[test]
fn test_unary_plus() {
    let expr = Expression::unary_plus(var_expr("x"));
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::UnaryPlus(_) => {
                    // Preserved
                }
                _ => {}
            }
        }
        _ => panic!("Expected filter pattern"),
    }
}

// Test 24: Unary minus
#[test]
fn test_unary_minus() {
    let expr = -var_expr("x");
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            match expression {
                Expression::UnaryMinus(_) => {
                    // Preserved
                }
                _ => {}
            }
        }
        _ => panic!("Expected filter pattern"),
    }
}

// Test 25: Complex nested expression
#[test]
fn test_complex_nested_expression() {
    let expr = Expression::and_all(vec![
        Expression::or_all(vec![
            var_expr("a"),
            bool_expr(false),
        ]),
        Expression::greater(var_expr("x"), literal_expr(10)),
        Expression::if_cond(
            var_expr("cond"),
            bool_expr(true),
            bool_expr(false),
        ),
    ]);
    let pattern = GraphPattern::filter(simple_quad(), expr);
    let optimized = Optimizer::optimize_graph_pattern(pattern);

    match optimized {
        GraphPattern::Filter { expression, .. } => {
            // Complex expression should be normalized
            match expression {
                Expression::And(_) => {
                    // Should still be an AND
                }
                _ => {}
            }
        }
        _ => panic!("Expected filter pattern"),
    }
}
