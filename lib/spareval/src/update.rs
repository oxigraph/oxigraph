use crate::{QueryEvaluationError, QuerySolutionIter};
#[cfg(feature = "sparql-12")]
use oxrdf::Triple;
use oxrdf::{BlankNode, GraphName, NamedNode, Quad, Term, Variable};
use rustc_hash::FxHashMap;
use sparesults::QuerySolution;
use spargebra::term::{
    GraphNamePattern, GroundQuadPattern, GroundTermPattern, NamedNodePattern, QuadPattern,
    TermPattern,
};
#[cfg(feature = "sparql-12")]
use spargebra::term::{GroundTriplePattern, TriplePattern};
use std::collections::VecDeque;

/// A [`Quad`] to delete or insert.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum DeleteInsertQuad {
    Delete(Quad),
    Insert(Quad),
}

/// Output of [`PreparedDeleteInsertUpdate::execute`](super::PreparedDeleteInsertUpdate::execute).
pub struct DeleteInsertIter<'a, 'b> {
    solutions: QuerySolutionIter<'a>,
    ground_delete: Vec<Quad>,
    variable_delete: Vec<&'b GroundQuadPattern>,
    ground_insert: Vec<Quad>,
    variable_insert: Vec<&'b QuadPattern>,
    buffer: VecDeque<DeleteInsertQuad>,
    bnodes: FxHashMap<BlankNode, BlankNode>,
    ground_deletes_emitted: bool,
    ground_inserts_emitted: bool,
    had_solution: bool,
}

impl<'a, 'b> DeleteInsertIter<'a, 'b> {
    pub(crate) fn new(
        solutions: QuerySolutionIter<'a>,
        delete: &'b [GroundQuadPattern],
        insert: &'b [QuadPattern],
    ) -> Self {
        let empty_solution =
            QuerySolution::from((Vec::<Variable>::new(), Vec::<Option<Term>>::new()));
        let mut ground_delete = Vec::new();
        let mut variable_delete = Vec::new();
        for quad in delete {
            if ground_quad_pattern_has_variable(quad) {
                variable_delete.push(quad);
            } else if let Some(quad) = fill_ground_quad_pattern(quad, &empty_solution) {
                ground_delete.push(quad);
            }
        }

        let mut ground_insert = Vec::new();
        let mut variable_insert = Vec::new();
        let mut bnodes = FxHashMap::default();
        for quad in insert {
            if insert_quad_pattern_needs_solution(quad) {
                variable_insert.push(quad);
            } else if let Some(quad) = fill_quad_pattern(quad, &empty_solution, &mut bnodes) {
                ground_insert.push(quad);
            }
        }

        Self {
            solutions,
            ground_delete,
            variable_delete,
            ground_insert,
            variable_insert,
            buffer: VecDeque::new(),
            bnodes: FxHashMap::default(),
            ground_deletes_emitted: false,
            ground_inserts_emitted: false,
            had_solution: false,
        }
    }
}

impl Iterator for DeleteInsertIter<'_, '_> {
    type Item = Result<DeleteInsertQuad, QueryEvaluationError>;

    fn next(&mut self) -> Option<Result<DeleteInsertQuad, QueryEvaluationError>> {
        loop {
            if let Some(quad) = self.buffer.pop_front() {
                return Some(Ok(quad));
            }
            let solution = match self.solutions.next() {
                Some(Ok(solution)) => solution,
                Some(Err(e)) => return Some(Err(e)),
                None => {
                    if self.had_solution && !self.ground_inserts_emitted {
                        for quad in &self.ground_insert {
                            self.buffer
                                .push_back(DeleteInsertQuad::Insert(quad.clone()));
                        }
                        self.ground_inserts_emitted = true;
                        continue;
                    }
                    return None;
                }
            };
            self.had_solution = true;
            if !self.ground_deletes_emitted {
                for quad in &self.ground_delete {
                    self.buffer
                        .push_back(DeleteInsertQuad::Delete(quad.clone()));
                }
                self.ground_deletes_emitted = true;
            }
            for quad in &self.variable_delete {
                if let Some(quad) = fill_ground_quad_pattern(quad, &solution) {
                    self.buffer.push_back(DeleteInsertQuad::Delete(quad));
                }
            }
            for quad in &self.variable_insert {
                if let Some(quad) = fill_quad_pattern(quad, &solution, &mut self.bnodes) {
                    self.buffer.push_back(DeleteInsertQuad::Insert(quad));
                }
            }
            self.bnodes.clear();
        }
    }
}

