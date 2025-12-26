//! N3 built-in functions for reasoning and logic programming.
//!
//! This module implements core N3 built-in functions as defined in the N3 specification.
//! These functions can be used during N3 reasoning and querying.
//!
//! # Categories
//!
//! - **Math**: Arithmetic operations (sum, difference, product, quotient)
//! - **String**: String manipulation (concatenation, contains, length)
//! - **Log**: Logic operations (equalTo, notEqualTo)
//! - **List**: List operations (first, rest, member)

use oxrdf::{Literal, NamedNode, Term};
use oxsdatatypes::{Decimal, Double, Float, Integer};
use std::sync::Arc;

/// Type alias for a custom N3 builtin function.
pub type N3BuiltinFn = Arc<dyn (Fn(&[Term]) -> Option<Term>) + Send + Sync>;

// N3 namespace URIs
const MATH_NS: &str = "http://www.w3.org/2000/10/swap/math#";
const STRING_NS: &str = "http://www.w3.org/2000/10/swap/string#";
const LOG_NS: &str = "http://www.w3.org/2000/10/swap/log#";
const LIST_NS: &str = "http://www.w3.org/2000/10/swap/list#";

// ============================================================================
// Math Builtins
// ============================================================================

/// Implements math:sum - computes the sum of two numbers.
///
/// # Arguments
/// - args[0]: First number (integer, decimal, float, or double)
/// - args[1]: Second number (integer, decimal, float, or double)
///
/// # Returns
/// The sum of the two numbers, or None if the arguments are invalid.
pub fn math_sum(args: &[Term]) -> Option<Term> {
    if args.len() != 2 {
        return None;
    }

    let (a, b) = (&args[0], &args[1]);
    let Term::Literal(a_lit) = a else { return None };
    let Term::Literal(b_lit) = b else { return None };

    // Try to parse as numeric types
    if let (Ok(a_int), Ok(b_int)) = (
        a_lit.value().parse::<Integer>(),
        b_lit.value().parse::<Integer>(),
    ) {
        return Some(Literal::from(a_int.checked_add(b_int)?).into());
    }

    if let (Ok(a_dec), Ok(b_dec)) = (
        a_lit.value().parse::<Decimal>(),
        b_lit.value().parse::<Decimal>(),
    ) {
        return Some(Literal::from(a_dec.checked_add(b_dec)?).into());
    }

    if let (Ok(a_flt), Ok(b_flt)) = (a_lit.value().parse::<Float>(), b_lit.value().parse::<Float>()) {
        return Some(Literal::from(a_flt + b_flt).into());
    }

    if let (Ok(a_dbl), Ok(b_dbl)) = (
        a_lit.value().parse::<Double>(),
        b_lit.value().parse::<Double>(),
    ) {
        return Some(Literal::from(a_dbl + b_dbl).into());
    }

    // Try mixed numeric types - promote to higher precision
    let a_val = to_double(a_lit)?;
    let b_val = to_double(b_lit)?;
    Some(Literal::from(Double::from(a_val + b_val)).into())
}

/// Implements math:difference - computes the difference of two numbers.
///
/// # Arguments
/// - args[0]: First number (minuend)
/// - args[1]: Second number (subtrahend)
///
/// # Returns
/// The difference (args[0] - args[1]), or None if the arguments are invalid.
pub fn math_difference(args: &[Term]) -> Option<Term> {
    if args.len() != 2 {
        return None;
    }

    let (a, b) = (&args[0], &args[1]);
    let Term::Literal(a_lit) = a else { return None };
    let Term::Literal(b_lit) = b else { return None };

    if let (Ok(a_int), Ok(b_int)) = (
        a_lit.value().parse::<Integer>(),
        b_lit.value().parse::<Integer>(),
    ) {
        return Some(Literal::from(a_int.checked_sub(b_int)?).into());
    }

    if let (Ok(a_dec), Ok(b_dec)) = (
        a_lit.value().parse::<Decimal>(),
        b_lit.value().parse::<Decimal>(),
    ) {
        return Some(Literal::from(a_dec.checked_sub(b_dec)?).into());
    }

    if let (Ok(a_flt), Ok(b_flt)) = (a_lit.value().parse::<Float>(), b_lit.value().parse::<Float>()) {
        return Some(Literal::from(a_flt - b_flt).into());
    }

    if let (Ok(a_dbl), Ok(b_dbl)) = (
        a_lit.value().parse::<Double>(),
        b_lit.value().parse::<Double>(),
    ) {
        return Some(Literal::from(a_dbl - b_dbl).into());
    }

    let a_val = to_double(a_lit)?;
    let b_val = to_double(b_lit)?;
    Some(Literal::from(Double::from(a_val - b_val)).into())
}

