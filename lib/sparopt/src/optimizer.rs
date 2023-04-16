use crate::algebra::{Expression, GraphPattern};

#[derive(Default)]
pub struct Optimizer {}

impl Optimizer {
    pub fn optimize(&mut self, pattern: GraphPattern) -> GraphPattern {
        Self::normalize_pattern(pattern)
    }

    fn normalize_pattern(pattern: GraphPattern) -> GraphPattern {
        match pattern {
            GraphPattern::QuadPattern {
                subject,
                predicate,
                object,
                graph_name,
            } => GraphPattern::QuadPattern {
                subject,
                predicate,
                object,
                graph_name,
            },
            GraphPattern::Path {
                subject,
                path,
                object,
                graph_name,
            } => GraphPattern::Path {
                subject,
                path,
                object,
                graph_name,
            },
            GraphPattern::Join { left, right } => GraphPattern::join(
                Self::normalize_pattern(*left),
                Self::normalize_pattern(*right),
            ),
            GraphPattern::LeftJoin {
                left,
                right,
                expression,
            } => GraphPattern::left_join(
                Self::normalize_pattern(*left),
                Self::normalize_pattern(*right),
                Self::normalize_expression(expression),
            ),
            #[cfg(feature = "sep-0006")]
            GraphPattern::Lateral { left, right } => GraphPattern::lateral(
                Self::normalize_pattern(*left),
                Self::normalize_pattern(*right),
            ),
            GraphPattern::Filter { inner, expression } => GraphPattern::filter(
                Self::normalize_pattern(*inner),
                Self::normalize_expression(expression),
            ),
            GraphPattern::Union { inner } => inner
                .into_iter()
                .map(Self::normalize_pattern)
                .reduce(GraphPattern::union)
                .unwrap_or_else(GraphPattern::empty),
            GraphPattern::Extend {
                inner,
                variable,
                expression,
            } => GraphPattern::extend(Self::normalize_pattern(*inner), variable, expression),
            GraphPattern::Minus { left, right } => GraphPattern::minus(
                Self::normalize_pattern(*left),
                Self::normalize_pattern(*right),
            ),
            GraphPattern::Values {
                variables,
                bindings,
            } => GraphPattern::values(variables, bindings),
            GraphPattern::OrderBy { inner, expression } => {
                GraphPattern::order_by(Self::normalize_pattern(*inner), expression)
            }
            GraphPattern::Project { inner, variables } => {
                GraphPattern::project(Self::normalize_pattern(*inner), variables)
            }
            GraphPattern::Distinct { inner } => {
                GraphPattern::distinct(Self::normalize_pattern(*inner))
            }
            GraphPattern::Reduced { inner } => {
                GraphPattern::reduced(Self::normalize_pattern(*inner))
            }
            GraphPattern::Slice {
                inner,
                start,
                length,
            } => GraphPattern::slice(Self::normalize_pattern(*inner), start, length),
            GraphPattern::Group {
                inner,
                variables,
                aggregates,
            } => GraphPattern::group(Self::normalize_pattern(*inner), variables, aggregates),
            GraphPattern::Service {
                name,
                inner,
                silent,
            } => GraphPattern::service(Self::normalize_pattern(*inner), name, silent),
        }
    }

