#![cfg(test)]

use spargebra::{Query, SparqlParser};

fn parse(expression: &str) -> Query {
    SparqlParser::new()
        .parse_query(&format!("SELECT ({expression} AS ?v) {{}}"))
        .unwrap()
}

fn assert_same_tree(expression: &str, bracketed: &str) {
    assert_eq!(
        parse(expression),
        parse(bracketed),
        "{expression} should parse as {bracketed}"
    );
}

#[test]
fn test_multiplicative_chain_is_left_associative() {
    assert_same_tree("7 / 20 * 1000", "(7 / 20) * 1000");
    assert_same_tree("7/20*1000", "(7/20)*1000");
    assert_same_tree("2 * 3 / 4", "(2 * 3) / 4");
    assert_same_tree("12 / 2 / 3", "(12 / 2) / 3");
    assert_same_tree(
        "6 / 20 * 0.8 / 1.0 * 100",
        "((((6 / 20) * 0.8) / 1.0) * 100)",
    );
}

#[test]
fn test_additive_chain_is_left_associative() {
    assert_same_tree("8 - 3 - 2", "(8 - 3) - 2");
    assert_same_tree("1 - 2 + 3", "(1 - 2) + 3");
    assert_same_tree("?a + ?b - ?c + ?d", "((?a + ?b) - ?c) + ?d");
}

#[test]
fn test_multiplication_binds_tighter_than_addition() {
    assert_same_tree("1 + 2 * 3 - 4 / 5", "(1 + (2 * 3)) - (4 / 5)");
    assert_same_tree("?a * ?b - ?c * ?d", "(?a * ?b) - (?c * ?d)");
}

#[test]
fn test_signed_literal_takes_following_multiplicative_operands() {
    // AdditiveExpression ::= MultiplicativeExpression ( '+' MultiplicativeExpression | '-' MultiplicativeExpression | ( NumericLiteralPositive | NumericLiteralNegative ) ( ( '*' UnaryExpression ) | ( '/' UnaryExpression ) )* )*
    assert_same_tree("1 -2*3", "1 + (-2*3)");
    assert_same_tree("?a -2*3", "?a + (-2*3)");
    assert_same_tree("?a +2*3", "?a + (+2*3)");
    assert_same_tree("1 -2*3 -4", "(1 + (-2*3)) + -4");
    assert_same_tree("?a -2/4*2", "?a + ((-2/4)*2)");
    assert_same_tree("?a -2 * -3", "?a + (-2 * -3)");
    assert_same_tree("?a -2*-?b", "?a + (-2 * -?b)");
    assert_same_tree("?a -2*!?b", "?a + (-2 * !?b)");
    assert_same_tree("?a * ?b -2*?c", "(?a * ?b) + (-2*?c)");
    assert_same_tree("?x = ?a -2*3", "?x = (?a + (-2*3))");
    assert_same_tree("?a -2", "?a + -2");
    assert_same_tree("8-3-2", "(8 + -3) + -2");
}

#[test]
fn test_signed_literal_after_infix_operator_is_a_new_step() {
    // A signed literal that follows `a + b` or `a - b` starts a new additive step.
    // It must not be absorbed into the right operand `b`.
    assert_same_tree("10 - 2 -3", "(10 - 2) + -3");
    assert_same_tree("10 - 2-3", "(10 - 2) + -3");
    assert_same_tree("10 - 2 +3", "(10 - 2) + +3");
    assert_same_tree("10 - 2 -3*2", "(10 - 2) + (-3*2)");
    assert_same_tree("1 - 1 -1 -1", "((1 - 1) + -1) + -1");
    assert_same_tree("?a - ?b -1", "(?a - ?b) + -1");
    assert_same_tree("?a + ?b -2*3", "(?a + ?b) + (-2*3)");
    assert_same_tree("?a - ?b * ?c -1/2", "(?a - (?b * ?c)) + (-1/2)");
    assert_same_tree("?a -1 - ?b", "(?a + -1) - ?b");
}

#[test]
fn test_unspaced_operator_before_digits_is_a_signed_literal_step() {
    assert_same_tree("?a+2*3", "?a + (+2*3)");
    assert_same_tree("?x+1*2", "?x + (+1*2)");
    assert_same_tree("10-4/2", "10 + (-4/2)");
    assert_same_tree("?a-?b-1", "(?a - ?b) + -1");
}
