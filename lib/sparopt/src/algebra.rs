//! [SPARQL 1.1 Query Algebra](https://www.w3.org/TR/sparql11-query/#sparqlQuery) representation.

use oxrdf::vocab::xsd;
use rand::random;
use spargebra::algebra::{
    AggregateExpression as AlAggregateExpression, Expression as AlExpression,
    GraphPattern as AlGraphPattern, OrderExpression as AlOrderExpression,
};
pub use spargebra::algebra::{Function, PropertyPathExpression};
#[cfg(feature = "rdf-star")]
use spargebra::term::GroundTriplePattern;
use spargebra::term::{BlankNode, TermPattern, TriplePattern};
pub use spargebra::term::{
    GroundTerm, GroundTermPattern, Literal, NamedNode, NamedNodePattern, Variable,
};
use std::collections::HashMap;

/// An [expression](https://www.w3.org/TR/sparql11-query/#expressions).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum Expression {
    NamedNode(NamedNode),
    Literal(Literal),
    Variable(Variable),
    /// [Logical-or](https://www.w3.org/TR/sparql11-query/#func-logical-or).
    Or(Box<Self>, Box<Self>),
    /// [Logical-and](https://www.w3.org/TR/sparql11-query/#func-logical-and).
    And(Box<Self>, Box<Self>),
    /// [RDFterm-equal](https://www.w3.org/TR/sparql11-query/#func-RDFterm-equal) and all the XSD equalities.
    Equal(Box<Self>, Box<Self>),
    /// [sameTerm](https://www.w3.org/TR/sparql11-query/#func-sameTerm).
    SameTerm(Box<Self>, Box<Self>),
    /// [op:numeric-greater-than](https://www.w3.org/TR/xpath-functions/#func-numeric-greater-than) and other XSD greater than operators.
    Greater(Box<Self>, Box<Self>),
    GreaterOrEqual(Box<Self>, Box<Self>),
    /// [op:numeric-less-than](https://www.w3.org/TR/xpath-functions/#func-numeric-less-than) and other XSD greater than operators.
    Less(Box<Self>, Box<Self>),
    LessOrEqual(Box<Self>, Box<Self>),
    /// [IN](https://www.w3.org/TR/sparql11-query/#func-in)
    In(Box<Self>, Vec<Self>),
    /// [op:numeric-add](https://www.w3.org/TR/xpath-functions/#func-numeric-add) and other XSD additions.
    Add(Box<Self>, Box<Self>),
    /// [op:numeric-subtract](https://www.w3.org/TR/xpath-functions/#func-numeric-subtract) and other XSD subtractions.
    Subtract(Box<Self>, Box<Self>),
    /// [op:numeric-multiply](https://www.w3.org/TR/xpath-functions/#func-numeric-multiply) and other XSD multiplications.
    Multiply(Box<Self>, Box<Self>),
    /// [op:numeric-divide](https://www.w3.org/TR/xpath-functions/#func-numeric-divide) and other XSD divides.
    Divide(Box<Self>, Box<Self>),
    /// [op:numeric-unary-plus](https://www.w3.org/TR/xpath-functions/#func-numeric-unary-plus) and other XSD unary plus.
    UnaryPlus(Box<Self>),
    /// [op:numeric-unary-minus](https://www.w3.org/TR/xpath-functions/#func-numeric-unary-minus) and other XSD unary minus.
    UnaryMinus(Box<Self>),
    /// [fn:not](https://www.w3.org/TR/xpath-functions/#func-not).
    Not(Box<Self>),
    /// [EXISTS](https://www.w3.org/TR/sparql11-query/#func-filter-exists).
    Exists(Box<GraphPattern>),
    /// [BOUND](https://www.w3.org/TR/sparql11-query/#func-bound).
    Bound(Variable),
    /// [IF](https://www.w3.org/TR/sparql11-query/#func-if).
    If(Box<Self>, Box<Self>, Box<Self>),
    /// [COALESCE](https://www.w3.org/TR/sparql11-query/#func-coalesce).
    Coalesce(Vec<Self>),
    /// A regular function call.
    FunctionCall(Function, Vec<Self>),
}

impl Expression {
    pub(crate) fn effective_boolean_value(&self) -> Option<bool> {
        match self {
            Expression::NamedNode(_) => Some(true),
            Expression::Literal(literal) => {
                if literal.datatype() == xsd::BOOLEAN {
                    match literal.value() {
                        "true" | "1" => Some(true),
                        "false" | "0" => Some(false),
                        _ => None, //TODO
                    }
                } else {
                    None
                }
            }
            _ => None, // We assume the expression has been normalized
        }
    }

