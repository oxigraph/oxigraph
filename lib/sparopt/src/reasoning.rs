//! Reasoning via query rewriting

use crate::algebra::*;
use rand::random;
use spargebra::term::GroundTriplePattern;
use spargebra::RuleSet;
use std::collections::hash_map::Entry;
use std::collections::HashMap;

pub struct QueryRewriter {
    rules: Vec<(Vec<GroundTriplePattern>, GraphPattern)>,
}

impl QueryRewriter {
    pub fn new(rule_set: RuleSet) -> Self {
        Self {
            rules: rule_set
                .rules
                .into_iter()
                .map(|rule| (rule.head, (&rule.body).into()))
                .collect(),
        }
    }

    pub fn rewrite_graph_pattern(&self, pattern: &GraphPattern) -> GraphPattern {
        //TODO: rewrite EXISTS
        match pattern {
            GraphPattern::QuadPattern {
                subject,
                predicate,
                object,
                graph_name,
            } => self.rewrite_quad_pattern(subject, predicate, object, graph_name.as_ref()),
            GraphPattern::Path { .. } => todo!(),
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
            GraphPattern::Filter { inner, expression } => {
                GraphPattern::filter(self.rewrite_graph_pattern(inner), expression.clone())
            }
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
                expression.clone(),
            ),
            GraphPattern::Minus { left, right } => GraphPattern::minus(
                self.rewrite_graph_pattern(left),
                self.rewrite_graph_pattern(right),
            ),
            GraphPattern::Values {
                variables,
                bindings,
            } => GraphPattern::values(variables.clone(), bindings.clone()),
            GraphPattern::OrderBy { inner, expression } => {
                GraphPattern::order_by(self.rewrite_graph_pattern(inner), expression.clone())
            }
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
                aggregates.clone(),
            ),
            GraphPattern::Service {
                inner,
                silent,
                name,
            } => GraphPattern::service(self.rewrite_graph_pattern(inner), name.clone(), *silent),
        }
    }

    fn rewrite_quad_pattern(
        &self,
        subject: &GroundTermPattern,
        predicate: &NamedNodePattern,
        object: &GroundTermPattern,
        graph_name: Option<&NamedNodePattern>,
    ) -> GraphPattern {
        // We rewrite based on rules
        let mut graph_pattern = GraphPattern::QuadPattern {
            subject: subject.clone(),
            predicate: predicate.clone(),
            object: object.clone(),
            graph_name: graph_name.cloned(),
        };
        for (rule_head, rule_body) in &self.rules {
            for head_pattern in rule_head {
                if let Some(nested) = self.apply_rule_for_quad_pattern(
                    subject,
                    predicate,
                    object,
                    graph_name,
                    head_pattern,
                    rule_body,
                ) {
                    graph_pattern = GraphPattern::union(graph_pattern, nested);
                }
            }
        }
        graph_pattern
    }

    /// Attempts to use a given rule to get new facts for a triple pattern
    fn apply_rule_for_quad_pattern(
        &self,
        subject: &GroundTermPattern,
        predicate: &NamedNodePattern,
        object: &GroundTermPattern,
        graph_name: Option<&NamedNodePattern>,
        head: &GroundTriplePattern,
        body: &GraphPattern,
    ) -> Option<GraphPattern> {
        let head_unification = Self::unify_triple_pattern(
            subject.clone(),
            predicate.clone(),
            object.clone(),
            head.clone(),
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
        let mut plan = self.rewrite_rule_body(body, graph_name, &mut replacements_in_rule)?;
        for (variable, value) in final_binds {
            plan = GraphPattern::extend(
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
        pattern: &'a GraphPattern,
        parent_graph_name: Option<&'a NamedNodePattern>,
        replacements_in_rule: &mut HashMap<Variable, TermOrVariable>,
    ) -> Option<GraphPattern> {
        Some(match pattern {
            GraphPattern::QuadPattern {
                subject,
                predicate,
                object,
                graph_name,
            } => self.rewrite_quad_pattern(
                &Self::apply_replacement_on_term_pattern(subject, replacements_in_rule)?,
                &Self::apply_replacement_on_named_node_pattern(predicate, replacements_in_rule)?,
                &Self::apply_replacement_on_term_pattern(object, replacements_in_rule)?,
                if let Some(graph_name) = graph_name {
                    Some(Self::apply_replacement_on_named_node_pattern(
                        graph_name,
                        replacements_in_rule,
                    )?)
                } else {
                    parent_graph_name.cloned()
                }
                .as_ref(),
            ),
            GraphPattern::Join { left, right } => GraphPattern::join(
                self.rewrite_rule_body(left, parent_graph_name, replacements_in_rule)?,
                self.rewrite_rule_body(right, parent_graph_name, replacements_in_rule)?,
            ),
            GraphPattern::Values {
                variables,
                bindings,
            } => {
                let variable_mapping = variables
                    .iter()
                    .map(|v| {
                        replacements_in_rule
                            .entry(v.clone())
                            .or_insert_with(|| TermOrVariable::Variable(new_var()))
                            .clone()
                    })
                    .collect::<Vec<_>>();
                GraphPattern::Values {
                    variables: variable_mapping
                        .iter()
                        .filter_map(|v| match v {
                            TermOrVariable::Term(_) => None,
                            TermOrVariable::Variable(v) => Some(v.clone()),
                        })
                        .collect(),
                    bindings: bindings
                        .iter()
                        .filter_map(|binding| {
                            let mut new_binding = Vec::with_capacity(binding.len());
                            for (variable, value) in variable_mapping.iter().zip(binding) {
                                match variable {
                                    TermOrVariable::Variable(_) => new_binding.push(value.clone()),
                                    TermOrVariable::Term(cst) => {
                                        let compatible = if let Some(value) = value {
                                            cst == value
                                        } else {
                                            true
                                        };
                                        if !compatible {
                                            return None;
                                        }
                                    }
                                }
                            }
                            Some(new_binding)
                        })
                        .collect(),
                }
            }
            _ => unreachable!("Not allowed by the parser yet: {pattern:?}"),
        })
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
        from_predicate: NamedNodePattern,
        from_object: GroundTermPattern,
        to: GroundTriplePattern,
    ) -> Option<Vec<Replacement>> {
        let mut mapping = Self::unify_ground_term_pattern(from_subject, to.subject)?;
        mapping.extend(Self::unify_named_node_pattern(
            from_predicate,
            to.predicate,
        )?);
        mapping.extend(Self::unify_ground_term_pattern(from_object, to.object)?);
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
                GroundTermPattern::NamedNode(_) => None,
                GroundTermPattern::Literal(to) => {
                    if from == to {
                        Some(Vec::new())
                    } else {
                        None
                    }
                }
                GroundTermPattern::Triple(_) => None,
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
