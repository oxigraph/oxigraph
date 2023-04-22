//! Reasoning via query rewriting

use crate::algebra::*;
use rand::random;
use spargebra::term::GroundTriplePattern;
use spargebra::RuleSet;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

pub struct QueryRewriter {
    rules: Vec<(Vec<GroundTriplePattern>, Vec<GroundTriplePattern>)>,
}

impl QueryRewriter {
    pub fn new(rule_set: RuleSet) -> Self {
        Self {
            rules: rule_set
                .rules
                .into_iter()
                .map(|rule| {
                    (rule.head, {
                        let mut blank_nodes = HashMap::new();
                        rule.body
                            .iter()
                            .map(|p| {
                                let (subject, predicate, object) =
                                    GraphPattern::triple_pattern_from_algebra(p, &mut blank_nodes);
                                GroundTriplePattern {
                                    subject,
                                    predicate,
                                    object,
                                }
                            })
                            .collect()
                    })
                })
                .collect(),
        }
    }

    pub fn rewrite_graph_pattern(&self, pattern: &GraphPattern) -> GraphPattern {
        match pattern {
            GraphPattern::QuadPattern {
                subject,
                predicate,
                object,
                graph_name,
            } => self
                .rewrite_quad_pattern(
                    subject,
                    predicate,
                    object,
                    graph_name.as_ref(),
                    &mut Vec::new(),
                )
                .try_into()
                .unwrap(),
            GraphPattern::Path {
                subject,
                path,
                object,
                graph_name,
            } => {
                let inner = self
                    .rewrite_property_path(subject, path, object, graph_name.as_ref(), 0)
                    .try_into()
                    .unwrap();
                if matches!(
                    inner,
                    GraphPattern::FixedPoint { .. } | GraphPattern::Distinct { .. }
                ) {
                    inner
                } else {
                    GraphPattern::distinct(inner) // We make sure we only return distinct results
                }
            }
            GraphPattern::Join { left, right } => GraphPattern::join(
                self.rewrite_graph_pattern(left),
                self.rewrite_graph_pattern(right),
            ),
            GraphPattern::LeftJoin {
                left,
                right,
                expression,
            } => GraphPattern::left_join(
                self.rewrite_graph_pattern(left),
                self.rewrite_graph_pattern(right),
                expression.clone(),
            ),
            #[cfg(feature = "sep-0006")]
            GraphPattern::Lateral { left, right } => GraphPattern::lateral(
                self.rewrite_graph_pattern(left),
                self.rewrite_graph_pattern(right),
            ),
            GraphPattern::Filter { inner, expression } => GraphPattern::filter(
                self.rewrite_graph_pattern(inner),
                self.rewrite_expression(expression),
            ),
            GraphPattern::Union { inner } => inner
                .iter()
                .map(|p| self.rewrite_graph_pattern(p))
                .reduce(GraphPattern::union)
                .unwrap_or_else(GraphPattern::empty),
            GraphPattern::Extend {
                inner,
                variable,
                expression,
            } => GraphPattern::extend(
                self.rewrite_graph_pattern(inner),
                variable.clone(),
                self.rewrite_expression(expression),
            ),
            GraphPattern::Minus { left, right } => GraphPattern::minus(
                self.rewrite_graph_pattern(left),
                self.rewrite_graph_pattern(right),
            ),
            GraphPattern::Values {
                variables,
                bindings,
            } => GraphPattern::values(variables.clone(), bindings.clone()),
            GraphPattern::OrderBy { inner, expression } => GraphPattern::order_by(
                self.rewrite_graph_pattern(inner),
                expression
                    .iter()
                    .map(|e| match e {
                        OrderExpression::Asc(e) => OrderExpression::Asc(self.rewrite_expression(e)),
                        OrderExpression::Desc(e) => {
                            OrderExpression::Desc(self.rewrite_expression(e))
                        }
                    })
                    .collect(),
            ),
            GraphPattern::Project { inner, variables } => {
                GraphPattern::project(self.rewrite_graph_pattern(inner), variables.clone())
            }
            GraphPattern::Distinct { inner } => {
                GraphPattern::distinct(self.rewrite_graph_pattern(inner))
            }
            GraphPattern::Reduced { inner } => {
                GraphPattern::reduced(self.rewrite_graph_pattern(inner))
            }
            GraphPattern::Slice {
                inner,
                start,
                length,
            } => GraphPattern::slice(self.rewrite_graph_pattern(inner), *start, *length),
            GraphPattern::Group {
                inner,
                variables,
                aggregates,
            } => GraphPattern::group(
                self.rewrite_graph_pattern(inner),
                variables.clone(),
                aggregates
                    .iter()
                    .map(|(v, e)| {
                        (
                            v.clone(),
                            match e {
                                AggregateExpression::Count { expr, distinct } => {
                                    AggregateExpression::Count {
                                        expr: expr
                                            .as_ref()
                                            .map(|e| Box::new(self.rewrite_expression(e))),
                                        distinct: *distinct,
                                    }
                                }
                                AggregateExpression::Sum { expr, distinct } => {
                                    AggregateExpression::Sum {
                                        expr: Box::new(self.rewrite_expression(expr)),
                                        distinct: *distinct,
                                    }
                                }
                                AggregateExpression::Min { expr, distinct } => {
                                    AggregateExpression::Min {
                                        expr: Box::new(self.rewrite_expression(expr)),
                                        distinct: *distinct,
                                    }
                                }
                                AggregateExpression::Max { expr, distinct } => {
                                    AggregateExpression::Max {
                                        expr: Box::new(self.rewrite_expression(expr)),
                                        distinct: *distinct,
                                    }
                                }
                                AggregateExpression::Avg { expr, distinct } => {
                                    AggregateExpression::Avg {
                                        expr: Box::new(self.rewrite_expression(expr)),
                                        distinct: *distinct,
                                    }
                                }
                                AggregateExpression::Sample { expr, distinct } => {
                                    AggregateExpression::Sample {
                                        expr: Box::new(self.rewrite_expression(expr)),
                                        distinct: *distinct,
                                    }
                                }
                                AggregateExpression::GroupConcat {
                                    expr,
                                    distinct,
                                    separator,
                                } => AggregateExpression::GroupConcat {
                                    expr: Box::new(self.rewrite_expression(expr)),
                                    distinct: *distinct,
                                    separator: separator.clone(),
                                },
                                AggregateExpression::Custom {
                                    name,
                                    expr,
                                    distinct,
                                } => AggregateExpression::Custom {
                                    name: name.clone(),
                                    expr: Box::new(self.rewrite_expression(expr)),
                                    distinct: *distinct,
                                },
                            },
                        )
                    })
                    .collect(),
            ),
            GraphPattern::Service {
                inner,
                silent,
                name,
            } => GraphPattern::service(self.rewrite_graph_pattern(inner), name.clone(), *silent),
            GraphPattern::FixedPoint { .. } => unreachable!(),
        }
    }

