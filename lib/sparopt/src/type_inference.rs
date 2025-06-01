use crate::algebra::{Expression, GraphPattern};
use oxrdf::Variable;
use spargebra::algebra::Function;
use spargebra::term::{GroundTerm, GroundTermPattern, NamedNodePattern};
use std::collections::HashMap;
use std::ops::{BitAnd, BitOr};

pub fn infer_graph_pattern_types(
    pattern: &GraphPattern,
    mut types: VariableTypes,
) -> VariableTypes {
    match pattern {
        GraphPattern::QuadPattern {
            subject,
            predicate,
            object,
            graph_name,
        } => {
            add_ground_term_pattern_types(subject, &mut types, false);
            if let NamedNodePattern::Variable(v) = predicate {
                types.intersect_variable_with(v.clone(), VariableType::NAMED_NODE)
            }
            add_ground_term_pattern_types(object, &mut types, true);
            if let Some(NamedNodePattern::Variable(v)) = graph_name {
                types.intersect_variable_with(v.clone(), VariableType::NAMED_NODE)
            }
            types
        }
        GraphPattern::Path {
            subject,
            object,
            graph_name,
            ..
        } => {
            add_ground_term_pattern_types(subject, &mut types, false);
            add_ground_term_pattern_types(object, &mut types, true);
            if let Some(NamedNodePattern::Variable(v)) = graph_name {
                types.intersect_variable_with(v.clone(), VariableType::NAMED_NODE)
            }
            types
        }
        GraphPattern::Graph { graph_name } => {
            if let NamedNodePattern::Variable(v) = graph_name {
                types.intersect_variable_with(v.clone(), VariableType::NAMED_NODE)
            }
            types
        }
        GraphPattern::Join { left, right, .. } => {
            let mut output_types = infer_graph_pattern_types(left, types.clone());
            output_types.intersect_with(infer_graph_pattern_types(right, types));
            output_types
        }
        #[cfg(feature = "sep-0006")]
        GraphPattern::Lateral { left, right } => {
            infer_graph_pattern_types(right, infer_graph_pattern_types(left, types))
        }
        GraphPattern::LeftJoin { left, right, .. } => {
            let mut right_types = infer_graph_pattern_types(right, types.clone()); // TODO: expression
            for t in right_types.inner.values_mut() {
                t.undef = true; // Right might be unset
            }
            let mut output_types = infer_graph_pattern_types(left, types);
            output_types.intersect_with(right_types);
            output_types
        }
        GraphPattern::Minus { left, .. } => infer_graph_pattern_types(left, types),
        GraphPattern::Union { inner } => inner
            .iter()
            .map(|inner| infer_graph_pattern_types(inner, types.clone()))
            .reduce(|mut a, b| {
                a.union_with(b);
                a
            })
            .unwrap_or_default(),
        GraphPattern::Extend {
            inner,
            variable,
            expression,
        } => {
            let mut types = infer_graph_pattern_types(inner, types);
            types.intersect_variable_with(
                variable.clone(),
                infer_expression_type(expression, &types),
            );
            types
        }
        GraphPattern::Filter { inner, .. } => infer_graph_pattern_types(inner, types),
        GraphPattern::Project { inner, variables } => VariableTypes {
            inner: infer_graph_pattern_types(inner, types)
                .inner
                .into_iter()
                .filter(|(v, _)| variables.contains(v))
                .collect(),
        },
        GraphPattern::Distinct { inner }
        | GraphPattern::Reduced { inner }
        | GraphPattern::OrderBy { inner, .. }
        | GraphPattern::Slice { inner, .. } => infer_graph_pattern_types(inner, types),
        GraphPattern::Group {
            inner,
            variables,
            aggregates,
        } => {
            let types = infer_graph_pattern_types(inner, types);
            VariableTypes {
                inner: infer_graph_pattern_types(inner, types)
                    .inner
                    .into_iter()
                    .filter(|(v, _)| variables.contains(v))
                    .chain(aggregates.iter().map(|(v, _)| (v.clone(), VariableType::ANY))) //TODO: guess from aggregate
                    .collect(),
            }
        }
        GraphPattern::Values {
            variables,
            bindings,
        } => {
            for (i, v) in variables.iter().enumerate() {
                let mut t = VariableType::default();
                for binding in bindings {
                    match binding[i] {
                        Some(GroundTerm::NamedNode(_)) => t.named_node = true,
                        Some(GroundTerm::Literal(_)) => t.literal = true,
                        #[cfg(feature = "sparql-12")]
                        Some(GroundTerm::Triple(_)) => t.triple = true,
                        None => t.undef = true,
                    }
                }
                types.intersect_variable_with(v.clone(), t)
            }
            types
        }
        GraphPattern::Service {
            name,
            inner,
            silent,
        } => {
            let parent_types = types.clone();
            let mut types = infer_graph_pattern_types(inner, types);
            if *silent {
                // On failure, single empty solution
                types.union_with(parent_types);
            } else if let NamedNodePattern::Variable(v) = name {
                types.intersect_variable_with(v.clone(), VariableType::NAMED_NODE)
            }
            types
        }
    }
}