fn ground_quad_pattern_has_variable(quad: &GroundQuadPattern) -> bool {
    ground_term_pattern_has_variable(&quad.subject)
        || named_node_pattern_has_variable(&quad.predicate)
        || ground_term_pattern_has_variable(&quad.object)
        || graph_name_pattern_has_variable(&quad.graph_name)
}

fn ground_term_pattern_has_variable(term: &GroundTermPattern) -> bool {
    match term {
        GroundTermPattern::NamedNode(_) | GroundTermPattern::Literal(_) => false,
        #[cfg(feature = "sparql-12")]
        GroundTermPattern::Triple(triple) => ground_triple_pattern_has_variable(triple),
        GroundTermPattern::Variable(_) => true,
    }
}

#[cfg(feature = "sparql-12")]
fn ground_triple_pattern_has_variable(triple: &GroundTriplePattern) -> bool {
    ground_term_pattern_has_variable(&triple.subject)
        || named_node_pattern_has_variable(&triple.predicate)
        || ground_term_pattern_has_variable(&triple.object)
}

fn insert_quad_pattern_needs_solution(quad: &QuadPattern) -> bool {
    insert_term_pattern_needs_solution(&quad.subject)
        || named_node_pattern_has_variable(&quad.predicate)
        || insert_term_pattern_needs_solution(&quad.object)
        || graph_name_pattern_has_variable(&quad.graph_name)
}

fn insert_term_pattern_needs_solution(term: &TermPattern) -> bool {
    match term {
        TermPattern::NamedNode(_) | TermPattern::Literal(_) => false,
        TermPattern::BlankNode(_) | TermPattern::Variable(_) => true,
        #[cfg(feature = "sparql-12")]
        TermPattern::Triple(triple) => triple_pattern_needs_solution(triple),
    }
}

#[cfg(feature = "sparql-12")]
fn triple_pattern_needs_solution(triple: &TriplePattern) -> bool {
    insert_term_pattern_needs_solution(&triple.subject)
        || named_node_pattern_has_variable(&triple.predicate)
        || insert_term_pattern_needs_solution(&triple.object)
}

fn named_node_pattern_has_variable(term: &NamedNodePattern) -> bool {
    matches!(term, NamedNodePattern::Variable(_))
}

fn graph_name_pattern_has_variable(term: &GraphNamePattern) -> bool {
    matches!(term, GraphNamePattern::Variable(_))
}

fn fill_quad_pattern(
    quad: &QuadPattern,
    solution: &QuerySolution,
    bnodes: &mut FxHashMap<BlankNode, BlankNode>,
) -> Option<Quad> {
    Some(Quad {
        subject: match fill_term_or_var(&quad.subject, solution, bnodes)? {
            Term::NamedNode(node) => node.into(),
            Term::BlankNode(node) => node.into(),
            #[cfg(feature = "sparql-12")]
            Term::Triple(_) => return None,
            Term::Literal(_) => return None,
        },
        predicate: fill_named_node_or_var(&quad.predicate, solution)?,
        object: fill_term_or_var(&quad.object, solution, bnodes)?,
        graph_name: fill_graph_name_or_var(&quad.graph_name, solution)?,
    })
}