/// Implements math:product - computes the product of two numbers.
///
/// # Arguments
/// - args[0]: First number
/// - args[1]: Second number
///
/// # Returns
/// The product of the two numbers, or None if the arguments are invalid.
pub fn math_product(args: &[Term]) -> Option<Term> {
    if args.len() != 2 {
        return None;
    }

    let (a, b) = (&args[0], &args[1]);
    let Term::Literal(a_lit) = a else { return None };
    let Term::Literal(b_lit) = b else { return None };

    if let (Ok(a_int), Ok(b_int)) = (
        a_lit.value().parse::<Integer>(),
        b_lit.value().parse::<Integer>(),
    ) {
        return Some(Literal::from(a_int.checked_mul(b_int)?).into());
    }

    if let (Ok(a_dec), Ok(b_dec)) = (
        a_lit.value().parse::<Decimal>(),
        b_lit.value().parse::<Decimal>(),
    ) {
        return Some(Literal::from(a_dec.checked_mul(b_dec)?).into());
    }

    if let (Ok(a_flt), Ok(b_flt)) = (a_lit.value().parse::<Float>(), b_lit.value().parse::<Float>()) {
        return Some(Literal::from(a_flt * b_flt).into());
    }

    if let (Ok(a_dbl), Ok(b_dbl)) = (
        a_lit.value().parse::<Double>(),
        b_lit.value().parse::<Double>(),
    ) {
        return Some(Literal::from(a_dbl * b_dbl).into());
    }

    let a_val = to_double(a_lit)?;
    let b_val = to_double(b_lit)?;
    Some(Literal::from(Double::from(a_val * b_val)).into())
}

/// Implements math:quotient - computes the quotient of two numbers.
///
/// # Arguments
/// - args[0]: Dividend
/// - args[1]: Divisor
///
/// # Returns
/// The quotient (args[0] / args[1]), or None if the arguments are invalid or division by zero.
pub fn math_quotient(args: &[Term]) -> Option<Term> {
    if args.len() != 2 {
        return None;
    }

    let (a, b) = (&args[0], &args[1]);
    let Term::Literal(a_lit) = a else { return None };
    let Term::Literal(b_lit) = b else { return None };

    if let (Ok(a_int), Ok(b_int)) = (
        a_lit.value().parse::<Integer>(),
        b_lit.value().parse::<Integer>(),
    ) {
        if b_int == Integer::from(0) {
            return None;
        }
        return Some(Literal::from(a_int.checked_div(b_int)?).into());
    }

    if let (Ok(a_dec), Ok(b_dec)) = (
        a_lit.value().parse::<Decimal>(),
        b_lit.value().parse::<Decimal>(),
    ) {
        if b_dec == Decimal::from(0_u8) {
            return None;
        }
        return Some(Literal::from(a_dec.checked_div(b_dec)?).into());
    }

    if let (Ok(a_flt), Ok(b_flt)) = (a_lit.value().parse::<Float>(), b_lit.value().parse::<Float>()) {
        if b_flt == Float::from(0.0) {
            return None;
        }
        return Some(Literal::from(a_flt / b_flt).into());
    }

    if let (Ok(a_dbl), Ok(b_dbl)) = (
        a_lit.value().parse::<Double>(),
        b_lit.value().parse::<Double>(),
    ) {
        if b_dbl == Double::from(0.0) {
            return None;
        }
        return Some(Literal::from(a_dbl / b_dbl).into());
    }

    let a_val = to_double(a_lit)?;
    let b_val = to_double(b_lit)?;
    if b_val == 0.0 {
        return None;
    }
    Some(Literal::from(Double::from(a_val / b_val)).into())
}