    fn rewrite_expression(&self, expression: &Expression) -> Expression {
        match expression {
            Expression::NamedNode(node) => node.clone().into(),
            Expression::Literal(literal) => literal.clone().into(),
            Expression::Variable(variable) => variable.clone().into(),
            Expression::Or(left, right) => Expression::or(
                self.rewrite_expression(left),
                self.rewrite_expression(right),
            ),
            Expression::And(left, right) => Expression::and(
                self.rewrite_expression(left),
                self.rewrite_expression(right),
            ),
            Expression::Equal(left, right) => Expression::equal(
                self.rewrite_expression(left),
                self.rewrite_expression(right),
            ),
            Expression::SameTerm(left, right) => Expression::same_term(
                self.rewrite_expression(left),
                self.rewrite_expression(right),
            ),
            Expression::Greater(left, right) => Expression::greater(
                self.rewrite_expression(left),
                self.rewrite_expression(right),
            ),
            Expression::GreaterOrEqual(left, right) => Expression::greater_or_equal(
                self.rewrite_expression(left),
                self.rewrite_expression(right),
            ),
            Expression::Less(left, right) => Expression::less(
                self.rewrite_expression(left),
                self.rewrite_expression(right),
            ),
            Expression::LessOrEqual(left, right) => Expression::less_or_equal(
                self.rewrite_expression(left),
                self.rewrite_expression(right),
            ),
            Expression::Add(left, right) => Expression::add(
                self.rewrite_expression(left),
                self.rewrite_expression(right),
            ),
            Expression::Subtract(left, right) => Expression::subtract(
                self.rewrite_expression(left),
                self.rewrite_expression(right),
            ),
            Expression::Multiply(left, right) => Expression::multiply(
                self.rewrite_expression(left),
                self.rewrite_expression(right),
            ),
            Expression::Divide(left, right) => Expression::divide(
                self.rewrite_expression(left),
                self.rewrite_expression(right),
            ),
            Expression::UnaryPlus(inner) => Expression::unary_plus(self.rewrite_expression(inner)),
            Expression::UnaryMinus(inner) => {
                Expression::unary_minus(self.rewrite_expression(inner))
            }
            Expression::Not(inner) => Expression::not(self.rewrite_expression(inner)),
            Expression::Exists(inner) => Expression::exists(self.rewrite_graph_pattern(inner)),
            Expression::Bound(variable) => Expression::Bound(variable.clone()),
            Expression::If(cond, then, els) => Expression::if_cond(
                self.rewrite_expression(cond),
                self.rewrite_expression(then),
                self.rewrite_expression(els),
            ),
            Expression::Coalesce(inners) => {
                Expression::coalesce(inners.iter().map(|a| self.rewrite_expression(a)).collect())
            }
            Expression::FunctionCall(name, args) => Expression::call(
                name.clone(),
                args.iter().map(|a| self.rewrite_expression(a)).collect(),
            ),
        }
    }