fn add_ground_term_pattern_types(
    pattern: &GroundTermPattern,
    types: &mut VariableTypes,
    is_object: bool,
) {
    if let GroundTermPattern::Variable(v) = pattern {
        types.intersect_variable_with(
            v.clone(),
            if is_object {
                VariableType::TERM
            } else {
                VariableType::SUBJECT
            },
        )
    }
    #[cfg(feature = "sparql-12")]
    if let GroundTermPattern::Triple(t) = pattern {
        add_ground_term_pattern_types(&t.subject, types, false);
        if let NamedNodePattern::Variable(v) = &t.predicate {
            types.intersect_variable_with(v.clone(), VariableType::NAMED_NODE)
        }
        add_ground_term_pattern_types(&t.object, types, true);
    }
}

pub fn infer_expression_type(expression: &Expression, types: &VariableTypes) -> VariableType {
    match expression {
        Expression::NamedNode(_) => VariableType::NAMED_NODE,
        Expression::Literal(_) | Expression::Exists(_) | Expression::Bound(_) => {
            VariableType::LITERAL
        }
        Expression::Variable(v) => types.get(v),
        Expression::FunctionCall(Function::Datatype | Function::Iri, _) => {
            VariableType::NAMED_NODE | VariableType::UNDEF
        }
        #[cfg(feature = "sparql-12")]
        Expression::FunctionCall(Function::Predicate, _) => {
            VariableType::NAMED_NODE | VariableType::UNDEF
        }
        Expression::FunctionCall(Function::BNode, args) => {
            if args.is_empty() {
                VariableType::BLANK_NODE
            } else {
                VariableType::BLANK_NODE | VariableType::UNDEF
            }
        }
        Expression::FunctionCall(
            Function::Rand | Function::Now | Function::Uuid | Function::StrUuid,
            _,
        ) => VariableType::LITERAL,
        Expression::Or(_)
        | Expression::And(_)
        | Expression::Equal(_, _)
        | Expression::Greater(_, _)
        | Expression::GreaterOrEqual(_, _)
        | Expression::Less(_, _)
        | Expression::LessOrEqual(_, _)
        | Expression::Add(_, _)
        | Expression::Subtract(_, _)
        | Expression::Multiply(_, _)
        | Expression::Divide(_, _)
        | Expression::UnaryPlus(_)
        | Expression::UnaryMinus(_)
        | Expression::Not(_)
        | Expression::FunctionCall(
            Function::Str
            | Function::Lang
            | Function::LangMatches
            | Function::Abs
            | Function::Ceil
            | Function::Floor
            | Function::Round
            | Function::Concat
            | Function::SubStr
            | Function::StrLen
            | Function::Replace
            | Function::UCase
            | Function::LCase
            | Function::EncodeForUri
            | Function::Contains
            | Function::StrStarts
            | Function::StrEnds
            | Function::StrBefore
            | Function::StrAfter
            | Function::Year
            | Function::Month
            | Function::Day
            | Function::Hours
            | Function::Minutes
            | Function::Seconds
            | Function::Timezone
            | Function::Tz
            | Function::Md5
            | Function::Sha1
            | Function::Sha256
            | Function::Sha384
            | Function::Sha512
            | Function::StrLang
            | Function::StrDt
            | Function::IsIri
            | Function::IsBlank
            | Function::IsLiteral
            | Function::IsNumeric
            | Function::Regex,
            _,
        ) => VariableType::LITERAL | VariableType::UNDEF,
        #[cfg(feature = "sparql-12")]
        Expression::FunctionCall(
            Function::LangDir | Function::StrLangDir | Function::HasLang | Function::HasLangDir,
            _,
        ) => VariableType::LITERAL | VariableType::UNDEF,
        #[cfg(feature = "sep-0002")]
        Expression::FunctionCall(Function::Adjust, _) => {
            VariableType::LITERAL | VariableType::UNDEF
        }
        #[cfg(feature = "sparql-12")]
        Expression::FunctionCall(Function::IsTriple, _) => {
            VariableType::LITERAL | VariableType::UNDEF
        }
        Expression::SameTerm(left, right) => {
            if infer_expression_type(left, types).undef || infer_expression_type(right, types).undef
            {
                VariableType::LITERAL | VariableType::UNDEF
            } else {
                VariableType::LITERAL
            }
        }
        Expression::If(_, then, els) => {
            infer_expression_type(then, types) | infer_expression_type(els, types)
        }
        Expression::Coalesce(inner) => {
            let mut t = VariableType::UNDEF;
            for e in inner {
                let new = infer_expression_type(e, types);
                t = t | new;
                if !new.undef {
                    t.undef = false;
                    return t;
                }
            }
            t
        }
        #[cfg(feature = "sparql-12")]
        Expression::FunctionCall(Function::Triple, _) => VariableType::TRIPLE | VariableType::UNDEF,
        #[cfg(feature = "sparql-12")]
        Expression::FunctionCall(Function::Subject, _) => {
            VariableType::SUBJECT | VariableType::UNDEF
        }
        #[cfg(feature = "sparql-12")]
        Expression::FunctionCall(Function::Object, _) => VariableType::TERM | VariableType::UNDEF,
        Expression::FunctionCall(Function::Custom(_), _) => VariableType::ANY,
    }
}