// ============================================================================
// String Builtins
// ============================================================================

/// Implements string:concatenation - concatenates two strings.
///
/// # Arguments
/// - args[0]: First string
/// - args[1]: Second string
///
/// # Returns
/// The concatenation of the two strings, or None if the arguments are invalid.
pub fn string_concatenation(args: &[Term]) -> Option<Term> {
    if args.len() != 2 {
        return None;
    }

    let a_str = term_to_string(&args[0])?;
    let b_str = term_to_string(&args[1])?;

    Some(Literal::new_simple_literal(format!("{}{}", a_str, b_str)).into())
}

/// Implements string:contains - checks if a string contains a substring.
///
/// # Arguments
/// - args[0]: The string to search in
/// - args[1]: The substring to search for
///
/// # Returns
/// A boolean literal (true if args[0] contains args[1], false otherwise).
pub fn string_contains(args: &[Term]) -> Option<Term> {
    if args.len() != 2 {
        return None;
    }

    let haystack = term_to_string(&args[0])?;
    let needle = term_to_string(&args[1])?;

    Some(Literal::from(haystack.contains(&*needle)).into())
}

/// Implements string:length - returns the length of a string.
///
/// # Arguments
/// - args[0]: The string to measure
///
/// # Returns
/// An integer literal representing the character count.
pub fn string_length(args: &[Term]) -> Option<Term> {
    if args.len() != 1 {
        return None;
    }

    let s = term_to_string(&args[0])?;
    Some(Literal::from(s.chars().count() as i64).into())
}

// ============================================================================
// Log Builtins
// ============================================================================

/// Implements log:equalTo - checks if two terms are equal.
///
/// # Arguments
/// - args[0]: First term
/// - args[1]: Second term
///
/// # Returns
/// A boolean literal (true if equal, false otherwise).
pub fn log_equal_to(args: &[Term]) -> Option<Term> {
    if args.len() != 2 {
        return None;
    }

    Some(Literal::from(args[0] == args[1]).into())
}

/// Implements log:notEqualTo - checks if two terms are not equal.
///
/// # Arguments
/// - args[0]: First term
/// - args[1]: Second term
///
/// # Returns
/// A boolean literal (true if not equal, false if equal).
pub fn log_not_equal_to(args: &[Term]) -> Option<Term> {
    if args.len() != 2 {
        return None;
    }

    Some(Literal::from(args[0] != args[1]).into())
}

// ============================================================================
// List Builtins
// ============================================================================

/// Implements list:first - returns the first element of an RDF list.
///
/// # Arguments
/// - args[0]: An RDF list (as a blank node or named node representing the list head)
///
/// # Returns
/// The first element of the list, or None if the list is empty or invalid.
///
/// Note: This is a simplified implementation. A full implementation would need
/// to traverse the RDF graph structure to find rdf:first and rdf:rest.
pub fn list_first(args: &[Term]) -> Option<Term> {
    if args.len() != 1 {
        return None;
    }

    // This is a placeholder implementation.
    // A real implementation would need access to the RDF graph to follow rdf:first
    None
}

/// Implements list:rest - returns the tail of an RDF list.
///
/// # Arguments
/// - args[0]: An RDF list
///
/// # Returns
/// The rest of the list (everything after the first element).
///
/// Note: This is a simplified implementation. A full implementation would need
/// to traverse the RDF graph structure.
pub fn list_rest(args: &[Term]) -> Option<Term> {
    if args.len() != 1 {
        return None;
    }

    // This is a placeholder implementation.
    // A real implementation would need access to the RDF graph to follow rdf:rest
    None
}