    fn rewrite_quad_pattern(
        &self,
        subject: &GroundTermPattern,
        predicate: &NamedNodePattern,
        object: &GroundTermPattern,
        graph_name: Option<&NamedNodePattern>,
        possible_fixed_points: &mut Vec<(
            GroundTermPattern,
            NamedNodePattern,
            GroundTermPattern,
            Option<NamedNodePattern>,
            FixedPointId,
        )>,
    ) -> FixedPointGraphPattern {
        // We check if we are in a loop
        for (
            fixed_point_subject,
            fixed_point_predicate,
            fixed_point_object,
            fixed_point_graph_name,
            fixed_point_id,
        ) in possible_fixed_points.iter()
        {
            let mut variable_mapping = Vec::new();
            if let (GroundTermPattern::Variable(from), GroundTermPattern::Variable(to)) =
                (fixed_point_subject, subject)
            {
                variable_mapping.push((from.clone(), to.clone()));
            } else if fixed_point_subject == subject {
                // Ok
            } else {
                continue; // Not compatible
            }
            if let (NamedNodePattern::Variable(from), NamedNodePattern::Variable(to)) =
                (fixed_point_predicate, predicate)
            {
                variable_mapping.push((from.clone(), to.clone()));
            } else if fixed_point_predicate == predicate {
                // Ok
            } else {
                continue; // Not compatible
            }
            if let (GroundTermPattern::Variable(from), GroundTermPattern::Variable(to)) =
                (fixed_point_object, object)
            {
                variable_mapping.push((from.clone(), to.clone()));
            } else if fixed_point_object == object {
                // Ok
            } else {
                continue; // Not compatible
            }
            if let (Some(NamedNodePattern::Variable(from)), Some(NamedNodePattern::Variable(to))) =
                (fixed_point_graph_name, graph_name)
            {
                variable_mapping.push((from.clone(), to.clone()));
            } else if fixed_point_graph_name.as_ref() == graph_name {
                // Ok
            } else {
                continue; // Not compatible
            }
            let mut plan = FixedPointGraphPattern::FixedPointEntry(*fixed_point_id);
            for (from, to) in &variable_mapping {
                plan = Self::copy_variable(plan, from.clone().into(), to.clone());
            }
            return FixedPointGraphPattern::project(
                plan,
                variable_mapping.into_iter().map(|(_, v)| v).collect(),
            );
        }

        let new_fixed_point_id = FixedPointId(1_000_000 + possible_fixed_points.len());
        possible_fixed_points.push((
            subject.clone(),
            predicate.clone(),
            object.clone(),
            graph_name.cloned(),
            new_fixed_point_id,
        ));

        // We get the output variables list:
        let mut output_variables = Vec::new();
        Self::add_pattern_variables(subject, &mut output_variables);
        if let NamedNodePattern::Variable(v) = predicate {
            output_variables.push(v.clone());
        }
        Self::add_pattern_variables(object, &mut output_variables);
        if let Some(NamedNodePattern::Variable(v)) = graph_name {
            output_variables.push(v.clone());
        }

        // We rewrite based on rules
        let mut pattern = FixedPointGraphPattern::QuadPattern {
            subject: subject.clone(),
            predicate: predicate.clone(),
            object: object.clone(),
            graph_name: graph_name.cloned(),
        };
        for (rule_head, rule_body) in &self.rules {
            for head_pattern in rule_head {
                if let Some(nested) = self.apply_rule_on_quad_pattern(
                    subject,
                    predicate,
                    object,
                    graph_name,
                    head_pattern,
                    rule_body,
                    possible_fixed_points,
                ) {
                    pattern = FixedPointGraphPattern::union(
                        pattern,
                        FixedPointGraphPattern::project(nested, output_variables.clone()),
                    );
                }
            }
        }
        possible_fixed_points.pop();
        FixedPointGraphPattern::fixed_point(new_fixed_point_id, pattern, output_variables)
    }