fn fill_term_or_var(
    term: &TermPattern,
    solution: &QuerySolution,
    bnodes: &mut FxHashMap<BlankNode, BlankNode>,
) -> Option<Term> {
    Some(match term {
        TermPattern::NamedNode(term) => term.clone().into(),
        TermPattern::BlankNode(bnode) => convert_blank_node(bnode, bnodes).into(),
        TermPattern::Literal(term) => term.clone().into(),
        #[cfg(feature = "sparql-12")]
        TermPattern::Triple(triple) => fill_triple_pattern(triple, solution, bnodes)?.into(),
        TermPattern::Variable(v) => solution.get(v)?.clone(),
    })
}

fn fill_named_node_or_var(term: &NamedNodePattern, solution: &QuerySolution) -> Option<NamedNode> {
    Some(match term {
        NamedNodePattern::NamedNode(term) => term.clone(),
        NamedNodePattern::Variable(v) => {
            if let Term::NamedNode(s) = solution.get(v)? {
                s.clone()
            } else {
                return None;
            }
        }
    })
}

fn fill_graph_name_or_var(term: &GraphNamePattern, solution: &QuerySolution) -> Option<GraphName> {
    Some(match term {
        GraphNamePattern::NamedNode(term) => term.clone().into(),
        GraphNamePattern::DefaultGraph => GraphName::DefaultGraph,
        GraphNamePattern::Variable(v) => match solution.get(v)? {
            Term::NamedNode(node) => node.clone().into(),
            Term::BlankNode(node) => node.clone().into(),
            Term::Literal(_) => return None,
            #[cfg(feature = "sparql-12")]
            Term::Triple(_) => return None,
        },
    })
}

#[cfg(feature = "sparql-12")]
fn fill_triple_pattern(
    triple: &TriplePattern,
    solution: &QuerySolution,
    bnodes: &mut FxHashMap<BlankNode, BlankNode>,
) -> Option<Triple> {
    Some(Triple {
        subject: match fill_term_or_var(&triple.subject, solution, bnodes)? {
            Term::NamedNode(node) => node.into(),
            Term::BlankNode(node) => node.into(),
            #[cfg(feature = "sparql-12")]
            Term::Triple(_) => return None,
            Term::Literal(_) => return None,
        },
        predicate: fill_named_node_or_var(&triple.predicate, solution)?,
        object: fill_term_or_var(&triple.object, solution, bnodes)?,
    })
}
fn fill_ground_quad_pattern(quad: &GroundQuadPattern, solution: &QuerySolution) -> Option<Quad> {
    Some(Quad {
        subject: match fill_ground_term_or_var(&quad.subject, solution)? {
            Term::NamedNode(node) => node.into(),
            Term::BlankNode(node) => node.into(),
            #[cfg(feature = "sparql-12")]
            Term::Triple(_) => return None,
            Term::Literal(_) => return None,
        },
        predicate: fill_named_node_or_var(&quad.predicate, solution)?,
        object: fill_ground_term_or_var(&quad.object, solution)?,
        graph_name: fill_graph_name_or_var(&quad.graph_name, solution)?,
    })
}

fn fill_ground_term_or_var(term: &GroundTermPattern, solution: &QuerySolution) -> Option<Term> {
    Some(match term {
        GroundTermPattern::NamedNode(term) => term.clone().into(),
        GroundTermPattern::Literal(term) => term.clone().into(),
        #[cfg(feature = "sparql-12")]
        GroundTermPattern::Triple(triple) => fill_ground_triple_pattern(triple, solution)?.into(),
        GroundTermPattern::Variable(v) => solution.get(v)?.clone(),
    })
}

#[cfg(feature = "sparql-12")]
fn fill_ground_triple_pattern(
    triple: &GroundTriplePattern,
    solution: &QuerySolution,
) -> Option<Triple> {
    Some(Triple {
        subject: match fill_ground_term_or_var(&triple.subject, solution)? {
            Term::NamedNode(node) => node.into(),
            Term::BlankNode(node) => node.into(),
            #[cfg(feature = "sparql-12")]
            Term::Triple(_) => return None,
            Term::Literal(_) => return None,
        },
        predicate: fill_named_node_or_var(&triple.predicate, solution)?,
        object: fill_ground_term_or_var(&triple.object, solution)?,
    })
}

