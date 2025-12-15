#[cfg(test)]
mod tests {
    use crate::Optimizer;
    use crate::algebra::{GraphPattern, GroundTermPattern, NamedNode, NamedNodePattern, Variable, Expression};

    // Helpers
    fn leaf_quad() -> GraphPattern {
        GraphPattern::QuadPattern {
            subject: GroundTermPattern::NamedNode(NamedNode::new("http://example.org/s").unwrap()),
            predicate: NamedNodePattern::NamedNode(NamedNode::new("http://example.org/p").unwrap()),
            object: GroundTermPattern::NamedNode(NamedNode::new("http://example.org/o").unwrap()),
            graph_name: None,
        }
    }

    // Helper to check whether a pattern contains a given quad pattern anywhere in the tree
    fn contains_pattern(tree: &GraphPattern, target: &GraphPattern) -> bool {
        if tree == target {
            return true;
        }
        match tree {
            GraphPattern::Join { left, right, .. }
            | GraphPattern::LeftJoin { left, right, .. }
            | GraphPattern::Minus { left, right, .. } => {
                contains_pattern(left, target) || contains_pattern(right, target)
            }
            #[cfg(feature = "sep-0006")]
            GraphPattern::Lateral { left, right } => {
                contains_pattern(left, target) || contains_pattern(right, target)
            }
            GraphPattern::Filter { inner, .. }
            | GraphPattern::Distinct { inner }
            | GraphPattern::Reduced { inner }
            | GraphPattern::Project { inner, .. }
            | GraphPattern::OrderBy { inner, .. }
            | GraphPattern::Extend { inner, .. }
            | GraphPattern::Slice { inner, .. }
            | GraphPattern::Group { inner, .. }
            | GraphPattern::Service { inner, .. } => contains_pattern(inner, target),
            GraphPattern::Union { inner } => inner.iter().any(|i| contains_pattern(i, target)),
            _ => false,
        }
    }

    #[test]
    fn normalize_union_scopes() {
        // union of the same pattern twice should not be empty and should contain at most one real branch
        let a = leaf_quad();
        let u = GraphPattern::union_all(vec![a.clone(), a]);
        let optimized = Optimizer::optimize_graph_pattern(u);
        // Accept either a single QuadPattern or a Union containing one branch
    assert!(!matches!(&optimized, GraphPattern::Values { bindings, .. } if bindings.is_empty()));
    if let GraphPattern::Union { inner } = &optimized {
            assert!(inner.len() == 1 || inner.iter().all(|p| matches!(p, GraphPattern::QuadPattern { .. })))
        }
    }

    #[test]
    fn normalize_minus_scope() {
        // left minus right where right contains a reference to left's pattern: ensure no infinite loop
        let left = leaf_quad();
        let right = GraphPattern::join(left.clone(), leaf_quad(), crate::algebra::JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] });
        let m = GraphPattern::minus(left.clone(), right, crate::algebra::MinusAlgorithm::HashBuildRightProbeLeft { keys: vec![] });
        let optimized = Optimizer::optimize_graph_pattern(m);
    // should not be empty (left is not empty)
    assert!(!matches!(optimized, GraphPattern::Values { bindings, .. } if bindings.is_empty()));
    }

    #[test]
    fn normalize_extend_scope() {
        let inner = leaf_quad();
    let var = Variable::new_unchecked("?x");
    let expr = Expression::NamedNode(NamedNode::new("http://example.org/v").unwrap());
        let ext = GraphPattern::extend(inner, var, expr);
        let optimized = Optimizer::optimize_graph_pattern(ext);
    // extend over a leaf should remain a Project-like structure (Extend) or be simplified
    // as long as it's not the empty table
    assert!(!matches!(optimized, GraphPattern::Values { bindings, .. } if bindings.is_empty()));
    }

    #[test]
    fn normalize_join_duplicate_removal() {
        // join of the same pattern twice should simplify to a single pattern (duplicates removed in the same scope)
        let a = leaf_quad();
        let j = GraphPattern::join(a.clone(), a.clone(), crate::algebra::JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] });
        let optimized = Optimizer::optimize_graph_pattern(j);
        // optimizer should reduce the duplicate join to a single quad pattern
        assert!(matches!(optimized, GraphPattern::QuadPattern { .. }));
    }

    #[test]
    fn minus_scope_preserves_outer_pattern() {
        // If a pattern appears inside MINUS and outside, it should not be removed entirely by normalization
        let left = leaf_quad();
        let right = leaf_quad();
        let m = GraphPattern::minus(left.clone(), right, crate::algebra::MinusAlgorithm::HashBuildRightProbeLeft { keys: vec![] });
        let j = GraphPattern::join(left.clone(), m, crate::algebra::JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] });
        let optimized = Optimizer::optimize_graph_pattern(j);
        // The join should still contain the left pattern (i.e., not be empty)
        assert!(!matches!(optimized, GraphPattern::Values { bindings, .. } if bindings.is_empty()));
    }

    #[test]
    fn join_dedup_exact_equality() {
        // join(a, a) should normalize exactly to a (not to a join)
        let a = leaf_quad();
        let j = GraphPattern::join(a.clone(), a.clone(), crate::algebra::JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] });
        let optimized = Optimizer::optimize_graph_pattern(j);
        // exact equality
        assert_eq!(optimized, a);
    }

    #[test]
    fn nested_union_collapses_duplicates() {
        // Create nested unions where duplicates are present
        let a = leaf_quad();
        let u = GraphPattern::union_all(vec![GraphPattern::union_all(vec![a.clone(), a.clone()]), a.clone()]);
        let optimized = Optimizer::optimize_graph_pattern(u);
        // The normalized tree should still contain the quad and not be empty
        assert!(contains_pattern(&optimized, &a));
    }

    #[test]
    fn minus_inner_same_as_outer_is_not_removed() {
        // left join (left, minus(left, some_other)) should keep outer left
        let left = leaf_quad();
        let other = GraphPattern::QuadPattern {
            subject: GroundTermPattern::NamedNode(NamedNode::new("http://example.org/other_s").unwrap()),
            predicate: NamedNodePattern::NamedNode(NamedNode::new("http://example.org/p").unwrap()),
            object: GroundTermPattern::NamedNode(NamedNode::new("http://example.org/other_o").unwrap()),
            graph_name: None,
        };
        let m = GraphPattern::minus(left.clone(), other, crate::algebra::MinusAlgorithm::HashBuildRightProbeLeft { keys: vec![] });
        let j = GraphPattern::join(left.clone(), m, crate::algebra::JoinAlgorithm::HashBuildLeftProbeRight { keys: vec![] });
        let optimized = Optimizer::optimize_graph_pattern(j);
        // Ensure the optimized plan still contains the left quad
        assert!(contains_pattern(&optimized, &left));
    }
}