    fn rewrite_property_path(
        &self,
        subject: &GroundTermPattern,
        path: &PropertyPathExpression,
        object: &GroundTermPattern,
        graph_name: Option<&NamedNodePattern>,
        fix_point_counter: usize,
    ) -> FixedPointGraphPattern {
        match path {
            PropertyPathExpression::NamedNode(p) => self.rewrite_quad_pattern(
                subject,
                &p.clone().into(),
                object,
                graph_name,
                &mut Vec::new(),
            ),
            PropertyPathExpression::Reverse(p) => {
                self.rewrite_property_path(object, p, subject, graph_name, fix_point_counter)
            }
            PropertyPathExpression::Sequence(left, right) => {
                let mut final_variables = Vec::new();
                Self::add_pattern_variables(subject, &mut final_variables);
                Self::add_pattern_variables(object, &mut final_variables);
                if let Some(NamedNodePattern::Variable(v)) = graph_name {
                    final_variables.push(v.clone());
                }
                let middle = new_var();
                FixedPointGraphPattern::project(
                    FixedPointGraphPattern::join(
                        self.rewrite_property_path(
                            subject,
                            left,
                            &middle.clone().into(),
                            graph_name,
                            fix_point_counter,
                        ),
                        self.rewrite_property_path(
                            &middle.into(),
                            right,
                            object,
                            graph_name,
                            fix_point_counter,
                        ),
                    ),
                    final_variables,
                )
            }
            PropertyPathExpression::Alternative(left, right) => FixedPointGraphPattern::union(
                self.rewrite_property_path(subject, left, object, graph_name, fix_point_counter),
                self.rewrite_property_path(subject, right, object, graph_name, fix_point_counter),
            ),
            PropertyPathExpression::ZeroOrOne(p) => FixedPointGraphPattern::union(
                self.zero_graph_pattern(subject, object, graph_name),
                self.rewrite_property_path(subject, p, object, graph_name, fix_point_counter),
            ),
            PropertyPathExpression::ZeroOrMore(p) => FixedPointGraphPattern::union(
                self.zero_graph_pattern(subject, object, graph_name),
                self.one_or_more_pattern(subject, p, object, graph_name, fix_point_counter),
            ),
            PropertyPathExpression::OneOrMore(p) => {
                self.one_or_more_pattern(subject, p, object, graph_name, fix_point_counter)
            }
            PropertyPathExpression::NegatedPropertySet(p) => {
                let var = new_var();
                FixedPointGraphPattern::filter(
                    self.rewrite_quad_pattern(
                        subject,
                        &var.clone().into(),
                        object,
                        graph_name,
                        &mut Vec::new(),
                    ),
                    FixedPointExpression::not(
                        p.iter()
                            .map(|p| {
                                FixedPointExpression::same_term(
                                    var.clone().into(),
                                    p.clone().into(),
                                )
                            })
                            .reduce(FixedPointExpression::or)
                            .unwrap_or_else(|| false.into()),
                    ),
                )
            }
        }
    }