    fn normalize_expression(expression: Expression) -> Expression {
        match expression {
            Expression::NamedNode(node) => node.into(),
            Expression::Literal(literal) => literal.into(),
            Expression::Variable(variable) => variable.into(),
            Expression::Or(left, right) => {
                let left = Self::normalize_expression(*left);
                let right = Self::normalize_expression(*right);
                match (
                    left.effective_boolean_value(),
                    right.effective_boolean_value(),
                ) {
                    (Some(true), _) | (_, Some(true)) => true.into(),
                    (Some(false), Some(false)) => false.into(),
                    _ => Expression::Or(Box::new(left), Box::new(right)),
                }
            }
            Expression::And(left, right) => {
                let left = Self::normalize_expression(*left);
                let right = Self::normalize_expression(*right);
                match (
                    left.effective_boolean_value(),
                    right.effective_boolean_value(),
                ) {
                    (Some(false), _) | (_, Some(false)) => false.into(),
                    (Some(true), Some(true)) => true.into(),
                    _ => Expression::And(Box::new(left), Box::new(right)),
                }
            }
            Expression::Equal(left, right) => {
                let left = Self::normalize_expression(*left);
                let right = Self::normalize_expression(*right);
                Expression::Equal(Box::new(left), Box::new(right))
            }
            Expression::SameTerm(left, right) => {
                let left = Self::normalize_expression(*left);
                let right = Self::normalize_expression(*right);
                Expression::SameTerm(Box::new(left), Box::new(right))
            }
            Expression::Greater(left, right) => {
                let left = Self::normalize_expression(*left);
                let right = Self::normalize_expression(*right);
                Expression::Greater(Box::new(left), Box::new(right))
            }
            Expression::GreaterOrEqual(left, right) => {
                let left = Self::normalize_expression(*left);
                let right = Self::normalize_expression(*right);
                Expression::GreaterOrEqual(Box::new(left), Box::new(right))
            }
            Expression::Less(left, right) => {
                let left = Self::normalize_expression(*left);
                let right = Self::normalize_expression(*right);
                Expression::Less(Box::new(left), Box::new(right))
            }
            Expression::LessOrEqual(left, right) => {
                let left = Self::normalize_expression(*left);
                let right = Self::normalize_expression(*right);
                Expression::LessOrEqual(Box::new(left), Box::new(right))
            }
            Expression::In(left, right) => {
                let left = Self::normalize_expression(*left);
                let right = right.into_iter().map(Self::normalize_expression).collect();
                Expression::In(Box::new(left), right)
            }
            Expression::Add(left, right) => {
                let left = Self::normalize_expression(*left);
                let right = Self::normalize_expression(*right);
                Expression::Add(Box::new(left), Box::new(right))
            }
            Expression::Subtract(left, right) => {
                let left = Self::normalize_expression(*left);
                let right = Self::normalize_expression(*right);
                Expression::Subtract(Box::new(left), Box::new(right))
            }
            Expression::Multiply(left, right) => {
                let left = Self::normalize_expression(*left);
                let right = Self::normalize_expression(*right);
                Expression::Multiply(Box::new(left), Box::new(right))
            }
            Expression::Divide(left, right) => {
                let left = Self::normalize_expression(*left);
                let right = Self::normalize_expression(*right);
                Expression::Divide(Box::new(left), Box::new(right))
            }
            Expression::UnaryPlus(inner) => {
                let inner = Self::normalize_expression(*inner);
                Expression::UnaryPlus(Box::new(inner))
            }
            Expression::UnaryMinus(inner) => {
                let inner = Self::normalize_expression(*inner);
                Expression::UnaryMinus(Box::new(inner))
            }
            Expression::Not(inner) => {
                let inner = Self::normalize_expression(*inner);
                Expression::Not(Box::new(inner))
            }
            Expression::Exists(inner) => {
                let inner = Self::normalize_pattern(*inner);
                Expression::Exists(Box::new(inner))
            }
            Expression::Bound(variable) => Expression::Bound(variable),
            Expression::If(cond, then, els) => {
                let cond = Self::normalize_expression(*cond);
                let then = Self::normalize_expression(*then);
                let els = Self::normalize_expression(*els);
                match cond.effective_boolean_value() {
                    Some(true) => then,
                    Some(false) => els,
                    None => Expression::If(Box::new(cond), Box::new(then), Box::new(els)),
                }
            }
            Expression::Coalesce(inners) => {
                Expression::Coalesce(inners.into_iter().map(Self::normalize_expression).collect())
            }
            Expression::FunctionCall(name, args) => Expression::FunctionCall(
                name,
                args.into_iter().map(Self::normalize_expression).collect(),
            ),
        }
    }
}