fn convert_blank_node(node: &BlankNode, bnodes: &mut FxHashMap<BlankNode, BlankNode>) -> BlankNode {
    bnodes.entry(node.clone()).or_default().clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::{Literal, Variable};
    use std::sync::Arc;

    fn ex(name: &str) -> NamedNode {
        NamedNode::new_unchecked(format!("http://example.com/{name}"))
    }

    fn delete_pattern(
        subject: impl Into<GroundTermPattern>,
        predicate: NamedNode,
        object: impl Into<GroundTermPattern>,
    ) -> GroundQuadPattern {
        GroundQuadPattern {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
            graph_name: GraphNamePattern::DefaultGraph,
        }
    }

    fn insert_pattern(
        subject: impl Into<TermPattern>,
        predicate: NamedNode,
        object: impl Into<TermPattern>,
    ) -> QuadPattern {
        QuadPattern {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
            graph_name: GraphNamePattern::DefaultGraph,
        }
    }

    fn solution_iter(
        variables: Vec<Variable>,
        rows: Vec<Vec<Option<Term>>>,
    ) -> QuerySolutionIter<'static> {
        let variables: Arc<[Variable]> = variables.into();
        QuerySolutionIter::from_tuples(
            variables,
            rows.into_iter().map(Ok::<_, QueryEvaluationError>),
        )
    }

    fn collect(
        delete: &[GroundQuadPattern],
        insert: &[QuadPattern],
        variables: Vec<Variable>,
        rows: Vec<Vec<Option<Term>>>,
    ) -> Result<Vec<DeleteInsertQuad>, QueryEvaluationError> {
        DeleteInsertIter::new(solution_iter(variables, rows), delete, insert).collect()
    }

    fn deleted_quads(results: &[DeleteInsertQuad]) -> Vec<Quad> {
        results
            .iter()
            .filter_map(|result| match result {
                DeleteInsertQuad::Delete(quad) => Some(quad.clone()),
                DeleteInsertQuad::Insert(_) => None,
            })
            .collect()
    }

    fn inserted_quads(results: &[DeleteInsertQuad]) -> Vec<Quad> {
        results
            .iter()
            .filter_map(|result| match result {
                DeleteInsertQuad::Delete(_) => None,
                DeleteInsertQuad::Insert(quad) => Some(quad.clone()),
            })
            .collect()
    }

    #[test]
    fn ground_insert_is_emitted_once_with_multiple_solutions() -> Result<(), QueryEvaluationError> {
        let s = Variable::new_unchecked("s");
        let p = ex("p");
        let old = ex("old");
        let inserted = Quad::new(
            ex("ground"),
            p.clone(),
            ex("inserted"),
            GraphName::DefaultGraph,
        );
        let delete = [delete_pattern(s.clone(), p.clone(), old.clone())];
        let insert = [insert_pattern(ex("ground"), p, ex("inserted"))];

        let results = collect(
            &delete,
            &insert,
            vec![s],
            vec![vec![Some(ex("s1").into())], vec![Some(ex("s2").into())]],
        )?;

        assert_eq!(deleted_quads(&results).len(), 2);
        assert_eq!(inserted_quads(&results), vec![inserted]);
        Ok(())
    }

    #[test]
    fn ground_insert_is_not_emitted_without_solutions() -> Result<(), QueryEvaluationError> {
        let p = ex("p");
        let delete = [];
        let insert = [insert_pattern(ex("ground"), p, ex("inserted"))];

        let results = collect(&delete, &insert, Vec::new(), Vec::new())?;

        assert!(results.is_empty());
        Ok(())
    }

    #[test]
    fn ground_insert_is_emitted_after_later_variable_deletes() -> Result<(), QueryEvaluationError> {
        let s = Variable::new_unchecked("s");
        let p = ex("p");
        let object = ex("o");
        let first_deleted = Quad::new(ex("a"), p.clone(), object.clone(), GraphName::DefaultGraph);
        let reinserted = Quad::new(ex("z"), p.clone(), object.clone(), GraphName::DefaultGraph);
        let delete = [delete_pattern(s.clone(), p.clone(), object)];
        let insert = [insert_pattern(ex("z"), p, ex("o"))];

        let results = collect(
            &delete,
            &insert,
            vec![s],
            vec![vec![Some(ex("a").into())], vec![Some(ex("z").into())]],
        )?;

        assert_eq!(
            results,
            vec![
                DeleteInsertQuad::Delete(first_deleted),
                DeleteInsertQuad::Delete(reinserted.clone()),
                DeleteInsertQuad::Insert(reinserted),
            ]
        );
        Ok(())
    }

    #[test]
    fn insert_with_blank_node_is_emitted_for_each_solution() -> Result<(), QueryEvaluationError> {
        let p = ex("p");
        let delete = [];
        let insert = [insert_pattern(
            BlankNode::new_unchecked("b"),
            p,
            Literal::from("value"),
        )];

        let results = collect(&delete, &insert, Vec::new(), vec![Vec::new(), Vec::new()])?;
        let inserted = inserted_quads(&results);

        assert_eq!(inserted.len(), 2);
        assert_ne!(inserted[0].subject, inserted[1].subject);
        Ok(())
    }

    #[test]
    fn mixed_insert_hoists_only_ground_templates() -> Result<(), QueryEvaluationError> {
        let o = Variable::new_unchecked("o");
        let p = ex("p");
        let ground_insert = Quad::new(
            ex("ground"),
            p.clone(),
            ex("ground-o"),
            GraphName::DefaultGraph,
        );
        let first_variable_insert =
            Quad::new(ex("variable"), p.clone(), ex("o1"), GraphName::DefaultGraph);
        let second_variable_insert =
            Quad::new(ex("variable"), p.clone(), ex("o2"), GraphName::DefaultGraph);
        let delete = [];
        let insert = [
            insert_pattern(ex("ground"), p.clone(), ex("ground-o")),
            insert_pattern(ex("variable"), p, o.clone()),
        ];

        let results = collect(
            &delete,
            &insert,
            vec![o],
            vec![vec![Some(ex("o1").into())], vec![Some(ex("o2").into())]],
        )?;
        let inserted = inserted_quads(&results);

        assert_eq!(inserted.len(), 3);
        assert_eq!(
            inserted
                .iter()
                .filter(|quad| **quad == ground_insert)
                .count(),
            1
        );
        assert!(inserted.contains(&first_variable_insert));
        assert!(inserted.contains(&second_variable_insert));
        Ok(())
    }

    #[test]
    fn ground_delete_is_emitted_once_with_multiple_solutions() -> Result<(), QueryEvaluationError> {
        let p = ex("p");
        let deleted = Quad::new(
            ex("ground"),
            p.clone(),
            ex("deleted"),
            GraphName::DefaultGraph,
        );
        let delete = [delete_pattern(ex("ground"), p, ex("deleted"))];
        let insert = [];

        let results = collect(&delete, &insert, Vec::new(), vec![Vec::new(), Vec::new()])?;

        assert_eq!(deleted_quads(&results), vec![deleted]);
        Ok(())
    }

    #[cfg(feature = "sparql-12")]
    #[test]
    fn nested_variable_in_quoted_triple_insert_is_not_hoisted() -> Result<(), QueryEvaluationError>
    {
        let o = Variable::new_unchecked("o");
        let p = ex("p");
        let quoted = TriplePattern {
            subject: ex("quoted-s").into(),
            predicate: p.clone().into(),
            object: o.clone().into(),
        };
        let delete = [];
        let insert = [insert_pattern(
            ex("subject"),
            p,
            TermPattern::Triple(Box::new(quoted)),
        )];

        let results = collect(
            &delete,
            &insert,
            vec![o],
            vec![vec![Some(ex("o1").into())], vec![Some(ex("o2").into())]],
        )?;
        let inserted = inserted_quads(&results);

        assert_eq!(inserted.len(), 2);
        assert_ne!(inserted[0].object, inserted[1].object);
        Ok(())
    }
}