    fn zero_graph_pattern(
        &self,
        subject: &GroundTermPattern,
        object: &GroundTermPattern,
        graph_name: Option<&NamedNodePattern>,
    ) -> FixedPointGraphPattern {
        //TODO: FixedPointGraphPattern::values check existence
        match subject {
            GroundTermPattern::NamedNode(subject) => match object {
                GroundTermPattern::NamedNode(object) if subject == object => {
                    FixedPointGraphPattern::singleton()
                }
                GroundTermPattern::Variable(object) => FixedPointGraphPattern::values(
                    vec![object.clone()],
                    vec![vec![Some(subject.clone().into())]],
                ),
                _ => FixedPointGraphPattern::empty(),
            },
            GroundTermPattern::Literal(subject) => match object {
                GroundTermPattern::Literal(object) if subject == object => {
                    FixedPointGraphPattern::singleton()
                }
                GroundTermPattern::Variable(object) => FixedPointGraphPattern::values(
                    vec![object.clone()],
                    vec![vec![Some(subject.clone().into())]],
                ),
                _ => FixedPointGraphPattern::empty(),
            },
            GroundTermPattern::Triple(_) => {
                let new_var = new_var();
                let mut output_variables = Vec::new();
                Self::add_pattern_variables(subject, &mut output_variables);
                Self::add_pattern_variables(object, &mut output_variables);
                FixedPointGraphPattern::project(
                    Self::pattern_mapping(
                        self.zero_graph_pattern(
                            &GroundTermPattern::Variable(new_var.clone()),
                            object,
                            graph_name,
                        ),
                        new_var.into(),
                        subject.clone(),
                    ),
                    output_variables,
                )
            }
            parent_subject @ GroundTermPattern::Variable(subject) => match object {
                GroundTermPattern::NamedNode(object) => FixedPointGraphPattern::values(
                    vec![subject.clone()],
                    vec![vec![Some(object.clone().into())]],
                ),
                GroundTermPattern::Literal(object) => FixedPointGraphPattern::values(
                    vec![subject.clone()],
                    vec![vec![Some(object.clone().into())]],
                ),
                GroundTermPattern::Triple(_) => {
                    let new_var = new_var();
                    let mut output_variables = Vec::new();
                    Self::add_pattern_variables(parent_subject, &mut output_variables);
                    Self::add_pattern_variables(object, &mut output_variables);
                    FixedPointGraphPattern::project(
                        Self::pattern_mapping(
                            self.zero_graph_pattern(
                                &GroundTermPattern::Variable(new_var.clone()),
                                parent_subject,
                                graph_name,
                            ),
                            new_var.into(),
                            object.clone(),
                        ),
                        output_variables,
                    )
                }
                GroundTermPattern::Variable(object) => {
                    let s = new_var();
                    let p = new_var();
                    let o = new_var();
                    let mut final_variables = vec![subject.clone(), object.clone()];
                    if let Some(NamedNodePattern::Variable(v)) = graph_name {
                        final_variables.push(v.clone());
                    }
                    let base_pattern = self.rewrite_quad_pattern(
                        &s.clone().into(),
                        &p.into(),
                        &o.clone().into(),
                        graph_name,
                        &mut Vec::new(),
                    );
                    FixedPointGraphPattern::project(
                        FixedPointGraphPattern::union(
                            Self::copy_variable(
                                Self::copy_variable(
                                    base_pattern.clone(),
                                    s.clone().into(),
                                    subject.clone(),
                                ),
                                s.into(),
                                object.clone(),
                            ),
                            Self::copy_variable(
                                Self::copy_variable(
                                    base_pattern,
                                    o.clone().into(),
                                    subject.clone(),
                                ),
                                o.into(),
                                object.clone(),
                            ),
                        ),
                        final_variables,
                    )
                }
            },
        }
    }

    fn one_or_more_pattern(
        &self,
        subject: &GroundTermPattern,
        path: &PropertyPathExpression,
        object: &GroundTermPattern,
        graph_name: Option<&NamedNodePattern>,
        fix_point_counter: usize,
    ) -> FixedPointGraphPattern {
        let mut final_variables = Vec::new();
        Self::add_pattern_variables(subject, &mut final_variables);
        Self::add_pattern_variables(object, &mut final_variables);
        if let Some(NamedNodePattern::Variable(v)) = graph_name {
            final_variables.push(v.clone());
        }

        let fix_point_id = FixedPointId(fix_point_counter);
        let start_var = new_var();
        let middle_var = new_var();
        let end_var = new_var();
        let mut in_loop_variables = vec![start_var.clone(), end_var.clone()];
        if let Some(NamedNodePattern::Variable(v)) = graph_name {
            in_loop_variables.push(v.clone());
        }
        let mut middle_variables = vec![start_var.clone(), middle_var.clone()];
        if let Some(NamedNodePattern::Variable(v)) = graph_name {
            middle_variables.push(v.clone());
        }
        FixedPointGraphPattern::project(
            Self::pattern_mapping(
                Self::pattern_mapping(
                    FixedPointGraphPattern::fixed_point(
                        FixedPointId(fix_point_counter),
                        FixedPointGraphPattern::union(
                            self.rewrite_property_path(
                                &start_var.clone().into(),
                                path,
                                &end_var.clone().into(),
                                graph_name,
                                fix_point_counter + 1,
                            ),
                            FixedPointGraphPattern::project(
                                FixedPointGraphPattern::join(
                                    FixedPointGraphPattern::project(
                                        Self::copy_variable(
                                            FixedPointGraphPattern::FixedPointEntry(fix_point_id),
                                            end_var.clone().into(),
                                            middle_var.clone(),
                                        ),
                                        middle_variables,
                                    ),
                                    self.rewrite_property_path(
                                        &middle_var.into(),
                                        path,
                                        &end_var.clone().into(),
                                        graph_name,
                                        fix_point_counter + 1,
                                    ),
                                ),
                                in_loop_variables.clone(),
                            ),
                        ),
                        in_loop_variables,
                    ),
                    start_var.into(),
                    subject.clone(),
                ),
                end_var.into(),
                object.clone(),
            ),
            final_variables,
        )
    }