    fn from_sparql_algebra(
        expression: &AlExpression,
        graph_name: Option<&NamedNodePattern>,
    ) -> Self {
        match expression {
            AlExpression::NamedNode(node) => Self::NamedNode(node.clone()),
            AlExpression::Literal(literal) => Self::Literal(literal.clone()),
            AlExpression::Variable(variable) => Self::Variable(variable.clone()),
            AlExpression::Or(left, right) => Self::Or(
                Box::new(Self::from_sparql_algebra(left, graph_name)),
                Box::new(Self::from_sparql_algebra(right, graph_name)),
            ),
            AlExpression::And(left, right) => Self::And(
                Box::new(Self::from_sparql_algebra(left, graph_name)),
                Box::new(Self::from_sparql_algebra(right, graph_name)),
            ),
            AlExpression::Equal(left, right) => Self::Equal(
                Box::new(Self::from_sparql_algebra(left, graph_name)),
                Box::new(Self::from_sparql_algebra(right, graph_name)),
            ),
            AlExpression::SameTerm(left, right) => Self::SameTerm(
                Box::new(Self::from_sparql_algebra(left, graph_name)),
                Box::new(Self::from_sparql_algebra(right, graph_name)),
            ),
            AlExpression::Greater(left, right) => Self::Greater(
                Box::new(Self::from_sparql_algebra(left, graph_name)),
                Box::new(Self::from_sparql_algebra(right, graph_name)),
            ),
            AlExpression::GreaterOrEqual(left, right) => Self::GreaterOrEqual(
                Box::new(Self::from_sparql_algebra(left, graph_name)),
                Box::new(Self::from_sparql_algebra(right, graph_name)),
            ),
            AlExpression::Less(left, right) => Self::Less(
                Box::new(Self::from_sparql_algebra(left, graph_name)),
                Box::new(Self::from_sparql_algebra(right, graph_name)),
            ),
            AlExpression::LessOrEqual(left, right) => Self::LessOrEqual(
                Box::new(Self::from_sparql_algebra(left, graph_name)),
                Box::new(Self::from_sparql_algebra(right, graph_name)),
            ),
            AlExpression::In(left, right) => Self::In(
                Box::new(Self::from_sparql_algebra(left, graph_name)),
                right
                    .iter()
                    .map(|e| Self::from_sparql_algebra(e, graph_name))
                    .collect(),
            ),
            AlExpression::Add(left, right) => Self::Add(
                Box::new(Self::from_sparql_algebra(left, graph_name)),
                Box::new(Self::from_sparql_algebra(right, graph_name)),
            ),
            AlExpression::Subtract(left, right) => Self::Subtract(
                Box::new(Self::from_sparql_algebra(left, graph_name)),
                Box::new(Self::from_sparql_algebra(right, graph_name)),
            ),
            AlExpression::Multiply(left, right) => Self::Multiply(
                Box::new(Self::from_sparql_algebra(left, graph_name)),
                Box::new(Self::from_sparql_algebra(right, graph_name)),
            ),
            AlExpression::Divide(left, right) => Self::Divide(
                Box::new(Self::from_sparql_algebra(left, graph_name)),
                Box::new(Self::from_sparql_algebra(right, graph_name)),
            ),
            AlExpression::UnaryPlus(inner) => {
                Self::UnaryPlus(Box::new(Self::from_sparql_algebra(inner, graph_name)))
            }
            AlExpression::UnaryMinus(inner) => {
                Self::UnaryMinus(Box::new(Self::from_sparql_algebra(inner, graph_name)))
            }
            AlExpression::Not(inner) => {
                Self::Not(Box::new(Self::from_sparql_algebra(inner, graph_name)))
            }
            AlExpression::Exists(inner) => Self::Exists(Box::new(
                GraphPattern::from_sparql_algebra(inner, graph_name, &mut HashMap::new()),
            )),
            AlExpression::Bound(variable) => Self::Bound(variable.clone()),
            AlExpression::If(cond, yes, no) => Self::If(
                Box::new(Self::from_sparql_algebra(cond, graph_name)),
                Box::new(Self::from_sparql_algebra(yes, graph_name)),
                Box::new(Self::from_sparql_algebra(no, graph_name)),
            ),
            AlExpression::Coalesce(inner) => Self::Coalesce(
                inner
                    .iter()
                    .map(|e| Self::from_sparql_algebra(e, graph_name))
                    .collect(),
            ),
            AlExpression::FunctionCall(name, args) => Self::FunctionCall(
                name.clone(),
                args.iter()
                    .map(|e| Self::from_sparql_algebra(e, graph_name))
                    .collect(),
            ),
        }
    }
}

impl From<NamedNode> for Expression {
    fn from(value: NamedNode) -> Self {
        Self::NamedNode(value)
    }
}

impl From<Literal> for Expression {
    fn from(value: Literal) -> Self {
        Self::Literal(value)
    }
}

impl From<Variable> for Expression {
    fn from(value: Variable) -> Self {
        Self::Variable(value)
    }
}

impl From<bool> for Expression {
    fn from(value: bool) -> Self {
        Literal::from(value).into()
    }
}