#[derive(Default, Clone, Debug)]
pub struct VariableTypes {
    inner: HashMap<Variable, VariableType>,
}

impl VariableTypes {
    pub fn get(&self, variable: &Variable) -> VariableType {
        self.inner
            .get(variable)
            .copied()
            .unwrap_or(VariableType::UNDEF)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Variable, &VariableType)> {
        self.inner.iter()
    }

    pub fn intersect_with(&mut self, other: Self) {
        for (v, t) in other.inner {
            self.intersect_variable_with(v, t);
        }
    }

    pub fn union_with(&mut self, other: Self) {
        for (v, t) in &mut self.inner {
            if other.get(v).undef {
                t.undef = true; // Might be undefined
            }
        }
        for (v, mut t) in other.inner {
            self.inner
                .entry(v)
                .and_modify(|ex| *ex = *ex | t)
                .or_insert({
                    t.undef = true;
                    t
                });
        }
    }

    fn intersect_variable_with(&mut self, variable: Variable, t: VariableType) {
        let t = self.get(&variable) & t;
        if t != VariableType::UNDEF {
            self.inner.insert(variable, t);
        }
    }
}

#[expect(clippy::struct_excessive_bools)]
#[derive(Clone, Copy, Eq, PartialEq, Debug, Default)]
pub struct VariableType {
    pub undef: bool,
    pub named_node: bool,
    pub blank_node: bool,
    pub literal: bool,
    #[cfg(feature = "sparql-12")]
    pub triple: bool,
}

impl VariableType {
    const ANY: Self = Self {
        undef: true,
        named_node: true,
        blank_node: true,
        literal: true,
        #[cfg(feature = "sparql-12")]
        triple: true,
    };
    const BLANK_NODE: Self = Self {
        undef: false,
        named_node: false,
        blank_node: true,
        literal: false,
        #[cfg(feature = "sparql-12")]
        triple: false,
    };
    const LITERAL: Self = Self {
        undef: false,
        named_node: false,
        blank_node: false,
        literal: true,
        #[cfg(feature = "sparql-12")]
        triple: false,
    };
    const NAMED_NODE: Self = Self {
        undef: false,
        named_node: true,
        blank_node: false,
        literal: false,
        #[cfg(feature = "sparql-12")]
        triple: false,
    };
    const SUBJECT: Self = Self {
        undef: false,
        named_node: true,
        blank_node: true,
        literal: false,
        #[cfg(feature = "sparql-12")]
        triple: true,
    };
    const TERM: Self = Self {
        undef: false,
        named_node: true,
        blank_node: true,
        literal: true,
        #[cfg(feature = "sparql-12")]
        triple: true,
    };
    #[cfg(feature = "sparql-12")]
    const TRIPLE: Self = Self {
        undef: false,
        named_node: false,
        blank_node: false,
        literal: false,
        triple: true,
    };
    pub const UNDEF: Self = Self {
        undef: true,
        named_node: false,
        blank_node: false,
        literal: false,
        #[cfg(feature = "sparql-12")]
        triple: false,
    };
}

impl BitOr for VariableType {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self {
        Self {
            undef: self.undef || rhs.undef,
            named_node: self.named_node || rhs.named_node,
            blank_node: self.blank_node || rhs.blank_node,
            literal: self.literal || rhs.literal,
            #[cfg(feature = "sparql-12")]
            triple: self.triple || rhs.triple,
        }
    }
}

impl BitAnd for VariableType {
    type Output = Self;

    #[expect(clippy::nonminimal_bool)]
    fn bitand(self, rhs: Self) -> Self {
        Self {
            undef: self.undef && rhs.undef,
            named_node: self.named_node && rhs.named_node
                || (self.undef && rhs.named_node)
                || (self.named_node && rhs.undef),
            blank_node: self.blank_node && rhs.blank_node
                || (self.undef && rhs.blank_node)
                || (self.blank_node && rhs.undef),
            literal: self.literal && rhs.literal
                || (self.undef && rhs.literal)
                || (self.literal && rhs.undef),
            #[cfg(feature = "sparql-12")]
            triple: self.triple && rhs.triple
                || (self.undef && rhs.triple)
                || (self.triple && rhs.undef),
        }
    }
}