    fn pattern_mapping(
        pattern: FixedPointGraphPattern,
        pattern_value: FixedPointExpression,
        target: GroundTermPattern,
    ) -> FixedPointGraphPattern {
        match target {
            GroundTermPattern::NamedNode(target) => FixedPointGraphPattern::filter(
                pattern,
                FixedPointExpression::same_term(pattern_value, target.into()),
            ),
            GroundTermPattern::Literal(target) => FixedPointGraphPattern::filter(
                pattern,
                FixedPointExpression::same_term(target.into(), pattern_value),
            ),
            GroundTermPattern::Triple(target) => Self::pattern_mapping(
                Self::pattern_mapping(
                    match &target.predicate {
                        NamedNodePattern::NamedNode(target_predicate) => {
                            FixedPointGraphPattern::filter(
                                pattern,
                                FixedPointExpression::same_term(
                                    FixedPointExpression::call(
                                        Function::Predicate,
                                        vec![pattern_value.clone()],
                                    ),
                                    target_predicate.clone().into(),
                                ),
                            )
                        }
                        NamedNodePattern::Variable(target_predicate) => Self::copy_variable(
                            pattern,
                            FixedPointExpression::call(
                                Function::Predicate,
                                vec![pattern_value.clone()],
                            ),
                            target_predicate.clone(),
                        ),
                    },
                    FixedPointExpression::call(Function::Subject, vec![pattern_value.clone()]),
                    target.subject,
                ),
                FixedPointExpression::call(Function::Object, vec![pattern_value]),
                target.object,
            ),
            GroundTermPattern::Variable(target) => {
                Self::copy_variable(pattern, pattern_value, target)
            }
        }
    }

    fn copy_variable(
        pattern: FixedPointGraphPattern,
        from_expression: FixedPointExpression,
        to_variable: Variable,
    ) -> FixedPointGraphPattern {
        if from_expression == FixedPointExpression::from(to_variable.clone()) {
            return pattern;
        }
        let mut does_target_exists = false;
        pattern.lookup_used_variables(&mut |v| {
            if *v == to_variable {
                does_target_exists = true;
            }
        });
        if does_target_exists {
            FixedPointGraphPattern::filter(
                pattern,
                FixedPointExpression::same_term(from_expression, to_variable.into()),
            )
        } else {
            FixedPointGraphPattern::extend(pattern, to_variable, from_expression)
        }
    }

    fn add_pattern_variables(pattern: &GroundTermPattern, variables: &mut Vec<Variable>) {
        if let GroundTermPattern::Variable(v) = pattern {
            variables.push(v.clone())
        } else if let GroundTermPattern::Triple(t) = pattern {
            Self::add_pattern_variables(&t.subject, variables);
            if let NamedNodePattern::Variable(v) = &t.predicate {
                variables.push(v.clone());
            }
            Self::add_pattern_variables(&t.object, variables);
        }
    }