/// Implements list:member - checks if an element is a member of an RDF list.
///
/// # Arguments
/// - args[0]: An RDF list
/// - args[1]: The element to search for
///
/// # Returns
/// A boolean literal (true if the element is in the list, false otherwise).
///
/// Note: This is a simplified implementation. A full implementation would need
/// to traverse the RDF graph structure.
pub fn list_member(args: &[Term]) -> Option<Term> {
    if args.len() != 2 {
        return None;
    }

    // This is a placeholder implementation.
    // A real implementation would need access to the RDF graph to traverse the list
    Some(Literal::from(false).into())
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Converts a literal to a double value, if possible.
fn to_double(lit: &Literal) -> Option<f64> {
    let value_str = lit.value();

    // Try parsing as integer first
    if let Ok(int) = value_str.parse::<i64>() {
        return Some(int as f64);
    }

    // Try parsing as decimal
    if let Ok(dec) = value_str.parse::<Decimal>() {
        return Some(f64::from(Double::from(dec)));
    }

    // Try parsing as float
    if let Ok(flt) = value_str.parse::<Float>() {
        return Some(f64::from(flt));
    }

    // Try parsing as double
    if let Ok(dbl) = value_str.parse::<Double>() {
        return Some(f64::from(dbl));
    }

    None
}

/// Extracts a string from a term (works with literals and IRIs).
fn term_to_string(term: &Term) -> Option<String> {
    match term {
        Term::Literal(lit) => Some(lit.value().to_string()),
        Term::NamedNode(nn) => Some(nn.as_str().to_string()),
        _ => None,
    }
}

// ============================================================================
// Registry Functions
// ============================================================================

/// Returns a vector of all N3 builtin functions with their IRI names.
///
/// This can be used to register all N3 builtins with a QueryEvaluator.
pub fn get_all_n3_builtins() -> Vec<(NamedNode, N3BuiltinFn)> {
    vec![
        // Math builtins
        (
            NamedNode::new_unchecked(format!("{MATH_NS}sum")),
            Arc::new(math_sum),
        ),
        (
            NamedNode::new_unchecked(format!("{MATH_NS}difference")),
            Arc::new(math_difference),
        ),
        (
            NamedNode::new_unchecked(format!("{MATH_NS}product")),
            Arc::new(math_product),
        ),
        (
            NamedNode::new_unchecked(format!("{MATH_NS}quotient")),
            Arc::new(math_quotient),
        ),
        // String builtins
        (
            NamedNode::new_unchecked(format!("{STRING_NS}concatenation")),
            Arc::new(string_concatenation),
        ),
        (
            NamedNode::new_unchecked(format!("{STRING_NS}contains")),
            Arc::new(string_contains),
        ),
        (
            NamedNode::new_unchecked(format!("{STRING_NS}length")),
            Arc::new(string_length),
        ),
        // Log builtins
        (
            NamedNode::new_unchecked(format!("{LOG_NS}equalTo")),
            Arc::new(log_equal_to),
        ),
        (
            NamedNode::new_unchecked(format!("{LOG_NS}notEqualTo")),
            Arc::new(log_not_equal_to),
        ),
        // List builtins
        (
            NamedNode::new_unchecked(format!("{LIST_NS}first")),
            Arc::new(list_first),
        ),
        (
            NamedNode::new_unchecked(format!("{LIST_NS}rest")),
            Arc::new(list_rest),
        ),
        (
            NamedNode::new_unchecked(format!("{LIST_NS}member")),
            Arc::new(list_member),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_math_sum_integers() {
        let args = vec![Literal::from(5).into(), Literal::from(3).into()];
        let result = math_sum(&args);
        assert_eq!(result, Some(Literal::from(8).into()));
    }

    #[test]
    fn test_math_sum_decimals() {
        let args = vec![
            Literal::from("2.5".parse::<Decimal>().unwrap()).into(),
            Literal::from("1.5".parse::<Decimal>().unwrap()).into(),
        ];
        let result = math_sum(&args);
        let expected = Literal::from("4.0".parse::<Decimal>().unwrap());
        assert_eq!(result, Some(expected.into()));
    }

    #[test]
    fn test_math_difference() {
        let args = vec![Literal::from(10).into(), Literal::from(3).into()];
        let result = math_difference(&args);
        assert_eq!(result, Some(Literal::from(7).into()));
    }

    #[test]
    fn test_math_product() {
        let args = vec![Literal::from(6).into(), Literal::from(7).into()];
        let result = math_product(&args);
        assert_eq!(result, Some(Literal::from(42).into()));
    }

    #[test]
    fn test_math_quotient() {
        let args = vec![Literal::from(20).into(), Literal::from(4).into()];
        let result = math_quotient(&args);
        assert_eq!(result, Some(Literal::from(5).into()));
    }

    #[test]
    fn test_math_quotient_division_by_zero() {
        let args = vec![Literal::from(10).into(), Literal::from(0).into()];
        let result = math_quotient(&args);
        assert_eq!(result, None);
    }

    #[test]
    fn test_string_concatenation() {
        let args = vec![
            Literal::new_simple_literal("Hello").into(),
            Literal::new_simple_literal(" World").into(),
        ];
        let result = string_concatenation(&args);
        assert_eq!(
            result,
            Some(Literal::new_simple_literal("Hello World").into())
        );
    }

    #[test]
    fn test_string_contains_true() {
        let args = vec![
            Literal::new_simple_literal("Hello World").into(),
            Literal::new_simple_literal("World").into(),
        ];
        let result = string_contains(&args);
        assert_eq!(result, Some(Literal::from(true).into()));
    }

    #[test]
    fn test_string_contains_false() {
        let args = vec![
            Literal::new_simple_literal("Hello").into(),
            Literal::new_simple_literal("World").into(),
        ];
        let result = string_contains(&args);
        assert_eq!(result, Some(Literal::from(false).into()));
    }

    #[test]
    fn test_string_length() {
        let args = vec![Literal::new_simple_literal("Hello").into()];
        let result = string_length(&args);
        assert_eq!(result, Some(Literal::from(5).into()));
    }

    #[test]
    fn test_log_equal_to_true() {
        let args = vec![Literal::from(42).into(), Literal::from(42).into()];
        let result = log_equal_to(&args);
        assert_eq!(result, Some(Literal::from(true).into()));
    }

    #[test]
    fn test_log_equal_to_false() {
        let args = vec![Literal::from(42).into(), Literal::from(24).into()];
        let result = log_equal_to(&args);
        assert_eq!(result, Some(Literal::from(false).into()));
    }

    #[test]
    fn test_log_not_equal_to_true() {
        let args = vec![Literal::from(42).into(), Literal::from(24).into()];
        let result = log_not_equal_to(&args);
        assert_eq!(result, Some(Literal::from(true).into()));
    }

    #[test]
    fn test_log_not_equal_to_false() {
        let args = vec![Literal::from(42).into(), Literal::from(42).into()];
        let result = log_not_equal_to(&args);
        assert_eq!(result, Some(Literal::from(false).into()));
    }

    #[test]
    fn test_get_all_n3_builtins() {
        let builtins = get_all_n3_builtins();
        assert_eq!(builtins.len(), 12);

        // Check that all expected builtins are present
        let names: Vec<String> = builtins.iter().map(|(n, _)| n.as_str().to_string()).collect();
        assert!(names.contains(&format!("{MATH_NS}sum")));
        assert!(names.contains(&format!("{STRING_NS}concatenation")));
        assert!(names.contains(&format!("{LOG_NS}equalTo")));
        assert!(names.contains(&format!("{LIST_NS}member")));
    }

    #[test]
    fn test_math_sum_wrong_arg_count() {
        let args = vec![Literal::from(5).into()];
        let result = math_sum(&args);
        assert_eq!(result, None);
    }

    #[test]
    fn test_string_concatenation_with_numbers() {
        let args = vec![Literal::from(42).into(), Literal::from(24).into()];
        let result = string_concatenation(&args);
        assert_eq!(result, Some(Literal::new_simple_literal("4224").into()));
    }
}