impl From<&Expression> for AlExpression {
    fn from(expression: &Expression) -> Self {
        match expression {
            Expression::NamedNode(node) => Self::NamedNode(node.clone()),
            Expression::Literal(literal) => Self::Literal(literal.clone()),
            Expression::Variable(variable) => Self::Variable(variable.clone()),
            Expression::Or(left, right) => Self::Or(
                Box::new(left.as_ref().into()),
                Box::new(right.as_ref().into()),
            ),
            Expression::And(left, right) => Self::And(
                Box::new(left.as_ref().into()),
                Box::new(right.as_ref().into()),
            ),
            Expression::Equal(left, right) => Self::Equal(
                Box::new(left.as_ref().into()),
                Box::new(right.as_ref().into()),
            ),
            Expression::SameTerm(left, right) => Self::SameTerm(
                Box::new(left.as_ref().into()),
                Box::new(right.as_ref().into()),
            ),
            Expression::Greater(left, right) => Self::Greater(
                Box::new(left.as_ref().into()),
                Box::new(right.as_ref().into()),
            ),
            Expression::GreaterOrEqual(left, right) => Self::GreaterOrEqual(
                Box::new(left.as_ref().into()),
                Box::new(right.as_ref().into()),
            ),
            Expression::Less(left, right) => Self::Less(
                Box::new(left.as_ref().into()),
                Box::new(right.as_ref().into()),
            ),
            Expression::LessOrEqual(left, right) => Self::LessOrEqual(
                Box::new(left.as_ref().into()),
                Box::new(right.as_ref().into()),
            ),
            Expression::In(left, right) => Self::In(
                Box::new(left.as_ref().into()),
                right.iter().map(|e| e.into()).collect(),
            ),
            Expression::Add(left, right) => Self::Add(
                Box::new(left.as_ref().into()),
                Box::new(right.as_ref().into()),
            ),
            Expression::Subtract(left, right) => Self::Subtract(
                Box::new(left.as_ref().into()),
                Box::new(right.as_ref().into()),
            ),
            Expression::Multiply(left, right) => Self::Multiply(
                Box::new(left.as_ref().into()),
                Box::new(right.as_ref().into()),
            ),
            Expression::Divide(left, right) => Self::Divide(
                Box::new(left.as_ref().into()),
                Box::new(right.as_ref().into()),
            ),
            Expression::UnaryPlus(inner) => Self::UnaryPlus(Box::new(inner.as_ref().into())),
            Expression::UnaryMinus(inner) => Self::UnaryMinus(Box::new(inner.as_ref().into())),
            Expression::Not(inner) => Self::Not(Box::new(inner.as_ref().into())),
            Expression::Exists(inner) => Self::Exists(Box::new(inner.as_ref().into())),
            Expression::Bound(variable) => Self::Bound(variable.clone()),
            Expression::If(cond, yes, no) => Self::If(
                Box::new(cond.as_ref().into()),
                Box::new(yes.as_ref().into()),
                Box::new(no.as_ref().into()),
            ),
            Expression::Coalesce(inner) => Self::Coalesce(inner.iter().map(|e| e.into()).collect()),
            Expression::FunctionCall(name, args) => {
                Self::FunctionCall(name.clone(), args.iter().map(|e| e.into()).collect())
            }
        }
    }
}

/// A SPARQL query [graph pattern](https://www.w3.org/TR/sparql11-query/#sparqlQuery).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum GraphPattern {
    /// A [basic graph pattern](https://www.w3.org/TR/sparql11-query/#defn_BasicGraphPattern).
    QuadPattern {
        subject: GroundTermPattern,
        predicate: NamedNodePattern,
        object: GroundTermPattern,
        graph_name: Option<NamedNodePattern>,
    },
    /// A [property path pattern](https://www.w3.org/TR/sparql11-query/#defn_evalPP_predicate).
    Path {
        subject: GroundTermPattern,
        path: PropertyPathExpression,
        object: GroundTermPattern,
        graph_name: Option<NamedNodePattern>,
    },
    /// [Join](https://www.w3.org/TR/sparql11-query/#defn_algJoin).
    Join { left: Box<Self>, right: Box<Self> },
    /// [LeftJoin](https://www.w3.org/TR/sparql11-query/#defn_algLeftJoin).
    LeftJoin {
        left: Box<Self>,
        right: Box<Self>,
        expression: Expression,
    },
    /// Lateral join i.e. evaluate right for all result row of left
    #[cfg(feature = "sep-0006")]
    Lateral { left: Box<Self>, right: Box<Self> },
    /// [Filter](https://www.w3.org/TR/sparql11-query/#defn_algFilter).
    Filter {
        expression: Expression,
        inner: Box<Self>,
    },
    /// [Union](https://www.w3.org/TR/sparql11-query/#defn_algUnion).
    Union { inner: Vec<Self> },
    /// [Extend](https://www.w3.org/TR/sparql11-query/#defn_extend).
    Extend {
        inner: Box<Self>,
        variable: Variable,
        expression: Expression,
    },
    /// [Minus](https://www.w3.org/TR/sparql11-query/#defn_algMinus).
    Minus { left: Box<Self>, right: Box<Self> },
    /// A table used to provide inline values
    Values {
        variables: Vec<Variable>,
        bindings: Vec<Vec<Option<GroundTerm>>>,
    },
    /// [OrderBy](https://www.w3.org/TR/sparql11-query/#defn_algOrdered).
    OrderBy {
        inner: Box<Self>,
        expression: Vec<OrderExpression>,
    },
    /// [Project](https://www.w3.org/TR/sparql11-query/#defn_algProjection).
    Project {
        inner: Box<Self>,
        variables: Vec<Variable>,
    },
    /// [Distinct](https://www.w3.org/TR/sparql11-query/#defn_algDistinct).
    Distinct { inner: Box<Self> },
    /// [Reduced](https://www.w3.org/TR/sparql11-query/#defn_algReduced).
    Reduced { inner: Box<Self> },
    /// [Slice](https://www.w3.org/TR/sparql11-query/#defn_algSlice).
    Slice {
        inner: Box<Self>,
        start: usize,
        length: Option<usize>,
    },
    /// [Group](https://www.w3.org/TR/sparql11-federated-query/#aggregateAlgebra).
    Group {
        inner: Box<Self>,
        variables: Vec<Variable>,
        aggregates: Vec<(Variable, AggregateExpression)>,
    },
    /// [Service](https://www.w3.org/TR/sparql11-federated-query/#defn_evalService).
    Service {
        name: NamedNodePattern,
        inner: Box<Self>,
        silent: bool,
    },
}