    /// Attempts to use a given rule to get new facts for a triple pattern
    fn apply_rule_on_quad_pattern(
        &self,
        subject: &GroundTermPattern,
        predicate: &NamedNodePattern,
        object: &GroundTermPattern,
        graph_name: Option<&NamedNodePattern>,
        head: &GroundTriplePattern,
        body: &[GroundTriplePattern],
        possible_fixed_points: &mut Vec<(
            GroundTermPattern,
            NamedNodePattern,
            GroundTermPattern,
            Option<NamedNodePattern>,
            FixedPointId,
        )>,
    ) -> Option<FixedPointGraphPattern> {
        let head_unification = Self::unify_triple_pattern(
            subject.clone(),
            head.subject.clone(),
            predicate.clone(),
            head.predicate.clone(),
            object.clone(),
            head.object.clone(),
        )?;
        // We build a nested query
        // from is the parent query and to is the nested one
        let mut replacements_in_rule = HashMap::new();
        let mut final_binds = Vec::new();
        for replacement in head_unification {
            match replacement {
                Replacement::ConstToVar { from, to } => match replacements_in_rule.entry(to) {
                    Entry::Vacant(e) => {
                        e.insert(TermOrVariable::Term(from));
                    }
                    Entry::Occupied(mut e) => match e.get() {
                        TermOrVariable::Term(c) => {
                            if from != *c {
                                return None; //Conflict
                            }
                        }
                        TermOrVariable::Variable(v) => {
                            final_binds.push((v.clone(), TermOrVariable::Term(from.clone())));
                            e.insert(TermOrVariable::Term(from));
                        }
                    },
                },
                Replacement::VarToConst { from, to } => {
                    final_binds.push((from, TermOrVariable::Term(to)));
                }
                Replacement::VarToVar { from, to } => match replacements_in_rule.entry(to) {
                    Entry::Vacant(e) => {
                        e.insert(TermOrVariable::Variable(from));
                    }
                    Entry::Occupied(e) => final_binds.push((from, e.get().clone())),
                },
            }
        }
        let mut plan = self.rewrite_rule_body(
            body,
            graph_name,
            &mut replacements_in_rule,
            possible_fixed_points,
        )?;
        for (variable, value) in final_binds {
            plan = FixedPointGraphPattern::extend(
                plan,
                variable,
                match value {
                    TermOrVariable::Term(v) => v.into(),
                    TermOrVariable::Variable(v) => v.into(),
                },
            );
        }
        Some(plan)
    }

