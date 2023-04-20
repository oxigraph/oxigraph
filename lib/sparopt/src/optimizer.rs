use crate::algebra::{Expression, FixedPointGraphPattern, GraphPattern};

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
            #[cfg(feature = "fixed-point")]
            GraphPattern::FixedPoint {
                id,
                variables,
                constant,
                recursive,
            } => {
                GraphPattern::fixed_point(
                    id,
                    FixedPointGraphPattern::union(*constant, *recursive),
                    variables,
                )
                //TODO: recursive normalization
            }
        }
    }

    fn normalize_expression(expression: Expression) -> Expression {
        match expression {
            Expression::NamedNode(node) => node.into(),
            Expression::Literal(literal) => literal.into(),
            Expression::Variable(variable) => variable.into(),
            Expression::Or(left, right) => Expression::or(
                Self::normalize_expression(*left),
                Self::normalize_expression(*right),
            ),
            Expression::And(left, right) => Expression::and(
                Self::normalize_expression(*left),
                Self::normalize_expression(*right),
            ),
            Expression::Equal(left, right) => Expression::equal(
                Self::normalize_expression(*left),
                Self::normalize_expression(*right),
            ),
            Expression::SameTerm(left, right) => Expression::same_term(
                Self::normalize_expression(*left),
                Self::normalize_expression(*right),
            ),
            Expression::Greater(left, right) => Expression::greater(
                Self::normalize_expression(*left),
                Self::normalize_expression(*right),
            ),
            Expression::GreaterOrEqual(left, right) => Expression::greater_or_equal(
                Self::normalize_expression(*left),
                Self::normalize_expression(*right),
            ),
            Expression::Less(left, right) => Expression::less(
                Self::normalize_expression(*left),
                Self::normalize_expression(*right),
            ),
            Expression::LessOrEqual(left, right) => Expression::less_or_equal(
                Self::normalize_expression(*left),
                Self::normalize_expression(*right),
            ),
            Expression::Add(left, right) => Expression::add(
                Self::normalize_expression(*left),
                Self::normalize_expression(*right),
            ),
            Expression::Subtract(left, right) => Expression::subtract(
                Self::normalize_expression(*left),
                Self::normalize_expression(*right),
            ),
            Expression::Multiply(left, right) => Expression::multiply(
                Self::normalize_expression(*left),
                Self::normalize_expression(*right),
            ),
            Expression::Divide(left, right) => Expression::divide(
                Self::normalize_expression(*left),
                Self::normalize_expression(*right),
            ),
            Expression::UnaryPlus(inner) => {
                Expression::unary_plus(Self::normalize_expression(*inner))
            }
            Expression::UnaryMinus(inner) => {
                Expression::unary_minus(Self::normalize_expression(*inner))
            }
            Expression::Not(inner) => Expression::not(Self::normalize_expression(*inner)),
            Expression::Exists(inner) => Expression::exists(Self::normalize_pattern(*inner)),
            Expression::Bound(variable) => Expression::Bound(variable),
            Expression::If(cond, then, els) => Expression::if_cond(
                Self::normalize_expression(*cond),
                Self::normalize_expression(*then),
                Self::normalize_expression(*els),
            ),
            Expression::Coalesce(inners) => {
                Expression::coalesce(inners.into_iter().map(Self::normalize_expression).collect())
            }
            Expression::FunctionCall(name, args) => Expression::call(
                name,
                args.into_iter().map(Self::normalize_expression).collect(),
            ),
        }
    }
}