impl GraphPattern {
    pub(crate) fn empty() -> Self {
        Self::Values {
            variables: Vec::new(),
            bindings: Vec::new(),
        }
    }

    /// Check if the pattern is the empty table
    fn is_empty(&self) -> bool {
        if let Self::Values { bindings, .. } = self {
            bindings.is_empty()
        } else {
            false
        }
    }

    fn singleton() -> Self {
        Self::Values {
            variables: Vec::new(),
            bindings: vec![Vec::new()],
        }
    }

    pub(crate) fn join(left: Self, right: Self) -> Self {
        if left.is_empty() || right.is_empty() {
            return Self::empty();
        }
        Self::Join {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    #[cfg(feature = "sep-0006")]
    pub(crate) fn lateral(left: Self, right: Self) -> Self {
        if left.is_empty() || right.is_empty() {
            return Self::empty();
        }
        Self::Lateral {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    pub(crate) fn left_join(left: Self, right: Self, expression: Expression) -> Self {
        let expression_ebv = expression.effective_boolean_value();
        if left.is_empty() || right.is_empty() || expression_ebv == Some(false) {
            return left;
        }
        Self::LeftJoin {
            left: Box::new(left),
            right: Box::new(right),
            expression: if expression_ebv == Some(true) {
                true.into()
            } else {
                expression
            },
        }
    }

    pub(crate) fn minus(left: Self, right: Self) -> Self {
        if left.is_empty() {
            return Self::empty();
        }
        if right.is_empty() {
            return left;
        }
        Self::Minus {
            left: Box::new(left),
            right: Box::new(right),
        }
    }

    pub(crate) fn union(left: Self, right: Self) -> Self {
        if left.is_empty() {
            return right;
        }
        if right.is_empty() {
            return left;
        }
        Self::Union {
            inner: match (left, right) {
                (Self::Union { inner: mut left }, Self::Union { inner: right }) => {
                    left.extend(right);
                    left
                }
                (Self::Union { inner: mut left }, right) => {
                    left.push(right);
                    left
                }
                (left, Self::Union { inner: mut right }) => {
                    right.insert(0, left);
                    right
                }
                (left, right) => vec![left, right],
            },
        }
    }

    pub(crate) fn filter(inner: Self, expression: Expression) -> Self {
        if inner.is_empty() {
            return Self::empty();
        }
        match expression.effective_boolean_value() {
            Some(true) => inner,
            Some(false) => Self::empty(),
            None => Self::Filter {
                inner: Box::new(inner),
                expression,
            },
        }
    }

    pub(crate) fn extend(inner: Self, variable: Variable, expression: Expression) -> Self {
        if inner.is_empty() {
            return Self::empty();
        }
        Self::Extend {
            inner: Box::new(inner),
            variable,
            expression,
        }
    }

    pub(crate) fn values(
        mut variables: Vec<Variable>,
        mut bindings: Vec<Vec<Option<GroundTerm>>>,
    ) -> Self {
        let empty_rows = (0..variables.len())
            .filter(|row| !bindings.iter().any(|binding| binding.get(*row).is_some()))
            .collect::<Vec<_>>();
        if !empty_rows.is_empty() {
            // We remove empty rows
            variables = variables
                .into_iter()
                .enumerate()
                .filter_map(|(i, v)| {
                    if empty_rows.contains(&i) {
                        None
                    } else {
                        Some(v)
                    }
                })
                .collect();
            bindings = bindings
                .into_iter()
                .map(|binding| {
                    binding
                        .into_iter()
                        .enumerate()
                        .filter_map(|(i, v)| {
                            if empty_rows.contains(&i) {
                                None
                            } else {
                                Some(v)
                            }
                        })
                        .collect()
                })
                .collect();
        }
        Self::Values {
            variables,
            bindings,
        }
    }

    pub(crate) fn order_by(inner: Self, expression: Vec<OrderExpression>) -> Self {
        if inner.is_empty() {
            return Self::empty();
        }
        if expression.is_empty() {
            return inner;
        }
        Self::OrderBy {
            inner: Box::new(inner),
            expression,
        }
    }

    pub(crate) fn project(inner: Self, variables: Vec<Variable>) -> Self {
        if inner.is_empty() {
            return Self::empty();
        }
        Self::Project {
            inner: Box::new(inner),
            variables,
        }
    }

    pub(crate) fn distinct(inner: Self) -> Self {
        if inner.is_empty() {
            return Self::empty();
        }
        Self::Distinct {
            inner: Box::new(inner),
        }
    }

    pub(crate) fn reduced(inner: Self) -> Self {
        if inner.is_empty() {
            return Self::empty();
        }
        Self::Reduced {
            inner: Box::new(inner),
        }
    }

    pub(crate) fn slice(inner: Self, start: usize, length: Option<usize>) -> Self {
        if inner.is_empty() {
            return Self::empty();
        }
        if start == 0 && length.is_none() {
            return inner;
        }
        Self::Slice {
            inner: Box::new(inner),
            start,
            length,
        }
    }

    pub(crate) fn group(
        inner: Self,
        variables: Vec<Variable>,
        aggregates: Vec<(Variable, AggregateExpression)>,
    ) -> Self {
        if inner.is_empty() {
            return Self::empty();
        }
        Self::Group {
            inner: Box::new(inner),
            variables,
            aggregates,
        }
    }

    pub(crate) fn service(inner: Self, name: NamedNodePattern, silent: bool) -> Self {
        if inner.is_empty() {
            return Self::empty();
        }
        Self::Service {
            inner: Box::new(inner),
            name,
            silent,
        }
    }
    fn from_sparql_algebra(
        pattern: &AlGraphPattern,
        graph_name: Option<&NamedNodePattern>,
        blank_nodes: &mut HashMap<BlankNode, Variable>,
    ) -> Self {
        match pattern {
            AlGraphPattern::Bgp { patterns } => patterns
                .iter()
                .map(|p| {
                    let (subject, predicate, object) =
                        Self::triple_pattern_from_algebra(p, blank_nodes);
                    Self::QuadPattern {
                        subject,
                        predicate,
                        object,
                        graph_name: graph_name.cloned(),
                    }
                })
                .reduce(|a, b| Self::Join {
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .unwrap_or_else(Self::singleton),
            AlGraphPattern::Path {
                subject,
                path,
                object,
            } => Self::Path {
                subject: Self::term_pattern_from_algebra(subject, blank_nodes),
                path: path.clone(),
                object: Self::term_pattern_from_algebra(object, blank_nodes),
                graph_name: graph_name.cloned(),
            },
            AlGraphPattern::Join { left, right } => Self::Join {
                left: Box::new(Self::from_sparql_algebra(left, graph_name, blank_nodes)),
                right: Box::new(Self::from_sparql_algebra(right, graph_name, blank_nodes)),
            },
            AlGraphPattern::LeftJoin {
                left,
                right,
                expression,
            } => Self::LeftJoin {
                left: Box::new(Self::from_sparql_algebra(left, graph_name, blank_nodes)),
                right: Box::new(Self::from_sparql_algebra(right, graph_name, blank_nodes)),
                expression: expression.as_ref().map_or_else(
                    || true.into(),
                    |e| Expression::from_sparql_algebra(e, graph_name),
                ),
            },
            #[cfg(feature = "sep-0006")]
            AlGraphPattern::Lateral { left, right } => Self::Lateral {
                left: Box::new(Self::from_sparql_algebra(left, graph_name, blank_nodes)),
                right: Box::new(Self::from_sparql_algebra(right, graph_name, blank_nodes)),
            },
            AlGraphPattern::Filter { inner, expr } => Self::Filter {
                inner: Box::new(Self::from_sparql_algebra(inner, graph_name, blank_nodes)),
                expression: Expression::from_sparql_algebra(expr, graph_name),
            },
            AlGraphPattern::Union { left, right } => Self::Union {
                inner: vec![
                    Self::from_sparql_algebra(left, graph_name, blank_nodes),
                    Self::from_sparql_algebra(right, graph_name, blank_nodes),
                ],
            },
            AlGraphPattern::Graph { inner, name } => {
                Self::from_sparql_algebra(inner, Some(name), blank_nodes)
            }
            AlGraphPattern::Extend {
                inner,
                expression,
                variable,
            } => Self::Extend {
                inner: Box::new(Self::from_sparql_algebra(inner, graph_name, blank_nodes)),
                expression: Expression::from_sparql_algebra(expression, graph_name),
                variable: variable.clone(),
            },
            AlGraphPattern::Minus { left, right } => Self::Minus {
                left: Box::new(Self::from_sparql_algebra(left, graph_name, blank_nodes)),
                right: Box::new(Self::from_sparql_algebra(right, graph_name, blank_nodes)),
            },
            AlGraphPattern::Values {
                variables,
                bindings,
            } => Self::Values {
                variables: variables.clone(),
                bindings: bindings.clone(),
            },
            AlGraphPattern::OrderBy { inner, expression } => Self::OrderBy {
                inner: Box::new(Self::from_sparql_algebra(inner, graph_name, blank_nodes)),
                expression: expression
                    .iter()
                    .map(|e| OrderExpression::from_sparql_algebra(e, graph_name))
                    .collect(),
            },
            AlGraphPattern::Project { inner, variables } => {
                let graph_name = if let Some(NamedNodePattern::Variable(graph_name)) = graph_name {
                    Some(NamedNodePattern::Variable(
                        if variables.contains(graph_name) {
                            graph_name.clone()
                        } else {
                            new_var()
                        },
                    ))
                } else {
                    graph_name.cloned()
                };
                Self::Project {
                    inner: Box::new(Self::from_sparql_algebra(
                        inner,
                        graph_name.as_ref(),
                        &mut HashMap::new(),
                    )),
                    variables: variables.clone(),
                }
            }
            AlGraphPattern::Distinct { inner } => Self::Distinct {
                inner: Box::new(Self::from_sparql_algebra(inner, graph_name, blank_nodes)),
            },
            AlGraphPattern::Reduced { inner } => Self::Distinct {
                inner: Box::new(Self::from_sparql_algebra(inner, graph_name, blank_nodes)),
            },
            AlGraphPattern::Slice {
                inner,
                start,
                length,
            } => Self::Slice {
                inner: Box::new(Self::from_sparql_algebra(inner, graph_name, blank_nodes)),
                start: *start,
                length: *length,
            },
            AlGraphPattern::Group {
                inner,
                variables,
                aggregates,
            } => Self::Group {
                inner: Box::new(Self::from_sparql_algebra(inner, graph_name, blank_nodes)),
                variables: variables.clone(),
                aggregates: aggregates
                    .iter()
                    .map(|(var, expr)| {
                        (
                            var.clone(),
                            AggregateExpression::from_sparql_algebra(expr, graph_name),
                        )
                    })
                    .collect(),
            },
            AlGraphPattern::Service {
                inner,
                name,
                silent,
            } => Self::Service {
                inner: Box::new(Self::from_sparql_algebra(inner, graph_name, blank_nodes)),
                name: name.clone(),
                silent: *silent,
            },
        }
    }

    fn triple_pattern_from_algebra(
        pattern: &TriplePattern,
        blank_nodes: &mut HashMap<BlankNode, Variable>,
    ) -> (GroundTermPattern, NamedNodePattern, GroundTermPattern) {
        (
            Self::term_pattern_from_algebra(&pattern.subject, blank_nodes),
            pattern.predicate.clone(),
            Self::term_pattern_from_algebra(&pattern.object, blank_nodes),
        )
    }

    fn term_pattern_from_algebra(
        pattern: &TermPattern,
        blank_nodes: &mut HashMap<BlankNode, Variable>,
    ) -> GroundTermPattern {
        match pattern {
            TermPattern::NamedNode(node) => node.clone().into(),
            TermPattern::BlankNode(node) => blank_nodes
                .entry(node.clone())
                .or_insert_with(new_var)
                .clone()
                .into(),
            TermPattern::Literal(literal) => literal.clone().into(),
            #[cfg(feature = "rdf-star")]
            TermPattern::Triple(pattern) => {
                let (subject, predicate, object) =
                    Self::triple_pattern_from_algebra(pattern, blank_nodes);
                GroundTriplePattern {
                    subject,
                    predicate,
                    object,
                }
                .into()
            }
            TermPattern::Variable(variable) => variable.clone().into(),
        }
    }
}

impl From<&AlGraphPattern> for GraphPattern {
    fn from(pattern: &AlGraphPattern) -> Self {
        Self::from_sparql_algebra(pattern, None, &mut HashMap::new())
    }
}

impl From<&GraphPattern> for AlGraphPattern {
    fn from(pattern: &GraphPattern) -> Self {
        match pattern {
            GraphPattern::QuadPattern {
                subject,
                predicate,
                object,
                graph_name,
            } => {
                let pattern = Self::Bgp {
                    patterns: vec![TriplePattern {
                        subject: subject.clone().into(),
                        predicate: predicate.clone(),
                        object: object.clone().into(),
                    }],
                };
                if let Some(graph_name) = graph_name {
                    Self::Graph {
                        inner: Box::new(pattern),
                        name: graph_name.clone(),
                    }
                } else {
                    pattern
                }
            }
            GraphPattern::Path {
                subject,
                path,
                object,
                graph_name,
            } => {
                let pattern = Self::Path {
                    subject: subject.clone().into(),
                    path: path.clone(),
                    object: object.clone().into(),
                };
                if let Some(graph_name) = graph_name {
                    Self::Graph {
                        inner: Box::new(pattern),
                        name: graph_name.clone(),
                    }
                } else {
                    pattern
                }
            }
            GraphPattern::Join { left, right } => Self::Join {
                left: Box::new(left.as_ref().into()),
                right: Box::new(right.as_ref().into()),
            },
            GraphPattern::LeftJoin {
                left,
                right,
                expression,
            } => {
                let empty_expr = if let Expression::Literal(l) = expression {
                    l.datatype() == xsd::BOOLEAN && l.value() == "true"
                } else {
                    false
                };
                Self::LeftJoin {
                    left: Box::new(left.as_ref().into()),
                    right: Box::new(right.as_ref().into()),
                    expression: if empty_expr {
                        None
                    } else {
                        Some(expression.into())
                    },
                }
            }
            #[cfg(feature = "sep-0006")]
            GraphPattern::Lateral { left, right } => Self::Lateral {
                left: Box::new(left.as_ref().into()),
                right: Box::new(right.as_ref().into()),
            },
            GraphPattern::Filter { inner, expression } => Self::Filter {
                inner: Box::new(inner.as_ref().into()),
                expr: expression.into(),
            },
            GraphPattern::Union { inner } => inner
                .iter()
                .map(|c| c.into())
                .reduce(|a, b| Self::Union {
                    left: Box::new(a),
                    right: Box::new(b),
                })
                .unwrap_or_else(|| Self::Values {
                    variables: Vec::new(),
                    bindings: Vec::new(),
                }),
            GraphPattern::Extend {
                inner,
                expression,
                variable,
            } => Self::Extend {
                inner: Box::new(inner.as_ref().into()),
                expression: expression.into(),
                variable: variable.clone(),
            },
            GraphPattern::Minus { left, right } => Self::Minus {
                left: Box::new(left.as_ref().into()),
                right: Box::new(right.as_ref().into()),
            },
            GraphPattern::Values {
                variables,
                bindings,
            } => Self::Values {
                variables: variables.clone(),
                bindings: bindings.clone(),
            },
            GraphPattern::OrderBy { inner, expression } => Self::OrderBy {
                inner: Box::new(inner.as_ref().into()),
                expression: expression.iter().map(|e| e.into()).collect(),
            },
            GraphPattern::Project { inner, variables } => Self::Project {
                inner: Box::new(inner.as_ref().into()),
                variables: variables.clone(),
            },
            GraphPattern::Distinct { inner } => Self::Distinct {
                inner: Box::new(inner.as_ref().into()),
            },
            GraphPattern::Reduced { inner } => Self::Distinct {
                inner: Box::new(inner.as_ref().into()),
            },
            GraphPattern::Slice {
                inner,
                start,
                length,
            } => Self::Slice {
                inner: Box::new(inner.as_ref().into()),
                start: *start,
                length: *length,
            },
            GraphPattern::Group {
                inner,
                variables,
                aggregates,
            } => Self::Group {
                inner: Box::new(inner.as_ref().into()),
                variables: variables.clone(),
                aggregates: aggregates
                    .iter()
                    .map(|(var, expr)| (var.clone(), expr.into()))
                    .collect(),
            },
            GraphPattern::Service {
                inner,
                name,
                silent,
            } => Self::Service {
                inner: Box::new(inner.as_ref().into()),
                name: name.clone(),
                silent: *silent,
            },
        }
    }
}

/// A set function used in aggregates (c.f. [`GraphPattern::Group`]).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum AggregateExpression {
    /// [Count](https://www.w3.org/TR/sparql11-query/#defn_aggCount).
    Count {
        expr: Option<Box<Expression>>,
        distinct: bool,
    },
    /// [Sum](https://www.w3.org/TR/sparql11-query/#defn_aggSum).
    Sum {
        expr: Box<Expression>,
        distinct: bool,
    },
    /// [Avg](https://www.w3.org/TR/sparql11-query/#defn_aggAvg).
    Avg {
        expr: Box<Expression>,
        distinct: bool,
    },
    /// [Min](https://www.w3.org/TR/sparql11-query/#defn_aggMin).
    Min {
        expr: Box<Expression>,
        distinct: bool,
    },
    /// [Max](https://www.w3.org/TR/sparql11-query/#defn_aggMax).
    Max {
        expr: Box<Expression>,
        distinct: bool,
    },
    /// [GroupConcat](https://www.w3.org/TR/sparql11-query/#defn_aggGroupConcat).
    GroupConcat {
        expr: Box<Expression>,
        distinct: bool,
        separator: Option<String>,
    },
    /// [Sample](https://www.w3.org/TR/sparql11-query/#defn_aggSample).
    Sample {
        expr: Box<Expression>,
        distinct: bool,
    },
    /// Custom function.
    Custom {
        name: NamedNode,
        expr: Box<Expression>,
        distinct: bool,
    },
}

impl AggregateExpression {
    fn from_sparql_algebra(
        expression: &AlAggregateExpression,
        graph_name: Option<&NamedNodePattern>,
    ) -> Self {
        match expression {
            AlAggregateExpression::Count { expr, distinct } => Self::Count {
                expr: expr
                    .as_ref()
                    .map(|e| Box::new(Expression::from_sparql_algebra(e, graph_name))),
                distinct: *distinct,
            },
            AlAggregateExpression::Sum { expr, distinct } => Self::Sum {
                expr: Box::new(Expression::from_sparql_algebra(expr, graph_name)),
                distinct: *distinct,
            },
            AlAggregateExpression::Avg { expr, distinct } => Self::Avg {
                expr: Box::new(Expression::from_sparql_algebra(expr, graph_name)),
                distinct: *distinct,
            },
            AlAggregateExpression::Min { expr, distinct } => Self::Min {
                expr: Box::new(Expression::from_sparql_algebra(expr, graph_name)),
                distinct: *distinct,
            },
            AlAggregateExpression::Max { expr, distinct } => Self::Max {
                expr: Box::new(Expression::from_sparql_algebra(expr, graph_name)),
                distinct: *distinct,
            },
            AlAggregateExpression::GroupConcat {
                expr,
                distinct,
                separator,
            } => Self::GroupConcat {
                expr: Box::new(Expression::from_sparql_algebra(expr, graph_name)),
                distinct: *distinct,
                separator: separator.clone(),
            },
            AlAggregateExpression::Sample { expr, distinct } => Self::Sample {
                expr: Box::new(Expression::from_sparql_algebra(expr, graph_name)),
                distinct: *distinct,
            },
            AlAggregateExpression::Custom {
                name,
                expr,
                distinct,
            } => Self::Custom {
                name: name.clone(),
                expr: Box::new(Expression::from_sparql_algebra(expr, graph_name)),
                distinct: *distinct,
            },
        }
    }
}

impl From<&AggregateExpression> for AlAggregateExpression {
    fn from(expression: &AggregateExpression) -> Self {
        match expression {
            AggregateExpression::Count { expr, distinct } => Self::Count {
                expr: expr.as_ref().map(|e| Box::new(e.as_ref().into())),
                distinct: *distinct,
            },
            AggregateExpression::Sum { expr, distinct } => Self::Sum {
                expr: Box::new(expr.as_ref().into()),
                distinct: *distinct,
            },
            AggregateExpression::Avg { expr, distinct } => Self::Avg {
                expr: Box::new(expr.as_ref().into()),
                distinct: *distinct,
            },
            AggregateExpression::Min { expr, distinct } => Self::Min {
                expr: Box::new(expr.as_ref().into()),
                distinct: *distinct,
            },
            AggregateExpression::Max { expr, distinct } => Self::Max {
                expr: Box::new(expr.as_ref().into()),
                distinct: *distinct,
            },
            AggregateExpression::GroupConcat {
                expr,
                distinct,
                separator,
            } => Self::GroupConcat {
                expr: Box::new(expr.as_ref().into()),
                distinct: *distinct,
                separator: separator.clone(),
            },
            AggregateExpression::Sample { expr, distinct } => Self::Sample {
                expr: Box::new(expr.as_ref().into()),
                distinct: *distinct,
            },
            AggregateExpression::Custom {
                name,
                expr,
                distinct,
            } => Self::Custom {
                name: name.clone(),
                expr: Box::new(expr.as_ref().into()),
                distinct: *distinct,
            },
        }
    }
}

/// An ordering comparator used by [`GraphPattern::OrderBy`].
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum OrderExpression {
    /// Ascending order
    Asc(Expression),
    /// Descending order
    Desc(Expression),
}

impl OrderExpression {
    fn from_sparql_algebra(
        expression: &AlOrderExpression,
        graph_name: Option<&NamedNodePattern>,
    ) -> Self {
        match expression {
            AlOrderExpression::Asc(e) => Self::Asc(Expression::from_sparql_algebra(e, graph_name)),
            AlOrderExpression::Desc(e) => {
                Self::Desc(Expression::from_sparql_algebra(e, graph_name))
            }
        }
    }
}

impl From<&OrderExpression> for AlOrderExpression {
    fn from(expression: &OrderExpression) -> Self {
        match expression {
            OrderExpression::Asc(e) => Self::Asc(e.into()),
            OrderExpression::Desc(e) => Self::Desc(e.into()),
        }
    }
}

fn new_var() -> Variable {
    Variable::new_unchecked(format!("{:x}", random::<u128>()))
}