    fn rewrite_rule_body<'a>(
        &self,
        body: &'a [GroundTriplePattern],
        parent_graph_name: Option<&'a NamedNodePattern>,
        replacements_in_rule: &mut HashMap<Variable, TermOrVariable>,
        possible_fixed_points: &mut Vec<(
            GroundTermPattern,
            NamedNodePattern,
            GroundTermPattern,
            Option<NamedNodePattern>,
            FixedPointId,
        )>,
    ) -> Option<FixedPointGraphPattern> {
        let mut patterns = Vec::new();
        for p in body {
            patterns.push(self.rewrite_quad_pattern(
                &Self::apply_replacement_on_term_pattern(&p.subject, replacements_in_rule)?,
                &Self::apply_replacement_on_named_node_pattern(&p.predicate, replacements_in_rule)?,
                &Self::apply_replacement_on_term_pattern(&p.object, replacements_in_rule)?,
                parent_graph_name,
                possible_fixed_points,
            ));
        }
        Some(
            patterns
                .into_iter()
                .reduce(FixedPointGraphPattern::join)
                .unwrap_or_else(FixedPointGraphPattern::singleton),
        )
    }

    fn apply_replacement_on_named_node_pattern(
        pattern: &NamedNodePattern,
        replacements: &mut HashMap<Variable, TermOrVariable>,
    ) -> Option<NamedNodePattern> {
        Some(match pattern {
            NamedNodePattern::NamedNode(node) => NamedNodePattern::NamedNode(node.clone()),
            NamedNodePattern::Variable(variable) => {
                match replacements
                    .entry(variable.clone())
                    .or_insert_with(|| TermOrVariable::Variable(new_var()))
                {
                    TermOrVariable::Term(c) => {
                        if let GroundTerm::NamedNode(node) = c {
                            NamedNodePattern::NamedNode(node.clone())
                        } else {
                            return None;
                        }
                    }
                    TermOrVariable::Variable(v) => NamedNodePattern::Variable(v.clone()),
                }
            }
        })
    }

    fn apply_replacement_on_term_pattern(
        pattern: &GroundTermPattern,
        replacements: &mut HashMap<Variable, TermOrVariable>,
    ) -> Option<GroundTermPattern> {
        Some(match pattern {
            GroundTermPattern::NamedNode(node) => node.clone().into(),
            GroundTermPattern::Literal(literal) => literal.clone().into(),
            GroundTermPattern::Triple(triple) => GroundTriplePattern {
                subject: Self::apply_replacement_on_term_pattern(&triple.subject, replacements)?,
                predicate: Self::apply_replacement_on_named_node_pattern(
                    &triple.predicate,
                    replacements,
                )?,
                object: Self::apply_replacement_on_term_pattern(&triple.subject, replacements)?,
            }
            .into(),
            GroundTermPattern::Variable(variable) => {
                match replacements
                    .entry(variable.clone())
                    .or_insert_with(|| TermOrVariable::Variable(new_var()))
                {
                    TermOrVariable::Term(c) => c.clone().into(),
                    TermOrVariable::Variable(v) => v.clone().into(),
                }
            }
        })
    }

    fn unify_triple_pattern(
        from_subject: GroundTermPattern,
        to_subject: GroundTermPattern,
        from_predicate: NamedNodePattern,
        to_predicate: NamedNodePattern,
        from_object: GroundTermPattern,
        to_object: GroundTermPattern,
    ) -> Option<Vec<Replacement>> {
        let mut mapping = Self::unify_ground_term_pattern(from_subject, to_subject)?;
        mapping.extend(Self::unify_named_node_pattern(
            from_predicate,
            to_predicate,
        )?);
        mapping.extend(Self::unify_ground_term_pattern(from_object, to_object)?);
        Some(mapping)
    }

    fn unify_named_node_pattern(
        from: NamedNodePattern,
        to: NamedNodePattern,
    ) -> Option<Vec<Replacement>> {
        match from {
            NamedNodePattern::NamedNode(from) => match to {
                NamedNodePattern::NamedNode(to) => {
                    if from == to {
                        Some(Vec::new())
                    } else {
                        None
                    }
                }
                NamedNodePattern::Variable(to) => Some(vec![Replacement::ConstToVar {
                    from: from.into(),
                    to,
                }]),
            },
            NamedNodePattern::Variable(from) => match to {
                NamedNodePattern::NamedNode(to) => Some(vec![Replacement::VarToConst {
                    from,
                    to: to.into(),
                }]),
                NamedNodePattern::Variable(to) => Some(vec![Replacement::VarToVar { from, to }]),
            },
        }
    }

    fn unify_ground_term_pattern(
        from: GroundTermPattern,
        to: GroundTermPattern,
    ) -> Option<Vec<Replacement>> {
        match from {
            GroundTermPattern::NamedNode(from) => match to {
                GroundTermPattern::NamedNode(to) => {
                    if from == to {
                        Some(Vec::new())
                    } else {
                        None
                    }
                }
                GroundTermPattern::Literal(_) | GroundTermPattern::Triple(_) => None,
                GroundTermPattern::Variable(to) => Some(vec![Replacement::ConstToVar {
                    from: from.into(),
                    to,
                }]),
            },
            GroundTermPattern::Literal(from) => match to {
                GroundTermPattern::NamedNode(_) | GroundTermPattern::Triple(_) => None,
                GroundTermPattern::Literal(to) => {
                    if from == to {
                        Some(Vec::new())
                    } else {
                        None
                    }
                }
                GroundTermPattern::Variable(to) => Some(vec![Replacement::ConstToVar {
                    from: from.into(),
                    to,
                }]),
            },
            GroundTermPattern::Triple(_) => unimplemented!(),
            GroundTermPattern::Variable(from) => match to {
                GroundTermPattern::NamedNode(to) => Some(vec![Replacement::VarToConst {
                    from,
                    to: to.into(),
                }]),
                GroundTermPattern::Literal(to) => Some(vec![Replacement::VarToConst {
                    from,
                    to: to.into(),
                }]),
                GroundTermPattern::Triple(_) => unimplemented!(),
                GroundTermPattern::Variable(to) => Some(vec![Replacement::VarToVar { from, to }]),
            },
        }
    }
}

#[derive(Clone)]
enum Replacement {
    VarToConst { from: Variable, to: GroundTerm },
    ConstToVar { from: GroundTerm, to: Variable },
    VarToVar { from: Variable, to: Variable },
}

#[derive(Clone)]
enum TermOrVariable {
    Term(GroundTerm),
    Variable(Variable),
}

fn new_var() -> Variable {
    Variable::new_unchecked(format!("{:x}", random::<u128>()))
}
